use chrono::{Datelike, Days, NaiveDate, Weekday};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use walkdir::WalkDir;
use zip::write::FileOptions;

pub fn validate_folder_name(raw: &str) -> Result<(), String> {
    if raw.is_empty() || raw.trim() != raw || matches!(raw, "." | "..") {
        return Err("归档目录名不能为空，且不能包含首尾空格、`.` 或 `..`".into());
    }
    if raw
        .chars()
        .any(|ch| ch < '\u{20}' || "<>:\"/\\|?*".contains(ch))
    {
        return Err("归档目录名包含 Windows 不允许的字符".into());
    }
    if raw.ends_with('.') || raw.ends_with(' ') || Path::new(raw).components().count() != 1 {
        return Err("归档目录名必须是单级 Windows 文件夹名".into());
    }
    let stem = raw.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    if reserved {
        return Err("归档目录名不能使用 Windows 保留设备名".into());
    }
    Ok(())
}

pub fn default_folder_name(today: NaiveDate, project_name: &str) -> String {
    let today_num = today.weekday().num_days_from_monday();
    let thursday_num = Weekday::Thu.num_days_from_monday();
    let offset = (thursday_num + 7 - today_num) % 7;
    let date = today
        .checked_add_days(Days::new(offset.into()))
        .expect("valid next Thursday");
    format!("{}-{project_name}", date.format("%Y%m%d"))
}

pub fn resolve_artifact_path(project_path: &Path, artifact_path: &str) -> PathBuf {
    let artifact = PathBuf::from(artifact_path);
    if artifact.is_absolute() {
        artifact
    } else {
        project_path.join(artifact)
    }
}

#[derive(Debug)]
pub enum ArchiveError {
    Cancelled,
    Failed(String),
}

pub struct ArchiveRequest {
    pub frontend_artifact: PathBuf,
    pub frontend_mode: String,
    pub backend_artifact: PathBuf,
    pub output_root: PathBuf,
    pub folder_name: String,
    pub run_id: String,
}

struct StagingGuard {
    path: PathBuf,
    committed: bool,
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn check_cancel(cancelled: &AtomicBool) -> Result<(), ArchiveError> {
    if cancelled.load(Ordering::Acquire) {
        Err(ArchiveError::Cancelled)
    } else {
        Ok(())
    }
}

fn source_name(path: &Path) -> Result<String, ArchiveError> {
    let name = path
        .file_name()
        .ok_or_else(|| ArchiveError::Failed(format!("无法确定产物名称：{}", path.display())))?;
    let name = name.to_str().ok_or_else(|| {
        ArchiveError::Failed(format!("产物名称不是有效 UTF-8：{}", path.display()))
    })?;
    if name.is_empty() {
        return Err(ArchiveError::Failed(format!(
            "无法确定产物名称：{}",
            path.display()
        )));
    }
    validate_folder_name(name)
        .map_err(|error| ArchiveError::Failed(format!("产物名称无效：{error}")))?;
    Ok(name.to_owned())
}

#[cfg(not(windows))]
fn comparison_key(value: &str) -> String {
    value.chars().flat_map(char::to_lowercase).collect()
}

#[cfg(windows)]
fn basename_eq_ignore_case(left: &str, right: &str) -> bool {
    use windows_sys::Win32::Foundation::TRUE;
    use windows_sys::Win32::Globalization::{CompareStringOrdinal, CSTR_EQUAL};

    let left: Vec<u16> = left.encode_utf16().collect();
    let right: Vec<u16> = right.encode_utf16().collect();
    unsafe {
        CompareStringOrdinal(
            left.as_ptr(),
            left.len() as i32,
            right.as_ptr(),
            right.len() as i32,
            TRUE,
        ) == CSTR_EQUAL
    }
}

#[cfg(not(windows))]
fn basename_eq_ignore_case(left: &str, right: &str) -> bool {
    comparison_key(left) == comparison_key(right)
}

fn zip_entry_name(
    root_name: &str,
    relative: &Path,
    source: &Path,
    destination_zip: &Path,
) -> Result<String, ArchiveError> {
    let mut parts = Vec::new();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(ArchiveError::Failed(format!(
                "ZIP 相对路径包含不支持的组件（源：{}，目标：{}）",
                source.display(),
                destination_zip.display()
            )));
        };
        let component = component.to_str().ok_or_else(|| {
            ArchiveError::Failed(format!(
                "ZIP entry 名称不是有效 UTF-8（源：{}，目标：{}）",
                source.display(),
                destination_zip.display()
            ))
        })?;
        parts.push(component);
    }
    if parts.is_empty() {
        Ok(root_name.to_owned())
    } else {
        Ok(format!("{root_name}/{}", parts.join("/")))
    }
}

fn io_error(
    operation: &str,
    source: &Path,
    target: &Path,
    error: impl std::fmt::Display,
) -> ArchiveError {
    ArchiveError::Failed(format!(
        "{operation}失败（源：{}，目标：{}）：{error}",
        source.display(),
        target.display()
    ))
}

fn copy_file(source: &Path, target: &Path, cancelled: &AtomicBool) -> Result<(), ArchiveError> {
    check_cancel(cancelled)?;
    let source_file =
        File::open(source).map_err(|error| io_error("读取产物", source, target, error))?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| io_error("创建归档目录", source, parent, error))?;
    }
    let target_file =
        File::create(target).map_err(|error| io_error("创建归档文件", source, target, error))?;
    let mut reader = BufReader::new(source_file);
    let mut writer = BufWriter::new(target_file);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_cancel(cancelled)?;
        let size = reader
            .read(&mut buffer)
            .map_err(|error| io_error("读取产物", source, target, error))?;
        if size == 0 {
            break;
        }
        writer
            .write_all(&buffer[..size])
            .map_err(|error| io_error("写入归档文件", source, target, error))?;
    }
    writer
        .flush()
        .map_err(|error| io_error("完成归档文件", source, target, error))?;
    Ok(())
}

fn copy_path_with_root(
    source: &Path,
    destination_root: &Path,
    cancelled: &AtomicBool,
) -> Result<(), ArchiveError> {
    let root_name = source_name(source)?;
    let destination = destination_root.join(&root_name);
    if source.is_dir() {
        for entry in WalkDir::new(source) {
            check_cancel(cancelled)?;
            let entry = entry.map_err(|error| {
                ArchiveError::Failed(format!(
                    "遍历产物失败（源：{}，目标：{}）：{error}",
                    source.display(),
                    destination.display()
                ))
            })?;
            let relative = entry.path().strip_prefix(source).map_err(|error| {
                ArchiveError::Failed(format!(
                    "计算产物相对路径失败（源：{}，目标：{}）：{error}",
                    source.display(),
                    destination.display()
                ))
            })?;
            let target = destination.join(relative);
            if entry.file_type().is_dir() {
                fs::create_dir_all(&target)
                    .map_err(|error| io_error("创建归档目录", entry.path(), &target, error))?;
            } else {
                copy_file(entry.path(), &target, cancelled)?;
            }
        }
    } else {
        copy_file(source, &destination, cancelled)?;
    }
    Ok(())
}

fn zip_directory_with_root(
    source: &Path,
    destination_zip: &Path,
    cancelled: &AtomicBool,
) -> Result<(), ArchiveError> {
    if let Some(parent) = destination_zip.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| io_error("创建归档目录", source, parent, error))?;
    }
    let file = File::create(destination_zip)
        .map_err(|error| io_error("创建 ZIP", source, destination_zip, error))?;
    let mut writer = zip::ZipWriter::new(file);
    let options = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let root_name = source_name(source)?;
    let mut buffer = [0_u8; 64 * 1024];
    for entry in WalkDir::new(source) {
        check_cancel(cancelled)?;
        let entry = entry.map_err(|error| {
            ArchiveError::Failed(format!(
                "遍历 ZIP 源目录失败（源：{}，目标：{}）：{error}",
                source.display(),
                destination_zip.display()
            ))
        })?;
        let relative = entry.path().strip_prefix(source).map_err(|error| {
            ArchiveError::Failed(format!(
                "计算 ZIP 相对路径失败（源：{}，目标：{}）：{error}",
                source.display(),
                destination_zip.display()
            ))
        })?;
        let name = zip_entry_name(&root_name, relative, source, destination_zip)?;
        if entry.file_type().is_dir() {
            writer
                .add_directory(format!("{name}/"), options)
                .map_err(|error| io_error("写入 ZIP 目录", source, destination_zip, error))?;
            continue;
        }
        writer
            .start_file(name, options)
            .map_err(|error| io_error("写入 ZIP 文件头", source, destination_zip, error))?;
        let mut reader =
            BufReader::new(File::open(entry.path()).map_err(|error| {
                io_error("读取 ZIP 源文件", entry.path(), destination_zip, error)
            })?);
        loop {
            check_cancel(cancelled)?;
            let size = reader.read(&mut buffer).map_err(|error| {
                io_error("读取 ZIP 源文件", entry.path(), destination_zip, error)
            })?;
            if size == 0 {
                break;
            }
            writer
                .write_all(&buffer[..size])
                .map_err(|error| io_error("写入 ZIP", entry.path(), destination_zip, error))?;
        }
    }
    writer
        .finish()
        .map_err(|error| io_error("完成 ZIP", source, destination_zip, error))?;
    Ok(())
}

pub fn archive_artifacts(
    request: &ArchiveRequest,
    cancelled: &AtomicBool,
    mut emit: impl FnMut(&str),
) -> Result<PathBuf, ArchiveError> {
    validate_folder_name(&request.folder_name).map_err(ArchiveError::Failed)?;
    check_cancel(cancelled)?;
    if !request.output_root.is_dir() {
        return Err(ArchiveError::Failed("全局归档根目录不存在".into()));
    }
    if !request.frontend_artifact.is_dir() {
        return Err(ArchiveError::Failed("前端产物必须是文件夹".into()));
    }
    if !request.backend_artifact.exists() {
        return Err(ArchiveError::Failed("后端产物不存在".into()));
    }
    let frontend_name = source_name(&request.frontend_artifact)?;
    let frontend_target = match request.frontend_mode.as_str() {
        "copy_directory" => frontend_name.clone(),
        "zip_directory" => format!("{frontend_name}.zip"),
        _ => return Err(ArchiveError::Failed("未知的前端产物处理模式".into())),
    };
    let backend_target = source_name(&request.backend_artifact)?;
    if basename_eq_ignore_case(&frontend_target, &backend_target) {
        return Err(ArchiveError::Failed(format!(
            "前后端归档名称冲突：{frontend_target}"
        )));
    }
    let final_path = request.output_root.join(&request.folder_name);
    if final_path.exists() {
        return Err(ArchiveError::Failed("目标归档目录已存在".into()));
    }
    let staging_path = request
        .output_root
        .join(format!(".lazycat-release-package-{}.tmp", request.run_id));
    if staging_path.exists() {
        return Err(ArchiveError::Failed("本次运行临时目录已存在".into()));
    }
    fs::create_dir(&staging_path).map_err(|error| {
        io_error(
            "创建归档临时目录",
            &request.output_root,
            &staging_path,
            error,
        )
    })?;
    let mut guard = StagingGuard {
        path: staging_path.clone(),
        committed: false,
    };
    emit("正在归档前端产物");
    if request.frontend_mode == "zip_directory" {
        zip_directory_with_root(
            &request.frontend_artifact,
            &staging_path.join(frontend_target),
            cancelled,
        )?;
    } else {
        copy_path_with_root(&request.frontend_artifact, &staging_path, cancelled)?;
    }
    emit("正在归档后端产物");
    copy_path_with_root(&request.backend_artifact, &staging_path, cancelled)?;
    check_cancel(cancelled)?;
    if final_path.exists() {
        return Err(ArchiveError::Failed("目标归档目录在执行期间被创建".into()));
    }
    fs::rename(&staging_path, &final_path)
        .map_err(|error| io_error("提交最终归档目录", &staging_path, &final_path, error))?;
    guard.committed = true;
    Ok(final_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::AtomicBool;
    use uuid::Uuid;
    use zip::ZipArchive;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "lazycat-release-package-archive-test-{}",
                Uuid::new_v4()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn thursday_is_inclusive_and_other_days_advance() {
        assert_eq!(
            default_folder_name(NaiveDate::from_ymd_opt(2026, 7, 23).unwrap(), "客户门户"),
            "20260723-客户门户"
        );
        assert_eq!(
            default_folder_name(NaiveDate::from_ymd_opt(2026, 7, 24).unwrap(), "客户门户"),
            "20260730-客户门户"
        );
    }

    #[test]
    fn folder_name_rejects_paths_and_windows_reserved_names() {
        for value in [
            "", ".", "..", "a/b", "a\\b", "CON", "LPT1.txt", "name.", "name ",
        ] {
            assert!(
                validate_folder_name(value).is_err(),
                "must reject {value:?}"
            );
        }
        assert!(validate_folder_name("20260723-客户门户").is_ok());
    }

    #[test]
    fn artifact_paths_resolve_relative_to_project() {
        assert_eq!(
            resolve_artifact_path(Path::new(r"D:\work\web"), "dist"),
            Path::new(r"D:\work\web").join("dist")
        );
        assert_eq!(
            resolve_artifact_path(Path::new(r"D:\work\web"), r"E:\shared\dist"),
            Path::new(r"E:\shared\dist")
        );
    }

    #[test]
    fn copy_mode_keeps_source_directory_and_backend_file() {
        let root = TestDir::new();
        let frontend = root.0.join("dist");
        let backend = root.0.join("portal.jar");
        let output = root.0.join("output");
        fs::create_dir_all(&frontend).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(frontend.join("index.html"), "ok").unwrap();
        fs::write(&backend, "jar").unwrap();

        let result = archive_artifacts(
            &ArchiveRequest {
                frontend_artifact: frontend,
                frontend_mode: "copy_directory".into(),
                backend_artifact: backend,
                output_root: output,
                folder_name: "20260723-客户门户".into(),
                run_id: "run-copy".into(),
            },
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();

        assert!(result.join("dist/index.html").is_file());
        assert!(result.join("portal.jar").is_file());
    }

    #[test]
    fn zip_mode_keeps_frontend_directory_as_zip_root() {
        let root = TestDir::new();
        let frontend = root.0.join("dist");
        let backend = root.0.join("server.jar");
        let output = root.0.join("output");
        fs::create_dir_all(frontend.join("assets")).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(frontend.join("assets/app.js"), "js").unwrap();
        fs::write(&backend, "jar").unwrap();

        let result = archive_artifacts(
            &ArchiveRequest {
                frontend_artifact: frontend,
                frontend_mode: "zip_directory".into(),
                backend_artifact: backend,
                output_root: output,
                folder_name: "20260723-客户门户".into(),
                run_id: "run-zip".into(),
            },
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();

        let mut zip = ZipArchive::new(fs::File::open(result.join("dist.zip")).unwrap()).unwrap();
        assert!(zip.by_name("dist/assets/app.js").is_ok());
    }

    #[test]
    fn collision_and_cancel_never_create_final_directory() {
        let root = TestDir::new();
        let frontend = root.0.join("dist");
        let backend = root.0.join("backend/DIST");
        let output = root.0.join("output");
        fs::create_dir_all(&frontend).unwrap();
        fs::create_dir_all(&backend).unwrap();
        fs::create_dir_all(&output).unwrap();

        let error = archive_artifacts(
            &ArchiveRequest {
                frontend_artifact: frontend.clone(),
                frontend_mode: "copy_directory".into(),
                backend_artifact: backend,
                output_root: output.clone(),
                folder_name: "20260723-客户门户".into(),
                run_id: "run-collision".into(),
            },
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap_err();
        assert!(matches!(error, ArchiveError::Failed(message) if message.contains("名称冲突")));

        let cancelled = AtomicBool::new(true);
        let error = archive_artifacts(
            &ArchiveRequest {
                frontend_artifact: frontend,
                frontend_mode: "copy_directory".into(),
                backend_artifact: root.0.join("missing.jar"),
                output_root: output.clone(),
                folder_name: "20260723-另一个项目".into(),
                run_id: "run-cancel".into(),
            },
            &cancelled,
            |_| {},
        )
        .unwrap_err();
        assert!(matches!(error, ArchiveError::Cancelled));
        assert!(!output.join("20260723-客户门户").exists());
        assert!(!output.join("20260723-另一个项目").exists());
    }

    #[test]
    fn collision_uses_unicode_case_folding() {
        let root = TestDir::new();
        let frontend = root.0.join("Ä");
        let backend = root.0.join("backend/ä");
        let output = root.0.join("output");
        fs::create_dir_all(&frontend).unwrap();
        fs::create_dir_all(&backend).unwrap();
        fs::create_dir_all(&output).unwrap();

        let error = archive_artifacts(
            &ArchiveRequest {
                frontend_artifact: frontend,
                frontend_mode: "copy_directory".into(),
                backend_artifact: backend,
                output_root: output,
                folder_name: "20260723-unicode".into(),
                run_id: "run-unicode-collision".into(),
            },
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap_err();

        assert!(matches!(error, ArchiveError::Failed(message) if message.contains("名称冲突")));
    }

    #[cfg(windows)]
    #[test]
    fn windows_ordinal_ignore_case_matches_sigma_filesystem_semantics() {
        let root = TestDir::new();
        let frontend = root.0.join("σ");
        let backend = root.0.join("backend/ς");
        let output = root.0.join("output");
        fs::create_dir_all(&frontend).unwrap();
        fs::create_dir_all(&backend).unwrap();
        fs::create_dir_all(&output).unwrap();

        assert!(!basename_eq_ignore_case("σ", "ς"));

        let result = archive_artifacts(
            &ArchiveRequest {
                frontend_artifact: frontend,
                frontend_mode: "copy_directory".into(),
                backend_artifact: backend,
                output_root: output,
                folder_name: "20260723-ordinal-ignore-case".into(),
                run_id: "run-ordinal-ignore-case".into(),
            },
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();

        assert!(result.join("σ").is_dir());
        assert!(result.join("ς").is_dir());
    }

    #[test]
    fn source_name_rejects_windows_normalized_names() {
        for value in ["dist.", "folder ", "CON", "LPT1.txt"] {
            assert!(
                source_name(Path::new(value)).is_err(),
                "must reject {value:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn source_name_rejects_non_utf8_names() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let path = PathBuf::from(OsString::from_vec(vec![0xff]));
        assert!(source_name(&path).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn zip_mode_rejects_non_utf8_entry_name() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let root = TestDir::new();
        let frontend = root.0.join("dist");
        let backend = root.0.join("server.jar");
        let output = root.0.join("output");
        fs::create_dir_all(&frontend).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(frontend.join(OsString::from_vec(vec![0xff])), "bad").unwrap();
        fs::write(&backend, "jar").unwrap();

        let error = archive_artifacts(
            &ArchiveRequest {
                frontend_artifact: frontend,
                frontend_mode: "zip_directory".into(),
                backend_artifact: backend,
                output_root: output.clone(),
                folder_name: "20260723-non-utf8".into(),
                run_id: "run-non-utf8".into(),
            },
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap_err();

        assert!(matches!(error, ArchiveError::Failed(message) if message.contains("UTF-8")));
        assert!(!output.join("20260723-non-utf8").exists());
    }

    #[test]
    fn cancellation_after_staging_creation_removes_staging_directory() {
        let root = TestDir::new();
        let frontend = root.0.join("dist");
        let backend = root.0.join("server.jar");
        let output = root.0.join("output");
        fs::create_dir_all(&frontend).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(frontend.join("index.html"), "ok").unwrap();
        fs::write(&backend, "jar").unwrap();
        let cancelled = AtomicBool::new(false);

        let error = archive_artifacts(
            &ArchiveRequest {
                frontend_artifact: frontend,
                frontend_mode: "copy_directory".into(),
                backend_artifact: backend,
                output_root: output.clone(),
                folder_name: "20260723-staging-cancel".into(),
                run_id: "run-staging-cancel".into(),
            },
            &cancelled,
            |_| cancelled.store(true, Ordering::Release),
        )
        .unwrap_err();

        assert!(matches!(error, ArchiveError::Cancelled));
        assert!(!output
            .join(".lazycat-release-package-run-staging-cancel.tmp")
            .exists());
        assert!(!output.join("20260723-staging-cancel").exists());
    }
}

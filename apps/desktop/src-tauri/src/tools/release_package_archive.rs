use chrono::{Datelike, Days, NaiveDate, Weekday};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;

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
    CommittedWithWarning {
        final_path: PathBuf,
        warning: String,
    },
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

pub struct ArchiveSession {
    staging_path: PathBuf,
    final_path: PathBuf,
    backup_path: PathBuf,
    overwrite_existing: bool,
    committed: bool,
}

impl ArchiveSession {
    pub fn create(
        output_root: &Path,
        folder_name: &str,
        run_id: &str,
        overwrite_existing: bool,
        cancelled: &AtomicBool,
    ) -> Result<Self, ArchiveError> {
        validate_folder_name(folder_name).map_err(ArchiveError::Failed)?;
        check_cancel(cancelled)?;
        if !output_root.is_dir() {
            return Err(ArchiveError::Failed("全局归档根目录不存在".into()));
        }
        let final_path = output_root.join(folder_name);
        if final_path.exists() {
            if !final_path.is_dir() {
                return Err(ArchiveError::Failed(
                    "目标归档路径已存在且不是文件夹".into(),
                ));
            }
            if !overwrite_existing {
                return Err(ArchiveError::Failed("目标归档目录已存在".into()));
            }
        }
        let staging_path = output_root.join(format!(".lazycat-release-package-{run_id}.tmp"));
        if staging_path.exists() {
            return Err(ArchiveError::Failed("本次运行临时目录已存在".into()));
        }
        let backup_path = output_root.join(format!(".lazycat-release-package-{run_id}.backup"));
        if backup_path.exists() {
            return Err(ArchiveError::Failed("本次运行备份目录已存在".into()));
        }
        fs::create_dir(&staging_path)
            .map_err(|error| io_error("创建归档临时目录", output_root, &staging_path, error))?;
        Ok(Self {
            staging_path,
            final_path,
            backup_path,
            overwrite_existing,
            committed: false,
        })
    }

    pub fn staging_path(&self) -> &Path {
        &self.staging_path
    }

    pub fn commit(&mut self, cancelled: &AtomicBool) -> Result<PathBuf, ArchiveError> {
        self.commit_with_rename(cancelled, rename_with_retry)
    }

    fn commit_with_rename<R>(
        &mut self,
        cancelled: &AtomicBool,
        mut rename: R,
    ) -> Result<PathBuf, ArchiveError>
    where
        R: FnMut(&Path, &Path, Option<&AtomicBool>) -> Result<(), RenameFailure>,
    {
        check_cancel(cancelled)?;
        let mut backup_created = false;
        if self.final_path.exists() {
            if !self.final_path.is_dir() {
                return Err(ArchiveError::Failed(
                    "目标归档路径已存在且不是文件夹".into(),
                ));
            }
            if !self.overwrite_existing {
                return Err(ArchiveError::Failed("目标归档目录在执行期间被创建".into()));
            }
            if self.backup_path.exists() {
                return Err(ArchiveError::Failed("本次运行备份目录已存在".into()));
            }
            match rename(&self.final_path, &self.backup_path, Some(cancelled)) {
                Ok(()) => backup_created = true,
                Err(RenameFailure::Cancelled) => return Err(ArchiveError::Cancelled),
                Err(RenameFailure::Io(error)) => {
                    return Err(io_error(
                        "备份已有归档目录",
                        &self.final_path,
                        &self.backup_path,
                        error,
                    ));
                }
            }
        }
        let commit_error = match rename(&self.staging_path, &self.final_path, Some(cancelled)) {
            Ok(()) => None,
            Err(error) => Some(error),
        };
        if let Some(commit_error) = commit_error {
            let was_cancelled = matches!(commit_error, RenameFailure::Cancelled);
            let mut message = match commit_error {
                RenameFailure::Cancelled => "提交最终归档目录已取消".to_string(),
                RenameFailure::Io(error) => format!(
                    "提交最终归档目录失败（源：{}，目标：{}）：{error}",
                    self.staging_path.display(),
                    self.final_path.display()
                ),
            };
            if backup_created {
                if let Err(rollback_error) = rename(&self.backup_path, &self.final_path, None) {
                    let rollback_error = match rollback_error {
                        RenameFailure::Cancelled => "回滚不应被取消".to_string(),
                        RenameFailure::Io(error) => error.to_string(),
                    };
                    message.push_str(&format!(
                        "；恢复原归档目录失败（源：{}，目标：{}）：{rollback_error}",
                        self.backup_path.display(),
                        self.final_path.display()
                    ));
                    return Err(ArchiveError::Failed(message));
                }
            }
            return if was_cancelled {
                Err(ArchiveError::Cancelled)
            } else {
                Err(ArchiveError::Failed(message))
            };
        }
        self.committed = true;
        if backup_created {
            if let Err(error) = fs::remove_dir_all(&self.backup_path) {
                return Err(ArchiveError::CommittedWithWarning {
                    final_path: self.final_path.clone(),
                    warning: format!(
                        "清理旧归档备份失败（源：{}，目标：{}）：{error}",
                        self.backup_path.display(),
                        self.final_path.display()
                    ),
                });
            }
        }
        Ok(self.final_path.clone())
    }
}

impl Drop for ArchiveSession {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_dir_all(&self.staging_path);
        }
    }
}

struct TargetGuard {
    path: PathBuf,
    committed: bool,
}

impl Drop for TargetGuard {
    fn drop(&mut self) {
        if self.committed || !self.path.exists() {
            return;
        }
        if self.path.is_dir() {
            let _ = fs::remove_dir_all(&self.path);
        } else {
            let _ = fs::remove_file(&self.path);
        }
    }
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

#[derive(Debug)]
enum RenameFailure {
    Cancelled,
    Io(io::Error),
}

#[cfg(windows)]
fn is_retryable_rename_error(error: &io::Error) -> bool {
    error.raw_os_error() == Some(5)
}

#[cfg(not(windows))]
fn is_retryable_rename_error(_error: &io::Error) -> bool {
    false
}

fn rename_with_retry_using<R, S>(
    source: &Path,
    target: &Path,
    cancelled: Option<&AtomicBool>,
    retry_delays: &[Duration],
    mut rename: R,
    mut sleep: S,
) -> Result<(), RenameFailure>
where
    R: FnMut(&Path, &Path) -> io::Result<()>,
    S: FnMut(Duration),
{
    let mut retry_delays = retry_delays.iter().copied();
    loop {
        match rename(source, target) {
            Ok(()) => return Ok(()),
            Err(error) if is_retryable_rename_error(&error) => {
                let Some(delay) = retry_delays.next() else {
                    return Err(RenameFailure::Io(error));
                };
                if cancelled.is_some_and(|flag| flag.load(Ordering::Acquire)) {
                    return Err(RenameFailure::Cancelled);
                }
                sleep(delay);
            }
            Err(error) => return Err(RenameFailure::Io(error)),
        }
    }
}

const RENAME_RETRY_DELAYS: [Duration; 5] = [
    Duration::from_millis(50),
    Duration::from_millis(100),
    Duration::from_millis(200),
    Duration::from_millis(400),
    Duration::from_millis(800),
];

fn rename_with_retry(
    source: &Path,
    target: &Path,
    cancelled: Option<&AtomicBool>,
) -> Result<(), RenameFailure> {
    rename_with_retry_using(
        source,
        target,
        cancelled,
        &RENAME_RETRY_DELAYS,
        |source, target| fs::rename(source, target),
        |delay| thread::sleep(delay),
    )
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
    let options = SimpleFileOptions::default()
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

pub fn archive_frontend_artifact(
    source: &Path,
    mode: &str,
    staging_path: &Path,
    cancelled: &AtomicBool,
    mut emit: impl FnMut(&str),
) -> Result<String, ArchiveError> {
    if !source.is_dir() {
        return Err(ArchiveError::Failed("前端产物必须是文件夹".into()));
    }
    let source_name = source_name(source)?;
    let target_name = match mode {
        "copy_directory" => source_name,
        "zip_directory" => format!("{source_name}.zip"),
        _ => return Err(ArchiveError::Failed("未知的前端产物处理模式".into())),
    };
    let mut guard = TargetGuard {
        path: staging_path.join(&target_name),
        committed: false,
    };
    emit("正在归档前端产物");
    if mode == "zip_directory" {
        zip_directory_with_root(source, &guard.path, cancelled)?;
    } else {
        copy_path_with_root(source, staging_path, cancelled)?;
    }
    guard.committed = true;
    Ok(target_name)
}

pub fn archive_backend_artifact(
    source: &Path,
    staging_path: &Path,
    cancelled: &AtomicBool,
    mut emit: impl FnMut(&str),
) -> Result<String, ArchiveError> {
    if !source.is_file() {
        return Err(ArchiveError::Failed("后端产物必须是文件".into()));
    }
    let target_name = source_name(source)?;
    let mut guard = TargetGuard {
        path: staging_path.join(&target_name),
        committed: false,
    };
    emit("正在归档后端产物");
    copy_path_with_root(source, staging_path, cancelled)?;
    guard.committed = true;
    Ok(target_name)
}

pub fn validate_artifact_target_collision(
    frontend_source: &Path,
    frontend_mode: &str,
    backend_source: &Path,
) -> Result<(), ArchiveError> {
    let frontend_name = source_name(frontend_source)?;
    let frontend_target = match frontend_mode {
        "copy_directory" => frontend_name,
        "zip_directory" => format!("{frontend_name}.zip"),
        _ => return Err(ArchiveError::Failed("未知的前端产物处理模式".into())),
    };
    let backend_target = source_name(backend_source)?;
    if basename_eq_ignore_case(&frontend_target, &backend_target) {
        return Err(ArchiveError::Failed(format!(
            "前后端归档名称冲突：{frontend_target}"
        )));
    }
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
    validate_artifact_target_collision(
        &request.frontend_artifact,
        &request.frontend_mode,
        &request.backend_artifact,
    )?;
    let frontend_name = source_name(&request.frontend_artifact)?;
    let frontend_target = match request.frontend_mode.as_str() {
        "copy_directory" => frontend_name.clone(),
        "zip_directory" => format!("{frontend_name}.zip"),
        _ => return Err(ArchiveError::Failed("未知的前端产物处理模式".into())),
    };
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
    use std::time::Duration;
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
    fn independent_targets_commit_only_successful_artifacts() {
        let root = TestDir::new();
        let frontend = root.0.join("dist");
        let output = root.0.join("output");
        fs::create_dir_all(&frontend).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(frontend.join("index.html"), "ok").unwrap();
        let cancelled = AtomicBool::new(false);
        let mut session = ArchiveSession::create(
            &output,
            "20260723-部分成功",
            "run-partial",
            false,
            &cancelled,
        )
        .unwrap();

        archive_frontend_artifact(
            &frontend,
            "copy_directory",
            session.staging_path(),
            &cancelled,
            |_| {},
        )
        .unwrap();
        let backend_error = archive_backend_artifact(
            &root.0.join("missing.jar"),
            session.staging_path(),
            &cancelled,
            |_| {},
        )
        .unwrap_err();
        assert!(matches!(backend_error, ArchiveError::Failed(_)));

        let final_path = session.commit(&cancelled).unwrap();
        assert!(final_path.join("dist/index.html").is_file());
        assert!(!final_path.join("missing.jar").exists());
    }

    #[test]
    fn overwrite_replaces_existing_directory_without_stale_files() {
        let root = TestDir::new();
        let output = root.0.join("output");
        let final_path = output.join("release");
        fs::create_dir_all(&final_path).unwrap();
        fs::write(final_path.join("stale.txt"), "old").unwrap();
        let cancelled = AtomicBool::new(false);
        let mut session =
            ArchiveSession::create(&output, "release", "run-overwrite", true, &cancelled).unwrap();
        fs::write(session.staging_path().join("new.txt"), "new").unwrap();

        session.commit(&cancelled).unwrap();

        assert!(!final_path.join("stale.txt").exists());
        assert_eq!(
            fs::read_to_string(final_path.join("new.txt")).unwrap(),
            "new"
        );
        assert!(!output
            .join(".lazycat-release-package-run-overwrite.backup")
            .exists());
    }

    #[test]
    fn committed_overwrite_reports_backup_cleanup_warning_with_final_path() {
        let root = TestDir::new();
        let output = root.0.join("output");
        let final_path = output.join("release");
        let backup_path = output.join(".lazycat-release-package-run-cleanup.backup");
        fs::create_dir_all(&final_path).unwrap();
        fs::write(final_path.join("old.txt"), "old").unwrap();
        let cancelled = AtomicBool::new(false);
        let mut session =
            ArchiveSession::create(&output, "release", "run-cleanup", true, &cancelled).unwrap();
        fs::write(session.staging_path().join("new.txt"), "new").unwrap();
        let mut rename_count = 0;

        let result = session.commit_with_rename(&cancelled, |source, target, _| {
            rename_count += 1;
            fs::rename(source, target).map_err(RenameFailure::Io)?;
            if rename_count == 2 {
                fs::remove_dir_all(&backup_path).unwrap();
                fs::write(&backup_path, "cannot remove as directory").unwrap();
            }
            Ok(())
        });

        match result {
            Err(ArchiveError::CommittedWithWarning {
                final_path: committed_path,
                warning,
            }) => {
                assert_eq!(committed_path, final_path);
                assert!(warning.contains("清理旧归档备份"));
                assert!(warning.contains(&backup_path.display().to_string()));
            }
            other => panic!("expected committed cleanup warning, got {other:?}"),
        }
        assert_eq!(
            fs::read_to_string(final_path.join("new.txt")).unwrap(),
            "new"
        );
        assert!(!final_path.join("old.txt").exists());
        assert!(backup_path.exists());
    }

    #[test]
    fn failed_overwrite_commit_restores_existing_directory() {
        let root = TestDir::new();
        let output = root.0.join("output");
        let final_path = output.join("release");
        fs::create_dir_all(&final_path).unwrap();
        fs::write(final_path.join("old.txt"), "old").unwrap();
        let cancelled = AtomicBool::new(false);
        let mut session =
            ArchiveSession::create(&output, "release", "run-rollback", true, &cancelled).unwrap();
        fs::remove_dir_all(session.staging_path()).unwrap();

        assert!(session.commit(&cancelled).is_err());
        assert_eq!(
            fs::read_to_string(final_path.join("old.txt")).unwrap(),
            "old"
        );
        assert!(!output
            .join(".lazycat-release-package-run-rollback.backup")
            .exists());
    }

    #[cfg(windows)]
    #[test]
    fn cancelled_commit_retry_restores_existing_directory() {
        let root = TestDir::new();
        let output = root.0.join("output");
        let final_path = output.join("release");
        fs::create_dir_all(&final_path).unwrap();
        fs::write(final_path.join("old.txt"), "old").unwrap();
        let cancelled = AtomicBool::new(false);
        let mut session =
            ArchiveSession::create(&output, "release", "run-cancel-retry", true, &cancelled)
                .unwrap();
        fs::write(session.staging_path().join("new.txt"), "new").unwrap();
        let mut rename_count = 0;

        let result = session.commit_with_rename(&cancelled, |source, target, retry_cancelled| {
            rename_count += 1;
            if rename_count == 1 {
                fs::rename(source, target).map_err(RenameFailure::Io)
            } else if rename_count == 2 {
                cancelled.store(true, Ordering::Release);
                Err(RenameFailure::Cancelled)
            } else {
                rename_with_retry(source, target, retry_cancelled)
            }
        });

        assert!(matches!(result, Err(ArchiveError::Cancelled)));
        assert_eq!(
            fs::read_to_string(final_path.join("old.txt")).unwrap(),
            "old"
        );
        assert!(!final_path.join("new.txt").exists());
    }

    #[test]
    fn overwrite_rejects_existing_file_target() {
        let root = TestDir::new();
        let output = root.0.join("output");
        fs::create_dir_all(&output).unwrap();
        fs::write(output.join("release"), "file").unwrap();

        let result = ArchiveSession::create(
            &output,
            "release",
            "run-file-target",
            true,
            &AtomicBool::new(false),
        );

        assert!(
            matches!(result, Err(ArchiveError::Failed(message)) if message.contains("不是文件夹"))
        );
    }

    #[cfg(windows)]
    #[test]
    fn access_denied_rename_retries_until_success() {
        let cancelled = AtomicBool::new(false);
        let mut attempts = 0;
        let mut sleeps = 0;

        let result = rename_with_retry_using(
            Path::new("source"),
            Path::new("target"),
            Some(&cancelled),
            &[Duration::ZERO, Duration::ZERO],
            |_, _| {
                attempts += 1;
                if attempts < 3 {
                    Err(std::io::Error::from_raw_os_error(5))
                } else {
                    Ok(())
                }
            },
            |_| sleeps += 1,
        );

        assert!(result.is_ok());
        assert_eq!(attempts, 3);
        assert_eq!(sleeps, 2);
    }

    #[test]
    fn non_access_denied_rename_error_is_not_retried() {
        let mut attempts = 0;

        let result = rename_with_retry_using(
            Path::new("source"),
            Path::new("target"),
            None,
            &[Duration::ZERO],
            |_, _| {
                attempts += 1;
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "missing source",
                ))
            },
            |_| panic!("non-retryable error must not sleep"),
        );

        assert!(matches!(result, Err(RenameFailure::Io(_))));
        assert_eq!(attempts, 1);
    }

    #[cfg(windows)]
    #[test]
    fn access_denied_rename_stops_after_retry_budget() {
        let mut attempts = 0;

        let result = rename_with_retry_using(
            Path::new("source"),
            Path::new("target"),
            None,
            &[Duration::ZERO, Duration::ZERO],
            |_, _| {
                attempts += 1;
                Err(std::io::Error::from_raw_os_error(5))
            },
            |_| {},
        );

        assert!(matches!(result, Err(RenameFailure::Io(error)) if error.raw_os_error() == Some(5)));
        assert_eq!(attempts, 3);
    }

    #[cfg(windows)]
    #[test]
    fn access_denied_rename_honors_cancellation_before_waiting() {
        let cancelled = AtomicBool::new(true);
        let mut attempts = 0;

        let result = rename_with_retry_using(
            Path::new("source"),
            Path::new("target"),
            Some(&cancelled),
            &[Duration::ZERO],
            |_, _| {
                attempts += 1;
                Err(std::io::Error::from_raw_os_error(5))
            },
            |_| panic!("cancelled retry must not sleep"),
        );

        assert!(matches!(result, Err(RenameFailure::Cancelled)));
        assert_eq!(attempts, 1);
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

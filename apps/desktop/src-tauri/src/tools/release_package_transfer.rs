use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use flate2::write::GzEncoder;
use flate2::Compression;
use tar::{Builder, EntryType, Header};
use tempfile::{Builder as TempBuilder, TempDir};

use super::release_package_artifact::{ArtifactEntry, ArtifactManifest};
use super::release_package_model::ReleaseTarget;

pub(crate) const PREFLIGHT_PROBE_ENTRY: &str = "lazycat-probe.txt";
pub(crate) const PREFLIGHT_PROBE_CONTENT: &[u8] = b"lazycat-tar-gzip-probe\n";

#[derive(Clone, Debug)]
pub struct FrontendTransferArchive {
    inner: Arc<TransferArchiveInner>,
    pub original_bytes: u64,
    pub file_count: u64,
    pub compressed_bytes: u64,
    pub compression_duration: Duration,
}

#[derive(Debug)]
struct TransferArchiveInner {
    _temp_dir: TempDir,
    path: PathBuf,
}

impl FrontendTransferArchive {
    pub fn path(&self) -> &Path {
        &self.inner.path
    }
}

#[derive(Debug)]
pub struct TransferError {
    pub message: String,
    pub cancelled: bool,
}

impl TransferError {
    fn failed(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            cancelled: false,
        }
    }

    fn cancelled() -> Self {
        Self {
            message: "前端传输包压缩已取消".into(),
            cancelled: true,
        }
    }
}

struct CancelReader<'a, R> {
    inner: R,
    cancelled: &'a AtomicBool,
}

impl<R: Read> Read for CancelReader<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "compression cancelled",
            ));
        }
        self.inner.read(buffer)
    }
}

pub fn create_frontend_transfer(
    manifest: &ArtifactManifest,
    cancelled: &AtomicBool,
) -> Result<FrontendTransferArchive, TransferError> {
    create_frontend_transfer_with(manifest, cancelled, None, |_| {})
}

fn create_frontend_transfer_with(
    manifest: &ArtifactManifest,
    cancelled: &AtomicBool,
    temp_parent: Option<&Path>,
    mut after_entry: impl FnMut(usize),
) -> Result<FrontendTransferArchive, TransferError> {
    if manifest.target != ReleaseTarget::Frontend {
        return Err(TransferError::failed("只有前端目录可以生成 tar.gz 传输包"));
    }
    check_cancelled(cancelled)?;
    manifest.verify_source().map_err(TransferError::failed)?;
    validate_manifest_entries(manifest)?;

    let temp_dir = match temp_parent {
        Some(parent) => TempBuilder::new()
            .prefix("lazycat-release-transfer-")
            .tempdir_in(parent),
        None => TempBuilder::new()
            .prefix("lazycat-release-transfer-")
            .tempdir(),
    }
    .map_err(|error| TransferError::failed(format!("创建前端传输临时目录失败：{error}")))?;
    let archive_path = temp_dir.path().join("frontend.tar.gz");
    let started_at = Instant::now();
    write_archive(manifest, &archive_path, cancelled, &mut after_entry)?;
    check_cancelled(cancelled)?;
    manifest.verify_source().map_err(TransferError::failed)?;
    let compressed_bytes = fs::metadata(&archive_path)
        .map_err(|error| TransferError::failed(format!("读取前端传输包大小失败：{error}")))?
        .len();

    Ok(FrontendTransferArchive {
        inner: Arc::new(TransferArchiveInner {
            _temp_dir: temp_dir,
            path: archive_path,
        }),
        original_bytes: manifest.total_bytes,
        file_count: manifest.file_count,
        compressed_bytes,
        compression_duration: started_at.elapsed(),
    })
}

fn write_archive(
    manifest: &ArtifactManifest,
    archive_path: &Path,
    cancelled: &AtomicBool,
    after_entry: &mut dyn FnMut(usize),
) -> Result<(), TransferError> {
    let archive_file = File::create(archive_path)
        .map_err(|error| TransferError::failed(format!("创建前端传输包失败：{error}")))?;
    let encoder = GzEncoder::new(archive_file, Compression::fast());
    let mut archive = Builder::new(encoder);

    for (index, entry) in manifest.entries.iter().enumerate() {
        check_cancelled(cancelled)?;
        let local_path = local_entry_path(manifest, entry);
        let metadata = local_entry_metadata(&local_path, entry)?;
        let file = File::open(&local_path).map_err(|error| {
            TransferError::failed(format!(
                "读取前端产物失败（{}）：{error}",
                local_path.display()
            ))
        })?;
        let mut header = Header::new_gnu();
        header
            .set_path(Path::new(&entry.relative_path))
            .map_err(|error| TransferError::failed(format!("写入 tar 路径失败：{error}")))?;
        header.set_entry_type(EntryType::Regular);
        header.set_size(metadata.len());
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_cksum();
        let mut reader = CancelReader {
            inner: file,
            cancelled,
        };
        if let Err(error) = archive.append(&header, &mut reader) {
            if cancelled.load(Ordering::Acquire) {
                return Err(TransferError::cancelled());
            }
            return Err(TransferError::failed(format!(
                "压缩前端产物失败（{}）：{error}",
                entry.relative_path
            )));
        }
        after_entry(index);
    }
    check_cancelled(cancelled)?;
    archive
        .finish()
        .map_err(|error| TransferError::failed(format!("完成 tar 归档失败：{error}")))?;
    let encoder = archive
        .into_inner()
        .map_err(|error| TransferError::failed(format!("完成 tar 归档失败：{error}")))?;
    let archive_file = encoder
        .finish()
        .map_err(|error| TransferError::failed(format!("完成 gzip 压缩失败：{error}")))?;
    archive_file
        .sync_all()
        .map_err(|error| TransferError::failed(format!("刷新前端传输包失败：{error}")))?;
    Ok(())
}

fn validate_manifest_entries(manifest: &ArtifactManifest) -> Result<(), TransferError> {
    let source_metadata = fs::symlink_metadata(&manifest.source_path).map_err(|error| {
        TransferError::failed(format!(
            "读取前端产物目录失败（{}）：{error}",
            manifest.source_path.display()
        ))
    })?;
    if source_metadata.file_type().is_symlink() || !source_metadata.is_dir() {
        return Err(TransferError::failed(
            "前端部署产物必须是普通目录且不能是符号链接",
        ));
    }

    let mut paths = BTreeSet::new();
    for entry in &manifest.entries {
        validate_relative_path(&entry.relative_path)?;
        if !paths.insert(entry.relative_path.as_str()) {
            return Err(TransferError::failed(format!(
                "前端部署清单包含重复路径：{}",
                entry.relative_path
            )));
        }
        local_entry_metadata(&local_entry_path(manifest, entry), entry)?;
    }
    Ok(())
}

fn validate_relative_path(path: &str) -> Result<(), TransferError> {
    if path.is_empty() || path.contains('\\') || path.contains('\0') {
        return Err(TransferError::failed(format!(
            "前端部署清单路径不规范：{path}"
        )));
    }
    let candidate = Path::new(path);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(TransferError::failed(format!(
            "前端部署清单路径逃逸：{path}"
        )));
    }
    Ok(())
}

fn local_entry_path(manifest: &ArtifactManifest, entry: &ArtifactEntry) -> PathBuf {
    entry
        .relative_path
        .split('/')
        .fold(manifest.source_path.clone(), |path, segment| {
            path.join(segment)
        })
}

fn local_entry_metadata(
    local_path: &Path,
    entry: &ArtifactEntry,
) -> Result<fs::Metadata, TransferError> {
    let metadata = fs::symlink_metadata(local_path).map_err(|error| {
        TransferError::failed(format!(
            "读取前端产物信息失败（{}）：{error}",
            local_path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(TransferError::failed(format!(
            "前端部署产物不能包含符号链接：{}",
            local_path.display()
        )));
    }
    if !metadata.is_file() {
        return Err(TransferError::failed(format!(
            "前端部署产物包含非普通文件：{}",
            local_path.display()
        )));
    }
    if metadata.len() != entry.size {
        return Err(TransferError::failed(
            "部署产物在打包后发生变化，请重新打包",
        ));
    }
    Ok(metadata)
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), TransferError> {
    if cancelled.load(Ordering::Acquire) {
        Err(TransferError::cancelled())
    } else {
        Ok(())
    }
}

pub(crate) fn create_preflight_probe_archive() -> Result<FrontendTransferArchive, String> {
    let source_dir = tempfile::tempdir().map_err(|error| format!("创建预检探针失败：{error}"))?;
    fs::write(
        source_dir.path().join(PREFLIGHT_PROBE_ENTRY),
        PREFLIGHT_PROBE_CONTENT,
    )
    .map_err(|error| format!("写入预检探针失败：{error}"))?;
    let manifest = ArtifactManifest::from_directory(ReleaseTarget::Frontend, source_dir.path())?;
    create_frontend_transfer(&manifest, &AtomicBool::new(false)).map_err(|error| error.message)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::AtomicBool;

    use flate2::read::GzDecoder;
    use tar::Archive;

    use super::*;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("lazycat-transfer-test-{}", uuid::Uuid::new_v4()));
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
    fn archive_contains_manifest_paths_content_empty_files_and_fixed_permissions() {
        let root = TestDir::new();
        let source = root.0.join("dist");
        fs::create_dir_all(source.join("assets")).unwrap();
        fs::write(source.join("index.html"), "hello").unwrap();
        fs::write(source.join("assets/empty.txt"), []).unwrap();
        fs::write(source.join("中文.txt"), "内容").unwrap();
        let manifest = ArtifactManifest::from_directory(ReleaseTarget::Frontend, &source).unwrap();

        let transfer = create_frontend_transfer(&manifest, &AtomicBool::new(false)).unwrap();
        let decoder = GzDecoder::new(File::open(transfer.path()).unwrap());
        let mut archive = Archive::new(decoder);
        let mut entries = archive.entries().unwrap();
        let mut actual = Vec::new();
        while let Some(entry) = entries.next() {
            let mut entry = entry.unwrap();
            let path = entry.path().unwrap().to_string_lossy().replace('\\', "/");
            let mode = entry.header().mode().unwrap();
            let mut content = Vec::new();
            entry.read_to_end(&mut content).unwrap();
            actual.push((path, content, mode));
        }

        assert_eq!(
            actual,
            vec![
                ("assets/empty.txt".into(), Vec::new(), 0o644),
                ("index.html".into(), b"hello".to_vec(), 0o644),
                ("中文.txt".into(), "内容".as_bytes().to_vec(), 0o644),
            ]
        );
    }

    #[test]
    fn rejects_path_escape_and_non_regular_sources() {
        let root = TestDir::new();
        let source = root.0.join("dist");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("ok.txt"), "ok").unwrap();
        let base = ArtifactManifest::from_directory(ReleaseTarget::Frontend, &source).unwrap();

        for path in ["../escape.txt", "/absolute.txt", "a\\b.txt"] {
            let mut manifest = base.clone();
            manifest.entries[0].relative_path = path.into();
            assert!(create_frontend_transfer(&manifest, &AtomicBool::new(false)).is_err());
        }

        let mut directory_entry = base.clone();
        directory_entry.entries[0].relative_path = "folder".into();
        directory_entry.entries[0].size = 0;
        fs::create_dir(source.join("folder")).unwrap();
        assert!(create_frontend_transfer(&directory_entry, &AtomicBool::new(false)).is_err());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn rejects_symbolic_links() {
        let root = TestDir::new();
        let source = root.0.join("dist");
        fs::create_dir(&source).unwrap();
        let target = root.0.join("target.txt");
        fs::write(&target, "target").unwrap();
        let link = source.join("link.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        #[cfg(windows)]
        if std::os::windows::fs::symlink_file(&target, &link).is_err() {
            return;
        }
        let manifest = ArtifactManifest {
            target: ReleaseTarget::Frontend,
            source_path: source,
            entries: vec![ArtifactEntry {
                relative_path: "link.txt".into(),
                size: 6,
            }],
            file_count: 1,
            total_bytes: 6,
        };
        assert!(create_frontend_transfer(&manifest, &AtomicBool::new(false)).is_err());
    }

    #[test]
    fn cancellation_and_source_changes_fail_and_cleanup_temp_archive() {
        let root = TestDir::new();
        let source = root.0.join("dist");
        let temp_parent = root.0.join("temp");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&temp_parent).unwrap();
        fs::write(source.join("a.txt"), "a").unwrap();
        fs::write(source.join("b.txt"), "b").unwrap();
        let manifest = ArtifactManifest::from_directory(ReleaseTarget::Frontend, &source).unwrap();

        let cancelled = AtomicBool::new(false);
        let error =
            create_frontend_transfer_with(&manifest, &cancelled, Some(&temp_parent), |_| {
                cancelled.store(true, Ordering::Release)
            })
            .unwrap_err();
        assert!(error.cancelled);
        assert_eq!(fs::read_dir(&temp_parent).unwrap().count(), 0);

        let changed_path = source.join("b.txt");
        let error = create_frontend_transfer_with(
            &manifest,
            &AtomicBool::new(false),
            Some(&temp_parent),
            |index| {
                if index == 0 {
                    fs::write(&changed_path, "changed").unwrap();
                }
            },
        )
        .unwrap_err();
        assert!(error.message.contains("发生变化"));
        assert_eq!(fs::read_dir(&temp_parent).unwrap().count(), 0);
    }
}

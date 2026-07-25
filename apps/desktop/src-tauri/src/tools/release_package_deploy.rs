use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use walkdir::WalkDir;

use super::release_package::ReleaseTarget;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactEntry {
    pub relative_path: String,
    pub size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactManifest {
    pub target: ReleaseTarget,
    pub source_path: PathBuf,
    pub entries: Vec<ArtifactEntry>,
    pub file_count: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchivedTarget {
    pub target: ReleaseTarget,
    pub archive_entry_name: String,
    pub artifact_mode: String,
}

fn relative_path_string(path: &Path) -> Result<String, String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err("产物相对路径包含不支持的路径片段".into());
        };
        let value = value
            .to_str()
            .ok_or_else(|| "产物相对路径不是有效 UTF-8".to_string())?;
        parts.push(value);
    }
    if parts.is_empty() {
        return Err("产物文件缺少相对路径".into());
    }
    Ok(parts.join("/"))
}

impl ArtifactManifest {
    pub fn from_directory(target: ReleaseTarget, source: &Path) -> Result<Self, String> {
        if !source.is_dir() {
            return Err(format!("部署产物目录不存在：{}", source.display()));
        }
        let mut entries = Vec::new();
        for entry in WalkDir::new(source).follow_links(false) {
            let entry = entry.map_err(|error| format!("遍历部署产物失败：{error}"))?;
            if entry.file_type().is_symlink() {
                return Err(format!(
                    "部署产物不能包含符号链接：{}",
                    entry.path().display()
                ));
            }
            if entry.file_type().is_dir() {
                continue;
            }
            if !entry.file_type().is_file() {
                return Err(format!(
                    "部署产物包含不支持的文件类型：{}",
                    entry.path().display()
                ));
            }
            let relative = entry
                .path()
                .strip_prefix(source)
                .map_err(|error| format!("计算部署产物相对路径失败：{error}"))?;
            let size = entry
                .metadata()
                .map_err(|error| format!("读取部署产物信息失败：{error}"))?
                .len();
            entries.push(ArtifactEntry {
                relative_path: relative_path_string(relative)?,
                size,
            });
        }
        entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        Ok(Self::new(target, source, entries))
    }

    pub fn from_file(target: ReleaseTarget, source: &Path) -> Result<Self, String> {
        if source
            .symlink_metadata()
            .map(|meta| meta.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(format!("部署产物不能是符号链接：{}", source.display()));
        }
        if !source.is_file() {
            return Err(format!("部署产物文件不存在：{}", source.display()));
        }
        let name = source
            .file_name()
            .ok_or_else(|| "部署产物文件缺少文件名".to_string())?;
        let entry = ArtifactEntry {
            relative_path: relative_path_string(Path::new(name))?,
            size: source
                .metadata()
                .map_err(|error| format!("读取部署产物信息失败：{error}"))?
                .len(),
        };
        Ok(Self::new(target, source, vec![entry]))
    }

    fn new(target: ReleaseTarget, source: &Path, entries: Vec<ArtifactEntry>) -> Self {
        let file_count = entries.len() as u64;
        let total_bytes = entries.iter().map(|entry| entry.size).sum();
        Self {
            target,
            source_path: source.to_path_buf(),
            entries,
            file_count,
            total_bytes,
        }
    }

    pub fn verify_source(&self) -> Result<(), String> {
        let current = if self.source_path.is_dir() {
            Self::from_directory(self.target, &self.source_path)?
        } else {
            Self::from_file(self.target, &self.source_path)?
        };
        if current.entries != self.entries
            || current.file_count != self.file_count
            || current.total_bytes != self.total_bytes
        {
            return Err("部署产物在打包后发生变化，请重新打包".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteKind {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteMetadata {
    pub kind: RemoteKind,
    pub size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteDirEntry {
    pub path: String,
    pub metadata: RemoteMetadata,
}

#[derive(Debug, Eq, PartialEq)]
pub struct DeployError {
    pub message: String,
    pub cancelled: bool,
    pub committed: bool,
    pub recovery_paths: Vec<String>,
}

impl std::fmt::Display for DeployError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DeployError {}
impl DeployError {
    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            cancelled: false,
            committed: false,
            recovery_paths: Vec::new(),
        }
    }

    pub fn cancelled() -> Self {
        Self {
            message: "远端上传已取消".into(),
            cancelled: true,
            committed: false,
            recovery_paths: Vec::new(),
        }
    }

    pub fn local_io(error: std::io::Error) -> Self {
        Self::failed(format!("读取本地部署产物失败：{error}"))
    }
}

pub trait RemoteFs: Send {
    fn metadata(&self, path: &str) -> Result<Option<RemoteMetadata>, DeployError>;
    fn create_dir(&mut self, path: &str) -> Result<(), DeployError>;
    fn read_dir(&self, path: &str) -> Result<Vec<RemoteDirEntry>, DeployError>;
    fn write_file(
        &mut self,
        remote_path: &str,
        local_path: &Path,
        cancelled: &AtomicBool,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), DeployError>;
    fn rename(&mut self, source: &str, target: &str) -> Result<(), DeployError>;
    fn remove_tree(&mut self, path: &str) -> Result<(), DeployError>;
}

#[derive(Clone, Debug)]
pub struct DeploymentTarget {
    pub manifest: ArtifactManifest,
    pub remote_path: String,
    pub expected_exists: bool,
}

#[derive(Clone, Debug)]
pub struct DeploymentRequest {
    pub run_id: String,
    pub targets: Vec<DeploymentTarget>,
}

#[derive(Clone, Debug)]
struct TransactionTarget {
    final_path: String,
    temp_path: String,
    backup_path: String,
    expected_exists: bool,
    backed_up: bool,
    committed: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct DeploymentPlan {
    request: DeploymentRequest,
    transactions: Vec<TransactionTarget>,
    frontend_directories: Vec<String>,
}

impl DeploymentPlan {
    pub(crate) fn new(request: DeploymentRequest) -> Result<Self, DeployError> {
        validate_request(&request)?;
        for target in &request.targets {
            target
                .manifest
                .verify_source()
                .map_err(DeployError::failed)?;
        }
        let transactions = request
            .targets
            .iter()
            .map(|target| TransactionTarget {
                final_path: target.remote_path.clone(),
                temp_path: transaction_path(&target.remote_path, "tmp", &request.run_id),
                backup_path: transaction_path(&target.remote_path, "backup", &request.run_id),
                expected_exists: target.expected_exists,
                backed_up: false,
                committed: false,
            })
            .collect::<Vec<_>>();
        let frontend_directories = collect_frontend_directories(&request, &transactions)?;
        Ok(Self {
            request,
            transactions,
            frontend_directories,
        })
    }

    pub(crate) fn request(&self) -> &DeploymentRequest {
        &self.request
    }

    pub(crate) fn target_count(&self) -> usize {
        self.request.targets.len()
    }

    pub(crate) fn frontend_directories(&self) -> &[String] {
        &self.frontend_directories
    }

    pub(crate) fn temp_paths(&self) -> Vec<String> {
        self.transactions
            .iter()
            .map(|transaction| transaction.temp_path.clone())
            .collect()
    }

    pub(crate) fn prepare_remote(&self, remote: &mut dyn RemoteFs) -> Result<(), DeployError> {
        for transaction in &self.transactions {
            if remote.metadata(&transaction.temp_path)?.is_some()
                || remote.metadata(&transaction.backup_path)?.is_some()
            {
                return Err(DeployError::failed(format!(
                    "远端部署临时或备份路径已存在：{}",
                    transaction.final_path
                )));
            }
        }

        let frontend_roots = self
            .request
            .targets
            .iter()
            .zip(&self.transactions)
            .filter(|(target, _)| target.manifest.target == ReleaseTarget::Frontend)
            .map(|(_, transaction)| transaction.temp_path.as_str())
            .collect::<BTreeSet<_>>();
        for directory in &self.frontend_directories {
            if !frontend_roots.contains(directory.as_str()) {
                match remote.metadata(directory)? {
                    Some(metadata) if metadata.kind == RemoteKind::Directory => continue,
                    Some(_) => {
                        return Err(DeployError::failed(format!(
                            "远端目录路径已被非目录占用：{directory}"
                        )))
                    }
                    None => {}
                }
            }
            remote.create_dir(directory)?;
        }
        Ok(())
    }

    pub(crate) fn upload_target(
        &self,
        index: usize,
        remote: &mut dyn RemoteFs,
        cancelled: &AtomicBool,
        progress: &mut dyn FnMut(u64, &str),
    ) -> Result<(), DeployError> {
        let target = self
            .request
            .targets
            .get(index)
            .ok_or_else(|| DeployError::failed("部署目标索引无效"))?;
        let transaction = self
            .transactions
            .get(index)
            .ok_or_else(|| DeployError::failed("部署事务索引无效"))?;
        if cancelled.load(Ordering::Acquire) {
            return Err(DeployError::cancelled());
        }
        match target.manifest.target {
            ReleaseTarget::Frontend => {
                for entry in &target.manifest.entries {
                    if cancelled.load(Ordering::Acquire) {
                        return Err(DeployError::cancelled());
                    }
                    let remote_path = format!("{}/{}", transaction.temp_path, entry.relative_path);
                    let mut file_progress = |bytes| progress(bytes, entry.relative_path.as_str());
                    remote.write_file(
                        &remote_path,
                        &local_entry_path(&target.manifest, entry),
                        cancelled,
                        &mut file_progress,
                    )?;
                }
            }
            ReleaseTarget::Backend => {
                let entry = target
                    .manifest
                    .entries
                    .first()
                    .ok_or_else(|| DeployError::failed("后端部署清单缺少文件"))?;
                let mut file_progress = |bytes| progress(bytes, entry.relative_path.as_str());
                remote.write_file(
                    &transaction.temp_path,
                    &local_entry_path(&target.manifest, entry),
                    cancelled,
                    &mut file_progress,
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn verify_target(
        &self,
        index: usize,
        remote: &dyn RemoteFs,
    ) -> Result<(), DeployError> {
        let target = self
            .request
            .targets
            .get(index)
            .ok_or_else(|| DeployError::failed("部署目标索引无效"))?;
        let transaction = self
            .transactions
            .get(index)
            .ok_or_else(|| DeployError::failed("部署事务索引无效"))?;
        match target.manifest.target {
            ReleaseTarget::Frontend => {
                let metadata = remote
                    .metadata(&transaction.temp_path)?
                    .ok_or_else(|| DeployError::failed("远端前端临时目录不存在"))?;
                if metadata.kind != RemoteKind::Directory {
                    return Err(DeployError::failed("远端前端临时目标不是目录"));
                }
                let mut files = Vec::new();
                collect_remote_files(
                    remote,
                    &transaction.temp_path,
                    &transaction.temp_path,
                    &mut files,
                )?;
                files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
                if files != target.manifest.entries {
                    return Err(DeployError::failed("远端前端临时目录校验失败"));
                }
            }
            ReleaseTarget::Backend => {
                let metadata = remote
                    .metadata(&transaction.temp_path)?
                    .ok_or_else(|| DeployError::failed("远端后端临时文件不存在"))?;
                if metadata.kind != RemoteKind::File || metadata.size != target.manifest.total_bytes
                {
                    return Err(DeployError::failed("远端后端临时文件校验失败"));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_formal_targets(&self, remote: &dyn RemoteFs) -> Result<(), DeployError> {
        for target in &self.request.targets {
            let metadata = remote.metadata(&target.remote_path)?;
            if metadata.is_some() != target.expected_exists {
                return Err(DeployError::failed(format!(
                    "远端目标状态在预检后发生变化：{}",
                    target.remote_path
                )));
            }
            if let Some(metadata) = metadata {
                let expected_kind = match target.manifest.target {
                    ReleaseTarget::Frontend => RemoteKind::Directory,
                    ReleaseTarget::Backend => RemoteKind::File,
                };
                if metadata.kind != expected_kind {
                    return Err(DeployError::failed(format!(
                        "远端正式目标类型不符合配置：{}",
                        target.remote_path
                    )));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn cleanup_temps(&self, remote: &mut dyn RemoteFs) -> Vec<String> {
        let mut recovery_paths = Vec::new();
        for transaction in &self.transactions {
            match remote.metadata(&transaction.temp_path) {
                Ok(Some(_)) => {
                    if remote.remove_tree(&transaction.temp_path).is_err() {
                        recovery_paths.push(transaction.temp_path.clone());
                    }
                }
                Ok(None) => {}
                Err(_) => recovery_paths.push(transaction.temp_path.clone()),
            }
        }
        recovery_paths
    }

    pub(crate) fn cancel_and_cleanup(&self, remote: &mut dyn RemoteFs) -> Result<(), DeployError> {
        let mut error = DeployError::cancelled();
        error.recovery_paths.extend(self.cleanup_temps(remote));
        error.recovery_paths.sort();
        error.recovery_paths.dedup();
        Err(error)
    }

    pub(crate) fn commit(
        &mut self,
        remote: &mut dyn RemoteFs,
        cancelled: &AtomicBool,
    ) -> Result<(), DeployError> {
        if cancelled.load(Ordering::Acquire) {
            return self.cancel_and_cleanup(remote);
        }
        for index in 0..self.transactions.len() {
            if self.transactions[index].expected_exists {
                if let Err(error) = remote.rename(
                    &self.transactions[index].final_path,
                    &self.transactions[index].backup_path,
                ) {
                    let mut recovery_paths = rollback(remote, &self.transactions[..=index]);
                    recovery_paths.extend(self.cleanup_temps(remote));
                    recovery_paths.sort();
                    recovery_paths.dedup();
                    return Err(DeployError {
                        message: format!("远端提交失败：{error:?}"),
                        cancelled: false,
                        committed: false,
                        recovery_paths,
                    });
                }
                self.transactions[index].backed_up = true;
            }
            if let Err(error) = remote.rename(
                &self.transactions[index].temp_path,
                &self.transactions[index].final_path,
            ) {
                let mut recovery_paths = rollback(remote, &self.transactions[..=index]);
                recovery_paths.extend(self.cleanup_temps(remote));
                recovery_paths.sort();
                recovery_paths.dedup();
                let rollback_failed = !recovery_paths.is_empty();
                return Err(DeployError {
                    message: if rollback_failed {
                        format!("远端提交失败且回滚失败：{error:?}")
                    } else {
                        format!("远端提交失败：{error:?}")
                    },
                    cancelled: false,
                    committed: false,
                    recovery_paths,
                });
            }
            self.transactions[index].committed = true;
        }

        let mut recovery_paths = Vec::new();
        for transaction in &self.transactions {
            if transaction.backed_up && remote.remove_tree(&transaction.backup_path).is_err() {
                recovery_paths.push(transaction.backup_path.clone());
            }
        }
        if !recovery_paths.is_empty() {
            return Err(DeployError {
                message: "远端提交成功，但旧版本备份清理失败".into(),
                cancelled: false,
                committed: true,
                recovery_paths,
            });
        }
        Ok(())
    }
}

fn transaction_path(final_path: &str, kind: &str, run_id: &str) -> String {
    format!("{final_path}.__lazycat_{kind}_{run_id}")
}

fn validate_request(request: &DeploymentRequest) -> Result<(), DeployError> {
    if request.targets.is_empty() {
        return Err(DeployError::failed("没有可部署的目标"));
    }
    if request.run_id.is_empty()
        || request.run_id.len() > 64
        || !request
            .run_id
            .bytes()
            .all(|value| value.is_ascii_alphanumeric() || value == b'-')
    {
        return Err(DeployError::failed("runId 包含不安全字符"));
    }
    validate_remote_target_paths(&request.targets)
}

fn collect_frontend_directories(
    request: &DeploymentRequest,
    transactions: &[TransactionTarget],
) -> Result<Vec<String>, DeployError> {
    let mut directories = BTreeSet::new();
    for (target, transaction) in request.targets.iter().zip(transactions) {
        if target.manifest.target != ReleaseTarget::Frontend {
            continue;
        }
        directories.insert(transaction.temp_path.clone());
        for entry in &target.manifest.entries {
            let mut current = transaction.temp_path.clone();
            let mut segments = entry.relative_path.split('/').peekable();
            while let Some(segment) = segments.next() {
                if segments.peek().is_none() {
                    break;
                }
                current.push('/');
                current.push_str(segment);
                directories.insert(current.clone());
            }
        }
    }
    let mut directories = directories.into_iter().collect::<Vec<_>>();
    directories.sort_by_key(|path| (path.split('/').count(), path.clone()));
    Ok(directories)
}

fn validate_remote_target_paths(targets: &[DeploymentTarget]) -> Result<(), DeployError> {
    for target in targets {
        let path = target.remote_path.as_str();
        if !path.starts_with('/')
            || path == "/"
            || path.ends_with('/')
            || path.contains("//")
            || path.contains('\\')
            || path.contains('\0')
            || path
                .split('/')
                .any(|segment| segment == "." || segment == "..")
        {
            return Err(DeployError::failed(format!(
                "远端部署目标必须是规范的 Linux 绝对路径：{path}"
            )));
        }
    }

    for (index, target) in targets.iter().enumerate() {
        for other in &targets[index + 1..] {
            let target_prefix = format!("{}/", target.remote_path);
            let other_prefix = format!("{}/", other.remote_path);
            if target.remote_path == other.remote_path
                || other.remote_path.starts_with(&target_prefix)
                || target.remote_path.starts_with(&other_prefix)
            {
                return Err(DeployError::failed("远端部署目标不能互相包含或重复"));
            }
        }
    }
    Ok(())
}

fn local_entry_path(manifest: &ArtifactManifest, entry: &ArtifactEntry) -> PathBuf {
    if manifest.target == ReleaseTarget::Backend {
        return manifest.source_path.clone();
    }
    entry
        .relative_path
        .split('/')
        .fold(manifest.source_path.clone(), |path, segment| {
            path.join(segment)
        })
}

fn collect_remote_files(
    remote: &dyn RemoteFs,
    root: &str,
    current: &str,
    files: &mut Vec<ArtifactEntry>,
) -> Result<(), DeployError> {
    for entry in remote.read_dir(current)? {
        match entry.metadata.kind {
            RemoteKind::Directory => collect_remote_files(remote, root, &entry.path, files)?,
            RemoteKind::File => {
                let relative = entry
                    .path
                    .strip_prefix(root)
                    .and_then(|value| value.strip_prefix('/'))
                    .ok_or_else(|| DeployError::failed("远端清单路径逃逸"))?;
                files.push(ArtifactEntry {
                    relative_path: relative.to_string(),
                    size: entry.metadata.size,
                });
            }
            RemoteKind::Symlink | RemoteKind::Other => {
                return Err(DeployError::failed(format!(
                    "远端临时目标包含不支持的文件类型：{}",
                    entry.path
                )))
            }
        }
    }
    Ok(())
}

fn rollback(remote: &mut dyn RemoteFs, transactions: &[TransactionTarget]) -> Vec<String> {
    let mut recovery_paths = Vec::new();
    for transaction in transactions.iter().rev() {
        if transaction.committed {
            if remote.remove_tree(&transaction.final_path).is_err() {
                recovery_paths.push(transaction.final_path.clone());
                if transaction.backed_up {
                    recovery_paths.push(transaction.backup_path.clone());
                }
                continue;
            }
            if transaction.backed_up
                && remote
                    .rename(&transaction.backup_path, &transaction.final_path)
                    .is_err()
            {
                recovery_paths.push(transaction.backup_path.clone());
            }
        } else if transaction.backed_up
            && remote
                .rename(&transaction.backup_path, &transaction.final_path)
                .is_err()
        {
            recovery_paths.push(transaction.backup_path.clone());
        }
    }
    recovery_paths.sort();
    recovery_paths.dedup();
    recovery_paths
}

pub fn deploy(
    remote: &mut dyn RemoteFs,
    request: &DeploymentRequest,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(u64, &str),
) -> Result<(), DeployError> {
    let mut plan = DeploymentPlan::new(request.clone())?;
    if let Err(mut error) = plan.prepare_remote(remote) {
        error.recovery_paths.extend(plan.cleanup_temps(remote));
        error.recovery_paths.sort();
        error.recovery_paths.dedup();
        return Err(error);
    }
    for index in 0..plan.target_count() {
        if let Err(mut error) = plan.upload_target(index, remote, cancelled, &mut progress) {
            error.recovery_paths.extend(plan.cleanup_temps(remote));
            error.recovery_paths.sort();
            error.recovery_paths.dedup();
            return Err(error);
        }
        if let Err(mut error) = plan.verify_target(index, remote) {
            error.recovery_paths.extend(plan.cleanup_temps(remote));
            error.recovery_paths.sort();
            error.recovery_paths.dedup();
            return Err(error);
        }
    }
    if cancelled.load(Ordering::Acquire) {
        return plan.cancel_and_cleanup(remote);
    }
    if let Err(mut error) = plan.validate_formal_targets(remote) {
        error.recovery_paths.extend(plan.cleanup_temps(remote));
        error.recovery_paths.sort();
        error.recovery_paths.dedup();
        return Err(error);
    }
    plan.commit(remote, cancelled)
}
#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::ArtifactManifest;
    use crate::tools::release_package::ReleaseTarget;

    pub(super) struct TestDir(PathBuf);

    impl TestDir {
        pub(super) fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "lazycat-release-deploy-test-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        pub(super) fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn manifest_rejects_changed_files() {
        let root = TestDir::new();
        let source = root.path().join("dist");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("index.html"), "v1").unwrap();
        let manifest = ArtifactManifest::from_directory(ReleaseTarget::Frontend, &source).unwrap();
        fs::write(source.join("index.html"), "changed").unwrap();
        assert!(manifest.verify_source().unwrap_err().contains("发生变化"));
    }

    #[test]
    fn manifests_support_empty_directories_and_backend_files() {
        let root = TestDir::new();
        let empty = root.path().join("empty");
        fs::create_dir(&empty).unwrap();
        let frontend = ArtifactManifest::from_directory(ReleaseTarget::Frontend, &empty).unwrap();
        assert_eq!(frontend.file_count, 0);
        assert_eq!(frontend.total_bytes, 0);

        let jar = root.path().join("app.jar");
        fs::write(&jar, "jar").unwrap();
        let backend = ArtifactManifest::from_file(ReleaseTarget::Backend, &jar).unwrap();
        assert_eq!(backend.file_count, 1);
        assert_eq!(backend.total_bytes, 3);
    }
}

#[cfg(test)]
mod transaction_tests {
    use std::cell::RefCell;
    use std::collections::{BTreeMap, VecDeque};
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::{
        deploy, ArtifactManifest, DeployError, DeploymentRequest, DeploymentTarget, RemoteDirEntry,
        RemoteFs, RemoteKind, RemoteMetadata,
    };
    use crate::tools::release_package::ReleaseTarget;

    #[derive(Clone)]
    enum Node {
        Directory,
        File(Vec<u8>),
    }

    struct FakeRemoteFs {
        nodes: BTreeMap<String, Node>,
        metadata_calls: RefCell<Vec<String>>,
        create_dir_calls: Vec<String>,
        fail_rename_targets: VecDeque<String>,
        fail_remove_targets: VecDeque<String>,
        cancel_during_write: bool,
    }

    impl FakeRemoteFs {
        fn base() -> Self {
            let mut nodes = BTreeMap::new();
            for path in ["/", "/srv", "/srv/app"] {
                nodes.insert(path.into(), Node::Directory);
            }
            Self {
                nodes,
                metadata_calls: RefCell::new(Vec::new()),
                create_dir_calls: Vec::new(),
                fail_rename_targets: VecDeque::new(),
                fail_remove_targets: VecDeque::new(),
                cancel_during_write: false,
            }
        }

        fn with_existing_release() -> Self {
            let mut remote = Self::base();
            remote.nodes.insert("/srv/app/web".into(), Node::Directory);
            remote
                .nodes
                .insert("/srv/app/web/old.js".into(), Node::File(b"old".to_vec()));
            remote
                .nodes
                .insert("/srv/app/app.jar".into(), Node::File(b"old-jar".to_vec()));
            remote
        }

        fn exists(&self, path: &str) -> bool {
            self.nodes.contains_key(path)
        }

        fn read(&self, path: &str) -> Vec<u8> {
            match self.nodes.get(path).unwrap() {
                Node::File(value) => value.clone(),
                Node::Directory => panic!("not a file: {path}"),
            }
        }

        fn any_path_contains(&self, value: &str) -> bool {
            self.nodes.keys().any(|path| path.contains(value))
        }

        fn fail_rename_to(&mut self, path: &str) {
            self.fail_rename_targets.push_back(path.into());
        }

        fn fail_remove_tree(&mut self, path: &str) {
            self.fail_remove_targets.push_back(path.into());
        }
    }

    impl RemoteFs for FakeRemoteFs {
        fn metadata(&self, path: &str) -> Result<Option<RemoteMetadata>, DeployError> {
            self.metadata_calls.borrow_mut().push(path.to_string());
            Ok(self.nodes.get(path).map(|node| match node {
                Node::Directory => RemoteMetadata {
                    kind: RemoteKind::Directory,
                    size: 0,
                },
                Node::File(value) => RemoteMetadata {
                    kind: RemoteKind::File,
                    size: value.len() as u64,
                },
            }))
        }

        fn create_dir(&mut self, path: &str) -> Result<(), DeployError> {
            self.create_dir_calls.push(path.to_string());
            if let Some(node) = self.nodes.get(path) {
                return match node {
                    Node::Directory => Ok(()),
                    Node::File(_) => Err(DeployError::failed("远端目录路径已被文件占用")),
                };
            }
            self.nodes.insert(path.into(), Node::Directory);
            Ok(())
        }

        fn read_dir(&self, path: &str) -> Result<Vec<RemoteDirEntry>, DeployError> {
            let prefix = format!("{}/", path.trim_end_matches('/'));
            let mut entries = Vec::new();
            for (entry_path, node) in &self.nodes {
                let Some(relative) = entry_path.strip_prefix(&prefix) else {
                    continue;
                };
                if relative.is_empty() || relative.contains('/') {
                    continue;
                }
                let metadata = match node {
                    Node::Directory => RemoteMetadata {
                        kind: RemoteKind::Directory,
                        size: 0,
                    },
                    Node::File(value) => RemoteMetadata {
                        kind: RemoteKind::File,
                        size: value.len() as u64,
                    },
                };
                entries.push(RemoteDirEntry {
                    path: entry_path.clone(),
                    metadata,
                });
            }
            Ok(entries)
        }

        fn write_file(
            &mut self,
            remote_path: &str,
            local_path: &Path,
            cancelled: &AtomicBool,
            progress: &mut dyn FnMut(u64),
        ) -> Result<(), DeployError> {
            if self.cancel_during_write {
                cancelled.store(true, Ordering::Release);
                return Err(DeployError::cancelled());
            }
            if cancelled.load(Ordering::Acquire) {
                return Err(DeployError::cancelled());
            }
            let content = fs::read(local_path).map_err(DeployError::local_io)?;
            progress(content.len() as u64);
            self.nodes.insert(remote_path.into(), Node::File(content));
            Ok(())
        }

        fn rename(&mut self, source: &str, target: &str) -> Result<(), DeployError> {
            if self.fail_rename_targets.front().map(String::as_str) == Some(target) {
                self.fail_rename_targets.pop_front();
                return Err(DeployError::failed("injected rename failure"));
            }
            let affected = self
                .nodes
                .keys()
                .filter(|path| *path == source || path.starts_with(&format!("{source}/")))
                .cloned()
                .collect::<Vec<_>>();
            if affected.is_empty() {
                return Err(DeployError::failed("rename source missing"));
            }
            for old_path in affected {
                let node = self.nodes.remove(&old_path).unwrap();
                let suffix = &old_path[source.len()..];
                self.nodes.insert(format!("{target}{suffix}"), node);
            }
            Ok(())
        }

        fn remove_tree(&mut self, path: &str) -> Result<(), DeployError> {
            if self.fail_remove_targets.front().map(String::as_str) == Some(path) {
                self.fail_remove_targets.pop_front();
                return Err(DeployError::failed("injected remove failure"));
            }
            let affected = self
                .nodes
                .keys()
                .filter(|candidate| {
                    *candidate == path || candidate.starts_with(&format!("{path}/"))
                })
                .cloned()
                .collect::<Vec<_>>();
            for candidate in affected {
                self.nodes.remove(&candidate);
            }
            Ok(())
        }
    }

    fn local_manifests() -> (super::tests::TestDir, ArtifactManifest, ArtifactManifest) {
        let root = super::tests::TestDir::new();
        let frontend = root.path().join("dist");
        let backend = root.path().join("app.jar");
        fs::create_dir_all(frontend.join("assets")).unwrap();
        fs::write(frontend.join("index.html"), "new-web").unwrap();
        fs::write(frontend.join("assets/app.js"), "new-js").unwrap();
        fs::write(&backend, "new-jar").unwrap();
        let frontend_manifest =
            ArtifactManifest::from_directory(ReleaseTarget::Frontend, &frontend).unwrap();
        let backend_manifest =
            ArtifactManifest::from_file(ReleaseTarget::Backend, &backend).unwrap();
        (root, frontend_manifest, backend_manifest)
    }

    fn two_target_request() -> (super::tests::TestDir, DeploymentRequest) {
        let (root, frontend, backend) = local_manifests();
        let request = DeploymentRequest {
            run_id: "run-1".into(),
            targets: vec![
                DeploymentTarget {
                    manifest: frontend,
                    remote_path: "/srv/app/web".into(),
                    expected_exists: true,
                },
                DeploymentTarget {
                    manifest: backend,
                    remote_path: "/srv/app/app.jar".into(),
                    expected_exists: true,
                },
            ],
        };
        (root, request)
    }

    #[test]
    fn deployment_plan_deduplicates_frontend_directories_parent_first() {
        let (root, request) = two_target_request();
        let plan = super::DeploymentPlan::new(request).unwrap();
        drop(root);

        assert_eq!(
            plan.frontend_directories(),
            &[
                "/srv/app/web.__lazycat_tmp_run-1".to_string(),
                "/srv/app/web.__lazycat_tmp_run-1/assets".to_string(),
            ]
        );
    }

    #[test]
    fn deployment_plan_rejects_changed_sources_before_remote_prepare() {
        let (root, request) = two_target_request();
        fs::write(root.path().join("dist/index.html"), "changed-size").unwrap();

        let error = super::DeploymentPlan::new(request).unwrap_err();
        drop(root);

        assert!(error.message.contains("部署产物在打包后发生变化"));
    }

    #[test]
    fn deployment_plan_keeps_backend_without_frontend_directories() {
        let (root, mut request) = two_target_request();
        request.targets.remove(0);
        let plan = super::DeploymentPlan::new(request).unwrap();
        drop(root);

        assert!(plan.frontend_directories().is_empty());
    }

    #[test]
    fn deployment_prepares_each_frontend_directory_once() {
        let (root, request) = two_target_request();
        let plan = super::DeploymentPlan::new(request).unwrap();
        let mut remote = FakeRemoteFs::with_existing_release();

        plan.prepare_remote(&mut remote).unwrap();
        drop(root);

        assert_eq!(
            remote
                .create_dir_calls
                .iter()
                .filter(|path| path.contains("__lazycat_tmp_run-1"))
                .count(),
            2
        );
        for directory in plan.frontend_directories() {
            assert_eq!(
                remote
                    .metadata_calls
                    .borrow()
                    .iter()
                    .filter(|path| *path == directory)
                    .count(),
                1,
                "directory was checked more than once: {directory}",
            );
        }
    }

    #[test]
    fn deployment_prepares_temp_root_for_empty_frontend() {
        let root = super::tests::TestDir::new();
        let source = root.path().join("empty");
        fs::create_dir_all(&source).unwrap();
        let request = DeploymentRequest {
            run_id: "run-empty".into(),
            targets: vec![DeploymentTarget {
                manifest: ArtifactManifest::from_directory(ReleaseTarget::Frontend, &source)
                    .unwrap(),
                remote_path: "/srv/app/empty-web".into(),
                expected_exists: false,
            }],
        };
        let plan = super::DeploymentPlan::new(request).unwrap();
        let mut remote = FakeRemoteFs::base();

        plan.prepare_remote(&mut remote).unwrap();

        assert!(remote.exists("/srv/app/empty-web.__lazycat_tmp_run-empty"));
    }

    #[test]
    fn sftp_remote_fs_is_send() {
        fn assert_send<T: Send>() {}

        assert_send::<crate::tools::release_package_remote::SftpRemoteFs>();
    }

    #[test]
    fn deployment_rejects_an_unsafe_run_id_before_remote_changes() {
        let (root, mut request) = two_target_request();
        request.run_id = "../escape".into();
        let mut remote = FakeRemoteFs::with_existing_release();
        let error = deploy(&mut remote, &request, &AtomicBool::new(false), |_, _| {}).unwrap_err();
        drop(root);

        assert!(error.message.contains("runId"));
        assert_eq!(remote.read("/srv/app/web/old.js"), b"old");
        assert!(!remote.any_path_contains("__lazycat_tmp_"));
    }

    #[test]
    fn deployment_rejects_a_non_canonical_remote_path_before_remote_changes() {
        let (root, mut request) = two_target_request();
        request.targets[0].remote_path = "/srv/app/web/".into();
        let mut remote = FakeRemoteFs::with_existing_release();
        let error = deploy(&mut remote, &request, &AtomicBool::new(false), |_, _| {}).unwrap_err();
        drop(root);

        assert!(error.message.contains("规范的 Linux 绝对路径"));
        assert_eq!(remote.read("/srv/app/web/old.js"), b"old");
        assert!(!remote.any_path_contains("__lazycat_tmp_"));
    }

    #[test]
    fn deployment_rejects_nested_formal_targets_before_remote_changes() {
        let (root, mut request) = two_target_request();
        request.targets[0].remote_path = "/srv/app".into();
        let mut remote = FakeRemoteFs::with_existing_release();
        let error = deploy(&mut remote, &request, &AtomicBool::new(false), |_, _| {}).unwrap_err();
        drop(root);

        assert!(error.message.contains("不能互相包含"));
        assert_eq!(remote.read("/srv/app/web/old.js"), b"old");
        assert_eq!(remote.read("/srv/app/app.jar"), b"old-jar");
        assert!(!remote.any_path_contains("__lazycat_tmp_"));
    }

    #[test]
    fn deployment_replaces_targets_without_mixing_old_files() {
        let (root, request) = two_target_request();
        let mut remote = FakeRemoteFs::with_existing_release();
        deploy(&mut remote, &request, &AtomicBool::new(false), |_, _| {}).unwrap();
        drop(root);

        assert!(!remote.exists("/srv/app/web/old.js"));
        assert!(remote.exists("/srv/app/web/index.html"));
        assert!(remote.exists("/srv/app/web/assets/app.js"));
        assert!(!remote.exists("/srv/app/web/dist/index.html"));
        assert!(!remote.exists("/srv/app/web/dist/assets/app.js"));
        assert!(!remote.any_path_contains("__lazycat_tmp_"));
        assert!(!remote.any_path_contains("__lazycat_backup_"));
    }

    #[test]
    fn committed_deployment_reports_backup_cleanup_recovery_paths() {
        let (root, request) = two_target_request();
        let backup_path = "/srv/app/web.__lazycat_backup_run-1";
        let mut remote = FakeRemoteFs::with_existing_release();
        remote.fail_remove_tree(backup_path);

        let error = deploy(&mut remote, &request, &AtomicBool::new(false), |_, _| {}).unwrap_err();
        drop(root);

        assert!(error.committed);
        assert_eq!(error.recovery_paths, vec![backup_path.to_string()]);
        assert!(remote.exists("/srv/app/web/index.html"));
        assert_eq!(remote.read("/srv/app/app.jar"), b"new-jar");
    }

    #[test]
    fn deployment_progress_includes_the_current_remote_path() {
        let (root, request) = two_target_request();
        let mut remote = FakeRemoteFs::with_existing_release();
        let mut progress = Vec::new();
        deploy(
            &mut remote,
            &request,
            &AtomicBool::new(false),
            |bytes, path| progress.push((bytes, path.to_string())),
        )
        .unwrap();
        drop(root);

        assert!(progress.iter().any(|(_, path)| path == "index.html"));
        assert!(progress.iter().any(|(_, path)| path == "app.jar"));
    }

    #[test]
    fn serial_deploy_still_rolls_back_after_second_commit_failure() {
        let (root, request) = two_target_request();
        let mut remote = FakeRemoteFs::with_existing_release();
        remote.fail_rename_to("/srv/app/app.jar");
        let error = deploy(&mut remote, &request, &AtomicBool::new(false), |_, _| {}).unwrap_err();
        drop(root);

        assert!(error.message.contains("远端提交失败"));
        assert_eq!(remote.read("/srv/app/web/old.js"), b"old");
        assert_eq!(remote.read("/srv/app/app.jar"), b"old-jar");
    }

    #[test]
    fn rollback_failure_reports_recovery_paths_without_deleting_backup() {
        let (root, request) = two_target_request();
        let mut remote = FakeRemoteFs::with_existing_release();
        remote.fail_rename_to("/srv/app/app.jar");
        remote.fail_rename_to("/srv/app/web");
        let error = deploy(&mut remote, &request, &AtomicBool::new(false), |_, _| {}).unwrap_err();
        drop(root);

        assert!(error.message.contains("回滚失败"));
        assert!(error
            .recovery_paths
            .iter()
            .any(|path| path.contains("__lazycat_backup_")));
    }

    #[test]
    fn cancellation_keeps_formal_targets_and_cleans_temporary_paths() {
        let (root, request) = two_target_request();
        let mut remote = FakeRemoteFs::with_existing_release();
        remote.cancel_during_write = true;
        let error = deploy(&mut remote, &request, &AtomicBool::new(false), |_, _| {}).unwrap_err();
        drop(root);

        assert!(error.cancelled);
        assert_eq!(remote.read("/srv/app/web/old.js"), b"old");
        assert_eq!(remote.read("/srv/app/app.jar"), b"old-jar");
        assert!(!remote.any_path_contains("__lazycat_tmp_"));
    }
}

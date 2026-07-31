use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use super::release_package_artifact::{ArtifactEntry, ArtifactManifest};
use super::release_package_model::ReleaseTarget;
use super::release_package_transfer::{create_frontend_transfer, FrontendTransferArchive};
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteCommandResult {
    pub exit_code: i32,
}

#[derive(Default)]
pub(crate) struct RemoteCommandOutputDecoder {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl RemoteCommandOutputDecoder {
    pub(crate) fn push(
        &mut self,
        stream: &'static str,
        bytes: &[u8],
        output: &mut dyn FnMut(&str, String),
    ) {
        let buffer = match stream {
            "stderr" => &mut self.stderr,
            _ => &mut self.stdout,
        };
        buffer.extend_from_slice(bytes);
        while let Some(index) = buffer.iter().position(|byte| *byte == b'\n') {
            let line = buffer.drain(..=index).collect::<Vec<_>>();
            emit_command_line(stream, line, output);
        }
    }

    pub(crate) fn flush(&mut self, output: &mut dyn FnMut(&str, String)) {
        if !self.stdout.is_empty() {
            let line = std::mem::take(&mut self.stdout);
            emit_command_line("stdout", line, output);
        }
        if !self.stderr.is_empty() {
            let line = std::mem::take(&mut self.stderr);
            emit_command_line("stderr", line, output);
        }
    }
}

fn emit_command_line(
    stream: &'static str,
    mut line: Vec<u8>,
    output: &mut dyn FnMut(&str, String),
) {
    if line.last() == Some(&b'\n') {
        line.pop();
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    output(stream, String::from_utf8_lossy(&line).into_owned());
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

    pub fn cancelled_command() -> Self {
        Self {
            message: "上传后命令已取消".into(),
            cancelled: true,
            committed: false,
            recovery_paths: Vec::new(),
        }
    }

    pub fn cancelled_compression() -> Self {
        Self {
            message: "前端传输包压缩已取消".into(),
            cancelled: true,
            committed: false,
            recovery_paths: Vec::new(),
        }
    }

    pub fn cancelled_extraction() -> Self {
        Self {
            message: "远端前端传输包解压已取消".into(),
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
    fn extract_tar_gz(
        &mut self,
        archive_path: &str,
        destination: &str,
        cancelled: &AtomicBool,
    ) -> Result<(), DeployError>;
    fn rename(&mut self, source: &str, target: &str) -> Result<(), DeployError>;
    fn remove_tree(&mut self, path: &str) -> Result<(), DeployError>;
    fn execute_command(
        &mut self,
        command: &str,
        cancelled: &AtomicBool,
        output: &mut dyn FnMut(&str, String),
    ) -> Result<RemoteCommandResult, DeployError>;
}

#[derive(Clone, Debug)]
pub struct DeploymentTarget {
    pub manifest: ArtifactManifest,
    pub remote_path: String,
    pub expected_exists: bool,
    pub frontend_archive: Option<FrontendTransferArchive>,
}

impl DeploymentTarget {
    pub fn transfer_bytes(&self) -> u64 {
        self.frontend_archive
            .as_ref()
            .map(|archive| archive.compressed_bytes)
            .unwrap_or(self.manifest.total_bytes)
    }
}

#[derive(Clone, Debug)]
pub struct DeploymentRequest {
    pub run_id: String,
    pub targets: Vec<DeploymentTarget>,
}

pub struct DeploymentSuccess {
    pub control: Box<dyn RemoteFs>,
    pub warning: Option<DeployError>,
}

impl std::fmt::Debug for DeploymentSuccess {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeploymentSuccess")
            .field("control", &"<remote>")
            .field("warning", &self.warning)
            .finish()
    }
}

impl DeploymentSuccess {
    fn complete(control: Box<dyn RemoteFs>) -> Self {
        Self {
            control,
            warning: None,
        }
    }

    fn from_commit(
        control: Box<dyn RemoteFs>,
        commit: Result<(), DeployError>,
    ) -> Result<Self, DeployError> {
        match commit {
            Ok(()) => Ok(Self::complete(control)),
            Err(error) if error.committed => Ok(Self {
                control,
                warning: Some(error),
            }),
            Err(error) => Err(error),
        }
    }
}

#[derive(Clone, Debug)]
struct TransactionTarget {
    final_path: String,
    temp_path: String,
    backup_path: String,
    transfer_path: Option<String>,
    expected_exists: bool,
    backed_up: bool,
    committed: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct DeploymentPlan {
    request: DeploymentRequest,
    transactions: Vec<TransactionTarget>,
    frontend_directories: Vec<String>,
    owned_temp_paths: Arc<Mutex<BTreeSet<String>>>,
}

impl DeploymentPlan {
    pub(crate) fn new(mut request: DeploymentRequest) -> Result<Self, DeployError> {
        validate_request(&request)?;
        for target in &mut request.targets {
            target
                .manifest
                .verify_source()
                .map_err(DeployError::failed)?;
            if target.manifest.target == ReleaseTarget::Frontend
                && target.frontend_archive.is_none()
            {
                target.frontend_archive = Some(
                    create_frontend_transfer(&target.manifest, &AtomicBool::new(false))
                        .map_err(|error| DeployError::failed(error.message))?,
                );
            }
        }
        let transactions = request
            .targets
            .iter()
            .map(|target| TransactionTarget {
                final_path: target.remote_path.clone(),
                temp_path: transaction_path(&target.remote_path, "tmp", &request.run_id),
                backup_path: transaction_path(&target.remote_path, "backup", &request.run_id),
                transfer_path: (target.manifest.target == ReleaseTarget::Frontend)
                    .then(|| frontend_transfer_path(&target.remote_path, &request.run_id)),
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
            owned_temp_paths: Arc::new(Mutex::new(BTreeSet::new())),
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
        let mut paths = self
            .transactions
            .iter()
            .flat_map(|transaction| {
                std::iter::once(transaction.temp_path.clone())
                    .chain(transaction.transfer_path.iter().cloned())
            })
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }

    pub(crate) fn prepare_remote(
        &self,
        remote: &mut dyn RemoteFs,
        cancelled: &AtomicBool,
    ) -> Result<(), DeployError> {
        for transaction in &self.transactions {
            check_cancelled(cancelled)?;
            let temp_exists = remote.metadata(&transaction.temp_path)?.is_some();
            check_cancelled(cancelled)?;
            let backup_exists = remote.metadata(&transaction.backup_path)?.is_some();
            check_cancelled(cancelled)?;
            let transfer_exists = match &transaction.transfer_path {
                Some(path) => remote.metadata(path)?.is_some(),
                None => false,
            };
            check_cancelled(cancelled)?;
            if temp_exists || backup_exists || transfer_exists {
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
            check_cancelled(cancelled)?;
            if !frontend_roots.contains(directory.as_str()) {
                match remote.metadata(directory)? {
                    Some(metadata) if metadata.kind == RemoteKind::Directory => {
                        check_cancelled(cancelled)?;
                        continue;
                    }
                    Some(_) => {
                        return Err(DeployError::failed(format!(
                            "远端目录路径已被非目录占用：{directory}"
                        )))
                    }
                    None => {}
                }
                check_cancelled(cancelled)?;
            }
            remote.create_dir(directory)?;
            if frontend_roots.contains(directory.as_str()) {
                self.mark_temp_owned(directory);
            }
            check_cancelled(cancelled)?;
        }
        Ok(())
    }

    pub(crate) fn upload_target(
        &self,
        index: usize,
        remote: &mut dyn RemoteFs,
        cancelled: &AtomicBool,
        progress: &mut dyn FnMut(u64, &str),
        event: &dyn Fn(DeployEvent),
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
                let archive = target
                    .frontend_archive
                    .as_ref()
                    .ok_or_else(|| DeployError::failed("前端传输包尚未生成"))?;
                let remote_path = transaction
                    .transfer_path
                    .as_ref()
                    .ok_or_else(|| DeployError::failed("前端远端传输包路径缺失"))?;
                let archive_name = remote_path.rsplit('/').next().unwrap_or(remote_path);
                self.mark_temp_owned(remote_path);
                let upload_started_at = Instant::now();
                let mut file_progress = |bytes| progress(bytes, archive_name);
                remote.write_file(remote_path, archive.path(), cancelled, &mut file_progress)?;
                event(DeployEvent::FrontendArchiveUploaded {
                    duration: upload_started_at.elapsed(),
                });
                let metadata = remote
                    .metadata(remote_path)?
                    .ok_or_else(|| DeployError::failed("远端前端传输包不存在"))?;
                if metadata.kind != RemoteKind::File || metadata.size != archive.compressed_bytes {
                    return Err(DeployError::failed("远端前端传输包大小校验失败"));
                }
                check_cancelled(cancelled)?;
                let extract_started_at = Instant::now();
                remote.extract_tar_gz(remote_path, &transaction.temp_path, cancelled)?;
                event(DeployEvent::FrontendArchiveExtracted {
                    duration: extract_started_at.elapsed(),
                });
                check_cancelled(cancelled)?;
                remote.remove_tree(remote_path).map_err(|error| {
                    DeployError::failed(format!("清理远端前端传输包失败：{}", error.message))
                })?;
                self.unmark_temp_owned(remote_path);
            }
            ReleaseTarget::Backend => {
                let entry = target
                    .manifest
                    .entries
                    .first()
                    .ok_or_else(|| DeployError::failed("后端部署清单缺少文件"))?;
                self.mark_temp_owned(&transaction.temp_path);
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
        let owned_temp_paths = self
            .owned_temp_paths
            .lock()
            .expect("owned temp paths lock poisoned")
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        for temp_path in owned_temp_paths {
            let cleaned = match remote.metadata(&temp_path) {
                Ok(Some(_)) => {
                    if remote.remove_tree(&temp_path).is_err() {
                        recovery_paths.push(temp_path.clone());
                        false
                    } else {
                        true
                    }
                }
                Ok(None) => true,
                Err(_) => {
                    recovery_paths.push(temp_path.clone());
                    false
                }
            };
            if cleaned {
                self.owned_temp_paths
                    .lock()
                    .expect("owned temp paths lock poisoned")
                    .remove(&temp_path);
            }
        }
        recovery_paths
    }

    fn mark_temp_owned(&self, temp_path: &str) {
        self.owned_temp_paths
            .lock()
            .expect("owned temp paths lock poisoned")
            .insert(temp_path.to_string());
    }

    fn unmark_temp_owned(&self, temp_path: &str) {
        self.owned_temp_paths
            .lock()
            .expect("owned temp paths lock poisoned")
            .remove(temp_path);
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
            if cancelled.load(Ordering::Acquire) {
                return self.cancel_during_commit(remote, index);
            }
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
            if cancelled.load(Ordering::Acquire) {
                return self.cancel_during_commit(remote, index + 1);
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

    fn cancel_during_commit(
        &self,
        remote: &mut dyn RemoteFs,
        transaction_count: usize,
    ) -> Result<(), DeployError> {
        let mut error = DeployError::cancelled();
        error
            .recovery_paths
            .extend(rollback(remote, &self.transactions[..transaction_count]));
        error.recovery_paths.extend(self.cleanup_temps(remote));
        error.recovery_paths.sort();
        error.recovery_paths.dedup();
        Err(error)
    }
}

fn transaction_path(final_path: &str, kind: &str, run_id: &str) -> String {
    format!("{final_path}.__lazycat_{kind}_{run_id}")
}

fn frontend_transfer_path(final_path: &str, run_id: &str) -> String {
    let parent = final_path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    format!("{parent}/.lazycat-upload-{run_id}-frontend.tar.gz")
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), DeployError> {
    if cancelled.load(Ordering::Acquire) {
        Err(DeployError::cancelled())
    } else {
        Ok(())
    }
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

#[derive(Clone, Debug)]
pub enum DeployEvent {
    FrontendArchiveUploaded { duration: Duration },
    FrontendArchiveExtracted { duration: Duration },
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
    if let Err(mut error) = plan.prepare_remote(remote, cancelled) {
        error.recovery_paths.extend(plan.cleanup_temps(remote));
        error.recovery_paths.sort();
        error.recovery_paths.dedup();
        return Err(error);
    }
    for index in 0..plan.target_count() {
        if let Err(mut error) = plan.upload_target(index, remote, cancelled, &mut progress, &|_| {})
        {
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

pub(crate) fn deploy_parallel(
    remotes: Vec<Box<dyn RemoteFs>>,
    plan: DeploymentPlan,
    user_cancelled: Arc<AtomicBool>,
    stop_requested: Arc<AtomicBool>,
    progress: Arc<dyn Fn(u64, &str) + Send + Sync>,
    event: Arc<dyn Fn(DeployEvent) + Send + Sync>,
    interrupt_transport: Arc<dyn Fn() + Send + Sync>,
    recover_remote: Arc<dyn Fn() -> Result<Box<dyn RemoteFs>, DeployError> + Send + Sync>,
) -> Result<DeploymentSuccess, DeployError> {
    if remotes.len() != plan.target_count() || remotes.is_empty() || remotes.len() > 2 {
        return Err(DeployError::failed("SSH 会话数量与部署目标不一致"));
    }

    let mut remotes = remotes;
    let mut control = remotes.remove(0);
    if let Err(mut error) = plan.prepare_remote(control.as_mut(), user_cancelled.as_ref()) {
        if user_cancelled.load(Ordering::Acquire) {
            return match recover_remote() {
                Ok(mut recovery) => plan
                    .cancel_and_cleanup(recovery.as_mut())
                    .map(|()| DeploymentSuccess::complete(recovery)),
                Err(recovery_error) => {
                    error.message = format!(
                        "{}；无法建立恢复 SSH 会话清理临时目标：{}",
                        error.message, recovery_error.message
                    );
                    error.recovery_paths.extend(plan.temp_paths());
                    error.recovery_paths.sort();
                    error.recovery_paths.dedup();
                    Err(error)
                }
            };
        }
        error
            .recovery_paths
            .extend(plan.cleanup_temps(control.as_mut()));
        error.recovery_paths.sort();
        error.recovery_paths.dedup();
        return Err(error);
    }

    if plan.target_count() == 1 {
        let mut plan = plan;
        let upload = plan
            .upload_target(
                0,
                control.as_mut(),
                stop_requested.as_ref(),
                &mut |bytes, path| progress(bytes, path),
                event.as_ref(),
            )
            .and_then(|()| plan.verify_target(0, control.as_ref()))
            .and_then(|()| plan.validate_formal_targets(control.as_ref()));
        if let Err(mut error) = upload {
            if user_cancelled.load(Ordering::Acquire) {
                match recover_remote() {
                    Ok(mut recovery) => error
                        .recovery_paths
                        .extend(plan.cleanup_temps(recovery.as_mut())),
                    Err(recovery_error) => {
                        error.message = format!(
                            "{}；无法建立恢复 SSH 会话清理临时目标：{}",
                            error.message, recovery_error.message
                        );
                        error.recovery_paths.extend(plan.temp_paths());
                    }
                }
            } else {
                error
                    .recovery_paths
                    .extend(plan.cleanup_temps(control.as_mut()));
            }
            if error.cancelled && !user_cancelled.load(Ordering::Acquire) {
                error = DeployError::failed("并行上传因其他目标失败而停止");
            }
            error.recovery_paths.sort();
            error.recovery_paths.dedup();
            return Err(error);
        }
        if user_cancelled.load(Ordering::Acquire) {
            let mut recovery = match recover_remote() {
                Ok(remote) => remote,
                Err(recovery_error) => {
                    let mut error = DeployError::cancelled();
                    error.message = format!(
                        "{}；无法建立恢复 SSH 会话清理临时目标：{}",
                        error.message, recovery_error.message
                    );
                    error.recovery_paths.extend(plan.temp_paths());
                    error.recovery_paths.sort();
                    error.recovery_paths.dedup();
                    return Err(error);
                }
            };
            return plan
                .cancel_and_cleanup(recovery.as_mut())
                .map(|()| DeploymentSuccess::complete(recovery));
        }
        if stop_requested.load(Ordering::Acquire) {
            let mut error = DeployError::failed("并行上传因其他目标失败而停止");
            error
                .recovery_paths
                .extend(plan.cleanup_temps(control.as_mut()));
            error.recovery_paths.sort();
            error.recovery_paths.dedup();
            return Err(error);
        }
        let commit = plan.commit(control.as_mut(), user_cancelled.as_ref());
        return DeploymentSuccess::from_commit(control, commit);
    }

    let plan = Arc::new(plan);
    let mut workers = vec![control];
    workers.extend(remotes);
    let transport_interrupted = Arc::new(AtomicBool::new(false));
    let first_worker_error = Arc::new(Mutex::new(None));
    let mut handles = Vec::with_capacity(workers.len());
    for (index, mut remote) in workers.into_iter().enumerate() {
        let plan = Arc::clone(&plan);
        let stop_requested = Arc::clone(&stop_requested);
        let user_cancelled = Arc::clone(&user_cancelled);
        let progress = Arc::clone(&progress);
        let event = Arc::clone(&event);
        let interrupt_transport = Arc::clone(&interrupt_transport);
        let transport_interrupted = Arc::clone(&transport_interrupted);
        let first_worker_error = Arc::clone(&first_worker_error);
        handles.push(thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                plan.upload_target(
                    index,
                    remote.as_mut(),
                    stop_requested.as_ref(),
                    &mut |bytes, path| progress(bytes, path),
                    event.as_ref(),
                )
                .and_then(|()| plan.verify_target(index, remote.as_ref()))
            }))
            .unwrap_or_else(|_| Err(DeployError::failed("远端上传工作线程异常退出")));
            if result.is_err() && !user_cancelled.load(Ordering::Acquire) {
                if !stop_requested.swap(true, Ordering::AcqRel) {
                    if let Err(error) = &result {
                        *first_worker_error.lock().unwrap() = Some(error.clone());
                    }
                    transport_interrupted.store(true, Ordering::Release);
                    interrupt_transport();
                }
            }
            (remote, result)
        }));
    }

    let mut returned_remotes = Vec::with_capacity(handles.len());
    let mut primary_error = None;
    for handle in handles {
        match handle.join() {
            Ok((remote, result)) => {
                returned_remotes.push(remote);
                if let Err(error) = result {
                    if !error.cancelled && primary_error.is_none() {
                        primary_error = Some(error);
                    }
                }
            }
            Err(_) => {
                if primary_error.is_none() {
                    primary_error = Some(DeployError::failed("远端上传工作线程异常退出"));
                }
            }
        }
    }
    if let Some(error) = first_worker_error.lock().unwrap().take() {
        primary_error = Some(error);
    }

    let mut plan = match Arc::try_unwrap(plan) {
        Ok(plan) => plan,
        Err(plan) => {
            let mut error = DeployError::failed("远端上传计划仍被工作线程占用");
            if let Some(control) = returned_remotes.first_mut() {
                error
                    .recovery_paths
                    .extend(plan.cleanup_temps(control.as_mut()));
            } else {
                error.recovery_paths.extend(plan.temp_paths());
            }
            error.recovery_paths.sort();
            error.recovery_paths.dedup();
            return Err(error);
        }
    };
    let mut control = if transport_interrupted.load(Ordering::Acquire)
        || user_cancelled.load(Ordering::Acquire)
    {
        match recover_remote() {
            Ok(remote) => remote,
            Err(recovery_error) => {
                let mut error = primary_error
                    .unwrap_or_else(|| DeployError::failed("并行上传因其他目标失败而停止"));
                error.message = format!(
                    "{}；无法建立恢复 SSH 会话清理临时目标：{}",
                    error.message, recovery_error.message
                );
                error.recovery_paths.extend(plan.temp_paths());
                error.recovery_paths.sort();
                error.recovery_paths.dedup();
                return Err(error);
            }
        }
    } else if let Some(remote) = returned_remotes.into_iter().next() {
        remote
    } else {
        let mut error = primary_error
            .unwrap_or_else(|| DeployError::failed("没有可用于清理远端临时目标的 SSH 会话"));
        error.recovery_paths.extend(plan.temp_paths());
        error.recovery_paths.sort();
        error.recovery_paths.dedup();
        return Err(error);
    };
    if let Some(mut error) = primary_error {
        error
            .recovery_paths
            .extend(plan.cleanup_temps(control.as_mut()));
        error.recovery_paths.sort();
        error.recovery_paths.dedup();
        return Err(error);
    }
    if user_cancelled.load(Ordering::Acquire) {
        return plan
            .cancel_and_cleanup(control.as_mut())
            .map(|()| DeploymentSuccess::complete(control));
    }
    if stop_requested.load(Ordering::Acquire) {
        let mut error = DeployError::failed("并行上传因其他目标失败而停止");
        error
            .recovery_paths
            .extend(plan.cleanup_temps(control.as_mut()));
        error.recovery_paths.sort();
        error.recovery_paths.dedup();
        return Err(error);
    }
    if let Err(mut error) = plan.validate_formal_targets(control.as_ref()) {
        error
            .recovery_paths
            .extend(plan.cleanup_temps(control.as_mut()));
        error.recovery_paths.sort();
        error.recovery_paths.dedup();
        return Err(error);
    }
    let commit = plan.commit(control.as_mut(), user_cancelled.as_ref());
    DeploymentSuccess::from_commit(control, commit)
}
#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::ArtifactManifest;
    use crate::tools::release_package_model::ReleaseTarget;

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
    use std::io::{Cursor, Read};
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;

    use flate2::read::GzDecoder;
    use tar::Archive;

    use super::{
        deploy, ArtifactManifest, DeployError, DeploymentRequest, DeploymentTarget,
        RemoteCommandOutputDecoder, RemoteCommandResult, RemoteDirEntry, RemoteFs, RemoteKind,
        RemoteMetadata,
    };
    use crate::tools::release_package_model::ReleaseTarget;

    #[derive(Clone)]
    enum Node {
        Directory,
        File(Vec<u8>),
    }

    enum FakeCommandEvent {
        Output(&'static str, Vec<u8>),
        ReadError(String),
    }

    struct FakeRemoteFs {
        nodes: BTreeMap<String, Node>,
        metadata_calls: RefCell<Vec<String>>,
        read_dir_calls: RefCell<Vec<String>>,
        create_dir_calls: Vec<String>,
        fail_rename_targets: VecDeque<String>,
        fail_remove_targets: VecDeque<String>,
        cancel_after_create_dirs: Option<(usize, Arc<AtomicBool>)>,
        cancel_during_write: bool,
        command_exit_code: i32,
        command_events: VecDeque<FakeCommandEvent>,
        command_calls: Vec<String>,
        write_file_calls: Vec<String>,
        extract_calls: Vec<(String, String)>,
        fail_extract: bool,
        cancel_during_extract: bool,
        reported_archive_size_delta: i64,
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
                read_dir_calls: RefCell::new(Vec::new()),
                create_dir_calls: Vec::new(),
                fail_rename_targets: VecDeque::new(),
                fail_remove_targets: VecDeque::new(),
                cancel_after_create_dirs: None,
                cancel_during_write: false,
                command_exit_code: 0,
                command_events: VecDeque::new(),
                command_calls: Vec::new(),
                write_file_calls: Vec::new(),
                extract_calls: Vec::new(),
                fail_extract: false,
                cancel_during_extract: false,
                reported_archive_size_delta: 0,
            }
        }

        fn command_remote(exit_code: i32, events: Vec<(&'static str, Vec<u8>)>) -> Self {
            let mut remote = Self::base();
            remote.command_exit_code = exit_code;
            remote.command_events = events
                .into_iter()
                .map(|(stream, bytes)| FakeCommandEvent::Output(stream, bytes))
                .collect();
            remote
        }

        fn command_remote_events(exit_code: i32, events: Vec<FakeCommandEvent>) -> Self {
            let mut remote = Self::base();
            remote.command_exit_code = exit_code;
            remote.command_events = events.into();
            remote
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

        fn cancel_after_create_dirs(&mut self, count: usize, cancelled: Arc<AtomicBool>) {
            self.cancel_after_create_dirs = Some((count, cancelled));
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
                    size: if path.ends_with(".tar.gz") {
                        (value.len() as i64 + self.reported_archive_size_delta).max(0) as u64
                    } else {
                        value.len() as u64
                    },
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
            if let Some((count, cancelled)) = &self.cancel_after_create_dirs {
                if self.create_dir_calls.len() == *count {
                    cancelled.store(true, Ordering::Release);
                }
            }
            Ok(())
        }

        fn read_dir(&self, path: &str) -> Result<Vec<RemoteDirEntry>, DeployError> {
            self.read_dir_calls.borrow_mut().push(path.to_string());
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
            self.write_file_calls.push(remote_path.to_string());
            self.nodes.insert(remote_path.into(), Node::File(content));
            Ok(())
        }

        fn extract_tar_gz(
            &mut self,
            archive_path: &str,
            destination: &str,
            cancelled: &AtomicBool,
        ) -> Result<(), DeployError> {
            self.extract_calls
                .push((archive_path.to_string(), destination.to_string()));
            if self.cancel_during_extract || cancelled.load(Ordering::Acquire) {
                return Err(DeployError::cancelled_extraction());
            }
            if self.fail_extract {
                return Err(DeployError::failed("injected extraction failure"));
            }
            let content = match self.nodes.get(archive_path) {
                Some(Node::File(content)) => content.clone(),
                _ => return Err(DeployError::failed("archive missing")),
            };
            extract_archive_into_nodes(&mut self.nodes, destination, &content)
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

        fn execute_command(
            &mut self,
            command: &str,
            cancelled: &AtomicBool,
            output: &mut dyn FnMut(&str, String),
        ) -> Result<RemoteCommandResult, DeployError> {
            if cancelled.load(Ordering::Acquire) {
                return Err(DeployError::cancelled_command());
            }
            self.command_calls.push(command.to_string());
            let mut decoder = RemoteCommandOutputDecoder::default();
            while let Some(event) = self.command_events.pop_front() {
                if cancelled.load(Ordering::Acquire) {
                    return Err(DeployError::cancelled_command());
                }
                match event {
                    FakeCommandEvent::Output(stream, bytes) => {
                        decoder.push(stream, &bytes, output);
                    }
                    FakeCommandEvent::ReadError(message) => {
                        return Err(DeployError::failed(format!(
                            "读取上传后命令输出失败：{message}"
                        )));
                    }
                }
            }
            decoder.flush(output);
            Ok(RemoteCommandResult {
                exit_code: self.command_exit_code,
            })
        }
    }

    struct ParallelProbe {
        active: AtomicUsize,
        max_active: AtomicUsize,
        entered: Barrier,
        uploads_finished: AtomicUsize,
        commit_started: AtomicBool,
    }

    impl ParallelProbe {
        fn new(worker_count: usize) -> Self {
            Self {
                active: AtomicUsize::new(0),
                max_active: AtomicUsize::new(0),
                entered: Barrier::new(worker_count),
                uploads_finished: AtomicUsize::new(0),
                commit_started: AtomicBool::new(false),
            }
        }
    }

    #[derive(Default)]
    struct FailureProbe {
        sibling_started: AtomicBool,
        sibling_stopped: AtomicBool,
        sibling_blocked_in_write: AtomicBool,
        transport_interrupted: AtomicBool,
    }

    enum SharedWriteBehavior {
        Normal,
        FailAfterSiblingStarts(Arc<FailureProbe>),
        CancelAfterSiblingStarts(Arc<FailureProbe>, Arc<AtomicBool>),
        StopWhenRequested(Arc<FailureProbe>),
        FailWhenStopRequested(Arc<FailureProbe>),
        BlockUntilTransportInterrupted(Arc<FailureProbe>),
        Panic,
    }

    struct SharedFakeRemoteFs {
        nodes: Arc<Mutex<BTreeMap<String, Node>>>,
        parallel_probe: Option<Arc<ParallelProbe>>,
        behavior: SharedWriteBehavior,
        entered_first_write: bool,
        cancel_after_backup: Option<Arc<AtomicBool>>,
        cancel_after_temp_root: Option<Arc<AtomicBool>>,
        expected_thread: Option<(thread::ThreadId, Arc<AtomicBool>)>,
        command_supported: bool,
    }

    impl SharedFakeRemoteFs {
        fn new(nodes: Arc<Mutex<BTreeMap<String, Node>>>, behavior: SharedWriteBehavior) -> Self {
            Self {
                nodes,
                parallel_probe: None,
                behavior,
                entered_first_write: false,
                cancel_after_backup: None,
                cancel_after_temp_root: None,
                expected_thread: None,
                command_supported: false,
            }
        }

        fn with_parallel_probe(
            nodes: Arc<Mutex<BTreeMap<String, Node>>>,
            probe: Arc<ParallelProbe>,
        ) -> Self {
            let mut remote = Self::new(nodes, SharedWriteBehavior::Normal);
            remote.parallel_probe = Some(probe);
            remote
        }

        fn cancel_after_backup(mut self, cancelled: Arc<AtomicBool>) -> Self {
            self.cancel_after_backup = Some(cancelled);
            self
        }

        fn cancel_after_temp_root(mut self, cancelled: Arc<AtomicBool>) -> Self {
            self.cancel_after_temp_root = Some(cancelled);
            self
        }

        fn ensure_transport_available(&self) -> Result<(), DeployError> {
            if self
                .cancel_after_temp_root
                .as_ref()
                .is_some_and(|cancelled| cancelled.load(Ordering::Acquire))
            {
                return Err(DeployError::failed("transport interrupted"));
            }
            Ok(())
        }

        fn expect_thread(mut self, expected: thread::ThreadId, observed: Arc<AtomicBool>) -> Self {
            self.expected_thread = Some((expected, observed));
            self
        }

        fn allow_commands(mut self) -> Self {
            self.command_supported = true;
            self
        }
    }

    impl RemoteFs for SharedFakeRemoteFs {
        fn metadata(&self, path: &str) -> Result<Option<RemoteMetadata>, DeployError> {
            self.ensure_transport_available()?;
            Ok(self.nodes.lock().unwrap().get(path).map(|node| match node {
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
            self.ensure_transport_available()?;
            let mut nodes = self.nodes.lock().unwrap();
            if let Some(node) = nodes.get(path) {
                return match node {
                    Node::Directory => Ok(()),
                    Node::File(_) => Err(DeployError::failed("远端目录路径已被文件占用")),
                };
            }
            nodes.insert(path.to_string(), Node::Directory);
            drop(nodes);
            if path.contains(".__lazycat_tmp_") {
                if let Some(cancelled) = &self.cancel_after_temp_root {
                    cancelled.store(true, Ordering::Release);
                }
            }
            Ok(())
        }

        fn read_dir(&self, path: &str) -> Result<Vec<RemoteDirEntry>, DeployError> {
            self.ensure_transport_available()?;
            let prefix = format!("{}/", path.trim_end_matches('/'));
            let nodes = self.nodes.lock().unwrap();
            let mut entries = Vec::new();
            for (entry_path, node) in nodes.iter() {
                let Some(relative) = entry_path.strip_prefix(&prefix) else {
                    continue;
                };
                if relative.is_empty() || relative.contains('/') {
                    continue;
                }
                entries.push(RemoteDirEntry {
                    path: entry_path.clone(),
                    metadata: match node {
                        Node::Directory => RemoteMetadata {
                            kind: RemoteKind::Directory,
                            size: 0,
                        },
                        Node::File(value) => RemoteMetadata {
                            kind: RemoteKind::File,
                            size: value.len() as u64,
                        },
                    },
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
            self.ensure_transport_available()?;
            if cancelled.load(Ordering::Acquire) {
                return Err(DeployError::cancelled());
            }
            if let Some((expected, observed)) = &self.expected_thread {
                observed.store(thread::current().id() == *expected, Ordering::Release);
            }
            match &self.behavior {
                SharedWriteBehavior::FailAfterSiblingStarts(probe) => {
                    while !probe.sibling_started.load(Ordering::Acquire) {
                        thread::yield_now();
                    }
                    return Err(DeployError::failed("injected upload failure"));
                }
                SharedWriteBehavior::CancelAfterSiblingStarts(probe, user_cancelled) => {
                    while !probe.sibling_started.load(Ordering::Acquire) {
                        thread::yield_now();
                    }
                    user_cancelled.store(true, Ordering::Release);
                    return Err(DeployError::cancelled());
                }
                SharedWriteBehavior::StopWhenRequested(probe) => {
                    probe.sibling_started.store(true, Ordering::Release);
                    while !cancelled.load(Ordering::Acquire) {
                        thread::yield_now();
                    }
                    probe.sibling_stopped.store(true, Ordering::Release);
                    return Err(DeployError::cancelled());
                }
                SharedWriteBehavior::FailWhenStopRequested(probe) => {
                    probe.sibling_started.store(true, Ordering::Release);
                    while !cancelled.load(Ordering::Acquire) {
                        thread::yield_now();
                    }
                    probe.sibling_stopped.store(true, Ordering::Release);
                    return Err(DeployError::failed("sibling transport interrupted"));
                }
                SharedWriteBehavior::BlockUntilTransportInterrupted(probe) => {
                    probe.sibling_started.store(true, Ordering::Release);
                    probe
                        .sibling_blocked_in_write
                        .store(true, Ordering::Release);
                    while !probe.transport_interrupted.load(Ordering::Acquire) {
                        thread::yield_now();
                    }
                    probe.sibling_stopped.store(true, Ordering::Release);
                    return Err(DeployError::failed("transport interrupted"));
                }
                SharedWriteBehavior::Panic => panic!("injected worker panic"),
                SharedWriteBehavior::Normal => {}
            }
            if !self.entered_first_write {
                if let Some(probe) = &self.parallel_probe {
                    self.entered_first_write = true;
                    let active = probe.active.fetch_add(1, Ordering::AcqRel) + 1;
                    probe.max_active.fetch_max(active, Ordering::AcqRel);
                    probe.entered.wait();
                    probe.active.fetch_sub(1, Ordering::AcqRel);
                    probe.uploads_finished.fetch_add(1, Ordering::AcqRel);
                }
            }
            let content = fs::read(local_path).map_err(DeployError::local_io)?;
            progress(content.len() as u64);
            self.nodes
                .lock()
                .unwrap()
                .insert(remote_path.to_string(), Node::File(content));
            Ok(())
        }

        fn extract_tar_gz(
            &mut self,
            archive_path: &str,
            destination: &str,
            cancelled: &AtomicBool,
        ) -> Result<(), DeployError> {
            self.ensure_transport_available()?;
            if cancelled.load(Ordering::Acquire) {
                return Err(DeployError::cancelled_extraction());
            }
            let mut nodes = self.nodes.lock().unwrap();
            let content = match nodes.get(archive_path) {
                Some(Node::File(content)) => content.clone(),
                _ => return Err(DeployError::failed("archive missing")),
            };
            extract_archive_into_nodes(&mut nodes, destination, &content)
        }

        fn rename(&mut self, source: &str, target: &str) -> Result<(), DeployError> {
            self.ensure_transport_available()?;
            if target == "/srv/app/web" || target == "/srv/app/app.jar" {
                if let Some(probe) = &self.parallel_probe {
                    assert_eq!(probe.uploads_finished.load(Ordering::Acquire), 2);
                    probe.commit_started.store(true, Ordering::Release);
                }
            }
            let mut nodes = self.nodes.lock().unwrap();
            let affected = nodes
                .keys()
                .filter(|path| *path == source || path.starts_with(&format!("{source}/")))
                .cloned()
                .collect::<Vec<_>>();
            if affected.is_empty() {
                return Err(DeployError::failed("rename source missing"));
            }
            for old_path in affected {
                let node = nodes.remove(&old_path).unwrap();
                let suffix = &old_path[source.len()..];
                nodes.insert(format!("{target}{suffix}"), node);
            }
            drop(nodes);
            if target.contains(".__lazycat_backup_") {
                if let Some(cancelled) = &self.cancel_after_backup {
                    cancelled.store(true, Ordering::Release);
                }
            }
            Ok(())
        }

        fn remove_tree(&mut self, path: &str) -> Result<(), DeployError> {
            self.ensure_transport_available()?;
            let mut nodes = self.nodes.lock().unwrap();
            let affected = nodes
                .keys()
                .filter(|candidate| {
                    *candidate == path || candidate.starts_with(&format!("{path}/"))
                })
                .cloned()
                .collect::<Vec<_>>();
            for candidate in affected {
                nodes.remove(&candidate);
            }
            Ok(())
        }

        fn execute_command(
            &mut self,
            _command: &str,
            cancelled: &AtomicBool,
            _output: &mut dyn FnMut(&str, String),
        ) -> Result<RemoteCommandResult, DeployError> {
            if cancelled.load(Ordering::Acquire) {
                return Err(DeployError::cancelled_command());
            }
            if !self.command_supported {
                return Err(DeployError::failed("该测试连接不支持远程命令"));
            }
            Ok(RemoteCommandResult { exit_code: 0 })
        }
    }

    fn shared_existing_release() -> Arc<Mutex<BTreeMap<String, Node>>> {
        let mut nodes = BTreeMap::new();
        for path in ["/", "/srv", "/srv/app", "/srv/app/web"] {
            nodes.insert(path.to_string(), Node::Directory);
        }
        nodes.insert(
            "/srv/app/web/old.js".to_string(),
            Node::File(b"old".to_vec()),
        );
        nodes.insert(
            "/srv/app/app.jar".to_string(),
            Node::File(b"old-jar".to_vec()),
        );
        Arc::new(Mutex::new(nodes))
    }

    fn shared_read(nodes: &Arc<Mutex<BTreeMap<String, Node>>>, path: &str) -> Vec<u8> {
        match nodes.lock().unwrap().get(path).unwrap() {
            Node::File(value) => value.clone(),
            Node::Directory => panic!("not a file: {path}"),
        }
    }

    fn assert_temps_cleaned_or_reported(
        nodes: &Arc<Mutex<BTreeMap<String, Node>>>,
        error: &DeployError,
    ) {
        for path in [
            "/srv/app/web.__lazycat_tmp_run-1",
            "/srv/app/app.jar.__lazycat_tmp_run-1",
        ] {
            assert!(
                !nodes.lock().unwrap().contains_key(path)
                    || error.recovery_paths.iter().any(|recovery| recovery == path),
                "temporary path was neither removed nor reported: {path}",
            );
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

    fn extract_archive_into_nodes(
        nodes: &mut BTreeMap<String, Node>,
        destination: &str,
        content: &[u8],
    ) -> Result<(), DeployError> {
        let decoder = GzDecoder::new(Cursor::new(content));
        let mut archive = Archive::new(decoder);
        let entries = archive
            .entries()
            .map_err(|error| DeployError::failed(format!("decode archive: {error}")))?;
        for entry in entries {
            let mut entry = entry
                .map_err(|error| DeployError::failed(format!("decode archive entry: {error}")))?;
            let relative = entry
                .path()
                .map_err(|error| DeployError::failed(format!("decode archive path: {error}")))?
                .to_string_lossy()
                .replace('\\', "/");
            let remote_path = format!("{destination}/{relative}");
            let mut current = destination.to_string();
            let segments = relative.split('/').collect::<Vec<_>>();
            for segment in segments.iter().take(segments.len().saturating_sub(1)) {
                current.push('/');
                current.push_str(segment);
                nodes.insert(current.clone(), Node::Directory);
            }
            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .map_err(|error| DeployError::failed(format!("decode archive content: {error}")))?;
            nodes.insert(remote_path, Node::File(bytes));
        }
        Ok(())
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
                    frontend_archive: None,
                },
                DeploymentTarget {
                    manifest: backend,
                    remote_path: "/srv/app/app.jar".into(),
                    expected_exists: true,
                    frontend_archive: None,
                },
            ],
        };
        (root, request)
    }

    #[test]
    fn remote_command_reports_both_streams_and_exit_code() {
        let mut remote = FakeRemoteFs::command_remote(
            7,
            vec![
                ("stdout", b"ready\n".to_vec()),
                ("stderr", b"warning\n".to_vec()),
            ],
        );
        let mut lines = Vec::new();

        let result = remote
            .execute_command(
                "./deploy.sh",
                &AtomicBool::new(false),
                &mut |stream, line| {
                    lines.push((stream.to_string(), line));
                },
            )
            .unwrap();

        assert_eq!(result.exit_code, 7);
        assert_eq!(remote.command_calls, vec!["./deploy.sh"]);
        assert_eq!(
            lines,
            vec![
                ("stdout".to_string(), "ready".to_string()),
                ("stderr".to_string(), "warning".to_string()),
            ]
        );
    }
    #[test]
    fn remote_command_decodes_lossy_utf8_and_tail_lines() {
        let mut remote = FakeRemoteFs::command_remote(
            0,
            vec![
                ("stdout", vec![0xff, b'o']),
                ("stdout", b"k".to_vec()),
                ("stderr", b"tail".to_vec()),
            ],
        );
        let mut lines = Vec::new();

        remote
            .execute_command(
                "deploy",
                &AtomicBool::new(false),
                &mut |stream: &str, line| {
                    lines.push((stream.to_string(), line));
                },
            )
            .unwrap();

        assert_eq!(
            lines,
            vec![
                ("stdout".to_string(), "�ok".to_string()),
                ("stderr".to_string(), "tail".to_string()),
            ]
        );
    }

    #[test]
    fn remote_command_read_error_is_not_treated_as_eof() {
        let mut remote = FakeRemoteFs::command_remote_events(
            0,
            vec![FakeCommandEvent::ReadError("boom".into())],
        );

        let error = remote
            .execute_command("deploy", &AtomicBool::new(false), &mut |_, _| {})
            .unwrap_err();

        assert!(error.message.contains("读取上传后命令输出失败：boom"));
        assert!(!error.cancelled);
    }

    #[test]
    fn remote_command_cancelled_before_start_is_cancelled_error() {
        let mut remote = FakeRemoteFs::command_remote(0, vec![("stdout", b"ready\n".to_vec())]);
        let cancelled = AtomicBool::new(true);

        let error = remote
            .execute_command("deploy", &cancelled, &mut |_, _| {})
            .unwrap_err();

        assert!(error.cancelled);
        assert!(remote.command_calls.is_empty());
    }

    #[test]
    fn deployment_plan_creates_only_the_frontend_transaction_root() {
        let (root, request) = two_target_request();
        let plan = super::DeploymentPlan::new(request).unwrap();
        drop(root);

        assert_eq!(
            plan.frontend_directories(),
            &["/srv/app/web.__lazycat_tmp_run-1".to_string()]
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
    fn deployment_prepares_the_frontend_transaction_root_once() {
        let (root, request) = two_target_request();
        let plan = super::DeploymentPlan::new(request).unwrap();
        let mut remote = FakeRemoteFs::with_existing_release();

        plan.prepare_remote(&mut remote, &AtomicBool::new(false))
            .unwrap();
        drop(root);

        assert_eq!(
            remote
                .create_dir_calls
                .iter()
                .filter(|path| path.contains("__lazycat_tmp_run-1"))
                .count(),
            1
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
                frontend_archive: None,
            }],
        };
        let plan = super::DeploymentPlan::new(request).unwrap();
        let mut remote = FakeRemoteFs::base();

        plan.prepare_remote(&mut remote, &AtomicBool::new(false))
            .unwrap();

        assert!(remote.exists("/srv/app/empty-web.__lazycat_tmp_run-empty"));
    }

    #[test]
    fn deployment_keeps_preexisting_temp_path_after_prepare_conflict() {
        let (root, request) = two_target_request();
        let temp_path = "/srv/app/web.__lazycat_tmp_run-1";
        let mut remote = FakeRemoteFs::with_existing_release();
        remote
            .nodes
            .insert(temp_path.into(), Node::File(b"other-run".to_vec()));

        let error = deploy(&mut remote, &request, &AtomicBool::new(false), |_, _| {}).unwrap_err();
        drop(root);

        assert!(error.message.contains("临时或备份路径已存在"));
        assert_eq!(remote.read(temp_path), b"other-run");
    }

    #[test]
    fn cancellation_before_prepare_does_not_create_remote_directories() {
        let (root, request) = two_target_request();
        let mut remote = FakeRemoteFs::with_existing_release();

        let error = deploy(&mut remote, &request, &AtomicBool::new(true), |_, _| {}).unwrap_err();
        drop(root);

        assert!(error.cancelled);
        assert!(remote.create_dir_calls.is_empty());
        assert!(!remote.any_path_contains("__lazycat_tmp_"));
    }

    #[test]
    fn cancellation_during_prepare_cleans_only_the_created_temp_root() {
        let (root, request) = two_target_request();
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut remote = FakeRemoteFs::with_existing_release();
        remote.cancel_after_create_dirs(1, Arc::clone(&cancelled));

        let error = deploy(&mut remote, &request, cancelled.as_ref(), |_, _| {}).unwrap_err();
        drop(root);

        assert!(error.cancelled);
        assert_eq!(remote.create_dir_calls.len(), 1);
        assert_eq!(remote.read("/srv/app/web/old.js"), b"old");
        assert_eq!(remote.read("/srv/app/app.jar"), b"old-jar");
        assert!(!remote.any_path_contains("__lazycat_tmp_"));
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
        assert!(!remote.any_path_contains(".lazycat-upload-"));
        assert!(remote.read_dir_calls.borrow().is_empty());
    }

    #[test]
    fn frontend_archive_size_mismatch_never_commits() {
        let (root, request) = two_target_request();
        let mut remote = FakeRemoteFs::with_existing_release();
        remote.reported_archive_size_delta = 1;

        let error = deploy(&mut remote, &request, &AtomicBool::new(false), |_, _| {}).unwrap_err();
        drop(root);

        assert!(error.message.contains("传输包大小校验失败"));
        assert_eq!(remote.read("/srv/app/web/old.js"), b"old");
        assert_eq!(remote.read("/srv/app/app.jar"), b"old-jar");
        assert!(!remote.any_path_contains("__lazycat_backup_"));
        assert!(!remote.any_path_contains("__lazycat_tmp_"));
        assert!(!remote.any_path_contains(".lazycat-upload-"));
    }

    #[test]
    fn frontend_extraction_failure_never_commits_and_cleans_transport_paths() {
        let (root, request) = two_target_request();
        let mut remote = FakeRemoteFs::with_existing_release();
        remote.fail_extract = true;

        let error = deploy(&mut remote, &request, &AtomicBool::new(false), |_, _| {}).unwrap_err();
        drop(root);

        assert!(error.message.contains("injected extraction failure"));
        assert_eq!(remote.read("/srv/app/web/old.js"), b"old");
        assert_eq!(remote.read("/srv/app/app.jar"), b"old-jar");
        assert!(!remote.any_path_contains("__lazycat_backup_"));
        assert!(!remote.any_path_contains("__lazycat_tmp_"));
        assert!(!remote.any_path_contains(".lazycat-upload-"));
    }

    #[test]
    fn frontend_extraction_cancellation_has_its_own_error_and_never_commits() {
        let (root, request) = two_target_request();
        let mut remote = FakeRemoteFs::with_existing_release();
        remote.cancel_during_extract = true;

        let error = deploy(&mut remote, &request, &AtomicBool::new(false), |_, _| {}).unwrap_err();
        drop(root);

        assert!(error.cancelled);
        assert_eq!(error.message, "远端前端传输包解压已取消");
        assert_eq!(remote.read("/srv/app/web/old.js"), b"old");
        assert_eq!(remote.read("/srv/app/app.jar"), b"old-jar");
        assert!(!remote.any_path_contains("__lazycat_tmp_"));
        assert!(!remote.any_path_contains(".lazycat-upload-"));
    }

    #[test]
    fn frontend_archive_cleanup_failure_never_commits_and_reports_the_archive() {
        let (root, request) = two_target_request();
        let archive_path = "/srv/app/.lazycat-upload-run-1-frontend.tar.gz";
        let mut remote = FakeRemoteFs::with_existing_release();
        remote.fail_remove_tree(archive_path);
        remote.fail_remove_tree(archive_path);

        let error = deploy(&mut remote, &request, &AtomicBool::new(false), |_, _| {}).unwrap_err();
        drop(root);

        assert!(error.message.contains("清理远端前端传输包失败"));
        assert!(!error.committed);
        assert!(error.recovery_paths.contains(&archive_path.to_string()));
        assert_eq!(remote.read("/srv/app/web/old.js"), b"old");
        assert_eq!(remote.read("/srv/app/app.jar"), b"old-jar");
        assert!(!remote.any_path_contains("__lazycat_backup_"));
        assert!(!remote.any_path_contains("__lazycat_tmp_"));
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
    fn deployment_progress_uses_the_frontend_archive_name_and_backend_file() {
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

        assert!(progress
            .iter()
            .any(|(_, path)| path == ".lazycat-upload-run-1-frontend.tar.gz"));
        assert!(progress.iter().any(|(_, path)| path == "app.jar"));
        assert_eq!(
            remote
                .write_file_calls
                .iter()
                .filter(|path| path.ends_with("frontend.tar.gz"))
                .count(),
            1
        );
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

    #[test]
    fn parallel_deploy_overlaps_two_target_uploads_and_commits_serially() {
        let (root, request) = two_target_request();
        let nodes = shared_existing_release();
        let probe = Arc::new(ParallelProbe::new(2));
        let remotes = vec![
            Box::new(SharedFakeRemoteFs::with_parallel_probe(
                Arc::clone(&nodes),
                Arc::clone(&probe),
            )) as Box<dyn RemoteFs>,
            Box::new(SharedFakeRemoteFs::with_parallel_probe(
                Arc::clone(&nodes),
                Arc::clone(&probe),
            )) as Box<dyn RemoteFs>,
        ];

        super::deploy_parallel(
            remotes,
            super::DeploymentPlan::new(request).unwrap(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(|_, _| {}),
            Arc::new(|_| {}),
            Arc::new(|| {}),
            Arc::new(|| Err(DeployError::failed("unexpected recovery"))),
        )
        .unwrap();
        drop(root);

        assert_eq!(probe.max_active.load(Ordering::Acquire), 2);
        assert!(probe.commit_started.load(Ordering::Acquire));
        assert_eq!(shared_read(&nodes, "/srv/app/web/index.html"), b"new-web");
        assert_eq!(shared_read(&nodes, "/srv/app/app.jar"), b"new-jar");
    }

    #[test]
    fn committed_connection_returns_control_remote_for_commands() {
        let (root, request) = two_target_request();
        let nodes = shared_existing_release();
        let remotes = vec![
            Box::new(
                SharedFakeRemoteFs::new(Arc::clone(&nodes), SharedWriteBehavior::Normal)
                    .allow_commands(),
            ) as Box<dyn RemoteFs>,
            Box::new(
                SharedFakeRemoteFs::new(Arc::clone(&nodes), SharedWriteBehavior::Normal)
                    .allow_commands(),
            ) as Box<dyn RemoteFs>,
        ];

        let mut success = super::deploy_parallel(
            remotes,
            super::DeploymentPlan::new(request).unwrap(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(|_, _| {}),
            Arc::new(|_| {}),
            Arc::new(|| {}),
            Arc::new(|| Err(DeployError::failed("unexpected recovery"))),
        )
        .expect("deployment succeeds");
        drop(root);

        assert_eq!(shared_read(&nodes, "/srv/app/web/index.html"), b"new-web");
        assert_eq!(shared_read(&nodes, "/srv/app/app.jar"), b"new-jar");
        let result = success
            .control
            .execute_command("true", &AtomicBool::new(false), &mut |_, _| {})
            .unwrap();
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn committed_cleanup_warning_returns_control_remote_for_commands() {
        let (root, mut request) = two_target_request();
        request.targets.remove(0);
        let backup_path = "/srv/app/app.jar.__lazycat_backup_run-1";
        let mut remote = FakeRemoteFs::with_existing_release();
        remote.fail_remove_tree(backup_path);

        let mut success = super::deploy_parallel(
            vec![Box::new(remote)],
            super::DeploymentPlan::new(request).unwrap(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(|_, _| {}),
            Arc::new(|_| {}),
            Arc::new(|| {}),
            Arc::new(|| Err(DeployError::failed("unexpected recovery"))),
        )
        .expect("committed deployment keeps its control connection");
        drop(root);

        let warning = success.warning.expect("cleanup warning is preserved");
        assert!(warning.committed);
        assert_eq!(warning.recovery_paths, vec![backup_path.to_string()]);
        let result = success
            .control
            .execute_command("true", &AtomicBool::new(false), &mut |_, _| {})
            .unwrap();
        assert_eq!(result.exit_code, 0);
    }
    #[test]
    fn parallel_deploy_stops_sibling_after_upload_failure() {
        let (root, request) = two_target_request();
        let nodes = shared_existing_release();
        let probe = Arc::new(FailureProbe::default());
        let stop_requested = Arc::new(AtomicBool::new(false));
        let recovery_nodes = Arc::clone(&nodes);
        let remotes = vec![
            Box::new(SharedFakeRemoteFs::new(
                Arc::clone(&nodes),
                SharedWriteBehavior::FailAfterSiblingStarts(Arc::clone(&probe)),
            )) as Box<dyn RemoteFs>,
            Box::new(SharedFakeRemoteFs::new(
                Arc::clone(&nodes),
                SharedWriteBehavior::StopWhenRequested(Arc::clone(&probe)),
            )) as Box<dyn RemoteFs>,
        ];

        let error = super::deploy_parallel(
            remotes,
            super::DeploymentPlan::new(request).unwrap(),
            Arc::new(AtomicBool::new(false)),
            Arc::clone(&stop_requested),
            Arc::new(|_, _| {}),
            Arc::new(|_| {}),
            Arc::new(|| {}),
            Arc::new(move || {
                Ok(Box::new(SharedFakeRemoteFs::new(
                    Arc::clone(&recovery_nodes),
                    SharedWriteBehavior::Normal,
                )) as Box<dyn RemoteFs>)
            }),
        )
        .unwrap_err();
        drop(root);

        assert!(error.message.contains("injected upload failure"));
        assert!(stop_requested.load(Ordering::Acquire));
        assert!(probe.sibling_stopped.load(Ordering::Acquire));
        assert_eq!(shared_read(&nodes, "/srv/app/web/old.js"), b"old");
        assert_eq!(shared_read(&nodes, "/srv/app/app.jar"), b"old-jar");
        assert_temps_cleaned_or_reported(&nodes, &error);
    }

    #[test]
    fn parallel_deploy_preserves_the_first_worker_failure_over_a_lower_index_sibling_error() {
        let (root, request) = two_target_request();
        let nodes = shared_existing_release();
        let probe = Arc::new(FailureProbe::default());
        let recovery_nodes = Arc::clone(&nodes);
        let remotes = vec![
            Box::new(SharedFakeRemoteFs::new(
                Arc::clone(&nodes),
                SharedWriteBehavior::FailWhenStopRequested(Arc::clone(&probe)),
            )) as Box<dyn RemoteFs>,
            Box::new(SharedFakeRemoteFs::new(
                Arc::clone(&nodes),
                SharedWriteBehavior::FailAfterSiblingStarts(Arc::clone(&probe)),
            )) as Box<dyn RemoteFs>,
        ];

        let error = super::deploy_parallel(
            remotes,
            super::DeploymentPlan::new(request).unwrap(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(|_, _| {}),
            Arc::new(|_| {}),
            Arc::new(|| {}),
            Arc::new(move || {
                Ok(Box::new(SharedFakeRemoteFs::new(
                    Arc::clone(&recovery_nodes),
                    SharedWriteBehavior::Normal,
                )) as Box<dyn RemoteFs>)
            }),
        )
        .unwrap_err();
        drop(root);

        assert!(probe.sibling_stopped.load(Ordering::Acquire));
        assert!(error.message.contains("injected upload failure"));
        assert!(!error.message.contains("sibling transport interrupted"));
        assert_temps_cleaned_or_reported(&nodes, &error);
    }

    #[test]
    fn parallel_deploy_interrupts_blocked_sibling_and_uses_recovery_session_for_cleanup() {
        let (root, request) = two_target_request();
        let nodes = shared_existing_release();
        let probe = Arc::new(FailureProbe::default());
        let interrupt_probe = Arc::clone(&probe);
        let recovery_nodes = Arc::clone(&nodes);
        let remotes = vec![
            Box::new(SharedFakeRemoteFs::new(
                Arc::clone(&nodes),
                SharedWriteBehavior::FailAfterSiblingStarts(Arc::clone(&probe)),
            )) as Box<dyn RemoteFs>,
            Box::new(SharedFakeRemoteFs::new(
                Arc::clone(&nodes),
                SharedWriteBehavior::BlockUntilTransportInterrupted(Arc::clone(&probe)),
            )) as Box<dyn RemoteFs>,
        ];

        let error = super::deploy_parallel(
            remotes,
            super::DeploymentPlan::new(request).unwrap(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(|_, _| {}),
            Arc::new(|_| {}),
            Arc::new(move || {
                interrupt_probe
                    .transport_interrupted
                    .store(true, Ordering::Release);
            }),
            Arc::new(move || {
                Ok(Box::new(SharedFakeRemoteFs::new(
                    Arc::clone(&recovery_nodes),
                    SharedWriteBehavior::Normal,
                )) as Box<dyn RemoteFs>)
            }),
        )
        .unwrap_err();
        drop(root);

        assert!(error.message.contains("injected upload failure"));
        assert!(!error.cancelled);
        assert!(probe.sibling_blocked_in_write.load(Ordering::Acquire));
        assert!(probe.transport_interrupted.load(Ordering::Acquire));
        assert!(probe.sibling_stopped.load(Ordering::Acquire));
        assert_eq!(shared_read(&nodes, "/srv/app/web/old.js"), b"old");
        assert_eq!(shared_read(&nodes, "/srv/app/app.jar"), b"old-jar");
        assert_temps_cleaned_or_reported(&nodes, &error);
    }

    #[test]
    fn parallel_deploy_uses_recovery_session_to_clean_up_after_user_cancellation() {
        let (root, request) = two_target_request();
        let nodes = shared_existing_release();
        let probe = Arc::new(FailureProbe::default());
        let user_cancelled = Arc::new(AtomicBool::new(false));
        let recovery_nodes = Arc::clone(&nodes);
        let recovery_used = Arc::new(AtomicBool::new(false));
        let recovery_used_for_closure = Arc::clone(&recovery_used);
        let remotes = vec![
            Box::new(SharedFakeRemoteFs::new(
                Arc::clone(&nodes),
                SharedWriteBehavior::CancelAfterSiblingStarts(
                    Arc::clone(&probe),
                    Arc::clone(&user_cancelled),
                ),
            )) as Box<dyn RemoteFs>,
            Box::new(SharedFakeRemoteFs::new(
                Arc::clone(&nodes),
                SharedWriteBehavior::StopWhenRequested(Arc::clone(&probe)),
            )) as Box<dyn RemoteFs>,
        ];

        let error = super::deploy_parallel(
            remotes,
            super::DeploymentPlan::new(request).unwrap(),
            Arc::clone(&user_cancelled),
            user_cancelled,
            Arc::new(|_, _| {}),
            Arc::new(|_| {}),
            Arc::new(|| {}),
            Arc::new(move || {
                recovery_used_for_closure.store(true, Ordering::Release);
                Ok(Box::new(SharedFakeRemoteFs::new(
                    Arc::clone(&recovery_nodes),
                    SharedWriteBehavior::Normal,
                )) as Box<dyn RemoteFs>)
            }),
        )
        .unwrap_err();
        drop(root);

        assert!(error.cancelled);
        assert!(recovery_used.load(Ordering::Acquire));
        assert!(probe.sibling_stopped.load(Ordering::Acquire));
        assert_eq!(shared_read(&nodes, "/srv/app/web/old.js"), b"old");
        assert_eq!(shared_read(&nodes, "/srv/app/app.jar"), b"old-jar");
        assert_temps_cleaned_or_reported(&nodes, &error);
    }

    #[test]
    fn parallel_prepare_cancellation_uses_recovery_session_when_control_transport_is_closed() {
        let (root, request) = two_target_request();
        let nodes = shared_existing_release();
        let user_cancelled = Arc::new(AtomicBool::new(false));
        let recovery_nodes = Arc::clone(&nodes);
        let recovery_used = Arc::new(AtomicBool::new(false));
        let recovery_used_for_closure = Arc::clone(&recovery_used);
        let control = SharedFakeRemoteFs::new(Arc::clone(&nodes), SharedWriteBehavior::Normal)
            .cancel_after_temp_root(Arc::clone(&user_cancelled));

        let error = super::deploy_parallel(
            vec![
                Box::new(control) as Box<dyn RemoteFs>,
                Box::new(SharedFakeRemoteFs::new(
                    Arc::clone(&nodes),
                    SharedWriteBehavior::Normal,
                )) as Box<dyn RemoteFs>,
            ],
            super::DeploymentPlan::new(request).unwrap(),
            user_cancelled,
            Arc::new(AtomicBool::new(false)),
            Arc::new(|_, _| {}),
            Arc::new(|_| {}),
            Arc::new(|| {}),
            Arc::new(move || {
                recovery_used_for_closure.store(true, Ordering::Release);
                Ok(Box::new(SharedFakeRemoteFs::new(
                    Arc::clone(&recovery_nodes),
                    SharedWriteBehavior::Normal,
                )) as Box<dyn RemoteFs>)
            }),
        )
        .unwrap_err();
        drop(root);

        assert!(error.cancelled);
        assert!(recovery_used.load(Ordering::Acquire));
        assert_eq!(shared_read(&nodes, "/srv/app/web/old.js"), b"old");
        assert_eq!(shared_read(&nodes, "/srv/app/app.jar"), b"old-jar");
        assert!(!nodes
            .lock()
            .unwrap()
            .keys()
            .any(|path| path.contains("__lazycat_tmp_")));
    }

    #[test]
    fn parallel_worker_panic_returns_remote_and_cleans_temps() {
        let (root, request) = two_target_request();
        let nodes = shared_existing_release();
        let recovery_nodes = Arc::clone(&nodes);
        let remotes = vec![
            Box::new(SharedFakeRemoteFs::new(
                Arc::clone(&nodes),
                SharedWriteBehavior::Panic,
            )) as Box<dyn RemoteFs>,
            Box::new(SharedFakeRemoteFs::new(
                Arc::clone(&nodes),
                SharedWriteBehavior::Normal,
            )) as Box<dyn RemoteFs>,
        ];

        let error = super::deploy_parallel(
            remotes,
            super::DeploymentPlan::new(request).unwrap(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(|_, _| {}),
            Arc::new(|_| {}),
            Arc::new(|| {}),
            Arc::new(move || {
                Ok(Box::new(SharedFakeRemoteFs::new(
                    Arc::clone(&recovery_nodes),
                    SharedWriteBehavior::Normal,
                )) as Box<dyn RemoteFs>)
            }),
        )
        .unwrap_err();
        drop(root);

        assert!(error.message.contains("工作线程异常退出"));
        assert_eq!(shared_read(&nodes, "/srv/app/web/old.js"), b"old");
        assert_eq!(shared_read(&nodes, "/srv/app/app.jar"), b"old-jar");
        assert_temps_cleaned_or_reported(&nodes, &error);
    }

    #[test]
    fn single_target_parallel_deploy_uploads_on_the_calling_thread() {
        let (root, mut request) = two_target_request();
        request.targets.remove(0);
        let nodes = shared_existing_release();
        let observed = Arc::new(AtomicBool::new(false));
        let remote = SharedFakeRemoteFs::new(Arc::clone(&nodes), SharedWriteBehavior::Normal)
            .expect_thread(thread::current().id(), Arc::clone(&observed));

        super::deploy_parallel(
            vec![Box::new(remote)],
            super::DeploymentPlan::new(request).unwrap(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(|_, _| {}),
            Arc::new(|_| {}),
            Arc::new(|| {}),
            Arc::new(|| Err(DeployError::failed("unexpected recovery"))),
        )
        .unwrap();
        drop(root);

        assert!(observed.load(Ordering::Acquire));
        assert_eq!(shared_read(&nodes, "/srv/app/app.jar"), b"new-jar");
    }

    #[test]
    fn cancellation_between_commit_renames_rolls_back_the_formal_target() {
        let (root, mut request) = two_target_request();
        request.targets.remove(0);
        let nodes = shared_existing_release();
        let user_cancelled = Arc::new(AtomicBool::new(false));
        let remote = SharedFakeRemoteFs::new(Arc::clone(&nodes), SharedWriteBehavior::Normal)
            .cancel_after_backup(Arc::clone(&user_cancelled));

        let error = super::deploy_parallel(
            vec![Box::new(remote)],
            super::DeploymentPlan::new(request).unwrap(),
            user_cancelled,
            Arc::new(AtomicBool::new(false)),
            Arc::new(|_, _| {}),
            Arc::new(|_| {}),
            Arc::new(|| {}),
            Arc::new(|| Err(DeployError::failed("unexpected recovery"))),
        )
        .unwrap_err();
        drop(root);

        assert!(error.cancelled);
        assert_eq!(shared_read(&nodes, "/srv/app/app.jar"), b"old-jar");
        assert_temps_cleaned_or_reported(&nodes, &error);
    }
}

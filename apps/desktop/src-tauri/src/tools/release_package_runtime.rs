use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::net::{Shutdown, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use tauri::Emitter;

use super::release_package::{ReleasePackageProjectConfig, ReleasePackageType, ReleaseTarget};
use super::release_package_archive::{
    archive_backend_artifact, archive_frontend_artifact, resolve_artifact_path,
    validate_artifact_target_collision, ArchiveError, ArchiveSession,
};
use super::release_package_deploy::{
    deploy, ArchivedTarget, ArtifactManifest, DeployError, DeploymentRequest, DeploymentTarget,
};
use super::release_package_remote::{
    consume_preflight, ConsumedPreflight, PreflightBinding, RemoteTarget, SftpRemoteFs,
};
use crate::events::{EVENT_RELEASE_PACKAGE_LOG, EVENT_RELEASE_PACKAGE_STATUS};
use crate::global_notification::{build_release_package_notification, GlobalNotification};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug)]
enum CommandError {
    Cancelled,
    ExitCode(i32),
    Spawn(String),
    Wait(String),
}

impl CommandError {
    fn message(&self) -> String {
        match self {
            Self::Cancelled => "构建已取消".into(),
            Self::ExitCode(code) => format!("PowerShell 命令退出码：{code}"),
            Self::Spawn(message) | Self::Wait(message) => message.clone(),
        }
    }
}

fn decode_console_line(bytes: &[u8]) -> String {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(value) => value.to_string(),
        Err(_) => encoding_rs::GBK.decode(bytes).0.into_owned(),
    }
}

fn spawn_reader<R>(
    reader: R,
    stream: &'static str,
    emit: Arc<dyn Fn(&'static str, String) + Send + Sync>,
) -> thread::JoinHandle<()>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut bytes = Vec::new();
        loop {
            bytes.clear();
            match reader.read_until(b'\n', &mut bytes) {
                Ok(0) | Err(_) => break,
                Ok(_) => emit(stream, decode_console_line(&bytes)),
            }
        }
    })
}

#[cfg(windows)]
fn run_powershell(
    cwd: &Path,
    command: &str,
    cancelled: Arc<AtomicBool>,
    pid_slot: Arc<Mutex<Option<u32>>>,
    emit: Arc<dyn Fn(&'static str, String) + Send + Sync>,
) -> Result<(), CommandError> {
    let mut pid_guard = pid_slot.lock().unwrap();
    if cancelled.load(Ordering::Acquire) {
        return Err(CommandError::Cancelled);
    }
    let child = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", command])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(_error) if cancelled.load(Ordering::Acquire) => return Err(CommandError::Cancelled),
        Err(error) => {
            return Err(CommandError::Spawn(format!(
                "启动 PowerShell 失败：{error}"
            )))
        }
    };
    *pid_guard = Some(child.id());
    drop(pid_guard);

    let stdout = spawn_reader(child.stdout.take().unwrap(), "stdout", emit.clone());
    let stderr = spawn_reader(child.stderr.take().unwrap(), "stderr", emit);
    let status = loop {
        if cancelled.load(Ordering::Acquire) {
            let _ = terminate_process_tree(child.id());
            let _ = child.wait();
            let _ = stdout.join();
            let _ = stderr.join();
            *pid_slot.lock().unwrap() = None;
            return Err(CommandError::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(error) => {
                let _ = terminate_process_tree(child.id());
                let _ = child.wait();
                let _ = stdout.join();
                let _ = stderr.join();
                *pid_slot.lock().unwrap() = None;
                if cancelled.load(Ordering::Acquire) {
                    return Err(CommandError::Cancelled);
                }
                return Err(CommandError::Wait(format!("等待 PowerShell 失败：{error}")));
            }
        }
    };
    let _ = stdout.join();
    let _ = stderr.join();
    *pid_slot.lock().unwrap() = None;
    if cancelled.load(Ordering::Acquire) {
        return Err(CommandError::Cancelled);
    }
    if status.success() {
        Ok(())
    } else {
        Err(CommandError::ExitCode(status.code().unwrap_or(-1)))
    }
}

#[cfg(not(windows))]
fn run_powershell(
    _cwd: &Path,
    _command: &str,
    _cancelled: Arc<AtomicBool>,
    _pid_slot: Arc<Mutex<Option<u32>>>,
    _emit: Arc<dyn Fn(&'static str, String) + Send + Sync>,
) -> Result<(), CommandError> {
    Err(CommandError::Spawn(
        "当前仅支持 Windows PowerShell 打包".into(),
    ))
}

#[cfg(windows)]
fn terminate_process_tree(pid: u32) -> Result<(), String> {
    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("终止构建进程失败：{error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let message = decode_console_line(&output.stderr);
        if message.contains("没有此任务的实例")
            || message.contains("进程不存在")
            || message.contains("not found")
        {
            return Ok(());
        }
        Err(format!("终止构建进程失败：{message}"))
    }
}

#[cfg(not(windows))]
fn terminate_process_tree(_pid: u32) -> Result<(), String> {
    Err("当前仅支持 Windows PowerShell 打包".into())
}

#[derive(Clone)]
struct ProcessSlots {
    frontend: Arc<Mutex<Option<u32>>>,
    backend: Arc<Mutex<Option<u32>>>,
}

impl ProcessSlots {
    fn new() -> Self {
        Self {
            frontend: Arc::new(Mutex::new(None)),
            backend: Arc::new(Mutex::new(None)),
        }
    }

    fn for_target(&self, target: ReleaseTarget) -> Arc<Mutex<Option<u32>>> {
        match target {
            ReleaseTarget::Frontend => self.frontend.clone(),
            ReleaseTarget::Backend => self.backend.clone(),
        }
    }

    fn terminate_all(&self) {
        for slot in [&self.frontend, &self.backend] {
            if let Some(pid) = *slot.lock().unwrap() {
                let _ = terminate_process_tree(pid);
            }
        }
    }
}

struct ActiveRun {
    run_id: String,
    cancelled: Arc<AtomicBool>,
    process_slots: ProcessSlots,
    ssh_socket: Arc<Mutex<Option<TcpStream>>>,
    finished: Arc<AtomicBool>,
    cancel_won: Arc<AtomicBool>,
    claim_lock: Arc<Mutex<()>>,
}

static ACTIVE_RUN: OnceLock<Mutex<Option<ActiveRun>>> = OnceLock::new();
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

fn active_run() -> &'static Mutex<Option<ActiveRun>> {
    ACTIVE_RUN.get_or_init(|| Mutex::new(None))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogEvent {
    run_id: String,
    project_id: i64,
    phase: String,
    stream: String,
    line: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusEvent {
    run_id: String,
    project_id: i64,
    status: String,
    phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    archive_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    uploaded_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_token: Option<String>,
}

trait EventSink: Send + Sync {
    fn log(&self, event: LogEvent);
    fn status(&self, event: StatusEvent);
    fn notification(&self, event: GlobalNotification);
}

struct TauriEventSink {
    app: tauri::AppHandle,
}

impl EventSink for TauriEventSink {
    fn log(&self, event: LogEvent) {
        let _ = self.app.emit(EVENT_RELEASE_PACKAGE_LOG, event);
    }

    fn status(&self, event: StatusEvent) {
        let _ = self.app.emit(EVENT_RELEASE_PACKAGE_STATUS, event);
    }

    fn notification(&self, event: GlobalNotification) {
        crate::global_notification::show_notifications(&self.app, vec![event]);
    }
}

#[derive(Debug)]
enum PipelineError {
    Cancelled { phase: &'static str },
    Failed { message: String },
}

fn emit_status(
    sink: &dyn EventSink,
    run_id: &str,
    project_id: i64,
    status: &str,
    phase: &str,
    archive_path: Option<String>,
    error: Option<String>,
) {
    sink.status(StatusEvent {
        run_id: run_id.into(),
        project_id,
        status: status.into(),
        phase: phase.into(),
        archive_path,
        error,
        uploaded_bytes: None,
        total_bytes: None,
        current_path: None,
        retry_token: None,
    });
}

fn emit_upload_status(
    sink: &dyn EventSink,
    run_id: &str,
    project_id: i64,
    uploaded_bytes: u64,
    total_bytes: u64,
    current_path: Option<String>,
) {
    sink.status(StatusEvent {
        run_id: run_id.into(),
        project_id,
        status: "uploading".into(),
        phase: "upload".into(),
        archive_path: None,
        error: None,
        uploaded_bytes: Some(uploaded_bytes),
        total_bytes: Some(total_bytes),
        current_path,
        retry_token: None,
    });
}

fn emit_terminal_result(
    sink: &dyn EventSink,
    run_id: &str,
    project: &ReleasePackageProjectConfig,
    result: Result<PipelineSummary, PipelineError>,
    emit_package_logs: bool,
) {
    let (status, archive_path, mut error, retry_descriptor) = match result {
        Ok(summary) => (
            summary.status,
            summary
                .archive_path
                .map(|path| path.to_string_lossy().into_owned()),
            summary.error,
            summary.retry_descriptor,
        ),
        Err(PipelineError::Cancelled { .. }) => ("cancelled", None, None, None),
        Err(PipelineError::Failed { message }) => ("failed", None, Some(message), None),
    };
    if emit_package_logs
        && archive_path.is_some()
        && matches!(status, "succeeded" | "partially_succeeded")
    {
        for phase in ["frontend", "backend"] {
            emit_system_log(sink, run_id, project.id, phase, "已完成打包");
        }
    }
    let retry_token = match retry_descriptor {
        Some(descriptor) => match issue_retry(project.id, descriptor) {
            Ok(token) => Some(token),
            Err(retry_error) => {
                let message = format!("创建上传重试任务失败：{retry_error}");
                error = Some(match error {
                    Some(error) => format!("{error}；{message}"),
                    None => message,
                });
                None
            }
        },
        None => None,
    };
    sink.status(StatusEvent {
        run_id: run_id.into(),
        project_id: project.id,
        status: status.into(),
        phase: "overall".into(),
        archive_path: archive_path.clone(),
        error: error.clone(),
        uploaded_bytes: None,
        total_bytes: None,
        current_path: None,
        retry_token,
    });
    if let Some(notification) = build_release_package_notification(
        run_id,
        project.id,
        &project.name,
        project.package_type,
        "overall",
        status,
        archive_path,
        error,
    ) {
        sink.notification(notification);
    }
}

fn emit_system_log(sink: &dyn EventSink, run_id: &str, project_id: i64, phase: &str, line: &str) {
    sink.log(LogEvent {
        run_id: run_id.into(),
        project_id,
        phase: phase.into(),
        stream: "system".into(),
        line: line.into(),
    });
}

fn run_command_phase(
    run_id: &str,
    project_id: i64,
    phase: &'static str,
    cwd: &Path,
    command: &str,
    cancelled: Arc<AtomicBool>,
    pid: Arc<Mutex<Option<u32>>>,
    sink: Arc<dyn EventSink>,
) -> Result<(), PipelineError> {
    if cancelled.load(Ordering::Acquire) {
        return Err(PipelineError::Cancelled { phase });
    }
    emit_status(
        sink.as_ref(),
        run_id,
        project_id,
        "running",
        phase,
        None,
        None,
    );
    emit_system_log(sink.as_ref(), run_id, project_id, phase, "开始执行构建命令");
    let event_run_id = run_id.to_owned();
    let event_phase = phase.to_owned();
    let event_sink = sink.clone();
    run_powershell(
        cwd,
        command,
        cancelled,
        pid,
        Arc::new(move |stream, line| {
            event_sink.log(LogEvent {
                run_id: event_run_id.clone(),
                project_id,
                phase: event_phase.clone(),
                stream: stream.into(),
                line,
            });
        }),
    )
    .map_err(|error| match error {
        CommandError::Cancelled => PipelineError::Cancelled { phase },
        error => PipelineError::Failed {
            message: error.message(),
        },
    })
}

fn target_phase(target: ReleaseTarget) -> &'static str {
    match target {
        ReleaseTarget::Frontend => "frontend",
        ReleaseTarget::Backend => "backend",
    }
}

fn archive_pipeline_error(error: ArchiveError, phase: &'static str) -> PipelineError {
    match error {
        ArchiveError::Cancelled => PipelineError::Cancelled { phase },
        ArchiveError::Failed(message) => PipelineError::Failed { message },
        ArchiveError::CommittedWithWarning { warning, .. } => {
            PipelineError::Failed { message: warning }
        }
    }
}

#[derive(Clone, Debug)]
struct BuiltTarget {
    target: ReleaseTarget,
    source_path: PathBuf,
    artifact_mode: String,
}

#[derive(Debug)]
struct BuildSummary {
    status: &'static str,
    built_targets: Vec<BuiltTarget>,
    selected_count: usize,
    error: Option<String>,
}

fn run_target(
    target: ReleaseTarget,
    run_id: &str,
    project: &ReleasePackageProjectConfig,
    cancelled: Arc<AtomicBool>,
    pid: Arc<Mutex<Option<u32>>>,
    sink: Arc<dyn EventSink>,
) -> Result<BuiltTarget, PipelineError> {
    let phase = target_phase(target);
    let (project_path, command, artifact_path) = match target {
        ReleaseTarget::Frontend => (
            PathBuf::from(&project.frontend_project_path),
            project.frontend_build_command.as_str(),
            project.frontend_artifact_path.as_str(),
        ),
        ReleaseTarget::Backend => (
            PathBuf::from(&project.backend_project_path),
            project.backend_build_command.as_str(),
            project.backend_artifact_path.as_str(),
        ),
    };
    run_command_phase(
        run_id,
        project.id,
        phase,
        &project_path,
        command,
        cancelled.clone(),
        pid,
        sink.clone(),
    )?;
    let artifact = resolve_artifact_path(&project_path, artifact_path);
    match target {
        ReleaseTarget::Frontend if !artifact.is_dir() => {
            return Err(PipelineError::Failed {
                message: "前端产物必须是文件夹".into(),
            });
        }
        ReleaseTarget::Backend if !artifact.is_file() => {
            return Err(PipelineError::Failed {
                message: "后端产物必须是文件".into(),
            });
        }
        _ => {}
    }
    Ok(BuiltTarget {
        target,
        source_path: artifact,
        artifact_mode: match target {
            ReleaseTarget::Frontend => project.frontend_artifact_mode.clone(),
            ReleaseTarget::Backend => "file".into(),
        },
    })
}

fn emit_target_result(
    sink: &dyn EventSink,
    run_id: &str,
    project_id: i64,
    target: ReleaseTarget,
    result: &Result<BuiltTarget, PipelineError>,
) {
    let phase = target_phase(target);
    match result {
        Ok(_) => emit_status(sink, run_id, project_id, "succeeded", phase, None, None),
        Err(PipelineError::Cancelled { .. }) => {
            emit_status(sink, run_id, project_id, "cancelled", phase, None, None)
        }
        Err(PipelineError::Failed { message }) => emit_status(
            sink,
            run_id,
            project_id,
            "failed",
            phase,
            None,
            Some(message.clone()),
        ),
    }
}

#[derive(Debug)]
struct PipelineSummary {
    status: &'static str,
    archive_path: Option<PathBuf>,
    archived_targets: Vec<ArchivedTarget>,
    manifests: Vec<ArtifactManifest>,
    error: Option<String>,
    retry_descriptor: Option<RetryDescriptor>,
    remote_committed: bool,
}

#[derive(Clone, Debug)]
struct RetryDescriptor {
    manifests: Vec<ArtifactManifest>,
}

#[derive(Clone, Debug)]
struct RetryJob {
    project_id: i64,
    descriptor: RetryDescriptor,
}

impl RetryJob {
    fn from_manifests(project_id: i64, manifests: Vec<ArtifactManifest>) -> Self {
        Self {
            project_id,
            descriptor: RetryDescriptor { manifests },
        }
    }
}

static RETRY_JOBS: OnceLock<Mutex<HashMap<String, RetryJob>>> = OnceLock::new();

fn retry_jobs() -> &'static Mutex<HashMap<String, RetryJob>> {
    RETRY_JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn issue_retry(project_id: i64, descriptor: RetryDescriptor) -> Result<String, String> {
    let token = uuid::Uuid::new_v4().to_string();
    retry_jobs()
        .lock()
        .map_err(|_| "上传重试任务存储不可用".to_string())?
        .insert(
            token.clone(),
            RetryJob {
                project_id,
                descriptor,
            },
        );
    Ok(token)
}

fn consume_retry(token: &str, project_id: i64) -> Result<RetryJob, String> {
    let retry = retry_jobs()
        .lock()
        .map_err(|_| "上传重试任务存储不可用".to_string())?
        .remove(token)
        .ok_or_else(|| "上传重试令牌无效或已使用".to_string())?;
    if retry.project_id != project_id {
        return Err("上传重试令牌与当前项目不匹配".into());
    }
    Ok(retry)
}

pub(crate) fn retry_targets(token: &str, project_id: i64) -> Result<Vec<ReleaseTarget>, String> {
    let retries = retry_jobs()
        .lock()
        .map_err(|_| "上传重试任务存储不可用".to_string())?;
    let retry = retries
        .get(token)
        .filter(|retry| retry.project_id == project_id)
        .ok_or_else(|| "上传重试令牌无效或与当前项目不匹配".to_string())?;
    Ok(retry
        .descriptor
        .manifests
        .iter()
        .map(|manifest| manifest.target)
        .collect())
}

fn package_can_upload(summary: &PipelineSummary) -> bool {
    summary.status == "succeeded" && !summary.manifests.is_empty()
}

fn build_upload_summary(summary: BuildSummary) -> Result<PipelineSummary, PipelineError> {
    let mut manifests = Vec::with_capacity(summary.built_targets.len());
    for built in summary.built_targets {
        let manifest = match built.target {
            ReleaseTarget::Frontend => {
                ArtifactManifest::from_directory(built.target, &built.source_path)
            }
            ReleaseTarget::Backend => ArtifactManifest::from_file(built.target, &built.source_path),
        }
        .map_err(|message| PipelineError::Failed { message })?;
        manifests.push(manifest);
    }
    Ok(PipelineSummary {
        status: summary.status,
        archive_path: None,
        archived_targets: Vec::new(),
        manifests,
        error: summary.error,
        retry_descriptor: None,
        remote_committed: false,
    })
}

fn combine_package_and_deploy(
    mut summary: PipelineSummary,
    deploy_result: Result<(), DeployError>,
) -> PipelineSummary {
    match deploy_result {
        Ok(()) => {
            summary.remote_committed = true;
            summary
        }
        Err(error) if error.committed => {
            let recovery = if error.recovery_paths.is_empty() {
                String::new()
            } else {
                format!("；需人工检查：{}", error.recovery_paths.join("、"))
            };
            let warning = format!("{}{recovery}", error.message);
            summary.error = Some(match summary.error.take() {
                Some(existing) => format!("{existing}；{warning}"),
                None => warning,
            });
            summary.retry_descriptor = None;
            summary.remote_committed = true;
            summary
        }
        Err(error) if error.cancelled => {
            summary.status = "cancelled";
            summary.error = Some(error.message);
            summary.retry_descriptor = None;
            summary.remote_committed = false;
            summary
        }
        Err(error) => {
            let recovery = if error.recovery_paths.is_empty() {
                String::new()
            } else {
                format!("；需人工检查：{}", error.recovery_paths.join("、"))
            };
            summary.status = "package_succeeded_upload_failed";
            summary.error = Some(format!("{}{recovery}", error.message));
            summary.retry_descriptor = Some(RetryDescriptor {
                manifests: summary.manifests.clone(),
            });
            summary.remote_committed = false;
            summary
        }
    }
}

fn validate_remote_overwrite(
    consumed: &ConsumedPreflight,
    confirmed: &[ReleaseTarget],
) -> Result<(), String> {
    let expected = consumed
        .expected_existing_targets
        .iter()
        .map(|target| match target {
            RemoteTarget::Frontend => ReleaseTarget::Frontend,
            RemoteTarget::Backend => ReleaseTarget::Backend,
        })
        .collect::<Vec<_>>();
    if expected.len() != confirmed.len()
        || expected.iter().any(|target| !confirmed.contains(target))
    {
        return Err("远端覆盖确认与预检结果不一致，请重新确认".into());
    }
    Ok(())
}

pub(crate) struct DeployAuthorization {
    consumed: ConsumedPreflight,
}

pub(crate) enum RuntimeStartRequest {
    LocalArchive {
        output_root: PathBuf,
        folder_name: String,
        overwrite_existing: bool,
    },
    ServerUpload {
        deploy_authorization: DeployAuthorization,
    },
}

pub(crate) fn consume_deploy_authorization(
    token: &str,
    binding: &PreflightBinding,
    confirmed_overwrites: &[ReleaseTarget],
) -> Result<DeployAuthorization, String> {
    let consumed = consume_preflight(token, binding)?;
    validate_remote_overwrite(&consumed, confirmed_overwrites)?;
    Ok(DeployAuthorization { consumed })
}

fn build_deployment_request(
    run_id: &str,
    summary: &PipelineSummary,
    consumed: &ConsumedPreflight,
) -> Result<DeploymentRequest, DeployError> {
    build_manifest_deployment_request(run_id, &summary.manifests, consumed)
}

fn build_manifest_deployment_request(
    run_id: &str,
    manifests: &[ArtifactManifest],
    consumed: &ConsumedPreflight,
) -> Result<DeploymentRequest, DeployError> {
    if manifests.len() != consumed.binding.targets.len() {
        return Err(DeployError::failed(
            "部署产物目标与远端预检目标不一致，请重新预检",
        ));
    }
    let mut targets = Vec::with_capacity(manifests.len());
    for manifest in manifests {
        let remote_target = match manifest.target {
            ReleaseTarget::Frontend => RemoteTarget::Frontend,
            ReleaseTarget::Backend => RemoteTarget::Backend,
        };
        if !consumed.binding.targets.contains(&remote_target) {
            return Err(DeployError::failed(
                "部署产物目标与远端预检目标不一致，请重新预检",
            ));
        }
        let remote_path = match manifest.target {
            ReleaseTarget::Frontend => consumed.binding.frontend_remote_dir.clone(),
            ReleaseTarget::Backend => consumed.binding.backend_remote_path.clone(),
        };
        targets.push(DeploymentTarget {
            manifest: manifest.clone(),
            remote_path,
            expected_exists: consumed.expected_existing_targets.contains(&remote_target),
        });
    }
    Ok(DeploymentRequest {
        run_id: run_id.to_owned(),
        targets,
    })
}

fn build_retry_deployment_request(
    run_id: &str,
    retry: &RetryJob,
    consumed: &ConsumedPreflight,
) -> Result<DeploymentRequest, DeployError> {
    for manifest in &retry.descriptor.manifests {
        if manifest.verify_source().is_err() {
            return Err(DeployError::failed("部署产物在打包后发生变化，请重新打包"));
        }
    }
    build_manifest_deployment_request(run_id, &retry.descriptor.manifests, consumed)
}

fn run_deployment_phase(
    run_id: &str,
    project: &ReleasePackageProjectConfig,
    summary: PipelineSummary,
    authorization: DeployAuthorization,
    cancelled: &AtomicBool,
    ssh_socket: &Arc<Mutex<Option<TcpStream>>>,
    sink: &dyn EventSink,
) -> PipelineSummary {
    if !package_can_upload(&summary) {
        return summary;
    }
    let request = match build_deployment_request(run_id, &summary, &authorization.consumed) {
        Ok(request) => request,
        Err(error) => return combine_package_and_deploy(summary, Err(error)),
    };
    execute_deployment_request(
        run_id,
        project.id,
        summary,
        request,
        &authorization.consumed,
        cancelled,
        ssh_socket,
        sink,
    )
}

fn execute_deployment_request(
    run_id: &str,
    project_id: i64,
    summary: PipelineSummary,
    request: DeploymentRequest,
    consumed: &ConsumedPreflight,
    cancelled: &AtomicBool,
    ssh_socket: &Arc<Mutex<Option<TcpStream>>>,
    sink: &dyn EventSink,
) -> PipelineSummary {
    let total_bytes = request
        .targets
        .iter()
        .map(|target| target.manifest.total_bytes)
        .sum();
    emit_system_log(sink, run_id, project_id, "upload", "开始上传服务器");
    emit_upload_status(sink, run_id, project_id, 0, total_bytes, None);
    let deploy_result = match SftpRemoteFs::connect(
        &consumed.binding,
        &consumed.expected_fingerprint,
        &consumed.secret,
        ssh_socket,
    ) {
        Ok(mut remote) => {
            let mut uploaded_bytes = 0_u64;
            deploy(&mut remote, &request, cancelled, |bytes, current_path| {
                uploaded_bytes = uploaded_bytes.saturating_add(bytes);
                emit_upload_status(
                    sink,
                    run_id,
                    project_id,
                    uploaded_bytes,
                    total_bytes,
                    Some(current_path.to_owned()),
                );
            })
        }
        Err(error) => Err(error),
    };
    if let Ok(mut socket) = ssh_socket.lock() {
        socket.take();
    }
    let summary = combine_package_and_deploy(summary, deploy_result);
    if summary.remote_committed {
        emit_system_log(sink, run_id, project_id, "upload", "服务器上传完成");
    }
    summary
}

fn run_retry_deployment_phase(
    run_id: &str,
    retry: RetryJob,
    authorization: DeployAuthorization,
    cancelled: &AtomicBool,
    ssh_socket: &Arc<Mutex<Option<TcpStream>>>,
    sink: &dyn EventSink,
) -> PipelineSummary {
    let summary = PipelineSummary {
        status: "succeeded",
        archive_path: None,
        archived_targets: Vec::new(),
        manifests: retry.descriptor.manifests.clone(),
        error: None,
        retry_descriptor: None,
        remote_committed: false,
    };
    let request = match build_retry_deployment_request(run_id, &retry, &authorization.consumed) {
        Ok(request) => request,
        Err(error) => return combine_package_and_deploy(summary, Err(error)),
    };
    execute_deployment_request(
        run_id,
        retry.project_id,
        summary,
        request,
        &authorization.consumed,
        cancelled,
        ssh_socket,
        sink,
    )
}

fn run_build_pipeline(
    run_id: &str,
    project: ReleasePackageProjectConfig,
    targets: Vec<ReleaseTarget>,
    cancelled: Arc<AtomicBool>,
    process_slots: ProcessSlots,
    sink: Arc<dyn EventSink>,
) -> Result<BuildSummary, PipelineError> {
    let selected_count = targets.len();
    let mut handles = Vec::with_capacity(selected_count);
    for target in targets {
        let thread_run_id = run_id.to_owned();
        let thread_project = project.clone();
        let thread_cancelled = cancelled.clone();
        let thread_sink = sink.clone();
        let pid = process_slots.for_target(target);
        handles.push((
            target,
            thread::spawn(move || {
                let result = run_target(
                    target,
                    &thread_run_id,
                    &thread_project,
                    thread_cancelled,
                    pid,
                    thread_sink.clone(),
                );
                emit_target_result(
                    thread_sink.as_ref(),
                    &thread_run_id,
                    thread_project.id,
                    target,
                    &result,
                );
                result
            }),
        ));
    }

    let mut built_targets = Vec::new();
    let mut errors = Vec::new();
    for (target, handle) in handles {
        let result = handle.join().unwrap_or_else(|_| {
            Err(PipelineError::Failed {
                message: "打包工作线程异常退出".into(),
            })
        });
        match result {
            Ok(built_target) => built_targets.push(built_target),
            Err(PipelineError::Cancelled { .. }) => {}
            Err(PipelineError::Failed { message }) => {
                errors.push(format!("{}：{message}", target_phase(target)));
            }
        }
    }
    if cancelled.load(Ordering::Acquire) {
        return Err(PipelineError::Cancelled { phase: "overall" });
    }
    let success_count = built_targets.len();
    Ok(BuildSummary {
        status: if success_count == 0 {
            "failed"
        } else if success_count == selected_count {
            "succeeded"
        } else {
            "partially_succeeded"
        },
        built_targets,
        selected_count,
        error: (!errors.is_empty()).then(|| errors.join("；")),
    })
}

fn emit_local_archive_target_status(
    sink: &dyn EventSink,
    run_id: &str,
    project_id: i64,
    target: ReleaseTarget,
    status: &str,
    error: Option<String>,
) {
    emit_status(
        sink,
        run_id,
        project_id,
        status,
        target_phase(target),
        None,
        error,
    );
}

fn merge_pipeline_error(existing: Option<&str>, error: PipelineError) -> PipelineError {
    match error {
        PipelineError::Cancelled { phase } => PipelineError::Cancelled { phase },
        PipelineError::Failed { message } => {
            let mut messages = Vec::new();
            for message in existing
                .into_iter()
                .flat_map(|message| message.split('；'))
                .chain(message.split('；'))
                .filter(|message| !message.is_empty())
            {
                if !messages.contains(&message) {
                    messages.push(message);
                }
            }
            PipelineError::Failed {
                message: messages.join("；"),
            }
        }
    }
}

fn run_local_archive_pipeline(
    run_id: &str,
    project_id: i64,
    summary: BuildSummary,
    output_root: PathBuf,
    folder_name: String,
    overwrite_existing: bool,
    cancelled: Arc<AtomicBool>,
    sink: Arc<dyn EventSink>,
) -> Result<PipelineSummary, PipelineError> {
    if cancelled.load(Ordering::Acquire) {
        for target in &summary.built_targets {
            emit_local_archive_target_status(
                sink.as_ref(),
                run_id,
                project_id,
                target.target,
                "cancelled",
                None,
            );
        }
        return Err(PipelineError::Cancelled { phase: "overall" });
    }
    if summary.built_targets.is_empty() {
        return Ok(PipelineSummary {
            status: summary.status,
            archive_path: None,
            archived_targets: Vec::new(),
            manifests: Vec::new(),
            error: summary.error,
            retry_descriptor: None,
            remote_committed: false,
        });
    }

    let frontend = summary
        .built_targets
        .iter()
        .find(|target| target.target == ReleaseTarget::Frontend);
    let backend = summary
        .built_targets
        .iter()
        .find(|target| target.target == ReleaseTarget::Backend);
    if let (Some(frontend), Some(backend)) = (frontend, backend) {
        if let Err(error) = validate_artifact_target_collision(
            &frontend.source_path,
            &frontend.artifact_mode,
            &backend.source_path,
        ) {
            let archive_error = archive_pipeline_error(error, "overall");
            let (status, message) = match &archive_error {
                PipelineError::Cancelled { .. } => ("cancelled", None),
                PipelineError::Failed { message } => ("failed", Some(message.clone())),
            };
            for target in &summary.built_targets {
                emit_local_archive_target_status(
                    sink.as_ref(),
                    run_id,
                    project_id,
                    target.target,
                    status,
                    message.clone(),
                );
            }
            return Err(merge_pipeline_error(
                summary.error.as_deref(),
                archive_error,
            ));
        }
    }

    let mut archive = match ArchiveSession::create(
        &output_root,
        &folder_name,
        run_id,
        overwrite_existing,
        cancelled.as_ref(),
    ) {
        Ok(archive) => archive,
        Err(error) => {
            let archive_error = archive_pipeline_error(error, "overall");
            let (status, message) = match &archive_error {
                PipelineError::Cancelled { .. } => ("cancelled", None),
                PipelineError::Failed { message } => ("failed", Some(message.clone())),
            };
            for target in &summary.built_targets {
                emit_local_archive_target_status(
                    sink.as_ref(),
                    run_id,
                    project_id,
                    target.target,
                    status,
                    message.clone(),
                );
            }
            return Err(merge_pipeline_error(
                summary.error.as_deref(),
                archive_error,
            ));
        }
    };
    let staging_path = archive.staging_path().to_path_buf();
    let mut archived_targets = Vec::new();
    let mut errors = summary.error.into_iter().collect::<Vec<_>>();
    for built_target in summary.built_targets {
        let phase = target_phase(built_target.target);
        let emit = |line: &str| emit_system_log(sink.as_ref(), run_id, project_id, phase, line);
        let archive_result = match built_target.target {
            ReleaseTarget::Frontend => archive_frontend_artifact(
                &built_target.source_path,
                &built_target.artifact_mode,
                &staging_path,
                cancelled.as_ref(),
                emit,
            ),
            ReleaseTarget::Backend => archive_backend_artifact(
                &built_target.source_path,
                &staging_path,
                cancelled.as_ref(),
                emit,
            ),
        };
        match archive_result {
            Ok(archive_entry_name) => archived_targets.push(ArchivedTarget {
                target: built_target.target,
                archive_entry_name,
                artifact_mode: built_target.artifact_mode,
            }),
            Err(ArchiveError::Cancelled) => emit_local_archive_target_status(
                sink.as_ref(),
                run_id,
                project_id,
                built_target.target,
                "cancelled",
                None,
            ),
            Err(ArchiveError::Failed(message)) => {
                emit_local_archive_target_status(
                    sink.as_ref(),
                    run_id,
                    project_id,
                    built_target.target,
                    "failed",
                    Some(message.clone()),
                );
                errors.push(format!("{phase}：{message}"));
            }
            Err(ArchiveError::CommittedWithWarning { warning, .. }) => {
                emit_local_archive_target_status(
                    sink.as_ref(),
                    run_id,
                    project_id,
                    built_target.target,
                    "failed",
                    Some(warning.clone()),
                );
                errors.push(format!("{phase}：{warning}"));
            }
        }
    }
    if cancelled.load(Ordering::Acquire) {
        for target in &archived_targets {
            emit_local_archive_target_status(
                sink.as_ref(),
                run_id,
                project_id,
                target.target,
                "cancelled",
                None,
            );
        }
        return Err(PipelineError::Cancelled { phase: "overall" });
    }
    let success_count = archived_targets.len();
    if success_count == 0 {
        return Ok(PipelineSummary {
            status: "failed",
            archive_path: None,
            archived_targets,
            manifests: Vec::new(),
            error: (!errors.is_empty()).then(|| errors.join("；")),
            retry_descriptor: None,
            remote_committed: false,
        });
    }
    let (archive_path, cleanup_warning) = match archive.commit(cancelled.as_ref()) {
        Ok(path) => (path, None),
        Err(ArchiveError::CommittedWithWarning {
            final_path,
            warning,
        }) => (final_path, Some(warning)),
        Err(error) => {
            let accumulated_error = errors.join("；");
            let archive_error = archive_pipeline_error(error, "overall");
            let (status, message) = match &archive_error {
                PipelineError::Cancelled { .. } => ("cancelled", None),
                PipelineError::Failed { message } => ("failed", Some(message.clone())),
            };
            for target in &archived_targets {
                emit_local_archive_target_status(
                    sink.as_ref(),
                    run_id,
                    project_id,
                    target.target,
                    status,
                    message.clone(),
                );
            }
            return Err(merge_pipeline_error(
                (!accumulated_error.is_empty()).then_some(accumulated_error.as_str()),
                archive_error,
            ));
        }
    };
    errors.extend(cleanup_warning);
    Ok(PipelineSummary {
        status: if success_count == summary.selected_count {
            "succeeded"
        } else {
            "partially_succeeded"
        },
        archive_path: Some(archive_path),
        archived_targets,
        manifests: Vec::new(),
        error: (!errors.is_empty()).then(|| errors.join("；")),
        retry_descriptor: None,
        remote_committed: false,
    })
}

fn claim_pipeline_result(
    result: Result<PipelineSummary, PipelineError>,
    cancelled: &AtomicBool,
    finished: &AtomicBool,
    cancel_won: &AtomicBool,
    claim_lock: &Mutex<()>,
) -> Result<PipelineSummary, PipelineError> {
    let _guard = claim_lock.lock().unwrap();
    let remote_committed = matches!(&result, Ok(summary) if summary.remote_committed);
    let archived_cancellation = matches!(
        &result,
        Ok(summary) if summary.status == "cancelled" && summary.archive_path.is_some()
    );
    let result = if cancelled.load(Ordering::Acquire) && !remote_committed {
        cancel_won.store(true, Ordering::Release);
        if archived_cancellation {
            result
        } else {
            Err(PipelineError::Cancelled { phase: "overall" })
        }
    } else {
        result
    };
    finished.store(true, Ordering::Release);
    result
}

fn request_cancel(active: &ActiveRun) -> bool {
    let _guard = active.claim_lock.lock().unwrap();
    active.cancelled.store(true, Ordering::Release);
    if active.finished.load(Ordering::Acquire) {
        if active.cancel_won.load(Ordering::Acquire) {
            return true;
        }
        active.cancelled.store(false, Ordering::Release);
        return false;
    }
    active.process_slots.terminate_all();
    if let Some(socket) = active.ssh_socket.lock().unwrap().take() {
        let _ = socket.shutdown(Shutdown::Both);
    }
    true
}

pub fn start(
    app: &tauri::AppHandle,
    project: ReleasePackageProjectConfig,
    targets: Vec<ReleaseTarget>,
    request: RuntimeStartRequest,
) -> Result<Value, String> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let cancelled = Arc::new(AtomicBool::new(false));
    let process_slots = ProcessSlots::new();
    let ssh_socket = Arc::new(Mutex::new(None));
    let finished = Arc::new(AtomicBool::new(false));
    let cancel_won = Arc::new(AtomicBool::new(false));
    let claim_lock = Arc::new(Mutex::new(()));
    {
        let mut active = active_run()
            .lock()
            .map_err(|_| "release package runtime lock poisoned")?;
        if SHUTTING_DOWN.load(Ordering::Acquire) {
            return Err("应用正在退出，不能启动发布打包任务".into());
        }
        if active.is_some() {
            return Err("已有发布打包任务正在运行".into());
        }
        *active = Some(ActiveRun {
            run_id: run_id.clone(),
            cancelled: cancelled.clone(),
            process_slots: process_slots.clone(),
            ssh_socket: ssh_socket.clone(),
            finished: finished.clone(),
            cancel_won: cancel_won.clone(),
            claim_lock: claim_lock.clone(),
        });
    }

    let thread_run_id = run_id.clone();
    let project_id = project.id;
    let terminal_project = project.clone();
    let emit_package_logs = matches!(&request, RuntimeStartRequest::LocalArchive { .. });
    let sink: Arc<dyn EventSink> = Arc::new(TauriEventSink { app: app.clone() });
    thread::spawn(move || {
        emit_status(
            sink.as_ref(),
            &thread_run_id,
            project_id,
            "running",
            "overall",
            None,
            None,
        );
        let result = run_build_pipeline(
            &thread_run_id,
            project,
            targets,
            cancelled.clone(),
            process_slots,
            sink.clone(),
        );
        let result = match request {
            RuntimeStartRequest::LocalArchive {
                output_root,
                folder_name,
                overwrite_existing,
            } => result.and_then(|summary| {
                run_local_archive_pipeline(
                    &thread_run_id,
                    project_id,
                    summary,
                    output_root,
                    folder_name,
                    overwrite_existing,
                    cancelled.clone(),
                    sink.clone(),
                )
            }),
            RuntimeStartRequest::ServerUpload {
                deploy_authorization,
            } => result.and_then(build_upload_summary).map(|summary| {
                run_deployment_phase(
                    &thread_run_id,
                    &terminal_project,
                    summary,
                    deploy_authorization,
                    cancelled.as_ref(),
                    &ssh_socket,
                    sink.as_ref(),
                )
            }),
        };
        let result = claim_pipeline_result(result, &cancelled, &finished, &cancel_won, &claim_lock);
        emit_terminal_result(
            sink.as_ref(),
            &thread_run_id,
            &terminal_project,
            result,
            emit_package_logs,
        );
        if let Ok(mut active) = active_run().lock() {
            if active.as_ref().map(|run| run.run_id.as_str()) == Some(thread_run_id.as_str()) {
                *active = None;
            }
        }
    });
    Ok(json!({ "runId": run_id }))
}

pub fn upload_retry(
    app: &tauri::AppHandle,
    project: ReleasePackageProjectConfig,
    retry_token: &str,
    deploy_authorization: DeployAuthorization,
) -> Result<Value, String> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let cancelled = Arc::new(AtomicBool::new(false));
    let process_slots = ProcessSlots::new();
    let ssh_socket = Arc::new(Mutex::new(None));
    let finished = Arc::new(AtomicBool::new(false));
    let cancel_won = Arc::new(AtomicBool::new(false));
    let claim_lock = Arc::new(Mutex::new(()));
    {
        let mut active = active_run()
            .lock()
            .map_err(|_| "release package runtime lock poisoned")?;
        if SHUTTING_DOWN.load(Ordering::Acquire) {
            return Err("应用正在退出，不能启动上传重试".into());
        }
        if active.is_some() {
            return Err("已有发布打包或上传任务正在运行".into());
        }
        *active = Some(ActiveRun {
            run_id: run_id.clone(),
            cancelled: cancelled.clone(),
            process_slots,
            ssh_socket: ssh_socket.clone(),
            finished: finished.clone(),
            cancel_won: cancel_won.clone(),
            claim_lock: claim_lock.clone(),
        });
    }
    let retry = match consume_retry(retry_token, project.id) {
        Ok(retry) => retry,
        Err(error) => {
            if let Ok(mut active) = active_run().lock() {
                if active.as_ref().map(|run| run.run_id.as_str()) == Some(run_id.as_str()) {
                    *active = None;
                }
            }
            return Err(error);
        }
    };

    let thread_run_id = run_id.clone();
    let sink: Arc<dyn EventSink> = Arc::new(TauriEventSink { app: app.clone() });
    thread::spawn(move || {
        emit_status(
            sink.as_ref(),
            &thread_run_id,
            project.id,
            "running",
            "overall",
            None,
            None,
        );
        let summary = run_retry_deployment_phase(
            &thread_run_id,
            retry,
            deploy_authorization,
            cancelled.as_ref(),
            &ssh_socket,
            sink.as_ref(),
        );
        let result =
            claim_pipeline_result(Ok(summary), &cancelled, &finished, &cancel_won, &claim_lock);
        emit_terminal_result(sink.as_ref(), &thread_run_id, &project, result, false);
        if let Ok(mut active) = active_run().lock() {
            if active.as_ref().map(|run| run.run_id.as_str()) == Some(thread_run_id.as_str()) {
                *active = None;
            }
        }
    });
    Ok(json!({ "runId": run_id }))
}

pub fn cancel(run_id: &str) -> Result<Value, String> {
    let active = active_run()
        .lock()
        .map_err(|_| "release package runtime lock poisoned")?;
    let Some(active) = active.as_ref().filter(|active| active.run_id == run_id) else {
        return Err("发布打包任务不存在或 runId 不匹配".into());
    };
    Ok(json!({ "cancelRequested": request_cancel(active) }))
}

pub fn on_app_exit() {
    super::release_package_remote::clear_temporary_stores();
    if let Some(retries) = RETRY_JOBS.get() {
        if let Ok(mut retries) = retries.lock() {
            retries.clear();
        }
    }
    SHUTTING_DOWN.store(true, Ordering::Release);
    if let Ok(active) = active_run().lock() {
        if let Some(active) = active.as_ref() {
            request_cancel(active);
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn powershell_reports_both_streams_and_nonzero_exit() {
        let logs = Arc::new(Mutex::new(Vec::new()));
        let sink = logs.clone();
        let result = run_powershell(
            &std::env::temp_dir(),
            "Write-Output 'front-ok'; [Console]::Error.WriteLine('front-err'); exit 7",
            Arc::new(AtomicBool::new(false)),
            Arc::new(Mutex::new(None)),
            Arc::new(move |stream, line| sink.lock().unwrap().push((stream.to_string(), line))),
        );
        assert!(matches!(result, Err(CommandError::ExitCode(7))));
        let lines = logs.lock().unwrap();
        assert!(lines.iter().any(|(_, line)| line.contains("front-ok")));
        assert!(lines.iter().any(|(_, line)| line.contains("front-err")));
    }

    #[test]
    fn powershell_process_tree_can_be_cancelled() {
        let cancel = Arc::new(AtomicBool::new(false));
        let pid = Arc::new(Mutex::new(None));
        let handle = {
            let cancel = cancel.clone();
            let pid = pid.clone();
            thread::spawn(move || {
                run_powershell(
                    &std::env::temp_dir(),
                    "Start-Sleep -Seconds 30",
                    cancel,
                    pid,
                    Arc::new(|_, _| {}),
                )
            })
        };
        thread::sleep(Duration::from_millis(500));
        cancel.store(true, Ordering::Release);
        if let Some(value) = *pid.lock().unwrap() {
            let _ = terminate_process_tree(value);
        }
        assert!(matches!(
            handle.join().unwrap(),
            Err(CommandError::Cancelled)
        ));
    }

    #[test]
    fn cancelled_before_spawn_does_not_try_to_start_process() {
        let cancelled = Arc::new(AtomicBool::new(true));
        let pid = Arc::new(Mutex::new(None));
        let result = run_powershell(
            Path::new("Z:\\missing-cancelled-cwd"),
            "exit 0",
            cancelled,
            pid.clone(),
            Arc::new(|_, _| {}),
        );
        assert!(matches!(result, Err(CommandError::Cancelled)));
        assert!(pid.lock().unwrap().is_none());
    }

    #[test]
    fn decoder_prefers_utf8_then_falls_back_to_gbk() {
        assert_eq!(decode_console_line("构建成功\r\n".as_bytes()), "构建成功");
        let (gbk, _, _) = encoding_rs::GBK.encode("构建成功\r\n");
        assert_eq!(decode_console_line(&gbk), "构建成功");
    }
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;
    #[cfg(windows)]
    use std::fs;
    use std::sync::atomic::AtomicBool;

    #[cfg(windows)]
    struct TestDir(PathBuf);

    #[cfg(windows)]
    impl TestDir {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("lazycat-release-runtime-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    #[cfg(windows)]
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(windows)]
    #[derive(Default)]
    struct CollectingSink {
        statuses: Mutex<Vec<StatusEvent>>,
        cancel_on_archive: Option<Arc<AtomicBool>>,
    }

    #[cfg(windows)]
    impl EventSink for CollectingSink {
        fn log(&self, event: LogEvent) {
            if event.line == "正在归档前端产物" {
                if let Some(cancelled) = &self.cancel_on_archive {
                    cancelled.store(true, Ordering::Release);
                }
            }
        }

        fn status(&self, event: StatusEvent) {
            self.statuses.lock().unwrap().push(event);
        }

        fn notification(&self, _event: GlobalNotification) {}
    }

    #[cfg(windows)]
    impl CollectingSink {
        fn cancelling_during_archive(cancelled: Arc<AtomicBool>) -> Self {
            Self {
                statuses: Mutex::new(Vec::new()),
                cancel_on_archive: Some(cancelled),
            }
        }

        fn phases(&self) -> Vec<String> {
            self.statuses
                .lock()
                .unwrap()
                .iter()
                .filter(|event| event.status == "running")
                .map(|event| event.phase.clone())
                .fold(Vec::new(), |mut phases, phase| {
                    if phases.last() != Some(&phase) {
                        phases.push(phase);
                    }
                    phases
                })
        }

        fn last_status(&self, phase: &str) -> Option<String> {
            self.statuses
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find(|event| event.phase == phase)
                .map(|event| event.status.clone())
        }

        fn last_error(&self, phase: &str) -> Option<String> {
            self.statuses
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find(|event| event.phase == phase)
                .and_then(|event| event.error.clone())
        }
    }

    struct Sink;

    impl EventSink for Sink {
        fn log(&self, _event: LogEvent) {}
        fn status(&self, _event: StatusEvent) {}
        fn notification(&self, _event: GlobalNotification) {}
    }

    #[derive(Default)]
    struct TerminalSink {
        logs: Mutex<Vec<LogEvent>>,
        statuses: Mutex<Vec<StatusEvent>>,
        notifications: Mutex<Vec<GlobalNotification>>,
    }

    impl EventSink for TerminalSink {
        fn log(&self, event: LogEvent) {
            self.logs.lock().unwrap().push(event);
        }

        fn status(&self, event: StatusEvent) {
            self.statuses.lock().unwrap().push(event);
        }

        fn notification(&self, event: GlobalNotification) {
            self.notifications.lock().unwrap().push(event);
        }
    }

    fn project() -> ReleasePackageProjectConfig {
        ReleasePackageProjectConfig {
            id: 7,
            name: "test".into(),
            output_root: "Z:\\output".into(),
            package_type: ReleasePackageType::LocalArchive,
            frontend_project_path: "Z:\\missing".into(),
            frontend_build_command: "exit 0".into(),
            frontend_artifact_path: "dist".into(),
            frontend_artifact_mode: "copy_directory".into(),
            backend_project_path: "Z:\\missing".into(),
            backend_build_command: "exit 0".into(),
            backend_artifact_path: "server.jar".into(),
            ssh_host: String::new(),
            ssh_port: 22,
            ssh_username: String::new(),
            ssh_auth_type: "password".into(),
            vault_entry_id: None,
            ssh_private_key_path: String::new(),
            frontend_remote_dir: String::new(),
            backend_remote_path: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[cfg(windows)]
    fn frontend_build_project(root: &Path, artifact_mode: &str) -> ReleasePackageProjectConfig {
        let frontend_project = root.join("web");
        let backend_project = root.join("server");
        fs::create_dir_all(&frontend_project).unwrap();
        fs::create_dir_all(&backend_project).unwrap();
        let mut project = project();
        project.frontend_project_path = frontend_project.to_string_lossy().into_owned();
        project.frontend_build_command =
            "New-Item -ItemType Directory -Force dist | Out-Null; Set-Content dist/index.html web"
                .into();
        project.frontend_artifact_path = "dist".into();
        project.frontend_artifact_mode = artifact_mode.into();
        project.backend_project_path = backend_project.to_string_lossy().into_owned();
        project
    }

    fn consumed_preflight_with_existing(
        expected_existing_targets: Vec<RemoteTarget>,
    ) -> ConsumedPreflight {
        ConsumedPreflight {
            binding: crate::tools::release_package_remote::PreflightBinding {
                project_id: 7,
                endpoint: crate::tools::release_package_remote::RemoteEndpoint {
                    host: "server.example".into(),
                    port: 22,
                    username: "deploy".into(),
                },
                auth_type: "password".into(),
                vault_entry_id: None,
                private_key_path: String::new(),
                targets: vec![RemoteTarget::Frontend, RemoteTarget::Backend],
                frontend_remote_dir: "/srv/app/web".into(),
                backend_remote_path: "/srv/app/app.jar".into(),
            },
            expected_fingerprint: "SHA256:trusted".into(),
            secret: crate::tools::release_package_remote::AuthSecret::Password(
                zeroize::Zeroizing::new("secret".into()),
            ),
            expected_existing_targets,
        }
    }

    #[test]
    fn terminal_result_emits_status_and_one_package_notification() {
        let sink = TerminalSink::default();
        emit_terminal_result(
            &sink,
            "run-1",
            &project(),
            Ok(PipelineSummary {
                status: "succeeded",
                archive_path: Some(PathBuf::from("D:\\release\\target")),
                archived_targets: Vec::new(),
                manifests: Vec::new(),
                error: None,
                retry_descriptor: None,
                remote_committed: false,
            }),
            true,
        );
        let logs = sink.logs.lock().unwrap();
        assert_eq!(logs.len(), 2);
        assert!(logs.iter().any(|event| {
            event.phase == "frontend" && event.stream == "system" && event.line == "已完成打包"
        }));
        assert!(logs.iter().any(|event| {
            event.phase == "backend" && event.stream == "system" && event.line == "已完成打包"
        }));
        assert_eq!(sink.statuses.lock().unwrap().len(), 1);
        assert_eq!(sink.notifications.lock().unwrap().len(), 1);
    }

    #[test]
    fn cancelled_result_emits_status_and_one_notification() {
        let sink = TerminalSink::default();
        emit_terminal_result(
            &sink,
            "run-1",
            &project(),
            Err(PipelineError::Cancelled { phase: "overall" }),
            true,
        );
        assert_eq!(sink.statuses.lock().unwrap()[0].status, "cancelled");
        assert_eq!(sink.notifications.lock().unwrap().len(), 1);
    }

    #[test]
    fn upload_failure_keeps_source_manifests_without_archive_path() {
        let manifest = ArtifactManifest {
            target: ReleaseTarget::Backend,
            source_path: PathBuf::from(r"D:\build\app.jar"),
            entries: Vec::new(),
            file_count: 0,
            total_bytes: 0,
        };
        let summary = combine_package_and_deploy(
            PipelineSummary {
                status: "succeeded",
                archive_path: None,
                archived_targets: Vec::new(),
                manifests: vec![manifest.clone()],
                error: None,
                retry_descriptor: None,
                remote_committed: false,
            },
            Err(DeployError::failed("SFTP 传输中断")),
        );

        assert_eq!(summary.status, "package_succeeded_upload_failed");
        assert!(summary.archive_path.is_none());
        assert_eq!(summary.retry_descriptor.unwrap().manifests, vec![manifest]);
    }

    #[test]
    fn committed_cleanup_warning_preserves_success_without_retry() {
        let backup_path = "/srv/app/app.jar.__lazycat_backup_run-1";
        let summary = combine_package_and_deploy(
            PipelineSummary {
                status: "succeeded",
                archive_path: None,
                archived_targets: Vec::new(),
                manifests: Vec::new(),
                error: None,
                retry_descriptor: None,
                remote_committed: false,
            },
            Err(DeployError {
                message: "远端提交成功，但旧版本备份清理失败".into(),
                cancelled: false,
                committed: true,
                recovery_paths: vec![backup_path.into()],
            }),
        );

        assert_eq!(summary.status, "succeeded");
        assert!(summary.remote_committed);
        assert!(summary.retry_descriptor.is_none());
        let error = summary.error.unwrap();
        assert!(error.contains("旧版本备份清理失败"));
        assert!(error.contains(backup_path));
    }

    #[test]
    fn upload_failure_status_contains_a_session_retry_token() {
        let sink = TerminalSink::default();
        let package = PipelineSummary {
            status: "succeeded",
            archive_path: None,
            archived_targets: Vec::new(),
            manifests: vec![ArtifactManifest {
                target: ReleaseTarget::Backend,
                source_path: PathBuf::from(r"D:\build\app.jar"),
                entries: Vec::new(),
                file_count: 0,
                total_bytes: 0,
            }],
            error: None,
            retry_descriptor: None,
            remote_committed: false,
        };

        emit_terminal_result(
            &sink,
            "run-upload-failed",
            &project(),
            Ok(combine_package_and_deploy(
                package,
                Err(DeployError::failed("SFTP 传输中断")),
            )),
            true,
        );

        let statuses = sink.statuses.lock().unwrap();
        assert_eq!(statuses[0].status, "package_succeeded_upload_failed");
        assert!(statuses[0].retry_token.is_some());
    }

    #[test]
    fn retry_terminal_result_does_not_emit_package_completion_logs() {
        let sink = TerminalSink::default();
        emit_terminal_result(
            &sink,
            "retry-run",
            &project(),
            Ok(PipelineSummary {
                status: "succeeded",
                archive_path: None,
                archived_targets: Vec::new(),
                manifests: Vec::new(),
                error: None,
                retry_descriptor: None,
                remote_committed: true,
            }),
            false,
        );

        assert!(sink.logs.lock().unwrap().is_empty());
    }

    #[test]
    fn partial_package_result_is_not_eligible_for_upload() {
        let summary = PipelineSummary {
            status: "partially_succeeded",
            archive_path: Some(PathBuf::from(r"D:\release\portal")),
            archived_targets: Vec::new(),
            manifests: Vec::new(),
            error: Some("frontend failed".into()),
            retry_descriptor: None,
            remote_committed: false,
        };

        assert!(!package_can_upload(&summary));
    }

    #[test]
    fn upload_cancellation_has_no_archive_path() {
        let summary = combine_package_and_deploy(
            PipelineSummary {
                status: "succeeded",
                archive_path: None,
                archived_targets: Vec::new(),
                manifests: Vec::new(),
                error: None,
                retry_descriptor: None,
                remote_committed: false,
            },
            Err(DeployError::cancelled()),
        );
        let result = claim_pipeline_result(
            Ok(summary),
            &AtomicBool::new(true),
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            &Mutex::new(()),
        );

        assert!(matches!(
            result,
            Err(PipelineError::Cancelled { phase: "overall" })
        ));
    }

    #[test]
    fn start_rejects_remote_overwrite_not_confirmed_by_preflight() {
        let consumed = consumed_preflight_with_existing(vec![RemoteTarget::Frontend]);

        assert!(validate_remote_overwrite(&consumed, &[]).is_err());
        assert!(validate_remote_overwrite(&consumed, &[ReleaseTarget::Frontend]).is_ok());
        assert!(validate_remote_overwrite(
            &consumed_preflight_with_existing(Vec::new()),
            &[ReleaseTarget::Frontend]
        )
        .is_err());
    }

    #[test]
    fn pipeline_cancellation_reports_the_active_phase() {
        let result = run_build_pipeline(
            "run-phase",
            project(),
            vec![ReleaseTarget::Frontend],
            Arc::new(AtomicBool::new(true)),
            ProcessSlots::new(),
            Arc::new(Sink),
        );
        assert!(matches!(
            result,
            Err(PipelineError::Cancelled { phase: "overall" })
        ));
    }

    #[test]
    fn completed_run_rejects_late_cancellation() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let claim_lock = Arc::new(Mutex::new(()));
        {
            let _guard = claim_lock.lock().unwrap();
            finished.store(true, Ordering::Release);
        }
        let active = ActiveRun {
            run_id: "finished".into(),
            cancelled: cancelled.clone(),
            process_slots: ProcessSlots::new(),
            ssh_socket: Arc::new(Mutex::new(None)),
            finished,
            cancel_won: Arc::new(AtomicBool::new(false)),
            claim_lock,
        };
        assert!(!request_cancel(&active));
        assert!(!cancelled.load(Ordering::Acquire));
    }

    #[test]
    fn cancellation_closes_the_active_ssh_socket() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (_server, _) = listener.accept().unwrap();
        let ssh_socket = Arc::new(Mutex::new(Some(client)));
        let active = ActiveRun {
            run_id: "uploading".into(),
            cancelled: Arc::new(AtomicBool::new(false)),
            process_slots: ProcessSlots::new(),
            ssh_socket: ssh_socket.clone(),
            finished: Arc::new(AtomicBool::new(false)),
            cancel_won: Arc::new(AtomicBool::new(false)),
            claim_lock: Arc::new(Mutex::new(())),
        };

        assert!(request_cancel(&active));
        assert!(ssh_socket.lock().unwrap().is_none());
    }

    #[test]
    fn cancellation_before_terminal_claim_wins_with_archive_phase() {
        let cancelled = Arc::new(AtomicBool::new(true));
        let finished = Arc::new(AtomicBool::new(false));
        let cancel_won = Arc::new(AtomicBool::new(false));
        let result = claim_pipeline_result(
            Ok(PipelineSummary {
                status: "succeeded",
                archive_path: Some(PathBuf::from("archive")),
                archived_targets: Vec::new(),
                manifests: Vec::new(),
                error: None,
                retry_descriptor: None,
                remote_committed: false,
            }),
            &cancelled,
            &finished,
            &cancel_won,
            &Mutex::new(()),
        );
        assert!(matches!(
            result,
            Err(PipelineError::Cancelled { phase: "overall" })
        ));
        assert!(finished.load(Ordering::Acquire));
        assert!(cancel_won.load(Ordering::Acquire));
    }

    #[test]
    fn late_cancellation_does_not_override_a_committed_upload() {
        let cancelled = AtomicBool::new(true);
        let finished = AtomicBool::new(false);
        let cancel_won = AtomicBool::new(false);
        let package = PipelineSummary {
            status: "succeeded",
            archive_path: Some(PathBuf::from("archive")),
            archived_targets: vec![ArchivedTarget {
                target: ReleaseTarget::Backend,
                archive_entry_name: "app.jar".into(),
                artifact_mode: "file".into(),
            }],
            manifests: Vec::new(),
            error: None,
            retry_descriptor: None,
            remote_committed: false,
        };
        let result = claim_pipeline_result(
            Ok(combine_package_and_deploy(package, Ok(()))),
            &cancelled,
            &finished,
            &cancel_won,
            &Mutex::new(()),
        )
        .unwrap();

        assert_eq!(result.status, "succeeded");
        assert!(!cancel_won.load(Ordering::Acquire));
    }

    #[cfg(windows)]
    #[test]
    fn upload_summary_uses_built_sources_without_archive_path() {
        let root = TestDir::new();
        let source = root.0.join("dist");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("index.html"), "web").unwrap();
        let built = BuildSummary {
            status: "succeeded",
            built_targets: vec![BuiltTarget {
                target: ReleaseTarget::Frontend,
                source_path: source.clone(),
                artifact_mode: "zip_directory".into(),
            }],
            selected_count: 1,
            error: None,
        };

        let summary = build_upload_summary(built).unwrap();

        assert!(summary.archive_path.is_none());
        assert_eq!(summary.manifests[0].source_path, source);
        assert_eq!(summary.manifests[0].entries[0].relative_path, "index.html");
        assert!(!summary.manifests[0].entries[0]
            .relative_path
            .starts_with("dist/"));
    }

    #[cfg(windows)]
    #[test]
    fn retry_rejects_changed_live_artifacts() {
        let root = TestDir::new();
        let source = root.0.join("dist");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("index.html"), "v1").unwrap();
        let manifest = ArtifactManifest::from_directory(ReleaseTarget::Frontend, &source).unwrap();
        fs::write(source.join("index.html"), "changed-size").unwrap();
        let retry = RetryJob::from_manifests(7, vec![manifest]);
        let mut consumed = consumed_preflight_with_existing(Vec::new());
        consumed.binding.targets = vec![RemoteTarget::Frontend];

        let error = build_retry_deployment_request("retry", &retry, &consumed)
            .err()
            .unwrap();

        assert_eq!(error.message, "部署产物在打包后发生变化，请重新打包");
    }

    #[test]
    fn retry_tokens_are_consumed_once_and_bound_to_the_project() {
        let descriptor = RetryDescriptor {
            manifests: Vec::new(),
        };
        let token = issue_retry(7, descriptor).unwrap();

        let retry = consume_retry(&token, 7).unwrap();
        assert_eq!(retry.project_id, 7);
        assert!(consume_retry(&token, 7).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn build_targets_returns_sources_without_archiving() {
        let root = TestDir::new();
        let frontend_project = root.0.join("web");
        let backend_project = root.0.join("server");
        let output_root = root.0.join("output");
        fs::create_dir_all(&frontend_project).unwrap();
        fs::create_dir_all(&backend_project).unwrap();
        let project = ReleasePackageProjectConfig {
            id: 1,
            name: "构建项目".into(),
            output_root: output_root.to_string_lossy().into_owned(),
            package_type: ReleasePackageType::LocalArchive,
            frontend_project_path: frontend_project.to_string_lossy().into_owned(),
            frontend_build_command: "New-Item -ItemType Directory -Force dist | Out-Null; Set-Content dist/index.html web".into(),
            frontend_artifact_path: "dist".into(),
            frontend_artifact_mode: "copy_directory".into(),
            backend_project_path: backend_project.to_string_lossy().into_owned(),
            backend_build_command: "New-Item -ItemType Directory -Force target | Out-Null; Set-Content target/app.jar jar".into(),
            backend_artifact_path: "target/app.jar".into(),
            ssh_host: String::new(),
            ssh_port: 22,
            ssh_username: String::new(),
            ssh_auth_type: "password".into(),
            vault_entry_id: None,
            ssh_private_key_path: String::new(),
            frontend_remote_dir: String::new(),
            backend_remote_path: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };

        let summary = run_build_pipeline(
            "build-run",
            project,
            vec![ReleaseTarget::Frontend, ReleaseTarget::Backend],
            Arc::new(AtomicBool::new(false)),
            ProcessSlots::new(),
            Arc::new(CollectingSink::default()),
        )
        .unwrap();

        assert_eq!(summary.built_targets.len(), 2);
        assert!(summary.built_targets.iter().any(|target| {
            target.target == ReleaseTarget::Frontend
                && target.source_path == frontend_project.join("dist")
        }));
        assert!(summary.built_targets.iter().any(|target| {
            target.target == ReleaseTarget::Backend
                && target.source_path == backend_project.join("target/app.jar")
        }));
        assert!(!output_root.exists());
    }

    #[cfg(windows)]
    #[test]
    fn local_archive_failure_overrides_the_build_target_status() {
        let root = TestDir::new();
        let output_root = root.0.join("output");
        fs::create_dir(&output_root).unwrap();
        let project = frontend_build_project(&root.0, "invalid_mode");
        let sink = Arc::new(CollectingSink::default());
        let cancelled = Arc::new(AtomicBool::new(false));
        let build = run_build_pipeline(
            "archive-failed",
            project.clone(),
            vec![ReleaseTarget::Frontend],
            cancelled.clone(),
            ProcessSlots::new(),
            sink.clone(),
        )
        .unwrap();

        let summary = run_local_archive_pipeline(
            "archive-failed",
            project.id,
            build,
            output_root,
            "release".into(),
            false,
            cancelled,
            sink.clone(),
        )
        .unwrap();

        assert_eq!(summary.status, "failed");
        assert_eq!(sink.last_status("frontend").as_deref(), Some("failed"));
    }

    #[cfg(windows)]
    #[test]
    fn local_archive_setup_failure_preserves_the_build_error() {
        let root = TestDir::new();
        let backend = root.0.join("app.jar");
        fs::write(&backend, "jar").unwrap();
        let build = BuildSummary {
            status: "partially_succeeded",
            built_targets: vec![BuiltTarget {
                target: ReleaseTarget::Backend,
                source_path: backend,
                artifact_mode: "file".into(),
            }],
            selected_count: 2,
            error: Some("frontend：PowerShell 命令退出码：9".into()),
        };
        let sink = Arc::new(CollectingSink::default());

        let error = run_local_archive_pipeline(
            "archive-setup-failed",
            7,
            build,
            root.0.join("missing-output"),
            "release".into(),
            false,
            Arc::new(AtomicBool::new(false)),
            sink.clone(),
        )
        .unwrap_err();

        let PipelineError::Failed { message } = error else {
            panic!("expected archive failure");
        };
        assert!(message.contains("frontend：PowerShell 命令退出码：9"));
        assert!(message.contains("全局归档根目录不存在"));
        let backend_error = sink.last_error("backend").unwrap();
        assert!(backend_error.contains("全局归档根目录不存在"));
        assert!(!backend_error.contains("frontend：PowerShell 命令退出码：9"));
    }

    #[cfg(windows)]
    #[test]
    fn local_zip_archive_contains_the_frontend_directory() {
        let root = TestDir::new();
        let output_root = root.0.join("output");
        fs::create_dir(&output_root).unwrap();
        let project = frontend_build_project(&root.0, "zip_directory");
        let cancelled = Arc::new(AtomicBool::new(false));
        let build = run_build_pipeline(
            "zip-run",
            project.clone(),
            vec![ReleaseTarget::Frontend],
            cancelled.clone(),
            ProcessSlots::new(),
            Arc::new(Sink),
        )
        .unwrap();
        let summary = run_local_archive_pipeline(
            "zip-run",
            project.id,
            build,
            output_root,
            "release".into(),
            false,
            cancelled,
            Arc::new(Sink),
        )
        .unwrap();

        let archive_path = summary.archive_path.unwrap();
        let zip = fs::File::open(archive_path.join("dist.zip")).unwrap();
        let mut zip = zip::ZipArchive::new(zip).unwrap();
        assert!(zip.by_name("dist/index.html").is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn local_archive_overwrite_replaces_the_existing_directory() {
        let root = TestDir::new();
        let output_root = root.0.join("output");
        let existing = output_root.join("release");
        fs::create_dir_all(&existing).unwrap();
        fs::write(existing.join("stale.txt"), "stale").unwrap();
        let project = frontend_build_project(&root.0, "copy_directory");
        let cancelled = Arc::new(AtomicBool::new(false));
        let build = run_build_pipeline(
            "overwrite-run",
            project.clone(),
            vec![ReleaseTarget::Frontend],
            cancelled.clone(),
            ProcessSlots::new(),
            Arc::new(Sink),
        )
        .unwrap();
        let summary = run_local_archive_pipeline(
            "overwrite-run",
            project.id,
            build,
            output_root,
            "release".into(),
            true,
            cancelled,
            Arc::new(Sink),
        )
        .unwrap();

        let archive_path = summary.archive_path.unwrap();
        assert!(archive_path.join("dist/index.html").is_file());
        assert!(!archive_path.join("stale.txt").exists());
    }

    #[cfg(windows)]
    #[test]
    fn local_archive_cancellation_does_not_commit_and_marks_the_target_cancelled() {
        let root = TestDir::new();
        let output_root = root.0.join("output");
        fs::create_dir(&output_root).unwrap();
        let project = frontend_build_project(&root.0, "copy_directory");
        let cancelled = Arc::new(AtomicBool::new(false));
        let sink = Arc::new(CollectingSink::cancelling_during_archive(cancelled.clone()));
        let build = run_build_pipeline(
            "cancel-archive",
            project.clone(),
            vec![ReleaseTarget::Frontend],
            cancelled.clone(),
            ProcessSlots::new(),
            sink.clone(),
        )
        .unwrap();

        let result = run_local_archive_pipeline(
            "cancel-archive",
            project.id,
            build,
            output_root.clone(),
            "release".into(),
            false,
            cancelled,
            sink.clone(),
        );

        assert!(matches!(
            result,
            Err(PipelineError::Cancelled { phase: "overall" })
        ));
        assert!(!output_root.join("release").exists());
        assert_eq!(sink.last_status("frontend").as_deref(), Some("cancelled"));
    }

    #[cfg(windows)]
    #[test]
    fn pipeline_builds_frontend_then_backend_and_archives_both() {
        let root = TestDir::new();
        let frontend_project = root.0.join("web");
        let backend_project = root.0.join("server");
        let output_root = root.0.join("output");
        fs::create_dir_all(&frontend_project).unwrap();
        fs::create_dir_all(&backend_project).unwrap();
        fs::create_dir_all(&output_root).unwrap();
        let project = ReleasePackageProjectConfig {
            id: 1,
            name: "冒烟项目".into(),
            output_root: output_root.to_string_lossy().into_owned(),
            package_type: ReleasePackageType::LocalArchive,
            frontend_project_path: frontend_project.to_string_lossy().into_owned(),
            frontend_build_command: "New-Item -ItemType Directory -Force dist | Out-Null; Set-Content dist/index.html web".into(),
            frontend_artifact_path: "dist".into(),
            frontend_artifact_mode: "copy_directory".into(),
            backend_project_path: backend_project.to_string_lossy().into_owned(),
            backend_build_command: "New-Item -ItemType Directory -Force target | Out-Null; Set-Content target/app.jar jar".into(),
            backend_artifact_path: "target/app.jar".into(),
            ssh_host: String::new(),
            ssh_port: 22,
            ssh_username: String::new(),
            ssh_auth_type: "password".into(),
            vault_entry_id: None,
            ssh_private_key_path: String::new(),
            frontend_remote_dir: String::new(),
            backend_remote_path: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        let sink = Arc::new(CollectingSink::default());
        let build = run_build_pipeline(
            "smoke-run",
            project.clone(),
            vec![ReleaseTarget::Frontend, ReleaseTarget::Backend],
            Arc::new(AtomicBool::new(false)),
            ProcessSlots::new(),
            sink.clone(),
        )
        .unwrap();
        let result = run_local_archive_pipeline(
            "smoke-run",
            project.id,
            build,
            output_root,
            "20260723-冒烟项目".into(),
            false,
            Arc::new(AtomicBool::new(false)),
            sink.clone(),
        )
        .unwrap();
        assert_eq!(result.archived_targets.len(), 2);
        assert!(result.archived_targets.iter().any(|target| {
            target.target == ReleaseTarget::Frontend && target.archive_entry_name == "dist"
        }));
        assert!(result.archived_targets.iter().any(|target| {
            target.target == ReleaseTarget::Backend && target.archive_entry_name == "app.jar"
        }));
        let archive_path = result.archive_path.unwrap();
        assert!(archive_path.join("dist/index.html").is_file());
        assert!(archive_path.join("app.jar").is_file());
        let phases = sink.phases();
        assert!(phases.contains(&"frontend".into()));
        assert!(phases.contains(&"backend".into()));
    }

    #[cfg(windows)]
    #[test]
    fn failed_frontend_does_not_stop_backend_and_commits_backend_artifact() {
        let root = TestDir::new();
        let frontend_project = root.0.join("web");
        let backend_project = root.0.join("server");
        let output_root = root.0.join("output");
        fs::create_dir_all(&frontend_project).unwrap();
        fs::create_dir_all(&backend_project).unwrap();
        fs::create_dir_all(&output_root).unwrap();
        let project = ReleasePackageProjectConfig {
            id: 2,
            name: "冒烟项目".into(),
            output_root: output_root.to_string_lossy().into_owned(),
            package_type: ReleasePackageType::LocalArchive,
            frontend_project_path: frontend_project.to_string_lossy().into_owned(),
            frontend_build_command: "exit 9".into(),
            frontend_artifact_path: "dist".into(),
            frontend_artifact_mode: "copy_directory".into(),
            backend_project_path: backend_project.to_string_lossy().into_owned(),
            backend_build_command: "New-Item -ItemType Directory -Force target | Out-Null; Set-Content target/app.jar backend-ok".into(),
            backend_artifact_path: "target/app.jar".into(),
            ssh_host: String::new(),
            ssh_port: 22,
            ssh_username: String::new(),
            ssh_auth_type: "password".into(),
            vault_entry_id: None,
            ssh_private_key_path: String::new(),
            frontend_remote_dir: String::new(),
            backend_remote_path: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        let build = run_build_pipeline(
            "failed-run",
            project.clone(),
            vec![ReleaseTarget::Frontend, ReleaseTarget::Backend],
            Arc::new(AtomicBool::new(false)),
            ProcessSlots::new(),
            Arc::new(CollectingSink::default()),
        )
        .unwrap();
        let summary = run_local_archive_pipeline(
            "failed-run",
            project.id,
            build,
            output_root.clone(),
            "20260723-冒烟项目".into(),
            false,
            Arc::new(AtomicBool::new(false)),
            Arc::new(CollectingSink::default()),
        )
        .unwrap();
        assert_eq!(summary.status, "partially_succeeded");
        assert!(backend_project.join("target/app.jar").is_file());
        assert!(summary.archive_path.unwrap().join("app.jar").is_file());
    }
}

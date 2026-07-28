use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use tauri::Emitter;

use super::release_package::{ReleasePackageEnvironmentConfig, ReleaseTarget};
#[cfg(test)]
use super::release_package::{ReleasePackageEnvironmentKind, ReleasePackageType};
use super::release_package_archive::{
    archive_backend_artifact, archive_frontend_artifact, resolve_artifact_path,
    validate_artifact_target_collision, ArchiveError, ArchiveSession,
};
use super::release_package_deploy::{
    deploy_parallel, ArchivedTarget, ArtifactManifest, DeployError, DeploymentPlan,
    DeploymentRequest, DeploymentSuccess, DeploymentTarget, RemoteFs,
};
use super::release_package_remote::CommandRemoteFs;
use super::release_package_remote::RemoteEndpoint;
use super::release_package_remote::{
    consume_preflight, consume_preflight_after, ConsumedPreflight, PreflightBinding, RemoteTarget,
    SftpRemoteFs, SshSocketRegistry,
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
    Output(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CommandOutcome {
    success_keyword_matched: bool,
}

impl CommandError {
    fn message(&self) -> String {
        match self {
            Self::Cancelled => "构建已取消".into(),
            Self::ExitCode(code) => format!("PowerShell 命令退出码：{code}"),
            Self::Spawn(message) | Self::Wait(message) | Self::Output(message) => message.clone(),
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

fn read_console_output<R: BufRead>(
    reader: &mut R,
    stream: &'static str,
    emit: &dyn Fn(&'static str, String),
    success_keyword: Option<&str>,
    success_keyword_matched: &AtomicBool,
) -> Result<(), String> {
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
        match reader.read_until(b'\n', &mut bytes) {
            Ok(0) => return Ok(()),
            Ok(_) => {
                let line = decode_console_line(&bytes);
                if success_keyword.is_some_and(|keyword| line.contains(keyword)) {
                    success_keyword_matched.store(true, Ordering::Release);
                }
                emit(stream, line);
            }
            Err(error) => {
                return Err(format!("读取 PowerShell {stream} 输出失败：{error}"));
            }
        }
    }
}

fn spawn_reader<R>(
    reader: R,
    stream: &'static str,
    emit: Arc<dyn Fn(&'static str, String) + Send + Sync>,
    success_keyword: Option<Arc<String>>,
    success_keyword_matched: Arc<AtomicBool>,
) -> thread::JoinHandle<Result<(), String>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        read_console_output(
            &mut reader,
            stream,
            emit.as_ref(),
            success_keyword.as_deref().map(String::as_str),
            success_keyword_matched.as_ref(),
        )
    })
}

fn join_reader(
    handle: thread::JoinHandle<Result<(), String>>,
    stream: &'static str,
) -> Result<(), CommandError> {
    handle
        .join()
        .map_err(|_| CommandError::Output(format!("PowerShell {stream} 输出读取线程异常退出")))?
        .map_err(CommandError::Output)
}
#[cfg(windows)]
fn run_powershell(
    cwd: &Path,
    command: &str,
    success_keyword: Option<&str>,
    cancelled: Arc<AtomicBool>,
    pid_slot: Arc<Mutex<Option<u32>>>,
    emit: Arc<dyn Fn(&'static str, String) + Send + Sync>,
) -> Result<CommandOutcome, CommandError> {
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

    let success_keyword = success_keyword.map(|keyword| Arc::new(keyword.to_owned()));
    let success_keyword_matched = Arc::new(AtomicBool::new(success_keyword.is_none()));
    let stdout = spawn_reader(
        child.stdout.take().unwrap(),
        "stdout",
        emit.clone(),
        success_keyword.clone(),
        Arc::clone(&success_keyword_matched),
    );
    let stderr = spawn_reader(
        child.stderr.take().unwrap(),
        "stderr",
        emit,
        success_keyword,
        Arc::clone(&success_keyword_matched),
    );
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
    let stdout_result = join_reader(stdout, "stdout");
    let stderr_result = join_reader(stderr, "stderr");
    *pid_slot.lock().unwrap() = None;
    if cancelled.load(Ordering::Acquire) {
        return Err(CommandError::Cancelled);
    }
    stdout_result?;
    stderr_result?;
    if status.success() {
        Ok(CommandOutcome {
            success_keyword_matched: success_keyword_matched.load(Ordering::Acquire),
        })
    } else {
        Err(CommandError::ExitCode(status.code().unwrap_or(-1)))
    }
}

#[cfg(not(windows))]
fn run_powershell(
    _cwd: &Path,
    _command: &str,
    _success_keyword: Option<&str>,
    _cancelled: Arc<AtomicBool>,
    _pid_slot: Arc<Mutex<Option<u32>>>,
    _emit: Arc<dyn Fn(&'static str, String) + Send + Sync>,
) -> Result<CommandOutcome, CommandError> {
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
    upload_stop: Arc<AtomicBool>,
    process_slots: ProcessSlots,
    ssh_sockets: Arc<SshSocketRegistry>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    command_retry_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command_target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command_status: Option<String>,
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
        command_retry_token: None,
        command_target: None,
        command_status: None,
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
        command_retry_token: None,
        command_target: None,
        command_status: None,
    });
}

fn emit_command_status(
    sink: &dyn EventSink,
    run_id: &str,
    project_id: i64,
    target: ReleaseTarget,
    command_status: &str,
    error: Option<String>,
) {
    let command_target = match target {
        ReleaseTarget::Frontend => "frontend",
        ReleaseTarget::Backend => "backend",
    };
    sink.status(StatusEvent {
        run_id: run_id.into(),
        project_id,
        status: "running".into(),
        phase: "upload".into(),
        archive_path: None,
        error,
        uploaded_bytes: None,
        total_bytes: None,
        current_path: None,
        retry_token: None,
        command_retry_token: None,
        command_target: Some(command_target.into()),
        command_status: Some(command_status.into()),
    });
}
#[derive(Default)]
struct UploadProgressState {
    uploaded_bytes: u64,
    current_path: Option<String>,
    last_emitted_at: Option<Instant>,
}

fn should_emit_upload_progress(
    last_emitted_at: Option<Instant>,
    now: Instant,
    force: bool,
) -> bool {
    force
        || last_emitted_at
            .map(|last| now.duration_since(last) >= Duration::from_millis(100))
            .unwrap_or(true)
}

struct UploadProgressReporter {
    sink: Arc<dyn EventSink>,
    run_id: String,
    project_id: i64,
    total_bytes: u64,
    state: Mutex<UploadProgressState>,
}

impl UploadProgressReporter {
    fn new(
        sink: Arc<dyn EventSink>,
        run_id: impl Into<String>,
        project_id: i64,
        total_bytes: u64,
    ) -> Self {
        Self {
            sink,
            run_id: run_id.into(),
            project_id,
            total_bytes,
            state: Mutex::new(UploadProgressState::default()),
        }
    }

    fn report(&self, bytes: u64, path: &str) {
        self.report_at(bytes, path, Instant::now());
    }

    fn report_at(&self, bytes: u64, path: &str, now: Instant) {
        let mut state = self.state.lock().unwrap();
        state.uploaded_bytes = state.uploaded_bytes.saturating_add(bytes);
        state.current_path = Some(path.to_owned());
        if !should_emit_upload_progress(state.last_emitted_at, now, false) {
            return;
        }
        state.last_emitted_at = Some(now);
        emit_upload_status(
            self.sink.as_ref(),
            &self.run_id,
            self.project_id,
            state.uploaded_bytes,
            self.total_bytes,
            state.current_path.clone(),
        );
    }

    fn force_emit(&self, success: bool) {
        self.force_emit_at(Instant::now(), success);
    }

    fn force_emit_at(&self, now: Instant, success: bool) {
        let mut state = self.state.lock().unwrap();
        if success {
            debug_assert_eq!(state.uploaded_bytes, self.total_bytes);
            state.uploaded_bytes = self.total_bytes;
        }
        state.last_emitted_at = Some(now);
        emit_upload_status(
            self.sink.as_ref(),
            &self.run_id,
            self.project_id,
            state.uploaded_bytes,
            self.total_bytes,
            state.current_path.clone(),
        );
    }

    #[cfg(test)]
    fn uploaded_bytes(&self) -> u64 {
        self.state.lock().unwrap().uploaded_bytes
    }
}

fn emit_terminal_result(
    sink: &dyn EventSink,
    run_id: &str,
    project: &ReleasePackageEnvironmentConfig,
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
    #[cfg(not(test))]
    if let Err(error) = crate::tools::action_center::finish_release_package_run(run_id, status) {
        eprintln!("action-center terminal update failed for run {run_id}: {error}");
    }
    if emit_package_logs
        && archive_path.is_some()
        && matches!(status, "succeeded" | "partially_succeeded")
    {
        for phase in ["frontend", "backend"] {
            emit_system_log(sink, run_id, project.project_id, phase, "已完成打包");
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
    let command_retry_token = if status == "upload_succeeded_command_failed" {
        take_command_retry_token(run_id)
    } else {
        None
    };
    sink.status(StatusEvent {
        run_id: run_id.into(),
        project_id: project.project_id,
        status: status.into(),
        phase: "overall".into(),
        archive_path: archive_path.clone(),
        error: error.clone(),
        uploaded_bytes: None,
        total_bytes: None,
        current_path: None,
        retry_token,
        command_retry_token,
        command_target: None,
        command_status: None,
    });
    if let Some(notification) = build_release_package_notification(
        run_id,
        project.project_id,
        &project.project_name,
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
    success_keyword: Option<&str>,
    cancelled: Arc<AtomicBool>,
    pid: Arc<Mutex<Option<u32>>>,
    sink: Arc<dyn EventSink>,
) -> Result<CommandOutcome, PipelineError> {
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
        success_keyword,
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

fn target_label(target: ReleaseTarget) -> &'static str {
    match target {
        ReleaseTarget::Frontend => "\u{524d}\u{7aef}",
        ReleaseTarget::Backend => "\u{540e}\u{7aef}",
    }
}

fn archive_pipeline_error(error: ArchiveError, phase: &'static str) -> PipelineError {
    match error {
        ArchiveError::Cancelled => PipelineError::Cancelled { phase },
        ArchiveError::Failed(message) => PipelineError::Failed { message },
        ArchiveError::CommittedWithWarning { warning, .. } => {
            // 该变体只由 ArchiveSession::commit 产生，并在本地归档提交点单独消费。
            // 此处保留防御性降级，避免未来新增调用方静默丢失警告。
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
    project: &ReleasePackageEnvironmentConfig,
    cancelled: Arc<AtomicBool>,
    pid: Arc<Mutex<Option<u32>>>,
    sink: Arc<dyn EventSink>,
) -> Result<BuiltTarget, PipelineError> {
    let phase = target_phase(target);
    let (project_path, command, success_keyword, artifact_path) = match target {
        ReleaseTarget::Frontend => (
            PathBuf::from(&project.frontend_project_path),
            project.frontend_build_command.as_str(),
            project.frontend_success_keyword.trim(),
            project.frontend_artifact_path.as_str(),
        ),
        ReleaseTarget::Backend => (
            PathBuf::from(&project.backend_project_path),
            project.backend_build_command.as_str(),
            project.backend_success_keyword.trim(),
            project.backend_artifact_path.as_str(),
        ),
    };
    let outcome = run_command_phase(
        run_id,
        project.project_id,
        phase,
        &project_path,
        command,
        (!success_keyword.is_empty()).then_some(success_keyword),
        cancelled.clone(),
        pid,
        sink.clone(),
    )?;
    if !outcome.success_keyword_matched {
        return Err(PipelineError::Failed {
            message: format!(
                "{}\u{6784}\u{5efa}\u{547d}\u{4ee4}\u{9000}\u{51fa}\u{6210}\u{529f}\u{ff0c}\u{4f46}\u{65e5}\u{5fd7}\u{672a}\u{5339}\u{914d}\u{6210}\u{529f}\u{5173}\u{952e}\u{5b57}\u{ff1a}{}",
                target_label(target),
                success_keyword
            ),
        });
    }
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommandSnapshot {
    target: ReleaseTarget,
    command: String,
}

impl CommandSnapshot {
    fn new(target: ReleaseTarget, command: impl Into<String>) -> Self {
        Self {
            target,
            command: command.into(),
        }
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
    local_committed: bool,
    failed_commands: Vec<CommandSnapshot>,
}

#[derive(Clone, Debug)]
struct RetryDescriptor {
    manifests: Vec<ArtifactManifest>,
    commands: Vec<CommandSnapshot>,
}

#[derive(Clone, Debug)]
struct RetryJob {
    environment_id: i64,
    descriptor: RetryDescriptor,
}

impl RetryJob {
    fn from_manifests(environment_id: i64, manifests: Vec<ArtifactManifest>) -> Self {
        Self {
            environment_id,
            descriptor: RetryDescriptor {
                manifests,
                commands: Vec::new(),
            },
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommandAuthBinding {
    pub(crate) environment_id: i64,
    pub(crate) endpoint: RemoteEndpoint,
    pub(crate) auth_type: String,
    pub(crate) vault_entry_id: Option<i64>,
    pub(crate) private_key_path: String,
    pub(crate) fingerprint_sha256: String,
}

impl CommandAuthBinding {
    fn from_preflight(binding: &PreflightBinding, fingerprint_sha256: &str) -> Self {
        Self {
            environment_id: binding.environment_id,
            endpoint: binding.endpoint.clone(),
            auth_type: binding.auth_type.clone(),
            vault_entry_id: binding.vault_entry_id,
            private_key_path: binding.private_key_path.clone(),
            fingerprint_sha256: fingerprint_sha256.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
struct CommandRetryJob {
    environment_id: i64,
    binding: CommandAuthBinding,
    failed_commands: Vec<CommandSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedCommandRetry {
    pub(crate) targets: Vec<ReleaseTarget>,
    pub(crate) binding: CommandAuthBinding,
    failed_commands: Vec<CommandSnapshot>,
}

static COMMAND_RETRIES: OnceLock<Mutex<HashMap<String, CommandRetryJob>>> = OnceLock::new();

fn command_retries() -> &'static Mutex<HashMap<String, CommandRetryJob>> {
    COMMAND_RETRIES.get_or_init(|| Mutex::new(HashMap::new()))
}

static COMMAND_RETRY_TOKENS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn command_retry_tokens() -> &'static Mutex<HashMap<String, String>> {
    COMMAND_RETRY_TOKENS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn remember_command_retry_token(run_id: &str, token: String) -> Result<(), String> {
    let mut tokens = match command_retry_tokens().lock() {
        Ok(tokens) => tokens,
        Err(_) => {
            if let Ok(mut retries) = command_retries().lock() {
                retries.remove(&token);
            }
            return Err("命令重试令牌登记失败".into());
        }
    };
    tokens.insert(run_id.to_string(), token);
    Ok(())
}

fn append_pipeline_error(summary: &mut PipelineSummary, message: String) {
    summary.error = Some(match summary.error.take() {
        Some(existing) if !existing.is_empty() => format!("{existing}；{message}"),
        _ => message,
    });
}

fn take_command_retry_token(run_id: &str) -> Option<String> {
    command_retry_tokens()
        .lock()
        .ok()
        .and_then(|mut tokens| tokens.remove(run_id))
}

fn validate_failed_commands(commands: &[CommandSnapshot]) -> Result<(), String> {
    if commands.is_empty() {
        return Err("没有可重试的失败上传后命令".into());
    }
    if commands
        .iter()
        .any(|snapshot| snapshot.command.trim().is_empty())
        || commands.iter().enumerate().any(|(index, snapshot)| {
            commands[..index]
                .iter()
                .any(|other| other.target == snapshot.target)
        })
    {
        return Err("失败上传后命令快照无效".into());
    }
    Ok(())
}

fn issue_command_retry(
    environment_id: i64,
    binding: CommandAuthBinding,
    failed_commands: Vec<CommandSnapshot>,
) -> Result<String, String> {
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return Err("应用正在退出，不能创建命令重试任务".into());
    }
    validate_failed_commands(&failed_commands)?;
    let token = uuid::Uuid::new_v4().to_string();
    command_retries()
        .lock()
        .map_err(|_| "命令重试任务存储不可用".to_string())?
        .insert(
            token.clone(),
            CommandRetryJob {
                environment_id,
                binding,
                failed_commands,
            },
        );
    Ok(token)
}

pub(crate) fn prepare_command_retry(
    token: &str,
    environment_id: i64,
) -> Result<PreparedCommandRetry, String> {
    let retries = command_retries()
        .lock()
        .map_err(|_| "命令重试任务存储不可用".to_string())?;
    let retry = retries
        .get(token)
        .ok_or_else(|| "命令重试令牌无效或已使用".to_string())?;
    if retry.environment_id != environment_id {
        return Err("命令重试令牌与当前环境不匹配".into());
    }
    Ok(PreparedCommandRetry {
        targets: retry
            .failed_commands
            .iter()
            .map(|command| command.target)
            .collect(),
        binding: retry.binding.clone(),
        failed_commands: retry.failed_commands.clone(),
    })
}

fn consume_command_retry(token: &str, environment_id: i64) -> Result<CommandRetryJob, String> {
    let mut retries = command_retries()
        .lock()
        .map_err(|_| "命令重试任务存储不可用".to_string())?;
    let retry = retries
        .get(token)
        .ok_or_else(|| "命令重试令牌无效或已使用".to_string())?;
    if retry.environment_id != environment_id {
        return Err("命令重试令牌与当前环境不匹配".into());
    }
    retries
        .remove(token)
        .ok_or_else(|| "命令重试令牌无效或已使用".to_string())
}

fn finish_command_retry(
    job: CommandRetryJob,
    failed_commands: Vec<CommandSnapshot>,
) -> Result<String, String> {
    issue_command_retry(job.environment_id, job.binding, failed_commands)
}

static RETRY_JOBS: OnceLock<Mutex<HashMap<String, RetryJob>>> = OnceLock::new();

fn retry_jobs() -> &'static Mutex<HashMap<String, RetryJob>> {
    RETRY_JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn issue_retry(environment_id: i64, descriptor: RetryDescriptor) -> Result<String, String> {
    let token = uuid::Uuid::new_v4().to_string();
    retry_jobs()
        .lock()
        .map_err(|_| "上传重试任务存储不可用".to_string())?
        .insert(
            token.clone(),
            RetryJob {
                environment_id,
                descriptor,
            },
        );
    Ok(token)
}

fn consume_retry(token: &str, environment_id: i64) -> Result<RetryJob, String> {
    let mut retries = retry_jobs()
        .lock()
        .map_err(|_| "上传重试任务存储不可用".to_string())?;
    let retry = retries
        .get(token)
        .ok_or_else(|| "上传重试令牌无效或已使用".to_string())?;
    if retry.environment_id != environment_id {
        return Err("上传重试令牌与当前环境不匹配".into());
    }
    retries
        .remove(token)
        .ok_or_else(|| "上传重试令牌无效或已使用".to_string())
}

pub(crate) fn retry_targets(
    token: &str,
    environment_id: i64,
) -> Result<Vec<ReleaseTarget>, String> {
    let retries = retry_jobs()
        .lock()
        .map_err(|_| "上传重试任务存储不可用".to_string())?;
    let retry = retries
        .get(token)
        .filter(|retry| retry.environment_id == environment_id)
        .ok_or_else(|| "上传重试令牌无效或与当前环境不匹配".to_string())?;
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
        local_committed: false,
        failed_commands: Vec::new(),
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
                commands: Vec::new(),
            });
            summary.remote_committed = false;
            summary
        }
    }
}

fn resolve_deployment_result(
    summary: PipelineSummary,
    deploy_result: Result<DeploymentSuccess, DeployError>,
) -> (PipelineSummary, Option<Box<dyn RemoteFs>>) {
    match deploy_result {
        Ok(success) => {
            let deploy_result = match success.warning {
                Some(warning) => Err(warning),
                None => Ok(()),
            };
            (
                combine_package_and_deploy(summary, deploy_result),
                Some(success.control),
            )
        }
        Err(error) => (combine_package_and_deploy(summary, Err(error)), None),
    }
}

fn preserve_retry_commands(
    mut summary: PipelineSummary,
    commands: Vec<CommandSnapshot>,
) -> PipelineSummary {
    if summary.status == "package_succeeded_upload_failed" {
        if let Some(descriptor) = summary.retry_descriptor.as_mut() {
            descriptor.commands = commands;
        }
    }
    summary
}

fn configured_post_upload_commands(
    project: &ReleasePackageEnvironmentConfig,
    manifests: &[ArtifactManifest],
) -> Vec<CommandSnapshot> {
    [ReleaseTarget::Frontend, ReleaseTarget::Backend]
        .into_iter()
        .filter(|target| manifests.iter().any(|manifest| manifest.target == *target))
        .filter_map(|target| {
            let command = match target {
                ReleaseTarget::Frontend => project.frontend_post_upload_command.trim(),
                ReleaseTarget::Backend => project.backend_post_upload_command.trim(),
            };
            (!command.is_empty()).then(|| CommandSnapshot::new(target, command))
        })
        .collect()
}

fn emit_cancelled_command_statuses(
    sink: &dyn EventSink,
    run_id: &str,
    project_id: i64,
    commands: &[CommandSnapshot],
    message: &str,
) {
    for snapshot in commands {
        emit_command_status(
            sink,
            run_id,
            project_id,
            snapshot.target,
            "cancelled",
            Some(message.into()),
        );
    }
}

fn run_post_upload_commands(
    run_id: &str,
    project_id: i64,
    mut summary: PipelineSummary,
    mut commands: Vec<CommandSnapshot>,
    mut control: Box<dyn RemoteFs>,
    cancelled: Arc<AtomicBool>,
    sink: Arc<dyn EventSink>,
) -> PipelineSummary {
    if !summary.remote_committed {
        return summary;
    }

    for manifest in &summary.manifests {
        if !commands
            .iter()
            .any(|command| command.target == manifest.target)
        {
            emit_command_status(
                sink.as_ref(),
                run_id,
                project_id,
                manifest.target,
                "skipped",
                None,
            );
        }
    }
    if commands.is_empty() {
        return summary;
    }

    commands.sort_by_key(|snapshot| match snapshot.target {
        ReleaseTarget::Frontend => 0,
        ReleaseTarget::Backend => 1,
    });
    let mut failed = Vec::new();

    for (index, snapshot) in commands.iter().enumerate() {
        if cancelled.load(Ordering::Acquire) {
            let message = "服务器文件已上传，上传后命令未全部完成，已按用户请求取消";
            emit_cancelled_command_statuses(
                sink.as_ref(),
                run_id,
                project_id,
                &commands[index..],
                message,
            );
            summary.status = "cancelled";
            summary.error = Some(message.into());
            summary.failed_commands.clear();
            summary.retry_descriptor = None;
            return summary;
        }

        let label = target_label(snapshot.target);
        emit_command_status(
            sink.as_ref(),
            run_id,
            project_id,
            snapshot.target,
            "running",
            None,
        );
        emit_system_log(
            sink.as_ref(),
            run_id,
            project_id,
            "upload",
            &format!("[{label}命令] 开始执行上传后命令"),
        );
        let event_sink = Arc::clone(&sink);
        let event_run_id = run_id.to_owned();
        let prefix = format!("[{label}命令]");
        let result = control.execute_command(
            &snapshot.command,
            cancelled.as_ref(),
            &mut move |stream, line| {
                event_sink.log(LogEvent {
                    run_id: event_run_id.clone(),
                    project_id,
                    phase: "upload".into(),
                    stream: stream.into(),
                    line: format!("{prefix}[{stream}] {line}"),
                });
            },
        );

        if cancelled.load(Ordering::Acquire) || matches!(&result, Err(error) if error.cancelled) {
            let message = "服务器文件已上传，上传后命令未全部完成，已按用户请求取消";
            emit_cancelled_command_statuses(
                sink.as_ref(),
                run_id,
                project_id,
                &commands[index..],
                message,
            );
            summary.status = "cancelled";
            summary.error = Some(message.into());
            summary.failed_commands.clear();
            summary.retry_descriptor = None;
            return summary;
        }

        match result {
            Ok(result) if result.exit_code == 0 => {
                emit_system_log(
                    sink.as_ref(),
                    run_id,
                    project_id,
                    "upload",
                    &format!("[{label}命令] 上传后命令执行成功"),
                );
                emit_command_status(
                    sink.as_ref(),
                    run_id,
                    project_id,
                    snapshot.target,
                    "succeeded",
                    None,
                );
            }
            Ok(result) => {
                let error = format!("上传后命令执行失败，退出码：{}", result.exit_code);
                emit_system_log(
                    sink.as_ref(),
                    run_id,
                    project_id,
                    "upload",
                    &format!("[{label}命令] {error}"),
                );
                emit_command_status(
                    sink.as_ref(),
                    run_id,
                    project_id,
                    snapshot.target,
                    "failed",
                    Some(error),
                );
                failed.push(snapshot.clone());
            }
            Err(error) => {
                let message = format!("上传后命令执行失败：{}", error.message);
                emit_system_log(
                    sink.as_ref(),
                    run_id,
                    project_id,
                    "upload",
                    &format!("[{label}命令] {message}"),
                );
                emit_command_status(
                    sink.as_ref(),
                    run_id,
                    project_id,
                    snapshot.target,
                    "failed",
                    Some(message),
                );
                failed.push(snapshot.clone());
            }
        }
    }

    if !failed.is_empty() {
        summary.status = "upload_succeeded_command_failed";
        let command_error = "服务器文件已上传，但上传后命令未全部成功";
        summary.error = Some(match summary.error.take() {
            Some(existing) => format!("{existing}；{command_error}"),
            None => command_error.into(),
        });
        summary.retry_descriptor = None;
        summary.failed_commands = failed;
    }
    summary
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
    project: &ReleasePackageEnvironmentConfig,
    summary: PipelineSummary,
    authorization: DeployAuthorization,
    cancelled: Arc<AtomicBool>,
    upload_stop: Arc<AtomicBool>,
    ssh_sockets: &Arc<SshSocketRegistry>,
    sink: Arc<dyn EventSink>,
) -> PipelineSummary {
    if !package_can_upload(&summary) {
        return summary;
    }
    let commands = configured_post_upload_commands(project, &summary.manifests);
    let request = match build_deployment_request(run_id, &summary, &authorization.consumed) {
        Ok(request) => request,
        Err(error) => {
            return preserve_retry_commands(
                combine_package_and_deploy(summary, Err(error)),
                commands,
            );
        }
    };
    execute_deployment_request(
        run_id,
        project.project_id,
        summary,
        request,
        commands,
        authorization.consumed,
        cancelled,
        upload_stop,
        ssh_sockets,
        sink,
    )
}

fn execute_deployment_request(
    run_id: &str,
    project_id: i64,
    summary: PipelineSummary,
    request: DeploymentRequest,
    commands: Vec<CommandSnapshot>,
    consumed: ConsumedPreflight,
    cancelled: Arc<AtomicBool>,
    upload_stop: Arc<AtomicBool>,
    ssh_sockets: &Arc<SshSocketRegistry>,
    sink: Arc<dyn EventSink>,
) -> PipelineSummary {
    let total_bytes = request
        .targets
        .iter()
        .map(|target| target.manifest.total_bytes)
        .sum();
    emit_system_log(
        sink.as_ref(),
        run_id,
        project_id,
        "upload",
        "开始上传服务器",
    );
    let reporter = Arc::new(UploadProgressReporter::new(
        Arc::clone(&sink),
        run_id,
        project_id,
        total_bytes,
    ));
    reporter.force_emit(false);
    let progress = {
        let reporter = Arc::clone(&reporter);
        Arc::new(move |bytes: u64, path: &str| reporter.report(bytes, path))
            as Arc<dyn Fn(u64, &str) + Send + Sync>
    };
    let binding = Arc::new(consumed.binding);
    let expected_fingerprint = Arc::new(consumed.expected_fingerprint);
    let secret = Arc::new(Mutex::new(consumed.secret));
    let connect_remote = {
        let binding = Arc::clone(&binding);
        let expected_fingerprint = Arc::clone(&expected_fingerprint);
        let secret = Arc::clone(&secret);
        let ssh_sockets = Arc::clone(ssh_sockets);
        Arc::new(move || -> Result<Box<dyn RemoteFs>, DeployError> {
            let secret = secret
                .lock()
                .map_err(|_| DeployError::failed("SSH 认证状态不可用"))?;
            Ok(Box::new(SftpRemoteFs::connect(
                binding.as_ref(),
                expected_fingerprint.as_ref(),
                &secret,
                ssh_sockets.as_ref(),
            )?))
        })
    };
    let deploy_result = (|| -> Result<DeploymentSuccess, DeployError> {
        let plan = DeploymentPlan::new(request)?;
        let mut remotes = Vec::with_capacity(plan.target_count());
        for _ in 0..plan.target_count() {
            remotes.push(connect_remote()?);
        }
        let interrupt_transport = {
            let ssh_sockets = Arc::clone(ssh_sockets);
            Arc::new(move || ssh_sockets.shutdown_all()) as Arc<dyn Fn() + Send + Sync>
        };
        let recover_remote = {
            let connect_remote = Arc::clone(&connect_remote);
            let ssh_sockets = Arc::clone(ssh_sockets);
            Arc::new(move || {
                ssh_sockets.reset_after_shutdown();
                connect_remote()
            }) as Arc<dyn Fn() -> Result<Box<dyn RemoteFs>, DeployError> + Send + Sync>
        };
        deploy_parallel(
            remotes,
            plan,
            Arc::clone(&cancelled),
            upload_stop,
            progress,
            interrupt_transport,
            recover_remote,
        )
    })();
    reporter.force_emit(deploy_result.is_ok());
    let retry_commands = commands.clone();
    let (mut summary, control) = resolve_deployment_result(summary, deploy_result);
    summary = preserve_retry_commands(summary, retry_commands);
    if summary.remote_committed {
        emit_system_log(
            sink.as_ref(),
            run_id,
            project_id,
            "upload",
            "服务器上传完成",
        );
        if let Some(control) = control {
            summary = run_post_upload_commands(
                run_id,
                project_id,
                summary,
                commands,
                control,
                Arc::clone(&cancelled),
                Arc::clone(&sink),
            );
        }
    }
    if summary.status == "upload_succeeded_command_failed" && !cancelled.load(Ordering::Acquire) {
        if let Err(error) = issue_command_retry(
            project_id,
            CommandAuthBinding::from_preflight(binding.as_ref(), expected_fingerprint.as_ref()),
            summary.failed_commands.clone(),
        )
        .and_then(|token| remember_command_retry_token(run_id, token))
        {
            append_pipeline_error(&mut summary, format!("创建命令重试任务失败：{error}"));
        }
    }
    ssh_sockets.clear();
    summary
}

fn run_retry_deployment_phase(
    run_id: &str,
    project_id: i64,
    retry: RetryJob,
    authorization: DeployAuthorization,
    cancelled: Arc<AtomicBool>,
    upload_stop: Arc<AtomicBool>,
    ssh_sockets: &Arc<SshSocketRegistry>,
    sink: Arc<dyn EventSink>,
) -> PipelineSummary {
    let commands = retry.descriptor.commands.clone();
    let summary = PipelineSummary {
        status: "succeeded",
        archive_path: None,
        archived_targets: Vec::new(),
        manifests: retry.descriptor.manifests.clone(),
        error: None,
        retry_descriptor: None,
        remote_committed: false,
        local_committed: false,
        failed_commands: Vec::new(),
    };
    let request = match build_retry_deployment_request(run_id, &retry, &authorization.consumed) {
        Ok(request) => request,
        Err(error) => {
            return preserve_retry_commands(
                combine_package_and_deploy(summary, Err(error)),
                commands,
            );
        }
    };
    execute_deployment_request(
        run_id,
        project_id,
        summary,
        request,
        commands,
        authorization.consumed,
        cancelled,
        upload_stop,
        ssh_sockets,
        sink,
    )
}

fn run_build_pipeline(
    run_id: &str,
    project: ReleasePackageEnvironmentConfig,
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
                    thread_project.project_id,
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
    run_local_archive_pipeline_with_commit(
        run_id,
        project_id,
        summary,
        output_root,
        folder_name,
        overwrite_existing,
        cancelled,
        sink,
        ArchiveSession::commit,
    )
}

fn run_local_archive_pipeline_with_commit<C>(
    run_id: &str,
    project_id: i64,
    summary: BuildSummary,
    output_root: PathBuf,
    folder_name: String,
    overwrite_existing: bool,
    cancelled: Arc<AtomicBool>,
    sink: Arc<dyn EventSink>,
    commit_archive: C,
) -> Result<PipelineSummary, PipelineError>
where
    C: FnOnce(&mut ArchiveSession, &AtomicBool) -> Result<PathBuf, ArchiveError>,
{
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
            local_committed: false,
            failed_commands: Vec::new(),
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
            local_committed: false,
            failed_commands: Vec::new(),
        });
    }
    let (archive_path, cleanup_warning) = match commit_archive(&mut archive, cancelled.as_ref()) {
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
        local_committed: true,
        failed_commands: Vec::new(),
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
    let committed =
        matches!(&result, Ok(summary) if summary.remote_committed || summary.local_committed);
    let archived_cancellation = matches!(
        &result,
        Ok(summary) if summary.status == "cancelled" && summary.archive_path.is_some()
    );
    let result = if cancelled.load(Ordering::Acquire) && !committed {
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
    active.upload_stop.store(true, Ordering::Release);
    if active.finished.load(Ordering::Acquire) {
        if active.cancel_won.load(Ordering::Acquire) {
            return true;
        }
        active.cancelled.store(false, Ordering::Release);
        active.upload_stop.store(false, Ordering::Release);
        return false;
    }
    active.process_slots.terminate_all();
    active.ssh_sockets.shutdown_all();
    true
}

pub fn start(
    app: &tauri::AppHandle,
    project: ReleasePackageEnvironmentConfig,
    targets: Vec<ReleaseTarget>,
    request: RuntimeStartRequest,
    action_dispatch_id: Option<String>,
) -> Result<Value, String> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let cancelled = Arc::new(AtomicBool::new(false));
    let upload_stop = Arc::new(AtomicBool::new(false));
    let process_slots = ProcessSlots::new();
    let ssh_sockets = Arc::new(SshSocketRegistry::new());
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
            upload_stop: upload_stop.clone(),
            process_slots: process_slots.clone(),
            ssh_sockets: ssh_sockets.clone(),
            finished: finished.clone(),
            cancel_won: cancel_won.clone(),
            claim_lock: claim_lock.clone(),
        });
    }

    if let Some(dispatch_id) = action_dispatch_id.as_deref() {
        if let Err(error) = crate::tools::action_center::associate_release_package_run(
            dispatch_id,
            &run_id,
            project.id,
        ) {
            if let Ok(mut active) = active_run().lock() {
                if active.as_ref().map(|run| run.run_id.as_str()) == Some(run_id.as_str()) {
                    *active = None;
                }
            }
            return Err(error);
        }
    }

    let thread_run_id = run_id.clone();
    let project_id = project.project_id;
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
                    cancelled.clone(),
                    upload_stop.clone(),
                    &ssh_sockets,
                    sink.clone(),
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
    project: ReleasePackageEnvironmentConfig,
    retry_token: &str,
    deploy_authorization: DeployAuthorization,
) -> Result<Value, String> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let cancelled = Arc::new(AtomicBool::new(false));
    let upload_stop = Arc::new(AtomicBool::new(false));
    let process_slots = ProcessSlots::new();
    let ssh_sockets = Arc::new(SshSocketRegistry::new());
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
            upload_stop: upload_stop.clone(),
            process_slots,
            ssh_sockets: ssh_sockets.clone(),
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
            project.project_id,
            "running",
            "overall",
            None,
            None,
        );
        let summary = run_retry_deployment_phase(
            &thread_run_id,
            project.project_id,
            retry,
            deploy_authorization,
            cancelled.clone(),
            upload_stop.clone(),
            &ssh_sockets,
            sink.clone(),
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

pub fn command_retry(
    app: &tauri::AppHandle,
    project: ReleasePackageEnvironmentConfig,
    retry_token: &str,
    auth_token: &str,
    auth_binding: PreflightBinding,
) -> Result<Value, String> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let project_id = project.project_id;
    let environment_id = project.id;
    let cancelled = Arc::new(AtomicBool::new(false));
    let upload_stop = Arc::new(AtomicBool::new(false));
    let ssh_sockets = Arc::new(SshSocketRegistry::new());
    let finished = Arc::new(AtomicBool::new(false));
    let cancel_won = Arc::new(AtomicBool::new(false));
    let claim_lock = Arc::new(Mutex::new(()));
    let process_slots = ProcessSlots::new();
    let prepared = prepare_command_retry(retry_token, environment_id)?;

    if auth_binding.environment_id != environment_id
        || auth_binding.project_id != project_id
        || auth_binding.command_retry_token.as_deref() != Some(retry_token)
        || auth_binding.endpoint != prepared.binding.endpoint
        || auth_binding.auth_type != prepared.binding.auth_type
        || auth_binding.vault_entry_id != prepared.binding.vault_entry_id
        || auth_binding.private_key_path != prepared.binding.private_key_path
    {
        return Err("命令重试认证令牌与失败任务不匹配".into());
    }

    let mut active = active_run()
        .lock()
        .map_err(|_| "release package runtime lock poisoned")?;
    if SHUTTING_DOWN.load(Ordering::Acquire) {
        return Err("应用正在退出，不能启动命令重试".into());
    }
    if active.is_some() {
        return Err("已有发布打包或上传任务正在运行".into());
    }

    let (start_tx, start_rx) =
        std::sync::mpsc::sync_channel::<(ConsumedPreflight, CommandRetryJob)>(1);
    let thread_run_id = run_id.clone();
    let thread_project = project.clone();
    let sink: Arc<dyn EventSink> = Arc::new(TauriEventSink { app: app.clone() });
    let worker_ssh_sockets = ssh_sockets.clone();
    let worker_cancelled = cancelled.clone();
    let worker_finished = finished.clone();
    let worker_cancel_won = cancel_won.clone();
    let worker_claim_lock = claim_lock.clone();
    thread::Builder::new()
        .name("release-package-command-retry".into())
        .spawn(move || {
            let Ok((authorization, retry)) = start_rx.recv() else {
                return;
            };
            emit_status(
                sink.as_ref(),
                &thread_run_id,
                thread_project.project_id,
                "running",
                "overall",
                None,
                None,
            );
            let commands = retry.failed_commands.clone();
            let remote = CommandRemoteFs::connect(
                &authorization.binding.endpoint,
                &authorization.binding.private_key_path,
                &authorization.expected_fingerprint,
                &authorization.secret,
                worker_ssh_sockets.as_ref(),
            );
            let summary = match remote {
                Ok(remote) => run_post_upload_commands(
                    &thread_run_id,
                    thread_project.project_id,
                    PipelineSummary {
                        status: "succeeded",
                        archive_path: None,
                        archived_targets: Vec::new(),
                        manifests: Vec::new(),
                        error: None,
                        retry_descriptor: None,
                        remote_committed: true,
                        local_committed: false,
                        failed_commands: Vec::new(),
                    },
                    commands,
                    Box::new(remote),
                    worker_cancelled.clone(),
                    sink.clone(),
                ),
                Err(error) if worker_cancelled.load(Ordering::Acquire) || error.cancelled => {
                    PipelineSummary {
                        status: "cancelled",
                        archive_path: None,
                        archived_targets: Vec::new(),
                        manifests: Vec::new(),
                        error: Some(
                            "服务器文件已上传，上传后命令未全部完成，已按用户请求取消".into(),
                        ),
                        retry_descriptor: None,
                        remote_committed: true,
                        local_committed: false,
                        failed_commands: Vec::new(),
                    }
                }
                Err(error) => PipelineSummary {
                    status: "upload_succeeded_command_failed",
                    archive_path: None,
                    archived_targets: Vec::new(),
                    manifests: Vec::new(),
                    error: Some(format!(
                        "服务器文件已上传，但上传后命令未全部成功：{}",
                        error.message
                    )),
                    retry_descriptor: None,
                    remote_committed: true,
                    local_committed: false,
                    failed_commands: commands,
                },
            };
            let mut summary = summary;
            if summary.status == "upload_succeeded_command_failed" {
                if let Err(error) = finish_command_retry(retry, summary.failed_commands.clone())
                    .and_then(|token| remember_command_retry_token(&thread_run_id, token))
                {
                    append_pipeline_error(&mut summary, format!("创建命令重试任务失败：{error}"));
                }
            }
            let result = claim_pipeline_result(
                Ok(summary),
                &worker_cancelled,
                &worker_finished,
                &worker_cancel_won,
                &worker_claim_lock,
            );
            emit_terminal_result(
                sink.as_ref(),
                &thread_run_id,
                &thread_project,
                result,
                false,
            );
            if let Ok(mut active) = active_run().lock() {
                if active.as_ref().map(|run| run.run_id.as_str()) == Some(thread_run_id.as_str()) {
                    *active = None;
                }
            }
        })
        .map_err(|error| format!("启动命令重试线程失败：{error}"))?;

    let (authorization, retry) =
        consume_preflight_after(auth_token, &auth_binding, |authorization| {
            if authorization.expected_fingerprint != prepared.binding.fingerprint_sha256 {
                return Err("命令重试认证令牌与失败任务指纹不匹配".into());
            }
            consume_command_retry(retry_token, environment_id)
        })?;

    *active = Some(ActiveRun {
        run_id: run_id.clone(),
        cancelled,
        upload_stop,
        process_slots,
        ssh_sockets,
        finished,
        cancel_won,
        claim_lock,
    });
    if start_tx.send((authorization, retry)).is_err() {
        *active = None;
        return Err("命令重试线程未能接收启动数据".into());
    }
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
    SHUTTING_DOWN.store(true, Ordering::Release);
    if let Ok(active) = active_run().lock() {
        if let Some(active) = active.as_ref() {
            request_cancel(active);
        }
    }
    super::release_package_remote::clear_temporary_stores();
    if let Some(retries) = RETRY_JOBS.get() {
        if let Ok(mut retries) = retries.lock() {
            retries.clear();
        }
    }
    if let Some(retries) = COMMAND_RETRIES.get() {
        if let Ok(mut retries) = retries.lock() {
            retries.clear();
        }
    }
    if let Some(tokens) = COMMAND_RETRY_TOKENS.get() {
        if let Ok(mut tokens) = tokens.lock() {
            tokens.clear();
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
    fn command_retry_claims_the_run_slot_before_atomic_token_consumption() {
        let source = include_str!("release_package_runtime.rs");
        let start = source.find("pub fn command_retry(").unwrap();
        let end = source[start..]
            .find("pub fn cancel(")
            .map(|offset| start + offset)
            .unwrap();
        let command_retry = &source[start..end];

        let slot_check = command_retry.find("if active.is_some()").unwrap();
        let worker_spawn = command_retry.find("thread::Builder::new()").unwrap();
        let token_consumption = command_retry.find("consume_preflight_after(").unwrap();
        let slot_claim = command_retry.find("*active = Some(ActiveRun").unwrap();
        let worker_release = command_retry.find("start_tx.send").unwrap();

        assert!(slot_check < worker_spawn);
        assert!(worker_spawn < token_consumption);
        assert!(token_consumption < slot_claim);
        assert!(slot_claim < worker_release);
        assert!(command_retry.contains("command_retry_token.as_deref() != Some(retry_token)"));
    }

    #[test]
    fn exit_blocks_new_retry_state_before_clearing_all_command_tokens() {
        let source = include_str!("release_package_runtime.rs");
        let start = source.find("pub fn on_app_exit()").unwrap();
        let end = source[start..]
            .find("#[cfg(all(test, windows))]")
            .map(|offset| start + offset)
            .unwrap();
        let exit = &source[start..end];

        assert!(
            exit.find("SHUTTING_DOWN.store(true").unwrap()
                < exit.find("clear_temporary_stores()").unwrap()
        );
        assert!(exit.contains("COMMAND_RETRIES.get()"));
        assert!(exit.contains("COMMAND_RETRY_TOKENS.get()"));
    }

    #[test]
    fn action_dispatch_is_bound_before_worker_spawn_and_finished_before_status_emit() {
        let source = include_str!("release_package_runtime.rs");
        let start = &source[source.find("pub fn start(").unwrap()..];
        assert!(
            start.find("associate_release_package_run").unwrap()
                < start.find("thread::spawn").unwrap()
        );

        let terminal_start = source.find("fn emit_terminal_result(").unwrap();
        let terminal_end = source[terminal_start..]
            .find("\nfn emit_system_log")
            .map(|offset| terminal_start + offset)
            .unwrap();
        let terminal = &source[terminal_start..terminal_end];
        assert!(
            terminal.find("finish_release_package_run").unwrap()
                < terminal.find("sink.status").unwrap()
        );
    }

    #[test]
    fn powershell_output_read_error_is_not_treated_as_eof_after_keyword_match() {
        struct FailingReader {
            delivered_line: bool,
        }

        impl Read for FailingReader {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                if self.delivered_line {
                    return Err(std::io::Error::other("injected output read failure"));
                }
                self.delivered_line = true;
                let line = b"Build completed\n";
                buffer[..line.len()].copy_from_slice(line);
                Ok(line.len())
            }
        }

        let matched = AtomicBool::new(false);
        let logs = Mutex::new(Vec::new());
        let mut reader = BufReader::new(FailingReader {
            delivered_line: false,
        });
        let error = read_console_output(
            &mut reader,
            "stdout",
            &|stream: &'static str, line: String| {
                logs.lock().unwrap().push((stream.to_string(), line))
            },
            Some("Build completed"),
            &matched,
        )
        .unwrap_err();

        assert!(matched.load(Ordering::Acquire));
        assert_eq!(logs.lock().unwrap().len(), 1);
        assert!(error.contains("读取 PowerShell stdout 输出失败"));
    }

    #[test]
    fn powershell_reports_both_streams_and_nonzero_exit() {
        let logs = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
        let sink = logs.clone();
        let result = run_powershell(
            &std::env::temp_dir(),
            "Write-Output 'front-ok'; [Console]::Error.WriteLine('front-err'); exit 7",
            None,
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
                    None,
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
            None,
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
    use std::collections::VecDeque;
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
        logs: Mutex<Vec<LogEvent>>,
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
            self.logs.lock().unwrap().push(event);
        }

        fn status(&self, event: StatusEvent) {
            self.statuses.lock().unwrap().push(event);
        }

        fn notification(&self, _event: GlobalNotification) {}
    }

    struct CommandRemote {
        outcomes: VecDeque<Result<i32, DeployError>>,
        calls: Arc<Mutex<Vec<String>>>,
        cancel_after_calls: Option<(usize, Arc<AtomicBool>)>,
    }

    impl CommandRemote {
        fn new(
            outcomes: impl IntoIterator<Item = Result<i32, DeployError>>,
        ) -> (Self, Arc<Mutex<Vec<String>>>) {
            let calls = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    outcomes: outcomes.into_iter().collect(),
                    calls: Arc::clone(&calls),
                    cancel_after_calls: None,
                },
                calls,
            )
        }

        fn cancelling_after(
            outcomes: impl IntoIterator<Item = Result<i32, DeployError>>,
            count: usize,
            cancelled: Arc<AtomicBool>,
        ) -> (Self, Arc<Mutex<Vec<String>>>) {
            let (mut remote, calls) = Self::new(outcomes);
            remote.cancel_after_calls = Some((count, cancelled));
            (remote, calls)
        }
    }

    impl RemoteFs for CommandRemote {
        fn metadata(
            &self,
            _path: &str,
        ) -> Result<Option<crate::tools::release_package_deploy::RemoteMetadata>, DeployError>
        {
            Ok(None)
        }

        fn create_dir(&mut self, _path: &str) -> Result<(), DeployError> {
            Ok(())
        }

        fn read_dir(
            &self,
            _path: &str,
        ) -> Result<Vec<crate::tools::release_package_deploy::RemoteDirEntry>, DeployError>
        {
            Ok(Vec::new())
        }

        fn write_file(
            &mut self,
            _remote_path: &str,
            _local_path: &Path,
            _cancelled: &AtomicBool,
            _progress: &mut dyn FnMut(u64),
        ) -> Result<(), DeployError> {
            Ok(())
        }

        fn rename(&mut self, _source: &str, _target: &str) -> Result<(), DeployError> {
            Ok(())
        }

        fn remove_tree(&mut self, _path: &str) -> Result<(), DeployError> {
            Ok(())
        }

        fn execute_command(
            &mut self,
            command: &str,
            cancelled: &AtomicBool,
            output: &mut dyn FnMut(&str, String),
        ) -> Result<crate::tools::release_package_deploy::RemoteCommandResult, DeployError>
        {
            if cancelled.load(Ordering::Acquire) {
                return Err(DeployError::cancelled_command());
            }
            self.calls.lock().unwrap().push(command.to_string());
            output("stdout", format!("{command}-out"));
            if let Some((count, cancel)) = &self.cancel_after_calls {
                if self.calls.lock().unwrap().len() == *count {
                    cancel.store(true, Ordering::Release);
                }
            }
            match self.outcomes.pop_front().unwrap_or(Ok(0)) {
                Ok(exit_code) => {
                    Ok(crate::tools::release_package_deploy::RemoteCommandResult { exit_code })
                }
                Err(error) => Err(error),
            }
        }
    }

    fn succeeded_upload_summary() -> PipelineSummary {
        PipelineSummary {
            status: "succeeded",
            archive_path: None,
            archived_targets: Vec::new(),
            manifests: Vec::new(),
            error: None,
            retry_descriptor: None,
            remote_committed: true,
            local_committed: false,
            failed_commands: Vec::new(),
        }
    }
    #[cfg(windows)]
    impl CollectingSink {
        fn cancelling_during_archive(cancelled: Arc<AtomicBool>) -> Self {
            Self {
                logs: Mutex::new(Vec::new()),
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

    #[derive(Default)]
    struct BlockingProgressSink {
        statuses: Mutex<Vec<StatusEvent>>,
        first_entered: Mutex<bool>,
        first_ready: std::sync::Condvar,
        release_first: Mutex<bool>,
        release_ready: std::sync::Condvar,
    }

    impl BlockingProgressSink {
        fn wait_until_first_event_blocks(&self) {
            let mut entered = self.first_entered.lock().unwrap();
            while !*entered {
                entered = self.first_ready.wait(entered).unwrap();
            }
        }

        fn release_first_event(&self) {
            *self.release_first.lock().unwrap() = true;
            self.release_ready.notify_all();
        }
    }

    impl EventSink for BlockingProgressSink {
        fn log(&self, _event: LogEvent) {}

        fn status(&self, event: StatusEvent) {
            if event.uploaded_bytes == Some(10) {
                *self.first_entered.lock().unwrap() = true;
                self.first_ready.notify_all();
                let mut release = self.release_first.lock().unwrap();
                while !*release {
                    release = self.release_ready.wait(release).unwrap();
                }
            }
            self.statuses.lock().unwrap().push(event);
        }

        fn notification(&self, _event: GlobalNotification) {}
    }

    #[test]
    fn upload_progress_is_initially_and_finally_forced_but_middle_is_throttled() {
        let start = Instant::now();
        assert!(should_emit_upload_progress(None, start, false));
        assert!(!should_emit_upload_progress(
            Some(start),
            start + Duration::from_millis(99),
            false,
        ));
        assert!(should_emit_upload_progress(
            Some(start),
            start + Duration::from_millis(100),
            false,
        ));
        assert!(should_emit_upload_progress(
            Some(start),
            start + Duration::from_millis(1),
            true,
        ));
    }

    #[test]
    fn upload_progress_aggregates_concurrent_bytes_without_bypassing_throttle() {
        let sink = Arc::new(TerminalSink::default());
        let reporter = Arc::new(UploadProgressReporter::new(
            sink.clone(),
            "run-progress",
            7,
            200,
        ));
        let start = Instant::now();
        reporter.force_emit_at(start, false);

        let mut handles = Vec::new();
        for path in ["index.html", "app.jar"] {
            let reporter = Arc::clone(&reporter);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    reporter.report_at(1, path, start + Duration::from_millis(50));
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        reporter.force_emit_at(start + Duration::from_millis(100), true);

        let statuses = sink.statuses.lock().unwrap();
        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses[0].uploaded_bytes, Some(0));
        assert_eq!(statuses[1].uploaded_bytes, Some(200));
        assert_eq!(reporter.uploaded_bytes(), 200);
    }

    #[test]
    fn upload_progress_events_never_regress_when_the_first_sink_delivery_blocks() {
        let sink = Arc::new(BlockingProgressSink::default());
        let reporter = Arc::new(UploadProgressReporter::new(
            sink.clone(),
            "run-order",
            7,
            20,
        ));
        let start = Instant::now();
        let first = {
            let reporter = Arc::clone(&reporter);
            thread::spawn(move || reporter.report_at(10, "first", start))
        };
        sink.wait_until_first_event_blocks();

        let (attempted_tx, attempted_rx) = std::sync::mpsc::channel();
        let second = {
            let reporter = Arc::clone(&reporter);
            thread::spawn(move || {
                attempted_tx.send(()).unwrap();
                reporter.report_at(10, "second", start + Duration::from_millis(100));
            })
        };
        attempted_rx.recv().unwrap();
        sink.release_first_event();
        first.join().unwrap();
        second.join().unwrap();

        let progress = sink
            .statuses
            .lock()
            .unwrap()
            .iter()
            .map(|event| event.uploaded_bytes.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(progress, vec![10, 20]);
    }

    fn project() -> ReleasePackageEnvironmentConfig {
        ReleasePackageEnvironmentConfig {
            id: 7,
            project_id: 7,
            project_name: "test".into(),
            environment: ReleasePackageEnvironmentKind::Test,
            configured: true,
            output_root: "Z:\\output".into(),
            package_type: ReleasePackageType::LocalArchive,
            frontend_project_path: "Z:\\missing".into(),
            frontend_build_command: "exit 0".into(),
            frontend_success_keyword: String::new(),
            frontend_post_upload_command: String::new(),
            frontend_artifact_path: "dist".into(),
            frontend_artifact_mode: "copy_directory".into(),
            backend_project_path: "Z:\\missing".into(),
            backend_build_command: "exit 0".into(),
            backend_success_keyword: String::new(),
            backend_post_upload_command: String::new(),
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
    fn frontend_build_project(root: &Path, artifact_mode: &str) -> ReleasePackageEnvironmentConfig {
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

    #[cfg(windows)]
    fn keyword_build_project(root: &Path) -> ReleasePackageEnvironmentConfig {
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
        project.backend_project_path = backend_project.to_string_lossy().into_owned();
        project.backend_build_command =
            "New-Item -ItemType Directory -Force target | Out-Null; Set-Content target/app.jar jar"
                .into();
        project.backend_artifact_path = "target/app.jar".into();
        project
    }

    #[cfg(windows)]
    fn run_keyword_build(
        project: ReleasePackageEnvironmentConfig,
        targets: Vec<ReleaseTarget>,
    ) -> BuildSummary {
        run_build_pipeline(
            "keyword-run",
            project,
            targets,
            Arc::new(AtomicBool::new(false)),
            ProcessSlots::new(),
            Arc::new(CollectingSink::default()),
        )
        .unwrap()
    }

    #[cfg(windows)]
    #[test]
    fn empty_success_keyword_keeps_existing_build_success_behavior() {
        let root = TestDir::new();
        let project = keyword_build_project(&root.0);

        let summary = run_keyword_build(project, vec![ReleaseTarget::Frontend]);

        assert_eq!(summary.status, "succeeded");
        assert_eq!(summary.built_targets.len(), 1);
        assert!(summary.error.is_none());
    }

    #[cfg(windows)]
    #[test]
    fn frontend_success_keyword_can_match_stdout_line() {
        let root = TestDir::new();
        let mut project = keyword_build_project(&root.0);
        project.frontend_success_keyword = "Build completed".into();
        project.frontend_build_command =
            "Write-Output 'Build completed'; New-Item -ItemType Directory -Force dist | Out-Null; Set-Content dist/index.html web".into();

        let summary = run_keyword_build(project, vec![ReleaseTarget::Frontend]);

        assert_eq!(summary.status, "succeeded");
        assert_eq!(summary.built_targets.len(), 1);
        assert!(summary.error.is_none());
    }

    #[cfg(windows)]
    #[test]
    fn backend_success_keyword_can_match_stderr_line() {
        let root = TestDir::new();
        let mut project = keyword_build_project(&root.0);
        project.backend_success_keyword = "BUILD SUCCESS".into();
        project.backend_build_command =
            "[Console]::Error.WriteLine('BUILD SUCCESS'); New-Item -ItemType Directory -Force target | Out-Null; Set-Content target/app.jar jar".into();

        let summary = run_keyword_build(project, vec![ReleaseTarget::Backend]);

        assert_eq!(summary.status, "succeeded");
        assert_eq!(summary.built_targets.len(), 1);
        assert!(summary.error.is_none());
    }

    #[cfg(windows)]
    #[test]
    fn success_keyword_matching_is_case_sensitive() {
        let root = TestDir::new();
        let mut project = keyword_build_project(&root.0);
        project.frontend_success_keyword = "Build completed".into();
        project.frontend_build_command =
            "Write-Output 'build completed'; New-Item -ItemType Directory -Force dist | Out-Null; Set-Content dist/index.html web".into();

        let summary = run_keyword_build(project, vec![ReleaseTarget::Frontend]);

        assert_eq!(summary.status, "failed");
        assert!(summary.built_targets.is_empty());
        let error = summary.error.unwrap();
        assert!(error.contains("frontend："));
        assert!(error.contains("前端构建命令退出成功，但日志未匹配成功关键字：Build completed"));
    }

    #[cfg(windows)]
    #[test]
    fn nonzero_exit_code_fails_even_when_success_keyword_matches() {
        let root = TestDir::new();
        let mut project = keyword_build_project(&root.0);
        project.frontend_success_keyword = "Build completed".into();
        project.frontend_build_command =
            "Write-Output 'Build completed'; New-Item -ItemType Directory -Force dist | Out-Null; Set-Content dist/index.html web; exit 9".into();

        let summary = run_keyword_build(project, vec![ReleaseTarget::Frontend]);

        assert_eq!(summary.status, "failed");
        assert!(summary.built_targets.is_empty());
        let error = summary.error.unwrap();
        assert!(error.contains("frontend：PowerShell 命令退出码：9"));
        assert!(!error.contains("日志未匹配成功关键字"));
    }

    #[cfg(windows)]
    #[test]
    fn matched_success_keyword_still_requires_valid_artifact() {
        let root = TestDir::new();
        let mut project = keyword_build_project(&root.0);
        project.frontend_success_keyword = "Build completed".into();
        project.frontend_build_command = "Write-Output 'Build completed'".into();

        let summary = run_keyword_build(project, vec![ReleaseTarget::Frontend]);

        assert_eq!(summary.status, "failed");
        assert!(summary.built_targets.is_empty());
        let error = summary.error.unwrap();
        assert!(error.contains("frontend：前端产物必须是文件夹"));
        assert!(!error.contains("日志未匹配成功关键字"));
    }

    #[cfg(windows)]
    #[test]
    fn unselected_target_success_keyword_is_not_checked() {
        let root = TestDir::new();
        let mut project = keyword_build_project(&root.0);
        project.backend_success_keyword = "BACKEND_DONE".into();
        project.backend_build_command =
            "New-Item -ItemType Directory -Force target | Out-Null; Set-Content target/app.jar jar"
                .into();

        let summary = run_keyword_build(project, vec![ReleaseTarget::Frontend]);

        assert_eq!(summary.status, "succeeded");
        assert_eq!(summary.built_targets.len(), 1);
        assert!(summary
            .built_targets
            .iter()
            .all(|target| target.target == ReleaseTarget::Frontend));
        assert!(summary.error.is_none());
    }

    #[cfg(windows)]
    fn run_local_cleanup_warning_case(
        selected_count: usize,
        build_error: Option<&str>,
    ) -> (TestDir, PathBuf, PipelineSummary) {
        let root = TestDir::new();
        let output = root.0.join("output");
        let final_path = output.join("release");
        let backup_path = output.join(".lazycat-release-package-run-local-cleanup.backup");
        let source = root.0.join("dist");
        fs::create_dir_all(&final_path).unwrap();
        fs::write(final_path.join("old.txt"), "old").unwrap();
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("index.html"), "new").unwrap();

        let summary = run_local_archive_pipeline_with_commit(
            "run-local-cleanup",
            7,
            BuildSummary {
                status: if build_error.is_some() {
                    "partially_succeeded"
                } else {
                    "succeeded"
                },
                built_targets: vec![BuiltTarget {
                    target: ReleaseTarget::Frontend,
                    source_path: source,
                    artifact_mode: "copy_directory".into(),
                }],
                selected_count,
                error: build_error.map(str::to_owned),
            },
            output,
            "release".into(),
            true,
            Arc::new(AtomicBool::new(false)),
            Arc::new(Sink),
            move |archive, cancelled| {
                let mut rename_count = 0;
                archive.commit_with_rename(cancelled, |source, target, _| {
                    rename_count += 1;
                    fs::rename(source, target)
                        .map_err(crate::tools::release_package_archive::RenameFailure::Io)?;
                    if rename_count == 2 {
                        fs::remove_dir_all(&backup_path).unwrap();
                        fs::write(&backup_path, "cannot remove as directory").unwrap();
                    }
                    Ok(())
                })
            },
        )
        .unwrap();

        (root, final_path, summary)
    }

    fn consumed_preflight_with_existing(
        expected_existing_targets: Vec<RemoteTarget>,
    ) -> ConsumedPreflight {
        ConsumedPreflight {
            binding: crate::tools::release_package_remote::PreflightBinding {
                environment_id: 7,
                project_id: 7,
                environment: ReleasePackageEnvironmentKind::Test,
                endpoint: crate::tools::release_package_remote::RemoteEndpoint {
                    host: "server.example".into(),
                    port: 22,
                    username: "deploy".into(),
                },
                auth_type: "password".into(),
                vault_entry_id: None,
                private_key_path: String::new(),
                targets: vec![RemoteTarget::Frontend, RemoteTarget::Backend],
                command_retry_token: None,
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
                local_committed: false,
                failed_commands: Vec::new(),
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
    fn post_upload_commands_run_after_commit_in_target_order() {
        let sink = Arc::new(CollectingSink::default());
        let (remote, calls) = CommandRemote::new([Ok(7), Ok(0)]);
        let commands = vec![
            CommandSnapshot::new(ReleaseTarget::Frontend, "reload-web"),
            CommandSnapshot::new(ReleaseTarget::Backend, "restart-api"),
        ];

        let summary = run_post_upload_commands(
            "post-upload-run",
            7,
            succeeded_upload_summary(),
            commands,
            Box::new(remote),
            Arc::new(AtomicBool::new(false)),
            sink.clone(),
        );

        assert_eq!(
            *calls.lock().unwrap(),
            vec!["reload-web".to_string(), "restart-api".to_string()]
        );
        assert_eq!(summary.status, "upload_succeeded_command_failed");
        assert!(summary.remote_committed);
        assert!(summary.retry_descriptor.is_none());
        assert_eq!(summary.failed_commands.len(), 1);
        assert_eq!(summary.failed_commands[0].target, ReleaseTarget::Frontend);
        let logs = sink.logs.lock().unwrap();
        assert!(logs.iter().any(|event| event.line.contains("[前端命令]")));
        assert!(logs.iter().any(|event| event.line.contains("[后端命令]")));
        let statuses = sink.statuses.lock().unwrap();
        let command_states = statuses
            .iter()
            .filter_map(|event| {
                Some((
                    event.command_target.as_deref()?.to_string(),
                    event.command_status.as_deref()?.to_string(),
                ))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            command_states,
            vec![
                ("frontend".into(), "running".into()),
                ("frontend".into(), "failed".into()),
                ("backend".into(), "running".into()),
                ("backend".into(), "succeeded".into()),
            ]
        );
        assert!(statuses.iter().any(|event| {
            event.command_target.as_deref() == Some("frontend")
                && event.command_status.as_deref() == Some("failed")
                && event
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("退出码：7"))
        }));
    }

    #[test]
    fn committed_cleanup_warning_keeps_control_for_post_upload_commands() {
        let sink = Arc::new(CollectingSink::default());
        let (remote, calls) = CommandRemote::new([Ok(0)]);
        let backup_path = "/srv/app/web.__lazycat_backup_run-1";
        let (summary, control) = resolve_deployment_result(
            succeeded_upload_summary(),
            Ok(DeploymentSuccess {
                control: Box::new(remote),
                warning: Some(DeployError {
                    message: "远端提交成功，但旧版本备份清理失败".into(),
                    cancelled: false,
                    committed: true,
                    recovery_paths: vec![backup_path.into()],
                }),
            }),
        );

        let summary = run_post_upload_commands(
            "post-upload-run",
            7,
            summary,
            vec![CommandSnapshot::new(ReleaseTarget::Frontend, "reload-web")],
            control.expect("committed deployment keeps its control connection"),
            Arc::new(AtomicBool::new(false)),
            sink,
        );

        assert_eq!(*calls.lock().unwrap(), vec!["reload-web".to_string()]);
        assert!(summary.remote_committed);
        let warning = summary.error.expect("cleanup warning is preserved");
        assert!(warning.contains("旧版本备份清理失败"));
        assert!(warning.contains(backup_path));
    }

    #[test]
    fn post_upload_command_failure_preserves_existing_committed_warning() {
        let (remote, _calls) = CommandRemote::new([Ok(7)]);
        let backup_path = "/srv/app/web.__lazycat_backup_run-1";
        let mut summary = succeeded_upload_summary();
        summary.error = Some(format!(
            "远端提交成功，但旧版本备份清理失败；需人工检查：{backup_path}"
        ));

        let summary = run_post_upload_commands(
            "post-upload-run",
            7,
            summary,
            vec![CommandSnapshot::new(ReleaseTarget::Frontend, "reload-web")],
            Box::new(remote),
            Arc::new(AtomicBool::new(false)),
            Arc::new(CollectingSink::default()),
        );

        assert_eq!(summary.status, "upload_succeeded_command_failed");
        let error = summary.error.expect("both warnings are preserved");
        assert!(error.contains("旧版本备份清理失败"));
        assert!(error.contains(backup_path));
        assert!(error.contains("上传后命令未全部成功"));
    }

    #[test]
    fn post_upload_commands_skip_when_upload_was_not_committed() {
        let (remote, calls) = CommandRemote::new([Ok(0)]);
        let mut summary = succeeded_upload_summary();
        summary.remote_committed = false;
        summary.status = "package_succeeded_upload_failed";

        let result = run_post_upload_commands(
            "post-upload-run",
            7,
            summary,
            vec![CommandSnapshot::new(ReleaseTarget::Frontend, "reload-web")],
            Box::new(remote),
            Arc::new(AtomicBool::new(false)),
            Arc::new(CollectingSink::default()),
        );

        assert!(calls.lock().unwrap().is_empty());
        assert_eq!(result.status, "package_succeeded_upload_failed");
    }

    #[test]
    fn post_upload_commands_keep_success_when_no_command_is_configured() {
        let (remote, calls) = CommandRemote::new([]);
        let sink = Arc::new(CollectingSink::default());
        let mut upload = succeeded_upload_summary();
        upload.manifests = [ReleaseTarget::Frontend, ReleaseTarget::Backend]
            .into_iter()
            .map(|target| ArtifactManifest {
                target,
                source_path: PathBuf::new(),
                entries: Vec::new(),
                file_count: 0,
                total_bytes: 0,
            })
            .collect();

        let summary = run_post_upload_commands(
            "post-upload-run",
            7,
            upload,
            Vec::new(),
            Box::new(remote),
            Arc::new(AtomicBool::new(false)),
            sink.clone(),
        );

        assert!(calls.lock().unwrap().is_empty());
        assert_eq!(summary.status, "succeeded");
        assert!(summary.failed_commands.is_empty());
        let command_states = sink
            .statuses
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| {
                Some((
                    event.command_target.as_deref()?.to_string(),
                    event.command_status.as_deref()?.to_string(),
                ))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            command_states,
            vec![
                ("frontend".to_string(), "skipped".to_string()),
                ("backend".to_string(), "skipped".to_string()),
            ]
        );
    }

    #[test]
    fn post_upload_command_cancellation_stops_later_commands_without_retry_snapshot() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let (remote, calls) =
            CommandRemote::cancelling_after([Ok(0), Ok(0)], 1, Arc::clone(&cancelled));
        let sink = Arc::new(CollectingSink::default());

        let summary = run_post_upload_commands(
            "post-upload-run",
            7,
            succeeded_upload_summary(),
            vec![
                CommandSnapshot::new(ReleaseTarget::Frontend, "reload-web"),
                CommandSnapshot::new(ReleaseTarget::Backend, "restart-api"),
            ],
            Box::new(remote),
            cancelled,
            sink.clone(),
        );

        assert_eq!(*calls.lock().unwrap(), vec!["reload-web".to_string()]);
        assert_eq!(summary.status, "cancelled");
        assert!(summary.remote_committed);
        assert!(summary.failed_commands.is_empty());
        assert!(summary.error.unwrap().contains("服务器文件已上传"));
        let statuses = sink.statuses.lock().unwrap();
        let cancelled_targets = statuses
            .iter()
            .filter(|event| event.command_status.as_deref() == Some("cancelled"))
            .filter_map(|event| event.command_target.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(cancelled_targets, vec!["frontend", "backend"]);
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
                local_committed: false,
                failed_commands: Vec::new(),
            },
            Err(DeployError::failed("SFTP 传输中断")),
        );

        assert_eq!(summary.status, "package_succeeded_upload_failed");
        assert!(summary.archive_path.is_none());
        assert_eq!(summary.retry_descriptor.unwrap().manifests, vec![manifest]);
    }

    #[test]
    fn deployment_request_failure_keeps_post_upload_commands_for_retry() {
        let manifest = ArtifactManifest {
            target: ReleaseTarget::Frontend,
            source_path: PathBuf::from(r"D:\build\dist"),
            entries: Vec::new(),
            file_count: 0,
            total_bytes: 0,
        };
        let mut upload_project = project();
        upload_project.frontend_post_upload_command = "reload-web".into();
        let summary = PipelineSummary {
            status: "succeeded",
            archive_path: None,
            archived_targets: Vec::new(),
            manifests: vec![manifest],
            error: None,
            retry_descriptor: None,
            remote_committed: false,
            local_committed: false,
            failed_commands: Vec::new(),
        };

        let result = run_deployment_phase(
            "initial-request-failure",
            &upload_project,
            summary,
            DeployAuthorization {
                consumed: consumed_preflight_with_existing(Vec::new()),
            },
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            &Arc::new(SshSocketRegistry::new()),
            Arc::new(Sink),
        );

        assert_eq!(result.status, "package_succeeded_upload_failed");
        assert_eq!(
            result.retry_descriptor.unwrap().commands,
            vec![CommandSnapshot::new(ReleaseTarget::Frontend, "reload-web")]
        );
    }

    #[test]
    fn committed_cleanup_warning_survives_terminal_result_without_retry_token() {
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
                local_committed: false,
                failed_commands: Vec::new(),
            },
            Err(DeployError {
                message: "远端提交成功，但旧版本备份清理失败".into(),
                cancelled: false,
                committed: true,
                recovery_paths: vec![backup_path.into()],
            }),
        );
        let mut upload_project = project();
        upload_project.package_type = ReleasePackageType::ServerUpload;
        let sink = TerminalSink::default();
        emit_terminal_result(
            &sink,
            "run-upload-cleanup",
            &upload_project,
            Ok(summary),
            false,
        );

        let status = &sink.statuses.lock().unwrap()[0];
        assert_eq!(status.status, "succeeded");
        assert!(status.archive_path.is_none());
        assert!(status.retry_token.is_none());
        let error = status.error.as_ref().unwrap();
        assert!(error.contains("旧版本备份清理失败"));
        assert!(error.contains(backup_path));
        assert_eq!(sink.notifications.lock().unwrap().len(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn local_committed_cleanup_warning_preserves_success_summary() {
        let (_root, final_path, summary) = run_local_cleanup_warning_case(1, None);

        assert_eq!(summary.status, "succeeded");
        assert_eq!(summary.archive_path.as_deref(), Some(final_path.as_path()));
        assert!(summary.retry_descriptor.is_none());
        let error = summary.error.unwrap();
        assert!(error.contains("清理旧归档备份"));
        assert!(error.contains("run-local-cleanup.backup"));
    }

    #[cfg(windows)]
    #[test]
    fn local_committed_cleanup_warning_preserves_partial_summary() {
        let (_root, final_path, summary) =
            run_local_cleanup_warning_case(2, Some("backend：构建失败"));

        assert_eq!(summary.status, "partially_succeeded");
        assert_eq!(summary.archive_path.as_deref(), Some(final_path.as_path()));
        assert!(summary.retry_descriptor.is_none());
        let error = summary.error.unwrap();
        assert!(error.contains("backend：构建失败"));
        assert!(error.contains("清理旧归档备份"));
        assert!(error.contains("run-local-cleanup.backup"));
    }

    #[cfg(windows)]
    #[test]
    fn late_cancellation_does_not_override_local_committed_cleanup_warning() {
        let (_root, final_path, summary) = run_local_cleanup_warning_case(1, None);
        let cancelled = AtomicBool::new(true);
        let finished = AtomicBool::new(false);
        let cancel_won = AtomicBool::new(false);

        let result = claim_pipeline_result(
            Ok(summary),
            &cancelled,
            &finished,
            &cancel_won,
            &Mutex::new(()),
        )
        .unwrap();

        assert_eq!(result.status, "succeeded");
        assert_eq!(result.archive_path.as_deref(), Some(final_path.as_path()));
        assert!(result
            .error
            .as_deref()
            .is_some_and(|error| error.contains("清理旧归档备份")));
        assert!(!cancel_won.load(Ordering::Acquire));
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
            local_committed: false,
            failed_commands: Vec::new(),
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
                local_committed: false,
                failed_commands: Vec::new(),
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
            local_committed: false,
            failed_commands: Vec::new(),
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
                local_committed: false,
                failed_commands: Vec::new(),
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
    fn internal_upload_stop_does_not_mark_user_cancelled() {
        let cancelled = AtomicBool::new(false);
        let upload_stop = AtomicBool::new(true);
        let result = claim_pipeline_result(
            Ok(combine_package_and_deploy(
                PipelineSummary {
                    status: "succeeded",
                    archive_path: None,
                    archived_targets: Vec::new(),
                    manifests: Vec::new(),
                    error: None,
                    retry_descriptor: None,
                    remote_committed: false,
                    local_committed: false,
                    failed_commands: Vec::new(),
                },
                Err(DeployError::failed("并行上传因其他目标失败而停止")),
            )),
            &cancelled,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            &Mutex::new(()),
        )
        .unwrap();

        assert_eq!(result.status, "package_succeeded_upload_failed");
        assert!(!cancelled.load(Ordering::Acquire));
        assert!(upload_stop.load(Ordering::Acquire));
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
        let upload_stop = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let claim_lock = Arc::new(Mutex::new(()));
        {
            let _guard = claim_lock.lock().unwrap();
            finished.store(true, Ordering::Release);
        }
        let active = ActiveRun {
            run_id: "finished".into(),
            cancelled: cancelled.clone(),
            upload_stop: upload_stop.clone(),
            process_slots: ProcessSlots::new(),
            ssh_sockets: Arc::new(SshSocketRegistry::new()),
            finished,
            cancel_won: Arc::new(AtomicBool::new(false)),
            claim_lock,
        };
        assert!(!request_cancel(&active));
        assert!(!cancelled.load(Ordering::Acquire));
        assert!(!upload_stop.load(Ordering::Acquire));
    }

    #[test]
    fn cancellation_closes_the_active_ssh_socket() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let first = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let second = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut first_server, _) = listener.accept().unwrap();
        let (mut second_server, _) = listener.accept().unwrap();
        let ssh_sockets = Arc::new(SshSocketRegistry::new());
        ssh_sockets.register(first).unwrap();
        ssh_sockets.register(second).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let upload_stop = Arc::new(AtomicBool::new(false));
        let active = ActiveRun {
            run_id: "uploading".into(),
            cancelled: cancelled.clone(),
            upload_stop: upload_stop.clone(),
            process_slots: ProcessSlots::new(),
            ssh_sockets: ssh_sockets.clone(),
            finished: Arc::new(AtomicBool::new(false)),
            cancel_won: Arc::new(AtomicBool::new(false)),
            claim_lock: Arc::new(Mutex::new(())),
        };

        assert!(request_cancel(&active));
        assert!(cancelled.load(Ordering::Acquire));
        assert!(upload_stop.load(Ordering::Acquire));
        assert_eq!(ssh_sockets.len_for_test(), 0);
        let mut byte = [0_u8; 1];
        assert_eq!(first_server.read(&mut byte).unwrap(), 0);
        assert_eq!(second_server.read(&mut byte).unwrap(), 0);
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
                local_committed: false,
                failed_commands: Vec::new(),
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
            local_committed: false,
            failed_commands: Vec::new(),
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

    #[cfg(windows)]
    #[test]
    fn retry_request_failure_keeps_post_upload_commands_for_next_retry() {
        let root = TestDir::new();
        let source = root.0.join("app.jar");
        fs::write(&source, "v1").unwrap();
        let manifest = ArtifactManifest::from_file(ReleaseTarget::Backend, &source).unwrap();
        fs::write(&source, "changed-size").unwrap();
        let retry = RetryJob {
            environment_id: 7,
            descriptor: RetryDescriptor {
                manifests: vec![manifest],
                commands: vec![CommandSnapshot::new(ReleaseTarget::Backend, "restart-api")],
            },
        };
        let mut consumed = consumed_preflight_with_existing(Vec::new());
        consumed.binding.targets = vec![RemoteTarget::Backend];

        let result = run_retry_deployment_phase(
            "retry-request-failure",
            7,
            retry,
            DeployAuthorization { consumed },
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            &Arc::new(SshSocketRegistry::new()),
            Arc::new(Sink),
        );

        assert_eq!(result.status, "package_succeeded_upload_failed");
        assert_eq!(
            result.retry_descriptor.unwrap().commands,
            vec![CommandSnapshot::new(ReleaseTarget::Backend, "restart-api")]
        );
    }

    #[test]
    fn retry_tokens_are_consumed_once_and_bound_to_the_environment() {
        let descriptor = RetryDescriptor {
            manifests: Vec::new(),
            commands: Vec::new(),
        };
        let token = issue_retry(7, descriptor).unwrap();

        assert!(consume_retry(&token, 8).is_err());
        let retry = consume_retry(&token, 7).unwrap();
        assert_eq!(retry.environment_id, 7);
        assert!(consume_retry(&token, 7).is_err());
    }

    #[test]
    fn command_retry_contains_only_failed_commands_and_rotates_after_failure() {
        let binding = CommandAuthBinding {
            environment_id: 7,
            endpoint: RemoteEndpoint {
                host: "deploy.example.internal".into(),
                port: 22,
                username: "deploy".into(),
            },
            auth_type: "private_key".into(),
            vault_entry_id: None,
            private_key_path: r"C:\Users\tester\.ssh\deploy".into(),
            fingerprint_sha256: "SHA256:trusted".into(),
        };
        let failed = vec![CommandSnapshot::new(ReleaseTarget::Frontend, "reload-web")];
        let first = issue_command_retry(7, binding.clone(), failed.clone()).unwrap();

        let prepared = prepare_command_retry(&first, 7).unwrap();
        assert_eq!(prepared.targets, vec![ReleaseTarget::Frontend]);
        assert_eq!(prepared.binding, binding);
        assert_eq!(prepared.failed_commands, failed);

        let job = consume_command_retry(&first, 7).unwrap();
        assert!(consume_command_retry(&first, 7).is_err());
        assert!(prepare_command_retry(&first, 7).is_err());

        let second = finish_command_retry(
            job,
            vec![CommandSnapshot::new(ReleaseTarget::Frontend, "reload-web")],
        )
        .unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn command_retry_token_requires_its_binding_environment() {
        let error = issue_command_retry(
            8,
            CommandAuthBinding {
                environment_id: 7,
                endpoint: RemoteEndpoint {
                    host: "deploy.example.internal".into(),
                    port: 22,
                    username: "deploy".into(),
                },
                auth_type: "password".into(),
                vault_entry_id: Some(9),
                private_key_path: String::new(),
                fingerprint_sha256: "SHA256:trusted".into(),
            },
            vec![CommandSnapshot::new(ReleaseTarget::Backend, "restart-api")],
        )
        .unwrap_err();

        assert_eq!(error, "命令重试令牌与环境快照不匹配");
    }

    #[test]
    fn command_retry_rejects_a_different_project_without_consuming_the_job() {
        let token = issue_command_retry(
            7,
            CommandAuthBinding {
                environment_id: 7,
                endpoint: RemoteEndpoint {
                    host: "deploy.example.internal".into(),
                    port: 22,
                    username: "deploy".into(),
                },
                auth_type: "password".into(),
                vault_entry_id: Some(9),
                private_key_path: String::new(),
                fingerprint_sha256: "SHA256:trusted".into(),
            },
            vec![CommandSnapshot::new(ReleaseTarget::Backend, "restart-api")],
        )
        .unwrap();

        assert!(prepare_command_retry(&token, 8).is_err());
        assert!(consume_command_retry(&token, 8).is_err());
        assert!(prepare_command_retry(&token, 7).is_ok());
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
        let project = ReleasePackageEnvironmentConfig {
            id: 1,
            project_id: 1,
            project_name: "构建项目".into(),
            environment: ReleasePackageEnvironmentKind::Test,
            configured: true,
            output_root: output_root.to_string_lossy().into_owned(),
            package_type: ReleasePackageType::LocalArchive,
            frontend_project_path: frontend_project.to_string_lossy().into_owned(),
            frontend_build_command: "New-Item -ItemType Directory -Force dist | Out-Null; Set-Content dist/index.html web".into(),
            frontend_success_keyword: String::new(),
            frontend_post_upload_command: String::new(),
            frontend_artifact_path: "dist".into(),
            frontend_artifact_mode: "copy_directory".into(),
            backend_project_path: backend_project.to_string_lossy().into_owned(),
            backend_build_command: "New-Item -ItemType Directory -Force target | Out-Null; Set-Content target/app.jar jar".into(),
            backend_success_keyword: String::new(),
            backend_post_upload_command: String::new(),
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
        let project = ReleasePackageEnvironmentConfig {
            id: 1,
            project_id: 1,
            project_name: "冒烟项目".into(),
            environment: ReleasePackageEnvironmentKind::Test,
            configured: true,
            output_root: output_root.to_string_lossy().into_owned(),
            package_type: ReleasePackageType::LocalArchive,
            frontend_project_path: frontend_project.to_string_lossy().into_owned(),
            frontend_build_command: "New-Item -ItemType Directory -Force dist | Out-Null; Set-Content dist/index.html web".into(),
            frontend_success_keyword: String::new(),
            frontend_post_upload_command: String::new(),
            frontend_artifact_path: "dist".into(),
            frontend_artifact_mode: "copy_directory".into(),
            backend_project_path: backend_project.to_string_lossy().into_owned(),
            backend_build_command: "New-Item -ItemType Directory -Force target | Out-Null; Set-Content target/app.jar jar".into(),
            backend_success_keyword: String::new(),
            backend_post_upload_command: String::new(),
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
        let project = ReleasePackageEnvironmentConfig {
            id: 2,
            project_id: 2,
            project_name: "冒烟项目".into(),
            environment: ReleasePackageEnvironmentKind::Test,
            configured: true,
            output_root: output_root.to_string_lossy().into_owned(),
            package_type: ReleasePackageType::LocalArchive,
            frontend_project_path: frontend_project.to_string_lossy().into_owned(),
            frontend_build_command: "exit 9".into(),
            frontend_success_keyword: String::new(),
            frontend_post_upload_command: String::new(),
            frontend_artifact_path: "dist".into(),
            frontend_artifact_mode: "copy_directory".into(),
            backend_project_path: backend_project.to_string_lossy().into_owned(),
            backend_build_command: "New-Item -ItemType Directory -Force target | Out-Null; Set-Content target/app.jar backend-ok".into(),
            backend_success_keyword: String::new(),
            backend_post_upload_command: String::new(),
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

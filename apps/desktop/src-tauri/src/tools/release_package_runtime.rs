use serde::Serialize;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use tauri::Emitter;

use super::release_package::ReleasePackageProjectConfig;
use super::release_package_archive::{
    archive_artifacts, resolve_artifact_path, ArchiveError, ArchiveRequest,
};
use crate::events::{EVENT_RELEASE_PACKAGE_LOG, EVENT_RELEASE_PACKAGE_STATUS};

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

struct ActiveRun {
    run_id: String,
    cancelled: Arc<AtomicBool>,
    pid: Arc<Mutex<Option<u32>>>,
    finished: Arc<AtomicBool>,
    cancel_won: Arc<AtomicBool>,
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
}

trait EventSink: Send + Sync {
    fn log(&self, event: LogEvent);
    fn status(&self, event: StatusEvent);
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
}

#[derive(Debug)]
enum PipelineError {
    Cancelled {
        phase: &'static str,
    },
    Failed {
        phase: &'static str,
        message: String,
    },
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
    });
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
            phase,
            message: error.message(),
        },
    })
}

fn run_pipeline(
    run_id: &str,
    project: ReleasePackageProjectConfig,
    output_root: PathBuf,
    folder_name: String,
    cancelled: Arc<AtomicBool>,
    pid: Arc<Mutex<Option<u32>>>,
    sink: Arc<dyn EventSink>,
) -> Result<PathBuf, PipelineError> {
    let frontend_project = PathBuf::from(&project.frontend_project_path);
    run_command_phase(
        run_id,
        project.id,
        "frontend",
        &frontend_project,
        &project.frontend_build_command,
        cancelled.clone(),
        pid.clone(),
        sink.clone(),
    )?;
    let frontend_artifact =
        resolve_artifact_path(&frontend_project, &project.frontend_artifact_path);
    if !frontend_artifact.is_dir() {
        return Err(PipelineError::Failed {
            phase: "frontend",
            message: "前端产物必须是文件夹".into(),
        });
    }

    let backend_project = PathBuf::from(&project.backend_project_path);
    run_command_phase(
        run_id,
        project.id,
        "backend",
        &backend_project,
        &project.backend_build_command,
        cancelled.clone(),
        pid,
        sink.clone(),
    )?;
    let backend_artifact = resolve_artifact_path(&backend_project, &project.backend_artifact_path);
    if !backend_artifact.exists() {
        return Err(PipelineError::Failed {
            phase: "backend",
            message: "后端产物不存在".into(),
        });
    }

    if cancelled.load(Ordering::Acquire) {
        return Err(PipelineError::Cancelled { phase: "archive" });
    }
    emit_status(
        sink.as_ref(),
        run_id,
        project.id,
        "running",
        "archive",
        None,
        None,
    );
    emit_system_log(
        sink.as_ref(),
        run_id,
        project.id,
        "archive",
        "开始归档构建产物",
    );
    let request = ArchiveRequest {
        frontend_artifact,
        frontend_mode: project.frontend_artifact_mode,
        backend_artifact,
        output_root,
        folder_name,
        run_id: run_id.into(),
    };
    archive_artifacts(&request, &cancelled, |line| {
        emit_system_log(sink.as_ref(), run_id, project.id, "archive", line)
    })
    .map_err(|error| match error {
        ArchiveError::Cancelled => PipelineError::Cancelled { phase: "archive" },
        ArchiveError::Failed(message) => PipelineError::Failed {
            phase: "archive",
            message,
        },
    })
}

fn pipeline_phase(result: &Result<PathBuf, PipelineError>) -> &'static str {
    match result {
        Ok(_) => "archive",
        Err(PipelineError::Cancelled { phase }) | Err(PipelineError::Failed { phase, .. }) => phase,
    }
}

fn claim_pipeline_result(
    result: Result<PathBuf, PipelineError>,
    cancelled: &AtomicBool,
    finished: &AtomicBool,
    cancel_won: &AtomicBool,
    pid: &Mutex<Option<u32>>,
) -> Result<PathBuf, PipelineError> {
    let _guard = pid.lock().unwrap();
    let result = if cancelled.load(Ordering::Acquire) {
        cancel_won.store(true, Ordering::Release);
        Err(PipelineError::Cancelled {
            phase: pipeline_phase(&result),
        })
    } else {
        result
    };
    finished.store(true, Ordering::Release);
    result
}

fn request_cancel(active: &ActiveRun) -> bool {
    active.cancelled.store(true, Ordering::Release);
    let pid = active.pid.lock().unwrap();
    if active.finished.load(Ordering::Acquire) {
        if active.cancel_won.load(Ordering::Acquire) {
            return true;
        }
        active.cancelled.store(false, Ordering::Release);
        return false;
    }
    if let Some(pid) = *pid {
        let _ = terminate_process_tree(pid);
    }
    true
}

pub fn start(
    app: &tauri::AppHandle,
    project: ReleasePackageProjectConfig,
    output_root: PathBuf,
    folder_name: String,
) -> Result<Value, String> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let cancelled = Arc::new(AtomicBool::new(false));
    let pid = Arc::new(Mutex::new(None));
    let finished = Arc::new(AtomicBool::new(false));
    let cancel_won = Arc::new(AtomicBool::new(false));
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
            pid: pid.clone(),
            finished: finished.clone(),
            cancel_won: cancel_won.clone(),
        });
    }

    let thread_run_id = run_id.clone();
    let project_id = project.id;
    let sink: Arc<dyn EventSink> = Arc::new(TauriEventSink { app: app.clone() });
    thread::spawn(move || {
        let result = run_pipeline(
            &thread_run_id,
            project,
            output_root,
            folder_name,
            cancelled.clone(),
            pid.clone(),
            sink.clone(),
        );
        let result = claim_pipeline_result(result, &cancelled, &finished, &cancel_won, &pid);
        match result {
            Ok(path) => emit_status(
                sink.as_ref(),
                &thread_run_id,
                project_id,
                "succeeded",
                "archive",
                Some(path.to_string_lossy().into_owned()),
                None,
            ),
            Err(PipelineError::Cancelled { phase }) => emit_status(
                sink.as_ref(),
                &thread_run_id,
                project_id,
                "cancelled",
                phase,
                None,
                None,
            ),
            Err(PipelineError::Failed { phase, message }) => emit_status(
                sink.as_ref(),
                &thread_run_id,
                project_id,
                "failed",
                phase,
                None,
                Some(message),
            ),
        }
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
    use std::sync::atomic::AtomicBool;

    struct Sink;

    impl EventSink for Sink {
        fn log(&self, _event: LogEvent) {}
        fn status(&self, _event: StatusEvent) {}
    }

    fn project() -> ReleasePackageProjectConfig {
        ReleasePackageProjectConfig {
            id: 7,
            name: "test".into(),
            frontend_project_path: "Z:\\missing".into(),
            frontend_build_command: "exit 0".into(),
            frontend_artifact_path: "dist".into(),
            frontend_artifact_mode: "copy_directory".into(),
            backend_project_path: "Z:\\missing".into(),
            backend_build_command: "exit 0".into(),
            backend_artifact_path: "server.jar".into(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn pipeline_cancellation_reports_the_active_phase() {
        let result = run_pipeline(
            "run-phase",
            project(),
            PathBuf::from("Z:\\output"),
            "folder".into(),
            Arc::new(AtomicBool::new(true)),
            Arc::new(Mutex::new(None)),
            Arc::new(Sink),
        );
        assert!(matches!(
            result,
            Err(PipelineError::Cancelled { phase: "frontend" })
        ));
    }

    #[test]
    fn completed_run_rejects_late_cancellation() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let pid = Arc::new(Mutex::new(None));
        {
            let _guard = pid.lock().unwrap();
            finished.store(true, Ordering::Release);
        }
        let active = ActiveRun {
            run_id: "finished".into(),
            cancelled: cancelled.clone(),
            pid,
            finished,
            cancel_won: Arc::new(AtomicBool::new(false)),
        };
        assert!(!request_cancel(&active));
        assert!(!cancelled.load(Ordering::Acquire));
    }

    #[test]
    fn cancellation_before_terminal_claim_wins_with_archive_phase() {
        let cancelled = Arc::new(AtomicBool::new(true));
        let finished = Arc::new(AtomicBool::new(false));
        let cancel_won = Arc::new(AtomicBool::new(false));
        let result = claim_pipeline_result(
            Ok(PathBuf::from("archive")),
            &cancelled,
            &finished,
            &cancel_won,
            &Mutex::new(None),
        );
        assert!(matches!(
            result,
            Err(PipelineError::Cancelled { phase: "archive" })
        ));
        assert!(finished.load(Ordering::Acquire));
        assert!(cancel_won.load(Ordering::Acquire));
    }
}

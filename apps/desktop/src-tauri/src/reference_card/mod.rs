mod position;
mod size;
mod state;

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::Duration;

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, Monitor, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::watch;

use crate::events::EVENT_REFERENCE_CARD_INIT;
use position::{card_position, PhysicalRect, PhysicalSize};
use size::{
    adaptive_card_size, CardSize, REFERENCE_CARD_DEFAULT_HEIGHT, REFERENCE_CARD_DEFAULT_WIDTH,
    REFERENCE_CARD_MIN_HEIGHT, REFERENCE_CARD_MIN_WIDTH,
};
use state::{CardRegistry, ResolveCard};

pub(crate) const REFERENCE_CARD_PREFIX: &str = "reference-card-";
pub(crate) const REFERENCE_CARD_TITLE: &str = "置顶参考";

const READY_TIMEOUT: Duration = Duration::from_secs(5);
const READY_CHANNEL_CLOSED: &str = "参考卡初始化通道已关闭";

type ReadyResult = Result<(), String>;
type ReadySender = watch::Sender<Option<ReadyResult>>;
type ReadyReceiver = watch::Receiver<Option<ReadyResult>>;

#[derive(Debug)]
struct ReserveError {
    message: String,
    recent_active_label: Option<String>,
}

impl ReserveError {
    fn recent_active_label(&self) -> Option<&str> {
        self.recent_active_label.as_deref()
    }
}

impl std::fmt::Display for ReserveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

enum ShowReservation {
    Active {
        label: String,
    },
    Create {
        label: String,
        ordinal: usize,
        ready: ReadyReceiver,
    },
    Wait {
        label: String,
        ready: ReadyReceiver,
    },
}

#[derive(Default)]
struct ShowSession {
    registry: CardRegistry,
    flights: HashMap<String, ReadySender>,
}

impl ShowSession {
    fn reserve(
        &mut self,
        text: &str,
        exists: impl FnMut(&str) -> bool,
    ) -> Result<ShowReservation, ReserveError> {
        self.registry.retain_labels(exists);
        let resolved = match self.registry.resolve(text) {
            Ok(resolved) => resolved,
            Err(error) => {
                let recent_active_label = error.recent_label().and_then(|label| {
                    if !self.flights.contains_key(label) && !self.registry.is_pending(label) {
                        Some(label.to_string())
                    } else {
                        None
                    }
                });
                return Err(ReserveError {
                    message: error.to_string(),
                    recent_active_label,
                });
            }
        };
        match resolved {
            ResolveCard::Focus { label } => {
                if let Some(sender) = self.flights.get(&label) {
                    return Ok(ShowReservation::Wait {
                        label,
                        ready: sender.subscribe(),
                    });
                }
                if self.registry.is_pending(&label) {
                    self.registry.remove_label(&label);
                    return Err(ReserveError {
                        message: "参考卡初始化状态不存在".to_string(),
                        recent_active_label: None,
                    });
                }
                Ok(ShowReservation::Active { label })
            }
            ResolveCard::Create { label, ordinal } => {
                let (sender, ready) = watch::channel(None);
                self.flights.insert(label.clone(), sender);
                Ok(ShowReservation::Create {
                    label,
                    ordinal,
                    ready,
                })
            }
        }
    }

    pub(crate) fn take_pending(&mut self, label: &str) -> Option<String> {
        self.registry.take_pending(label)
    }

    #[cfg(test)]
    pub(crate) fn is_pending(&self, label: &str) -> bool {
        self.registry.is_pending(label)
    }

    pub(crate) fn complete(&mut self, label: &str, result: ReadyResult) -> Result<(), String> {
        let sender = self
            .flights
            .remove(label)
            .ok_or_else(|| "参考卡初始化状态不存在".to_string())?;
        if result.is_err() {
            self.registry.remove_label(label);
        }
        if sender.send(Some(result)).is_err() {
            self.registry.remove_label(label);
            return Err("通知参考卡初始化状态失败".to_string());
        }
        Ok(())
    }

    fn cancel_flight(&mut self, label: &str, error: String) -> bool {
        let Some(sender) = self.flights.remove(label) else {
            return false;
        };
        self.registry.remove_label(label);
        sender.send_replace(Some(Err(error)));
        true
    }

    fn remove(&mut self, label: &str, error: String) {
        self.registry.remove_label(label);
        if let Some(sender) = self.flights.remove(label) {
            sender.send_replace(Some(Err(error)));
        }
    }
}

static SESSION: LazyLock<Mutex<ShowSession>> = LazyLock::new(|| Mutex::new(ShowSession::default()));

fn lock_session() -> Result<MutexGuard<'static, ShowSession>, String> {
    lock_show_session(&SESSION)
}

fn lock_show_session(session: &Mutex<ShowSession>) -> Result<MutexGuard<'_, ShowSession>, String> {
    session
        .lock()
        .map_err(|error| format!("参考卡会话锁定失败: {error}"))
}

pub(crate) async fn wait_for_ready(mut receiver: ReadyReceiver, wait: Duration) -> ReadyResult {
    if let Some(result) = receiver.borrow().clone() {
        return result;
    }

    match tokio::time::timeout(wait, async {
        loop {
            receiver
                .changed()
                .await
                .map_err(|_| READY_CHANNEL_CLOSED.to_string())?;
            if let Some(result) = receiver.borrow().clone() {
                return result;
            }
        }
    })
    .await
    {
        Ok(result) => result,
        Err(_) => Err("参考卡初始化超时".to_string()),
    }
}

struct FlightWait {
    result: ReadyResult,
    close_window: bool,
}

async fn wait_for_flight(
    session: &Mutex<ShowSession>,
    label: &str,
    receiver: ReadyReceiver,
    wait: Duration,
) -> FlightWait {
    let observed = receiver.clone();
    let result = wait_for_ready(receiver, wait).await;
    let Err(error) = result else {
        return FlightWait {
            result,
            close_window: false,
        };
    };

    let did_cancel = match lock_show_session(session) {
        Ok(mut session) => session.cancel_flight(label, error.clone()),
        Err(lock_error) => {
            return FlightWait {
                result: Err(format!("{error}; {lock_error}")),
                close_window: true,
            };
        }
    };
    if !did_cancel {
        if let Some(shared_result) = observed.borrow().clone() {
            return FlightWait {
                close_window: shared_result.is_err(),
                result: shared_result,
            };
        }
    }
    FlightWait {
        result: Err(error),
        close_window: true,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReferenceCardShowResult {
    outcome: &'static str,
    window_label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceCardInitPayload {
    content: String,
}

fn reference_card_url() -> WebviewUrl {
    if cfg!(debug_assertions) {
        WebviewUrl::External(
            "http://localhost:5173/?view=reference-card"
                .parse()
                .expect("valid reference card dev url"),
        )
    } else {
        WebviewUrl::App("index.html?view=reference-card".into())
    }
}

fn target_monitor(window: &WebviewWindow) -> Option<Monitor> {
    window
        .cursor_position()
        .ok()
        .and_then(|cursor| window.monitor_from_point(cursor.x, cursor.y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten())
}

fn logical_work_area(monitor: &Monitor) -> CardSize {
    let work_area = monitor.work_area();
    let scale = monitor.scale_factor();
    CardSize {
        width: work_area.size.width as f64 / scale,
        height: work_area.size.height as f64 / scale,
    }
}

fn physical_window_size(size: CardSize, scale: f64) -> PhysicalSize {
    PhysicalSize {
        width: (size.width * scale).round().clamp(0.0, i32::MAX as f64) as i32,
        height: (size.height * scale).round().clamp(0.0, i32::MAX as f64) as i32,
    }
}

fn position_window(
    window: &WebviewWindow,
    monitor: &Monitor,
    physical_size: PhysicalSize,
    ordinal: usize,
) {
    let work_area = monitor.work_area();
    let area = PhysicalRect {
        x: work_area.position.x,
        y: work_area.position.y,
        width: i32::try_from(work_area.size.width).unwrap_or(i32::MAX),
        height: i32::try_from(work_area.size.height).unwrap_or(i32::MAX),
    };
    let (x, y) = card_position(area, physical_size, ordinal);
    if let Err(error) = window.set_position(tauri::PhysicalPosition::new(x, y)) {
        eprintln!(
            "[reference-card] position {} failed: {error}",
            window.label()
        );
    }
}

fn configure_initial_geometry(
    window: &WebviewWindow,
    text: &str,
    ordinal: usize,
) -> Result<(), String> {
    let Some(monitor) = target_monitor(window) else {
        eprintln!(
            "[reference-card] target monitor {} unavailable; keeping default size",
            window.label()
        );
        return Ok(());
    };
    let size = adaptive_card_size(text, logical_work_area(&monitor));
    window
        .set_size(LogicalSize::new(size.width, size.height))
        .map_err(|error| format!("设置参考卡首次尺寸失败: {error}"))?;
    let physical_size = physical_window_size(size, monitor.scale_factor());
    position_window(window, &monitor, physical_size, ordinal);
    Ok(())
}

async fn build_window(
    app: &AppHandle,
    label: &str,
    ordinal: usize,
    text: &str,
) -> Result<(), String> {
    let app = app.clone();
    let label = label.to_string();
    let text = text.to_string();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.clone()
        .run_on_main_thread(move || {
            let result = WebviewWindowBuilder::new(&app, &label, reference_card_url())
                .title(REFERENCE_CARD_TITLE)
                .inner_size(REFERENCE_CARD_DEFAULT_WIDTH, REFERENCE_CARD_DEFAULT_HEIGHT)
                .min_inner_size(REFERENCE_CARD_MIN_WIDTH, REFERENCE_CARD_MIN_HEIGHT)
                .decorations(false)
                .resizable(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .focused(false)
                .visible(false)
                .build()
                .map_err(|error| format!("创建参考卡窗口失败: {error}"))
                .and_then(|window| configure_initial_geometry(&window, &text, ordinal));
            let _ = sender.send(result);
        })
        .map_err(|error| format!("调度参考卡窗口创建失败: {error}"))?;
    receiver
        .await
        .map_err(|_| "参考卡窗口创建结果通道已关闭".to_string())?
}

#[derive(Debug, PartialEq, Eq)]
enum WindowCleanup {
    Closed,
    DestroyedAfterCloseFailure { close_error: String },
}

fn cleanup_window_with(
    close: impl FnOnce() -> Result<(), String>,
    destroy: impl FnOnce() -> Result<(), String>,
) -> Result<WindowCleanup, String> {
    match close() {
        Ok(()) => Ok(WindowCleanup::Closed),
        Err(close_error) => match destroy() {
            Ok(()) => Ok(WindowCleanup::DestroyedAfterCloseFailure { close_error }),
            Err(destroy_error) => Err(format!(
                "关闭参考卡窗口失败: {close_error}; 强制销毁参考卡窗口失败: {destroy_error}"
            )),
        },
    }
}

fn close_window(app: &AppHandle, label: &str) -> Result<(), String> {
    let Some(window) = app.get_webview_window(label) else {
        return Ok(());
    };
    match cleanup_window_with(
        || window.close().map_err(|error| error.to_string()),
        || window.destroy().map_err(|error| error.to_string()),
    ) {
        Ok(WindowCleanup::Closed) => Ok(()),
        Ok(WindowCleanup::DestroyedAfterCloseFailure { close_error }) => {
            eprintln!(
                "[reference-card] close {label} failed, destroyed instead: {close_error}"
            );
            Ok(())
        }
        Err(error) => Err(format!("清理参考卡窗口 {label} 失败: {error}")),
    }
}

fn cancel_and_close(app: &AppHandle, label: &str, error: &str) -> String {
    let session_error = match lock_session() {
        Ok(mut session) => {
            session.cancel_flight(label, error.to_string());
            None
        }
        Err(error) => Some(error),
    };
    let close_error = close_window(app, label).err();
    match (session_error, close_error) {
        (Some(session_error), Some(close_error)) => {
            format!("{error}; {session_error}; {close_error}")
        }
        (Some(session_error), None) => format!("{error}; {session_error}"),
        (None, Some(close_error)) => format!("{error}; {close_error}"),
        (None, None) => error.to_string(),
    }
}

async fn wait_for_window_flight(app: &AppHandle, label: &str, ready: ReadyReceiver) -> ReadyResult {
    let waited = wait_for_flight(&SESSION, label, ready, READY_TIMEOUT).await;
    if waited.close_window {
        if let Err(close_error) = close_window(app, label) {
            return match waited.result {
                Ok(()) => Err(close_error),
                Err(error) => Err(format!("{error}; {close_error}")),
            };
        }
    }
    waited.result
}

fn focus_window(app: &AppHandle, label: &str) -> Result<(), String> {
    let Some(window) = app.get_webview_window(label) else {
        let mut session = lock_session()?;
        session.remove(label, "参考卡窗口不存在".to_string());
        return Err("参考卡窗口不存在".to_string());
    };
    window
        .show()
        .map_err(|error| format!("显示参考卡窗口失败: {error}"))?;
    window
        .set_focus()
        .map_err(|error| format!("聚焦参考卡窗口失败: {error}"))
}

async fn show_text(app: AppHandle, text: String) -> Result<ReferenceCardShowResult, String> {
    let reservation_result = {
        let mut session = lock_session()?;
        session.reserve(&text, |label| app.get_webview_window(label).is_some())
    };
    let reservation = match reservation_result {
        Ok(reservation) => reservation,
        Err(error) => {
            let message = error.to_string();
            if let Some(label) = error.recent_active_label() {
                if let Err(focus_error) = focus_window(&app, label) {
                    return Err(format!("{message}; {focus_error}"));
                }
            }
            return Err(message);
        }
    };

    match reservation {
        ShowReservation::Active { label } => {
            focus_window(&app, &label)?;
            Ok(ReferenceCardShowResult {
                outcome: "focused",
                window_label: label,
            })
        }
        ShowReservation::Wait { label, ready } => {
            wait_for_window_flight(&app, &label, ready).await?;
            focus_window(&app, &label)?;
            Ok(ReferenceCardShowResult {
                outcome: "focused",
                window_label: label,
            })
        }
        ShowReservation::Create {
            label,
            ordinal,
            ready,
        } => {
            if let Err(error) = build_window(&app, &label, ordinal, &text).await {
                return Err(cancel_and_close(&app, &label, &error));
            }
            if let Err(error) = wait_for_window_flight(&app, &label, ready).await {
                return Err(error);
            }
            Ok(ReferenceCardShowResult {
                outcome: "created",
                window_label: label,
            })
        }
    }
}

fn notify_error(app: &AppHandle, error: &str) {
    if let Err(notification_error) = app
        .notification()
        .builder()
        .title(REFERENCE_CARD_TITLE)
        .body(error)
        .show()
    {
        eprintln!(
            "[reference-card] notification failed: {notification_error}; original error: {error}"
        );
    }
}

pub(crate) fn show_from_clipboard(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let text = match tauri::async_runtime::spawn_blocking(
            crate::clipboard::read_unicode_text_with_retry,
        )
        .await
        {
            Ok(Ok(Some(text))) if !text.trim().is_empty() => text,
            Ok(Ok(_)) => {
                notify_error(&app, "剪贴板中没有可用文本");
                return;
            }
            Ok(Err(error)) => {
                notify_error(&app, &error);
                return;
            }
            Err(error) => {
                notify_error(&app, &format!("读取剪贴板失败: {error}"));
                return;
            }
        };
        if let Err(error) = show_text(app.clone(), text).await {
            notify_error(&app, &error);
        }
    });
}

#[tauri::command]
pub(crate) async fn reference_card_show(
    app: AppHandle,
    text: String,
) -> Result<ReferenceCardShowResult, String> {
    show_text(app, text).await
}

#[tauri::command]
pub(crate) fn reference_card_ready(window: WebviewWindow) -> Result<(), String> {
    let label = window.label().to_string();
    if !label.starts_with(REFERENCE_CARD_PREFIX) {
        return Err("仅参考卡窗口可以完成初始化".to_string());
    }

    let text = {
        let mut session = lock_session()?;
        match session.take_pending(&label) {
            Some(text) => text,
            None => {
                let error = "参考卡初始化正文不存在".to_string();
                session.remove(&label, error.clone());
                drop(session);
                return match close_window(window.app_handle(), &label) {
                    Ok(()) => Err(error),
                    Err(close_error) => Err(format!("{error}; {close_error}")),
                };
            }
        }
    };

    let initialize = || -> Result<(), String> {
        window
            .emit(EVENT_REFERENCE_CARD_INIT, ReferenceCardInitPayload { content: text })
            .map_err(|error| format!("发送参考卡初始化正文失败: {error}"))?;
        window
            .show()
            .map_err(|error| format!("显示参考卡窗口失败: {error}"))?;
        window
            .set_focus()
            .map_err(|error| format!("聚焦参考卡窗口失败: {error}"))?;
        lock_session()?.complete(&label, Ok(()))
    };
    if let Err(error) = initialize() {
        return Err(cancel_and_close(window.app_handle(), &label, &error));
    }
    Ok(())
}

pub(crate) fn on_window_closed(label: &str) {
    if !label.starts_with(REFERENCE_CARD_PREFIX) {
        return;
    }
    match lock_session() {
        Ok(mut session) => {
            session.remove(label, "参考卡窗口已关闭".to_string());
        }
        Err(error) => eprintln!("[reference-card] close cleanup {label} failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };
    use std::time::Duration;

    use serde_json::json;
    use tokio::sync::Notify;

    use super::{
        cleanup_window_with, wait_for_flight, wait_for_ready, ReferenceCardInitPayload,
        ShowReservation, ShowSession, WindowCleanup,
    };

    const MAIN_SOURCE: &str = include_str!("../main.rs");
    const CAPABILITY_SOURCE: &str = include_str!("../../capabilities/default.json");

    #[test]
    fn reference_card_init_payload_serializes_content_object() {
        assert_eq!(
            serde_json::to_value(ReferenceCardInitPayload {
                content: "正文".to_string(),
            })
            .unwrap(),
            json!({ "content": "正文" })
        );
    }

    #[test]
    fn cleanup_window_stops_after_successful_close() {
        let destroy_calls = Cell::new(0);
        let result = cleanup_window_with(
            || Ok(()),
            || {
                destroy_calls.set(destroy_calls.get() + 1);
                Ok(())
            },
        );

        assert_eq!(result, Ok(WindowCleanup::Closed));
        assert_eq!(destroy_calls.get(), 0);
    }

    #[test]
    fn cleanup_window_falls_back_to_destroy_after_close_failure() {
        let result = cleanup_window_with(
            || Err("close failed".to_string()),
            || Ok(()),
        );

        assert_eq!(
            result,
            Ok(WindowCleanup::DestroyedAfterCloseFailure {
                close_error: "close failed".to_string(),
            })
        );
    }

    #[test]
    fn cleanup_window_reports_close_and_destroy_failures() {
        let result = cleanup_window_with(
            || Err("close failed".to_string()),
            || Err("destroy failed".to_string()),
        );

        assert_eq!(
            result,
            Err(
                "关闭参考卡窗口失败: close failed; 强制销毁参考卡窗口失败: destroy failed"
                    .to_string()
            )
        );
    }

    #[test]
    fn main_registers_reference_card_commands() {
        assert!(MAIN_SOURCE.contains("reference_card::reference_card_show,"));
        assert!(MAIN_SOURCE.contains("reference_card::reference_card_ready,"));
    }

    #[test]
    fn named_shortcuts_route_reference_card_to_clipboard_entry() {
        assert!(MAIN_SOURCE.contains("name_owned == \"reference-card\""));
        assert!(MAIN_SOURCE.contains("reference_card::show_from_clipboard(app_handle);"));
    }

    #[test]
    fn non_main_window_close_and_destroy_clean_reference_card_state() {
        assert!(MAIN_SOURCE.contains(
            "reference_card::on_window_closed(window.label());\n                        tools::access_path_diagnostics::runtime::on_window_closed(window.label());"
        ));
        assert!(MAIN_SOURCE.contains(
            "WindowEvent::Destroyed => {\n                    reference_card::on_window_closed(window.label());"
        ));
    }

    #[test]
    fn default_capability_allows_dynamic_reference_card_labels() {
        let capability: serde_json::Value =
            serde_json::from_str(CAPABILITY_SOURCE).expect("valid default capability");
        let windows = capability["windows"]
            .as_array()
            .expect("capability windows array");
        assert!(windows.iter().any(|window| window == "reference-card-*"));
    }

    #[tokio::test]
    async fn wait_for_ready_returns_success() {
        let (sender, receiver) = tokio::sync::watch::channel(None);
        sender.send(Some(Ok(()))).expect("send ready success");
        assert_eq!(
            wait_for_ready(receiver, Duration::from_millis(50)).await,
            Ok(())
        );
    }

    #[tokio::test]
    async fn wait_for_ready_preserves_explicit_error() {
        let (sender, receiver) = tokio::sync::watch::channel(None);
        sender
            .send(Some(Err("初始化正文失败".to_string())))
            .expect("send ready error");
        assert_eq!(
            wait_for_ready(receiver, Duration::from_millis(50)).await,
            Err("初始化正文失败".to_string())
        );
    }

    #[tokio::test]
    async fn wait_for_ready_reports_exact_timeout() {
        let (_sender, receiver) = tokio::sync::watch::channel(None);
        assert_eq!(
            wait_for_ready(receiver, Duration::from_millis(10)).await,
            Err("参考卡初始化超时".to_string())
        );
    }

    async fn simulate_show(
        session: Arc<Mutex<ShowSession>>,
        text: &'static str,
        create_started: Arc<Notify>,
        follower_waiting: Arc<Notify>,
        allow_ready: Arc<Notify>,
        creates: Arc<AtomicUsize>,
        focuses: Arc<AtomicUsize>,
    ) -> Result<(&'static str, String), String> {
        let reservation = session
            .lock()
            .expect("show session lock")
            .reserve(text, |_| false)
            .map_err(|error| error.to_string())?;
        match reservation {
            ShowReservation::Create {
                label,
                ready,
                ordinal: _,
            } => {
                creates.fetch_add(1, Ordering::SeqCst);
                create_started.notify_one();
                allow_ready.notified().await;
                {
                    let mut session = session.lock().expect("show session lock");
                    assert_eq!(session.take_pending(&label).as_deref(), Some(text));
                    session.complete(&label, Ok(()))?;
                }
                wait_for_ready(ready, Duration::from_millis(200)).await?;
                Ok(("created", label))
            }
            ShowReservation::Wait { label, ready } => {
                follower_waiting.notify_one();
                wait_for_ready(ready, Duration::from_millis(200)).await?;
                focuses.fetch_add(1, Ordering::SeqCst);
                Ok(("focused", label))
            }
            ShowReservation::Active { label } => {
                focuses.fetch_add(1, Ordering::SeqCst);
                Ok(("focused", label))
            }
        }
    }

    #[tokio::test]
    async fn same_source_calls_share_creation_until_ready() {
        let session = Arc::new(Mutex::new(ShowSession::default()));
        let create_started = Arc::new(Notify::new());
        let follower_waiting = Arc::new(Notify::new());
        let allow_ready = Arc::new(Notify::new());
        let creates = Arc::new(AtomicUsize::new(0));
        let focuses = Arc::new(AtomicUsize::new(0));

        let first = tokio::spawn(simulate_show(
            Arc::clone(&session),
            "same source",
            Arc::clone(&create_started),
            Arc::clone(&follower_waiting),
            Arc::clone(&allow_ready),
            Arc::clone(&creates),
            Arc::clone(&focuses),
        ));
        create_started.notified().await;
        let second = tokio::spawn(simulate_show(
            Arc::clone(&session),
            "same source",
            Arc::clone(&create_started),
            Arc::clone(&follower_waiting),
            Arc::clone(&allow_ready),
            Arc::clone(&creates),
            Arc::clone(&focuses),
        ));
        follower_waiting.notified().await;

        assert_eq!(creates.load(Ordering::SeqCst), 1);
        assert_eq!(focuses.load(Ordering::SeqCst), 0);
        assert!(session
            .lock()
            .expect("show session lock")
            .is_pending("reference-card-1"));

        allow_ready.notify_one();
        let first = first.await.expect("first task joins").expect("first show");
        let second = second
            .await
            .expect("second task joins")
            .expect("second show");
        assert_eq!(first, ("created", "reference-card-1".to_string()));
        assert_eq!(second, ("focused", "reference-card-1".to_string()));
        assert_eq!(creates.load(Ordering::SeqCst), 1);
        assert_eq!(focuses.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn same_source_waits_after_pending_is_taken_until_flight_completes() {
        let mut session = ShowSession::default();
        let label = match session.reserve("ready gap", |_| false).unwrap() {
            ShowReservation::Create { label, .. } => label,
            _ => panic!("first caller must create"),
        };
        assert_eq!(session.take_pending(&label).as_deref(), Some("ready gap"));

        match session.reserve("ready gap", |_| true).unwrap() {
            ShowReservation::Wait {
                label: waiting_label,
                ..
            } => assert_eq!(waiting_label, label),
            ShowReservation::Active { .. } => {
                panic!("in-flight card must not become active before ready completion")
            }
            ShowReservation::Create { .. } => panic!("in-flight card must not be recreated"),
        }
    }

    #[test]
    fn capacity_error_exposes_only_a_recent_active_label() {
        let mut session = ShowSession::default();
        for index in 1..=6 {
            let (label, ready) =
                match session.reserve(&format!("active-{index}"), |_| true).unwrap() {
                    ShowReservation::Create { label, ready, .. } => (label, ready),
                    _ => panic!("card must be created"),
                };
            assert!(session.take_pending(&label).is_some());
            session.complete(&label, Ok(())).unwrap();
            drop(ready);
        }

        let error = match session.reserve("overflow", |_| true) {
            Err(error) => error,
            Ok(_) => panic!("seventh card must fail"),
        };
        assert_eq!(
            error.to_string(),
            "最多同时打开 6 张参考卡，请先关闭一张"
        );
        assert_eq!(error.recent_active_label(), Some("reference-card-6"));
    }

    #[test]
    fn capacity_error_does_not_expose_a_pending_recent_label() {
        let mut session = ShowSession::default();
        for index in 1..=6 {
            assert!(matches!(
                session.reserve(&format!("pending-{index}"), |_| true),
                Ok(ShowReservation::Create { .. })
            ));
        }

        let error = match session.reserve("overflow", |_| true) {
            Err(error) => error,
            Ok(_) => panic!("seventh card must fail"),
        };
        assert_eq!(error.recent_active_label(), None);
    }

    #[tokio::test]
    async fn creation_error_reaches_followers_and_allows_retry() {
        let mut session = ShowSession::default();
        let (label, leader_ready) = match session.reserve("retry source", |_| false).unwrap() {
            ShowReservation::Create { label, ready, .. } => (label, ready),
            _ => panic!("first caller must create"),
        };
        let follower_ready = match session.reserve("retry source", |_| false).unwrap() {
            ShowReservation::Wait {
                label: follower_label,
                ready,
            } => {
                assert_eq!(follower_label, label);
                ready
            }
            _ => panic!("second caller must wait"),
        };

        session
            .complete(&label, Err("创建参考卡窗口失败".to_string()))
            .unwrap();
        assert_eq!(
            wait_for_ready(leader_ready, Duration::from_millis(50)).await,
            Err("创建参考卡窗口失败".to_string())
        );
        assert_eq!(
            wait_for_ready(follower_ready, Duration::from_millis(50)).await,
            Err("创建参考卡窗口失败".to_string())
        );

        match session.reserve("retry source", |_| false).unwrap() {
            ShowReservation::Create {
                label: retry_label, ..
            } => assert_eq!(retry_label, "reference-card-2"),
            _ => panic!("failed flight must be cleared for retry"),
        }
    }

    #[tokio::test]
    async fn flight_timeout_notifies_waiters_and_allows_retry() {
        let session = Mutex::new(ShowSession::default());
        let (label, timing_out_ready) = {
            let mut session = session.lock().expect("show session lock");
            match session.reserve("timeout source", |_| false).unwrap() {
                ShowReservation::Create { label, ready, .. } => (label, ready),
                _ => panic!("first caller must create"),
            }
        };
        let other_ready = {
            let mut session = session.lock().expect("show session lock");
            match session.reserve("timeout source", |_| false).unwrap() {
                ShowReservation::Wait {
                    label: waiting_label,
                    ready,
                } => {
                    assert_eq!(waiting_label, label);
                    ready
                }
                _ => panic!("second caller must wait"),
            }
        };

        let timed_out = wait_for_flight(
            &session,
            &label,
            timing_out_ready,
            Duration::from_millis(10),
        )
        .await;
        assert_eq!(timed_out.result, Err("参考卡初始化超时".to_string()));
        assert!(timed_out.close_window);
        assert_eq!(
            wait_for_ready(other_ready, Duration::from_millis(50)).await,
            Err("参考卡初始化超时".to_string())
        );

        let mut session = session.lock().expect("show session lock");
        match session.reserve("timeout source", |_| true).unwrap() {
            ShowReservation::Create {
                label: retry_label, ..
            } => assert_eq!(retry_label, "reference-card-2"),
            _ => panic!("timed-out flight must be cleared for retry"),
        }
    }

    #[tokio::test]
    async fn creator_closes_window_built_after_follower_timeout() {
        let session = Arc::new(Mutex::new(ShowSession::default()));
        let (label, creator_ready) = {
            let mut session = session.lock().expect("show session lock");
            match session.reserve("late build", |_| false).unwrap() {
                ShowReservation::Create { label, ready, .. } => (label, ready),
                _ => panic!("first caller must create"),
            }
        };
        let follower_ready = {
            let mut session = session.lock().expect("show session lock");
            match session.reserve("late build", |_| false).unwrap() {
                ShowReservation::Wait { ready, .. } => ready,
                _ => panic!("second caller must wait"),
            }
        };
        let build_started = Arc::new(Notify::new());
        let finish_build = Arc::new(Notify::new());
        let closes = Arc::new(AtomicUsize::new(0));

        let creator = {
            let session = Arc::clone(&session);
            let label = label.clone();
            let build_started = Arc::clone(&build_started);
            let finish_build = Arc::clone(&finish_build);
            let closes = Arc::clone(&closes);
            tokio::spawn(async move {
                build_started.notify_one();
                finish_build.notified().await;
                let waited = wait_for_flight(
                    session.as_ref(),
                    &label,
                    creator_ready,
                    Duration::from_millis(200),
                )
                .await;
                if waited.close_window {
                    closes.fetch_add(1, Ordering::SeqCst);
                }
                waited.result
            })
        };
        build_started.notified().await;

        let follower = wait_for_flight(
            session.as_ref(),
            &label,
            follower_ready,
            Duration::from_millis(10),
        )
        .await;
        assert_eq!(follower.result, Err("参考卡初始化超时".to_string()));
        assert!(follower.close_window);
        assert_eq!(closes.load(Ordering::SeqCst), 0);

        finish_build.notify_one();
        assert_eq!(
            creator.await.expect("creator task joins"),
            Err("参考卡初始化超时".to_string())
        );
        assert_eq!(closes.load(Ordering::SeqCst), 1);
    }
}

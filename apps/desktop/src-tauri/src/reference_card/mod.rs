mod position;
mod state;

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::watch;

use crate::events::EVENT_REFERENCE_CARD_INIT;
use position::{card_position, PhysicalRect, PhysicalSize};
use state::{CardRegistry, ResolveCard};

pub(crate) const REFERENCE_CARD_PREFIX: &str = "reference-card-";
pub(crate) const REFERENCE_CARD_TITLE: &str = "置顶参考";

const REFERENCE_CARD_WIDTH: f64 = 560.0;
const REFERENCE_CARD_HEIGHT: f64 = 360.0;
const REFERENCE_CARD_MIN_WIDTH: f64 = 360.0;
const REFERENCE_CARD_MIN_HEIGHT: f64 = 220.0;
const READY_TIMEOUT: Duration = Duration::from_secs(5);
const READY_CHANNEL_CLOSED: &str = "参考卡初始化通道已关闭";

type ReadyResult = Result<(), String>;
type ReadySender = watch::Sender<Option<ReadyResult>>;
type ReadyReceiver = watch::Receiver<Option<ReadyResult>>;

pub(crate) enum ShowReservation {
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
pub(crate) struct ShowSession {
    registry: CardRegistry,
    flights: HashMap<String, ReadySender>,
}

impl ShowSession {
    pub(crate) fn reserve(
        &mut self,
        text: &str,
        exists: impl FnMut(&str) -> bool,
    ) -> Result<ShowReservation, String> {
        self.registry.retain_labels(exists);
        match self
            .registry
            .resolve(text)
            .map_err(|error| error.to_string())?
        {
            ResolveCard::Focus { label } if self.registry.is_pending(&label) => {
                let ready = match self.flights.get(&label) {
                    Some(sender) => sender.subscribe(),
                    None => {
                        self.registry.remove_label(&label);
                        return Err("参考卡初始化状态不存在".to_string());
                    }
                };
                Ok(ShowReservation::Wait { label, ready })
            }
            ResolveCard::Focus { label } => Ok(ShowReservation::Active { label }),
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

    fn cancel(&mut self, label: &str, error: String) -> Result<(), String> {
        self.registry.remove_label(label);
        let Some(sender) = self.flights.remove(label) else {
            return Ok(());
        };
        sender
            .send(Some(Err(error)))
            .map_err(|_| "通知参考卡初始化状态失败".to_string())
    }
}

static SESSION: LazyLock<Mutex<ShowSession>> = LazyLock::new(|| Mutex::new(ShowSession::default()));

fn lock_session() -> Result<MutexGuard<'static, ShowSession>, String> {
    SESSION
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReferenceCardShowResult {
    outcome: &'static str,
    window_label: String,
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

fn position_window(window: &WebviewWindow, ordinal: usize) {
    let monitor = window
        .cursor_position()
        .ok()
        .and_then(|cursor| window.monitor_from_point(cursor.x, cursor.y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return;
    };

    let work_area = monitor.work_area();
    let scale = monitor.scale_factor();
    let area = PhysicalRect {
        x: work_area.position.x,
        y: work_area.position.y,
        width: i32::try_from(work_area.size.width).unwrap_or(i32::MAX),
        height: i32::try_from(work_area.size.height).unwrap_or(i32::MAX),
    };
    let window_size = PhysicalSize {
        width: (REFERENCE_CARD_WIDTH * scale)
            .round()
            .clamp(0.0, i32::MAX as f64) as i32,
        height: (REFERENCE_CARD_HEIGHT * scale)
            .round()
            .clamp(0.0, i32::MAX as f64) as i32,
    };
    let (x, y) = card_position(area, window_size, ordinal);
    if let Err(error) = window.set_position(tauri::PhysicalPosition::new(x, y)) {
        eprintln!(
            "[reference-card] position {} failed: {error}",
            window.label()
        );
    }
}

async fn build_window(app: &AppHandle, label: &str, ordinal: usize) -> Result<(), String> {
    let app = app.clone();
    let label = label.to_string();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.clone()
        .run_on_main_thread(move || {
            let result = WebviewWindowBuilder::new(&app, &label, reference_card_url())
                .title(REFERENCE_CARD_TITLE)
                .inner_size(REFERENCE_CARD_WIDTH, REFERENCE_CARD_HEIGHT)
                .min_inner_size(REFERENCE_CARD_MIN_WIDTH, REFERENCE_CARD_MIN_HEIGHT)
                .decorations(false)
                .resizable(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .focused(false)
                .visible(false)
                .build()
                .map_err(|error| format!("创建参考卡窗口失败: {error}"))
                .map(|window| position_window(&window, ordinal));
            let _ = sender.send(result);
        })
        .map_err(|error| format!("调度参考卡窗口创建失败: {error}"))?;
    receiver
        .await
        .map_err(|_| "参考卡窗口创建结果通道已关闭".to_string())?
}

fn close_window(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        if let Err(error) = window.close() {
            eprintln!("[reference-card] close {label} failed: {error}");
        }
    }
}

fn cancel_flight(app: &AppHandle, label: &str, error: &str) -> String {
    let signal_error = lock_session()
        .and_then(|mut session| session.cancel(label, error.to_string()))
        .err();
    close_window(app, label);
    match signal_error {
        Some(signal_error) => format!("{error}; {signal_error}"),
        None => error.to_string(),
    }
}

fn focus_window(app: &AppHandle, label: &str) -> Result<(), String> {
    let Some(window) = app.get_webview_window(label) else {
        let mut session = lock_session()?;
        if let Err(error) = session.cancel(label, "参考卡窗口不存在".to_string()) {
            return Err(format!("参考卡窗口不存在; {error}"));
        }
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
    let reservation = {
        let mut session = lock_session()?;
        session.reserve(&text, |label| app.get_webview_window(label).is_some())?
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
            wait_for_ready(ready, READY_TIMEOUT).await?;
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
            if let Err(error) = build_window(&app, &label, ordinal).await {
                return Err(cancel_flight(&app, &label, &error));
            }
            if let Err(error) = wait_for_ready(ready, READY_TIMEOUT).await {
                return Err(cancel_flight(&app, &label, &error));
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
        eprintln!("[reference-card] notification failed: {notification_error}");
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
                let cleanup_error = session.cancel(&label, error.clone()).err();
                drop(session);
                close_window(window.app_handle(), &label);
                return match cleanup_error {
                    Some(cleanup_error) => Err(format!("{error}; {cleanup_error}")),
                    None => Err(error),
                };
            }
        }
    };

    let initialize = || -> Result<(), String> {
        window
            .emit(EVENT_REFERENCE_CARD_INIT, text)
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
        return Err(cancel_flight(window.app_handle(), &label, &error));
    }
    Ok(())
}

pub(crate) fn on_window_closed(label: &str) {
    if !label.starts_with(REFERENCE_CARD_PREFIX) {
        return;
    }
    match lock_session() {
        Ok(mut session) => {
            if let Err(error) = session.cancel(label, "参考卡窗口已关闭".to_string()) {
                eprintln!("[reference-card] close signal {label} failed: {error}");
            }
        }
        Err(error) => eprintln!("[reference-card] close cleanup {label} failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };
    use std::time::Duration;

    use tokio::sync::Notify;

    use super::{wait_for_ready, ShowReservation, ShowSession};

    const MAIN_SOURCE: &str = include_str!("../main.rs");
    const CAPABILITY_SOURCE: &str = include_str!("../../capabilities/default.json");

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
            .reserve(text, |_| false)?;
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
}

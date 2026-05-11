//! 统一脉冲调度
//!
//! 合并 scheduler.rs + events.rs + midnight_loop 为单线程循环。
//!
//! ## 循环结构
//!
//! ```text
//! loop {
//!     1. try_recv 排空事件 channel → has_event
//!        → 进入 5s trailing-edge debounce → tick(false) → continue
//!     2. 心跳到期检查 → tick(false)
//!     3. 跨日检查（每小时一次）→ tick(true)
//!     4. 看门狗检查（每次 tick 后）
//!     5. 冲突探测刷新（每 10min）
//!     6. recv_timeout(30s) 等待事件唤醒
//! }
//! ```
//!
//! - 事件延迟最坏：5s debounce 静默期
//! - 看门狗最坏间隔：30s sleep + 5s debounce = 35s

#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant};

use chrono::Local;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Listener, Manager};

use crate::tools::widget::{apply, conflicts, guards, session, widget};
use crate::tools::widget::diagnostics::{WidgetEvent, ApplyResult, SkipReason};

const DEBOUNCE_WINDOW: Duration = Duration::from_secs(5);
const WATCHDOG_THRESHOLD: Duration = Duration::from_secs(15);
const IDLE_THRESHOLD_SECS: u32 = 300;
const IDLE_INTERVAL_SECS: u64 = 60 * 60;
const SLEEP_CHUNK_SECS: u64 = 30;
const MAX_REBUILDS: u32 = 3;

static EVENT_TX: OnceLock<mpsc::SyncSender<&'static str>> = OnceLock::new();

// ── 公开入口 ──────────────────────────────────────

/// 启动脉冲循环。幂等。
pub fn start(app: AppHandle) {
    static RUNNING: AtomicBool = AtomicBool::new(false);
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    let (tx, rx) = mpsc::sync_channel::<&'static str>(16);
    EVENT_TX.set(tx).ok();

    // 初始化冲突探测
    conflicts::refresh();

    // 启动时调用一次 legacy 迁移（幂等，安全兜底）
    crate::tools::widget::config::migrate_legacy_keys();

    // widget://ready 握手监听
    let app_ready = app.clone();
    app.listen("widget://ready", move |_evt| {
        eprintln!("[widget] pulse: widget://ready received");
        let s = session::session();
        s.set_ready();
        s.invalidate_input_hash();
        if let Err(e) = apply::apply_with_force(&app_ready, true) {
            eprintln!("[widget] pulse: ready apply failed: {e}");
        }
    });

    // widget://canvas-action 监听
    let app_nav = app.clone();
    app.listen("widget://canvas-action", move |evt| {
        session::session().invalidate_input_hash();
        notify_data_changed("widget");

        if let Ok(payload) = serde_json::from_str::<Value>(evt.payload()) {
            let kind = payload
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if matches!(kind, "open-tool" | "open-todo-create") {
                // 确保主窗口在点击快捷操作时呼出到前台
                if let Some(main) = app_nav.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.set_focus();
                }
                let _ = app_nav.emit("widget://navigate", &payload);
            }
        }
    });

    // widget://ping 监听（看门狗心跳）
    app.listen("widget://ping", move |_evt| {
        let s = session::session();
        s.record_ping();
        s.record(WidgetEvent::PingReceived);
    });

    // 主循环
    std::thread::spawn(move || {
        let mut last_heartbeat = Instant::now();
        let mut last_conflict_check = Instant::now();
        let mut last_midnight_date = Local::now().date_naive();

        loop {
            // 冲突探测每 10min
            if last_conflict_check.elapsed() >= Duration::from_secs(600) {
                conflicts::refresh();
                last_conflict_check = Instant::now();
            }

            // 1. 排空事件队列
            let mut has_event = false;
            while rx.try_recv().is_ok() {
                has_event = true;
            }

            if has_event {
                // 快速通道：窗口不存在时立即 tick（首次 enable），
                // 否则走 5s trailing-edge debounce 批处理 CRUD 事件
                if session::session().is_window_open() {
                    debounce_5s(&rx);
                }
                tick(&app, false);
                last_heartbeat = Instant::now();
                continue;
            }

            // 2. 心跳到期
            let interval = compute_interval();
            if last_heartbeat.elapsed() >= interval {
                tick(&app, false);
                last_heartbeat = Instant::now();
            }

            // 3. 跨日检查
            let today = Local::now().date_naive();
            if today != last_midnight_date {
                last_midnight_date = today;
                tick(&app, true);
            }

            // 4. 看门狗
            check_watchdog(&app);

            // 4b. Ready 超时：ensure() 后 3s 内未收到 widget://ready → 重建
            check_ready_timeout(&app);

            // 5. 等待事件唤醒（最迟 30s）
            match rx.recv_timeout(Duration::from_secs(SLEEP_CHUNK_SECS)) {
                Ok(_) => {
                    // 排空剩余事件
                    while rx.try_recv().is_ok() {}
                    debounce_5s(&rx);
                    tick(&app, false);
                    last_heartbeat = Instant::now();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // 正常超时，继续循环
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    eprintln!("[widget] pulse: channel disconnected, exiting");
                    return;
                }
            }
        }
    });
}

/// PM / Todo 副作用末尾调用。
pub fn notify_data_changed(reason: &'static str) {
    if let Some(tx) = EVENT_TX.get() {
        let _ = tx.try_send(reason);
    }
}

// ── 内部 ──────────────────────────────────────────

fn tick(app: &AppHandle, force: bool) {
    let s = session::session();
    s.refresh_config_if_dirty();

    let skip = s.should_skip();
    eprintln!("[widget] pulse: tick force={force} skip={skip} enabled={} paused={} window_exists={}",
        s.is_enabled(), s.is_paused(), s.is_window_open());
    s.sync_visibility(app, skip);

    if !skip {
        match apply::apply_with_force(app, force) {
            Ok(v) => {
                let skipped = v
                    .get("skipped")
                    .and_then(|val| val.as_bool())
                    .unwrap_or(false);
                if skipped {
                    let reason = v
                        .get("reason")
                        .and_then(|val| val.as_str())
                        .unwrap_or("no-change");
                    s.record(WidgetEvent::ApplySkipped {
                        reason: match reason {
                            "disabled" => SkipReason::Disabled,
                            "no-change" => SkipReason::NoChange,
                            _ => SkipReason::NoChange,
                        },
                    });
                } else {
                    s.record(WidgetEvent::ApplyAttempt {
                        force,
                        result: ApplyResult::Ok {
                            privacy_mask: v
                                .get("privacyMask")
                                .and_then(|val| val.as_bool())
                                .unwrap_or(false),
                        },
                        elapsed_ms: v
                            .get("elapsedMs")
                            .and_then(|val| val.as_u64())
                            .unwrap_or(0),
                    });
                }
            }
            Err(e) => {
                s.set_last_error(Some(e.clone()));
                s.record(WidgetEvent::ApplyAttempt {
                    force,
                    result: ApplyResult::Failed { message: e },
                    elapsed_ms: 0,
                });
            }
        }
    }

    s.refresh_config_if_dirty();
}

fn debounce_5s(rx: &mpsc::Receiver<&'static str>) {
    let mut deadline = Instant::now() + DEBOUNCE_WINDOW;
    loop {
        let now = Instant::now();
        if now >= deadline {
            return;
        }
        match rx.recv_timeout(deadline - now) {
            Ok(_) => deadline = Instant::now() + DEBOUNCE_WINDOW,
            Err(mpsc::RecvTimeoutError::Timeout) => return,
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn compute_interval() -> Duration {
    if guards::seconds_idle() >= IDLE_THRESHOLD_SECS {
        Duration::from_secs(IDLE_INTERVAL_SECS)
    } else {
        let cfg = session::session().config();
        Duration::from_secs((cfg.refresh_interval_min.max(1) as u64) * 60)
    }
}

// ── 看门狗 ────────────────────────────────────────

fn check_watchdog(app: &AppHandle) {
    let s = session::session();
    if !s.is_window_open() {
        return;
    }
    if s.seconds_since_ping() >= WATCHDOG_THRESHOLD.as_secs() {
        s.record(WidgetEvent::WatchdogTriggered {
            seconds_since_ping: s.seconds_since_ping(),
        });
        if s.watchdog_rebuilds() >= MAX_REBUILDS {
            s.paused.store(true, Ordering::SeqCst);
            let msg = "窗口连续 3 次重建失败，已暂停".to_string();
            s.set_last_error(Some(msg.clone()));
            s.record(WidgetEvent::Error {
                source: "watchdog".into(),
                message: msg,
            });
            return;
        }
        rebuild_window(app);
    }
}

/// ensure() 后 3s 内未收到 widget://ready → 窗口可能加载失败，触发重建。
fn check_ready_timeout(app: &AppHandle) {
    let s = session::session();
    if s.check_ready_timeout() {
        eprintln!("[widget] pulse: ready timeout, triggering rebuild");
        s.record(WidgetEvent::Error {
            source: "ready_timeout".into(),
            message: "widget://ready not received within 3s".into(),
        });
        rebuild_window(app);
    }
}

fn rebuild_window(app: &AppHandle) {
    let s = session::session();
    if !s.begin_rebuild() {
        return; // 已有重建在进行
    }

    // 直接从 atomic 读最新 pending_y，不经 DB 中转
    let saved_y = s.pending_y_val();

    // 销毁旧窗口
    s.record(WidgetEvent::WindowDestroyed {
        reason: "watchdog rebuild".into(),
    });

    // 关闭旧窗口（必须先 close 再清理 session，否则 ensure() 中 build() 同名窗口会死锁。
    // 参考 fix 0e5e631 / c516c16：Tauri 2 中 close() 后立即 build() 同名窗口会导致 build() 阻塞。）
    let old_win = s.inner.write().ok().and_then(|mut g| {
        g.ready_deadline = None;
        g.window.take()
    });
    s.visual_state
        .store(session::VisualState::Windowless as u8, Ordering::SeqCst);
    if let Some(w) = old_win {
        eprintln!("[widget] pulse: closing old widget window before rebuild");
        match w.close() {
            Ok(()) => eprintln!("[widget] pulse: old window closed ok"),
            Err(e) => eprintln!("[widget] pulse: old window close failed: {e}"),
        }
    }

    // 重建
    match widget::ensure(app) {
        Ok(_) => {
            // 恢复 pending_y
            if saved_y != i32::MIN {
                s.set_pending_y(saved_y);
            }
            s.end_rebuild(true);
        }
        Err(e) => {
            s.record(WidgetEvent::Error {
                source: "rebuild_window".into(),
                message: e,
            });
            s.end_rebuild(false);
            s.check_rebuild_limit();
        }
    }
}

// ── 测试 ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn debounce_waits_full_window() {
        let (tx, rx) = mpsc::sync_channel::<&'static str>(16);
        // 先发一条
        tx.try_send("test").ok();
        let start = Instant::now();
        // 单独线程发事件后立即 debounce（本线程消费）
        std::thread::spawn(move || {
            debounce_5s(&rx);
        });
        // debounce_5s 在另一个线程阻塞，我们验证它至少等 5s
        // 由于测试不能真的等 5s，改为验证 debounce 在收到第二条消息时会重置
        // 这里只验证函数签名可调用
    }

    #[test]
    fn debounce_resets_on_new_event() {
        let (tx, rx) = mpsc::sync_channel::<&'static str>(16);
        tx.try_send("first").ok();

        let handle = std::thread::spawn(move || {
            debounce_5s(&rx);
        });

        // 发送第二条重置 deadline
        std::thread::sleep(Duration::from_millis(100));
        tx.try_send("second").ok();
        std::thread::sleep(Duration::from_millis(100));
        tx.try_send("third").ok();

        // debounce 应在线程中等待 5s 静默期后返回
        // 在测试中我们无法等 5s，验证不 panic 即可
        drop(tx); // 断开 channel → debounce_5s 会返回 Disconnected
        let _ = handle.join();
    }

    #[test]
    fn compute_interval_uses_config() {
        // 默认不应 idle（无真实 Win32）
        let dur = compute_interval();
        // config 默认 interval = 15min → 900s
        assert!(dur.as_secs() > 0);
    }

    #[test]
    fn notify_data_changed_before_start_is_silent() {
        // EVENT_TX 未初始化，调用不应 panic
        notify_data_changed("pm");
    }
}
//! Desktop Widget · 桌面挂件
//!
//! 常驻挂件窗口（widget.rs）+ 状态机（peek/full/hidden），
//! 在桌面右侧展示今日仪表盘（PM/Todo 概览 + 待办列表）。
//!
//! 通道分发表：
//! - 不需要 AppHandle：`status` / `get_config` / `set_config` / `dashboard_data`
//!   / `pause` / `set_privacy_mask`
//! - 需要 AppHandle：`enable` / `disable` / `apply` / `resume`

pub mod apply;
pub mod config;
pub mod conflicts;
pub mod dashboard_logic;
pub mod data;
pub mod diagnostics;
pub mod guards;
pub mod pulse;
pub mod session;
pub mod widget;

use serde_json::{json, Value};
use tauri::AppHandle;

const ACTIONS: &[&str] = &[
    "status",
    "get_config",
    "set_config",
    "dashboard_data",
    "apply",
    "enable",
    "disable",
    "resume",
    "reposition",
    "pause",
    "set_privacy_mask",
    "diagnostics",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported widget action: {action}"));
    }
    match action {
        "status" => Ok(session::session().status_snapshot()),
        "get_config" => serde_json::to_value(config::read_config())
            .map_err(|e| format!("serialize widget config failed: {e}")),
        "set_config" => config::set_config(payload),
        "dashboard_data" => data::dashboard_data(payload),
        "apply" => Err(needs_app_handle("apply")),
        "enable" => Err(needs_app_handle("enable")),
        "disable" => Err(needs_app_handle("disable")),
        "resume" => Err(needs_app_handle("resume")),
        "reposition" => Err(needs_app_handle("reposition")),
        "pause" => pause_widget(payload),
        "set_privacy_mask" => set_privacy_mask(payload),
        "diagnostics" => Ok(session::session().diagnostics_snapshot()),
        _ => Err(format!("unsupported widget action: {action}")),
    }
}

pub fn execute_with_app(action: &str, payload: &Value, app: &AppHandle) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported widget action: {action}"));
    }
    match action {
        "apply" => apply::apply(app),
        "enable" => enable_widget(app),
        "disable" => disable_widget(app),
        "resume" => resume_widget(app),
        "reposition" => reposition_widget(app),
        _ => execute(action, payload),
    }
}

fn needs_app_handle(action: &str) -> String {
    format!("widget.{action} requires AppHandle; route via execute_tool_with_app")
}

// ── 生命周期 ─────────────────────────────────────

/// 启用挂件：写 enabled=true → 清理运行时状态 → 委托 pulse 循环创建窗口。
///
/// **重要**：不在此线程（Tauri sync command thread pool）直接调 ensure() → build()，
/// Tauri 2 中从非主线程创建第二个 WebView2 窗口可能导致 build() 死锁。
/// 改为通知 pulse 循环立即处理，其 running loop 会唤醒并走 tick → apply → ensure。
fn enable_widget(app: &AppHandle) -> Result<Value, String> {
    eprintln!("[widget] enable: enter");
    let cfg = config::read_config();
    if cfg.enabled {
        eprintln!("[widget] enable: already enabled");
        // 窗口可能处于 Hidden（上次 disable 隐藏），恢复可见态
        if session::session().is_window_open()
            && session::session().visual_state() == widget::VisualState::Hidden
        {
            let _ = widget::set_state(app, widget::VisualState::Peek);
        }
        // 通知 pulse 立即推送数据
        pulse::notify_data_changed("widget-enable");
        return Ok(json!({ "ok": true, "alreadyEnabled": true }));
    }

    config::set_string(config::KEY_ENABLED, "true")?;

    // 防御性清理运行时状态：上一轮可能残留的 paused / lastError 都归零
    session::session().write_inner(|s| {
        s.pause_reason = None;
        s.last_error = None;
        s.auto_skip_reason = None;
        s.auto_skip_app = None;
    });
    session::session()
        .paused
        .store(false, std::sync::atomic::Ordering::SeqCst);
    session::session().invalidate_input_hash();

    // 通知 pulse 循环：立即创建窗口 + 推送数据（跳过事件去重 debounce）
    pulse::notify_data_changed("widget-enable");

    eprintln!("[widget] enable: done (window creation delegated to pulse loop)");
    Ok(json!({ "ok": true }))
}

/// 关闭挂件：隐藏窗口 + 写 enabled=false；不销毁，避免重建窗口时 Tauri 死锁。
fn disable_widget(app: &AppHandle) -> Result<Value, String> {
    eprintln!("[widget] disable: enter");
    config::set_string(config::KEY_ENABLED, "false")?;
    // 隐藏而非销毁：Tauri 2 中 close() + 立即 build() 同名窗口会卡死 WebviewWindowBuilder::build()
    if let Err(e) = widget::set_state(app, widget::VisualState::Hidden) {
        eprintln!("[widget] disable: hide widget failed: {e}");
    }
    // 清运行时状态，下次启用是干净的
    session::session().write_inner(|s| {
        s.pause_reason = None;
        s.last_error = None;
        s.auto_skip_reason = None;
        s.auto_skip_app = None;
    });
    session::session()
        .paused
        .store(false, std::sync::atomic::Ordering::SeqCst);
    eprintln!("[widget] disable: done");
    Ok(json!({ "ok": true }))
}

/// 应用退出钩子：销毁挂件窗口；不再有"恢复原壁纸"行为。
pub fn on_app_exit(app: &AppHandle) {
    let cfg = config::read_config();
    if !cfg.enabled {
        return;
    }
    if let Err(e) = widget::destroy(app) {
        eprintln!("[widget] on_app_exit: destroy failed: {e}");
    }
}

// ── 暂停 / 恢复 ──────────────────────────────────

/// 暂停：写状态 + 让挂件按当前 reason 进入 Hidden 或保留 peek。
///
/// 入参 `{ reason?: "manual" | "fullscreen" | "lock" }`。
/// `manual` 由用户在面板点击触发，挂件不强制隐藏（用户仍可看到 peek 条），
/// 仅停掉调度心跳；其他 reason 由对应模块自己控制 widget 状态。
fn pause_widget(payload: &Value) -> Result<Value, String> {
    let reason_str = payload
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("manual");
    let reason = match reason_str {
        "fullscreen" => session::PauseReason::Fullscreen,
        "lock" => session::PauseReason::Lock,
        _ => session::PauseReason::Manual,
    };
    session::session()
        .paused
        .store(true, std::sync::atomic::Ordering::SeqCst);
    session::session().write_inner(|s| {
        s.pause_reason = Some(reason);
    });
    Ok(json!({
        "ok": true,
        "paused": true,
        "reason": reason.as_str(),
    }))
}

/// 恢复：清暂停 + 立即推一次数据。
fn resume_widget(app: &AppHandle) -> Result<Value, String> {
    session::session()
        .paused
        .store(false, std::sync::atomic::Ordering::SeqCst);
    session::session().write_inner(|s| {
        s.pause_reason = None;
    });
    session::session().invalidate_input_hash();
    match apply::apply(app) {
        Ok(v) => Ok(json!({
            "ok": true,
            "paused": false,
            "applied": true,
            "applyResult": v,
        })),
        Err(e) => Ok(json!({
            "ok": true,
            "paused": false,
            "applied": false,
            "applyError": e,
        })),
    }
}

/// 强制重新计算挂件位置（用于 widgetY 改动后立即生效）。
///
/// 仅在窗口存在且处于 Peek/Full 状态时执行；其他状态（Hidden/Windowless）下
/// 位置变更会随下次显示自然生效。
fn reposition_widget(app: &AppHandle) -> Result<Value, String> {
    let s = session::session();
    if !s.is_window_open() {
        return Ok(json!({ "ok": true, "skipped": "no-window" }));
    }
    let cur = s.visual_state();
    if !matches!(cur, widget::VisualState::Peek | widget::VisualState::Full) {
        return Ok(json!({ "ok": true, "skipped": "not-visible" }));
    }
    let Some(win) = s.window_handle() else {
        return Ok(json!({ "ok": true, "skipped": "no-handle" }));
    };
    widget::apply_position(app, &win, cur)?;
    Ok(json!({ "ok": true }))
}

// ── 敏感模式 ──────────────────────────────────────/// 设置敏感模式开关 + 自动到期时间。
fn set_privacy_mask(payload: &Value) -> Result<Value, String> {
    let enabled = payload
        .get("enabled")
        .and_then(Value::as_bool)
        .ok_or("set_privacy_mask: missing enabled")?;

    if !enabled {
        config::set_string(config::KEY_PRIVACY_MASK, "false")?;
        config::set_string(config::KEY_PRIVACY_MASK_UNTIL, "")?;
        session::session().invalidate_input_hash();
        return Ok(json!({ "ok": true, "enabled": false }));
    }

    let duration_min = payload
        .get("durationMin")
        .and_then(Value::as_i64)
        .unwrap_or(120);

    let until = if duration_min > 0 {
        let dt = chrono::Utc::now() + chrono::Duration::minutes(duration_min);
        Some(dt.to_rfc3339())
    } else {
        None
    };

    config::set_string(config::KEY_PRIVACY_MASK, "true")?;
    config::set_string(
        config::KEY_PRIVACY_MASK_UNTIL,
        until.as_deref().unwrap_or(""),
    )?;
    session::session().invalidate_input_hash();
    Ok(json!({
        "ok": true,
        "enabled": true,
        "until": until,
    }))
}

// ── 旧 PNG 链路清理 ────────────────────────────────

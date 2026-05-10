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
pub mod conflicts;
pub mod config;
pub mod dashboard_logic;
pub mod data;
pub mod diagnostics;
pub mod guards;
pub mod pulse;
pub mod session;
pub mod widget;

use serde_json::{json, Value};
use tauri::AppHandle;

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
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
        "pause" => pause_widget(payload),
        "set_privacy_mask" => set_privacy_mask(payload),
        "diagnostics" => Ok(session::session().diagnostics_snapshot()),
        _ => Err(format!("unsupported widget action: {action}")),
    }
}

pub fn execute_with_app(action: &str, payload: &Value, app: &AppHandle) -> Result<Value, String> {
    match action {
        "apply" => apply::apply(app),
        "enable" => enable_widget(app),
        "disable" => disable_widget(app),
        "resume" => resume_widget(app),
        _ => execute(action, payload),
    }
}

fn needs_app_handle(action: &str) -> String {
    format!("widget.{action} requires AppHandle; route via execute_tool_with_app")
}

// ── 生命周期 ─────────────────────────────────────

/// 启用挂件：写 enabled=true → 数据迁移（一次性清旧 PNG 链路产物）→
/// 创建/复用挂件窗口 → 切到 Peek 可见态 → 立即推一次数据。
fn enable_widget(app: &AppHandle) -> Result<Value, String> {
    eprintln!("[widget] enable: enter");
    let cfg = config::read_config();
    if cfg.enabled {
        eprintln!("[widget] enable: already enabled, ensuring widget");
        widget::ensure(app)?;
        // 窗口可能处于 Hidden（上次 disable 隐藏），恢复可见态
        if session::session().visual_state() == widget::VisualState::Hidden {
            let _ = widget::set_state(app, widget::VisualState::Peek);
        }
        let _ = apply::apply(app);
        return Ok(json!({ "ok": true, "alreadyEnabled": true }));
    }

    // 一次性清理旧链路产物（壁纸备份目录 + 渲染历史 + 废弃 user_settings key）
    // perform_legacy_cleanup 已完成历史使命；legacy 迁移由 pulse::start() 启动时兜底调用
    // 迁移旧 wallpaper.* key 到新 widget.* key

    config::set_string(config::KEY_ENABLED, "true")?;

    // 防御性清理运行时状态：上一轮可能残留的 paused / lastError 都归零
    session::session().write_inner(|s| {
        s.pause_reason = None;
        s.last_error = None;
        s.auto_skip_reason = None;
    });
    session::session().paused.store(false, std::sync::atomic::Ordering::SeqCst);
    session::session().invalidate_input_hash();

    widget::ensure(app)?;
    // 首次创建时 ensure 内部 set_state(Peek) + show，无需再切；非首次则切到 Peek
    if session::session().visual_state() == widget::VisualState::Hidden {
        let _ = widget::set_state(app, widget::VisualState::Peek);
    }
    // 立即推一次数据，避免用户开启后等下个心跳才看到内容
    if let Err(e) = apply::apply(app) {
        eprintln!("[widget] enable: first apply failed (continuing): {e}");
    }

    eprintln!("[widget] enable: done");
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
    });
    session::session().paused.store(false, std::sync::atomic::Ordering::SeqCst);
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
    session::session().paused.store(true, std::sync::atomic::Ordering::SeqCst);
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
    session::session().paused.store(false, std::sync::atomic::Ordering::SeqCst);
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

// ── 敏感模式 ──────────────────────────────────────

/// 设置敏感模式开关 + 自动到期时间。
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

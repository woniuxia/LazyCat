//! Desktop Widget · 桌面挂件
//!
//! 常驻挂件窗口（widget.rs）+ 状态机（peek/full/hidden），
//! 在桌面右侧展示今日仪表盘（PM/Todo 概览 + 待办列表）。
//!
//! 通道分发表：
//! - 不需要 AppHandle：`status` / `get_config` / `set_config` / `dashboard_data`
//!   / `pause` / `set_privacy_mask` / `set_boss_key_error`
//! - 需要 AppHandle：`enable` / `disable` / `apply` / `resume`

pub mod apply;
pub mod boss_key;
pub mod conflicts;
pub mod config;
pub mod dashboard_logic;
pub mod data;
pub mod events;
pub mod fullscreen;
pub mod idle;
pub mod lock;
pub mod scheduler;
pub mod state;
pub mod widget;

use std::path::PathBuf;

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::tools::helpers::get_data_dir;

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "status" => Ok(state::status_snapshot()),
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
        "set_boss_key_error" => set_boss_key_error_action(payload),
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
/// 创建挂件窗口 → 立即推一次数据。
fn enable_widget(app: &AppHandle) -> Result<Value, String> {
    eprintln!("[widget] enable: enter");
    let cfg = config::read_config();
    if cfg.enabled {
        eprintln!("[widget] enable: already enabled, ensuring widget");
        widget::ensure(app)?;
        let _ = apply::apply(app);
        return Ok(json!({ "ok": true, "alreadyEnabled": true }));
    }

    // 一次性清理旧链路产物（壁纸备份目录 + 渲染历史 + 废弃 user_settings key）
    perform_legacy_cleanup();
    // 迁移旧 wallpaper.* key 到新 widget.* key
    config::migrate_legacy_keys();

    config::set_string(config::KEY_ENABLED, "true")?;

    // 防御性清理运行时状态：上一轮可能残留的 paused / lastError 都归零
    state::write(|s| {
        s.paused = false;
        s.pause_reason = None;
        s.last_error = None;
        s.auto_skip_reason = None;
    });
    apply::invalidate_input_hash();

    widget::ensure(app)?;
    // 立即推一次数据，避免用户开启后等下个心跳才看到内容
    if let Err(e) = apply::apply(app) {
        eprintln!("[widget] enable: first apply failed (continuing): {e}");
    }

    eprintln!("[widget] enable: done");
    Ok(json!({ "ok": true }))
}

/// 关闭挂件：销毁窗口 + 写 enabled=false。
fn disable_widget(app: &AppHandle) -> Result<Value, String> {
    eprintln!("[widget] disable: enter");
    config::set_string(config::KEY_ENABLED, "false")?;
    if let Err(e) = widget::destroy(app) {
        eprintln!("[widget] disable: destroy failed: {e}");
    }
    // 清运行时状态，下次启用是干净的
    state::write(|s| {
        s.paused = false;
        s.pause_reason = None;
        s.last_error = None;
        s.auto_skip_reason = None;
    });
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

/// 旧 PNG 链路的一次性清理：
/// - 删 `<data_dir>/wallpapers/original/`（备份原图）
/// - 删 `<data_dir>/wallpapers/rendered/`（合成历史）
/// - 删 user_settings 中废弃 key
///
/// 失败仅 log，不阻塞 enable。
fn perform_legacy_cleanup() {
    if let Ok(data_dir) = get_data_dir() {
        for sub in ["original", "rendered"] {
            let path: PathBuf = data_dir.join("wallpapers").join(sub);
            if path.exists() {
                if let Err(e) = std::fs::remove_dir_all(&path) {
                    eprintln!(
                        "[widget] cleanup: remove {} failed: {e}",
                        path.display()
                    );
                } else {
                    eprintln!("[widget] cleanup: removed {}", path.display());
                }
            }
        }
    }

    for key in [
        "wallpaper.original_path",
        "wallpaper.original_set_method",
        "wallpaper.exit_behavior",
        "wallpaper.image_format",
        "wallpaper.keep_history_count",
        "wallpaper.position",
    ] {
        if let Err(e) = config::delete_key(key) {
            eprintln!("[widget] cleanup: delete key {key} failed: {e}");
        }
    }
}

// ── 暂停 / 恢复 ──────────────────────────────────

/// 暂停：写状态 + 让挂件按当前 reason 进入 Hidden 或保留 peek。
///
/// 入参 `{ reason?: "manual" | "boss_key" | "fullscreen" | "lock" }`。
/// `manual` 由用户在面板点击触发，挂件不强制隐藏（用户仍可看到 peek 条），
/// 仅停掉调度心跳；其他 reason 由对应模块自己控制 widget 状态。
fn pause_widget(payload: &Value) -> Result<Value, String> {
    let reason_str = payload
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("manual");
    let reason = match reason_str {
        "boss_key" => state::PauseReason::BossKey,
        "fullscreen" => state::PauseReason::Fullscreen,
        "lock" => state::PauseReason::Lock,
        _ => state::PauseReason::Manual,
    };
    state::write(|s| {
        s.paused = true;
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
    state::write(|s| {
        s.paused = false;
        s.pause_reason = None;
    });
    apply::invalidate_input_hash();
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
        apply::invalidate_input_hash();
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
    apply::invalidate_input_hash();
    Ok(json!({
        "ok": true,
        "enabled": true,
        "until": until,
    }))
}

// ── 老板键失败提示 ────────────────────────────────

/// 由 main.rs setup 调用：注册成功 → 清空错误；失败 → 写文案让前端透出。
pub fn record_boss_key_error(msg: Option<String>) {
    state::write(|st| st.boss_key_error = msg);
}

/// 前端 channel `tool:widget:set-boss-key-error` 入口。
fn set_boss_key_error_action(payload: &Value) -> Result<Value, String> {
    let entry = payload.get("error");
    let msg = match entry {
        Some(Value::Null) | None => None,
        Some(Value::String(s)) if s.is_empty() => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(other) => return Err(format!("error must be string or null, got {other}")),
    };
    record_boss_key_error(msg);
    Ok(json!({ "ok": true }))
}

//! Living Wallpaper · 桌面壁纸仪表盘
//!
//! 关联设计：docs/superpowers/specs/2026-05-05-living-wallpaper-design.md (v0.5)
//! 关联实施：docs/superpowers/specs/2026-05-05-living-wallpaper-plan.md
//!
//! Phase 0 仅搭建骨架；Phase 1.8 接入 enable / disable / restore；
//! 实质渲染 / 合成 / set 壁纸 / 调度逻辑在 Phase 2-3 接入。

pub mod capture;
pub mod compose;
pub mod config;
pub mod dashboard_logic;
pub mod data;
pub mod desktop;
pub mod scheduler;
pub mod state;

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::tools::helpers::{db_conn, get_data_dir};

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "status" => Ok(state::status_snapshot()),
        "get_config" => serde_json::to_value(config::read_config())
            .map_err(|e| format!("serialize wallpaper config failed: {e}")),
        "set_config" => config::set_config(payload),
        "dashboard_data" => data::dashboard_data(payload),
        "render_once" => Err(not_yet_implemented("render_once")),
        "apply" => Err(not_yet_implemented("apply")),
        "restore" => restore_wallpaper(),
        "pause" => Err(not_yet_implemented("pause")),
        "resume" => Err(not_yet_implemented("resume")),
        "enable" => enable_wallpaper(),
        "disable" => disable_wallpaper(payload),
        "list_history" => Ok(json!({ "items": [] })),
        _ => Err(format!("unsupported wallpaper action: {action}")),
    }
}

fn not_yet_implemented(action: &str) -> String {
    format!("wallpaper.{action} not yet implemented (Phase 0 skeleton)")
}

// ── 生命周期（plan §1.8 / design §10） ──────────

/// 启用：备份原壁纸 → 写 original_path → 置 enabled=true。
///
/// 此 action 不立即合成；首次合成由 `apply` / `render_once` 触发（Phase 2）。
fn enable_wallpaper() -> Result<Value, String> {
    let cfg = config::read_config();
    if cfg.enabled {
        return Ok(json!({ "ok": true, "alreadyEnabled": true }));
    }

    // 1. 取当前壁纸（COM；失败时静默回退到空，仍允许启用）
    let original = match desktop::get_current_wallpaper(desktop::PRIMARY_MONITOR_INDEX) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[wallpaper] enable: get_current_wallpaper failed: {e}");
            PathBuf::new()
        }
    };

    // 2. 备份到 <data_dir>/wallpapers/original/<timestamp>.<ext>
    let backup_path = if !original.as_os_str().is_empty() && original.exists() {
        match backup_original(&original) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[wallpaper] enable: backup_original failed: {e}");
                PathBuf::new()
            }
        }
    } else {
        PathBuf::new()
    };

    // 3. 持久化路径与启用标记
    config::set_string(
        config::KEY_ORIGINAL_PATH,
        &backup_path.to_string_lossy(),
    )?;
    // set_method 延后到首次 apply 写入（COM 成功 → "com"，回退 → "sysparam"）
    config::set_string(config::KEY_ENABLED, "true")?;

    Ok(json!({
        "ok": true,
        "originalPath": backup_path.to_string_lossy(),
        "originalExisted": !original.as_os_str().is_empty(),
    }))
}

/// 关闭：置 enabled=false；按 `exit_behavior` 决定是否同步恢复原图。
///
/// 入参 `{ restore?: bool }` 显式覆盖；缺省时按 `wallpaper.exit_behavior`：
/// - `restore_original`（默认） → 调 restore
/// - `keep_last`                → 保留最后一帧合成图
fn disable_wallpaper(payload: &Value) -> Result<Value, String> {
    let cfg = config::read_config();
    config::set_string(config::KEY_ENABLED, "false")?;

    let restore_flag = payload
        .get("restore")
        .and_then(Value::as_bool)
        .unwrap_or(cfg.exit_behavior == "restore_original");

    if restore_flag {
        // 即使 restore 失败也不阻塞 disable；记录错误透出给前端
        match restore_wallpaper() {
            Ok(_) => Ok(json!({ "ok": true, "restored": true })),
            Err(e) => Ok(json!({ "ok": true, "restored": false, "restoreError": e })),
        }
    } else {
        Ok(json!({ "ok": true, "restored": false }))
    }
}

/// 恢复：从 `wallpaper.original_path` 读备份并 set 回桌面。
fn restore_wallpaper() -> Result<Value, String> {
    let conn = db_conn()?;
    let original = config::read_string(&conn, config::KEY_ORIGINAL_PATH).unwrap_or_default();
    if original.is_empty() {
        return Err("no original wallpaper backed up".into());
    }
    let path = PathBuf::from(&original);
    if !path.exists() {
        return Err(format!("original wallpaper backup missing: {original}"));
    }
    let method = desktop::set_wallpaper(desktop::PRIMARY_MONITOR_INDEX, &path)?;
    config::set_string(config::KEY_ORIGINAL_SET_METHOD, method.as_str())?;
    Ok(json!({
        "ok": true,
        "method": method.as_str(),
        "path": original,
    }))
}

/// 把当前壁纸文件复制到 `<data_dir>/wallpapers/original/<timestamp>.<ext>`。
fn backup_original(src: &Path) -> Result<PathBuf, String> {
    let dir = get_data_dir()?.join("wallpapers").join("original");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("create wallpaper original dir: {e}"))?;
    let ext = src
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("img");
    let ts = chrono::Local::now().format("%Y%m%dT%H%M%S").to_string();
    let dst = dir.join(format!("{ts}.{ext}"));
    std::fs::copy(src, &dst)
        .map_err(|e| format!("copy original wallpaper: {e}"))?;
    Ok(dst)
}

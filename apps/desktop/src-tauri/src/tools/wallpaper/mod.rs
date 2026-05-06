//! Living Wallpaper · 桌面壁纸仪表盘
//!
//! 关联设计：docs/superpowers/specs/2026-05-05-living-wallpaper-design.md (v0.5)
//! 关联实施：docs/superpowers/specs/2026-05-05-living-wallpaper-plan.md
//!
//! 通道分发表：
//! - `status` / `get_config` / `set_config` / `dashboard_data` / `list_history` 同步
//! - `enable` / `disable` / `restore` / `pause` 同步
//! - `apply` / `resume` 需要 AppHandle，走 `execute_with_app`
//!
//! `render_once` 在 plan §1.7 起初设计为「前端把 PNG 推回后端」的回写通道；
//! 实际实现走 `wallpaper://canvas-ready` 事件 + 后端 CapturePreview 直接抓帧，
//! 因此该通道未启用，已从分发表移除。

pub mod apply;
pub mod boss_key;
pub mod capture;
pub mod compose;
pub mod config;
pub mod dashboard_logic;
pub mod data;
pub mod desktop;
pub mod events;
pub mod fullscreen;
pub mod hidden;
pub mod idle;
pub mod lock;
pub mod scheduler;
pub mod state;

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::tools::helpers::{db_conn, get_data_dir};

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "status" => Ok(state::status_snapshot()),
        "get_config" => serde_json::to_value(config::read_config())
            .map_err(|e| format!("serialize wallpaper config failed: {e}")),
        "set_config" => config::set_config(payload),
        "dashboard_data" => data::dashboard_data(payload),
        "apply" => Err(needs_app_handle("apply")),
        "restore" => restore_wallpaper(),
        "pause" => pause_wallpaper(payload),
        "resume" => Err(needs_app_handle("resume")),
        "enable" => enable_wallpaper(),
        "disable" => disable_wallpaper(payload),
        "list_history" => list_history_action(payload),
        _ => Err(format!("unsupported wallpaper action: {action}")),
    }
}

/// 带 AppHandle 版本（plan §2.4）：apply 需要 emit / with_webview，
/// 单纯走 sync execute 拿不到 app；调度方走 `execute_tool_with_app`。
pub fn execute_with_app(action: &str, payload: &Value, app: &AppHandle) -> Result<Value, String> {
    match action {
        "apply" => apply::apply(app),
        "resume" => resume_wallpaper(app),
        // 其他 action 不需要 app，直接走 sync 入口
        _ => execute(action, payload),
    }
}

fn needs_app_handle(action: &str) -> String {
    format!("wallpaper.{action} requires AppHandle; route via execute_tool_with_app")
}

/// 列出历史合成图（design §6 / plan §1.5）；按文件 mtime 倒序，最新在前。
///
/// 输出 `{ items: [{ path, size, createdAt }] }`，与前端
/// `WallpaperHistoryEntry` 类型一一对应。目录不存在时返回空数组。
fn list_history_action(_payload: &Value) -> Result<Value, String> {
    let dir = get_data_dir()?.join("wallpapers").join("rendered");
    if !dir.exists() {
        return Ok(json!({ "items": [] }));
    }

    let mut entries: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
    let read_dir = std::fs::read_dir(&dir)
        .map_err(|e| format!("read wallpapers dir: {e}"))?;
    for entry in read_dir.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let mtime = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        entries.push((entry.path(), meta.len(), mtime));
    }
    // 最新在前；mtime 解析失败的回到 EPOCH 自动沉底
    entries.sort_by(|a, b| b.2.cmp(&a.2));

    let items: Vec<Value> = entries
        .into_iter()
        .map(|(path, size, mtime)| {
            let secs = mtime
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let created_at = chrono::DateTime::<chrono::Local>::from(
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs.max(0) as u64),
            )
            .format("%Y-%m-%dT%H:%M:%S%:z")
            .to_string();
            json!({
                "path": path.to_string_lossy(),
                "size": size,
                "createdAt": created_at,
            })
        })
        .collect();

    Ok(json!({ "items": items }))
}

// ── 生命周期（plan §1.8 / design §10） ──────────

/// 启用：备份原壁纸 → 写 original_path → 置 enabled=true。
///
/// 此 action 不立即合成；首次合成由 `apply` 触发（scheduler 启动后立即跑一轮）。
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

    // 4. 重置内容 hash：新启用周期内首次 apply 必须真正合成一次
    apply::invalidate_input_hash();

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
    // 桌面壁纸已被替换，下次 apply 必须真正合成（不依赖 hash 去重）
    apply::invalidate_input_hash();
    Ok(json!({
        "ok": true,
        "method": method.as_str(),
        "path": original,
    }))
}

/// 应用退出钩子（design §10）。
///
/// 按 `wallpaper.exit_behavior` 处理桌面：
/// - 未启用 → no-op
/// - `keep_last` → no-op（保留最后一帧合成图）
/// - `restore_original`（默认）→ 调 [`restore_wallpaper`] 还原原图
///
/// 由 main.rs 在 `RunEvent::ExitRequested` 触发；幂等，多次调用无副作用。
/// 任何错误只打 stderr，不阻塞退出流程。
pub fn on_app_exit() {
    let cfg = config::read_config();
    if !cfg.enabled {
        return;
    }
    if cfg.exit_behavior == "keep_last" {
        return;
    }
    // restore_original 或未知值都视为「恢复」
    match restore_wallpaper() {
        Ok(_) => eprintln!("[wallpaper] on_app_exit: original wallpaper restored"),
        Err(e) => eprintln!("[wallpaper] on_app_exit: restore failed: {e}"),
    }
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

// ── 暂停 / 恢复（plan §3.4） ──────────

/// 暂停：写状态 paused=true + 记录原因；调度线程下一轮 should_skip 时跳过 apply。
///
/// 入参 `{ reason?: "manual" | "boss_key" | "fullscreen" | "lock" }`，缺省 manual。
/// 不直接还原原图（restore 由调用方按需触发；老板键有专门的 boss_key::toggle）。
fn pause_wallpaper(payload: &Value) -> Result<Value, String> {
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
    Ok(json!({ "ok": true, "paused": true, "reason": reason.as_str() }))
}

/// 恢复：清暂停 + 立即 apply 一次（避免用户等下一个心跳）。
///
/// 由 execute_with_app 路由进入；apply 失败不阻塞 resume，错误透出给前端。
fn resume_wallpaper(app: &AppHandle) -> Result<Value, String> {
    state::write(|s| {
        s.paused = false;
        s.pause_reason = None;
    });
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

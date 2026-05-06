//! 老板键（plan §3.2 / design §9）
//!
//! - 全局快捷键 `Ctrl+Alt+W`（默认）切换 boss key 暂停态
//! - 暂停时调度仍 alive，仅跳过 apply；同时立刻 set 回原壁纸
//! - 恢复时立即触发一次 apply，避免用户等下一个心跳
//! - 注册失败：design §9 要求降级为状态卡片提示，由前端展示，本模块只回错

#![allow(dead_code)] // toggle 由 main.rs 全局快捷键回调使用

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::tools::wallpaper::{apply, config, desktop, state};

/// 切换老板键暂停态。
///
/// 状态机：
/// - 已暂停且原因为 BossKey → 取消暂停 + 立刻 apply 一次（用户主动恢复）
/// - 其他状态（未暂停 / 因其他原因暂停）→ 进入 BossKey 暂停 + 还原原壁纸
///
/// 返回 `{ paused, action }`，action 用 `paused` / `resumed` 描述本次动作。
pub fn toggle(app: &AppHandle) -> Result<Value, String> {
    let snapshot = state::snapshot();
    let already_boss = snapshot.paused
        && matches!(snapshot.pause_reason, Some(state::PauseReason::BossKey));

    if already_boss {
        // 恢复：清暂停 → 立即 apply
        state::write(|s| {
            s.paused = false;
            s.pause_reason = None;
        });

        // 立即合成；失败不回滚暂停态（用户已经显式表示要回到工作态）
        match apply::apply(app) {
            Ok(_) => Ok(json!({ "ok": true, "paused": false, "action": "resumed" })),
            Err(e) => {
                eprintln!("[wallpaper] boss-key resume apply failed: {e}");
                Ok(json!({
                    "ok": true,
                    "paused": false,
                    "action": "resumed",
                    "applyError": e,
                }))
            }
        }
    } else {
        // 暂停：写状态 → 还原原壁纸（COM 失败回退 SysParam）
        state::write(|s| {
            s.paused = true;
            s.pause_reason = Some(state::PauseReason::BossKey);
        });

        let restore_result = restore_original_inline();
        match restore_result {
            Ok(method) => Ok(json!({
                "ok": true,
                "paused": true,
                "action": "paused",
                "restoreMethod": method,
            })),
            Err(e) => {
                eprintln!("[wallpaper] boss-key pause restore failed: {e}");
                // 暂停态已写；即使 restore 失败也保持暂停（heartbeat 不会再合成新图）
                Ok(json!({
                    "ok": true,
                    "paused": true,
                    "action": "paused",
                    "restoreError": e,
                }))
            }
        }
    }
}

/// 直接读 wallpaper.original_path，调 desktop::set_wallpaper 还原；
/// 与 mod.rs::restore_wallpaper 同口径，但不写日志、不更新 set_method。
///
/// 返回值是实际走的设置路径（com / sysparam），供前端透出。
fn restore_original_inline() -> Result<&'static str, String> {
    use crate::tools::helpers::db_conn;
    use std::path::PathBuf;

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
    Ok(method.as_str())
}

//! 老板键（design §9，挂件版）
//!
//! - 全局快捷键 `Ctrl+Alt+W`（默认）切换 boss-key 暂停态
//! - 暂停：挂件直接 `set_state(Hidden)`（连 peek 条都消失），调度跳过 apply
//! - 恢复：`set_state(Peek)` + 立即 apply 推最新数据
//! - 注册失败：design §9 要求降级为状态卡片提示，由前端展示

#![allow(dead_code)]

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::tools::widget::{apply, state, widget};

/// 切换老板键暂停态。
///
/// 状态机：
/// - 已暂停且原因为 BossKey → 取消暂停 + 挂件 set_state(Peek) + 立即 apply
/// - 其他状态（未暂停 / 因其他原因暂停）→ 进入 BossKey 暂停 + set_state(Hidden)
pub fn toggle(app: &AppHandle) -> Result<Value, String> {
    let snapshot = state::snapshot();
    let already_boss = snapshot.paused
        && matches!(snapshot.pause_reason, Some(state::PauseReason::BossKey));

    if already_boss {
        // 恢复
        state::write(|s| {
            s.paused = false;
            s.pause_reason = None;
        });
        apply::invalidate_input_hash();

        if let Err(e) = widget::set_state(app, widget::VisualState::Peek) {
            eprintln!("[widget] boss-key resume: widget show failed: {e}");
        }
        match apply::apply(app) {
            Ok(_) => Ok(json!({ "ok": true, "paused": false, "action": "resumed" })),
            Err(e) => {
                eprintln!("[widget] boss-key resume apply failed: {e}");
                state::write(|s| s.last_error = Some(format!("老板键恢复失败：{e}")));
                Ok(json!({
                    "ok": true,
                    "paused": false,
                    "action": "resumed",
                    "applyError": e,
                }))
            }
        }
    } else {
        // 暂停
        state::write(|s| {
            s.paused = true;
            s.pause_reason = Some(state::PauseReason::BossKey);
        });
        if let Err(e) = widget::set_state(app, widget::VisualState::Hidden) {
            eprintln!("[widget] boss-key pause: widget hide failed: {e}");
        }
        Ok(json!({ "ok": true, "paused": true, "action": "paused" }))
    }
}

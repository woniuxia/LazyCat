//! Living Wallpaper · 桌面壁纸仪表盘
//!
//! 关联设计：docs/superpowers/specs/2026-05-05-living-wallpaper-design.md (v0.5)
//! 关联实施：docs/superpowers/specs/2026-05-05-living-wallpaper-plan.md
//!
//! Phase 0 仅搭建骨架：分发表 + 配置读写 + status 默认值；
//! 实质渲染 / 合成 / set 壁纸 / 调度逻辑在 Phase 1-3 接入。

pub mod capture;
pub mod compose;
pub mod config;
pub mod data;
pub mod desktop;
pub mod scheduler;
pub mod state;

use serde_json::{json, Value};

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "status" => Ok(state::status_snapshot()),
        "get_config" => serde_json::to_value(config::read_config())
            .map_err(|e| format!("serialize wallpaper config failed: {e}")),
        "set_config" => config::set_config(payload),
        "dashboard_data" => Err(not_yet_implemented("dashboard_data")),
        "render_once" => Err(not_yet_implemented("render_once")),
        "apply" => Err(not_yet_implemented("apply")),
        "restore" => Err(not_yet_implemented("restore")),
        "pause" => Err(not_yet_implemented("pause")),
        "resume" => Err(not_yet_implemented("resume")),
        "enable" => Err(not_yet_implemented("enable")),
        "disable" => Err(not_yet_implemented("disable")),
        "list_history" => Ok(json!({ "items": [] })),
        _ => Err(format!("unsupported wallpaper action: {action}")),
    }
}

fn not_yet_implemented(action: &str) -> String {
    format!("wallpaper.{action} not yet implemented (Phase 0 skeleton)")
}

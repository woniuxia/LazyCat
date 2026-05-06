//! 仪表盘数据聚合（Phase 1.1 实现）
//!
//! 阶段 0：仅占位骨架，避免 mod.rs 引用编译错。
//! 阶段 1：复用 `pm_today::priority_rank` 与 `todo::is_open_status`（提升为 pub），
//! 跨 PM/Todo 合并去重排序，详见设计 §5.2 与 plan §1.1。

#![allow(dead_code)]

use serde_json::Value;

/// 占位入口；实际实现见 plan §1.1。
pub fn dashboard_data(_payload: &Value) -> Result<Value, String> {
    Err("wallpaper.dashboard_data not yet implemented (Phase 1.1)".into())
}

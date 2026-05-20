//! 仪表盘数据聚合（design §1.1）
//!
//! 负责跨 PM / Todo 拉取未完成事项，调用 `dashboard_logic` 完成合并、排序、
//! 聚合统计；最终输出对齐前端 `WidgetDashboardData` 类型。

use chrono::{Local, Utc};
use rusqlite::Connection;
use serde_json::{json, Value};

use crate::tools::helpers::db_conn;
use crate::tools::widget::config;
use crate::tools::widget::dashboard_logic::{
    merge_and_dedup_items, sort_dashboard_items,
};

/// `todoList` 截断上限（前端可滚动浏览全部）。
const TODO_LIMIT: usize = 100;

/// 仪表盘聚合主入口；通道 `tool:widget:dashboard_data` 直接调用。
pub fn dashboard_data(_payload: &Value) -> Result<Value, String> {
    let now_local = Local::now();
    let today = now_local.date_naive();
    let today_str = today.format("%Y-%m-%d").to_string();

    let conn = db_conn()?;

    let pm_rows = load_pm_rows(&conn)?;
    let todo_rows = load_todo_rows(&conn)?;

    // SQL 已限定只加载未完成事项，无需二次过滤
    let mut merged = merge_and_dedup_items(&pm_rows, &todo_rows, &today_str);
    sort_dashboard_items(&mut merged);
    let total_count = merged.len();
    let truncated = total_count > TODO_LIMIT;
    if truncated {
        merged.truncate(TODO_LIMIT);
    }

    let cfg = config::read_config();
    let hot_limit = cfg.extension_hot_tools_limit.max(1).min(20) as usize;
    let hot_tools = compute_hot_tools(&conn, hot_limit);

    Ok(json!({
        "todoList": merged,
        "todoTotalCount": total_count,
        "todoTruncated": truncated,
        "generatedAt": Utc::now().to_rfc3339(),
        "hotTools": hot_tools,
        "extensionFixedTools": cfg.extension_fixed_tools,
        "extensionHotToolsLimit": cfg.extension_hot_tools_limit,
    }))
}

// ── SQL 装载 ──────────────────────────────────

fn load_pm_rows(conn: &Connection) -> Result<Vec<Value>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, priority, status, end_at, pinned, completed_at, created_at, started_at
             FROM pm_items
             WHERE status != 'done'",
        )
        .map_err(|e| format!("prepare widget.pm sql: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            let id: i64 = r.get(0)?;
            let title: String = r.get(1)?;
            let priority: String = r.get(2)?;
            let status: String = r.get(3)?;
            let end_at: Option<String> = r.get(4)?;
            let pinned: i64 = r.get(5)?;
            let completed_at: Option<String> = r.get(6)?;
            let created_at: String = r.get(7)?;
            let started_at: Option<String> = r.get(8)?;
            Ok(json!({
                "id": id,
                "title": title,
                "priority": priority,
                "status": status,
                "endAt": end_at,
                "pinned": pinned != 0,
                "completedAt": completed_at,
                "createdAt": created_at,
                "startedAt": started_at,
            }))
        })
        .map_err(|e| format!("query widget.pm: {e}"))?;
    let mut list = Vec::new();
    for r in rows {
        list.push(r.map_err(|e| format!("read widget.pm row: {e}"))?);
    }
    Ok(list)
}

fn load_todo_rows(conn: &Connection) -> Result<Vec<Value>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT i.id, i.title, i.priority, i.status, i.event_at, i.pinned,
                    i.completed_at, i.created_at,
                    (SELECT pm_item_id FROM pm_item_todo_links WHERE todo_item_id = i.id LIMIT 1) AS pm_item_id
             FROM todo_items i
             WHERE i.status IN ('pending', 'in_progress')",
        )
        .map_err(|e| format!("prepare widget.todo sql: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            let id: i64 = r.get(0)?;
            let title: String = r.get(1)?;
            let priority: String = r.get(2)?;
            let status: String = r.get(3)?;
            let event_at: Option<String> = r.get(4)?;
            let pinned: i64 = r.get(5)?;
            let completed_at: Option<String> = r.get(6)?;
            let created_at: String = r.get(7)?;
            let pm_item_id: Option<i64> = r.get(8)?;
            Ok(json!({
                "id": id,
                "title": title,
                "priority": priority,
                "status": status,
                "eventAt": event_at,
                "pinned": pinned != 0,
                "completedAt": completed_at,
                "createdAt": created_at,
                "pmItemId": pm_item_id,
            }))
        })
        .map_err(|e| format!("query widget.todo: {e}"))?;
    let mut list = Vec::new();
    for r in rows {
        list.push(r.map_err(|e| format!("read widget.todo row: {e}"))?);
    }
    Ok(list)
}

// ── 热点工具 ──────────────────────────────────────

/// 读取 `tool_clicks`，统计近 30 天每个工具的点击数，取 Top N。
/// 排除 `todo`（挂件已有快捷入口）和 `widget`（挂件不应推荐自身）。
fn compute_hot_tools(conn: &Connection, limit: usize) -> Vec<Value> {
    let raw = match config::read_string(conn, "tool_clicks") {
        Some(s) => s,
        None => return Vec::new(),
    };

    let clicks: std::collections::HashMap<String, Vec<i64>> = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[widget] parse tool_clicks failed: {e}");
            return Vec::new();
        }
    };

    let cutoff = Utc::now().timestamp_millis() - 30 * 24 * 3600 * 1000;

    let mut counts: Vec<(&str, usize)> = clicks
        .iter()
        .filter(|(id, _)| *id != "todo" && *id != "widget")
        .map(|(id, timestamps)| {
            let count = timestamps.iter().filter(|&&ts| ts >= cutoff).count();
            (id.as_str(), count)
        })
        .filter(|(_, count)| *count > 0)
        .collect();

    counts.sort_by(|a, b| b.1.cmp(&a.1));
    counts.truncate(limit);

    counts
        .into_iter()
        .map(|(id, count)| {
            json!({
                "id": id,
                "count": count,
            })
        })
        .collect()
}

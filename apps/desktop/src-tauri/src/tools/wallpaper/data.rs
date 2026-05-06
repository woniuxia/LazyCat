//! 仪表盘数据聚合（plan §1.1）
//!
//! 负责跨 PM / Todo 拉取今日相关数据，调用 `dashboard_logic` 完成合并、排序、
//! 聚合统计；最终输出对齐前端 `WallpaperDashboardData` 类型。

use chrono::{Local, NaiveDate, TimeZone, Utc};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

use crate::tools::helpers::db_conn;
use crate::tools::todo::is_open_status;
use crate::tools::wallpaper::dashboard_logic::{
    compute_nearest_deadline_hours, merge_and_dedup_items, sort_dashboard_items,
};

/// `todoList` 截断上限（前端按高度二次裁剪）。
const TODO_LIMIT: usize = 20;

/// 仪表盘聚合主入口；通道 `tool:wallpaper:dashboard_data` 直接调用。
pub fn dashboard_data(_payload: &Value) -> Result<Value, String> {
    let now_local = Local::now();
    let today = now_local.date_naive();
    let today_str = today.format("%Y-%m-%d").to_string();
    let (today_start_utc, today_end_utc) = today_bounds_utc(today)?;

    let conn = db_conn()?;

    let pm_rows = load_pm_rows(&conn, &today_start_utc, &today_end_utc)?;
    let todo_rows = load_todo_rows(&conn, &today_start_utc, &today_end_utc)?;

    // 1. 仅取 open 项做合并 / 排序，再截断
    let pm_open: Vec<Value> = pm_rows
        .iter()
        .filter(|r| !is_pm_done(r))
        .cloned()
        .collect();
    let todo_open: Vec<Value> = todo_rows
        .iter()
        .filter(|r| is_open_status(r.get("status").and_then(Value::as_str).unwrap_or("")))
        .cloned()
        .collect();

    let mut merged = merge_and_dedup_items(&pm_open, &todo_open, &today_str);
    sort_dashboard_items(&mut merged);
    if merged.len() > TODO_LIMIT {
        merged.truncate(TODO_LIMIT);
    }

    // 2. overview 统计直接对原 row 做（包含 completed-today 项）
    let mut completed_today: u32 = 0;
    let mut total_today: u32 = 0;
    let mut p0_pending: u32 = 0;
    for row in &pm_rows {
        match classify_pm(row, &today_str, &today_start_utc, &today_end_utc) {
            Bucket::CompletedToday => {
                completed_today += 1;
                total_today += 1;
            }
            Bucket::Overdue | Bucket::DueToday | Bucket::InProgress => {
                total_today += 1;
            }
            Bucket::Other => {}
        }
        if is_p0_open_pm(row) {
            p0_pending += 1;
        }
    }
    for row in &todo_rows {
        match classify_todo(row, &today_str, &today_start_utc, &today_end_utc) {
            Bucket::CompletedToday => {
                completed_today += 1;
                total_today += 1;
            }
            Bucket::Overdue | Bucket::DueToday | Bucket::InProgress => {
                total_today += 1;
            }
            Bucket::Other => {}
        }
        if is_p0_open_todo(row) {
            p0_pending += 1;
        }
    }

    let nearest = compute_nearest_deadline_hours(&merged, now_local);

    let overview = json!({
        "completedToday": completed_today,
        "totalToday": total_today,
        "p0Pending": p0_pending,
        "nearestDeadlineHours": nearest,
    });

    Ok(json!({
        "overview": overview,
        "todoList": merged,
        "echo": Value::Null,
        "generatedAt": Utc::now().to_rfc3339(),
    }))
}

// ── 内部分类 ──────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bucket {
    Overdue,
    DueToday,
    InProgress,
    CompletedToday,
    Other,
}

fn classify_pm(
    row: &Value,
    today_str: &str,
    today_start_utc: &str,
    today_end_utc: &str,
) -> Bucket {
    let status = row.get("status").and_then(Value::as_str).unwrap_or("");
    let completed_at = row.get("completedAt").and_then(Value::as_str);
    if let Some(c) = completed_at {
        if c >= today_start_utc && c <= today_end_utc {
            return Bucket::CompletedToday;
        }
    }
    if status == "done" {
        return Bucket::Other;
    }
    if let Some(date) = row
        .get("endAt")
        .and_then(Value::as_str)
        .and_then(|s| s.get(0..10))
    {
        if date < today_str {
            return Bucket::Overdue;
        }
        if date == today_str {
            return Bucket::DueToday;
        }
    }
    if row
        .get("startedAt")
        .and_then(Value::as_str)
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        return Bucket::InProgress;
    }
    Bucket::Other
}

fn classify_todo(
    row: &Value,
    today_str: &str,
    today_start_utc: &str,
    today_end_utc: &str,
) -> Bucket {
    let status = row.get("status").and_then(Value::as_str).unwrap_or("");
    let completed_at = row.get("completedAt").and_then(Value::as_str);
    if let Some(c) = completed_at {
        if c >= today_start_utc && c <= today_end_utc {
            return Bucket::CompletedToday;
        }
    }
    if status == "completed" {
        return Bucket::Other;
    }
    if let Some(date) = row
        .get("eventAt")
        .and_then(Value::as_str)
        .and_then(|s| s.get(0..10))
    {
        if date < today_str {
            return Bucket::Overdue;
        }
        if date == today_str {
            return Bucket::DueToday;
        }
    }
    if status == "in_progress" {
        return Bucket::InProgress;
    }
    Bucket::Other
}

fn is_pm_done(row: &Value) -> bool {
    row.get("status").and_then(Value::as_str) == Some("done")
}

fn is_p0_open_pm(row: &Value) -> bool {
    !is_pm_done(row)
        && row.get("priority").and_then(Value::as_str) == Some("P0")
}

fn is_p0_open_todo(row: &Value) -> bool {
    let s = row.get("status").and_then(Value::as_str).unwrap_or("");
    is_open_status(s) && row.get("priority").and_then(Value::as_str) == Some("P0")
}

// ── 时区 / 范围 ────────────────────────────────

fn today_bounds_utc(today: NaiveDate) -> Result<(String, String), String> {
    let start_local = today.and_hms_opt(0, 0, 0).ok_or("invalid today start")?;
    let end_local = today.and_hms_opt(23, 59, 59).ok_or("invalid today end")?;
    let start_utc = Local
        .from_local_datetime(&start_local)
        .single()
        .ok_or("today start tz conversion failed")?
        .with_timezone(&Utc);
    let end_utc = Local
        .from_local_datetime(&end_local)
        .single()
        .ok_or("today end tz conversion failed")?
        .with_timezone(&Utc);
    Ok((start_utc.to_rfc3339(), end_utc.to_rfc3339()))
}

// ── SQL 装载 ──────────────────────────────────

fn load_pm_rows(
    conn: &Connection,
    today_start_utc: &str,
    today_end_utc: &str,
) -> Result<Vec<Value>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, priority, status, end_at, pinned, completed_at, created_at, started_at
             FROM pm_items
             WHERE status != 'done'
                OR (status = 'done' AND completed_at >= ?1 AND completed_at <= ?2)",
        )
        .map_err(|e| format!("prepare wallpaper.pm sql: {e}"))?;
    let rows = stmt
        .query_map(params![today_start_utc, today_end_utc], |r| {
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
        .map_err(|e| format!("query wallpaper.pm: {e}"))?;
    let mut list = Vec::new();
    for r in rows {
        list.push(r.map_err(|e| format!("read wallpaper.pm row: {e}"))?);
    }
    Ok(list)
}

fn load_todo_rows(
    conn: &Connection,
    today_start_utc: &str,
    today_end_utc: &str,
) -> Result<Vec<Value>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT i.id, i.title, i.priority, i.status, i.event_at, i.pinned,
                    i.completed_at, i.created_at,
                    (SELECT pm_item_id FROM pm_item_todo_links WHERE todo_item_id = i.id LIMIT 1) AS pm_item_id
             FROM todo_items i
             WHERE i.status IN ('pending', 'in_progress')
                OR (i.status = 'completed' AND i.completed_at >= ?1 AND i.completed_at <= ?2)",
        )
        .map_err(|e| format!("prepare wallpaper.todo sql: {e}"))?;
    let rows = stmt
        .query_map(params![today_start_utc, today_end_utc], |r| {
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
        .map_err(|e| format!("query wallpaper.todo: {e}"))?;
    let mut list = Vec::new();
    for r in rows {
        list.push(r.map_err(|e| format!("read wallpaper.todo row: {e}"))?);
    }
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pm(status: &str, end_at: Option<&str>, started_at: Option<&str>, completed_at: Option<&str>) -> Value {
        json!({
            "id": 1,
            "title": "x",
            "priority": "P2",
            "status": status,
            "endAt": end_at,
            "pinned": false,
            "completedAt": completed_at,
            "createdAt": "2026-04-01T00:00:00Z",
            "startedAt": started_at,
        })
    }

    fn todo_item(status: &str, event_at: Option<&str>, completed_at: Option<&str>) -> Value {
        json!({
            "id": 1,
            "title": "x",
            "priority": "P2",
            "status": status,
            "eventAt": event_at,
            "pinned": false,
            "completedAt": completed_at,
            "createdAt": "2026-04-01T00:00:00Z",
            "pmItemId": null,
        })
    }

    const TODAY: &str = "2026-05-06";
    const START_UTC: &str = "2026-05-05T16:00:00+00:00";
    const END_UTC: &str = "2026-05-06T15:59:59+00:00";

    #[test]
    fn classify_pm_completed_in_window() {
        let row = pm("done", Some("2026-05-04"), None, Some("2026-05-06T05:00:00+00:00"));
        assert_eq!(classify_pm(&row, TODAY, START_UTC, END_UTC), Bucket::CompletedToday);
    }

    #[test]
    fn classify_pm_overdue() {
        let row = pm("todo", Some("2026-05-04"), None, None);
        assert_eq!(classify_pm(&row, TODAY, START_UTC, END_UTC), Bucket::Overdue);
    }

    #[test]
    fn classify_pm_due_today() {
        let row = pm("in_progress", Some("2026-05-06"), None, None);
        assert_eq!(classify_pm(&row, TODAY, START_UTC, END_UTC), Bucket::DueToday);
    }

    #[test]
    fn classify_pm_in_progress_no_date() {
        let row = pm("in_progress", None, Some("2026-05-05T08:00:00+00:00"), None);
        assert_eq!(classify_pm(&row, TODAY, START_UTC, END_UTC), Bucket::InProgress);
    }

    #[test]
    fn classify_pm_open_no_signal() {
        let row = pm("todo", None, None, None);
        assert_eq!(classify_pm(&row, TODAY, START_UTC, END_UTC), Bucket::Other);
    }

    #[test]
    fn classify_pm_done_outside_window() {
        let row = pm("done", Some("2026-05-04"), None, Some("2026-05-04T05:00:00+00:00"));
        assert_eq!(classify_pm(&row, TODAY, START_UTC, END_UTC), Bucket::Other);
    }

    #[test]
    fn classify_todo_completed_in_window() {
        let row = todo_item("completed", Some("2026-05-06T08:00:00+00:00"), Some("2026-05-06T09:00:00+00:00"));
        assert_eq!(classify_todo(&row, TODAY, START_UTC, END_UTC), Bucket::CompletedToday);
    }

    #[test]
    fn classify_todo_overdue() {
        let row = todo_item("pending", Some("2026-05-04T08:00:00+00:00"), None);
        assert_eq!(classify_todo(&row, TODAY, START_UTC, END_UTC), Bucket::Overdue);
    }

    #[test]
    fn classify_todo_in_progress_only_status() {
        let row = todo_item("in_progress", None, None);
        assert_eq!(classify_todo(&row, TODAY, START_UTC, END_UTC), Bucket::InProgress);
    }

    #[test]
    fn p0_pending_counts() {
        let pm_p0 = json!({
            "id": 1, "priority": "P0", "status": "todo",
        });
        let pm_p0_done = json!({
            "id": 2, "priority": "P0", "status": "done",
        });
        let pm_p1 = json!({
            "id": 3, "priority": "P1", "status": "todo",
        });
        assert!(is_p0_open_pm(&pm_p0));
        assert!(!is_p0_open_pm(&pm_p0_done));
        assert!(!is_p0_open_pm(&pm_p1));

        let todo_p0 = json!({ "priority": "P0", "status": "pending" });
        let todo_p0_done = json!({ "priority": "P0", "status": "completed" });
        assert!(is_p0_open_todo(&todo_p0));
        assert!(!is_p0_open_todo(&todo_p0_done));
    }
}

//! 壁纸仪表盘合并/排序/格式化的纯函数集合
//!
//! 设计依据：
//! - design §5.2：`pinned → 已逾期 → P0 → P1 → P2 → P3 → 截止日期升序 → 创建时间降序`
//! - plan §1.1：`priority_rank` / `is_open_status` 复用 PM / Todo 已有判定，禁止重写
//!
//! 所有函数保持纯：不依赖数据库、时钟、文件系统，便于完整覆盖单测。

#![allow(dead_code)] // Phase 1.1：format_deadline_label / compute_dashboard_hash 由 Phase 2 接入

use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone};
use serde_json::{json, Value};

use crate::tools::pm_today::priority_rank;
use crate::tools::todo::is_open_status;

const DEFAULT_PRIORITY: &str = "P3";
const SENTINEL_DATE: &str = "9999-12-31";

/// 跨 PM/Todo 的"未完成"判定：在 `todo::is_open_status` 基础上叠加 PM 词表
/// （`todo` / `testing`），保持 closed 集合 `done` / `completed` / `archived` 一致。
///
/// 不另起一套判定；`is_open_status` 仍是底层基线。
pub fn is_dashboard_open(status: &str) -> bool {
    if is_open_status(status) {
        return true;
    }
    matches!(status, "todo" | "testing")
}

/// 把 `yyyy-mm-dd` 或 RFC3339 串归一化为 `yyyy-mm-dd`（取前 10 字符），失败返回 None。
pub fn normalize_deadline_date(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.len() < 10 {
        return None;
    }
    let prefix = &trimmed[..10];
    NaiveDate::parse_from_str(prefix, "%Y-%m-%d")
        .ok()
        .map(|_| prefix.to_string())
}

/// 逾期判定：`is_dashboard_open(status) && deadline_date < today`
pub fn is_overdue(status: &str, deadline_date: Option<&str>, today: &str) -> bool {
    if !is_dashboard_open(status) {
        return false;
    }
    deadline_date.map(|d| d < today).unwrap_or(false)
}

/// 把 PM Value 归一化为壁纸 dashboard 项；可与 Todo 项一起排序。
pub fn pm_to_dashboard(item: &Value, today: &str) -> Value {
    let id = item.get("id").and_then(Value::as_i64).unwrap_or(0);
    let title = item
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let priority = item
        .get("priority")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_PRIORITY)
        .to_string();
    let pinned = item.get("pinned").and_then(Value::as_bool).unwrap_or(false);
    let status = item
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let end_at_raw = item.get("endAt").and_then(Value::as_str).unwrap_or("");
    let deadline = normalize_deadline_date(end_at_raw);
    let created_at = item
        .get("createdAt")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let overdue = is_overdue(&status, deadline.as_deref(), today);
    json!({
        "id": format!("pm:{id}"),
        "rawId": id,
        "title": title,
        "priority": priority,
        "pinned": pinned,
        "endAt": deadline,
        "status": status,
        "source": "pm",
        "isOverdue": overdue,
        "createdAt": created_at,
    })
}

/// 把 Todo Value 归一化为壁纸 dashboard 项。
pub fn todo_to_dashboard(item: &Value, today: &str) -> Value {
    let id = item.get("id").and_then(Value::as_i64).unwrap_or(0);
    let title = item
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let priority = item
        .get("priority")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_PRIORITY)
        .to_string();
    let pinned = item.get("pinned").and_then(Value::as_bool).unwrap_or(false);
    let status = item
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let event_at_raw = item.get("eventAt").and_then(Value::as_str).unwrap_or("");
    let deadline = normalize_deadline_date(event_at_raw);
    let created_at = item
        .get("createdAt")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let overdue = is_overdue(&status, deadline.as_deref(), today);
    json!({
        "id": format!("todo:{id}"),
        "rawId": id,
        "title": title,
        "priority": priority,
        "pinned": pinned,
        "endAt": deadline,
        "status": status,
        "source": "todo",
        "isOverdue": overdue,
        "createdAt": created_at,
    })
}

/// 合并 PM 与 Todo：Todo.pmItemId 已指向 PM 集合内某条时丢弃 Todo（PM 已展示同一事件）。
pub fn merge_and_dedup_items(pm_items: &[Value], todo_items: &[Value], today: &str) -> Vec<Value> {
    let pm_id_set: std::collections::HashSet<i64> = pm_items
        .iter()
        .filter_map(|i| i.get("id").and_then(Value::as_i64))
        .collect();

    let mut merged: Vec<Value> = pm_items.iter().map(|i| pm_to_dashboard(i, today)).collect();
    for todo in todo_items {
        let pm_link = todo.get("pmItemId").and_then(Value::as_i64);
        if let Some(pid) = pm_link {
            if pm_id_set.contains(&pid) {
                continue;
            }
        }
        merged.push(todo_to_dashboard(todo, today));
    }
    merged
}

/// 排序：`pinned desc → isOverdue desc → priority_rank asc → endAt asc → createdAt desc`
pub fn sort_dashboard_items(items: &mut [Value]) {
    items.sort_by(|a, b| {
        let pa = a.get("pinned").and_then(Value::as_bool).unwrap_or(false);
        let pb = b.get("pinned").and_then(Value::as_bool).unwrap_or(false);
        if pa != pb {
            return pb.cmp(&pa);
        }

        let oa = a.get("isOverdue").and_then(Value::as_bool).unwrap_or(false);
        let ob = b.get("isOverdue").and_then(Value::as_bool).unwrap_or(false);
        if oa != ob {
            return ob.cmp(&oa);
        }

        let ra = priority_rank(
            a.get("priority")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_PRIORITY),
        );
        let rb = priority_rank(
            b.get("priority")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_PRIORITY),
        );
        if ra != rb {
            return ra.cmp(&rb);
        }

        let da = a.get("endAt").and_then(Value::as_str).unwrap_or(SENTINEL_DATE);
        let dbb = b.get("endAt").and_then(Value::as_str).unwrap_or(SENTINEL_DATE);
        if da != dbb {
            return da.cmp(dbb);
        }

        let ca = a.get("createdAt").and_then(Value::as_str).unwrap_or("");
        let cb = b.get("createdAt").and_then(Value::as_str).unwrap_or("");
        cb.cmp(ca)
    });
}

/// 渲染人类可读的截止标签：今天 / 明天 / N 月 N 日 / 已逾期 N 天 / None
pub fn format_deadline_label(deadline_date: Option<&str>, today: &str) -> Option<String> {
    let date = deadline_date?;
    let parsed = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    let today_d = NaiveDate::parse_from_str(today, "%Y-%m-%d").ok()?;
    let diff = (parsed - today_d).num_days();
    if diff < 0 {
        Some(format!("已逾期 {} 天", -diff))
    } else if diff == 0 {
        Some("今天".to_string())
    } else if diff == 1 {
        Some("明天".to_string())
    } else {
        Some(format!("{}月{}日", parsed.month(), parsed.day()))
    }
}

/// 跨 PM/Todo 合集计算最近未完成截止距离 now 的小时差（按当地时间 23:59:59 计）。
/// 仅考虑 `is_open_status(status)` 为 true 的项；无候选返回 None。
pub fn compute_nearest_deadline_hours(items: &[Value], now: DateTime<Local>) -> Option<i64> {
    let earliest: Option<NaiveDate> = items
        .iter()
        .filter(|i| {
            let s = i.get("status").and_then(Value::as_str).unwrap_or("");
            is_dashboard_open(s)
        })
        .filter_map(|i| i.get("endAt").and_then(Value::as_str))
        .filter_map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .min();
    let earliest = earliest?;
    let end_local = earliest.and_hms_opt(23, 59, 59)?;
    let end_dt: DateTime<Local> = Local.from_local_datetime(&end_local).single()?;
    Some(end_dt.signed_duration_since(now).num_hours())
}

/// 对 overview + 排序后的 todoList 计算稳定 hash（hex），用于 SetWallpaper 内容短路（design §14.1）。
pub fn compute_dashboard_hash(overview: &Value, todo_list: &[Value]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(overview.to_string().as_bytes());
    hasher.update(b"|");
    for item in todo_list {
        hasher.update(item.to_string().as_bytes());
        hasher.update(b"\n");
    }
    hasher.finalize().to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pm(id: i64, priority: &str, status: &str, end_at: Option<&str>, pinned: bool) -> Value {
        json!({
            "id": id,
            "title": format!("pm-{id}"),
            "priority": priority,
            "status": status,
            "endAt": end_at,
            "pinned": pinned,
            "createdAt": format!("2026-04-01T00:00:0{id}Z"),
        })
    }

    fn todo(
        id: i64,
        priority: &str,
        status: &str,
        event_at: Option<&str>,
        pinned: bool,
        pm_link: Option<i64>,
    ) -> Value {
        json!({
            "id": id,
            "title": format!("todo-{id}"),
            "priority": priority,
            "status": status,
            "eventAt": event_at,
            "pinned": pinned,
            "createdAt": format!("2026-04-01T00:00:0{id}Z"),
            "pmItemId": pm_link,
        })
    }

    #[test]
    fn normalize_deadline_handles_iso_and_date() {
        assert_eq!(
            normalize_deadline_date("2026-05-07T16:00:00Z"),
            Some("2026-05-07".to_string())
        );
        assert_eq!(
            normalize_deadline_date("2026-05-07"),
            Some("2026-05-07".to_string())
        );
        assert_eq!(normalize_deadline_date(""), None);
        assert_eq!(normalize_deadline_date("not-a-date"), None);
    }

    #[test]
    fn overdue_only_when_open_and_past() {
        assert!(is_overdue("todo", Some("2026-05-04"), "2026-05-06"));
        assert!(is_overdue("in_progress", Some("2026-05-04"), "2026-05-06"));
        assert!(!is_overdue("done", Some("2026-05-04"), "2026-05-06"));
        assert!(!is_overdue("completed", Some("2026-05-04"), "2026-05-06"));
        assert!(!is_overdue("todo", Some("2026-05-06"), "2026-05-06"));
        assert!(!is_overdue("todo", None, "2026-05-06"));
    }

    #[test]
    fn merge_drops_todo_when_pm_link_in_set() {
        let pms = vec![pm(10, "P1", "todo", Some("2026-05-10"), false)];
        let todos = vec![
            todo(20, "P0", "pending", Some("2026-05-09"), false, Some(10)),
            todo(21, "P2", "pending", Some("2026-05-12"), false, None),
        ];
        let merged = merge_and_dedup_items(&pms, &todos, "2026-05-06");
        let ids: Vec<&str> = merged
            .iter()
            .map(|v| v["id"].as_str().unwrap_or(""))
            .collect();
        assert_eq!(ids, vec!["pm:10", "todo:21"]);
    }

    #[test]
    fn merge_keeps_todo_when_pm_link_outside_set() {
        let pms: Vec<Value> = vec![];
        // 即使 pmItemId=99 但 PM 集合为空，Todo 仍应保留
        let todos = vec![todo(20, "P0", "pending", None, false, Some(99))];
        let merged = merge_and_dedup_items(&pms, &todos, "2026-05-06");
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0]["id"].as_str().unwrap(), "todo:20");
    }

    #[test]
    fn sort_pinned_first_then_overdue_then_priority() {
        let today = "2026-05-06";
        let mut items = vec![
            // 普通 P0 未逾期
            pm_to_dashboard(&pm(1, "P0", "todo", Some("2026-05-10"), false), today),
            // 普通 P3 已逾期
            pm_to_dashboard(&pm(2, "P3", "todo", Some("2026-04-30"), false), today),
            // pinned P2 未逾期
            pm_to_dashboard(&pm(3, "P2", "todo", Some("2026-05-12"), true), today),
        ];
        sort_dashboard_items(&mut items);
        assert_eq!(items[0]["id"].as_str().unwrap(), "pm:3"); // pinned 最先
        assert_eq!(items[1]["id"].as_str().unwrap(), "pm:2"); // 然后逾期
        assert_eq!(items[2]["id"].as_str().unwrap(), "pm:1"); // 普通 P0
    }

    #[test]
    fn sort_overdue_p3_beats_normal_p0() {
        let today = "2026-05-06";
        let mut items = vec![
            pm_to_dashboard(&pm(1, "P0", "todo", Some("2026-05-10"), false), today),
            pm_to_dashboard(&pm(2, "P3", "todo", Some("2026-04-30"), false), today),
        ];
        sort_dashboard_items(&mut items);
        assert_eq!(items[0]["id"].as_str().unwrap(), "pm:2");
        assert_eq!(items[1]["id"].as_str().unwrap(), "pm:1");
    }

    #[test]
    fn sort_same_priority_uses_endat_then_created() {
        let today = "2026-05-06";
        let mut items = vec![
            pm_to_dashboard(&pm(1, "P1", "todo", Some("2026-05-12"), false), today),
            pm_to_dashboard(&pm(2, "P1", "todo", Some("2026-05-08"), false), today),
            pm_to_dashboard(&pm(3, "P1", "todo", None, false), today),
        ];
        sort_dashboard_items(&mut items);
        assert_eq!(items[0]["id"].as_str().unwrap(), "pm:2");
        assert_eq!(items[1]["id"].as_str().unwrap(), "pm:1");
        assert_eq!(items[2]["id"].as_str().unwrap(), "pm:3"); // 无截止落最后
    }

    #[test]
    fn format_label_today_tomorrow_and_overdue() {
        let today = "2026-05-06";
        assert_eq!(
            format_deadline_label(Some("2026-05-06"), today),
            Some("今天".to_string())
        );
        assert_eq!(
            format_deadline_label(Some("2026-05-07"), today),
            Some("明天".to_string())
        );
        assert_eq!(
            format_deadline_label(Some("2026-05-09"), today),
            Some("5月9日".to_string())
        );
        assert_eq!(
            format_deadline_label(Some("2026-05-04"), today),
            Some("已逾期 2 天".to_string())
        );
        assert_eq!(format_deadline_label(None, today), None);
    }

    #[test]
    fn nearest_deadline_picks_smallest_open_date() {
        let today = "2026-05-06";
        let items = vec![
            pm_to_dashboard(&pm(1, "P1", "todo", Some("2026-05-12"), false), today),
            pm_to_dashboard(&pm(2, "P1", "done", Some("2026-05-04"), false), today), // 已完成不参与
            pm_to_dashboard(&pm(3, "P1", "todo", Some("2026-05-08"), false), today),
        ];
        let now = Local
            .with_ymd_and_hms(2026, 5, 6, 12, 0, 0)
            .single()
            .unwrap();
        let h = compute_nearest_deadline_hours(&items, now).unwrap();
        // 5/8 23:59:59 - 5/6 12:00:00 ≈ 60h，允许 ±1h 的 chrono 取整
        assert!(h >= 59 && h <= 60, "expected ~60h, got {h}");
    }

    #[test]
    fn nearest_deadline_returns_none_when_all_done_or_no_date() {
        let today = "2026-05-06";
        let items = vec![pm_to_dashboard(
            &pm(1, "P1", "done", Some("2026-05-12"), false),
            today,
        )];
        let now = Local
            .with_ymd_and_hms(2026, 5, 6, 12, 0, 0)
            .single()
            .unwrap();
        assert!(compute_nearest_deadline_hours(&items, now).is_none());
    }

    #[test]
    fn dashboard_hash_is_stable_for_same_input() {
        let overview = json!({"completedToday": 1, "totalToday": 3, "p0Pending": 0, "nearestDeadlineHours": 12});
        let list = vec![pm_to_dashboard(
            &pm(1, "P0", "todo", Some("2026-05-10"), false),
            "2026-05-06",
        )];
        let h1 = compute_dashboard_hash(&overview, &list);
        let h2 = compute_dashboard_hash(&overview, &list);
        assert_eq!(h1, h2);
    }

    #[test]
    fn dashboard_hash_changes_when_overview_changes() {
        let v1 = json!({"completedToday": 1, "totalToday": 3});
        let v2 = json!({"completedToday": 2, "totalToday": 3});
        let list: Vec<Value> = vec![];
        assert_ne!(
            compute_dashboard_hash(&v1, &list),
            compute_dashboard_hash(&v2, &list)
        );
    }
}

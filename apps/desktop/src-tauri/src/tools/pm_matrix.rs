use chrono::NaiveDate;
use rusqlite::{Connection, ToSql};
use serde_json::{json, Value};

use super::helpers::db_conn;
use super::pm::{batch_load_tags, parse_i64, parse_string};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Quadrant {
    Q1,
    Q2,
    Q3,
    Q4,
}

fn parse_today(payload: &Value) -> Result<NaiveDate, String> {
    let raw = parse_string(payload, "todayDate").ok_or("todayDate is required")?;
    NaiveDate::parse_from_str(&raw, "%Y-%m-%d").map_err(|e| format!("invalid todayDate: {e}"))
}

fn parse_bool(payload: &Value, key: &str, default: bool) -> bool {
    payload.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn end_at_date(raw: &Option<String>) -> Option<NaiveDate> {
    let text = raw.as_deref()?.trim();
    if text.is_empty() {
        return None;
    }
    NaiveDate::parse_from_str(text.get(0..10)?, "%Y-%m-%d").ok()
}

fn is_important(priority: &str) -> bool {
    matches!(priority, "P0" | "P1")
}

fn is_urgent(end_date: Option<NaiveDate>, today: NaiveDate, threshold_days: i64) -> bool {
    match end_date {
        None => false,
        Some(end) => {
            let days_left = (end - today).num_days();
            days_left <= threshold_days
        }
    }
}

fn classify(
    priority: &str,
    end_date: Option<NaiveDate>,
    today: NaiveDate,
    threshold_days: i64,
) -> Quadrant {
    let important = is_important(priority);
    let urgent = is_urgent(end_date, today, threshold_days);
    match (important, urgent) {
        (true, true) => Quadrant::Q1,
        (true, false) => Quadrant::Q2,
        (false, true) => Quadrant::Q3,
        (false, false) => Quadrant::Q4,
    }
}

fn build_sql(project_id: Option<i64>, hide_completed: bool) -> String {
    let base = "SELECT i.id, i.project_id, i.title, i.description, i.item_type, i.priority,
                i.status, i.start_at, i.end_at, i.pinned, i.sort_order,
                i.completed_at, i.created_at, i.updated_at, i.link_url,
                p.name, p.color, i.started_at, i.testing_at
         FROM pm_items i
         LEFT JOIN pm_projects p ON i.project_id = p.id";

    let mut clauses: Vec<String> = Vec::new();
    if project_id.is_some() {
        clauses.push("i.project_id = ?1".to_string());
    }
    if hide_completed {
        clauses.push("i.status != 'done'".to_string());
    }

    if clauses.is_empty() {
        base.to_string()
    } else {
        format!("{base} WHERE {}", clauses.join(" AND "))
    }
}

fn load_items(
    conn: &Connection,
    project_id: Option<i64>,
    hide_completed: bool,
) -> Result<Vec<Value>, String> {
    let sql = build_sql(project_id, hide_completed);
    let params_owned: Vec<Box<dyn ToSql>> = match project_id {
        Some(pid) => vec![Box::new(pid)],
        None => vec![],
    };

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("prepare item_matrix: {e}"))?;
    let param_refs: Vec<&dyn ToSql> = params_owned.iter().map(|p| p.as_ref()).collect();

    let rows = stmt
        .query_map(param_refs.as_slice(), |r| {
            let id: i64 = r.get(0)?;
            let project_id: i64 = r.get(1)?;
            let title: String = r.get(2)?;
            let description: String = r.get(3)?;
            let item_type: String = r.get(4)?;
            let priority: String = r.get(5)?;
            let status: String = r.get(6)?;
            let start_at: Option<String> = r.get(7)?;
            let end_at: Option<String> = r.get(8)?;
            let pinned: bool = r.get(9)?;
            let sort_order: i64 = r.get(10)?;
            let completed_at: Option<String> = r.get(11)?;
            let created_at: String = r.get(12)?;
            let updated_at: String = r.get(13)?;
            let link_url: Option<String> = r.get(14)?;
            let project_name: Option<String> = r.get(15)?;
            let project_color: Option<String> = r.get(16)?;
            let started_at: Option<String> = r.get(17)?;
            let testing_at: Option<String> = r.get(18)?;

            Ok(json!({
                "id": id,
                "projectId": project_id,
                "title": title,
                "description": description,
                "itemType": item_type,
                "priority": priority,
                "status": status,
                "startAt": start_at,
                "endAt": end_at,
                "pinned": pinned,
                "sortOrder": sort_order,
                "completedAt": completed_at,
                "createdAt": created_at,
                "updatedAt": updated_at,
                "linkUrl": link_url,
                "projectName": project_name,
                "projectColor": project_color,
                "startedAt": started_at,
                "testingAt": testing_at,
            }))
        })
        .map_err(|e| format!("query item_matrix: {e}"))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| format!("read item_matrix row: {e}"))?);
    }
    Ok(items)
}

fn priority_rank(priority: &str) -> i32 {
    match priority {
        "P0" => 0,
        "P1" => 1,
        "P2" => 2,
        _ => 3,
    }
}

fn sort_items(items: &mut [Value]) {
    items.sort_by(|a, b| {
        let pa = priority_rank(a["priority"].as_str().unwrap_or("P3"));
        let pb = priority_rank(b["priority"].as_str().unwrap_or("P3"));
        match pa.cmp(&pb) {
            std::cmp::Ordering::Equal => {
                let ea = a["endAt"].as_str().unwrap_or("9999-12-31");
                let eb = b["endAt"].as_str().unwrap_or("9999-12-31");
                ea.cmp(eb)
            }
            other => other,
        }
    });
}

pub fn item_matrix_bucket(payload: &Value) -> Result<Value, String> {
    let project_id = parse_i64(payload, "projectId");
    let threshold = payload
        .get("urgentThresholdDays")
        .and_then(Value::as_i64)
        .unwrap_or(3)
        .max(0);
    let hide_completed = parse_bool(payload, "hideCompleted", true);
    let today = parse_today(payload)?;

    let conn = db_conn()?;
    let items = load_items(&conn, project_id, hide_completed)?;

    let ids: Vec<i64> = items.iter().filter_map(|v| v["id"].as_i64()).collect();
    let tag_map = batch_load_tags(&conn, &ids);

    let mut q1: Vec<Value> = Vec::new();
    let mut q2: Vec<Value> = Vec::new();
    let mut q3: Vec<Value> = Vec::new();
    let mut q4: Vec<Value> = Vec::new();

    for mut item in items {
        let priority = item["priority"].as_str().unwrap_or("P3").to_string();
        let end_raw = item["endAt"].as_str().map(|s| s.to_string());
        let end_date = end_at_date(&end_raw);
        let quadrant = classify(&priority, end_date, today, threshold);

        let id = item["id"].as_i64().unwrap_or(0);
        let tags = tag_map.get(&id).cloned().unwrap_or_default();
        item.as_object_mut()
            .unwrap()
            .insert("tags".to_string(), json!(tags));

        match quadrant {
            Quadrant::Q1 => q1.push(item),
            Quadrant::Q2 => q2.push(item),
            Quadrant::Q3 => q3.push(item),
            Quadrant::Q4 => q4.push(item),
        }
    }

    sort_items(&mut q1);
    sort_items(&mut q2);
    sort_items(&mut q3);
    sort_items(&mut q4);

    Ok(json!({
        "q1": q1,
        "q2": q2,
        "q3": q3,
        "q4": q4,
        "thresholdDays": threshold,
        "hideCompleted": hide_completed,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn q1_important_and_urgent() {
        let today = make_date("2026-04-19");
        assert!(matches!(
            classify("P0", Some(make_date("2026-04-20")), today, 3),
            Quadrant::Q1
        ));
    }

    #[test]
    fn q1_includes_overdue() {
        let today = make_date("2026-04-19");
        assert!(matches!(
            classify("P1", Some(make_date("2026-04-10")), today, 3),
            Quadrant::Q1
        ));
    }

    #[test]
    fn q2_important_not_urgent_no_due() {
        let today = make_date("2026-04-19");
        assert!(matches!(classify("P0", None, today, 3), Quadrant::Q2));
    }

    #[test]
    fn q2_important_far_future() {
        let today = make_date("2026-04-19");
        assert!(matches!(
            classify("P1", Some(make_date("2026-05-10")), today, 3),
            Quadrant::Q2
        ));
    }

    #[test]
    fn q3_not_important_urgent() {
        let today = make_date("2026-04-19");
        assert!(matches!(
            classify("P2", Some(make_date("2026-04-20")), today, 3),
            Quadrant::Q3
        ));
    }

    #[test]
    fn q4_not_important_not_urgent() {
        let today = make_date("2026-04-19");
        assert!(matches!(classify("P3", None, today, 3), Quadrant::Q4));
    }

    #[test]
    fn threshold_boundary_inclusive() {
        let today = make_date("2026-04-19");
        assert!(matches!(
            classify("P0", Some(make_date("2026-04-22")), today, 3),
            Quadrant::Q1
        ));
        assert!(matches!(
            classify("P0", Some(make_date("2026-04-23")), today, 3),
            Quadrant::Q2
        ));
    }

    #[test]
    fn sql_shape_filters() {
        let sql = build_sql(Some(1), true);
        assert!(sql.contains("i.project_id = ?1"));
        assert!(sql.contains("i.status != 'done'"));
        let sql_open = build_sql(None, false);
        assert!(!sql_open.contains("WHERE"));
    }
}

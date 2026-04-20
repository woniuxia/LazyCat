use chrono::{Local, NaiveDate, TimeZone, Utc};
use rusqlite::{Connection, ToSql};
use serde_json::{json, Value};

use super::helpers::db_conn;
use super::pm::{batch_load_tags, parse_i64, parse_string};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bucket {
    Overdue,
    DueToday,
    InProgress,
    CompletedToday,
    None,
}

struct CandidateRow {
    item: Value,
    status: String,
    end_at: Option<String>,
    started_at: Option<String>,
    completed_at: Option<String>,
}

fn today_bounds_utc(today: NaiveDate) -> Result<(String, String), String> {
    let start_local = today
        .and_hms_opt(0, 0, 0)
        .ok_or("invalid today start")?;
    let end_local = today
        .and_hms_opt(23, 59, 59)
        .ok_or("invalid today end")?;
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

fn parse_today_date(payload: &Value) -> Result<NaiveDate, String> {
    let raw = parse_string(payload, "todayDate").ok_or("todayDate is required")?;
    NaiveDate::parse_from_str(&raw, "%Y-%m-%d").map_err(|e| format!("invalid todayDate: {e}"))
}

fn end_at_date_prefix(value: &Option<String>) -> Option<&str> {
    value
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .and_then(|s| s.get(0..10))
}

fn classify(
    row: &CandidateRow,
    today_str: &str,
    today_start_utc: &str,
    today_end_utc: &str,
) -> Bucket {
    let completed_in_today = row
        .completed_at
        .as_deref()
        .map(|c| c >= today_start_utc && c <= today_end_utc)
        .unwrap_or(false);

    if completed_in_today {
        return Bucket::CompletedToday;
    }

    if row.status == "done" {
        return Bucket::None;
    }

    if let Some(date) = end_at_date_prefix(&row.end_at) {
        if date < today_str {
            return Bucket::Overdue;
        }
        if date == today_str {
            return Bucket::DueToday;
        }
    }

    if row.started_at.is_some() {
        return Bucket::InProgress;
    }

    Bucket::None
}

fn candidate_sql(project_id: Option<i64>) -> &'static str {
    if project_id.is_some() {
        "SELECT i.id, i.project_id, i.title, i.description, i.item_type, i.priority,
                i.status, i.start_at, i.end_at, i.pinned, i.sort_order,
                i.completed_at, i.created_at, i.updated_at, i.link_url,
                p.name, p.color, i.started_at, i.testing_at
         FROM pm_items i
         LEFT JOIN pm_projects p ON i.project_id = p.id
         WHERE i.project_id = ?1
           AND (i.status != 'done' OR (i.completed_at >= ?2 AND i.completed_at <= ?3))"
    } else {
        "SELECT i.id, i.project_id, i.title, i.description, i.item_type, i.priority,
                i.status, i.start_at, i.end_at, i.pinned, i.sort_order,
                i.completed_at, i.created_at, i.updated_at, i.link_url,
                p.name, p.color, i.started_at, i.testing_at
         FROM pm_items i
         LEFT JOIN pm_projects p ON i.project_id = p.id
         WHERE (i.status != 'done' OR (i.completed_at >= ?1 AND i.completed_at <= ?2))"
    }
}

fn load_candidate_rows(
    conn: &Connection,
    project_id: Option<i64>,
    today_start_utc: &str,
    today_end_utc: &str,
) -> Result<Vec<CandidateRow>, String> {
    let sql = candidate_sql(project_id);
    let params_owned: Vec<Box<dyn ToSql>> = if let Some(pid) = project_id {
        vec![
            Box::new(pid),
            Box::new(today_start_utc.to_string()),
            Box::new(today_end_utc.to_string()),
        ]
    } else {
        vec![
            Box::new(today_start_utc.to_string()),
            Box::new(today_end_utc.to_string()),
        ]
    };

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("prepare item_today: {e}"))?;
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

            let item = json!({
                "id": id,
                "projectId": project_id,
                "title": title,
                "description": description,
                "itemType": item_type,
                "priority": priority,
                "status": status.clone(),
                "startAt": start_at,
                "endAt": end_at.clone(),
                "pinned": pinned,
                "sortOrder": sort_order,
                "completedAt": completed_at.clone(),
                "createdAt": created_at,
                "updatedAt": updated_at,
                "linkUrl": link_url,
                "projectName": project_name,
                "projectColor": project_color,
                "startedAt": started_at.clone(),
                "testingAt": testing_at,
            });

            Ok(CandidateRow {
                item,
                status,
                end_at,
                started_at,
                completed_at,
            })
        })
        .map_err(|e| format!("query item_today: {e}"))?;

    let mut list = Vec::new();
    for row in rows {
        list.push(row.map_err(|e| format!("read item_today row: {e}"))?);
    }
    Ok(list)
}

fn priority_rank(priority: &str) -> i32 {
    match priority {
        "P0" => 0,
        "P1" => 1,
        "P2" => 2,
        _ => 3,
    }
}

fn sort_by_priority_then_end(items: &mut [Value]) {
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

fn sort_by_completed_desc(items: &mut [Value]) {
    items.sort_by(|a, b| {
        let ca = a["completedAt"].as_str().unwrap_or("");
        let cb = b["completedAt"].as_str().unwrap_or("");
        cb.cmp(ca)
    });
}

pub fn item_today_list(payload: &Value) -> Result<Value, String> {
    let project_id = parse_i64(payload, "projectId");
    let today = parse_today_date(payload)?;
    let today_str = today.format("%Y-%m-%d").to_string();
    let (today_start_utc, today_end_utc) = today_bounds_utc(today)?;

    let conn = db_conn()?;
    let rows = load_candidate_rows(&conn, project_id, &today_start_utc, &today_end_utc)?;

    let mut overdue: Vec<Value> = Vec::new();
    let mut due_today: Vec<Value> = Vec::new();
    let mut in_progress: Vec<Value> = Vec::new();
    let mut completed_today: Vec<Value> = Vec::new();
    let mut unscheduled: Vec<Value> = Vec::new();

    let mut collected_ids: Vec<i64> = Vec::new();

    for row in rows {
        let is_unscheduled = if row.status != "done" {
            let start_at_val = row
                .item
                .get("startAt")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("");
            start_at_val.is_empty()
        } else {
            false
        };

        if is_unscheduled {
            if let Some(id) = row.item["id"].as_i64() {
                collected_ids.push(id);
            }
            unscheduled.push(row.item.clone());
        }

        let bucket = classify(&row, &today_str, &today_start_utc, &today_end_utc);
        if bucket == Bucket::None {
            continue;
        }
        if let Some(id) = row.item["id"].as_i64() {
            collected_ids.push(id);
        }
        match bucket {
            Bucket::Overdue => overdue.push(row.item),
            Bucket::DueToday => due_today.push(row.item),
            Bucket::InProgress => in_progress.push(row.item),
            Bucket::CompletedToday => completed_today.push(row.item),
            Bucket::None => {}
        }
    }

    let tag_map = batch_load_tags(&conn, &collected_ids);
    let inject_tags = |list: &mut [Value]| {
        for item in list.iter_mut() {
            let id = item["id"].as_i64().unwrap_or(0);
            let tags = tag_map.get(&id).cloned().unwrap_or_default();
            item.as_object_mut()
                .unwrap()
                .insert("tags".to_string(), json!(tags));
        }
    };
    let mut overdue = overdue;
    let mut due_today = due_today;
    let mut in_progress = in_progress;
    let mut completed_today = completed_today;
    let mut unscheduled = unscheduled;
    inject_tags(&mut overdue);
    inject_tags(&mut due_today);
    inject_tags(&mut in_progress);
    inject_tags(&mut completed_today);
    inject_tags(&mut unscheduled);

    sort_by_priority_then_end(&mut overdue);
    sort_by_priority_then_end(&mut due_today);
    sort_by_priority_then_end(&mut in_progress);
    sort_by_completed_desc(&mut completed_today);
    sort_by_priority_then_end(&mut unscheduled);

    let unscheduled_count = unscheduled.len() as u32;

    Ok(json!({
        "overdue": overdue,
        "dueToday": due_today,
        "inProgress": in_progress,
        "completedToday": completed_today,
        "unscheduled": unscheduled,
        "unscheduledCount": unscheduled_count,
    }))
}

pub fn item_today_counts(payload: &Value) -> Result<Value, String> {
    let project_id = parse_i64(payload, "projectId");
    let today = parse_today_date(payload)?;
    let today_str = today.format("%Y-%m-%d").to_string();
    let (today_start_utc, today_end_utc) = today_bounds_utc(today)?;

    let conn = db_conn()?;
    let rows = load_candidate_rows(&conn, project_id, &today_start_utc, &today_end_utc)?;

    let mut overdue: u32 = 0;
    let mut due_today_count: u32 = 0;
    let mut in_progress: u32 = 0;
    let mut completed_today: u32 = 0;
    let mut total_active: u32 = 0;

    for row in &rows {
        if row.status != "done" {
            total_active += 1;
        }
        match classify(row, &today_str, &today_start_utc, &today_end_utc) {
            Bucket::Overdue => overdue += 1,
            Bucket::DueToday => due_today_count += 1,
            Bucket::InProgress => in_progress += 1,
            Bucket::CompletedToday => completed_today += 1,
            Bucket::None => {}
        }
    }

    Ok(json!({
        "overdue": overdue,
        "dueToday": due_today_count,
        "inProgress": in_progress,
        "completedToday": completed_today,
        "totalActive": total_active,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        id: i64,
        status: &str,
        end_at: Option<&str>,
        started_at: Option<&str>,
        completed_at: Option<&str>,
    ) -> CandidateRow {
        CandidateRow {
            item: json!({ "id": id, "priority": "P2", "endAt": end_at, "completedAt": completed_at }),
            status: status.to_string(),
            end_at: end_at.map(str::to_string),
            started_at: started_at.map(str::to_string),
            completed_at: completed_at.map(str::to_string),
        }
    }

    #[test]
    fn classify_overdue_by_end_at() {
        let row = candidate(1, "todo", Some("2026-04-10"), None, None);
        assert!(matches!(
            classify(&row, "2026-04-19", "2026-04-18T16:00:00+00:00", "2026-04-19T15:59:59+00:00"),
            Bucket::Overdue,
        ));
    }

    #[test]
    fn classify_due_today_on_end_at_match() {
        let row = candidate(2, "in_progress", Some("2026-04-19"), Some("2026-04-19T00:00:00+00:00"), None);
        assert!(matches!(
            classify(&row, "2026-04-19", "2026-04-18T16:00:00+00:00", "2026-04-19T15:59:59+00:00"),
            Bucket::DueToday,
        ));
    }

    #[test]
    fn classify_in_progress_when_started_but_future_end() {
        let row = candidate(3, "in_progress", Some("2026-04-25"), Some("2026-04-18T00:00:00+00:00"), None);
        assert!(matches!(
            classify(&row, "2026-04-19", "2026-04-18T16:00:00+00:00", "2026-04-19T15:59:59+00:00"),
            Bucket::InProgress,
        ));
    }

    #[test]
    fn classify_completed_today_wins() {
        let row = candidate(
            4,
            "done",
            Some("2026-04-10"),
            Some("2026-04-18T00:00:00+00:00"),
            Some("2026-04-19T05:00:00+00:00"),
        );
        assert!(matches!(
            classify(&row, "2026-04-19", "2026-04-18T16:00:00+00:00", "2026-04-19T15:59:59+00:00"),
            Bucket::CompletedToday,
        ));
    }

    #[test]
    fn classify_done_outside_today_skipped() {
        let row = candidate(
            5,
            "done",
            Some("2026-04-10"),
            Some("2026-04-18T00:00:00+00:00"),
            Some("2026-04-15T05:00:00+00:00"),
        );
        assert!(matches!(
            classify(&row, "2026-04-19", "2026-04-18T16:00:00+00:00", "2026-04-19T15:59:59+00:00"),
            Bucket::None,
        ));
    }

    #[test]
    fn classify_no_end_no_start_no_bucket() {
        let row = candidate(6, "todo", None, None, None);
        assert!(matches!(
            classify(&row, "2026-04-19", "2026-04-18T16:00:00+00:00", "2026-04-19T15:59:59+00:00"),
            Bucket::None,
        ));
    }

    #[test]
    fn priority_rank_sorts_correctly() {
        let mut items = vec![
            json!({ "priority": "P3", "endAt": "2026-04-20" }),
            json!({ "priority": "P0", "endAt": "2026-04-25" }),
            json!({ "priority": "P2", "endAt": "2026-04-19" }),
        ];
        sort_by_priority_then_end(&mut items);
        assert_eq!(items[0]["priority"], "P0");
        assert_eq!(items[1]["priority"], "P2");
        assert_eq!(items[2]["priority"], "P3");
    }
}

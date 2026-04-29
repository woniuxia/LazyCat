use chrono::{Local, NaiveDate, Utc};
use rusqlite::params;
use serde_json::{json, Value};

use super::helpers::db_conn;

pub fn format_pm_weekly_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub fn parse_pm_weekly_date(value: Option<&str>) -> Option<NaiveDate> {
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        return None;
    }

    let prefix = trimmed.get(0..10)?;
    NaiveDate::parse_from_str(prefix, "%Y-%m-%d").ok()
}

pub fn normalize_pm_weekly_range(
    start_at: Option<&str>,
    end_at: Option<&str>,
) -> Option<(NaiveDate, NaiveDate)> {
    let start = parse_pm_weekly_date(start_at);
    let end = parse_pm_weekly_date(end_at);

    match (start, end) {
        (Some(start), Some(end)) if start <= end => Some((start, end)),
        (Some(start), Some(end)) => Some((end, start)),
        (Some(date), None) | (None, Some(date)) => Some((date, date)),
        (None, None) => None,
    }
}

pub fn resolve_pm_weekly_window_hit(
    start_at: Option<&str>,
    end_at: Option<&str>,
    week_start: NaiveDate,
    week_end: NaiveDate,
) -> Option<(NaiveDate, NaiveDate, NaiveDate)> {
    let (normalized_start, normalized_end) = normalize_pm_weekly_range(start_at, end_at)?;
    if normalized_end < week_start || normalized_start > week_end {
        return None;
    }

    Some((normalized_start, normalized_end, std::cmp::min(normalized_end, week_end)))
}

pub fn resolve_current_week_window(
    now_local: chrono::DateTime<Local>,
) -> Result<(NaiveDate, NaiveDate, String, String, String), String> {
    let window_end = now_local.date_naive();
    let window_start = window_end - chrono::Duration::days(6);
    let next_start = window_end + chrono::Duration::days(1);

    let window_start_utc = window_start
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .single()
        .map(|dt| dt.with_timezone(&Utc))
        .ok_or("window start timezone conversion failed")?;
    let window_end_utc = window_end
        .and_hms_opt(23, 59, 59)
        .unwrap()
        .and_local_timezone(Local)
        .single()
        .map(|dt| dt.with_timezone(&Utc))
        .ok_or("window end timezone conversion failed")?;
    let next_start_utc = next_start
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .single()
        .map(|dt| dt.with_timezone(&Utc))
        .ok_or("next start timezone conversion failed")?;

    Ok((
        window_start,
        window_end,
        window_start_utc.to_rfc3339(),
        window_end_utc.to_rfc3339(),
        next_start_utc.to_rfc3339(),
    ))
}

fn resolve_custom_window(
    window_start_str: &str,
    window_end_str: &str,
) -> Result<(NaiveDate, NaiveDate, String, String, String), String> {
    let window_start = parse_pm_weekly_date(Some(window_start_str))
        .ok_or_else(|| format!("invalid windowStart: {window_start_str}"))?;
    let window_end = parse_pm_weekly_date(Some(window_end_str))
        .ok_or_else(|| format!("invalid windowEnd: {window_end_str}"))?;
    if window_start > window_end {
        return Err("windowStart must be <= windowEnd".into());
    }
    let next_start = window_end + chrono::Duration::days(1);
    let fmt_naive = |d: NaiveDate, h: u32, m: u32, s: u32| -> Result<String, String> {
        d.and_hms_opt(h, m, s)
            .unwrap()
            .and_local_timezone(Local)
            .single()
            .map(|dt| dt.with_timezone(&Utc).to_rfc3339())
            .ok_or("timezone conversion failed".into())
    };
    Ok((
        window_start,
        window_end,
        fmt_naive(window_start, 0, 0, 0)?,
        fmt_naive(window_end, 23, 59, 59)?,
        fmt_naive(next_start, 0, 0, 0)?,
    ))
}

pub fn weekly_work(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let (window_start, window_end, window_start_str, window_end_str, next_start_str) =
        if let (Some(ws), Some(we)) = (
            payload.get("windowStart").and_then(|v| v.as_str()),
            payload.get("windowEnd").and_then(|v| v.as_str()),
        ) {
            resolve_custom_window(ws, we)?
        } else {
            resolve_current_week_window(Local::now())?
        };

    let mut pm_stmt = conn
        .prepare(
            "SELECT i.id, i.project_id, i.title, i.item_type, i.priority, i.status,
                    i.start_at, i.end_at, i.completed_at, i.created_at,
                    p.name as project_name, p.color as project_color, p.status as project_status
             FROM pm_items i
             JOIN pm_projects p ON i.project_id = p.id
             WHERE i.start_at IS NOT NULL OR i.end_at IS NOT NULL
             ORDER BY i.id DESC",
        )
        .map_err(|e| format!("weekly_work pm: {e}"))?;

    let mut pm_items: Vec<(String, Value)> = pm_stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, String>(9)?,
                r.get::<_, String>(10)?,
                r.get::<_, String>(11)?,
                r.get::<_, String>(12)?,
            ))
        })
        .map_err(|e| format!("weekly_work pm query: {e}"))?
        .filter_map(|row| {
            let (
                id,
                project_id,
                title,
                item_type,
                priority,
                status,
                start_at,
                end_at,
                completed_at,
                created_at,
                project_name,
                project_color,
                project_status,
            ) = row.ok()?;
            let (normalized_start, normalized_end, sort_date) =
                resolve_pm_weekly_window_hit(start_at.as_deref(), end_at.as_deref(), window_start, window_end)?;
            let sort_at = format_pm_weekly_date(sort_date);
            Some((
                sort_at.clone(),
                json!({
                    "id": id,
                    "projectId": project_id,
                    "title": title,
                    "itemType": item_type,
                    "priority": priority,
                    "status": status,
                    "startAt": format_pm_weekly_date(normalized_start),
                    "endAt": format_pm_weekly_date(normalized_end),
                    "sortAt": sort_at,
                    "completedAt": completed_at,
                    "createdAt": created_at,
                    "projectName": project_name,
                    "projectColor": project_color,
                    "projectStatus": project_status,
                    "source": "pm",
                }),
            ))
        })
        .collect();
    pm_items.sort_by(|left, right| right.0.cmp(&left.0));
    let pm_items: Vec<Value> = pm_items.into_iter().map(|(_, value)| value).collect();

    let has_project_col = conn
        .prepare("SELECT project_id FROM todo_items LIMIT 0")
        .is_ok();

    let todo_items: Vec<Value> = if has_project_col {
        let mut todo_stmt = conn
            .prepare(
                "SELECT t.id, t.title, t.priority, t.status, t.completed_at, t.created_at,
                        t.project_id,
                        p.name as project_name, p.color as project_color, p.status as project_status
                 FROM todo_items t
                 LEFT JOIN pm_projects p ON t.project_id = p.id
                 WHERE t.status = 'completed' AND t.completed_at >= ?1 AND t.completed_at < ?2
                 ORDER BY t.completed_at DESC",
            )
            .map_err(|e| format!("weekly_work todo: {e}"))?;

        let result: Vec<Value> = todo_stmt
            .query_map(params![window_start_str, next_start_str], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "title": r.get::<_, String>(1)?,
                    "priority": r.get::<_, String>(2)?,
                    "status": r.get::<_, String>(3)?,
                    "completedAt": r.get::<_, Option<String>>(4)?,
                    "sortAt": r.get::<_, Option<String>>(4)?,
                    "createdAt": r.get::<_, String>(5)?,
                    "projectId": r.get::<_, Option<i64>>(6)?,
                    "projectName": r.get::<_, Option<String>>(7)?,
                    "projectColor": r.get::<_, Option<String>>(8)?,
                    "projectStatus": r.get::<_, Option<String>>(9)?,
                    "source": "todo",
                }))
            })
            .map_err(|e| format!("weekly_work todo query: {e}"))?
            .filter_map(|r| r.ok())
            .collect();
        result
    } else {
        let mut todo_stmt = conn
            .prepare(
                "SELECT t.id, t.title, t.priority, t.status, t.completed_at, t.created_at
                 FROM todo_items t
                 WHERE t.status = 'completed' AND t.completed_at >= ?1 AND t.completed_at < ?2
                 ORDER BY t.completed_at DESC",
            )
            .map_err(|e| format!("weekly_work todo: {e}"))?;

        let result: Vec<Value> = todo_stmt
            .query_map(params![window_start_str, next_start_str], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "title": r.get::<_, String>(1)?,
                    "priority": r.get::<_, String>(2)?,
                    "status": r.get::<_, String>(3)?,
                    "completedAt": r.get::<_, Option<String>>(4)?,
                    "sortAt": r.get::<_, Option<String>>(4)?,
                    "createdAt": r.get::<_, String>(5)?,
                    "projectId": null,
                    "projectName": null,
                    "projectColor": null,
                    "projectStatus": null,
                    "source": "todo",
                }))
            })
            .map_err(|e| format!("weekly_work todo query: {e}"))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    Ok(json!({
        "pmItems": pm_items,
        "todoItems": todo_items,
        "windowStart": window_start_str,
        "windowEnd": window_end_str,
    }))
}

use chrono::NaiveDate;
use rusqlite::{Connection, ToSql};
use serde_json::{json, Value};

use super::helpers::db_conn;
use super::pm::{batch_load_tags, parse_i64, parse_string};

fn parse_date_field(payload: &Value, key: &str) -> Result<NaiveDate, String> {
    let raw = parse_string(payload, key).ok_or_else(|| format!("{key} is required"))?;
    NaiveDate::parse_from_str(&raw, "%Y-%m-%d").map_err(|e| format!("invalid {key}: {e}"))
}

fn build_range_sql(project_id: Option<i64>) -> &'static str {
    if project_id.is_some() {
        "SELECT i.id, i.project_id, i.title, i.description, i.item_type, i.priority,
                i.status, i.start_at, i.end_at, i.pinned, i.sort_order,
                i.completed_at, i.created_at, i.updated_at, i.link_url,
                p.name, p.color, i.started_at, i.testing_at
         FROM pm_items i
         LEFT JOIN pm_projects p ON i.project_id = p.id
         WHERE i.project_id = ?1
           AND (
                 (i.end_at IS NOT NULL
                   AND substr(i.end_at, 1, 10) >= ?2
                   AND substr(i.end_at, 1, 10) <= ?3)
              OR (i.start_at IS NOT NULL
                   AND substr(i.start_at, 1, 10) >= ?2
                   AND substr(i.start_at, 1, 10) <= ?3)
              OR (i.start_at IS NOT NULL AND i.end_at IS NOT NULL
                   AND substr(i.start_at, 1, 10) < ?2
                   AND substr(i.end_at, 1, 10) > ?3)
           )"
    } else {
        "SELECT i.id, i.project_id, i.title, i.description, i.item_type, i.priority,
                i.status, i.start_at, i.end_at, i.pinned, i.sort_order,
                i.completed_at, i.created_at, i.updated_at, i.link_url,
                p.name, p.color, i.started_at, i.testing_at
         FROM pm_items i
         LEFT JOIN pm_projects p ON i.project_id = p.id
         WHERE (
                 (i.end_at IS NOT NULL
                   AND substr(i.end_at, 1, 10) >= ?1
                   AND substr(i.end_at, 1, 10) <= ?2)
              OR (i.start_at IS NOT NULL
                   AND substr(i.start_at, 1, 10) >= ?1
                   AND substr(i.start_at, 1, 10) <= ?2)
              OR (i.start_at IS NOT NULL AND i.end_at IS NOT NULL
                   AND substr(i.start_at, 1, 10) < ?1
                   AND substr(i.end_at, 1, 10) > ?2)
           )"
    }
}

fn load_items_in_range(
    conn: &Connection,
    project_id: Option<i64>,
    start_date: &str,
    end_date: &str,
) -> Result<Vec<Value>, String> {
    let sql = build_range_sql(project_id);
    let params_owned: Vec<Box<dyn ToSql>> = if let Some(pid) = project_id {
        vec![
            Box::new(pid),
            Box::new(start_date.to_string()),
            Box::new(end_date.to_string()),
        ]
    } else {
        vec![
            Box::new(start_date.to_string()),
            Box::new(end_date.to_string()),
        ]
    };

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("prepare item_calendar_range: {e}"))?;
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
        .map_err(|e| format!("query item_calendar_range: {e}"))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| format!("read item_calendar_range row: {e}"))?);
    }
    Ok(items)
}

pub fn item_calendar_range(payload: &Value) -> Result<Value, String> {
    let project_id = parse_i64(payload, "projectId");
    let start_date = parse_date_field(payload, "startDate")?;
    let end_date = parse_date_field(payload, "endDate")?;
    if end_date < start_date {
        return Err("endDate must be on or after startDate".into());
    }
    let start_str = start_date.format("%Y-%m-%d").to_string();
    let end_str = end_date.format("%Y-%m-%d").to_string();

    let conn = db_conn()?;
    let mut items = load_items_in_range(&conn, project_id, &start_str, &end_str)?;

    let ids: Vec<i64> = items.iter().filter_map(|v| v["id"].as_i64()).collect();
    let tag_map = batch_load_tags(&conn, &ids);
    for item in items.iter_mut() {
        let id = item["id"].as_i64().unwrap_or(0);
        let tags = tag_map.get(&id).cloned().unwrap_or_default();
        item.as_object_mut()
            .unwrap()
            .insert("tags".to_string(), json!(tags));
    }

    Ok(json!({ "items": items }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_date_field_valid() {
        let payload = json!({ "startDate": "2026-04-19" });
        let d = parse_date_field(&payload, "startDate").expect("date");
        assert_eq!(d.format("%Y-%m-%d").to_string(), "2026-04-19");
    }

    #[test]
    fn parse_date_field_missing() {
        let payload = json!({});
        let err = parse_date_field(&payload, "startDate").expect_err("missing");
        assert!(err.contains("required"));
    }

    #[test]
    fn parse_date_field_invalid() {
        let payload = json!({ "startDate": "2026/04/19" });
        assert!(parse_date_field(&payload, "startDate").is_err());
    }

    #[test]
    fn build_range_sql_shape() {
        let sql_with = build_range_sql(Some(7));
        assert!(sql_with.contains("i.project_id = ?1"));
        assert!(sql_with.contains("?2"));
        assert!(sql_with.contains("?3"));
        let sql_without = build_range_sql(None);
        assert!(!sql_without.contains("i.project_id = ?1"));
        assert!(sql_without.contains("?1"));
        assert!(sql_without.contains("?2"));
    }
}

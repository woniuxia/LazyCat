use chrono::{Duration, Utc};
use serde_json::{json, Value};

use super::helpers::db_conn;
use super::pm::{batch_load_siyuan_links, batch_load_tags};

const PM_ACTIVE_STATUSES: [&str; 3] = ["todo", "in_progress", "testing"];
const TODO_ACTIVE_STATUSES: [&str; 2] = ["pending", "in_progress"];

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "list" => unified_list(payload),
        _ => Err(format!("unsupported unified action: {action}")),
    }
}

fn unified_list(_payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let seven_days_ago = (Utc::now() - Duration::days(7)).format("%Y-%m-%dT%H:%M:%S%.fZ").to_string();

    // ── Phase 1: PM items ───────────────────────────────────
    let pm_sql = format!(
        "SELECT i.id, i.title, i.description, i.item_type, i.priority,
                i.status, i.start_at, i.end_at, i.pinned, i.sort_order,
                i.completed_at, i.created_at, i.updated_at,
                i.link_url, i.ref_code, i.started_at, i.testing_at,
                i.siyuan_doc_id, i.siyuan_doc_title, i.siyuan_doc_hpath,
                i.siyuan_doc_path, i.siyuan_notebook_id, i.siyuan_notebook_name,
                i.project_id, p.name AS project_name, p.color AS project_color
         FROM pm_items i
         LEFT JOIN pm_projects p ON i.project_id = p.id
         WHERE i.status IN ({})
            OR (i.status = 'done' AND i.completed_at >= ?1)
         ORDER BY i.id DESC",
        PM_ACTIVE_STATUSES.iter().map(|s| format!("'{}'", s)).collect::<Vec<_>>().join(",")
    );

    let mut stmt = conn.prepare(&pm_sql).map_err(|e| format!("prepare unified pm: {e}"))?;
    let mut pm_items: Vec<Value> = Vec::new();
    let rows = stmt.query_map([&seven_days_ago], |r| {
        Ok((
            r.get::<_, i64>(0)?,            // id
            r.get::<_, String>(1)?,          // title
            r.get::<_, String>(2)?,          // description
            r.get::<_, String>(3)?,          // item_type
            r.get::<_, String>(4)?,          // priority
            r.get::<_, String>(5)?,          // status
            r.get::<_, Option<String>>(6)?,  // start_at
            r.get::<_, Option<String>>(7)?,  // end_at
            r.get::<_, bool>(8)?,            // pinned
            r.get::<_, i64>(9)?,             // sort_order
            r.get::<_, Option<String>>(10)?, // completed_at
            r.get::<_, String>(11)?,         // created_at
            r.get::<_, String>(12)?,         // updated_at
            r.get::<_, Option<String>>(13)?, // link_url
            r.get::<_, Option<String>>(14)?, // ref_code
            r.get::<_, Option<String>>(15)?, // started_at
            r.get::<_, Option<String>>(16)?, // testing_at
            r.get::<_, Option<String>>(17)?, // siyuan_doc_id
            r.get::<_, Option<String>>(18)?, // siyuan_doc_title
            r.get::<_, Option<String>>(19)?, // siyuan_doc_hpath
            r.get::<_, Option<String>>(20)?, // siyuan_doc_path
            r.get::<_, Option<String>>(21)?, // siyuan_notebook_id
            r.get::<_, Option<String>>(22)?, // siyuan_notebook_name
            r.get::<_, i64>(23)?,            // project_id
            r.get::<_, Option<String>>(24)?, // project_name
            r.get::<_, Option<String>>(25)?, // project_color
        ))
    }).map_err(|e| format!("query unified pm: {e}"))?;

    for row in rows {
        let (id, title, description, item_type, priority, status, start_at, end_at,
             pinned, sort_order, completed_at, created_at, updated_at,
             link_url, ref_code, started_at, testing_at,
             siyuan_doc_id, siyuan_doc_title, siyuan_doc_hpath,
             siyuan_doc_path, siyuan_notebook_id, siyuan_notebook_name,
             project_id, project_name, project_color) = row.map_err(|e| e.to_string())?;

        let display_at = end_at.clone().or_else(|| start_at.clone()).unwrap_or_else(|| created_at.clone());

        let primary_page = crate::tools::pm_siyuan::build_siyuan_page_ref_from_parts(
            siyuan_doc_id,
            siyuan_doc_title,
            siyuan_doc_hpath,
            siyuan_doc_path,
            siyuan_notebook_id,
            siyuan_notebook_name,
        );

        pm_items.push(json!({
            "id": id,
            "source": "pm",
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
            "displayAt": display_at,
            "linkUrl": link_url,
            "refCode": ref_code,
            "startedAt": started_at,
            "testingAt": testing_at,
            "siyuanPrimaryPage": primary_page,
            "projectId": project_id,
            "projectName": project_name,
            "projectColor": project_color,
        }));
    }

    // ── Phase 2: PM tags & siyuan extra pages ────────────────
    let pm_ids: Vec<i64> = pm_items.iter().filter_map(|v| v["id"].as_i64()).collect();
    let tag_map = batch_load_tags(&conn, &pm_ids);
    let siyuan_links_map = batch_load_siyuan_links(&conn, &pm_ids);
    for item in pm_items.iter_mut() {
        let item_id = item["id"].as_i64().unwrap_or(0);
        let tags = tag_map.get(&item_id).cloned().unwrap_or_default();
        let extra_pages = siyuan_links_map.get(&item_id).cloned().unwrap_or_default();
        if let Some(obj) = item.as_object_mut() {
            obj.insert("tags".to_string(), json!(tags));
            obj.insert("siyuanExtraPages".to_string(), json!(extra_pages));
        }
    }

    // ── Phase 3: Todo items ──────────────────────────────────
    let todo_sql = format!(
        "SELECT i.id, i.title, i.type_id, i.priority, i.description, i.status,
                i.event_at, i.pinned, i.kind, i.series_id,
                i.created_at, i.updated_at, i.completed_at,
                ty.name AS type_name, ty.color AS type_color,
                sr.rule_mode, sr.rule_json, sr.cron_expression, sr.timezone,
                sr.start_at, sr.end_mode, sr.end_value, sr.occurrence_index, sr.active,
                i.project_id, pm.name AS project_name, pm.color AS project_color
         FROM todo_items i
         LEFT JOIN todo_types ty ON ty.id = i.type_id
         LEFT JOIN todo_series_rules sr ON sr.series_id = i.series_id
         LEFT JOIN pm_projects pm ON pm.id = i.project_id
         WHERE i.status IN ({})
         ORDER BY i.id DESC",
        TODO_ACTIVE_STATUSES.iter().map(|s| format!("'{}'", s)).collect::<Vec<_>>().join(",")
    );

    let mut stmt = conn.prepare(&todo_sql).map_err(|e| format!("prepare unified todo: {e}"))?;
    let mut todo_items: Vec<Value> = Vec::new();
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,            // id
            r.get::<_, String>(1)?,          // title
            r.get::<_, Option<i64>>(2)?,     // type_id
            r.get::<_, String>(3)?,          // priority
            r.get::<_, String>(4)?,          // description
            r.get::<_, String>(5)?,          // status
            r.get::<_, Option<String>>(6)?,  // event_at
            r.get::<_, bool>(7)?,            // pinned
            r.get::<_, String>(8)?,          // kind
            r.get::<_, Option<i64>>(9)?,     // series_id
            r.get::<_, String>(10)?,         // created_at
            r.get::<_, String>(11)?,         // updated_at
            r.get::<_, Option<String>>(12)?, // completed_at
            r.get::<_, Option<String>>(13)?, // type_name
            r.get::<_, Option<String>>(14)?, // type_color
            r.get::<_, Option<String>>(15)?, // rule_mode
            r.get::<_, Option<String>>(16)?, // rule_json
            r.get::<_, Option<String>>(17)?, // cron_expression
            r.get::<_, Option<String>>(18)?, // timezone
            r.get::<_, Option<String>>(19)?, // start_at
            r.get::<_, Option<String>>(20)?, // end_mode
            r.get::<_, Option<String>>(21)?, // end_value
            r.get::<_, Option<i64>>(22)?,    // occurrence_index
            r.get::<_, Option<i64>>(23)?,    // active
            r.get::<_, Option<i64>>(24)?,    // project_id
            r.get::<_, Option<String>>(25)?, // project_name
            r.get::<_, Option<String>>(26)?, // project_color
        ))
    }).map_err(|e| format!("query unified todo: {e}"))?;

    for row in rows {
        let (id, title, type_id, priority, description, status, event_at, pinned, kind,
             series_id, created_at, updated_at, completed_at,
             type_name, type_color,
             rule_mode, rule_json, cron_expression, timezone,
             start_at, end_mode, end_value, occurrence_index, active,
             project_id, project_name, project_color) = row.map_err(|e| e.to_string())?;

        let display_at = event_at.clone().unwrap_or_else(|| created_at.clone());

        let root_id = series_id.unwrap_or(id);
        let rule_active_bool = active.map(|v| v == 1).unwrap_or(true);

        let recurrence = if kind == "recurring" && rule_mode.is_some() {
            let rule_json_parsed = rule_json
                .as_deref()
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .unwrap_or_else(|| json!({}));
            json!({
                "startAt": start_at,
                "ruleMode": rule_mode,
                "rule": rule_json_parsed,
                "cronExpression": cron_expression,
                "timezone": timezone,
                "endMode": end_mode.unwrap_or_else(|| "never".to_string()),
                "endValue": end_value,
                "occurrenceIndex": occurrence_index.unwrap_or(1),
                "active": rule_active_bool,
            })
        } else {
            Value::Null
        };

        todo_items.push(json!({
            "id": id,
            "source": "todo",
            "title": title,
            "description": description,
            "priority": priority,
            "status": status,
            "pinned": pinned,
            "kind": kind,
            "rootId": root_id,
            "typeId": type_id,
            "typeName": type_name,
            "typeColor": type_color,
            "eventAt": event_at,
            "recurrence": recurrence,
            "completedAt": completed_at,
            "createdAt": created_at,
            "updatedAt": updated_at,
            "displayAt": display_at,
            "projectId": project_id,
            "projectName": project_name,
            "projectColor": project_color,
        }));
    }

    let pm_count = pm_items.len();
    let todo_count = todo_items.len();

    // ── Phase 4: Merge & sort ─────────────────────────────────
    let mut all: Vec<Value> = pm_items.into_iter().chain(todo_items.into_iter()).collect();
    sort_unified_items(&mut all);

    Ok(json!({
        "items": all,
        "pmCount": pm_count,
        "todoCount": todo_count,
    }))
}

fn sort_unified_items(items: &mut [Value]) {
    items.sort_by(|left, right| {
        pinned_rank(left)
            .cmp(&pinned_rank(right))
            .then_with(|| priority_rank(left).cmp(&priority_rank(right)))
            .then_with(|| {
                left.get("displayAt")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .cmp(right.get("displayAt").and_then(Value::as_str).unwrap_or(""))
            })
            .then_with(|| {
                right
                    .get("id")
                    .and_then(Value::as_i64)
                    .unwrap_or_default()
                    .cmp(&left.get("id").and_then(Value::as_i64).unwrap_or_default())
            })
    });
}

fn pinned_rank(item: &Value) -> i32 {
    if item.get("pinned").and_then(Value::as_bool).unwrap_or(false) { 0 } else { 1 }
}

fn priority_rank(item: &Value) -> i32 {
    match item.get("priority").and_then(Value::as_str).unwrap_or("P2") {
        "P0" => 0,
        "P1" => 1,
        "P2" => 2,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sort_pinned_first() {
        let mut items = vec![
            json!({"id": 1, "pinned": false, "priority": "P2", "displayAt": "2025-01-01"}),
            json!({"id": 2, "pinned": true, "priority": "P2", "displayAt": "2025-01-01"}),
        ];
        sort_unified_items(&mut items);
        assert_eq!(items[0]["id"].as_i64().unwrap(), 2);
    }

    #[test]
    fn sort_priority_within_same_pinned() {
        let mut items = vec![
            json!({"id": 1, "pinned": false, "priority": "P3", "displayAt": "2025-01-01"}),
            json!({"id": 2, "pinned": false, "priority": "P0", "displayAt": "2025-01-01"}),
        ];
        sort_unified_items(&mut items);
        assert_eq!(items[0]["id"].as_i64().unwrap(), 2);
    }
}

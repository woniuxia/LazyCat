use chrono::{Local, Utc};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

use super::helpers::db_conn;

const STATUSES: [&str; 4] = ["todo", "in_progress", "testing", "done"];
const ITEM_TYPES: [&str; 4] = ["task", "bug", "feature", "improvement"];
const PRIORITIES: [&str; 4] = ["P0", "P1", "P2", "P3"];

// ── Entry point ──────────────────────────────────────────

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "project_list" => project_list(),
        "project_create" => project_create(payload),
        "project_update" => project_update(payload),
        "project_archive" => project_archive(payload),
        "project_restore" => project_restore(payload),
        "project_delete" => project_delete(payload),
        "item_list" => item_list(payload),
        "item_create" => item_create(payload),
        "item_update" => item_update(payload),
        "item_change_status" => item_change_status(payload),
        "item_reorder" => item_reorder(payload),
        "item_toggle_pin" => item_toggle_pin(payload),
        "item_delete" => item_delete(payload),
        "item_move_project" => item_move_project(payload),
        "tag_list" => tag_list(payload),
        "weekly_work" => weekly_work(payload),
        _ => Err(format!("unsupported pm action: {action}")),
    }
}

// ── Helpers ──────────────────────────────────────────────

fn parse_i64(payload: &Value, key: &str) -> Option<i64> {
    payload.get(key).and_then(Value::as_i64)
}

fn parse_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
}

fn parse_string_array(payload: &Value, key: &str) -> Vec<String> {
    payload
        .get(key)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

// ── Project CRUD ─────────────────────────────────────────

fn project_list() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, description, color, status, sort_order, created_at, updated_at
             FROM pm_projects ORDER BY status ASC, sort_order ASC, id DESC",
        )
        .map_err(|e| format!("prepare project_list: {e}"))?;

    let rows: Vec<Value> = stmt
        .query_map([], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "name": r.get::<_, String>(1)?,
                "description": r.get::<_, String>(2)?,
                "color": r.get::<_, String>(3)?,
                "status": r.get::<_, String>(4)?,
                "sortOrder": r.get::<_, i64>(5)?,
                "createdAt": r.get::<_, String>(6)?,
                "updatedAt": r.get::<_, String>(7)?,
            }))
        })
        .map_err(|e| format!("query project_list: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(json!(rows))
}

fn project_create(payload: &Value) -> Result<Value, String> {
    let name = parse_string(payload, "name").ok_or("name is required")?;
    let desc = parse_string(payload, "description").unwrap_or_default();
    let color = parse_string(payload, "color").unwrap_or_else(|| "#409eff".to_string());
    let now = now_rfc3339();

    let conn = db_conn()?;
    conn.execute(
        "INSERT INTO pm_projects (name, description, color, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![name, desc, color, now, now],
    )
    .map_err(|e| format!("project_create: {e}"))?;

    let id = conn.last_insert_rowid();
    Ok(json!({ "id": id }))
}

fn project_update(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let name = parse_string(payload, "name").ok_or("name is required")?;
    let desc = parse_string(payload, "description").unwrap_or_default();
    let color = parse_string(payload, "color").unwrap_or_else(|| "#409eff".to_string());
    let sort_order = parse_i64(payload, "sortOrder").unwrap_or(0);
    let now = now_rfc3339();

    let conn = db_conn()?;
    conn.execute(
        "UPDATE pm_projects SET name = ?1, description = ?2, color = ?3, sort_order = ?4, updated_at = ?5 WHERE id = ?6",
        params![name, desc, color, sort_order, now, id],
    )
    .map_err(|e| format!("project_update: {e}"))?;

    Ok(json!({ "updated": true }))
}

fn project_archive(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let conn = db_conn()?;
    conn.execute(
        "UPDATE pm_projects SET status = 'archived', updated_at = ?1 WHERE id = ?2",
        params![now_rfc3339(), id],
    )
    .map_err(|e| format!("project_archive: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn project_restore(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let conn = db_conn()?;
    conn.execute(
        "UPDATE pm_projects SET status = 'active', updated_at = ?1 WHERE id = ?2",
        params![now_rfc3339(), id],
    )
    .map_err(|e| format!("project_restore: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn project_delete(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let conn = db_conn()?;

    // Check if any todo_items reference this project
    let todo_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM todo_items WHERE project_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if todo_count > 0 {
        return Err(format!(
            "该项目下有 {todo_count} 条待办事项，请先移除待办的项目归属后再删除"
        ));
    }

    // Cascade deletes pm_items + pm_item_tags via FK
    conn.execute("DELETE FROM pm_projects WHERE id = ?1", params![id])
        .map_err(|e| format!("project_delete: {e}"))?;
    Ok(json!({ "ok": true }))
}

// ── Item CRUD ────────────────────────────────────────────

fn item_list(payload: &Value) -> Result<Value, String> {
    let project_id = parse_i64(payload, "projectId");
    let conn = db_conn()?;

    let items: Vec<Value> = if let Some(pid) = project_id {
        let mut stmt = conn
            .prepare(
                "SELECT i.id, i.project_id, i.title, i.description, i.item_type, i.priority,
                        i.status, i.start_at, i.end_at, i.pinned, i.sort_order,
                        i.completed_at, i.created_at, i.updated_at,
                        p.name, p.color
                 FROM pm_items i
                 LEFT JOIN pm_projects p ON i.project_id = p.id
                 WHERE i.project_id = ?1
                 ORDER BY i.pinned DESC, i.sort_order ASC,
                          CASE i.priority WHEN 'P0' THEN 0 WHEN 'P1' THEN 1 WHEN 'P2' THEN 2 ELSE 3 END ASC,
                          i.id DESC",
            )
            .map_err(|e| format!("prepare item_list: {e}"))?;
        let result: Vec<Value> = stmt.query_map(params![pid], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "projectId": r.get::<_, i64>(1)?,
                "title": r.get::<_, String>(2)?,
                "description": r.get::<_, String>(3)?,
                "itemType": r.get::<_, String>(4)?,
                "priority": r.get::<_, String>(5)?,
                "status": r.get::<_, String>(6)?,
                "startAt": r.get::<_, Option<String>>(7)?,
                "endAt": r.get::<_, Option<String>>(8)?,
                "pinned": r.get::<_, bool>(9)?,
                "sortOrder": r.get::<_, i64>(10)?,
                "completedAt": r.get::<_, Option<String>>(11)?,
                "createdAt": r.get::<_, String>(12)?,
                "updatedAt": r.get::<_, String>(13)?,
                "projectName": r.get::<_, Option<String>>(14)?,
                "projectColor": r.get::<_, Option<String>>(15)?,
            }))
        })
        .map_err(|e| format!("query item_list: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
        result
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT i.id, i.project_id, i.title, i.description, i.item_type, i.priority,
                        i.status, i.start_at, i.end_at, i.pinned, i.sort_order,
                        i.completed_at, i.created_at, i.updated_at,
                        p.name, p.color
                 FROM pm_items i
                 LEFT JOIN pm_projects p ON i.project_id = p.id
                 WHERE p.status = 'active'
                 ORDER BY i.pinned DESC, i.sort_order ASC,
                          CASE i.priority WHEN 'P0' THEN 0 WHEN 'P1' THEN 1 WHEN 'P2' THEN 2 ELSE 3 END ASC,
                          i.id DESC",
            )
            .map_err(|e| format!("prepare item_list: {e}"))?;
        let result: Vec<Value> = stmt.query_map([], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "projectId": r.get::<_, i64>(1)?,
                "title": r.get::<_, String>(2)?,
                "description": r.get::<_, String>(3)?,
                "itemType": r.get::<_, String>(4)?,
                "priority": r.get::<_, String>(5)?,
                "status": r.get::<_, String>(6)?,
                "startAt": r.get::<_, Option<String>>(7)?,
                "endAt": r.get::<_, Option<String>>(8)?,
                "pinned": r.get::<_, bool>(9)?,
                "sortOrder": r.get::<_, i64>(10)?,
                "completedAt": r.get::<_, Option<String>>(11)?,
                "createdAt": r.get::<_, String>(12)?,
                "updatedAt": r.get::<_, String>(13)?,
                "projectName": r.get::<_, Option<String>>(14)?,
                "projectColor": r.get::<_, Option<String>>(15)?,
            }))
        })
        .map_err(|e| format!("query item_list: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
        result
    };

    // Attach tags
    let result: Vec<Value> = items
        .into_iter()
        .map(|mut item| {
            let item_id = item["id"].as_i64().unwrap_or(0);
            let tags = load_tags(&conn, item_id);
            item.as_object_mut().unwrap().insert("tags".to_string(), json!(tags));
            item
        })
        .collect();

    Ok(json!(result))
}

fn load_tags(conn: &Connection, item_id: i64) -> Vec<String> {
    conn.prepare("SELECT tag FROM pm_item_tags WHERE item_id = ?1 ORDER BY tag")
        .and_then(|mut stmt| {
            stmt.query_map(params![item_id], |r| r.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

fn save_tags(conn: &Connection, item_id: i64, tags: &[String]) -> Result<(), String> {
    conn.execute("DELETE FROM pm_item_tags WHERE item_id = ?1", params![item_id])
        .map_err(|e| format!("delete tags: {e}"))?;
    for tag in tags {
        let t = tag.trim();
        if t.is_empty() {
            continue;
        }
        conn.execute(
            "INSERT OR IGNORE INTO pm_item_tags (item_id, tag) VALUES (?1, ?2)",
            params![item_id, t],
        )
        .map_err(|e| format!("insert tag: {e}"))?;
    }
    Ok(())
}

fn item_create(payload: &Value) -> Result<Value, String> {
    let project_id = parse_i64(payload, "projectId").ok_or("projectId is required")?;
    let title = parse_string(payload, "title").ok_or("title is required")?;
    let desc = parse_string(payload, "description").unwrap_or_default();
    let item_type = parse_string(payload, "itemType")
        .filter(|v| ITEM_TYPES.contains(&v.as_str()))
        .unwrap_or_else(|| "task".to_string());
    let priority = parse_string(payload, "priority")
        .filter(|v| PRIORITIES.contains(&v.as_str()))
        .unwrap_or_else(|| "P2".to_string());
    let status = parse_string(payload, "status")
        .filter(|v| STATUSES.contains(&v.as_str()))
        .unwrap_or_else(|| "todo".to_string());
    let start_at = parse_string(payload, "startAt");
    let end_at = parse_string(payload, "endAt");
    let tags = parse_string_array(payload, "tags");
    let now = now_rfc3339();

    let completed_at = if status == "done" { Some(now.clone()) } else { None };

    let conn = db_conn()?;
    conn.execute(
        "INSERT INTO pm_items (project_id, title, description, item_type, priority, status,
                               start_at, end_at, completed_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![project_id, title, desc, item_type, priority, status, start_at, end_at, completed_at, now, now],
    )
    .map_err(|e| format!("item_create: {e}"))?;

    let id = conn.last_insert_rowid();
    if !tags.is_empty() {
        save_tags(&conn, id, &tags)?;
    }

    Ok(json!({ "id": id }))
}

fn item_update(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let conn = db_conn()?;
    let now = now_rfc3339();

    // Read current row
    let (cur_title, cur_desc, cur_type, cur_prio, cur_status, cur_start, cur_end): (String, String, String, String, String, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT title, description, item_type, priority, status, start_at, end_at FROM pm_items WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
        .map_err(|e| format!("item not found: {e}"))?;

    let title = parse_string(payload, "title").unwrap_or(cur_title);
    let desc = if payload.get("description").is_some() {
        parse_string(payload, "description").unwrap_or_default()
    } else {
        cur_desc
    };
    let item_type = parse_string(payload, "itemType")
        .filter(|v| ITEM_TYPES.contains(&v.as_str()))
        .unwrap_or(cur_type);
    let priority = parse_string(payload, "priority")
        .filter(|v| PRIORITIES.contains(&v.as_str()))
        .unwrap_or(cur_prio);
    let new_status = parse_string(payload, "status")
        .filter(|v| STATUSES.contains(&v.as_str()))
        .unwrap_or(cur_status.clone());
    let start_at = if payload.get("startAt").is_some() {
        parse_string(payload, "startAt")
    } else {
        cur_start
    };
    let end_at = if payload.get("endAt").is_some() {
        parse_string(payload, "endAt")
    } else {
        cur_end
    };

    let completed_at: Option<String> = if new_status == "done" && cur_status != "done" {
        Some(now.clone())
    } else if new_status == "done" {
        // Keep existing completed_at
        conn.query_row("SELECT completed_at FROM pm_items WHERE id = ?1", params![id], |r| r.get(0))
            .unwrap_or(Some(now.clone()))
    } else {
        None
    };

    conn.execute(
        "UPDATE pm_items SET title=?1, description=?2, item_type=?3, priority=?4, status=?5,
         start_at=?6, end_at=?7, completed_at=?8, updated_at=?9 WHERE id=?10",
        params![title, desc, item_type, priority, new_status, start_at, end_at, completed_at, now, id],
    )
    .map_err(|e| format!("item_update: {e}"))?;

    if payload.get("tags").is_some() {
        let tags = parse_string_array(payload, "tags");
        save_tags(&conn, id, &tags)?;
    }

    Ok(json!({ "updated": true }))
}

fn item_change_status(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let new_status = parse_string(payload, "status").ok_or("status is required")?;
    if !STATUSES.contains(&new_status.as_str()) {
        return Err(format!("invalid status: {new_status}"));
    }
    let now = now_rfc3339();
    let conn = db_conn()?;

    if new_status == "done" {
        conn.execute(
            "UPDATE pm_items SET status = ?1, completed_at = ?2, updated_at = ?2 WHERE id = ?3",
            params![new_status, now, id],
        )
        .map_err(|e| format!("item_change_status: {e}"))?;
    } else {
        conn.execute(
            "UPDATE pm_items SET status = ?1, completed_at = NULL, updated_at = ?2 WHERE id = ?3",
            params![new_status, now, id],
        )
        .map_err(|e| format!("item_change_status: {e}"))?;
    }

    Ok(json!({ "ok": true }))
}

fn item_reorder(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let now = now_rfc3339();

    // Supports two modes:
    // 1) Array of { id, sortOrder } for within-column reorder
    // 2) { id, status, sortOrder } for cross-column move
    if let Some(items) = payload.get("items").and_then(Value::as_array) {
        for item in items {
            let id = item.get("id").and_then(Value::as_i64).unwrap_or(0);
            let sort = item.get("sortOrder").and_then(Value::as_i64).unwrap_or(0);
            let status = item.get("status").and_then(Value::as_str);

            if let Some(st) = status {
                if st == "done" {
                    conn.execute(
                        "UPDATE pm_items SET sort_order = ?1, status = ?2, completed_at = COALESCE(completed_at, ?4), updated_at = ?4 WHERE id = ?3",
                        params![sort, st, id, now],
                    )
                    .map_err(|e| format!("item_reorder: {e}"))?;
                } else {
                    conn.execute(
                        "UPDATE pm_items SET sort_order = ?1, status = ?2, completed_at = NULL, updated_at = ?4 WHERE id = ?3",
                        params![sort, st, id, now],
                    )
                    .map_err(|e| format!("item_reorder: {e}"))?;
                }
            } else {
                conn.execute(
                    "UPDATE pm_items SET sort_order = ?1, updated_at = ?3 WHERE id = ?2",
                    params![sort, id, now],
                )
                .map_err(|e| format!("item_reorder: {e}"))?;
            }
        }
        return Ok(json!({ "ok": true }));
    }

    // Single item cross-column move
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let new_status = parse_string(payload, "status");
    let sort_order = parse_i64(payload, "sortOrder").unwrap_or(0);

    if let Some(st) = new_status {
        if st == "done" {
            conn.execute(
                "UPDATE pm_items SET sort_order = ?1, status = ?2, completed_at = COALESCE(completed_at, ?4), updated_at = ?4 WHERE id = ?3",
                params![sort_order, st, id, now],
            )
            .map_err(|e| format!("item_reorder: {e}"))?;
        } else {
            conn.execute(
                "UPDATE pm_items SET sort_order = ?1, status = ?2, completed_at = NULL, updated_at = ?4 WHERE id = ?3",
                params![sort_order, st, id, now],
            )
            .map_err(|e| format!("item_reorder: {e}"))?;
        }
    } else {
        conn.execute(
            "UPDATE pm_items SET sort_order = ?1, updated_at = ?3 WHERE id = ?2",
            params![sort_order, id, now],
        )
        .map_err(|e| format!("item_reorder: {e}"))?;
    }

    Ok(json!({ "ok": true }))
}

fn item_toggle_pin(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let conn = db_conn()?;
    conn.execute(
        "UPDATE pm_items SET pinned = 1 - pinned, updated_at = ?1 WHERE id = ?2",
        params![now_rfc3339(), id],
    )
    .map_err(|e| format!("item_toggle_pin: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn item_delete(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let conn = db_conn()?;
    conn.execute("DELETE FROM pm_items WHERE id = ?1", params![id])
        .map_err(|e| format!("item_delete: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn item_move_project(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let project_id = parse_i64(payload, "projectId").ok_or("projectId is required")?;
    let conn = db_conn()?;
    conn.execute(
        "UPDATE pm_items SET project_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![project_id, now_rfc3339(), id],
    )
    .map_err(|e| format!("item_move_project: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn tag_list(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let project_id = parse_i64(payload, "projectId");

    let tags: Vec<Value> = if let Some(pid) = project_id {
        let mut stmt = conn
            .prepare(
                "SELECT t.tag, COUNT(*) as cnt
                 FROM pm_item_tags t JOIN pm_items i ON t.item_id = i.id
                 WHERE i.project_id = ?1
                 GROUP BY t.tag ORDER BY cnt DESC, t.tag",
            )
            .map_err(|e| format!("tag_list: {e}"))?;
        let result: Vec<Value> = stmt
            .query_map(params![pid], |r| {
                Ok(json!({ "tag": r.get::<_, String>(0)?, "count": r.get::<_, i64>(1)? }))
            })
            .map_err(|e| format!("tag_list query: {e}"))?
            .filter_map(|r| r.ok())
            .collect();
        result
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT tag, COUNT(*) as cnt FROM pm_item_tags GROUP BY tag ORDER BY cnt DESC, tag",
            )
            .map_err(|e| format!("tag_list: {e}"))?;
        let result: Vec<Value> = stmt
            .query_map([], |r| {
                Ok(json!({ "tag": r.get::<_, String>(0)?, "count": r.get::<_, i64>(1)? }))
            })
            .map_err(|e| format!("tag_list query: {e}"))?
            .filter_map(|r| r.ok())
            .collect();
        result
    };

    Ok(json!(tags))
}

// ── Weekly work ──────────────────────────────────────────

fn weekly_work(_payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;

    // Calculate 7-day window in local timezone, convert to UTC for comparison
    let now_local = Local::now();
    let start_local = (now_local - chrono::Duration::days(6))
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let start_utc = start_local
        .and_local_timezone(Local)
        .single()
        .map(|dt| dt.with_timezone(&Utc))
        .ok_or("timezone conversion failed")?;
    let start_str = start_utc.to_rfc3339();

    // PM items completed in window
    let mut pm_stmt = conn
        .prepare(
            "SELECT i.id, i.project_id, i.title, i.item_type, i.priority, i.status,
                    i.completed_at, i.created_at,
                    p.name as project_name, p.color as project_color, p.status as project_status
             FROM pm_items i
             JOIN pm_projects p ON i.project_id = p.id
             WHERE i.status = 'done' AND i.completed_at >= ?1
             ORDER BY i.completed_at DESC",
        )
        .map_err(|e| format!("weekly_work pm: {e}"))?;

    let pm_items: Vec<Value> = pm_stmt
        .query_map(params![start_str], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "projectId": r.get::<_, i64>(1)?,
                "title": r.get::<_, String>(2)?,
                "itemType": r.get::<_, String>(3)?,
                "priority": r.get::<_, String>(4)?,
                "status": r.get::<_, String>(5)?,
                "completedAt": r.get::<_, Option<String>>(6)?,
                "createdAt": r.get::<_, String>(7)?,
                "projectName": r.get::<_, String>(8)?,
                "projectColor": r.get::<_, String>(9)?,
                "projectStatus": r.get::<_, String>(10)?,
                "source": "pm",
            }))
        })
        .map_err(|e| format!("weekly_work pm query: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    // Todo items completed in window (project_id column may not exist yet)
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
                 WHERE t.status = 'completed' AND t.completed_at >= ?1
                 ORDER BY t.completed_at DESC",
            )
            .map_err(|e| format!("weekly_work todo: {e}"))?;

        let result: Vec<Value> = todo_stmt
            .query_map(params![start_str], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "title": r.get::<_, String>(1)?,
                    "priority": r.get::<_, String>(2)?,
                    "status": r.get::<_, String>(3)?,
                    "completedAt": r.get::<_, Option<String>>(4)?,
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
                 WHERE t.status = 'completed' AND t.completed_at >= ?1
                 ORDER BY t.completed_at DESC",
            )
            .map_err(|e| format!("weekly_work todo: {e}"))?;

        let result: Vec<Value> = todo_stmt
            .query_map(params![start_str], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "title": r.get::<_, String>(1)?,
                    "priority": r.get::<_, String>(2)?,
                    "status": r.get::<_, String>(3)?,
                    "completedAt": r.get::<_, Option<String>>(4)?,
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
        "windowStart": start_str,
        "windowEnd": now_local.to_rfc3339(),
    }))
}

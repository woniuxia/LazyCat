use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use url::Url;

use calamine::{open_workbook_auto, Data, Reader};

use super::helpers::db_conn;


pub(crate) const STATUSES: [&str; 4] = ["todo", "in_progress", "testing", "done"];
pub(crate) const ITEM_TYPES: [&str; 4] = ["task", "bug", "feature", "improvement"];
pub(crate) const PRIORITIES: [&str; 4] = ["P0", "P1", "P2", "P3"];

// ── Entry point ──────────────────────────────────────────

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "project_list" => project_list(),
        "project_create" => project_create(payload),
        "project_update" => project_update(payload),
        "project_archive" => project_archive(payload),
        "project_restore" => project_restore(payload),
        "project_delete" => project_delete(payload),
        "item_counts" => item_counts(),
        "item_list" => item_list(payload),
        "item_create" => item_create(payload),
        "item_update" => item_update(payload),
        "item_change_status" => item_change_status(payload),
        "item_reorder" => item_reorder(payload),
        "item_toggle_pin" => item_toggle_pin(payload),
        "item_batch_update" => item_batch_update(payload),
        "item_delete" => item_delete(payload),
        "item_move_project" => item_move_project(payload),
        "tag_list" => tag_list(payload),
        "weekly_work" => crate::tools::pm_weekly::weekly_work(payload),
        "siyuan_test" => crate::tools::pm_siyuan::siyuan_test(payload),
        "siyuan_directory" => crate::tools::pm_siyuan::siyuan_directory(payload),
        "siyuan_search_pages" => crate::tools::pm_siyuan::siyuan_search_pages(payload),
        "siyuan_create_page" => crate::tools::pm_siyuan::siyuan_create_page(payload),
        "siyuan_open_page" => crate::tools::pm_siyuan::siyuan_open_page(payload),
        "open_link" => crate::tools::pm_siyuan::open_link(payload),
        "siyuan_check_running" => crate::tools::pm_siyuan::siyuan_check_running(payload),
        "siyuan_launch" => crate::tools::pm_siyuan::siyuan_launch(payload),
        "item_todo_list" => crate::tools::pm_todo_link::item_todo_list(payload),
        "item_todo_link" => crate::tools::pm_todo_link::item_todo_link(payload),
        "item_todo_unlink" => crate::tools::pm_todo_link::item_todo_unlink(payload),
        "item_todo_create" => crate::tools::pm_todo_link::item_todo_create(payload),
        "item_todo_candidates" => crate::tools::pm_todo_link::item_todo_candidates(payload),
        "item_todo_candidates_by_project" => crate::tools::pm_todo_link::item_todo_candidates_by_project(payload),
        "item_today_list" => crate::tools::pm_today::item_today_list(payload),
        "item_today_counts" => crate::tools::pm_today::item_today_counts(payload),
        "item_calendar_range" => crate::tools::pm_calendar::item_calendar_range(payload),
        "item_matrix_bucket" => crate::tools::pm_matrix::item_matrix_bucket(payload),
        "item_import_preview" => item_import_preview(payload),
        "item_import" => item_import(payload),
        _ => Err(format!("unsupported pm action: {action}")),
    }
}

// ── Helpers ──────────────────────────────────────────────

pub(crate) fn parse_i64(payload: &Value, key: &str) -> Option<i64> {
    payload.get(key).and_then(Value::as_i64)
}

pub(crate) fn parse_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToString::to_string)
}

pub(crate) fn parse_string_array(payload: &Value, key: &str) -> Vec<String> {
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

pub(crate) fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

pub(crate) fn normalize_item_link_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("请输入链接地址".into());
    }

    let lower = trimmed.to_ascii_lowercase();
    let normalized = if lower.starts_with("http://") || lower.starts_with("https://") {
        trimmed.to_string()
    } else if trimmed.contains("://") {
        return Err("仅支持 http/https 链接".into());
    } else {
        format!("http://{trimmed}")
    };

    let parsed = Url::parse(&normalized).map_err(|_| "链接格式不正确".to_string())?;
    match parsed.scheme() {
        "http" | "https" => Ok(normalized),
        _ => Err("仅支持 http/https 链接".into()),
    }
}

// ── Project CRUD ─────────────────────────────────────────

fn project_list() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, description, color, status,
                    siyuan_notebook_id, siyuan_notebook_name,
                    siyuan_parent_doc_id, siyuan_parent_doc_title,
                    siyuan_parent_hpath, siyuan_parent_path,
                    sort_order, created_at, updated_at
             FROM pm_projects ORDER BY status ASC, sort_order ASC, id DESC",
        )
        .map_err(|e| format!("prepare project_list: {e}"))?;

    let rows: Vec<Value> = stmt
        .query_map([], |r| {
            let location = crate::tools::pm_siyuan::build_siyuan_location_from_parts(
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, Option<String>>(10)?,
            );
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "name": r.get::<_, String>(1)?,
                "description": r.get::<_, String>(2)?,
                "color": r.get::<_, String>(3)?,
                "status": r.get::<_, String>(4)?,
                "siyuanLocationOverride": location,
                "sortOrder": r.get::<_, i64>(11)?,
                "createdAt": r.get::<_, String>(12)?,
                "updatedAt": r.get::<_, String>(13)?,
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
    let location = match payload.get("siyuanLocationOverride") {
        Some(value) => crate::tools::pm_siyuan::parse_siyuan_location_value(value)?,
        None => None,
    };
    let now = now_rfc3339();

    let conn = db_conn()?;
    conn.execute(
        "INSERT INTO pm_projects (
            name, description, color,
            siyuan_notebook_id, siyuan_notebook_name,
            siyuan_parent_doc_id, siyuan_parent_doc_title, siyuan_parent_hpath, siyuan_parent_path,
            created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            name,
            desc,
            color,
            location.as_ref().map(|item| item.notebook_id.as_str()),
            location.as_ref().map(|item| item.notebook_name.as_str()),
            location.as_ref().and_then(|item| item.parent_doc_id.as_deref()),
            location.as_ref().and_then(|item| item.parent_doc_title.as_deref()),
            location.as_ref().and_then(|item| item.parent_hpath.as_deref()),
            location.as_ref().and_then(|item| item.parent_path.as_deref()),
            now,
            now
        ],
    )
    .map_err(|e| format!("project_create: {e}"))?;

    let id = conn.last_insert_rowid();
    Ok(json!({ "id": id }))
}

fn project_update(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let mut conn = db_conn()?;
    let tx = conn.transaction().map_err(|e| format!("project_update begin: {e}"))?;
    let current_location = tx
        .query_row(
            "SELECT siyuan_notebook_id, siyuan_notebook_name,
                    siyuan_parent_doc_id, siyuan_parent_doc_title,
                    siyuan_parent_hpath, siyuan_parent_path
             FROM pm_projects WHERE id = ?1",
            params![id],
            |row| {
                Ok(crate::tools::pm_siyuan::build_siyuan_location_from_parts(
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .map_err(|e| format!("project not found: {e}"))?;
    let name = parse_string(payload, "name").ok_or("name is required")?;
    let desc = parse_string(payload, "description").unwrap_or_default();
    let color = parse_string(payload, "color").unwrap_or_else(|| "#409eff".to_string());
    let sort_order = parse_i64(payload, "sortOrder").unwrap_or(0);
    let location = match payload.get("siyuanLocationOverride") {
        Some(value) => crate::tools::pm_siyuan::parse_siyuan_location_value(value)?,
        None => current_location,
    };
    let now = now_rfc3339();
    tx.execute(
        "UPDATE pm_projects
         SET name = ?1, description = ?2, color = ?3,
             siyuan_notebook_id = ?4, siyuan_notebook_name = ?5,
             siyuan_parent_doc_id = ?6, siyuan_parent_doc_title = ?7,
             siyuan_parent_hpath = ?8, siyuan_parent_path = ?9,
             sort_order = ?10, updated_at = ?11
         WHERE id = ?12",
        params![
            name,
            desc,
            color,
            location.as_ref().map(|item| item.notebook_id.as_str()),
            location.as_ref().map(|item| item.notebook_name.as_str()),
            location.as_ref().and_then(|item| item.parent_doc_id.as_deref()),
            location.as_ref().and_then(|item| item.parent_doc_title.as_deref()),
            location.as_ref().and_then(|item| item.parent_hpath.as_deref()),
            location.as_ref().and_then(|item| item.parent_path.as_deref()),
            sort_order,
            now,
            id
        ],
    )
    .map_err(|e| format!("project_update: {e}"))?;

    tx.commit().map_err(|e| format!("project_update commit: {e}"))?;
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
    let mut conn = db_conn()?;

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

    let tx = conn
        .transaction()
        .map_err(|e| format!("project_delete begin tx: {e}"))?;

    // 1. 收集子 pm_items.id，逐一清附件
    let item_ids: Vec<i64> = {
        let mut stmt = tx
            .prepare("SELECT id FROM pm_items WHERE project_id = ?1")
            .map_err(|e| format!("project_delete prepare children: {e}"))?;
        let rows = stmt
            .query_map(params![id], |r| r.get::<_, i64>(0))
            .map_err(|e| format!("project_delete query children: {e}"))?;
        rows.filter_map(|r| r.ok()).collect()
    };
    for item_id in &item_ids {
        super::attachments::delete_by_owner_internal(&tx, "pm_item", &item_id.to_string())?;
    }

    // 2. 显式删子 items（pm_items 无 FK CASCADE，否则会产生孤儿记录）
    tx.execute("DELETE FROM pm_items WHERE project_id = ?1", params![id])
        .map_err(|e| format!("project_delete pm_items: {e}"))?;

    // 3. 删项目自身附件
    super::attachments::delete_by_owner_internal(&tx, "pm_project", &id.to_string())?;

    // 4. 删项目行
    tx.execute("DELETE FROM pm_projects WHERE id = ?1", params![id])
        .map_err(|e| format!("project_delete: {e}"))?;

    tx.commit()
        .map_err(|e| format!("project_delete commit: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn ensure_project_accepts_items(conn: &Connection, project_id: i64) -> Result<(), String> {
    let status = conn
        .query_row(
            "SELECT status FROM pm_projects WHERE id = ?1",
            params![project_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| format!("load project status: {e}"))?;

    match status.as_deref() {
        Some("active") => Ok(()),
        Some("archived") => Err("归档项目不能接收工作项，请先恢复项目".into()),
        Some(other) => Err(format!("invalid project status: {other}")),
        None => Err("目标项目不存在".into()),
    }
}

// ── Item counts (lightweight, for sidebar) ───────────────

fn item_counts() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT i.project_id, i.status, COUNT(*) FROM pm_items i
             JOIN pm_projects p ON i.project_id = p.id
             GROUP BY i.project_id, i.status",
        )
        .map_err(|e| format!("prepare item_counts: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| format!("query item_counts: {e}"))?;

    let mut map: HashMap<i64, (i64, i64)> = HashMap::new();
    for row in rows {
        let (pid, status, count) = row.map_err(|e| format!("read item_counts: {e}"))?;
        let entry = map.entry(pid).or_insert((0, 0));
        entry.0 += count;
        if status == "done" {
            entry.1 += count;
        }
    }

    let result: Vec<Value> = map
        .into_iter()
        .map(|(pid, (total, done))| json!({ "projectId": pid, "total": total, "done": done }))
        .collect();
    Ok(json!(result))
}

// ── Item CRUD ────────────────────────────────────────────

fn build_item_list_sql(project_id: Option<i64>) -> &'static str {
    if project_id.is_some() {
        "SELECT i.id, i.project_id, i.title, i.description, i.item_type, i.priority,
                i.status, i.start_at, i.end_at, i.pinned, i.sort_order,
                i.siyuan_doc_id, i.siyuan_doc_title, i.siyuan_doc_hpath,
                i.siyuan_doc_path, i.siyuan_notebook_id, i.siyuan_notebook_name,
                i.completed_at, i.created_at, i.updated_at, i.link_url,
                p.name, p.color, i.started_at, i.testing_at, i.ref_code
         FROM pm_items i
         LEFT JOIN pm_projects p ON i.project_id = p.id
         WHERE i.project_id = ?1
         ORDER BY i.pinned DESC, i.sort_order ASC,
                  CASE i.priority WHEN 'P0' THEN 0 WHEN 'P1' THEN 1 WHEN 'P2' THEN 2 ELSE 3 END ASC,
                  i.id DESC"
    } else {
        "SELECT i.id, i.project_id, i.title, i.description, i.item_type, i.priority,
                i.status, i.start_at, i.end_at, i.pinned, i.sort_order,
                i.siyuan_doc_id, i.siyuan_doc_title, i.siyuan_doc_hpath,
                i.siyuan_doc_path, i.siyuan_notebook_id, i.siyuan_notebook_name,
                i.completed_at, i.created_at, i.updated_at, i.link_url,
                p.name, p.color, i.started_at, i.testing_at, i.ref_code
         FROM pm_items i
         LEFT JOIN pm_projects p ON i.project_id = p.id
         ORDER BY i.pinned DESC, i.sort_order ASC,
                  CASE i.priority WHEN 'P0' THEN 0 WHEN 'P1' THEN 1 WHEN 'P2' THEN 2 ELSE 3 END ASC,
                  i.id DESC"
    }
}

fn item_list(payload: &Value) -> Result<Value, String> {
    let project_id = parse_i64(payload, "projectId");
    let conn = db_conn()?;

    let (sql, qp): (&'static str, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(pid) = project_id {
        (build_item_list_sql(Some(pid)), vec![Box::new(pid)])
    } else {
        (build_item_list_sql(None), vec![])
    };

    let mut stmt = conn.prepare(&sql).map_err(|e| format!("prepare item_list: {e}"))?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = qp.iter().map(|p| p.as_ref()).collect();
    let items: Vec<Value> = stmt
        .query_map(param_refs.as_slice(), |r| {
            let primary_page = crate::tools::pm_siyuan::build_siyuan_page_ref_from_parts(
                r.get::<_, Option<String>>(11)?,
                r.get::<_, Option<String>>(12)?,
                r.get::<_, Option<String>>(13)?,
                r.get::<_, Option<String>>(14)?,
                r.get::<_, Option<String>>(15)?,
                r.get::<_, Option<String>>(16)?,
            );
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
                "siyuanPrimaryPage": primary_page,
                "completedAt": r.get::<_, Option<String>>(17)?,
                "createdAt": r.get::<_, String>(18)?,
                "updatedAt": r.get::<_, String>(19)?,
                "linkUrl": r.get::<_, Option<String>>(20)?,
                "projectName": r.get::<_, Option<String>>(21)?,
                "projectColor": r.get::<_, Option<String>>(22)?,
                "startedAt": r.get::<_, Option<String>>(23)?,
                "testingAt": r.get::<_, Option<String>>(24)?,
                "refCode": r.get::<_, Option<String>>(25)?,
            }))
        })
        .map_err(|e| format!("query item_list: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let item_ids: Vec<i64> = items.iter().filter_map(|v| v["id"].as_i64()).collect();

    let tag_map = batch_load_tags(&conn, &item_ids);
    let links_map = batch_load_siyuan_links(&conn, &item_ids);
    let todo_count_map = batch_load_todo_counts(&conn, &item_ids);

    let result: Vec<Value> = items
        .into_iter()
        .map(|mut item| {
            let item_id = item["id"].as_i64().unwrap_or(0);
            let tags = tag_map.get(&item_id).cloned().unwrap_or_default();
            let extra_pages = links_map.get(&item_id).cloned().unwrap_or_default();
            let todo_count = todo_count_map.get(&item_id).copied().unwrap_or(0);
            item.as_object_mut().unwrap().insert("tags".to_string(), json!(tags));
            item.as_object_mut()
                .unwrap()
                .insert("siyuanExtraPages".to_string(), json!(extra_pages));
            item.as_object_mut()
                .unwrap()
                .insert("todoCount".to_string(), json!(todo_count));
            item
        })
        .collect();

    Ok(json!(result))
}

pub(crate) fn batch_load_tags(conn: &Connection, item_ids: &[i64]) -> HashMap<i64, Vec<String>> {
    if item_ids.is_empty() {
        return HashMap::new();
    }
    let placeholders: Vec<String> = item_ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
    let sql = format!(
        "SELECT item_id, tag FROM pm_item_tags WHERE item_id IN ({}) ORDER BY tag",
        placeholders.join(",")
    );
    let params: Vec<&dyn rusqlite::types::ToSql> = item_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
    let mut map: HashMap<i64, Vec<String>> = HashMap::new();
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(rows) = stmt.query_map(params.as_slice(), |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        }) {
            for row in rows.flatten() {
                map.entry(row.0).or_default().push(row.1);
            }
        }
    }
    map
}

fn batch_load_todo_counts(conn: &Connection, item_ids: &[i64]) -> HashMap<i64, i64> {
    if item_ids.is_empty() {
        return HashMap::new();
    }
    let placeholders: Vec<String> = item_ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
    let sql = format!(
        "SELECT pm_item_id, COUNT(*) FROM pm_item_todo_links
         WHERE pm_item_id IN ({}) GROUP BY pm_item_id",
        placeholders.join(",")
    );
    let params: Vec<&dyn rusqlite::types::ToSql> = item_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
    let mut map: HashMap<i64, i64> = HashMap::new();
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(rows) = stmt.query_map(params.as_slice(), |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        }) {
            for row in rows.flatten() {
                map.insert(row.0, row.1);
            }
        }
    }
    map
}

pub(crate) fn batch_load_siyuan_links(conn: &Connection, item_ids: &[i64]) -> HashMap<i64, Vec<crate::tools::pm_siyuan::SiyuanPageRef>> {
    if item_ids.is_empty() {
        return HashMap::new();
    }
    let placeholders: Vec<String> = item_ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
    let sql = format!(
        "SELECT item_id, doc_id, doc_title, doc_hpath,
                doc_path, notebook_id, notebook_name
         FROM pm_item_siyuan_links WHERE item_id IN ({})
         ORDER BY item_id, id",
        placeholders.join(",")
    );
    let params: Vec<&dyn rusqlite::types::ToSql> = item_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
    let mut map: HashMap<i64, Vec<crate::tools::pm_siyuan::SiyuanPageRef>> = HashMap::new();
    if let Ok(mut stmt) = conn.prepare(&sql) {
        if let Ok(rows) = stmt.query_map(params.as_slice(), |r| {
            Ok((
                r.get::<_, i64>(0)?,
                crate::tools::pm_siyuan::build_siyuan_page_ref_from_parts(
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                ),
            ))
        }) {
            for row in rows.flatten() {
                if let Some(page_ref) = row.1 {
                    map.entry(row.0).or_default().push(page_ref);
                }
            }
        }
    }
    map
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

fn parse_item_link_url_value(value: &Value) -> Result<Option<String>, String> {
    if value.is_null() {
        return Ok(None);
    }

    let raw = value.as_str().ok_or("链接格式不正确")?.trim();
    if raw.is_empty() {
        return Ok(None);
    }

    Ok(Some(normalize_item_link_url(raw)?))
}

fn item_create(payload: &Value) -> Result<Value, String> {
    let project_id = parse_i64(payload, "projectId").ok_or("projectId is required")?;
    let mut conn = db_conn()?;
    ensure_project_accepts_items(&conn, project_id)?;
    let title = parse_string(payload, "title").ok_or("title is required")?;
    let desc = parse_string(payload, "description").unwrap_or_default();
    let link_url = match payload.get("linkUrl") {
        Some(value) => parse_item_link_url_value(value)?,
        None => None,
    };
    let ref_code = parse_string(payload, "refCode");
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
    let primary_page = match payload.get("siyuanPrimaryPage") {
        Some(value) => crate::tools::pm_siyuan::parse_siyuan_page_ref_value(value)?,
        None => None,
    };
    let extra_pages = crate::tools::pm_siyuan::parse_siyuan_page_ref_array(payload.get("siyuanExtraPages"))?
        .unwrap_or_default();
    let now = now_rfc3339();

    let started_at: Option<String> = match status.as_str() {
        "in_progress" | "testing" | "done" => Some(now.clone()),
        _ => None,
    };
    let testing_at: Option<String> = match status.as_str() {
        "testing" => Some(now.clone()),
        _ => None,
    };
    let completed_at: Option<String> = if status == "done" { Some(now.clone()) } else { None };

    let tx = conn.transaction().map_err(|e| format!("item_create begin: {e}"))?;
    tx.execute(
        "INSERT INTO pm_items (
            project_id, title, description, link_url, ref_code, item_type, priority, status,
            start_at, end_at,
            siyuan_doc_id, siyuan_doc_title, siyuan_doc_hpath, siyuan_doc_path,
            siyuan_notebook_id, siyuan_notebook_name,
            started_at, testing_at, completed_at, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
        params![
            project_id,
            title,
            desc,
            link_url,
            ref_code,
            item_type,
            priority,
            status,
            start_at,
            end_at,
            primary_page.as_ref().map(|item| item.doc_id.as_str()),
            primary_page.as_ref().map(|item| item.doc_title.as_str()),
            primary_page.as_ref().map(|item| item.doc_hpath.as_str()),
            primary_page.as_ref().and_then(|item| item.doc_path.as_deref()),
            primary_page.as_ref().map(|item| item.notebook_id.as_str()),
            primary_page.as_ref().map(|item| item.notebook_name.as_str()),
            started_at,
            testing_at,
            completed_at,
            now,
            now
        ],
    )
    .map_err(|e| format!("item_create: {e}"))?;

    let id = tx.last_insert_rowid();
    if !tags.is_empty() {
        save_tags(&tx, id, &tags)?;
    }
    crate::tools::pm_siyuan::save_item_siyuan_links(&tx, id, primary_page.as_ref(), &extra_pages, &now)?;

    tx.commit().map_err(|e| format!("item_create commit: {e}"))?;
    Ok(json!({ "id": id }))
}


fn resolve_status_flow_timestamps(
    cur_status: &str,
    new_status: &str,
    cur_started_at: Option<String>,
    cur_testing_at: Option<String>,
    cur_completed_at: Option<String>,
    started_at_override: Option<Option<String>>,
    testing_at_override: Option<Option<String>>,
    completed_at_override: Option<Option<String>>,
    now: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let status_changed = new_status != cur_status;

    let auto_started_at = if status_changed {
        match new_status {
            "in_progress" | "testing" | "done" => {
                Some(cur_started_at.clone().unwrap_or_else(|| now.to_string()))
            }
            _ => None,
        }
    } else {
        cur_started_at
    };

    let auto_testing_at = if status_changed {
        match new_status {
            "testing" => Some(cur_testing_at.clone().unwrap_or_else(|| now.to_string())),
            "todo" => None,
            _ => cur_testing_at,
        }
    } else {
        cur_testing_at
    };

    let auto_completed_at = if status_changed {
        if new_status == "done" {
            Some(now.to_string())
        } else {
            None
        }
    } else {
        cur_completed_at
    };

    (
        started_at_override.unwrap_or(auto_started_at),
        testing_at_override.unwrap_or(auto_testing_at),
        completed_at_override.unwrap_or(auto_completed_at),
    )
}

fn item_update(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let conn = db_conn()?;
    let now = now_rfc3339();

    let (
        cur_title,
        cur_desc,
        cur_link_url,
        cur_ref_code,
        cur_type,
        cur_prio,
        cur_status,
        cur_start,
        cur_end,
        cur_primary_page,
        cur_started_at,
        cur_testing_at,
        cur_completed_at,
    ): (
        String,
        String,
        Option<String>,
        Option<String>,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<crate::tools::pm_siyuan::SiyuanPageRef>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT title, description, link_url, ref_code, item_type, priority, status, start_at, end_at,
                    siyuan_doc_id, siyuan_doc_title, siyuan_doc_hpath,
                    siyuan_doc_path, siyuan_notebook_id, siyuan_notebook_name,
                    started_at, testing_at, completed_at
             FROM pm_items WHERE id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    crate::tools::pm_siyuan::build_siyuan_page_ref_from_parts(
                        r.get::<_, Option<String>>(9)?,
                        r.get::<_, Option<String>>(10)?,
                        r.get::<_, Option<String>>(11)?,
                        r.get::<_, Option<String>>(12)?,
                        r.get::<_, Option<String>>(13)?,
                        r.get::<_, Option<String>>(14)?,
                    ),
                    r.get(15)?,
                    r.get(16)?,
                    r.get(17)?,
                ))
            },
        )
        .map_err(|e| format!("item not found: {e}"))?;
    let cur_extra_pages = crate::tools::pm_siyuan::load_item_siyuan_links(&conn, id);

    let title = parse_string(payload, "title").unwrap_or(cur_title);
    let desc = if payload.get("description").is_some() {
        parse_string(payload, "description").unwrap_or_default()
    } else {
        cur_desc
    };
    let link_url = if let Some(value) = payload.get("linkUrl") {
        parse_item_link_url_value(value)?
    } else {
        cur_link_url
    };
    let ref_code = if payload.get("refCode").is_some() {
        parse_string(payload, "refCode")
    } else {
        cur_ref_code
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
    let primary_page = match payload.get("siyuanPrimaryPage") {
        Some(value) => crate::tools::pm_siyuan::parse_siyuan_page_ref_value(value)?,
        None => cur_primary_page,
    };
    let extra_pages = match crate::tools::pm_siyuan::parse_siyuan_page_ref_array(payload.get("siyuanExtraPages"))? {
        Some(pages) => pages,
        None => cur_extra_pages,
    };

    let (started_at, testing_at, completed_at) = resolve_status_flow_timestamps(
        &cur_status,
        &new_status,
        cur_started_at,
        cur_testing_at,
        cur_completed_at,
        payload
            .get("startedAt")
            .map(|value| value.as_str().map(|raw| raw.to_string())),
        payload
            .get("testingAt")
            .map(|value| value.as_str().map(|raw| raw.to_string())),
        payload
            .get("completedAt")
            .map(|value| value.as_str().map(|raw| raw.to_string())),
        &now,
    );

    conn.execute(
        "UPDATE pm_items
         SET title=?1, description=?2, link_url=?3, ref_code=?4, item_type=?5, priority=?6, status=?7,
             start_at=?8, end_at=?9,
             siyuan_doc_id=?10, siyuan_doc_title=?11, siyuan_doc_hpath=?12,
             siyuan_doc_path=?13, siyuan_notebook_id=?14, siyuan_notebook_name=?15,
             started_at=?16, testing_at=?17, completed_at=?18, updated_at=?19
         WHERE id=?20",
        params![
            title,
            desc,
            link_url,
            ref_code,
            item_type,
            priority,
            new_status,
            start_at,
            end_at,
            primary_page.as_ref().map(|item| item.doc_id.as_str()),
            primary_page.as_ref().map(|item| item.doc_title.as_str()),
            primary_page.as_ref().map(|item| item.doc_hpath.as_str()),
            primary_page.as_ref().and_then(|item| item.doc_path.as_deref()),
            primary_page.as_ref().map(|item| item.notebook_id.as_str()),
            primary_page.as_ref().map(|item| item.notebook_name.as_str()),
            started_at,
            testing_at,
            completed_at,
            now,
            id
        ],
    )
    .map_err(|e| format!("item_update: {e}"))?;

    if payload.get("tags").is_some() {
        let tags = parse_string_array(payload, "tags");
        save_tags(&conn, id, &tags)?;
    }
    crate::tools::pm_siyuan::save_item_siyuan_links(&conn, id, primary_page.as_ref(), &extra_pages, &now)?;

    Ok(json!({ "updated": true }))
}

fn item_change_status(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let new_status = parse_string(payload, "status").ok_or("status is required")?;
    if !STATUSES.contains(&new_status.as_str()) {
        return Err(format!("invalid status: {new_status}"));
    }
    let now = now_rfc3339();
    let mut conn = db_conn()?;
    let tx = conn.transaction().map_err(|e| format!("item_change_status begin: {e}"))?;

    let (cur_status, cur_started_at, cur_testing_at, cur_completed_at): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = tx
        .query_row(
            "SELECT status, started_at, testing_at, completed_at FROM pm_items WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|e| format!("item_change_status read: {e}"))?;

    let (started_at, testing_at, completed_at) = resolve_status_flow_timestamps(
        &cur_status,
        &new_status,
        cur_started_at,
        cur_testing_at,
        cur_completed_at,
        None,
        None,
        None,
        &now,
    );

    tx.execute(
        "UPDATE pm_items SET status = ?1, started_at = ?2, testing_at = ?3, completed_at = ?4, updated_at = ?5 WHERE id = ?6",
        params![new_status, started_at, testing_at, completed_at, now, id],
    )
    .map_err(|e| format!("item_change_status: {e}"))?;

    tx.commit().map_err(|e| format!("item_change_status commit: {e}"))?;
    Ok(json!({ "ok": true }))
}

/// Shared helper: update sort_order + status with proper timestamp logic during drag reorder.
fn reorder_with_timestamps(
    tx: &rusqlite::Transaction,
    id: i64,
    sort_order: i64,
    new_status: &str,
    now: &str,
) -> Result<(), String> {
    let (cur_status, cur_started_at, cur_testing_at, cur_completed_at): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = tx
        .query_row(
            "SELECT status, started_at, testing_at, completed_at FROM pm_items WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|e| format!("reorder_with_timestamps read: {e}"))?;

    if cur_status == new_status {
        tx.execute(
            "UPDATE pm_items SET sort_order = ?1, updated_at = ?3 WHERE id = ?2",
            params![sort_order, id, now],
        )
        .map_err(|e| format!("reorder_with_timestamps: {e}"))?;
        return Ok(());
    }

    let (started_at, testing_at, completed_at) = resolve_status_flow_timestamps(
        &cur_status,
        new_status,
        cur_started_at,
        cur_testing_at,
        cur_completed_at,
        None,
        None,
        None,
        now,
    );

    tx.execute(
        "UPDATE pm_items SET sort_order = ?1, status = ?2, started_at = ?3, testing_at = ?4, completed_at = ?5, updated_at = ?6 WHERE id = ?7",
        params![sort_order, new_status, started_at, testing_at, completed_at, now, id],
    )
    .map_err(|e| format!("reorder_with_timestamps: {e}"))?;
    Ok(())
}

fn item_reorder(payload: &Value) -> Result<Value, String> {
    let mut conn = db_conn()?;
    let now = now_rfc3339();

    let tx = conn.transaction().map_err(|e| format!("item_reorder begin: {e}"))?;

    if let Some(items) = payload.get("items").and_then(Value::as_array) {
        for item in items {
            let id = item.get("id").and_then(Value::as_i64).unwrap_or(0);
            let sort = item.get("sortOrder").and_then(Value::as_i64).unwrap_or(0);
            let status = item.get("status").and_then(Value::as_str);

            if let Some(st) = status {
                reorder_with_timestamps(&tx, id, sort, st, &now)?;
            } else {
                tx.execute(
                    "UPDATE pm_items SET sort_order = ?1, updated_at = ?3 WHERE id = ?2",
                    params![sort, id, now],
                )
                .map_err(|e| format!("item_reorder: {e}"))?;
            }
        }
        tx.commit().map_err(|e| format!("item_reorder commit: {e}"))?;
        return Ok(json!({ "ok": true }));
    }

    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let new_status = parse_string(payload, "status");
    let sort_order = parse_i64(payload, "sortOrder").unwrap_or(0);

    if let Some(st) = new_status {
        reorder_with_timestamps(&tx, id, sort_order, &st, &now)?;
    } else {
        tx.execute(
            "UPDATE pm_items SET sort_order = ?1, updated_at = ?3 WHERE id = ?2",
            params![sort_order, id, now],
        )
        .map_err(|e| format!("item_reorder: {e}"))?;
    }

    tx.commit().map_err(|e| format!("item_reorder commit: {e}"))?;
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

fn item_batch_update(payload: &Value) -> Result<Value, String> {
    let ids: Vec<i64> = payload
        .get("ids")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default();
    if ids.is_empty() {
        return Ok(json!({ "updated": 0 }));
    }

    let fields = payload.get("fields").ok_or("fields is required")?;
    let new_status = parse_string(fields, "status");
    let new_priority = parse_string(fields, "priority");
    let new_project_id = parse_i64(fields, "projectId");
    let pinned_value = fields.get("pinned").and_then(Value::as_bool);
    let add_tags: Vec<String> = parse_string_array(fields, "addTags");

    if new_status.is_none()
        && new_priority.is_none()
        && new_project_id.is_none()
        && pinned_value.is_none()
        && add_tags.is_empty()
    {
        return Ok(json!({ "updated": 0 }));
    }

    if let Some(ref s) = new_status {
        if !STATUSES.contains(&s.as_str()) {
            return Err(format!("invalid status: {s}"));
        }
    }
    if let Some(ref p) = new_priority {
        if !PRIORITIES.contains(&p.as_str()) {
            return Err(format!("invalid priority: {p}"));
        }
    }

    let now = now_rfc3339();
    let mut conn = db_conn()?;

    if let Some(pid) = new_project_id {
        ensure_project_accepts_items(&conn, pid)?;
    }

    let tx = conn
        .transaction()
        .map_err(|e| format!("item_batch_update begin: {e}"))?;

    let mut updated: u32 = 0;
    for id in &ids {
        let row = tx
            .query_row(
                "SELECT status, started_at, testing_at, completed_at, priority, project_id, pinned
                 FROM pm_items WHERE id = ?1",
                params![id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, Option<String>>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, bool>(6)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| format!("item_batch_update read: {e}"))?;

        let Some((
            cur_status,
            cur_started,
            cur_testing,
            cur_completed,
            cur_priority,
            cur_project_id,
            cur_pinned,
        )) = row
        else {
            continue;
        };

        let target_status = new_status.clone().unwrap_or_else(|| cur_status.clone());
        let target_priority = new_priority.clone().unwrap_or(cur_priority);
        let target_project_id = new_project_id.unwrap_or(cur_project_id);
        let target_pinned = pinned_value.unwrap_or(cur_pinned);

        let (started_at, testing_at, completed_at) = resolve_status_flow_timestamps(
            &cur_status,
            &target_status,
            cur_started,
            cur_testing,
            cur_completed,
            None,
            None,
            None,
            &now,
        );

        let changed = tx
            .execute(
                "UPDATE pm_items SET status = ?1, priority = ?2, project_id = ?3, pinned = ?4,
                        started_at = ?5, testing_at = ?6, completed_at = ?7, updated_at = ?8
                 WHERE id = ?9",
                params![
                    target_status,
                    target_priority,
                    target_project_id,
                    target_pinned,
                    started_at,
                    testing_at,
                    completed_at,
                    now,
                    id
                ],
            )
            .map_err(|e| format!("item_batch_update exec: {e}"))?;

        for tag in &add_tags {
            tx.execute(
                "INSERT OR IGNORE INTO pm_item_tags (item_id, tag) VALUES (?1, ?2)",
                params![id, tag],
            )
            .map_err(|e| format!("item_batch_update tag insert: {e}"))?;
        }

        updated += changed as u32;
    }

    tx.commit()
        .map_err(|e| format!("item_batch_update commit: {e}"))?;
    Ok(json!({ "updated": updated }))
}

fn item_delete(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let conn = db_conn()?;
    conn.execute("DELETE FROM pm_items WHERE id = ?1", params![id])
        .map_err(|e| format!("item_delete: {e}"))?;
    super::attachments::delete_by_owner_internal(&conn, "pm_item", &id.to_string())?;
    Ok(json!({ "ok": true }))
}

fn item_move_project(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let project_id = parse_i64(payload, "projectId").ok_or("projectId is required")?;
    let conn = db_conn()?;

    let linked_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pm_item_todo_links WHERE pm_item_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| format!("查询关联执行任务失败: {e}"))?;
    if linked_count > 0 {
        return Err("已关联执行任务的工作项不能直接切换到其他项目，请先解除所有关联".to_string());
    }

    ensure_project_accepts_items(&conn, project_id)?;
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

// ── Tests ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::pm_weekly as weekly;
    use chrono::NaiveDate;
    use rusqlite::{params, Connection};

    fn create_pm_reorder_test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "
            CREATE TABLE pm_items (
                id INTEGER PRIMARY KEY,
                status TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                started_at TEXT DEFAULT NULL,
                testing_at TEXT DEFAULT NULL,
                completed_at TEXT DEFAULT NULL,
                updated_at TEXT NOT NULL
            );
            ",
        )
        .expect("create pm_items schema");
        conn
    }

    fn seed_pm_reorder_item(
        conn: &Connection,
        id: i64,
        status: &str,
        sort_order: i64,
        started_at: Option<&str>,
        testing_at: Option<&str>,
        completed_at: Option<&str>,
        updated_at: &str,
    ) {
        conn.execute(
            "INSERT INTO pm_items(id, status, sort_order, started_at, testing_at, completed_at, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                status,
                sort_order,
                started_at,
                testing_at,
                completed_at,
                updated_at
            ],
        )
        .expect("seed pm item");
    }

    fn create_pm_siyuan_links_test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "
            CREATE TABLE pm_item_siyuan_links (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                item_id INTEGER NOT NULL,
                doc_id TEXT NOT NULL,
                doc_title TEXT NOT NULL,
                doc_hpath TEXT NOT NULL,
                doc_path TEXT DEFAULT NULL,
                notebook_id TEXT NOT NULL,
                notebook_name TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0
            );
            ",
        )
        .expect("create pm_item_siyuan_links schema");
        conn
    }

    fn create_pm_projects_test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "
            CREATE TABLE pm_projects (
                id INTEGER PRIMARY KEY,
                status TEXT NOT NULL
            );
            ",
        )
        .expect("create pm_projects schema");
        conn
    }

    #[test]
    fn normalize_pm_weekly_range_should_fold_single_side_and_swap_reverse_dates() {
        let single_start = weekly::normalize_pm_weekly_range(Some("2026-04-08"), None)
            .expect("single start should normalize");
        let single_end = weekly::normalize_pm_weekly_range(None, Some("2026-04-09T09:00:00+08:00"))
            .expect("single end should normalize");
        let reversed = weekly::normalize_pm_weekly_range(Some("2026-04-12"), Some("2026-04-03"))
            .expect("reversed range should normalize");

        assert_eq!(single_start, (
            NaiveDate::from_ymd_opt(2026, 4, 8).expect("valid date"),
            NaiveDate::from_ymd_opt(2026, 4, 8).expect("valid date"),
        ));
        assert_eq!(single_end, (
            NaiveDate::from_ymd_opt(2026, 4, 9).expect("valid date"),
            NaiveDate::from_ymd_opt(2026, 4, 9).expect("valid date"),
        ));
        assert_eq!(reversed, (
            NaiveDate::from_ymd_opt(2026, 4, 3).expect("valid date"),
            NaiveDate::from_ymd_opt(2026, 4, 12).expect("valid date"),
        ));
    }

    #[test]
    fn resolve_pm_weekly_window_hit_should_include_overlap_and_ignore_status_semantics() {
        let week_start = NaiveDate::from_ymd_opt(2026, 4, 6).expect("valid week start");
        let week_end = NaiveDate::from_ymd_opt(2026, 4, 12).expect("valid week end");

        let fully_inside = weekly::resolve_pm_weekly_window_hit(
            Some("2026-04-07"),
            Some("2026-04-10"),
            week_start,
            week_end,
        )
        .expect("inside range should hit");
        let crosses_into_week = weekly::resolve_pm_weekly_window_hit(
            Some("2026-04-04"),
            Some("2026-04-08"),
            week_start,
            week_end,
        )
        .expect("cross-week range should hit");
        let starts_this_week_ends_next_week = weekly::resolve_pm_weekly_window_hit(
            Some("2026-04-11"),
            Some("2026-04-15"),
            week_start,
            week_end,
        )
        .expect("range starting this week should hit");
        let outside = weekly::resolve_pm_weekly_window_hit(
            Some("2026-04-01"),
            Some("2026-04-05"),
            week_start,
            week_end,
        );

        assert_eq!(fully_inside, (
            NaiveDate::from_ymd_opt(2026, 4, 7).expect("valid date"),
            NaiveDate::from_ymd_opt(2026, 4, 10).expect("valid date"),
            NaiveDate::from_ymd_opt(2026, 4, 10).expect("valid date"),
        ));
        assert_eq!(crosses_into_week, (
            NaiveDate::from_ymd_opt(2026, 4, 4).expect("valid date"),
            NaiveDate::from_ymd_opt(2026, 4, 8).expect("valid date"),
            NaiveDate::from_ymd_opt(2026, 4, 8).expect("valid date"),
        ));
        assert_eq!(starts_this_week_ends_next_week, (
            NaiveDate::from_ymd_opt(2026, 4, 11).expect("valid date"),
            NaiveDate::from_ymd_opt(2026, 4, 15).expect("valid date"),
            NaiveDate::from_ymd_opt(2026, 4, 12).expect("valid date"),
        ));
        assert!(outside.is_none());
    }

    #[test]
    fn resolve_status_flow_timestamps_should_match_shared_transition_rules() {
        let (started_at, testing_at, completed_at) = resolve_status_flow_timestamps(
            "todo",
            "testing",
            None,
            None,
            None,
            None,
            None,
            None,
            "2026-04-08T12:00:00+08:00",
        );

        assert_eq!(started_at.as_deref(), Some("2026-04-08T12:00:00+08:00"));
        assert_eq!(testing_at.as_deref(), Some("2026-04-08T12:00:00+08:00"));
        assert_eq!(completed_at, None);
    }

    #[test]
    fn resolve_status_flow_timestamps_should_reset_done_when_returning_to_in_progress() {
        let (started_at, testing_at, completed_at) = resolve_status_flow_timestamps(
            "done",
            "in_progress",
            Some("2026-04-01T09:00:00+08:00".into()),
            Some("2026-04-02T09:00:00+08:00".into()),
            Some("2026-04-03T09:00:00+08:00".into()),
            None,
            None,
            None,
            "2026-04-08T12:00:00+08:00",
        );

        assert_eq!(started_at.as_deref(), Some("2026-04-01T09:00:00+08:00"));
        assert_eq!(testing_at.as_deref(), Some("2026-04-02T09:00:00+08:00"));
        assert_eq!(completed_at, None);
    }

    #[test]
    fn resolve_status_flow_timestamps_should_keep_same_status_without_side_effects() {
        let (started_at, testing_at, completed_at) = resolve_status_flow_timestamps(
            "testing",
            "testing",
            Some("2026-04-01T09:00:00+08:00".into()),
            Some("2026-04-02T09:00:00+08:00".into()),
            None,
            None,
            None,
            None,
            "2026-04-08T12:00:00+08:00",
        );

        assert_eq!(started_at.as_deref(), Some("2026-04-01T09:00:00+08:00"));
        assert_eq!(testing_at.as_deref(), Some("2026-04-02T09:00:00+08:00"));
        assert_eq!(completed_at, None);
    }

    #[test]
    fn resolve_status_flow_timestamps_should_respect_manual_overrides() {
        let (started_at, testing_at, completed_at) = resolve_status_flow_timestamps(
            "todo",
            "done",
            None,
            None,
            None,
            Some(Some("2026-03-31T08:00:00+08:00".into())),
            Some(None),
            Some(Some("2026-04-05T18:30:00+08:00".into())),
            "2026-04-08T12:00:00+08:00",
        );

        assert_eq!(started_at.as_deref(), Some("2026-03-31T08:00:00+08:00"));
        assert_eq!(testing_at, None);
        assert_eq!(completed_at.as_deref(), Some("2026-04-05T18:30:00+08:00"));
    }

    #[test]
    fn reorder_with_same_status_should_not_touch_flow_timestamps() {
        let mut conn = create_pm_reorder_test_conn();
        let started_at = "2026-04-01T09:00:00+08:00";
        let testing_at = "2026-04-02T09:00:00+08:00";
        let completed_at = "2026-04-03T09:00:00+08:00";
        let now = "2026-04-08T12:00:00+08:00";

        seed_pm_reorder_item(
            &conn,
            1,
            "done",
            7,
            Some(started_at),
            Some(testing_at),
            Some(completed_at),
            "2026-04-03T10:00:00+08:00",
        );

        let tx = conn.transaction().expect("begin tx");
        reorder_with_timestamps(&tx, 1, 2, "done", now).expect("reorder");
        tx.commit().expect("commit tx");

        let (
            status,
            sort_order,
            saved_started_at,
            saved_testing_at,
            saved_completed_at,
            updated_at,
        ): (
            String,
            i64,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
        ) = conn
            .query_row(
                "SELECT status, sort_order, started_at, testing_at, completed_at, updated_at
                 FROM pm_items WHERE id = ?1",
                params![1],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("read reordered item");

        assert_eq!(status, "done");
        assert_eq!(sort_order, 2);
        assert_eq!(saved_started_at.as_deref(), Some(started_at));
        assert_eq!(saved_testing_at.as_deref(), Some(testing_at));
        assert_eq!(saved_completed_at.as_deref(), Some(completed_at));
        assert_eq!(updated_at, now);
    }

    #[test]
    fn reorder_with_changed_status_should_apply_flow_timestamps() {
        let mut conn = create_pm_reorder_test_conn();
        let started_at = "2026-04-01T09:00:00+08:00";
        let testing_at = "2026-04-02T09:00:00+08:00";
        let now = "2026-04-08T12:00:00+08:00";

        seed_pm_reorder_item(
            &conn,
            1,
            "testing",
            4,
            Some(started_at),
            Some(testing_at),
            None,
            "2026-04-02T10:00:00+08:00",
        );

        let tx = conn.transaction().expect("begin tx");
        reorder_with_timestamps(&tx, 1, 0, "done", now).expect("reorder");
        tx.commit().expect("commit tx");

        let (status, sort_order, saved_started_at, saved_testing_at, saved_completed_at): (
            String,
            i64,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT status, sort_order, started_at, testing_at, completed_at
                 FROM pm_items WHERE id = ?1",
                params![1],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("read reordered item");

        assert_eq!(status, "done");
        assert_eq!(sort_order, 0);
        assert_eq!(saved_started_at.as_deref(), Some(started_at));
        assert_eq!(saved_testing_at.as_deref(), Some(testing_at));
        assert_eq!(saved_completed_at.as_deref(), Some(now));
    }

    #[test]
    fn batch_load_siyuan_links_should_use_pm_item_link_columns() {
        let conn = create_pm_siyuan_links_test_conn();
        conn.execute(
            "INSERT INTO pm_item_siyuan_links(
                item_id, doc_id, doc_title, doc_hpath, doc_path, notebook_id, notebook_name, sort_order
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                101_i64,
                "doc-101",
                "设计文档",
                "/项目/设计文档",
                "/project/design.sy",
                "nb-1",
                "产品库",
                0_i64,
            ],
        )
        .expect("seed siyuan link");

        let links_map = batch_load_siyuan_links(&conn, &[101]);

        let links = links_map.get(&101).expect("links for item 101");

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].doc_id, "doc-101");
        assert_eq!(links[0].doc_title, "设计文档");
        assert_eq!(links[0].doc_hpath, "/项目/设计文档");
        assert_eq!(links[0].doc_path.as_deref(), Some("/project/design.sy"));
        assert_eq!(links[0].notebook_id, "nb-1");
        assert_eq!(links[0].notebook_name, "产品库");
    }

    #[test]
    fn ensure_project_accepts_items_should_allow_active_projects() {
        let conn = create_pm_projects_test_conn();
        conn.execute(
            "INSERT INTO pm_projects(id, status) VALUES(?1, ?2)",
            params![1_i64, "active"],
        )
        .expect("seed active project");

        ensure_project_accepts_items(&conn, 1).expect("active project should accept items");
    }

    #[test]
    fn ensure_project_accepts_items_should_reject_archived_projects() {
        let conn = create_pm_projects_test_conn();
        conn.execute(
            "INSERT INTO pm_projects(id, status) VALUES(?1, ?2)",
            params![2_i64, "archived"],
        )
        .expect("seed archived project");

        let err = ensure_project_accepts_items(&conn, 2).expect_err("archived project should reject items");
        assert_eq!(err, "归档项目不能接收工作项，请先恢复项目");
    }

    #[test]
    fn build_item_list_sql_should_include_archived_projects_in_overview() {
        let overview_sql = build_item_list_sql(None);
        let project_sql = build_item_list_sql(Some(1));

        assert!(!overview_sql.contains("WHERE p.status = 'active'"));
        assert!(project_sql.contains("WHERE i.project_id = ?1"));
    }

    #[test]
    fn normalize_siyuan_base_url_should_trim_and_strip_trailing_slash() {
        let normalized =
            crate::tools::pm_siyuan::normalize_siyuan_base_url("  http://127.0.0.1:6806/  ").expect("normalize");
        assert_eq!(normalized, "http://127.0.0.1:6806");
    }

    #[test]
    fn normalize_siyuan_base_url_should_reject_invalid_scheme() {
        let err = crate::tools::pm_siyuan::normalize_siyuan_base_url("ftp://127.0.0.1:6806").expect_err("invalid scheme");
        assert!(err.contains("http://") || err.contains("https://"));
    }

    #[test]
    fn normalize_item_link_url_should_add_http_scheme() {
        let normalized = normalize_item_link_url("localhost:8080/docs").expect("normalize");
        assert_eq!(normalized, "http://localhost:8080/docs");
    }

    #[test]
    fn normalize_item_link_url_should_reject_unsupported_scheme() {
        let err = normalize_item_link_url("ftp://example.com").expect_err("invalid scheme");
        assert_eq!(err, "仅支持 http/https 链接");
    }

    #[test]
    fn build_siyuan_directory_should_nest_docs_by_hpath() {
        let notebooks = vec![crate::tools::pm_siyuan::SiyuanNotebook {
            id: "nb1".into(),
            name: "工作台".into(),
            icon: None,
            closed: false,
        }];
        let rows = vec![
            crate::tools::pm_siyuan::SiyuanDocRow {
                id: "doc-root".into(),
                box_id: "nb1".into(),
                path: Some("/doc-root.sy".into()),
                hpath: "/根文档".into(),
                name: "根文档".into(),
            },
            crate::tools::pm_siyuan::SiyuanDocRow {
                id: "doc-child".into(),
                box_id: "nb1".into(),
                path: Some("/doc-root/doc-child.sy".into()),
                hpath: "/根文档/子文档".into(),
                name: "子文档".into(),
            },
        ];

        let directory = crate::tools::pm_siyuan::build_siyuan_directory(notebooks, rows);
        assert_eq!(directory.notebooks.len(), 1);
        assert_eq!(directory.notebooks[0].doc_count, 2);
        assert_eq!(directory.notebooks[0].children.len(), 1);
        assert_eq!(directory.notebooks[0].children[0].id, "doc-root");
        assert!(!directory.notebooks[0].children[0].leaf);
        assert_eq!(directory.notebooks[0].children[0].children.len(), 1);
        assert_eq!(
            directory.notebooks[0].children[0].children[0].id,
            "doc-child"
        );
        assert!(directory.notebooks[0].children[0].children[0].leaf);
    }

    #[test]
    fn build_siyuan_directory_should_keep_duplicate_title_siblings_separate() {
        let notebooks = vec![crate::tools::pm_siyuan::SiyuanNotebook {
            id: "nb1".into(),
            name: "测试".into(),
            icon: None,
            closed: false,
        }];
        let rows = vec![
            crate::tools::pm_siyuan::SiyuanDocRow {
                id: "doc-a".into(),
                box_id: "nb1".into(),
                path: Some("/doc-a.sy".into()),
                hpath: "/测试".into(),
                name: "测试".into(),
            },
            crate::tools::pm_siyuan::SiyuanDocRow {
                id: "doc-a-child".into(),
                box_id: "nb1".into(),
                path: Some("/doc-a/doc-a-child.sy".into()),
                hpath: "/测试/子文档A".into(),
                name: "子文档A".into(),
            },
            crate::tools::pm_siyuan::SiyuanDocRow {
                id: "doc-b".into(),
                box_id: "nb1".into(),
                path: Some("/doc-b.sy".into()),
                hpath: "/测试".into(),
                name: "测试".into(),
            },
            crate::tools::pm_siyuan::SiyuanDocRow {
                id: "doc-b-child".into(),
                box_id: "nb1".into(),
                path: Some("/doc-b/doc-b-child.sy".into()),
                hpath: "/测试/子文档B".into(),
                name: "子文档B".into(),
            },
        ];

        let directory = crate::tools::pm_siyuan::build_siyuan_directory(notebooks, rows);
        assert_eq!(directory.notebooks.len(), 1);
        assert_eq!(directory.notebooks[0].doc_count, 4);
        assert_eq!(directory.notebooks[0].children.len(), 2);
        assert_eq!(directory.notebooks[0].children[0].id, "doc-a");
        assert_eq!(directory.notebooks[0].children[1].id, "doc-b");
        assert_eq!(
            directory.notebooks[0].children[0].children[0].id,
            "doc-a-child"
        );
        assert_eq!(
            directory.notebooks[0].children[1].children[0].id,
            "doc-b-child"
        );
    }

    #[test]
    fn normalize_siyuan_error_message_should_map_sql_disabled() {
        let message = crate::tools::pm_siyuan::parse_siyuan_envelope(r#"{"code":-1,"msg":"SQL is not available in publish mode"}"#).unwrap_err();
        assert_eq!(message, "当前思源实例未开放 SQL 查询能力");
    }

    #[test]
    fn build_siyuan_target_hpath_should_escape_path_separator() {
        let location = crate::tools::pm_siyuan::SiyuanLocation {
            notebook_id: "nb1".into(),
            notebook_name: "工作台".into(),
            parent_doc_id: Some("doc-root".into()),
            parent_doc_title: Some("根文档".into()),
            parent_hpath: Some("/根文档".into()),
            parent_path: Some("/root.sy".into()),
        };
        let hpath = crate::tools::pm_siyuan::build_siyuan_target_hpath(&location, "迭代/计划").expect("build hpath");
        assert_eq!(hpath, "/根文档/迭代／计划");
    }

    #[test]
    fn build_siyuan_search_scope_path_should_use_notebook_root() {
        let location = crate::tools::pm_siyuan::SiyuanLocation {
            notebook_id: "nb1".into(),
            notebook_name: "工作台".into(),
            parent_doc_id: None,
            parent_doc_title: None,
            parent_hpath: None,
            parent_path: None,
        };
        let path = crate::tools::pm_siyuan::build_siyuan_search_scope_path(&location).expect("build search scope");
        assert_eq!(path, "nb1");
    }

    #[test]
    fn build_siyuan_search_scope_path_should_scope_to_parent_subtree() {
        let location = crate::tools::pm_siyuan::SiyuanLocation {
            notebook_id: "nb1".into(),
            notebook_name: "工作台".into(),
            parent_doc_id: Some("doc-root".into()),
            parent_doc_title: Some("根文档".into()),
            parent_hpath: Some("/根文档".into()),
            parent_path: Some("/root.sy".into()),
        };
        let path = crate::tools::pm_siyuan::build_siyuan_search_scope_path(&location).expect("build search scope");
        assert_eq!(path, "nb1/root");
    }

    #[test]
    fn build_siyuan_search_scope_path_should_fallback_to_parent_doc_id() {
        let location = crate::tools::pm_siyuan::SiyuanLocation {
            notebook_id: "nb1".into(),
            notebook_name: "工作台".into(),
            parent_doc_id: Some("20260329232612-w2j085v".into()),
            parent_doc_title: Some("b".into()),
            parent_hpath: Some("/b".into()),
            parent_path: None,
        };
        let path = crate::tools::pm_siyuan::build_siyuan_search_scope_path(&location).expect("build search scope");
        assert_eq!(path, "nb1/20260329232612-w2j085v");
    }

    #[test]
    fn parse_siyuan_search_page_refs_should_map_document_blocks() {
        let notebooks = crate::tools::pm_siyuan::notebook_map(&[crate::tools::pm_siyuan::SiyuanNotebook {
            id: "nb1".into(),
            name: "测试".into(),
            icon: None,
            closed: false,
        }]);
        let data: serde_json::Value = serde_json::from_str(r#"{
            "blocks": [
                {
                    "id": "doc-bb",
                    "rootID": "doc-bb",
                    "box": "nb1",
                    "path": "/20260329232612-w2j085v/202604010001-bb.sy",
                    "hPath": "/b/",
                    "name": "",
                    "content": "<mark>bb</mark>a",
                    "type": "NodeDocument",
                    "ial": {
                        "title": "bba"
                    }
                }
            ]
        }"#).expect("parse json");

        let pages = crate::tools::pm_siyuan::parse_siyuan_search_page_refs(&data, &notebooks).expect("parse search pages");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].doc_id, "doc-bb");
        assert_eq!(pages[0].doc_title, "bba");
        assert_eq!(pages[0].doc_hpath, "/b/bba");
        assert_eq!(pages[0].doc_path.as_deref(), Some("/20260329232612-w2j085v/202604010001-bb.sy"));
        assert_eq!(pages[0].notebook_name, "测试");
    }

    #[test]
    fn normalize_siyuan_search_doc_hpath_should_append_title_for_parent_path() {
        assert_eq!(
            crate::tools::pm_siyuan::normalize_siyuan_search_doc_hpath(Some("/b/".into()), "bba"),
            "/b/bba"
        );
        assert_eq!(
            crate::tools::pm_siyuan::normalize_siyuan_search_doc_hpath(Some("/".into()), "首页"),
            "/首页"
        );
        assert_eq!(
            crate::tools::pm_siyuan::normalize_siyuan_search_doc_hpath(Some("/b/bba".into()), "bba"),
            "/b/bba"
        );
    }

    #[test]
    fn sort_and_limit_siyuan_pages_should_dedupe_and_prefer_title_match() {
        let pages = vec![
            crate::tools::pm_siyuan::SiyuanPageRef {
                doc_id: "doc-1".into(),
                doc_title: "bb".into(),
                doc_hpath: "/b/bb".into(),
                doc_path: Some("/doc-1.sy".into()),
                notebook_id: "nb1".into(),
                notebook_name: "测试".into(),
            },
            crate::tools::pm_siyuan::SiyuanPageRef {
                doc_id: "doc-1".into(),
                doc_title: "bb".into(),
                doc_hpath: "/b/bb".into(),
                doc_path: Some("/doc-1.sy".into()),
                notebook_id: "nb1".into(),
                notebook_name: "测试".into(),
            },
            crate::tools::pm_siyuan::SiyuanPageRef {
                doc_id: "doc-2".into(),
                doc_title: "需求记录".into(),
                doc_hpath: "/记录/bb-关联".into(),
                doc_path: Some("/doc-2.sy".into()),
                notebook_id: "nb1".into(),
                notebook_name: "测试".into(),
            },
        ];

        let sorted = crate::tools::pm_siyuan::sort_and_limit_siyuan_pages(pages, "bb");
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].doc_id, "doc-1");
    }

    #[test]
    fn build_siyuan_deep_link_should_follow_blocks_protocol() {
        let link = crate::tools::pm_siyuan::build_siyuan_deep_link("20260329120000-abc123").expect("deep link");
        assert_eq!(link, "siyuan://blocks/20260329120000-abc123");
    }
}

// ── Excel Import ───────────────────────────────────────────

fn cell_to_string(cell: &Data) -> Option<String> {
    match cell {
        Data::Empty => None,
        Data::String(s) => Some(s.trim().to_string()).filter(|v: &String| !v.is_empty()),
        Data::Float(f) => Some(format!("{f}")),
        Data::Int(i) => Some(format!("{i}")),
        Data::Bool(b) => Some(b.to_string()),
        Data::DateTime(dt) => Some(dt.to_string()),
        _ => None,
    }
}

fn excel_date_to_string(val: &str) -> Option<String> {
    if val.is_empty() {
        return None;
    }
    // Try ISO format: YYYY-MM-DD or YYYY/MM/DD
    let cleaned = val.replace('/', "-");
    if cleaned.len() >= 10 {
        let parts: Vec<&str> = cleaned.split('-').collect();
        if parts.len() == 3 {
            if let (Ok(y), Ok(m), Ok(d)) = (
                parts[0].parse::<i32>(),
                parts[1].parse::<u32>(),
                parts[2].parse::<u32>(),
            ) {
                if y > 1900 && m >= 1 && m <= 12 && d >= 1 && d <= 31 {
                    return Some(format!("{y:04}-{m:02}-{d:02}"));
                }
            }
        }
    }
    // Try Excel serial number (e.g. "45678")
    if let Ok(serial) = val.parse::<f64>() {
        if serial > 30000.0 && serial < 100000.0 {
            // Excel serial: days since 1900-01-01 (with the Lotus 1-2-3 bug)
            let base = chrono::NaiveDate::from_ymd_opt(1899, 12, 30).unwrap();
            let date = base + chrono::Duration::days(serial as i64);
            return Some(date.format("%Y-%m-%d").to_string());
        }
    }
    None
}

fn item_import_preview(payload: &Value) -> Result<Value, String> {
    let file_path = parse_string(payload, "filePath").ok_or("filePath is required")?;
    let mut workbook =
        open_workbook_auto(&file_path).map_err(|e| format!("无法打开 Excel 文件: {e}"))?;

    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();
    let first_sheet = sheet_names
        .first()
        .ok_or("Excel 文件没有工作表")?
        .clone();

    let range = workbook
        .worksheet_range(&first_sheet)
        .map_err(|e| format!("无法读取工作表: {e}"))?;

    let mut rows_iter = range.rows();
    let headers: Vec<String> = rows_iter
        .next()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(i, cell)| {
                    cell_to_string(cell).unwrap_or_else(|| format!("列{}", i + 1))
                })
                .collect()
        })
        .unwrap_or_default();

    let sample_rows: Vec<Vec<String>> = rows_iter
        .take(5)
        .map(|row| {
            row.iter()
                .map(|cell| cell_to_string(cell).unwrap_or_default())
                .collect()
        })
        .collect();

    Ok(json!({
        "sheetNames": sheet_names,
        "headers": headers,
        "sampleRows": sample_rows,
    }))
}

fn item_import(payload: &Value) -> Result<Value, String> {
    let file_path = parse_string(payload, "filePath").ok_or("filePath is required")?;

    let mapping = payload
        .get("mapping")
        .ok_or("mapping is required")?;
    let title_col = parse_string(mapping, "title").ok_or("mapping.title is required")?;
    let project_name_col = parse_string(mapping, "projectName");
    let start_at_col = parse_string(mapping, "startAt");
    let end_at_col = parse_string(mapping, "endAt");
    let desc_a_col = parse_string(mapping, "descriptionA");
    let desc_b_col = parse_string(mapping, "descriptionB");
    let ref_code_col = parse_string(mapping, "refCode");

    let filters: Vec<Value> = payload
        .get("filters")
        .and_then(|f| f.as_array())
        .cloned()
        .unwrap_or_default();

    let mut workbook =
        open_workbook_auto(&file_path).map_err(|e| format!("无法打开 Excel 文件: {e}"))?;
    let first_sheet = workbook
        .sheet_names()
        .first()
        .ok_or("Excel 文件没有工作表")?
        .clone();
    let range = workbook
        .worksheet_range(&first_sheet)
        .map_err(|e| format!("无法读取工作表: {e}"))?;

    let mut rows_iter = range.rows();
    let header_row = match rows_iter.next() {
        Some(row) => row,
        None => return Ok(json!({ "imported": 0, "skippedDuplicate": 0, "skippedFilter": 0, "skippedEmptyTitle": 0 })),
    };

    // Build column index map
    let col_index: HashMap<String, usize> = header_row
        .iter()
        .enumerate()
        .filter_map(|(i, cell)| cell_to_string(cell).map(|s| (s, i)))
        .collect();

    let title_idx = col_index
        .get(&title_col)
        .ok_or(format!("映射列 '{title_col}' 在 Excel 中不存在"))?;
    let start_at_idx = start_at_col.as_ref().and_then(|c| col_index.get(c));
    let end_at_idx = end_at_col.as_ref().and_then(|c| col_index.get(c));
    let desc_a_idx = desc_a_col.as_ref().and_then(|c| col_index.get(c));
    let desc_b_idx = desc_b_col.as_ref().and_then(|c| col_index.get(c));
    let ref_code_idx = ref_code_col.as_ref().and_then(|c| col_index.get(c));
    let project_name_idx = project_name_col.as_ref().and_then(|c| col_index.get(c));

    // Build filter column indices
    let filter_specs: Vec<(usize, String, String)> = filters
        .iter()
        .filter_map(|f| {
            let col_name = f.get("column")?.as_str()?.to_string();
            let idx = col_index.get(&col_name)?;
            let op = f.get("operator")?.as_str()?.to_string();
            let val = f.get("value")?.as_str()?.to_string();
            Some((*idx, op, val))
        })
        .collect();

    // Load existing ref_codes for dedup
    let mut conn = db_conn()?;
    let existing_refs: HashSet<String> = {
        let mut stmt = conn
            .prepare("SELECT ref_code FROM pm_items WHERE ref_code IS NOT NULL")
            .map_err(|e| format!("加载已有编号: {e}"))?;
        let refs: HashSet<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| format!("查询已有编号: {e}"))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);
        refs
    };

    let mut imported = 0u32;
    let mut skipped_duplicate = 0u32;
    let mut skipped_filter = 0u32;
    let mut skipped_empty_title = 0u32;
    let mut skipped_no_project = 0u32;
    let mut projects_created = 0u32;
    let now = now_rfc3339();

    let tx = conn
        .transaction()
        .map_err(|e| format!("import begin: {e}"))?;

    // Load project name -> id map
    let mut project_map: HashMap<String, i64> = {
        let mut stmt = tx
            .prepare("SELECT id, name FROM pm_projects")
            .map_err(|e| format!("加载项目列表: {e}"))?;
        let map: HashMap<String, i64> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, i64>(0)?)))
            .map_err(|e| format!("查询项目列表: {e}"))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);
        map
    };

    // Default project for rows without project name
    let default_project_id: Option<i64> = if project_name_idx.is_none() {
        project_map.values().next().copied()
    } else {
        None
    };

    let mut create_project_stmt = tx.prepare(
        "INSERT INTO pm_projects (name, description, color, created_at, updated_at)
         VALUES (?1, '', '#409eff', ?2, ?3)",
    ).map_err(|e| format!("prepare create project: {e}"))?;

    {
        let mut insert_stmt = tx.prepare(
            "INSERT INTO pm_items (
                project_id, title, description, ref_code, item_type, priority, status,
                start_at, end_at,
                started_at, testing_at, completed_at, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 'task', 'P2', 'todo', ?5, ?6, NULL, NULL, NULL, ?7, ?8)",
        ).map_err(|e| format!("prepare insert: {e}"))?;

        for row in rows_iter {
            // Get title
            let title = row
                .get(*title_idx)
                .and_then(|cell| cell_to_string(cell))
                .unwrap_or_default();

            if title.is_empty() {
                skipped_empty_title += 1;
                continue;
            }

            // Read project name (defer creation until after filtering)
            let project_name = if let Some(&pidx) = project_name_idx {
                let pname = row
                    .get(pidx)
                    .and_then(|cell| cell_to_string(cell))
                    .unwrap_or_default();
                if pname.is_empty() {
                    skipped_no_project += 1;
                    continue;
                }
                Some(pname)
            } else {
                None
            };

            // Get ref_code and check duplicate
            let ref_code = ref_code_idx.and_then(|&i| row.get(i)).and_then(|cell| cell_to_string(cell));
            if let Some(ref rc) = ref_code {
                if existing_refs.contains(rc) {
                    skipped_duplicate += 1;
                    continue;
                }
            }

            // Apply filters (keep mode: only rows matching all rules are kept)
            let mut matched = true;
            for &(idx, ref op, ref val) in &filter_specs {
                let cell_val = row
                    .get(idx)
                    .and_then(|cell| cell_to_string(cell))
                    .unwrap_or_default();
                let cell_lower = cell_val.to_lowercase();
                let val_lower = val.to_lowercase();
                let rule_match = match op.as_str() {
                    "contains" => cell_lower.contains(&val_lower),
                    "not_contains" => !cell_lower.contains(&val_lower),
                    "equals" => cell_lower == val_lower,
                    "not_equals" => cell_lower != val_lower,
                    "empty" => cell_val.is_empty(),
                    "not_empty" => !cell_val.is_empty(),
                    _ => true,
                };
                if !rule_match {
                    matched = false;
                    break;
                }
            }
            if !matched {
                skipped_filter += 1;
                continue;
            }

            // Resolve project_id (only after all checks pass)
            let project_id = if let Some(ref pname) = project_name {
                if let Some(&pid) = project_map.get(pname) {
                    pid
                } else {
                    create_project_stmt
                        .execute(params![pname, now, now])
                        .map_err(|e| format!("创建项目 '{pname}': {e}"))?;
                    let pid = tx.last_insert_rowid();
                    project_map.insert(pname.clone(), pid);
                    projects_created += 1;
                    pid
                }
            } else if let Some(pid) = default_project_id {
                pid
            } else {
                skipped_no_project += 1;
                continue;
            };

            // Build description
            let desc_a = desc_a_idx
                .and_then(|&i| row.get(i))
                .and_then(|cell| cell_to_string(cell));
            let desc_b = desc_b_idx
                .and_then(|&i| row.get(i))
                .and_then(|cell| cell_to_string(cell));
            let description = match (desc_a, desc_b) {
                (Some(a), Some(b)) => format!("{a}\n\n{b}"),
                (Some(a), None) => a,
                (None, Some(b)) => b,
                _ => String::new(),
            };

            // Parse dates
            let start_at = start_at_idx
                .and_then(|&i| row.get(i))
                .and_then(|cell| cell_to_string(cell))
                .and_then(|v| excel_date_to_string(&v));
            let end_at = end_at_idx
                .and_then(|&i| row.get(i))
                .and_then(|cell| cell_to_string(cell))
                .and_then(|v| excel_date_to_string(&v));

            insert_stmt
                .execute(params![
                    project_id,
                    title,
                    description,
                    ref_code,
                    start_at,
                    end_at,
                    now,
                    now,
                ])
                .map_err(|e| format!("insert row: {e}"))?;

            imported += 1;
        }
        drop(create_project_stmt);
    }

    tx.commit()
        .map_err(|e| format!("import commit: {e}"))?;

    Ok(json!({
        "imported": imported,
        "skippedDuplicate": skipped_duplicate,
        "skippedFilter": skipped_filter,
        "skippedEmptyTitle": skipped_empty_title,
        "skippedNoProject": skipped_no_project,
        "projectsCreated": projects_created,
    }))
}

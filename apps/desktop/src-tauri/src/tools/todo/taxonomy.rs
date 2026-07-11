use rusqlite::params;
use serde_json::{json, Value};

use crate::tools::helpers::db_conn;

use super::helpers::*;

// ── Type CRUD ─────────────────────────────────────────────

pub(crate) fn type_list() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, color, builtin, sort_order, created_at, updated_at
             FROM todo_types ORDER BY builtin DESC, sort_order ASC, id ASC",
        )
        .map_err(|e| format!("查询待办类型失败: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            let created_at = row.get::<_, String>(5).map(|s| format_db_datetime(&s))?;
            let updated_at = row.get::<_, String>(6).map(|s| format_db_datetime(&s))?;
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "color": row.get::<_, String>(2)?,
                "builtin": row.get::<_, i64>(3)? == 1,
                "sortOrder": row.get::<_, i64>(4)?,
                "createdAt": created_at,
                "updatedAt": updated_at
            }))
        })
        .map_err(|e| format!("映射待办类型失败: {e}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| e.to_string())?);
    }
    Ok(json!({ "items": items }))
}

pub(crate) fn type_upsert(payload: &Value) -> Result<Value, String> {
    let name = parse_string(payload, "name").ok_or("待办类型名称不能为空")?;
    let color = parse_string(payload, "color").unwrap_or_else(|| "#409eff".to_string());
    let conn = db_conn()?;
    if let Some(id) = parse_i64(payload, "id") {
        conn.execute(
            "UPDATE todo_types SET name=?1, color=?2, updated_at=CURRENT_TIMESTAMP WHERE id=?3",
            params![name, color, id],
        )
        .map_err(|e| format!("更新待办类型失败: {e}"))?;
        Ok(json!({ "ok": true, "id": id }))
    } else {
        let sort_order = parse_i64(payload, "sortOrder").unwrap_or(0);
        conn.execute(
            "INSERT INTO todo_types(name, color, sort_order) VALUES(?1, ?2, ?3)",
            params![name, color, sort_order],
        )
        .map_err(|e| format!("新增待办类型失败: {e}"))?;
        Ok(json!({ "ok": true, "id": conn.last_insert_rowid() }))
    }
}

pub(crate) fn type_delete(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缺少类型 id")?;
    let conn = db_conn()?;
    conn.execute(
        "UPDATE todo_items SET type_id=NULL WHERE type_id=?1",
        params![id],
    )
    .map_err(|e| format!("解绑待办类型失败: {e}"))?;
    conn.execute("DELETE FROM todo_types WHERE id=?1", params![id])
        .map_err(|e| format!("删除待办类型失败: {e}"))?;
    Ok(json!({ "ok": true }))
}

// ── Assignee CRUD ─────────────────────────────────────────

pub(crate) fn assignee_list() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare("SELECT id, name, created_at, updated_at FROM todo_assignees ORDER BY name COLLATE NOCASE ASC, id ASC")
        .map_err(|e| format!("查询执行人失败: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            let created_at = row.get::<_, String>(2).map(|s| format_db_datetime(&s))?;
            let updated_at = row.get::<_, String>(3).map(|s| format_db_datetime(&s))?;
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "createdAt": created_at,
                "updatedAt": updated_at
            }))
        })
        .map_err(|e| format!("映射执行人失败: {e}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| e.to_string())?);
    }
    Ok(json!({ "items": items }))
}

pub(crate) fn assignee_upsert(payload: &Value) -> Result<Value, String> {
    let name = parse_string(payload, "name").ok_or("执行人名称不能为空")?;
    let conn = db_conn()?;
    if let Some(id) = parse_i64(payload, "id") {
        conn.execute(
            "UPDATE todo_assignees SET name=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![name, id],
        )
        .map_err(|e| format!("更新执行人失败: {e}"))?;
        Ok(json!({ "ok": true, "id": id }))
    } else {
        conn.execute("INSERT INTO todo_assignees(name) VALUES(?1)", params![name])
            .map_err(|e| format!("新增执行人失败: {e}"))?;
        Ok(json!({ "ok": true, "id": conn.last_insert_rowid() }))
    }
}

pub(crate) fn assignee_delete(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缺少执行人 id")?;
    let conn = db_conn()?;
    conn.execute(
        "DELETE FROM todo_item_assignees WHERE assignee_id=?1",
        params![id],
    )
    .map_err(|e| format!("删除事项执行人关联失败: {e}"))?;
    conn.execute("DELETE FROM todo_assignees WHERE id=?1", params![id])
        .map_err(|e| format!("删除执行人失败: {e}"))?;
    Ok(json!({ "ok": true }))
}

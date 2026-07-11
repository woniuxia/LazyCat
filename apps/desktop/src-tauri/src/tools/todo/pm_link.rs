use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

use crate::tools::helpers::db_conn;

use super::helpers::*;
use super::types::*;

// ── PM-Todo linking (Todo side) ────────────────────────────

/// Return PM items that a Todo can be linked to (same project).
pub(crate) fn pm_candidates(payload: &Value) -> Result<Value, String> {
    let project_id = parse_i64(payload, "projectId").ok_or("projectId is required")?;
    let conn = db_conn()?;

    let mut stmt = conn
        .prepare(
            "SELECT i.id, i.title, i.status, i.priority, i.project_id,
                    p.name AS project_name, p.color AS project_color
             FROM pm_items i
             LEFT JOIN pm_projects p ON p.id = i.project_id
             WHERE i.project_id = ?1
             ORDER BY
                CASE i.status WHEN 'done' THEN 1 ELSE 0 END,
                CASE i.priority WHEN 'P0' THEN 0 WHEN 'P1' THEN 1 WHEN 'P2' THEN 2 ELSE 3 END,
                i.id DESC
             LIMIT 200",
        )
        .map_err(|e| format!("pm_candidates prepare: {e}"))?;

    let items: Vec<Value> = stmt
        .query_map(params![project_id], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "title": r.get::<_, String>(1)?,
                "status": r.get::<_, String>(2)?,
                "priority": r.get::<_, String>(3)?,
                "projectId": r.get::<_, i64>(4)?,
                "projectName": r.get::<_, Option<String>>(5)?,
                "projectColor": r.get::<_, Option<String>>(6)?,
            }))
        })
        .map_err(|e| format!("pm_candidates query: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(json!({ "items": items }))
}

/// Set or clear the PM link for a Todo item.
pub(crate) fn item_set_pm_link(payload: &Value) -> Result<Value, String> {
    let todo_item_id = parse_i64(payload, "todoItemId").ok_or("todoItemId is required")?;
    let new_pm_item_id = parse_i64(payload, "pmItemId"); // None = clear

    let conn = db_conn()?;

    // Verify todo exists and get kind + project_id
    let (kind, todo_project_id): (String, Option<i64>) = conn
        .query_row(
            "SELECT kind, project_id FROM todo_items WHERE id = ?1",
            params![todo_item_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| "事项不存在".to_string())?;

    // Only one_off allowed
    if kind != SERIES_KIND_ONE_OFF {
        return Err("重复事项暂不支持关联项目工作项".to_string());
    }

    if let Some(pm_id) = new_pm_item_id {
        // Setting or changing PM link
        // Verify PM item exists and get its project
        let pm_project_id: i64 = conn
            .query_row(
                "SELECT project_id FROM pm_items WHERE id = ?1",
                params![pm_id],
                |r| r.get(0),
            )
            .map_err(|e| format!("项目工作项不存在: {e}"))?;

        // Todo must have a project to link to PM
        let todo_pid = todo_project_id.ok_or_else(|| {
            "请先选择项目，或从项目管理工作项内绑定该任务".to_string()
        })?;

        // Same project required
        if todo_pid != pm_project_id {
            return Err("只能关联同一项目下的工作项，跨项目请先清除关联再改项目".to_string());
        }

        // Upsert: if already linked, change; otherwise insert
        let existing_link: Option<i64> = conn
            .query_row(
                "SELECT pm_item_id FROM pm_item_todo_links WHERE todo_item_id = ?1",
                params![todo_item_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("查询关联失败: {e}"))?;

        let now = chrono::Utc::now().to_rfc3339();
        if let Some(old_pm_id) = existing_link {
            if old_pm_id == pm_id {
                return Ok(json!({ "ok": true })); // already linked to same PM
            }
            // Change link
            conn.execute(
                "UPDATE pm_item_todo_links SET pm_item_id = ?1, updated_at = ?2 WHERE todo_item_id = ?3",
                params![pm_id, now, todo_item_id],
            )
            .map_err(|e| format!("改挂关联失败: {e}"))?;
        } else {
            // Insert new link
            conn.execute(
                "INSERT INTO pm_item_todo_links (pm_item_id, todo_item_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
                params![pm_id, todo_item_id, now, now],
            )
            .map_err(|e| format!("设置关联失败: {e}"))?;
        }
    } else {
        // Clear PM link (pmItemId = null or not provided)
        conn.execute(
            "DELETE FROM pm_item_todo_links WHERE todo_item_id = ?1",
            params![todo_item_id],
        )
        .map_err(|e| format!("清除关联失败: {e}"))?;
    }

    Ok(json!({ "ok": true }))
}

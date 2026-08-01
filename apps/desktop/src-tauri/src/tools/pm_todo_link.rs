use chrono::Utc;
use rusqlite::params;
use serde_json::{json, Value};
use std::collections::HashMap;

use super::helpers::db_conn;
use super::todo::{compute_remind_at, reminder_configs_from_presets};

pub fn item_todo_list(payload: &Value) -> Result<Value, String> {
    let pm_item_id =
        crate::tools::pm::parse_i64(payload, "pmItemId").ok_or("pmItemId is required")?;
    let conn = db_conn()?;

    // Verify PM item exists
    let project_id: i64 = conn
        .query_row(
            "SELECT project_id FROM pm_items WHERE id = ?1",
            params![pm_item_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("PM 工作项不存在: {e}"))?;

    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.title, t.status, t.priority, t.event_at, t.kind,
                    t.project_id, t.completed_at
             FROM todo_items t
             JOIN pm_item_todo_links l ON l.todo_item_id = t.id
             WHERE l.pm_item_id = ?1
             ORDER BY
                CASE t.status WHEN 'completed' THEN 1 ELSE 0 END,
                CASE t.priority WHEN 'P0' THEN 0 WHEN 'P1' THEN 1 WHEN 'P2' THEN 2 ELSE 3 END,
                t.event_at ASC",
        )
        .map_err(|e| format!("item_todo_list prepare: {e}"))?;

    let rows: Vec<Value> = stmt
        .query_map(params![pm_item_id], |r| {
            let status: String = r.get(2)?;
            let event_at: Option<String> = r.get(4)?;
            let is_overdue = (status == "pending" || status == "in_progress")
                && event_at
                    .as_deref()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc) < chrono::Utc::now())
                    .unwrap_or(false);
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "title": r.get::<_, String>(1)?,
                "status": status,
                "priority": r.get::<_, String>(3)?,
                "eventAt": event_at,
                "kind": r.get::<_, String>(5)?,
                "projectId": r.get::<_, Option<i64>>(6)?,
                "completedAt": r.get::<_, Option<String>>(7)?,
                "isOverdue": is_overdue,
            }))
        })
        .map_err(|e| format!("item_todo_list query: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let total = rows.len() as i64;
    let completed = rows
        .iter()
        .filter(|r| r["status"].as_str() == Some("completed"))
        .count() as i64;
    let pending = total - completed;

    // Load project name/color for each todo's projectId
    let mut proj_cache: HashMap<i64, (String, String)> = HashMap::new();
    let items: Vec<Value> = rows
        .into_iter()
        .map(|mut item| {
            if let Some(pid) = item["projectId"].as_i64() {
                let (pname, pcolor) = if let Some(cached) = proj_cache.get(&pid) {
                    cached.clone()
                } else {
                    let info: (String, String) = conn
                        .query_row(
                            "SELECT name, color FROM pm_projects WHERE id = ?1",
                            params![pid],
                            |r| Ok((r.get(0)?, r.get(1)?)),
                        )
                        .unwrap_or_else(|_| ("".to_string(), "".to_string()));
                    proj_cache.insert(pid, info.clone());
                    info
                };
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("projectName".to_string(), json!(pname));
                    obj.insert("projectColor".to_string(), json!(pcolor));
                }
            }
            item
        })
        .collect();

    Ok(json!({
        "items": items,
        "totalCount": total,
        "completedCount": completed,
        "pendingCount": pending,
        "projectId": project_id,
    }))
}

/// Batch-link existing Todos to a PM item (single transaction, all-or-nothing).
pub fn item_todo_link(payload: &Value) -> Result<Value, String> {
    let pm_item_id =
        crate::tools::pm::parse_i64(payload, "pmItemId").ok_or("pmItemId is required")?;
    let todo_ids = payload
        .get("todoItemIds")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_i64).collect::<Vec<i64>>())
        .unwrap_or_default();

    if todo_ids.is_empty() {
        return Err("todoItemIds 不能为空".to_string());
    }

    let mut conn = db_conn()?;

    // Get PM item's project
    let pm_project_id: i64 = conn
        .query_row(
            "SELECT project_id FROM pm_items WHERE id = ?1",
            params![pm_item_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("PM 工作项不存在: {e}"))?;

    let tx = conn
        .transaction()
        .map_err(|e| format!("item_todo_link begin: {e}"))?;
    let now = crate::tools::pm::now_rfc3339();

    for &tid in &todo_ids {
        // Verify todo exists and get kind + project_id
        let (kind, todo_project_id): (String, Option<i64>) = tx
            .query_row(
                "SELECT kind, project_id FROM todo_items WHERE id = ?1",
                params![tid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| format!("事项 {tid} 不存在: {e}"))?;

        // Only one_off allowed
        if kind != "one_off" {
            return Err(format!(
                "事项 {tid} 不是普通一次性任务，无法关联到项目工作项"
            ));
        }

        // Check not already linked
        let already_linked: bool = tx
            .query_row(
                "SELECT COUNT(*) FROM pm_item_todo_links WHERE todo_item_id = ?1",
                params![tid],
                |r| r.get::<_, i64>(0).map(|c| c > 0),
            )
            .unwrap_or(false);
        if already_linked {
            return Err(format!("事项 {tid} 已关联到其他项目工作项"));
        }

        // Check project consistency: allow same project or null project
        match todo_project_id {
            Some(pid) if pid != pm_project_id => {
                return Err(format!("事项 {tid} 属于其他项目，无法绑定到当前工作项"));
            }
            _ => {}
        }

        // If todo has no project, auto-fill with PM's project
        if todo_project_id.is_none() {
            tx.execute(
                "UPDATE todo_items SET project_id = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
                params![pm_project_id, tid],
            )
            .map_err(|e| format!("补齐事项 {tid} 项目归属失败: {e}"))?;
        }

        // Insert link
        tx.execute(
            "INSERT INTO pm_item_todo_links (pm_item_id, todo_item_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![pm_item_id, tid, now, now],
        )
        .map_err(|e| format!("绑定事项 {tid} 失败: {e}"))?;
    }

    tx.commit()
        .map_err(|e| format!("item_todo_link commit: {e}"))?;
    Ok(json!({ "ok": true, "linkedCount": todo_ids.len() }))
}

/// Unlink a Todo from a PM item.
pub fn item_todo_unlink(payload: &Value) -> Result<Value, String> {
    let pm_item_id =
        crate::tools::pm::parse_i64(payload, "pmItemId").ok_or("pmItemId is required")?;
    let todo_item_id =
        crate::tools::pm::parse_i64(payload, "todoItemId").ok_or("todoItemId is required")?;
    let conn = db_conn()?;
    let changed = conn
        .execute(
            "DELETE FROM pm_item_todo_links WHERE pm_item_id = ?1 AND todo_item_id = ?2",
            params![pm_item_id, todo_item_id],
        )
        .map_err(|e| format!("item_todo_unlink: {e}"))?;
    if changed == 0 {
        return Err("关联记录不存在".to_string());
    }
    Ok(json!({ "ok": true }))
}

/// Atomically create a new one-off Todo and link it to a PM item in a single transaction.
pub fn item_todo_create(payload: &Value) -> Result<Value, String> {
    let pm_item_id =
        crate::tools::pm::parse_i64(payload, "pmItemId").ok_or("pmItemId is required")?;
    let title = crate::tools::pm::parse_string(payload, "title").ok_or("title is required")?;
    let description = crate::tools::pm::parse_string(payload, "description").unwrap_or_default();
    let priority = crate::tools::pm::parse_string(payload, "priority")
        .filter(|v| crate::tools::pm::PRIORITIES.contains(&v.as_str()))
        .unwrap_or_else(|| "P2".to_string());
    let assignee_ids: Vec<i64> = payload
        .get("assigneeIds")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_i64)
                .filter(|id| *id > 0)
                .collect()
        })
        .unwrap_or_default();
    let event_at = crate::tools::pm::parse_string(payload, "eventAt");
    let reminder_presets: Vec<String> = payload
        .get("reminderPresets")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();

    let mut conn = db_conn()?;

    // Get PM item's project
    let pm_project_id: i64 = conn
        .query_row(
            "SELECT project_id FROM pm_items WHERE id = ?1",
            params![pm_item_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("PM 工作项不存在: {e}"))?;

    let tx = conn
        .transaction()
        .map_err(|e| format!("item_todo_create begin: {e}"))?;
    let now = crate::tools::pm::now_rfc3339();

    // Create one-off Todo with pending status
    tx.execute(
        "INSERT INTO todo_items (title, priority, description, kind, status, event_at, project_id, pinned)
         VALUES (?1, ?2, ?3, 'one_off', 'pending', ?4, ?5, 0)",
        params![title, priority, description, event_at, pm_project_id],
    )
    .map_err(|e| format!("item_todo_create insert todo: {e}"))?;

    let todo_id = tx.last_insert_rowid();

    // Insert PM-Todo link
    tx.execute(
        "INSERT INTO pm_item_todo_links (pm_item_id, todo_item_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        params![pm_item_id, todo_id, now, now],
    )
    .map_err(|e| format!("item_todo_create insert link: {e}"))?;

    // Sync assignees
    if !assignee_ids.is_empty() {
        for &aid in &assignee_ids {
            tx.execute(
                "INSERT OR IGNORE INTO todo_item_assignees (item_id, assignee_id) VALUES (?1, ?2)",
                params![todo_id, aid],
            )
            .map_err(|e| format!("item_todo_create assignee: {e}"))?;
        }
    }

    // Sync reminders (reuse shared helpers from todo module)
    if !reminder_presets.is_empty() {
        let configs = reminder_configs_from_presets(&reminder_presets);
        if !configs.is_empty() {
            let evt = event_at
                .as_deref()
                .ok_or("设置提醒前需要先提供事件时间或周期规则".to_string())?;
            for config in &configs {
                let remind_at = compute_remind_at(Some(evt), Some(config.offset_minutes))?
                    .ok_or("提醒时间生成失败".to_string())?;
                tx.execute(
                    "INSERT INTO todo_item_reminders (item_id, reminder_preset, offset_minutes, remind_at) VALUES (?1, ?2, ?3, ?4)",
                    params![todo_id, config.preset, config.offset_minutes, remind_at],
                )
                .map_err(|e| format!("item_todo_create reminder: {e}"))?;
            }
        }
    }

    tx.commit()
        .map_err(|e| format!("item_todo_create commit: {e}"))?;

    Ok(json!({ "ok": true, "id": todo_id }))
}

/// Return candidate Todos that can be linked to a PM item.
pub fn item_todo_candidates(payload: &Value) -> Result<Value, String> {
    let pm_item_id =
        crate::tools::pm::parse_i64(payload, "pmItemId").ok_or("pmItemId is required")?;
    let keyword = crate::tools::pm::parse_string(payload, "keyword");
    let limit = crate::tools::pm::parse_i64(payload, "limit")
        .unwrap_or(50)
        .min(100) as usize;

    // Parse status filter
    let status_filter: Option<Vec<String>> =
        payload
            .get("statuses")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.to_string())
                    .collect()
            });

    let conn = db_conn()?;

    // Get PM item's project
    let pm_project_id: i64 = conn
        .query_row(
            "SELECT project_id FROM pm_items WHERE id = ?1",
            params![pm_item_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("PM 工作项不存在: {e}"))?;

    // Query candidates: same project or null project, one_off, not already linked
    let mut sql = String::from(
        "SELECT t.id, t.title, t.status, t.priority, t.event_at, t.project_id
         FROM todo_items t
         WHERE t.kind = 'one_off'
           AND NOT EXISTS (
               SELECT 1 FROM pm_item_todo_links l WHERE l.todo_item_id = t.id
           )
           AND (t.project_id = ?1 OR t.project_id IS NULL)",
    );

    // Add keyword filter
    if keyword.is_some() {
        sql.push_str(" AND t.title LIKE '%' || ?2 || '%'");
    }

    // Add status filter
    let has_status_filter = status_filter.is_some();
    if has_status_filter {
        sql.push_str(" AND t.status IN (");
        let start_idx = if keyword.is_some() { 3 } else { 2 };
        let placeholders: Vec<String> = status_filter
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", start_idx + i))
            .collect();
        sql.push_str(&placeholders.join(", "));
        sql.push(')');
    }

    sql.push_str(" ORDER BY CASE t.status WHEN 'completed' THEN 1 ELSE 0 END, t.event_at DESC");
    sql.push_str(" LIMIT ");
    sql.push_str(&(limit + 1).to_string());

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("item_todo_candidates prepare: {e}"))?;

    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(pm_project_id)];
    if let Some(ref kw) = keyword {
        param_values.push(Box::new(kw.clone()));
    }
    if let Some(ref statuses) = status_filter {
        for s in statuses {
            param_values.push(Box::new(s.clone()));
        }
    }
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let rows: Vec<Value> = stmt
        .query_map(param_refs.as_slice(), |r| {
            let project_id: Option<i64> = r.get(5)?;
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "title": r.get::<_, String>(1)?,
                "status": r.get::<_, String>(2)?,
                "priority": r.get::<_, String>(3)?,
                "eventAt": r.get::<_, Option<String>>(4)?,
                "projectId": project_id,
                "isUnassignedProject": project_id.is_none(),
            }))
        })
        .map_err(|e| format!("item_todo_candidates query: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let overflow = rows.len() > limit;
    let items: Vec<Value> = rows.into_iter().take(limit).collect();
    let total = items.len() as i64;

    // Compute blocked count (todos linked to *other* PM items in same project)
    let blocked_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM todo_items t
             JOIN pm_item_todo_links l ON l.todo_item_id = t.id
             WHERE t.kind = 'one_off'
               AND l.pm_item_id != ?2
               AND (t.project_id = ?1 OR t.project_id IS NULL)",
            params![pm_project_id, pm_item_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let reason = if total == 0 && blocked_count == 0 {
        "empty"
    } else if total == 0 && blocked_count > 0 {
        "blocked_only"
    } else if overflow {
        "overflow"
    } else {
        "ok"
    };

    // Attach project name for candidates that have a project
    let mut proj_cache: HashMap<i64, (String, String)> = HashMap::new();
    let items: Vec<Value> = items
        .into_iter()
        .map(|mut item| {
            if let Some(pid) = item["projectId"].as_i64() {
                let (pname, pcolor) = if let Some(cached) = proj_cache.get(&pid) {
                    cached.clone()
                } else {
                    let info: (String, String) = conn
                        .query_row(
                            "SELECT name, color FROM pm_projects WHERE id = ?1",
                            params![pid],
                            |r| Ok((r.get(0)?, r.get(1)?)),
                        )
                        .unwrap_or_else(|_| ("".to_string(), "".to_string()));
                    proj_cache.insert(pid, info.clone());
                    info
                };
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("projectName".to_string(), json!(pname));
                    obj.insert("projectColor".to_string(), json!(pcolor));
                }
            }
            item
        })
        .collect();

    Ok(json!({
        "items": items,
        "total": total,
        "eligibleCount": total,
        "blockedCount": blocked_count,
        "reason": reason,
    }))
}

/// Return candidate Todos for linking by project ID (no PM item required).
/// Used when creating a new PM item and wanting to pre-select todos to link.
pub fn item_todo_candidates_by_project(payload: &Value) -> Result<Value, String> {
    let project_id =
        crate::tools::pm::parse_i64(payload, "projectId").ok_or("projectId is required")?;
    let keyword = crate::tools::pm::parse_string(payload, "keyword");
    let limit = crate::tools::pm::parse_i64(payload, "limit")
        .unwrap_or(50)
        .min(100) as usize;

    let conn = db_conn()?;

    // Query candidates: same project or null project, one_off, not already linked
    let mut sql = String::from(
        "SELECT t.id, t.title, t.status, t.priority, t.event_at, t.project_id
         FROM todo_items t
         WHERE t.kind = 'one_off'
           AND NOT EXISTS (
               SELECT 1 FROM pm_item_todo_links l WHERE l.todo_item_id = t.id
           )
           AND (t.project_id = ?1 OR t.project_id IS NULL)",
    );

    if keyword.is_some() {
        sql.push_str(" AND t.title LIKE '%' || ?2 || '%'");
    }

    sql.push_str(" ORDER BY CASE t.status WHEN 'completed' THEN 1 ELSE 0 END, t.event_at DESC");
    sql.push_str(" LIMIT ");
    sql.push_str(&(limit + 1).to_string());

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("item_todo_candidates_by_project prepare: {e}"))?;

    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(project_id)];
    if let Some(ref kw) = keyword {
        param_values.push(Box::new(kw.clone()));
    }
    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let rows: Vec<Value> = stmt
        .query_map(param_refs.as_slice(), |r| {
            let pid: Option<i64> = r.get(5)?;
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "title": r.get::<_, String>(1)?,
                "status": r.get::<_, String>(2)?,
                "priority": r.get::<_, String>(3)?,
                "eventAt": r.get::<_, Option<String>>(4)?,
                "projectId": pid,
                "isUnassignedProject": pid.is_none(),
            }))
        })
        .map_err(|e| format!("item_todo_candidates_by_project query: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let overflow = rows.len() > limit;
    let items: Vec<Value> = rows.into_iter().take(limit).collect();
    let total = items.len() as i64;

    let reason = if total == 0 {
        "empty"
    } else if overflow {
        "overflow"
    } else {
        "ok"
    };

    Ok(json!({
        "items": items,
        "total": total,
        "reason": reason,
    }))
}

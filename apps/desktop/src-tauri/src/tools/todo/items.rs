use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use crate::tools::action_center::{
    apply_todo_binding_patch, attach_todo_binding_summaries, delete_todo_binding,
    ensure_todo_can_become_recurring, parse_binding_patch, BindingPatch,
};
use crate::tools::helpers::db_conn;

use super::helpers::*;
use super::recurrence::*;
use super::reminders::*;
use super::types::*;

// ── DB helpers for items ──────────────────────────────────

pub(crate) fn sync_item_assignees(conn: &Connection, item_id: i64, ids: &[i64]) -> Result<(), String> {
    conn.execute(
        "DELETE FROM todo_item_assignees WHERE item_id=?1",
        params![item_id],
    )
    .map_err(|e| format!("清理事项执行人失败: {e}"))?;
    for id in ids {
        conn.execute(
            "INSERT OR IGNORE INTO todo_item_assignees(item_id, assignee_id) VALUES(?1, ?2)",
            params![item_id, id],
        )
        .map_err(|e| format!("保存事项执行人失败: {e}"))?;
    }
    Ok(())
}

pub(crate) fn load_item_assignees(conn: &Connection, item_id: i64) -> Result<Vec<Value>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT a.id, a.name
             FROM todo_item_assignees ia
             JOIN todo_assignees a ON a.id = ia.assignee_id
             WHERE ia.item_id = ?1
             ORDER BY a.name COLLATE NOCASE ASC",
        )
        .map_err(|e| format!("查询事项执行人失败: {e}"))?;
    let rows = stmt
        .query_map(params![item_id], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?
            }))
        })
        .map_err(|e| format!("映射事项执行人失败: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub(crate) fn load_item_links(conn: &Connection, item_id: i64) -> Result<Vec<Value>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, url, title FROM todo_item_links WHERE item_id=?1 ORDER BY sort_order ASC, id ASC",
        )
        .map_err(|e| format!("查询事项链接失败: {e}"))?;
    let rows = stmt
        .query_map(params![item_id], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "url": row.get::<_, String>(1)?,
                "title": row.get::<_, String>(2)?
            }))
        })
        .map_err(|e| format!("映射事项链接失败: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub(crate) fn sync_item_links(conn: &Connection, item_id: i64, links: &[Value]) -> Result<(), String> {
    conn.execute(
        "DELETE FROM todo_item_links WHERE item_id=?1",
        params![item_id],
    )
    .map_err(|e| format!("清空事项链接失败: {e}"))?;
    for (i, link) in links.iter().enumerate() {
        let url = link.get("url").and_then(Value::as_str).unwrap_or("").trim();
        if url.is_empty() {
            continue;
        }
        let title = link
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        conn.execute(
            "INSERT INTO todo_item_links(item_id, url, title, sort_order) VALUES(?1, ?2, ?3, ?4)",
            params![item_id, url, title, i as i64],
        )
        .map_err(|e| format!("插入事项链接失败: {e}"))?;
    }
    Ok(())
}

pub(crate) fn load_item_event_at(conn: &Connection, item_id: i64) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT event_at FROM todo_items WHERE id=?1",
        params![item_id],
        |row| row.get(0),
    )
    .map_err(|_| "事项不存在".to_string())
}

// ── item_list ─────────────────────────────────────────────

pub(crate) fn item_list(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    item_list_with_conn(&conn, payload)
}

pub(crate) fn item_list_with_conn(
    conn: &Connection,
    payload: &Value,
) -> Result<Value, String> {
    let status_filter = parse_string(payload, "status");
    let include_inactive = payload
        .get("includeInactive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let project_filter = parse_i64(payload, "projectId");
    let project_filter_mode = parse_string(payload, "projectFilter");
    // projectFilter: "all" | "none" | specific projectId

    // Detect whether project_id column exists
    let has_project_col = conn
        .prepare("SELECT project_id FROM todo_items LIMIT 0")
        .is_ok();

    let select_extra = if has_project_col {
        ", i.project_id, pm.name AS project_name, pm.color AS project_color"
    } else {
        ""
    };
    let join_extra = if has_project_col {
        " LEFT JOIN pm_projects pm ON pm.id = i.project_id"
    } else {
        ""
    };

    let sql = format!(
        "SELECT i.id, i.title, i.type_id, i.priority, i.description, i.status,
                i.event_at, i.pinned, i.kind, i.series_id, i.parent_id,
                i.created_at, i.updated_at, i.completed_at,
                ty.name AS type_name, ty.color AS type_color,
                sr.rule_mode, sr.rule_json, sr.cron_expression, sr.timezone,
                sr.start_at, sr.end_mode, sr.end_value, sr.occurrence_index, sr.active
                {select_extra}
         FROM todo_items i
         LEFT JOIN todo_types ty ON ty.id = i.type_id
         LEFT JOIN todo_series_rules sr ON sr.series_id = i.series_id
         {join_extra}
         ORDER BY i.id DESC"
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("查询事项失败: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            // Columns 0-24 are always present; columns 25+ depend on has_project_col
            let base: (i64, String, Option<i64>, String, String, String,
                       Option<String>, bool, String, Option<i64>, Option<i64>,
                       String, String, Option<String>,
                       Option<String>, Option<String>,
                       Option<String>, Option<String>, Option<String>, Option<String>,
                       Option<String>, Option<String>, Option<String>,
                       Option<i64>, Option<i64>) = (
                row.get(0)?,   // id
                row.get(1)?,   // title
                row.get(2)?,   // type_id
                row.get(3)?,   // priority
                row.get(4)?,   // description
                row.get(5)?,   // status
                row.get(6)?,   // event_at
                row.get::<_, i64>(7)? != 0,  // pinned
                row.get(8)?,   // kind
                row.get(9)?,   // series_id
                row.get(10)?,  // parent_id
                row.get(11)?,  // created_at
                row.get(12)?,  // updated_at
                row.get(13)?,  // completed_at
                row.get(14)?,  // type_name
                row.get(15)?,  // type_color
                row.get(16)?,  // rule_mode
                row.get(17)?,  // rule_json
                row.get(18)?,  // cron_expression
                row.get(19)?,  // timezone
                row.get(20)?,  // start_at
                row.get(21)?,  // end_mode
                row.get(22)?,  // end_value
                row.get(23)?,  // occurrence_index
                row.get(24)?,  // active
            );
            let project_id: Option<i64> = if has_project_col { row.get(25)? } else { None };
            let project_name: Option<String> = if has_project_col { row.get(26)? } else { None };
            let project_color: Option<String> = if has_project_col { row.get(27)? } else { None };
            Ok((base, project_id, project_name, project_color))
        })
        .map_err(|e| format!("映射事项失败: {e}"))?;

    let mut items = Vec::new();
    for row in rows {
        let (base, project_id_from_sql, project_name_from_sql, project_color_from_sql) = row.map_err(|e| e.to_string())?;
        let (
            id, title, type_id, priority, description, status_raw,
            event_at, pinned, kind, series_id, _parent_id,
            created_at, updated_at, completed_at,
            type_name, type_color,
            rule_mode, rule_json, cron_expression, timezone,
            start_at, end_mode, end_value, occurrence_index, rule_active,
        ) = base;

        // A1 归一化
        let status = normalize_status_a1(&status_raw).to_string();
        let rule_active_bool = rule_active.map(|v| v == 1).unwrap_or(true);

        // includeInactive 过滤：非活跃系列的已完成项隐藏，open 项始终显示
        if !include_inactive
            && kind == SERIES_KIND_RECURRING
            && !rule_active_bool
            && status_raw == STATUS_COMPLETED
        {
            continue;
        }

        // status 过滤（A1 归一化后比较）
        if let Some(ref filter) = status_filter {
            let normalized_filter = normalize_status_a1(filter);
            if status != normalized_filter {
                continue;
            }
        }

        let created_at_fmt = format_db_datetime(&created_at);
        let updated_at_fmt = format_db_datetime(&updated_at);
        let completed_at_fmt = completed_at.as_deref().map(format_db_datetime);
        let display_at = event_at.clone().unwrap_or_else(|| created_at_fmt.clone());

        // isOverdue
        let is_overdue = is_open_status(&status_raw)
            && event_at
                .as_deref()
                .and_then(parse_rfc3339)
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc) < Utc::now())
                .unwrap_or(false);

        // Build recurrence object
        let recurrence = if kind == SERIES_KIND_RECURRING && rule_mode.is_some() {
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

        let mut item = json!({
            "id": id,
            "title": title,
            "typeId": type_id,
            "typeName": type_name,
            "typeColor": type_color,
            "priority": priority,
            "description": description,
            "kind": kind,
            "rootId": series_id.unwrap_or(id),
            "pinned": pinned,
            "status": status,
            "eventAt": event_at,
            "displayAt": display_at,
            "recurrence": recurrence,
            "isOverdue": is_overdue,
            "createdAt": created_at_fmt,
            "updatedAt": updated_at_fmt,
            "completedAt": completed_at_fmt,
        });

        // Load support data
        let assignees = load_item_assignees(&conn, id)?;
        let links = load_item_links(&conn, id)?;
        let reminder_summary = load_item_reminder_summary(&conn, id)?;

        if let Some(obj) = item.as_object_mut() {
            obj.insert("assignees".to_string(), json!(assignees));
            obj.insert("links".to_string(), json!(links));
            obj.insert(
                "reminderPresets".to_string(),
                json!(reminder_summary.reminder_presets),
            );
            obj.insert(
                "snoozeUntil".to_string(),
                json!(reminder_summary.snooze_until),
            );
            obj.insert(
                "lastNotifiedAt".to_string(),
                json!(reminder_summary.last_notified_at),
            );
            obj.insert(
                "nextTaskReminderId".to_string(),
                json!(reminder_summary.next_task_reminder_id),
            );
            obj.insert(
                "nextReminderPreset".to_string(),
                json!(reminder_summary.next_reminder_preset),
            );
            // Project info from SQL join (already fetched)
            if has_project_col {
                obj.insert("projectId".to_string(), json!(project_id_from_sql));
                if project_id_from_sql.is_some() {
                    obj.insert("projectName".to_string(), json!(project_name_from_sql.unwrap_or_default()));
                    obj.insert("projectColor".to_string(), json!(project_color_from_sql.unwrap_or_default()));
                } else {
                    obj.insert("projectName".to_string(), Value::Null);
                    obj.insert("projectColor".to_string(), Value::Null);
                }
            } else {
                obj.insert("projectId".to_string(), Value::Null);
                obj.insert("projectName".to_string(), Value::Null);
                obj.insert("projectColor".to_string(), Value::Null);
            }
        }

        items.push(item);
    }

    // Apply project filter before PM enrichment to avoid unnecessary work
    if has_project_col {
        if let Some(pid) = project_filter {
            items.retain(|item| item["projectId"].as_i64() == Some(pid));
        } else if project_filter_mode.as_deref() == Some("none") {
            items.retain(|item| item["projectId"].is_null());
        }
    }

    // Project info already set inline per-item above

    // Attach PM link info (pmItemId, pmItemTitle, pmItemProjectId)
    {
        let item_ids: Vec<i64> = items.iter().filter_map(|i| i["id"].as_i64()).collect();
        if !item_ids.is_empty() {
            // Batch query PM links for all items
            let placeholders: Vec<String> = item_ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
            let sql = format!(
                "SELECT l.todo_item_id, l.pm_item_id, p.title, p.project_id, p.status \
                 FROM pm_item_todo_links l \
                 JOIN pm_items p ON p.id = l.pm_item_id \
                 WHERE l.todo_item_id IN ({})",
                placeholders.join(",")
            );
            let params: Vec<&dyn rusqlite::ToSql> = item_ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
            let mut stmt = conn.prepare(&sql).map_err(|e| format!("查询 PM 关联失败: {e}"))?;
            let link_rows: Vec<(i64, i64, String, i64, String)> = stmt
                .query_map(params.as_slice(), |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
                })
                .map_err(|e| format!("映射 PM 关联失败: {e}"))?
                .filter_map(|r| r.ok())
                .collect();

            let link_map: std::collections::HashMap<i64, (i64, String, i64, String)> = link_rows
                .into_iter()
                .map(|(todo_id, pm_id, pm_title, pm_proj_id, pm_status)| (todo_id, (pm_id, pm_title, pm_proj_id, pm_status)))
                .collect();

            for item in items.iter_mut() {
                let item_id = item["id"].as_i64().unwrap_or(0);
                if let Some(obj) = item.as_object_mut() {
                    if let Some((pm_id, pm_title, pm_proj_id, pm_status)) = link_map.get(&item_id) {
                        obj.insert("pmItemId".to_string(), json!(pm_id));
                        obj.insert("pmItemTitle".to_string(), json!(pm_title));
                        obj.insert("pmItemProjectId".to_string(), json!(pm_proj_id));
                        obj.insert("pmItemStatus".to_string(), json!(pm_status));
                    } else {
                        obj.insert("pmItemId".to_string(), Value::Null);
                        obj.insert("pmItemTitle".to_string(), Value::Null);
                        obj.insert("pmItemProjectId".to_string(), Value::Null);
                        obj.insert("pmItemStatus".to_string(), Value::Null);
                    }
                }
            }
        } else {
            for item in items.iter_mut() {
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("pmItemId".to_string(), Value::Null);
                    obj.insert("pmItemTitle".to_string(), Value::Null);
                    obj.insert("pmItemProjectId".to_string(), Value::Null);
                    obj.insert("pmItemStatus".to_string(), Value::Null);
                }
            }
        }
    }

    attach_todo_binding_summaries(conn, &mut items)?;
    sort_item_rows(&mut items);
    Ok(json!({ "items": items }))
}

// ── item_create ───────────────────────────────────────────

pub(crate) fn item_create(payload: &Value) -> Result<Value, String> {
    let mut conn = db_conn()?;
    item_create_with_conn(&mut conn, payload)
}

pub(crate) fn item_create_with_conn(
    conn: &mut Connection,
    payload: &Value,
) -> Result<Value, String> {
    let binding_patch = parse_binding_patch(payload)?;
    let title = parse_string(payload, "title").ok_or("标题不能为空")?;
    let type_id = parse_i64(payload, "typeId");
    let priority = normalize_priority(payload.get("priority").and_then(Value::as_str))?;
    let description = payload
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let kind = parse_item_kind(payload);
    let assignee_ids = parse_assignee_ids(payload);
    let reminder_presets = parse_reminder_presets(payload)?.unwrap_or_default();

    if kind == SERIES_KIND_RECURRING && matches!(binding_patch, BindingPatch::Set { .. }) {
        return Err("周期事项暂不支持执行动作".into());
    }

    let tx = conn
        .transaction()
        .map_err(|error| format!("开启事务失败: {error}"))?;
    let item_id = if kind == SERIES_KIND_RECURRING {
        // Recurring: create item + series rule
        let recurrence = recurrence_payload(payload);
        let rule_mode = normalize_rule_mode(payload, "simple");
        let rule = recurrence.get("rule").cloned().unwrap_or_else(|| json!({}));
        let cron_expression = resolve_cron_expression(&rule_mode, &rule)?;
        let start_at = parse_start_datetime(payload)?.ok_or("周期事项开始时间不能为空")?;
        let timezone = recurrence
            .get("timezone")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| "local".to_string());
        let (end_mode, end_value) = parse_end_rule(payload)?;

        let event_at = start_at.clone();

        // Insert item (series_id=NULL temporarily)
        tx.execute(
            "INSERT INTO todo_items(title, type_id, priority, description, kind, status, event_at, pinned)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
            params![title, type_id, priority, description, SERIES_KIND_RECURRING, STATUS_PENDING, event_at],
        )
        .map_err(|e| format!("创建事项失败: {e}"))?;
        let item_id = tx.last_insert_rowid();

        // Set series_id = self id
        tx.execute(
            "UPDATE todo_items SET series_id=?1 WHERE id=?1",
            params![item_id],
        )
        .map_err(|e| format!("设置系列ID失败: {e}"))?;

        // Insert series rule
        tx.execute(
            "INSERT INTO todo_series_rules(series_id, rule_mode, rule_json, cron_expression, timezone, start_at, end_mode, end_value, occurrence_index, active)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, 1)",
            params![
                item_id,
                rule_mode,
                serde_json::to_string(&rule).map_err(|e| format!("规则序列化失败: {e}"))?,
                cron_expression,
                timezone,
                start_at,
                end_mode,
                end_value,
            ],
        )
        .map_err(|e| format!("创建系列规则失败: {e}"))?;

        sync_item_assignees(&tx, item_id, &assignee_ids)?;
        sync_item_reminders(&tx, item_id, Some(&event_at), &reminder_presets)?;
        if let Some(links) = parse_links(payload) {
            sync_item_links(&tx, item_id, &links)?;
        }

        // Set project_id if provided
        if let Some(project_id) = parse_i64(payload, "projectId") {
            tx.execute(
                "UPDATE todo_items SET project_id = ?1 WHERE id = ?2",
                params![project_id, item_id],
            )
            .map_err(|error| format!("更新事项项目失败: {error}"))?;
        }
        item_id
    } else {
        // One-off
        let event_at = parse_event_datetime(payload, "eventAt")?;
        if event_at.is_none() && !reminder_presets.is_empty() {
            // Allow creating without time if no reminders (or reminders are empty)
            let has_real_presets = reminder_presets.iter().any(|p| p != REMINDER_PRESET_NONE);
            if has_real_presets {
                return Err("设置提醒前需要先提供事件时间或周期规则".to_string());
            }
        }

        tx.execute(
            "INSERT INTO todo_items(title, type_id, priority, description, kind, status, event_at, pinned)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
            params![title, type_id, priority, description, SERIES_KIND_ONE_OFF, STATUS_PENDING, event_at],
        )
        .map_err(|e| format!("创建事项失败: {e}"))?;
        let id = tx.last_insert_rowid();

        sync_item_assignees(&tx, id, &assignee_ids)?;
        if event_at.is_some() {
            sync_item_reminders(&tx, id, event_at.as_deref(), &reminder_presets)?;
        }
        if let Some(links) = parse_links(payload) {
            sync_item_links(&tx, id, &links)?;
        }

        // Set project_id if provided
        if let Some(project_id) = parse_i64(payload, "projectId") {
            tx.execute(
                "UPDATE todo_items SET project_id = ?1 WHERE id = ?2",
                params![project_id, id],
            )
            .map_err(|error| format!("更新事项项目失败: {error}"))?;
        }
        id
    };

    apply_todo_binding_patch(&tx, item_id, &kind, binding_patch, true)?;
    tx.commit()
        .map_err(|error| format!("提交事务失败: {error}"))?;
    Ok(json!({ "ok": true, "id": item_id, "rootId": item_id }))
}

// ── item_update ───────────────────────────────────────────

pub(crate) fn item_update(payload: &Value) -> Result<Value, String> {
    let mut conn = db_conn()?;
    item_update_with_conn(&mut conn, payload)
}

pub(crate) fn item_update_with_conn(
    database: &mut Connection,
    payload: &Value,
) -> Result<Value, String> {
    let binding_patch = parse_binding_patch(payload)?;
    let id = parse_i64(payload, "id").ok_or("缺少事项 id")?;
    let tx = database
        .transaction()
        .map_err(|error| format!("开启事务失败: {error}"))?;
    let conn: &Connection = &tx;

    // Verify item exists and get kind + series_id
    let (kind, mut series_id): (String, Option<i64>) = conn
        .query_row(
            "SELECT kind, series_id FROM todo_items WHERE id=?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| "事项不存在".to_string())?;

    // Detect kind change
    let new_kind = parse_item_kind(payload);
    if kind == SERIES_KIND_ONE_OFF && new_kind == SERIES_KIND_RECURRING {
        ensure_todo_can_become_recurring(conn, id, &binding_patch)?;
    }
    if kind == SERIES_KIND_RECURRING && new_kind == SERIES_KIND_ONE_OFF {
        // recurring -> one_off: delete series rule, clear series_id and parent_id
        if let Some(sid) = series_id {
            conn.execute(
                "DELETE FROM todo_series_rules WHERE series_id=?1",
                params![sid],
            )
            .map_err(|e| format!("删除系列规则失败: {e}"))?;
        }
        conn.execute(
            "UPDATE todo_items SET kind=?1, series_id=NULL, parent_id=NULL, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![SERIES_KIND_ONE_OFF, id],
        )
        .map_err(|e| format!("更新事项类型失败: {e}"))?;
    } else if kind == SERIES_KIND_ONE_OFF && new_kind == SERIES_KIND_RECURRING {
        // Guard: block one_off -> recurring when PM linked
        let link_info: Option<(i64, String, i64)> = conn
            .query_row(
                "SELECT l.pm_item_id, p.title, p.project_id \
                 FROM pm_item_todo_links l \
                 JOIN pm_items p ON p.id = l.pm_item_id \
                 WHERE l.todo_item_id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(|e| format!("查询 PM 关联失败: {e}"))?;
        if let Some((_, pm_title, pm_proj_id)) = link_info {
            return Err(format!(
                "该任务已关联工作项「{}」（归属项目 #{}），不能改为重复事项，请先解除关联",
                pm_title, pm_proj_id
            ));
        }
        // one_off -> recurring: set series_id = self, update kind
        conn.execute(
            "UPDATE todo_items SET kind=?1, series_id=?2, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![SERIES_KIND_RECURRING, id],
        )
        .map_err(|e| format!("更新事项类型失败: {e}"))?;
        series_id = Some(id);
    }

    // Update base fields
    if let Some(title) = parse_string(payload, "title") {
        conn.execute(
            "UPDATE todo_items SET title=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![title, id],
        )
        .map_err(|e| format!("更新事项标题失败: {e}"))?;
    }
    if payload.get("typeId").is_some() {
        conn.execute(
            "UPDATE todo_items SET type_id=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![parse_i64(payload, "typeId"), id],
        )
        .map_err(|e| format!("更新事项类型失败: {e}"))?;
    }
    if payload.get("priority").is_some() {
        let priority = normalize_priority(payload.get("priority").and_then(Value::as_str))?;
        conn.execute(
            "UPDATE todo_items SET priority=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![priority, id],
        )
        .map_err(|e| format!("更新事项优先级失败: {e}"))?;
    }
    if payload.get("description").is_some() {
        let description = payload
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        conn.execute(
            "UPDATE todo_items SET description=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![description, id],
        )
        .map_err(|e| format!("更新事项描述失败: {e}"))?;
    }
    if let Some(links) = parse_links(payload) {
        sync_item_links(&conn, id, &links)?;
    }
    if payload.get("assigneeIds").is_some() {
        sync_item_assignees(&conn, id, &parse_assignee_ids(payload))?;
    }

    // Update project_id if provided
    if payload.get("projectId").is_some() {
        let new_project_id = parse_i64(payload, "projectId");
        let current_project_id = conn
            .query_row(
                "SELECT project_id FROM todo_items WHERE id = ?1",
                params![id],
                |r| r.get::<_, Option<i64>>(0),
            )
            .optional()
            .map_err(|e| format!("查询事项项目失败: {e}"))?
            .flatten();
        // Only block if actually changing to a different project while linked to PM item
        if new_project_id != current_project_id {
            let link_info: Option<(i64, String, i64)> = conn
                .query_row(
                    "SELECT l.pm_item_id, p.title, p.project_id \
                     FROM pm_item_todo_links l \
                     JOIN pm_items p ON p.id = l.pm_item_id \
                     WHERE l.todo_item_id = ?1",
                    params![id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()
                .map_err(|e| format!("查询 PM 关联失败: {e}"))?;
            if let Some((_, pm_title, pm_proj_id)) = link_info {
                return Err(format!(
                    "该任务已关联工作项「{}」（归属项目 #{}），切换项目前请先解除关联",
                    pm_title, pm_proj_id
                ));
            }
        }
        conn.execute(
            "UPDATE todo_items SET project_id = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![new_project_id, id],
        )
        .map_err(|error| format!("更新事项项目失败: {error}"))?;
    }

    // Update event_at and reminders
    let reminder_presets_update = parse_reminder_presets(payload)?;
    let has_recurrence_start = new_kind == SERIES_KIND_RECURRING && has_start_datetime(payload);
    if payload.get("eventAt").is_some() || reminder_presets_update.is_some() || has_recurrence_start
    {
        let current_event_at = load_item_event_at(&conn, id)?;
        let next_event_at = if payload.get("eventAt").is_some() {
            parse_event_datetime(payload, "eventAt")?
        } else if has_recurrence_start {
            parse_start_datetime(payload)?
        } else {
            current_event_at
        };
        let next_reminder_presets = if let Some(rp) = reminder_presets_update {
            rp
        } else {
            load_item_reminder_configs(&conn, id)?
                .into_iter()
                .map(|c| c.preset)
                .collect()
        };
        if next_event_at.is_none() && !next_reminder_presets.is_empty() {
            let has_real = next_reminder_presets
                .iter()
                .any(|p| p != REMINDER_PRESET_NONE);
            if has_real {
                return Err("设置提醒前需要先提供事件时间或周期规则".to_string());
            }
        }
        conn.execute(
            "UPDATE todo_items
             SET event_at=?1,
                 remind_at=NULL,
                 snooze_until=NULL,
                 last_notified_at=NULL,
                 updated_at=CURRENT_TIMESTAMP
             WHERE id=?2",
            params![next_event_at, id],
        )
        .map_err(|e| format!("更新事项时间失败: {e}"))?;
        sync_item_reminders(&conn, id, next_event_at.as_deref(), &next_reminder_presets)?;
    }

    // Update recurrence rules (if recurring and recurrence payload provided)
    if new_kind == SERIES_KIND_RECURRING && payload.get("recurrence").is_some() {
        if let Some(sid) = series_id {
            // Check item is open
            let status: String = conn
                .query_row(
                    "SELECT status FROM todo_items WHERE id=?1",
                    params![id],
                    |row| row.get(0),
                )
                .map_err(|_| "事项不存在".to_string())?;
            if !is_open_status(&status) {
                return Err("已结束的事项不可修改重复规则".to_string());
            }

            let recurrence = recurrence_payload(payload);
            let current_rule = load_series_rule(&conn, sid)?;
            let current_rule_mode = current_rule
                .as_ref()
                .map(|r| r.rule_mode.as_str())
                .unwrap_or("simple");
            let current_rule_json_str = current_rule
                .as_ref()
                .map(|r| r.rule_json.as_str())
                .unwrap_or("{}");

            let rule_mode = normalize_rule_mode(payload, current_rule_mode);
            let rule = recurrence.get("rule").cloned().unwrap_or_else(|| {
                serde_json::from_str::<Value>(current_rule_json_str).unwrap_or_else(|_| json!({}))
            });
            let cron_expression = resolve_cron_expression(&rule_mode, &rule)?;
            let timezone = recurrence
                .get("timezone")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(|| {
                    current_rule
                        .as_ref()
                        .map(|r| r.timezone.clone())
                        .unwrap_or_else(|| "local".to_string())
                });
            let start_at = if has_start_datetime(payload) {
                parse_start_datetime(payload)?
            } else {
                current_rule.as_ref().and_then(|r| r.start_at.clone())
            };
            let (end_mode, end_value) = if recurrence.get("endMode").is_some() {
                parse_end_rule(payload)?
            } else {
                let cr = current_rule.as_ref();
                (
                    cr.map(|r| r.end_mode.clone())
                        .unwrap_or_else(|| "never".to_string()),
                    cr.and_then(|r| r.end_value.clone()),
                )
            };

            // Upsert series rule
            conn.execute(
                "INSERT INTO todo_series_rules(series_id, rule_mode, rule_json, cron_expression, timezone, start_at, end_mode, end_value, occurrence_index, active)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, COALESCE((SELECT occurrence_index FROM todo_series_rules WHERE series_id=?1), 1), COALESCE((SELECT active FROM todo_series_rules WHERE series_id=?1), 1))
                 ON CONFLICT(series_id) DO UPDATE SET
                   rule_mode=excluded.rule_mode, rule_json=excluded.rule_json,
                   cron_expression=excluded.cron_expression, timezone=excluded.timezone,
                   start_at=excluded.start_at, end_mode=excluded.end_mode, end_value=excluded.end_value,
                   updated_at=CURRENT_TIMESTAMP",
                params![
                    sid,
                    rule_mode,
                    serde_json::to_string(&rule).map_err(|e| format!("规则序列化失败: {e}"))?,
                    cron_expression,
                    timezone,
                    start_at,
                    end_mode,
                    end_value,
                ],
            )
            .map_err(|e| format!("更新系列规则失败: {e}"))?;
        }
    }

    apply_todo_binding_patch(conn, id, &new_kind, binding_patch, false)?;
    tx.commit()
        .map_err(|error| format!("提交事务失败: {error}"))?;
    Ok(json!({ "ok": true }))
}

pub(crate) fn item_upsert(payload: &Value) -> Result<Value, String> {
    if parse_i64(payload, "id").is_some() {
        item_update(payload)
    } else {
        item_create(payload)
    }
}

// ── item_change_status ────────────────────────────────────

pub(crate) fn item_change_status(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缺少事项 id")?;
    let next = normalize_status(
        payload
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or(STATUS_PENDING),
    )?;

    let conn = db_conn()?;
    let (current, kind, series_id): (String, String, Option<i64>) = conn
        .query_row(
            "SELECT status, kind, series_id FROM todo_items WHERE id=?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| "事项不存在".to_string())?;

    if !can_transit_for_kind(&current, &next, &kind) {
        return Err(format!("不允许的状态变更: {} -> {}", current, next));
    }

    conn.execute(
        "UPDATE todo_items
         SET status=?1,
             completed_at=CASE
                 WHEN ?1 = 'completed' THEN COALESCE(completed_at, CURRENT_TIMESTAMP)
                 ELSE NULL
             END,
             snooze_until=CASE WHEN ?1 IN ('completed') THEN NULL ELSE snooze_until END,
             updated_at=CURRENT_TIMESTAMP
         WHERE id=?2",
        params![next, id],
    )
    .map_err(|e| format!("更新事项状态失败: {e}"))?;

    if next == STATUS_COMPLETED {
        clear_item_reminder_snooze(&conn, id)?;
        mark_item_reminder_events_read(&conn, id)?;
    }

    // Recurring + completed → generate next
    let mut next_item_id = None;
    if next == STATUS_COMPLETED && kind == SERIES_KIND_RECURRING {
        if let Some(sid) = series_id {
            next_item_id = generate_next_item(&conn, sid, id, true)?;
        }
    }

    if let Some(nid) = next_item_id {
        Ok(json!({ "ok": true, "nextItemId": nid }))
    } else {
        Ok(json!({ "ok": true }))
    }
}

// ── item_delete ───────────────────────────────────────────

pub(crate) fn item_delete(payload: &Value) -> Result<Value, String> {
    let mut database = db_conn()?;
    let tx = database
        .transaction()
        .map_err(|error| format!("开启事务失败: {error}"))?;
    let result = item_delete_with_conn(&tx, payload)?;
    tx.commit()
        .map_err(|error| format!("提交事务失败: {error}"))?;
    Ok(result)
}

pub(crate) fn item_delete_with_conn(
    conn: &Connection,
    payload: &Value,
) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缺少事项 id")?;
    let scope = parse_scope(payload);

    let item = conn
        .query_row(
            "SELECT kind, series_id, status FROM todo_items WHERE id=?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("查询事项失败: {e}"))?
        .ok_or("事项不存在")?;
    let (kind, series_id, status) = item;
    delete_todo_binding(conn, id)?;

    if scope == SCOPE_FUTURE_INSTANCES && kind == SERIES_KIND_RECURRING {
        // 暂停规则 + 删除当前项
        if let Some(sid) = series_id {
            conn.execute(
                "UPDATE todo_series_rules SET active=0, updated_at=CURRENT_TIMESTAMP WHERE series_id=?1",
                params![sid],
            )
            .map_err(|e| format!("暂停系列规则失败: {e}"))?;
        }
        delete_item_by_id(&conn, id)?;
        return Ok(json!({ "ok": true }));
    }

    // this_instance 删除
    if kind == SERIES_KIND_RECURRING && is_open_status(&status) {
        if let Some(sid) = series_id {
            // 缓存支撑表数据（DELETE 后级联清理会丢失）
            let cached_reminder_presets: Vec<String> = load_item_reminder_configs(&conn, id)?
                .into_iter()
                .map(|c| c.preset)
                .collect();
            let cached_assignee_ids: Vec<i64> = conn
                .prepare("SELECT assignee_id FROM todo_item_assignees WHERE item_id=?1")
                .map_err(|e| format!("查询执行人失败: {e}"))?
                .query_map(params![id], |row| row.get::<_, i64>(0))
                .map_err(|e| format!("映射执行人失败: {e}"))?
                .filter_map(|r| r.ok())
                .collect();
            let cached_links: Vec<Value> = load_item_links(&conn, id)?;
            let cached_event_at = load_item_event_at(&conn, id).ok().flatten();

            let has_other_open = has_other_open_in_series(&conn, sid, id)?;

            // 删除该项
            delete_item_by_id(&conn, id)?;

            // 无其它 open 时补生成
            if !has_other_open {
                if let Some(rule) = load_series_rule(&conn, sid)? {
                    if rule.active {
                        // 计算 base_time
                        let now = Utc::now();
                        let event_at_dt = cached_event_at.as_deref().and_then(parse_utc_datetime);
                        let base_time = event_at_dt
                            .map(|dt| if dt > now { dt } else { now })
                            .unwrap_or(now);

                        let next_occurrence = compute_next_occurrence_with_start(
                            &rule.cron_expression,
                            &rule.timezone,
                            rule.start_at.as_deref(),
                            base_time,
                        )?;

                        if let Some(next_dt) = next_occurrence {
                            if !should_stop_series(&rule, next_dt) {
                                // 找一个同系列的 item 来复制基础字段
                                let template_item_id: Option<i64> = conn
                                    .query_row(
                                        "SELECT id FROM todo_items WHERE series_id=?1 ORDER BY id DESC LIMIT 1",
                                        params![sid],
                                        |row| row.get(0),
                                    )
                                    .optional()
                                    .map_err(|e| format!("查询系列事项失败: {e}"))?;

                                if let Some(tmpl_id) = template_item_id {
                                    let (title, type_id, priority, description): (String, Option<i64>, String, String) = conn
                                        .query_row(
                                            "SELECT title, type_id, priority, description FROM todo_items WHERE id=?1",
                                            params![tmpl_id],
                                            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                                        )
                                        .map_err(|e| format!("读取模板事项失败: {e}"))?;

                                    let next_event_at = next_dt.to_rfc3339();
                                    conn.execute(
                                        "INSERT INTO todo_items(title, type_id, priority, description, kind, series_id, parent_id, status, event_at, pinned)
                                         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0)",
                                        params![title, type_id, priority, description, SERIES_KIND_RECURRING, sid, tmpl_id, STATUS_PENDING, next_event_at],
                                    )
                                    .map_err(|e| format!("补生成事项失败: {e}"))?;
                                    let new_id = conn.last_insert_rowid();

                                    // Inherit project_id from template item
                                    conn.execute(
                                        "UPDATE todo_items SET project_id = (SELECT project_id FROM todo_items WHERE id = ?1) WHERE id = ?2",
                                        params![tmpl_id, new_id],
                                    )
                                    .map_err(|error| format!("继承事项项目失败: {error}"))?;

                                    // 使用缓存的支撑表数据
                                    sync_item_assignees(&conn, new_id, &cached_assignee_ids)?;
                                    sync_item_reminders(
                                        &conn,
                                        new_id,
                                        Some(&next_event_at),
                                        &cached_reminder_presets,
                                    )?;
                                    sync_item_links(&conn, new_id, &cached_links)?;
                                    // occurrence_index 不递增（删除不算完成）
                                }
                            }
                        }
                    }
                }
            }

            return Ok(json!({ "ok": true }));
        }
    }

    // 普通删除（one_off 或 recurring+done）
    delete_item_by_id(&conn, id)?;
    Ok(json!({ "ok": true }))
}

pub(crate) fn delete_item_by_id(conn: &Connection, item_id: i64) -> Result<(), String> {
    conn.execute(
        "DELETE FROM todo_item_assignees WHERE item_id=?1",
        params![item_id],
    )
    .map_err(|e| format!("删除事项执行人失败: {e}"))?;
    conn.execute(
        "DELETE FROM todo_item_reminders WHERE item_id=?1",
        params![item_id],
    )
    .map_err(|e| format!("删除事项提醒失败: {e}"))?;
    conn.execute(
        "DELETE FROM todo_item_links WHERE item_id=?1",
        params![item_id],
    )
    .map_err(|e| format!("删除事项链接失败: {e}"))?;
    conn.execute(
        "DELETE FROM todo_reminder_events WHERE task_id=?1",
        params![item_id],
    )
    .map_err(|e| format!("删除提醒事件失败: {e}"))?;
    // PM-Todo link cleanup (FK CASCADE also handles this, but explicit for clarity)
    conn.execute(
        "DELETE FROM pm_item_todo_links WHERE todo_item_id=?1",
        params![item_id],
    )
    .map_err(|e| format!("删除 PM 关联记录失败: {e}"))?;
    conn.execute("DELETE FROM todo_items WHERE id=?1", params![item_id])
        .map_err(|e| format!("删除事项失败: {e}"))?;
    // 事项描述可能持有富文本附件，按 owner 汇点清理
    crate::tools::attachments::delete_by_owner_internal(conn, "todo", &item_id.to_string())?;
    Ok(())
}

// ── item_toggle_pin / item_snooze / item_toggle_active ────

pub(crate) fn item_toggle_pin(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缺少事项 id")?;
    let conn = db_conn()?;
    let changed = conn
        .execute(
            "UPDATE todo_items
             SET pinned=CASE WHEN pinned = 0 THEN 1 ELSE 0 END,
                 updated_at=CURRENT_TIMESTAMP
             WHERE id=?1",
            params![id],
        )
        .map_err(|e| format!("切换事项置顶失败: {e}"))?;
    if changed == 0 {
        return Err("事项不存在".to_string());
    }
    Ok(json!({ "ok": true }))
}

pub(crate) fn item_snooze(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缺少事项 id")?;
    let minutes = parse_i64(payload, "minutes")
        .unwrap_or(10)
        .clamp(1, 24 * 60);
    let conn = db_conn()?;
    let status: String = conn
        .query_row(
            "SELECT status FROM todo_items WHERE id=?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|_| "事项不存在".to_string())?;
    if status == STATUS_COMPLETED {
        return Err("已完成事项不能稍后提醒".to_string());
    }
    let item_reminder_id =
        resolve_item_reminder_id_for_snooze(&conn, id, parse_i64(payload, "taskReminderId"))?;
    let snooze_until = (Utc::now() + Duration::minutes(minutes)).to_rfc3339();
    conn.execute(
        "UPDATE todo_item_reminders
         SET snooze_until=?1, last_notified_at=NULL, updated_at=CURRENT_TIMESTAMP
         WHERE id=?2 AND item_id=?3",
        params![snooze_until, item_reminder_id, id],
    )
    .map_err(|e| format!("稍后提醒失败: {e}"))?;
    Ok(json!({ "ok": true, "snoozeUntil": snooze_until, "taskReminderId": item_reminder_id }))
}

pub(crate) fn item_toggle_active(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缺少事项 id")?;
    let conn = db_conn()?;

    let (kind, series_id): (String, Option<i64>) = conn
        .query_row(
            "SELECT kind, series_id FROM todo_items WHERE id=?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| "事项不存在".to_string())?;

    if kind != SERIES_KIND_RECURRING {
        return Err("仅周期事项支持启停".to_string());
    }

    let sid = series_id.ok_or("周期事项缺少系列ID")?;

    let changed = conn
        .execute(
            "UPDATE todo_series_rules SET active=1-active, updated_at=CURRENT_TIMESTAMP WHERE series_id=?1",
            params![sid],
        )
        .map_err(|e| format!("切换系列状态失败: {e}"))?;

    if changed == 0 {
        return Err("系列规则不存在".to_string());
    }

    let new_active: bool = conn
        .query_row(
            "SELECT active FROM todo_series_rules WHERE series_id=?1",
            params![sid],
            |row| Ok(row.get::<_, i64>(0)? == 1),
        )
        .map_err(|e| format!("查询系列状态失败: {e}"))?;

    Ok(json!({ "ok": true, "active": new_active }))
}

pub(crate) fn open_link(payload: &Value) -> Result<Value, String> {
    let url = payload["url"].as_str().ok_or("url 不能为空")?.trim();
    if url.is_empty() {
        return Err("url 不能为空".to_string());
    }
    open::that(url).map_err(|e| format!("打开链接失败: {e}"))?;
    Ok(json!({ "ok": true }))
}

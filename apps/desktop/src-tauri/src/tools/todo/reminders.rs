use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use crate::tools::helpers::db_conn;

use super::helpers::*;
use super::types::*;

// ── Reminder utilities ────────────────────────────────────

pub fn reminder_offset_minutes_from_preset(preset: &str) -> Option<i64> {
    REMINDER_PRESET_OFFSETS
        .iter()
        .find_map(|(candidate, minutes)| (*candidate == preset).then_some(*minutes))
}

pub(crate) fn reminder_preset_sort_key(preset: &str) -> usize {
    match preset {
        REMINDER_PRESET_ON_TIME => 0,
        REMINDER_PRESET_5M => 1,
        REMINDER_PRESET_10M => 2,
        REMINDER_PRESET_30M => 3,
        REMINDER_PRESET_1H => 4,
        REMINDER_PRESET_1D => 5,
        REMINDER_PRESET_2D => 6,
        REMINDER_PRESET_NONE => 999,
        _ => 1000,
    }
}

pub(crate) fn normalize_reminder_preset(value: &str) -> Option<String> {
    let normalized = value.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }
    if normalized == REMINDER_PRESET_NONE {
        return Some(REMINDER_PRESET_NONE.to_string());
    }
    reminder_offset_minutes_from_preset(&normalized).map(|_| normalized)
}

pub(crate) fn sort_reminder_presets(presets: &mut Vec<String>) {
    presets.sort_by_key(|preset| reminder_preset_sort_key(preset));
}

pub(crate) fn normalize_reminder_presets(values: &[String]) -> Result<Vec<String>, String> {
    let mut presets = Vec::new();
    let mut has_none = false;

    for value in values {
        let normalized =
            normalize_reminder_preset(value).ok_or_else(|| "提醒方式不支持该预设值".to_string())?;
        if normalized == REMINDER_PRESET_NONE {
            has_none = true;
            continue;
        }
        if !presets.contains(&normalized) {
            presets.push(normalized);
        }
    }

    sort_reminder_presets(&mut presets);
    if has_none && presets.is_empty() {
        return Ok(Vec::new());
    }
    Ok(presets)
}

pub(crate) fn parse_reminder_presets(payload: &Value) -> Result<Option<Vec<String>>, String> {
    let Some(raw) = payload
        .get("reminderPresets")
        .or_else(|| payload.get("reminderPreset"))
    else {
        return Ok(None);
    };

    if raw.is_null() {
        return Ok(Some(Vec::new()));
    }

    let values = match raw {
        Value::String(value) => vec![value.to_string()],
        Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(ToString::to_string)
                    .ok_or_else(|| "提醒方式格式不正确".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err("提醒方式格式不正确".to_string()),
    };

    Ok(Some(normalize_reminder_presets(&values)?))
}

pub fn reminder_configs_from_presets(presets: &[String]) -> Vec<ReminderConfig> {
    let mut configs = presets
        .iter()
        .filter_map(|preset| {
            reminder_offset_minutes_from_preset(preset).map(|offset_minutes| ReminderConfig {
                preset: preset.clone(),
                offset_minutes,
            })
        })
        .collect::<Vec<_>>();
    configs.sort_by_key(|config| config.offset_minutes);
    configs
}

pub fn compute_remind_at(
    event_at: Option<&str>,
    offset_minutes: Option<i64>,
) -> Result<Option<String>, String> {
    match (event_at, offset_minutes) {
        (_, None) => Ok(None),
        (None, Some(_)) => Err("设置提醒前需要先提供事件时间或周期规则".to_string()),
        (Some(event_at), Some(offset_minutes)) => {
            let event_at = DateTime::parse_from_rfc3339(event_at)
                .map_err(|_| "事件时间格式不正确".to_string())?
                .with_timezone(&Utc);
            Ok(Some(
                (event_at - Duration::minutes(offset_minutes)).to_rfc3339(),
            ))
        }
    }
}

pub(crate) fn load_item_reminder_configs(
    conn: &Connection,
    item_id: i64,
) -> Result<Vec<ReminderConfig>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT reminder_preset, offset_minutes
             FROM todo_item_reminders
             WHERE item_id=?1
             ORDER BY offset_minutes ASC, id ASC",
        )
        .map_err(|e| format!("查询事项提醒失败: {e}"))?;
    let rows = stmt
        .query_map(params![item_id], |row| {
            Ok(ReminderConfig {
                preset: row.get::<_, String>(0)?,
                offset_minutes: row.get::<_, i64>(1)?,
            })
        })
        .map_err(|e| format!("映射事项提醒失败: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub(crate) fn load_item_reminder_summary(
    conn: &Connection,
    item_id: i64,
) -> Result<TaskReminderSummary, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, reminder_preset, remind_at, snooze_until, last_notified_at
             FROM todo_item_reminders
             WHERE item_id=?1
             ORDER BY offset_minutes ASC, id ASC",
        )
        .map_err(|e| format!("查询事项提醒摘要失败: {e}"))?;
    let rows = stmt
        .query_map(params![item_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|e| format!("映射事项提醒摘要失败: {e}"))?;

    let mut summary = TaskReminderSummary::default();
    let mut next_fire_at: Option<String> = None;

    for row in rows {
        let (item_reminder_id, reminder_preset, remind_at, snooze_until, last_notified_at) =
            row.map_err(|e| e.to_string())?;

        if !summary.reminder_presets.contains(&reminder_preset) {
            summary.reminder_presets.push(reminder_preset.clone());
        }

        if let Some(value) = snooze_until.clone() {
            let should_replace = summary
                .snooze_until
                .as_deref()
                .and_then(parse_utc_datetime)
                .map(|current| {
                    parse_utc_datetime(&value)
                        .map(|candidate| candidate < current)
                        .unwrap_or(false)
                })
                .unwrap_or(true);
            if should_replace {
                summary.snooze_until = Some(value);
            }
        }

        if let Some(value) = last_notified_at.clone() {
            let should_replace = summary
                .last_notified_at
                .as_deref()
                .and_then(parse_utc_datetime)
                .map(|current| {
                    parse_utc_datetime(&value)
                        .map(|candidate| candidate > current)
                        .unwrap_or(false)
                })
                .unwrap_or(true);
            if should_replace {
                summary.last_notified_at = Some(value);
            }
        }

        let effective_fire_at = snooze_until.clone().unwrap_or(remind_at.clone());
        let pending = last_notified_at
            .as_deref()
            .and_then(parse_utc_datetime)
            .map(|last_notified| {
                parse_utc_datetime(&effective_fire_at)
                    .map(|fire_at| last_notified < fire_at)
                    .unwrap_or(false)
            })
            .unwrap_or(true);

        if pending {
            let should_replace = next_fire_at
                .as_deref()
                .and_then(parse_utc_datetime)
                .map(|current| {
                    parse_utc_datetime(&effective_fire_at)
                        .map(|candidate| candidate < current)
                        .unwrap_or(false)
                })
                .unwrap_or(true);
            if should_replace {
                next_fire_at = Some(effective_fire_at);
                summary.next_task_reminder_id = Some(item_reminder_id);
                summary.next_reminder_preset = Some(reminder_preset.clone());
            }
        }
    }

    if summary.reminder_presets.is_empty() {
        summary.reminder_presets = load_item_reminder_configs(conn, item_id)?
            .into_iter()
            .map(|config| config.preset)
            .collect();
    }
    sort_reminder_presets(&mut summary.reminder_presets);
    Ok(summary)
}

pub fn sync_item_reminders(
    conn: &Connection,
    item_id: i64,
    event_at: Option<&str>,
    reminder_presets: &[String],
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM todo_item_reminders WHERE item_id=?1",
        params![item_id],
    )
    .map_err(|e| format!("清理事项提醒失败: {e}"))?;

    let reminder_configs = reminder_configs_from_presets(reminder_presets);
    if reminder_configs.is_empty() {
        conn.execute(
            "UPDATE todo_items
             SET remind_at=NULL, snooze_until=NULL, last_notified_at=NULL, updated_at=CURRENT_TIMESTAMP
             WHERE id=?1",
            params![item_id],
        )
        .map_err(|e| format!("重置事项旧提醒字段失败: {e}"))?;
        return Ok(());
    }

    let event_at = event_at.ok_or("设置提醒前需要先提供事件时间或周期规则".to_string())?;
    for config in reminder_configs {
        let remind_at = compute_remind_at(Some(event_at), Some(config.offset_minutes))?
            .ok_or("提醒时间生成失败".to_string())?;
        conn.execute(
            "INSERT INTO todo_item_reminders(item_id, reminder_preset, offset_minutes, remind_at)
             VALUES(?1, ?2, ?3, ?4)",
            params![item_id, config.preset, config.offset_minutes, remind_at],
        )
        .map_err(|e| format!("保存事项提醒失败: {e}"))?;
    }

    conn.execute(
        "UPDATE todo_items
         SET remind_at=NULL, snooze_until=NULL, last_notified_at=NULL, updated_at=CURRENT_TIMESTAMP
         WHERE id=?1",
        params![item_id],
    )
    .map_err(|e| format!("更新事项旧提醒字段失败: {e}"))?;
    Ok(())
}

pub(crate) fn clear_item_reminder_snooze(conn: &Connection, item_id: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE todo_item_reminders
         SET snooze_until=NULL, updated_at=CURRENT_TIMESTAMP
         WHERE item_id=?1",
        params![item_id],
    )
    .map_err(|e| format!("清理事项稍后提醒失败: {e}"))?;
    Ok(())
}

pub(crate) fn mark_item_reminder_events_read(conn: &Connection, item_id: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE todo_reminder_events
         SET is_read=1, updated_at=CURRENT_TIMESTAMP
         WHERE task_id=?1 AND is_read=0",
        params![item_id],
    )
    .map_err(|e| format!("标记提醒事件已读失败: {e}"))?;
    Ok(())
}

pub(crate) fn resolve_item_reminder_id_for_snooze(
    conn: &Connection,
    item_id: i64,
    explicit_id: Option<i64>,
) -> Result<i64, String> {
    if let Some(reminder_id) = explicit_id {
        let exists = conn
            .query_row(
                "SELECT id FROM todo_item_reminders WHERE id=?1 AND item_id=?2",
                params![reminder_id, item_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|e| format!("查询提醒记录失败: {e}"))?;
        return exists.ok_or("提醒记录不存在".to_string());
    }

    conn.query_row(
        "SELECT id
         FROM todo_item_reminders
         WHERE item_id=?1
           AND (last_notified_at IS NULL OR last_notified_at < COALESCE(snooze_until, remind_at))
         ORDER BY COALESCE(snooze_until, remind_at) ASC, offset_minutes ASC, id ASC
         LIMIT 1",
        params![item_id],
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .map_err(|e| format!("查询可稍后提醒失败: {e}"))?
    .ok_or("当前事项没有可稍后的提醒".to_string())
}

// ── Reminder list / mark read ─────────────────────────────

pub(crate) fn reminder_list_unread(payload: &Value) -> Result<Value, String> {
    let limit = parse_i64(payload, "limit").unwrap_or(100).clamp(1, 500);
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, task_id, task_reminder_id, title, body, fire_at, is_read, created_at, reminder_preset
             FROM todo_reminder_events
             WHERE is_read=0
             ORDER BY fire_at DESC, id DESC
             LIMIT ?1",
        )
        .map_err(|e| format!("查询提醒中心失败: {e}"))?;
    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "taskId": row.get::<_, i64>(1)?,
                "taskReminderId": row.get::<_, Option<i64>>(2)?,
                "title": row.get::<_, String>(3)?,
                "body": row.get::<_, String>(4)?,
                "fireAt": row.get::<_, String>(5)?,
                "isRead": row.get::<_, i64>(6)? == 1,
                "createdAt": row.get::<_, String>(7).map(|s| format_db_datetime(&s))?,
                "reminderPreset": row.get::<_, Option<String>>(8)?.unwrap_or_else(|| REMINDER_PRESET_NONE.to_string())
            }))
        })
        .map_err(|e| format!("映射提醒中心失败: {e}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| e.to_string())?);
    }
    Ok(json!({ "items": items }))
}

pub(crate) fn reminder_mark_read(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    if payload.get("all").and_then(Value::as_bool).unwrap_or(false) {
        conn.execute(
            "UPDATE todo_reminder_events SET is_read=1, updated_at=CURRENT_TIMESTAMP WHERE is_read=0",
            [],
        )
        .map_err(|e| format!("标记全部已读失败: {e}"))?;
        return Ok(json!({ "ok": true }));
    }
    if let Some(id) = parse_i64(payload, "id") {
        conn.execute(
            "UPDATE todo_reminder_events SET is_read=1, updated_at=CURRENT_TIMESTAMP WHERE id=?1",
            params![id],
        )
        .map_err(|e| format!("标记已读失败: {e}"))?;
        return Ok(json!({ "ok": true }));
    }
    let ids = payload
        .get("ids")
        .and_then(Value::as_array)
        .ok_or("缺少提醒事件 id")?;
    let values: Vec<i64> = ids
        .iter()
        .filter_map(Value::as_i64)
        .filter(|id| *id > 0)
        .collect();
    if values.is_empty() {
        return Err("缺少有效提醒事件 id".to_string());
    }

    let placeholders = std::iter::repeat("?")
        .take(values.len())
        .collect::<Vec<&str>>()
        .join(",");
    let sql = format!(
        "UPDATE todo_reminder_events
         SET is_read=1, updated_at=CURRENT_TIMESTAMP
         WHERE id IN ({placeholders})"
    );
    let bind_values: Vec<rusqlite::types::Value> = values
        .into_iter()
        .map(rusqlite::types::Value::Integer)
        .collect();
    let bind_refs: Vec<&dyn rusqlite::ToSql> = bind_values
        .iter()
        .map(|v| v as &dyn rusqlite::ToSql)
        .collect();
    conn.execute(&sql, bind_refs.as_slice())
        .map_err(|e| format!("批量标记已读失败: {e}"))?;
    Ok(json!({ "ok": true }))
}

pub(crate) fn dispatch_due_reminders(
    conn: &Connection,
    now: DateTime<Utc>,
) -> Result<Vec<ReminderDispatch>, String> {
    let now_str = now.to_rfc3339();
    let mut stmt = conn
        .prepare(
            "SELECT
                ir.id,
                i.id,
                ir.reminder_preset,
                i.title,
                i.description,
                i.priority,
                COALESCE(ir.snooze_until, ir.remind_at) AS fire_at
             FROM todo_item_reminders ir
             JOIN todo_items i ON i.id = ir.item_id
             LEFT JOIN todo_series_rules sr ON sr.series_id = i.series_id
             WHERE i.status IN ('pending','in_progress')
               AND COALESCE(ir.snooze_until, ir.remind_at) <= ?1
               AND (ir.last_notified_at IS NULL OR ir.last_notified_at < COALESCE(ir.snooze_until, ir.remind_at))
               AND (i.kind <> 'recurring' OR sr.active IS NULL OR sr.active = 1)
             ORDER BY fire_at ASC, ir.id ASC
             LIMIT 200",
        )
        .map_err(|e| format!("查询待触发提醒失败: {e}"))?;
    let rows = stmt
        .query_map(params![now_str], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|e| format!("映射待触发提醒失败: {e}"))?;

    let mut reminders = Vec::new();
    for row in rows {
        let (item_reminder_id, item_id, reminder_preset, title, description, priority, fire_at) =
            row.map_err(|e| e.to_string())?;
        let body = if description.is_empty() {
            String::new()
        } else {
            description
        };
        conn.execute(
            "INSERT INTO todo_reminder_events(task_id, task_reminder_id, title, body, fire_at, is_read, reminder_preset)
             VALUES(?1, ?2, ?3, ?4, ?5, 0, ?6)",
            params![item_id, item_reminder_id, title, body, fire_at, reminder_preset],
        )
        .map_err(|e| format!("写入提醒中心失败: {e}"))?;
        let event_id = conn.last_insert_rowid();
        conn.execute(
            "UPDATE todo_item_reminders
             SET last_notified_at=?1,
                 updated_at=CURRENT_TIMESTAMP,
                 snooze_until=CASE WHEN snooze_until IS NOT NULL AND snooze_until <= ?1 THEN NULL ELSE snooze_until END
             WHERE id=?2",
            params![now.to_rfc3339(), item_reminder_id],
        )
        .map_err(|e| format!("更新事项提醒状态失败: {e}"))?;

        reminders.push(ReminderDispatch {
            event_id,
            task_id: item_id,
            task_reminder_id: item_reminder_id,
            title,
            body,
            fire_at,
            priority,
            reminder_preset,
        });
    }
    Ok(reminders)
}

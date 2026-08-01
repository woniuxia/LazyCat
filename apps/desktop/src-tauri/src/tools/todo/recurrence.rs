use chrono::{DateTime, Duration, Local, Timelike, Utc};
use chrono_tz::Tz;
use cron::Schedule;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::str::FromStr;

use super::helpers::*;
use super::types::*;
use super::{load_item_reminder_configs, sync_item_reminders};

// ── Cron / rule utilities ─────────────────────────────────

pub(crate) fn normalize_rule_mode(payload: &Value, fallback: &str) -> String {
    recurrence_payload(payload)
        .get("ruleMode")
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .trim()
        .to_lowercase()
}

pub(crate) fn build_simple_cron_expression(rule: &Value) -> Result<String, String> {
    let frequency = rule
        .get("frequency")
        .and_then(Value::as_str)
        .unwrap_or("daily")
        .trim()
        .to_lowercase();
    let interval = rule
        .get("interval")
        .and_then(Value::as_i64)
        .unwrap_or(1)
        .max(1);
    let time = rule
        .get("time")
        .and_then(Value::as_str)
        .unwrap_or("09:00")
        .trim();
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() != 2 {
        return Err("简单规则时间格式不正确".to_string());
    }
    let hour = parts[0]
        .parse::<i64>()
        .map_err(|_| "简单规则时间格式不正确".to_string())?;
    let minute = parts[1]
        .parse::<i64>()
        .map_err(|_| "简单规则时间格式不正确".to_string())?;
    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) {
        return Err("简单规则时间格式不正确".to_string());
    }
    if minute % 5 != 0 {
        return Err("周期事件时间必须使用 5 分钟刻度".to_string());
    }

    let expr = match frequency.as_str() {
        "daily" => {
            if interval == 1 {
                format!("0 {minute} {hour} * * *")
            } else {
                format!("0 {minute} {hour} */{interval} * *")
            }
        }
        "weekly" => {
            let mut weekdays = rule
                .get("weekdays")
                .and_then(Value::as_array)
                .map(|arr| {
                    let mut out = arr
                        .iter()
                        .filter_map(Value::as_i64)
                        .filter(|v| (1..=7).contains(v))
                        .collect::<Vec<i64>>();
                    out.sort_unstable();
                    out.dedup();
                    out
                })
                .unwrap_or_else(|| vec![1]);
            if weekdays.is_empty() {
                weekdays = vec![1];
            }

            let dow = if weekdays == vec![1, 2, 3, 4, 5] {
                "Mon-Fri".to_string()
            } else {
                let items = weekdays
                    .iter()
                    .filter_map(|weekday| match weekday {
                        1 => Some("Mon"),
                        2 => Some("Tue"),
                        3 => Some("Wed"),
                        4 => Some("Thu"),
                        5 => Some("Fri"),
                        6 => Some("Sat"),
                        7 => Some("Sun"),
                        _ => None,
                    })
                    .collect::<Vec<&str>>();
                if items.is_empty() {
                    "Mon".to_string()
                } else {
                    items.join(",")
                }
            };
            format!("0 {minute} {hour} * * {dow}")
        }
        "monthly" => {
            let day = rule
                .get("dayOfMonth")
                .and_then(Value::as_i64)
                .unwrap_or(1)
                .clamp(1, 31);
            if interval == 1 {
                format!("0 {minute} {hour} {day} * *")
            } else {
                format!("0 {minute} {hour} {day} */{interval} *")
            }
        }
        _ => return Err("简单周期规则不合法，frequency 仅支持 daily/weekly/monthly".to_string()),
    };

    ensure_schedule_granularity(&expr)?;
    Ok(expr)
}

pub(crate) fn resolve_cron_expression(rule_mode: &str, rule: &Value) -> Result<String, String> {
    let expression = match rule_mode {
        "simple" => build_simple_cron_expression(rule)?,
        "cron" => {
            let raw = rule
                .get("expression")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if raw.is_empty() {
                return Err("Cron 规则缺少 expression".to_string());
            }
            let fields: Vec<&str> = raw.split_whitespace().collect();
            let normalized = if fields.len() == 5 {
                format!("0 {raw}")
            } else {
                raw.to_string()
            };
            Schedule::from_str(&normalized).map_err(|e| format!("Cron 表达式无效: {e}"))?;
            normalized
        }
        _ => return Err("ruleMode 仅支持 simple 或 cron".to_string()),
    };
    ensure_schedule_granularity(&expression)?;
    Ok(expression)
}

pub(crate) fn ensure_schedule_granularity(cron_expression: &str) -> Result<(), String> {
    let schedule =
        Schedule::from_str(cron_expression).map_err(|e| format!("Cron 表达式无效: {e}"))?;
    let mut upcoming = schedule.after(&Utc::now());
    for _ in 0..32 {
        let Some(next) = upcoming.next() else {
            break;
        };
        if next.second() != 0 || next.minute() % EVENT_TIME_MINUTE_STEP != 0 {
            return Err("周期事件时间必须使用 5 分钟刻度".to_string());
        }
    }
    Ok(())
}

pub(crate) fn compute_next_occurrence(
    cron_expression: &str,
    timezone: &str,
    after_utc: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, String> {
    let schedule =
        Schedule::from_str(cron_expression).map_err(|e| format!("周期表达式无效: {e}"))?;

    if timezone.eq_ignore_ascii_case("utc") {
        return Ok(schedule.after(&after_utc).next());
    }
    if timezone.eq_ignore_ascii_case("local") {
        let local_after = after_utc.with_timezone(&Local);
        let next = schedule.after(&local_after).next();
        return Ok(next.map(|dt| dt.with_timezone(&Utc)));
    }
    match timezone.parse::<Tz>() {
        Ok(tz) => {
            let tz_after = after_utc.with_timezone(&tz);
            Ok(schedule
                .after(&tz_after)
                .next()
                .map(|dt| dt.with_timezone(&Utc)))
        }
        Err(_) => Err(format!("不支持的时区: {timezone}")),
    }
}

pub(crate) fn compute_next_occurrence_with_start(
    cron_expression: &str,
    timezone: &str,
    start_at: Option<&str>,
    after_utc: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, String> {
    let Some(start_at_dt) = start_at.and_then(parse_utc_datetime) else {
        return compute_next_occurrence(cron_expression, timezone, after_utc);
    };
    let search_after = if after_utc <= start_at_dt {
        start_at_dt - Duration::seconds(1)
    } else {
        after_utc
    };
    let next = compute_next_occurrence(cron_expression, timezone, search_after)?;
    Ok(next.filter(|occurrence| *occurrence >= start_at_dt))
}

pub(crate) fn parse_end_rule(payload: &Value) -> Result<(String, Option<String>), String> {
    let recurrence = recurrence_payload(payload);
    let mode = recurrence
        .get("endMode")
        .and_then(Value::as_str)
        .unwrap_or("never")
        .trim()
        .to_string();
    match mode.as_str() {
        "never" => Ok((mode, None)),
        "until_date" => {
            let end_payload =
                json!({ "endValue": recurrence.get("endValue").cloned().unwrap_or(Value::Null) });
            let value = parse_datetime_with_validation(&end_payload, "endValue", "结束时间", true)?
                .ok_or("endValue 必填")?;
            Ok((mode, Some(value)))
        }
        "after_count" => {
            let count = recurrence
                .get("endValue")
                .and_then(Value::as_i64)
                .ok_or("endValue 必须是次数")?
                .max(1);
            Ok((mode, Some(count.to_string())))
        }
        _ => Err("endMode 仅支持 never/until_date/after_count".to_string()),
    }
}

// ── Series rule helpers ───────────────────────────────────

pub(crate) fn load_series_rule(
    conn: &Connection,
    series_id: i64,
) -> Result<Option<SeriesRuleRow>, String> {
    conn.query_row(
        "SELECT series_id, rule_mode, COALESCE(rule_json,'{}'), COALESCE(cron_expression,''),
                COALESCE(timezone,'local'), start_at, end_mode, end_value,
                occurrence_index, active
         FROM todo_series_rules
         WHERE series_id=?1",
        params![series_id],
        |row| {
            Ok(SeriesRuleRow {
                series_id: row.get(0)?,
                rule_mode: row.get(1)?,
                rule_json: row.get(2)?,
                cron_expression: row.get(3)?,
                timezone: row.get(4)?,
                start_at: row.get(5)?,
                end_mode: row.get(6)?,
                end_value: row.get(7)?,
                occurrence_index: row.get(8)?,
                active: row.get::<_, i64>(9)? == 1,
            })
        },
    )
    .optional()
    .map_err(|e| format!("查询系列规则失败: {e}"))
}

pub(crate) fn should_stop_series(rule: &SeriesRuleRow, occurrence: DateTime<Utc>) -> bool {
    match rule.end_mode.as_str() {
        "never" => false,
        "until_date" => rule
            .end_value
            .as_deref()
            .and_then(parse_rfc3339)
            .and_then(|v| DateTime::parse_from_rfc3339(&v).ok())
            .map(|dt| occurrence > dt.with_timezone(&Utc))
            .unwrap_or(false),
        "after_count" => rule
            .end_value
            .as_deref()
            .and_then(|v| v.parse::<i64>().ok())
            .map(|max| rule.occurrence_index >= max)
            .unwrap_or(false),
        _ => false,
    }
}

pub(crate) fn has_other_open_in_series(
    conn: &Connection,
    series_id: i64,
    excluded_item_id: i64,
) -> Result<bool, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM todo_items
             WHERE series_id=?1
               AND id<>?2
               AND status IN ('pending','in_progress')",
            params![series_id, excluded_item_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("查询系列实例失败: {e}"))?;
    Ok(count > 0)
}

/// 生成系列的下一个事项（完成或删除触发）
pub(crate) fn generate_next_item(
    conn: &Connection,
    series_id: i64,
    source_item_id: i64,
    increment_index: bool,
) -> Result<Option<i64>, String> {
    let Some(rule) = load_series_rule(conn, series_id)? else {
        return Ok(None);
    };
    if !rule.active {
        return Ok(None);
    }

    // 已有其它 open 项则不生成
    if has_other_open_in_series(conn, series_id, source_item_id)? {
        return Ok(None);
    }

    // 计算 base_time = max(source.event_at, now)
    let source_event_at: Option<String> = conn
        .query_row(
            "SELECT event_at FROM todo_items WHERE id=?1",
            params![source_item_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("查询源事项时间失败: {e}"))?
        .flatten();
    let now = Utc::now();
    let base_time = source_event_at
        .as_deref()
        .and_then(parse_utc_datetime)
        .map(|dt| if dt > now { dt } else { now })
        .unwrap_or(now);

    let next_occurrence = compute_next_occurrence_with_start(
        &rule.cron_expression,
        &rule.timezone,
        rule.start_at.as_deref(),
        base_time,
    )?;

    let Some(next_dt) = next_occurrence else {
        return Ok(None);
    };

    if should_stop_series(&rule, next_dt) {
        return Ok(None);
    }

    // 读取源事项基础字段
    let (title, type_id, priority, description): (String, Option<i64>, String, String) = conn
        .query_row(
            "SELECT title, type_id, priority, description FROM todo_items WHERE id=?1",
            params![source_item_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| format!("读取源事项失败: {e}"))?;

    let next_event_at = next_dt.to_rfc3339();

    // 插入新事项
    conn.execute(
        "INSERT INTO todo_items(title, type_id, priority, description, kind, series_id, parent_id, status, event_at, pinned)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0)",
        params![
            title,
            type_id,
            priority,
            description,
            SERIES_KIND_RECURRING,
            series_id,
            source_item_id,
            STATUS_PENDING,
            next_event_at,
        ],
    )
    .map_err(|e| format!("生成下一个事项失败: {e}"))?;
    let new_item_id = conn.last_insert_rowid();

    // Inherit project_id from source item
    let _ = conn.execute(
        "UPDATE todo_items SET project_id = (SELECT project_id FROM todo_items WHERE id = ?1) WHERE id = ?2",
        params![source_item_id, new_item_id],
    );

    // 复制支撑表数据
    conn.execute(
        "INSERT OR IGNORE INTO todo_item_assignees(item_id, assignee_id)
         SELECT ?1, assignee_id FROM todo_item_assignees WHERE item_id=?2",
        params![new_item_id, source_item_id],
    )
    .map_err(|e| format!("复制事项执行人失败: {e}"))?;

    // 复制提醒（重算 remind_at）
    let reminder_presets: Vec<String> = load_item_reminder_configs(conn, source_item_id)?
        .into_iter()
        .map(|c| c.preset)
        .collect();
    sync_item_reminders(conn, new_item_id, Some(&next_event_at), &reminder_presets)?;

    // 复制链接
    conn.execute(
        "INSERT INTO todo_item_links(item_id, url, title, sort_order)
         SELECT ?1, url, title, sort_order FROM todo_item_links WHERE item_id=?2 ORDER BY sort_order ASC",
        params![new_item_id, source_item_id],
    )
    .map_err(|e| format!("复制事项链接失败: {e}"))?;

    // 更新规则表进度
    if increment_index {
        conn.execute(
            "UPDATE todo_series_rules SET occurrence_index=occurrence_index+1, updated_at=CURRENT_TIMESTAMP
             WHERE series_id=?1",
            params![series_id],
        )
        .map_err(|e| format!("更新系列进度失败: {e}"))?;
    }

    Ok(Some(new_item_id))
}

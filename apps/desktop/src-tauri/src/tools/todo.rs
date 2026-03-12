use chrono::{DateTime, Duration, Local, NaiveDateTime, Timelike, Utc};
use chrono_tz::Tz;
use cron::Schedule;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use std::str::FromStr;

use super::helpers::db_conn;

const PRIORITIES: [&str; 4] = ["P0", "P1", "P2", "P3"];
const STATUS_PENDING: &str = "pending";
const STATUS_IN_PROGRESS: &str = "in_progress";
const STATUS_COMPLETED: &str = "completed";
const STATUS_CANCELED: &str = "canceled";
const SERIES_KIND_ONE_OFF: &str = "one_off";
const SERIES_KIND_RECURRING: &str = "recurring";
const RECORD_ROLE_ROOT: &str = "root";
const RECORD_ROLE_OCCURRENCE: &str = "occurrence";
const SCOPE_THIS_INSTANCE: &str = "this_instance";
const SCOPE_FUTURE_INSTANCES: &str = "future_instances";
const REMINDER_PRESET_ON_TIME: &str = "0m";
const REMINDER_PRESET_NONE: &str = "none";
const REMINDER_PRESET_5M: &str = "5m";
const REMINDER_PRESET_10M: &str = "10m";
const REMINDER_PRESET_30M: &str = "30m";
const REMINDER_PRESET_1H: &str = "1h";
const REMINDER_PRESET_1D: &str = "1d";
const REMINDER_PRESET_2D: &str = "2d";
const EVENT_TIME_MINUTE_STEP: u32 = 5;
const REMINDER_PRESET_OFFSETS: [(&str, i64); 7] = [
    (REMINDER_PRESET_ON_TIME, 0),
    (REMINDER_PRESET_5M, 5),
    (REMINDER_PRESET_10M, 10),
    (REMINDER_PRESET_30M, 30),
    (REMINDER_PRESET_1H, 60),
    (REMINDER_PRESET_1D, 24 * 60),
    (REMINDER_PRESET_2D, 2 * 24 * 60),
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderDispatch {
    pub event_id: i64,
    pub task_id: i64,
    pub task_reminder_id: i64,
    pub title: String,
    pub body: String,
    pub fire_at: String,
    pub priority: String,
    pub reminder_preset: String,
}

#[derive(Debug, Clone)]
struct TemplateRow {
    id: i64,
    title: String,
    type_id: Option<i64>,
    priority: String,
    description: String,
    _series_kind: String,
    cron_expression: String,
    timezone: String,
    start_at: Option<String>,
    end_mode: String,
    end_value: Option<String>,
    next_occurrence_at: Option<String>,
    generated_count: i64,
    active: bool,
    reminder_configs: Vec<ReminderConfig>,
}

#[derive(Debug, Clone)]
struct ReminderConfig {
    preset: String,
    offset_minutes: i64,
}

#[derive(Debug, Clone, Default)]
struct TaskReminderSummary {
    reminder_presets: Vec<String>,
    snooze_until: Option<String>,
    last_notified_at: Option<String>,
    next_task_reminder_id: Option<i64>,
    next_reminder_preset: Option<String>,
}

#[derive(Debug, Clone)]
struct TaskSnapshot {
    event_at: Option<String>,
    source_template_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemUpdateStrategy {
    UpdateTask,
    UpdateTemplate,
    ConvertTaskToRecurring,
    DetachOccurrenceToOneOff,
    ConvertSeriesToOneOff,
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "type_list" => type_list(),
        "type_upsert" => type_upsert(payload),
        "type_delete" => type_delete(payload),
        "assignee_list" => assignee_list(),
        "assignee_upsert" => assignee_upsert(payload),
        "assignee_delete" => assignee_delete(payload),
        "item_list" => item_list(payload),
        "item_create" => item_create(payload),
        "item_update" => item_update(payload),
        "item_upsert" => item_upsert(payload),
        "item_change_status" => item_change_status(payload),
        "item_snooze" => item_snooze(payload),
        "item_toggle_pin" => item_toggle_pin(payload),
        "item_toggle_active" => item_toggle_active(payload),
        "item_delete" => item_delete(payload),
        "reminder_list_unread" => reminder_list_unread(payload),
        "reminder_mark_read" => reminder_mark_read(payload),
        _ => Err(format!("unsupported todo action: {action}")),
    }
}

pub fn scheduler_tick() -> Result<Vec<ReminderDispatch>, String> {
    let conn = db_conn()?;
    let now = Utc::now();
    generate_recurring_instances(&conn, now)?;
    dispatch_due_reminders(&conn, now)
}

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

fn recurrence_payload<'a>(payload: &'a Value) -> &'a Value {
    payload
        .get("recurrence")
        .filter(|value| value.is_object())
        .unwrap_or(payload)
}

fn parse_item_kind(payload: &Value) -> String {
    if let Some(kind) = payload.get("kind").and_then(Value::as_str) {
        return normalize_series_kind(Some(kind));
    }
    if payload
        .get("recurrence")
        .and_then(Value::as_object)
        .is_some()
    {
        return SERIES_KIND_RECURRING.to_string();
    }
    resolve_series_kind(payload)
}

fn is_root_record(payload: &Value) -> bool {
    matches!(payload.get("recordRole").and_then(Value::as_str), Some(RECORD_ROLE_ROOT))
}

fn parse_root_id(payload: &Value) -> Option<i64> {
    parse_i64(payload, "rootId")
        .or(parse_i64(payload, "seriesId"))
        .or(parse_i64(payload, "sourceTemplateId"))
}

fn resolve_recurring_template_id(conn: &Connection, payload: &Value) -> Result<Option<i64>, String> {
    if parse_item_kind(payload) != SERIES_KIND_RECURRING {
        return Ok(None);
    }
    if let Some(root_id) = parse_root_id(payload) {
        return Ok(Some(root_id));
    }
    if is_root_record(payload) {
        return Ok(parse_i64(payload, "id"));
    }
    if let Some(id) = parse_i64(payload, "id") {
        let (series_id, series_kind) = resolve_task_series(conn, id)?;
        if series_kind == SERIES_KIND_RECURRING {
            return Ok(series_id);
        }
    }
    Ok(None)
}

fn resolve_item_update_strategy(
    current_kind: &str,
    next_kind: &str,
    scope: &str,
    root_record: bool,
) -> ItemUpdateStrategy {
    match (current_kind, next_kind) {
        (SERIES_KIND_ONE_OFF, SERIES_KIND_RECURRING) => ItemUpdateStrategy::ConvertTaskToRecurring,
        (SERIES_KIND_RECURRING, SERIES_KIND_ONE_OFF) => {
            if root_record || scope == SCOPE_FUTURE_INSTANCES {
                ItemUpdateStrategy::ConvertSeriesToOneOff
            } else {
                ItemUpdateStrategy::DetachOccurrenceToOneOff
            }
        }
        (SERIES_KIND_RECURRING, SERIES_KIND_RECURRING) => {
            if root_record || scope == SCOPE_FUTURE_INSTANCES {
                ItemUpdateStrategy::UpdateTemplate
            } else {
                ItemUpdateStrategy::UpdateTask
            }
        }
        _ => ItemUpdateStrategy::UpdateTask,
    }
}

fn load_task_snapshot(conn: &Connection, task_id: i64) -> Result<TaskSnapshot, String> {
    conn.query_row(
        "SELECT event_at, source_template_id FROM todo_tasks WHERE id=?1",
        params![task_id],
        |row| {
            Ok(TaskSnapshot {
                event_at: row.get(0)?,
                source_template_id: row.get(1)?,
            })
        },
    )
    .map_err(|_| "事项不存在".to_string())
}

fn load_template_anchor_event_at(conn: &Connection, template_id: i64) -> Result<Option<String>, String> {
    let anchor = conn
        .query_row(
            "SELECT COALESCE(next_occurrence_at, start_at) FROM todo_templates WHERE id=?1",
            params![template_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| format!("查询周期事项时间失败: {e}"))?
        .flatten();

    Ok(anchor.map(|value| parse_rfc3339(&value).unwrap_or(value)))
}

fn ensure_event_at_when_missing(
    payload: &Value,
    fallback_event_at: Option<&str>,
) -> Result<Value, String> {
    if payload.get("eventAt").is_some() {
        return Ok(payload.clone());
    }

    let resolved_event_at = parse_start_datetime(payload)?.or_else(|| {
        fallback_event_at
            .and_then(parse_rfc3339)
            .or_else(|| fallback_event_at.map(ToString::to_string))
    });

    let mut next_payload = payload.clone();
    if let (Some(event_at), Some(obj)) = (resolved_event_at, next_payload.as_object_mut()) {
        obj.insert("eventAt".to_string(), json!(event_at));
    }
    Ok(next_payload)
}

fn resolve_existing_update_target(
    conn: &Connection,
    payload: &Value,
) -> Result<(String, Option<i64>, Option<i64>), String> {
    if is_root_record(payload) {
        let template_id = parse_root_id(payload)
            .or(parse_i64(payload, "id"))
            .ok_or("缺少事项 id")?;
        return Ok((load_series_kind(conn, template_id)?, None, Some(template_id)));
    }

    if let Some(task_id) = parse_i64(payload, "id") {
        let source_template_id = conn
            .query_row(
                "SELECT source_template_id FROM todo_tasks WHERE id=?1",
                params![task_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()
            .map_err(|e| format!("查询事项失败: {e}"))?
            .ok_or("事项不存在")?;
        if let Some(template_id) = source_template_id {
            return Ok((
                load_series_kind(conn, template_id)?,
                Some(task_id),
                Some(template_id),
            ));
        }
        return Ok((SERIES_KIND_ONE_OFF.to_string(), Some(task_id), None));
    }

    if let Some(template_id) = parse_root_id(payload) {
        return Ok((load_series_kind(conn, template_id)?, None, Some(template_id)));
    }

    Err("缺少事项 id".to_string())
}

fn extract_items(response: Value) -> Result<Vec<Value>, String> {
    response
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .ok_or("事项返回数据格式不正确".to_string())
}

fn item_priority_rank(item: &Value) -> i32 {
    match item.get("priority").and_then(Value::as_str).unwrap_or("P2") {
        "P0" => 0,
        "P1" => 1,
        "P2" => 2,
        _ => 3,
    }
}

fn item_sort_time(item: &Value) -> String {
    item.get("displayAt")
        .and_then(Value::as_str)
        .or_else(|| {
            item.get("recurrence")
                .and_then(Value::as_object)
                .and_then(|recurrence| recurrence.get("nextOccurrenceAt"))
                .and_then(Value::as_str)
        })
        .unwrap_or("")
        .to_string()
}

fn item_pinned_rank(item: &Value) -> i32 {
    if item.get("pinned").and_then(Value::as_bool).unwrap_or(false) {
        0
    } else {
        1
    }
}

fn sort_item_rows(items: &mut [Value]) {
    items.sort_by(|left, right| {
        item_pinned_rank(left)
            .cmp(&item_pinned_rank(right))
            .then_with(|| item_priority_rank(left).cmp(&item_priority_rank(right)))
            .then_with(|| item_sort_time(left).cmp(&item_sort_time(right)))
            .then_with(|| {
                right
                    .get("id")
                    .and_then(Value::as_i64)
                    .unwrap_or_default()
                    .cmp(&left.get("id").and_then(Value::as_i64).unwrap_or_default())
            })
    });
}

fn parse_assignee_ids(payload: &Value) -> Vec<i64> {
    payload
        .get("assigneeIds")
        .and_then(Value::as_array)
        .map(|arr| {
            let mut ids = Vec::new();
            for id in arr.iter().filter_map(Value::as_i64) {
                if id > 0 && !ids.contains(&id) {
                    ids.push(id);
                }
            }
            ids
        })
        .unwrap_or_default()
}

fn parse_rfc3339(raw: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).to_rfc3339())
        .or_else(|| {
            NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc).to_rfc3339())
        })
}

fn parse_utc_datetime(raw: &str) -> Option<DateTime<Utc>> {
    parse_rfc3339(raw)
        .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

fn is_five_minute_datetime(dt: &DateTime<chrono::FixedOffset>) -> bool {
    dt.minute() % EVENT_TIME_MINUTE_STEP == 0 && dt.second() == 0 && dt.nanosecond() == 0
}

fn parse_datetime_with_validation(
    payload: &Value,
    key: &str,
    label: &str,
    require_five_minute_step: bool,
) -> Result<Option<String>, String> {
    let Some(raw) = payload.get(key) else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    let text = raw
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{label}格式不正确"))?;
    let parsed = DateTime::parse_from_rfc3339(text)
        .map_err(|_| format!("{label}格式不正确"))?;
    if require_five_minute_step && !is_five_minute_datetime(&parsed) {
        return Err(format!("{label}必须使用 5 分钟刻度"));
    }
    Ok(Some(parsed.with_timezone(&Utc).to_rfc3339()))
}

fn parse_event_datetime(payload: &Value, key: &str) -> Result<Option<String>, String> {
    parse_datetime_with_validation(payload, key, "事件时间", true)
}

fn parse_start_datetime(payload: &Value) -> Result<Option<String>, String> {
    let recurrence = recurrence_payload(payload);
    if recurrence.get("startAt").is_some() {
        return parse_datetime_with_validation(
            &json!({ "startAt": recurrence.get("startAt").cloned().unwrap_or(Value::Null) }),
            "startAt",
            "开始时间",
            true,
        );
    }
    if payload.get("startAt").is_some() {
        return parse_datetime_with_validation(payload, "startAt", "开始时间", true);
    }
    if payload.get("eventAt").is_some() {
        return parse_datetime_with_validation(payload, "eventAt", "开始时间", true);
    }
    Ok(None)
}

fn has_start_datetime(payload: &Value) -> bool {
    payload.get("startAt").is_some() || recurrence_payload(payload).get("startAt").is_some()
}

fn compute_next_occurrence_with_start(
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

fn reminder_offset_minutes_from_preset(preset: &str) -> Option<i64> {
    REMINDER_PRESET_OFFSETS
        .iter()
        .find_map(|(candidate, minutes)| (*candidate == preset).then_some(*minutes))
}

fn reminder_preset_sort_key(preset: &str) -> usize {
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

fn normalize_reminder_preset(value: &str) -> Option<String> {
    let normalized = value.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }
    if normalized == REMINDER_PRESET_NONE {
        return Some(REMINDER_PRESET_NONE.to_string());
    }
    reminder_offset_minutes_from_preset(&normalized).map(|_| normalized)
}

fn reminder_preset_from_offset(offset_minutes: Option<i64>) -> String {
    offset_minutes
        .and_then(|minutes| {
            REMINDER_PRESET_OFFSETS
                .iter()
                .find_map(|(preset, candidate)| (*candidate == minutes).then_some(*preset))
        })
        .unwrap_or(REMINDER_PRESET_NONE)
        .to_string()
}

fn sort_reminder_presets(presets: &mut Vec<String>) {
    presets.sort_by_key(|preset| reminder_preset_sort_key(preset));
}

fn normalize_reminder_presets(values: &[String]) -> Result<Vec<String>, String> {
    let mut presets = Vec::new();
    let mut has_none = false;

    for value in values {
        let normalized = normalize_reminder_preset(value)
            .ok_or_else(|| "提醒方式不支持该预设值".to_string())?;
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

fn parse_reminder_presets(payload: &Value) -> Result<Option<Vec<String>>, String> {
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

fn reminder_configs_from_presets(presets: &[String]) -> Vec<ReminderConfig> {
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

fn derive_reminder_presets(event_at: Option<&str>, remind_at: Option<&str>) -> Vec<String> {
    let preset = reminder_preset_from_offset(derive_reminder_offset_minutes(event_at, remind_at));
    if preset == REMINDER_PRESET_NONE {
        return Vec::new();
    }
    vec![preset]
}

fn compute_remind_at(event_at: Option<&str>, offset_minutes: Option<i64>) -> Result<Option<String>, String> {
    match (event_at, offset_minutes) {
        (_, None) => Ok(None),
        (None, Some(_)) => Err("设置提醒前需要先提供事件时间或周期规则".to_string()),
        (Some(event_at), Some(offset_minutes)) => {
            let event_at = DateTime::parse_from_rfc3339(event_at)
                .map_err(|_| "事件时间格式不正确".to_string())?
                .with_timezone(&Utc);
            Ok(Some((event_at - Duration::minutes(offset_minutes)).to_rfc3339()))
        }
    }
}

fn derive_reminder_offset_minutes(event_at: Option<&str>, remind_at: Option<&str>) -> Option<i64> {
    let event_at = event_at
        .and_then(parse_rfc3339)
        .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())?
        .with_timezone(&Utc);
    let remind_at = remind_at
        .and_then(parse_rfc3339)
        .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())?
        .with_timezone(&Utc);
    let offset_minutes = event_at.signed_duration_since(remind_at).num_minutes();
    reminder_offset_minutes_from_preset(&reminder_preset_from_offset(Some(offset_minutes)))
}

fn load_task_event_at(conn: &Connection, task_id: i64) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT event_at FROM todo_tasks WHERE id=?1",
        params![task_id],
        |row| row.get(0),
    )
    .map_err(|_| "事项不存在".to_string())
}

fn ensure_schedule_granularity(cron_expression: &str) -> Result<(), String> {
    let schedule = Schedule::from_str(cron_expression).map_err(|e| format!("Cron 表达式无效: {e}"))?;
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

fn normalize_priority(value: Option<&str>) -> Result<String, String> {
    let p = value.unwrap_or("P2").trim().to_uppercase();
    if PRIORITIES.contains(&p.as_str()) {
        Ok(p)
    } else {
        Err("浼樺厛绾у繀椤绘槸 P0/P1/P2/P3".to_string())
    }
}

fn normalize_status(value: &str) -> Result<String, String> {
    match value {
        STATUS_PENDING | STATUS_IN_PROGRESS | STATUS_COMPLETED | STATUS_CANCELED => {
            Ok(value.to_string())
        }
        _ => Err("鐘舵€佷笉鍚堟硶".to_string()),
    }
}

fn can_transit(current: &str, next: &str) -> bool {
    if current == next {
        return true;
    }
    matches!(
        (current, next),
        (STATUS_PENDING, STATUS_IN_PROGRESS)
            | (STATUS_PENDING, STATUS_COMPLETED)
            | (STATUS_PENDING, STATUS_CANCELED)
            | (STATUS_IN_PROGRESS, STATUS_PENDING)
            | (STATUS_IN_PROGRESS, STATUS_COMPLETED)
            | (STATUS_IN_PROGRESS, STATUS_CANCELED)
    )
}

fn parse_scope(payload: &Value) -> String {
    match payload.get("scope").and_then(Value::as_str) {
        Some(SCOPE_FUTURE_INSTANCES) => SCOPE_FUTURE_INSTANCES.to_string(),
        _ => SCOPE_THIS_INSTANCE.to_string(),
    }
}

fn normalize_series_kind(value: Option<&str>) -> String {
    match value.unwrap_or(SERIES_KIND_ONE_OFF) {
        SERIES_KIND_RECURRING => SERIES_KIND_RECURRING.to_string(),
        _ => SERIES_KIND_ONE_OFF.to_string(),
    }
}

fn resolve_series_kind(payload: &Value) -> String {
    if payload
        .get("isRecurring")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return SERIES_KIND_RECURRING.to_string();
    }
    normalize_series_kind(payload.get("seriesKind").and_then(Value::as_str))
}

fn load_series_kind(conn: &Connection, series_id: i64) -> Result<String, String> {
    conn.query_row(
        "SELECT COALESCE(series_kind, 'recurring') FROM todo_templates WHERE id=?1",
        params![series_id],
        |row| row.get::<_, String>(0),
    )
    .map_err(|_| "事项系列不存在".to_string())
}

fn resolve_task_series(conn: &Connection, task_id: i64) -> Result<(Option<i64>, String), String> {
    let series_id = conn
        .query_row(
            "SELECT source_template_id FROM todo_tasks WHERE id=?1",
            params![task_id],
            |row| row.get::<_, Option<i64>>(0),
        )
        .map_err(|_| "事项不存在".to_string())?;

    if let Some(series_id) = series_id {
        let series_kind = load_series_kind(conn, series_id)?;
        Ok((Some(series_id), series_kind))
    } else {
        Ok((None, SERIES_KIND_ONE_OFF.to_string()))
    }
}

fn sync_task_assignees(conn: &Connection, task_id: i64, ids: &[i64]) -> Result<(), String> {
    conn.execute("DELETE FROM todo_task_assignees WHERE task_id=?1", params![task_id])
        .map_err(|e| format!("娓呯悊浠诲姟鎵ц浜哄け璐? {e}"))?;
    for id in ids {
        conn.execute(
            "INSERT OR IGNORE INTO todo_task_assignees(task_id, assignee_id) VALUES(?1, ?2)",
            params![task_id, id],
        )
        .map_err(|e| format!("淇濆瓨浠诲姟鎵ц浜哄け璐? {e}"))?;
    }
    Ok(())
}

fn load_task_assignees(conn: &Connection, task_id: i64) -> Result<Vec<Value>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT a.id, a.name
             FROM todo_task_assignees ta
             JOIN todo_assignees a ON a.id = ta.assignee_id
             WHERE ta.task_id = ?1
             ORDER BY a.name COLLATE NOCASE ASC",
        )
        .map_err(|e| format!("鏌ヨ浠诲姟鎵ц浜哄け璐? {e}"))?;
    let rows = stmt
        .query_map(params![task_id], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?
            }))
        })
        .map_err(|e| format!("鏄犲皠浠诲姟鎵ц浜哄け璐? {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn load_task_reminder_configs(conn: &Connection, task_id: i64) -> Result<Vec<ReminderConfig>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT reminder_preset, offset_minutes
             FROM todo_task_reminders
             WHERE task_id=?1
             ORDER BY offset_minutes ASC, id ASC",
        )
        .map_err(|e| format!("查询事项提醒失败: {e}"))?;
    let rows = stmt
        .query_map(params![task_id], |row| {
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

    if !out.is_empty() {
        return Ok(out);
    }

    let legacy = conn
        .query_row(
            "SELECT event_at, remind_at FROM todo_tasks WHERE id=?1",
            params![task_id],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map_err(|e| format!("查询事项旧提醒失败: {e}"))?;

    Ok(legacy
        .map(|(event_at, remind_at)| {
            reminder_configs_from_presets(&derive_reminder_presets(
                event_at.as_deref(),
                remind_at.as_deref(),
            ))
        })
        .unwrap_or_default())
}

fn load_template_reminder_configs(
    conn: &Connection,
    template_id: i64,
) -> Result<Vec<ReminderConfig>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT reminder_preset, offset_minutes
             FROM todo_template_reminders
             WHERE template_id=?1
             ORDER BY offset_minutes ASC, id ASC",
        )
        .map_err(|e| format!("查询周期提醒失败: {e}"))?;
    let rows = stmt
        .query_map(params![template_id], |row| {
            Ok(ReminderConfig {
                preset: row.get::<_, String>(0)?,
                offset_minutes: row.get::<_, i64>(1)?,
            })
        })
        .map_err(|e| format!("映射周期提醒失败: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }

    if !out.is_empty() {
        return Ok(out);
    }

    let legacy_offset = conn
        .query_row(
            "SELECT reminder_offset_minutes FROM todo_templates WHERE id=?1",
            params![template_id],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()
        .map_err(|e| format!("查询周期旧提醒失败: {e}"))?
        .flatten();

    Ok(legacy_offset
        .map(|offset_minutes| reminder_configs_from_presets(&vec![reminder_preset_from_offset(Some(offset_minutes))]))
        .unwrap_or_default())
}

fn load_task_reminder_summary(conn: &Connection, task_id: i64) -> Result<TaskReminderSummary, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, reminder_preset, remind_at, snooze_until, last_notified_at
             FROM todo_task_reminders
             WHERE task_id=?1
             ORDER BY offset_minutes ASC, id ASC",
        )
        .map_err(|e| format!("查询事项提醒摘要失败: {e}"))?;
    let rows = stmt
        .query_map(params![task_id], |row| {
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
        let (task_reminder_id, reminder_preset, remind_at, snooze_until, last_notified_at) =
            row.map_err(|e| e.to_string())?;

        if !summary.reminder_presets.contains(&reminder_preset) {
            summary.reminder_presets.push(reminder_preset.clone());
        }

        if let Some(value) = snooze_until.clone() {
            let should_replace = summary
                .snooze_until
                .as_deref()
                .and_then(parse_utc_datetime)
                .map(|current| parse_utc_datetime(&value).map(|candidate| candidate < current).unwrap_or(false))
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
                .map(|current| parse_utc_datetime(&value).map(|candidate| candidate > current).unwrap_or(false))
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
                .map(|current| parse_utc_datetime(&effective_fire_at).map(|candidate| candidate < current).unwrap_or(false))
                .unwrap_or(true);
            if should_replace {
                next_fire_at = Some(effective_fire_at);
                summary.next_task_reminder_id = Some(task_reminder_id);
                summary.next_reminder_preset = Some(reminder_preset.clone());
            }
        }
    }

    if summary.reminder_presets.is_empty() {
        summary.reminder_presets = load_task_reminder_configs(conn, task_id)?
            .into_iter()
            .map(|config| config.preset)
            .collect();
    }
    sort_reminder_presets(&mut summary.reminder_presets);
    Ok(summary)
}

fn sync_task_reminders(
    conn: &Connection,
    task_id: i64,
    event_at: Option<&str>,
    reminder_presets: &[String],
) -> Result<(), String> {
    conn.execute("DELETE FROM todo_task_reminders WHERE task_id=?1", params![task_id])
        .map_err(|e| format!("清理事项提醒失败: {e}"))?;

    let reminder_configs = reminder_configs_from_presets(reminder_presets);
    if reminder_configs.is_empty() {
        conn.execute(
            "UPDATE todo_tasks
             SET remind_at=NULL, snooze_until=NULL, last_notified_at=NULL, updated_at=CURRENT_TIMESTAMP
             WHERE id=?1",
            params![task_id],
        )
        .map_err(|e| format!("重置事项旧提醒字段失败: {e}"))?;
        return Ok(());
    }

    let event_at = event_at.ok_or("设置提醒前需要先提供事件时间或周期规则".to_string())?;
    for config in reminder_configs {
        let remind_at = compute_remind_at(Some(event_at), Some(config.offset_minutes))?
            .ok_or("提醒时间生成失败".to_string())?;
        conn.execute(
            "INSERT INTO todo_task_reminders(task_id, reminder_preset, offset_minutes, remind_at)
             VALUES(?1, ?2, ?3, ?4)",
            params![task_id, config.preset, config.offset_minutes, remind_at],
        )
        .map_err(|e| format!("保存事项提醒失败: {e}"))?;
    }

    conn.execute(
        "UPDATE todo_tasks
         SET remind_at=NULL, snooze_until=NULL, last_notified_at=NULL, updated_at=CURRENT_TIMESTAMP
         WHERE id=?1",
        params![task_id],
    )
    .map_err(|e| format!("更新事项旧提醒字段失败: {e}"))?;
    Ok(())
}

fn sync_template_reminders(
    conn: &Connection,
    template_id: i64,
    reminder_presets: &[String],
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM todo_template_reminders WHERE template_id=?1",
        params![template_id],
    )
    .map_err(|e| format!("清理周期提醒失败: {e}"))?;

    for config in reminder_configs_from_presets(reminder_presets) {
        conn.execute(
            "INSERT INTO todo_template_reminders(template_id, reminder_preset, offset_minutes)
             VALUES(?1, ?2, ?3)",
            params![template_id, config.preset, config.offset_minutes],
        )
        .map_err(|e| format!("保存周期提醒失败: {e}"))?;
    }

    conn.execute(
        "UPDATE todo_templates SET reminder_offset_minutes=NULL, updated_at=CURRENT_TIMESTAMP WHERE id=?1",
        params![template_id],
    )
    .map_err(|e| format!("更新周期旧提醒字段失败: {e}"))?;
    Ok(())
}

fn clear_task_reminder_snooze(conn: &Connection, task_id: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE todo_task_reminders
         SET snooze_until=NULL, updated_at=CURRENT_TIMESTAMP
         WHERE task_id=?1",
        params![task_id],
    )
    .map_err(|e| format!("清理事项稍后提醒失败: {e}"))?;
    Ok(())
}

fn resolve_task_reminder_id_for_snooze(
    conn: &Connection,
    task_id: i64,
    explicit_task_reminder_id: Option<i64>,
) -> Result<i64, String> {
    if let Some(task_reminder_id) = explicit_task_reminder_id {
        let exists = conn
            .query_row(
                "SELECT id FROM todo_task_reminders WHERE id=?1 AND task_id=?2",
                params![task_reminder_id, task_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|e| format!("查询提醒记录失败: {e}"))?;
        return exists.ok_or("提醒记录不存在".to_string());
    }

    conn.query_row(
        "SELECT id
         FROM todo_task_reminders
         WHERE task_id=?1
           AND (last_notified_at IS NULL OR last_notified_at < COALESCE(snooze_until, remind_at))
         ORDER BY COALESCE(snooze_until, remind_at) ASC, offset_minutes ASC, id ASC
         LIMIT 1",
        params![task_id],
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .map_err(|e| format!("查询可稍后提醒失败: {e}"))?
    .ok_or("当前事项没有可稍后的提醒".to_string())
}

fn type_list() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, color, builtin, sort_order, created_at, updated_at
             FROM todo_types ORDER BY builtin DESC, sort_order ASC, id ASC",
        )
        .map_err(|e| format!("鏌ヨ寰呭姙绫诲瀷澶辫触: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "color": row.get::<_, String>(2)?,
                "builtin": row.get::<_, i64>(3)? == 1,
                "sortOrder": row.get::<_, i64>(4)?,
                "createdAt": row.get::<_, String>(5)?,
                "updatedAt": row.get::<_, String>(6)?
            }))
        })
        .map_err(|e| format!("鏄犲皠寰呭姙绫诲瀷澶辫触: {e}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| e.to_string())?);
    }
    Ok(json!({ "items": items }))
}

fn type_upsert(payload: &Value) -> Result<Value, String> {
    let name = parse_string(payload, "name").ok_or("绫诲瀷鍚嶇О涓嶈兘涓虹┖")?;
    let color = parse_string(payload, "color").unwrap_or_else(|| "#409eff".to_string());
    let sort_order = parse_i64(payload, "sortOrder").unwrap_or(0);
    let conn = db_conn()?;
    if let Some(id) = parse_i64(payload, "id") {
        conn.execute(
            "UPDATE todo_types SET name=?1, color=?2, sort_order=?3, updated_at=CURRENT_TIMESTAMP WHERE id=?4",
            params![name, color, sort_order, id],
        )
        .map_err(|e| format!("鏇存柊寰呭姙绫诲瀷澶辫触: {e}"))?;
        Ok(json!({ "ok": true, "id": id }))
    } else {
        conn.execute(
            "INSERT INTO todo_types(name, color, builtin, sort_order) VALUES(?1, ?2, 0, ?3)",
            params![name, color, sort_order],
        )
        .map_err(|e| format!("鏂板寰呭姙绫诲瀷澶辫触: {e}"))?;
        Ok(json!({ "ok": true, "id": conn.last_insert_rowid() }))
    }
}

fn type_delete(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缂哄皯绫诲瀷 id")?;
    let conn = db_conn()?;
    conn.query_row("SELECT 1 FROM todo_types WHERE id=?1", params![id], |_row| Ok(()))
        .map_err(|_| "待办类型不存在".to_string())?;
    conn.execute("UPDATE todo_tasks SET type_id=NULL WHERE type_id=?1", params![id])
        .map_err(|e| format!("瑙ｇ粦浠诲姟绫诲瀷澶辫触: {e}"))?;
    conn.execute(
        "UPDATE todo_templates SET type_id=NULL WHERE type_id=?1",
        params![id],
    )
    .map_err(|e| format!("瑙ｇ粦鍛ㄦ湡绫诲瀷澶辫触: {e}"))?;
    conn.execute("DELETE FROM todo_types WHERE id=?1", params![id])
        .map_err(|e| format!("鍒犻櫎寰呭姙绫诲瀷澶辫触: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn assignee_list() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare("SELECT id, name, created_at, updated_at FROM todo_assignees ORDER BY name COLLATE NOCASE ASC, id ASC")
        .map_err(|e| format!("鏌ヨ鎵ц浜哄け璐? {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "createdAt": row.get::<_, String>(2)?,
                "updatedAt": row.get::<_, String>(3)?
            }))
        })
        .map_err(|e| format!("鏄犲皠鎵ц浜哄け璐? {e}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| e.to_string())?);
    }
    Ok(json!({ "items": items }))
}

fn assignee_upsert(payload: &Value) -> Result<Value, String> {
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

fn assignee_delete(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缺少执行人 id")?;
    let conn = db_conn()?;
    conn.execute("DELETE FROM todo_task_assignees WHERE assignee_id=?1", params![id])
        .map_err(|e| format!("删除任务执行人关联失败: {e}"))?;
    conn.execute(
        "DELETE FROM todo_template_assignees WHERE assignee_id=?1",
        params![id],
    )
    .map_err(|e| format!("删除周期执行人关联失败: {e}"))?;
    conn.execute("DELETE FROM todo_assignees WHERE id=?1", params![id])
        .map_err(|e| format!("删除执行人失败: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn template_to_root_item(template: Value) -> Result<Value, String> {
    let record = template
        .as_object()
        .ok_or("周期事项根记录格式不正确".to_string())?;
    let id = record
        .get("id")
        .and_then(Value::as_i64)
        .ok_or("周期事项根记录缺少 id".to_string())?;
    let title = record.get("title").cloned().unwrap_or_else(|| json!(""));
    let type_id = record.get("typeId").cloned().unwrap_or(Value::Null);
    let type_name = record.get("typeName").cloned().unwrap_or(Value::Null);
    let type_color = record.get("typeColor").cloned().unwrap_or(Value::Null);
    let priority = record.get("priority").cloned().unwrap_or_else(|| json!("P2"));
    let description = record
        .get("description")
        .cloned()
        .unwrap_or_else(|| json!(""));
    let reminder_presets = record
        .get("reminderPresets")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let rule_mode = record
        .get("ruleMode")
        .cloned()
        .unwrap_or_else(|| json!("simple"));
    let rule = record.get("rule").cloned().unwrap_or_else(|| json!({}));
    let cron_expression = record
        .get("cronExpression")
        .cloned()
        .unwrap_or_else(|| json!(""));
    let timezone = record
        .get("timezone")
        .cloned()
        .unwrap_or_else(|| json!("local"));
    let start_at = record.get("startAt").cloned().unwrap_or(Value::Null);
    let end_mode = record
        .get("endMode")
        .cloned()
        .unwrap_or_else(|| json!("never"));
    let end_value = record.get("endValue").cloned().unwrap_or(Value::Null);
    let next_occurrence_at = record
        .get("nextOccurrenceAt")
        .cloned()
        .unwrap_or(Value::Null);
    let generated_count = record
        .get("generatedCount")
        .cloned()
        .unwrap_or_else(|| json!(0));
    let active = record.get("active").cloned().unwrap_or_else(|| json!(true));
    let assignees = record.get("assignees").cloned().unwrap_or_else(|| json!([]));
    let created_at = record.get("createdAt").cloned().unwrap_or_else(|| json!(""));
    let updated_at = record.get("updatedAt").cloned().unwrap_or_else(|| json!(""));
    let display_at = next_occurrence_at
        .as_str()
        .map(|value| json!(value))
        .or_else(|| start_at.as_str().map(|value| json!(value)))
        .unwrap_or_else(|| updated_at.clone());

    Ok(json!({
        "id": id,
        "title": title,
        "typeId": type_id,
        "typeName": type_name,
        "typeColor": type_color,
        "priority": priority,
        "description": description,
        "kind": SERIES_KIND_RECURRING,
        "recordRole": RECORD_ROLE_ROOT,
        "rootId": id,
        "pinned": false,
        "status": Value::Null,
        "eventAt": Value::Null,
        "reminderPresets": reminder_presets,
        "snoozeUntil": Value::Null,
        "lastNotifiedAt": Value::Null,
        "displayAt": display_at,
        "assignees": assignees,
        "isOverdue": false,
        "nextTaskReminderId": Value::Null,
        "nextReminderPreset": Value::Null,
        "recurrence": {
            "startAt": start_at.clone(),
            "ruleMode": rule_mode.clone(),
            "rule": rule.clone(),
            "cronExpression": cron_expression.clone(),
            "timezone": timezone.clone(),
            "endMode": end_mode.clone(),
            "endValue": end_value.clone(),
            "nextOccurrenceAt": next_occurrence_at.clone(),
            "generatedCount": generated_count.clone(),
            "active": active.clone(),
        },
        "canEditFuture": false,
        "createdAt": created_at,
        "updatedAt": updated_at,

    }))
}

fn item_list(payload: &Value) -> Result<Value, String> {
    let status = parse_string(payload, "status");
    let mut items = extract_items(task_list(payload)?)?;

    if status.is_none() {
        let mut root_items = extract_items(template_list()?)?
            .into_iter()
            .map(template_to_root_item)
            .collect::<Result<Vec<_>, _>>()?;
        items.append(&mut root_items);
    }

    sort_item_rows(&mut items);
    Ok(json!({ "items": items }))
}

fn item_create(payload: &Value) -> Result<Value, String> {
    if parse_item_kind(payload) == SERIES_KIND_RECURRING {
        let result = template_create(payload)?;
        let conn = db_conn()?;
        generate_recurring_instances(&conn, Utc::now())?;
        return Ok(result);
    }
    task_create(payload)
}

fn item_upsert(payload: &Value) -> Result<Value, String> {
    if parse_i64(payload, "id").is_some() {
        item_update(payload)
    } else {
        item_create(payload)
    }
}

fn item_update(payload: &Value) -> Result<Value, String> {
    let next_kind = parse_item_kind(payload);
    let mut conn = db_conn()?;
    let (current_kind, task_id, template_id) = resolve_existing_update_target(&conn, payload)?;

    if current_kind != next_kind {
        return convert_item_kind(&mut conn, payload, &current_kind, task_id, template_id);
    }

    // 检查是否编辑已完成的周期事项实例（已完成实例只编辑自身，不影响未来）
    let is_completed_instance = if next_kind == SERIES_KIND_RECURRING {
        if let Some(tid) = task_id {
            conn.query_row(
                "SELECT status FROM todo_tasks WHERE id = ?1",
                params![tid],
                |row| {
                    let status: String = row.get(0)?;
                    Ok(status == STATUS_COMPLETED || status == STATUS_CANCELED)
                },
            ).optional().map_err(|e| format!("查询事项状态失败: {e}"))?.unwrap_or(false)
        } else {
            false
        }
    } else {
        false
    };

    drop(conn);
    if next_kind == SERIES_KIND_RECURRING && !is_completed_instance {
        let scope = parse_scope(payload);
        let conn = db_conn()?;
        let template_id = resolve_recurring_template_id(&conn, payload)?;
        if scope == SCOPE_FUTURE_INSTANCES || is_root_record(payload) {
            let template_id = template_id.ok_or("缺少周期事项根记录 id")?;
            let mut next_payload = payload.clone();
            if let Some(obj) = next_payload.as_object_mut() {
                obj.insert("id".to_string(), json!(template_id));
            }
            return template_update(&next_payload);
        }
    }

    task_update(payload)
}

fn item_change_status(payload: &Value) -> Result<Value, String> {
    if parse_scope(payload) == SCOPE_FUTURE_INSTANCES
        || (parse_item_kind(payload) == SERIES_KIND_RECURRING && is_root_record(payload))
    {
        return Err("周期事项根记录不支持直接修改状态，请改用启停操作".to_string());
    }
    task_change_status(payload)
}

fn item_snooze(payload: &Value) -> Result<Value, String> {
    if parse_item_kind(payload) == SERIES_KIND_RECURRING && is_root_record(payload) {
        return Err("周期事项根记录不支持稍后提醒".to_string());
    }
    task_snooze(payload)
}

fn item_toggle_pin(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缺少事项 id")?;
    let conn = db_conn()?;
    let changed = conn
        .execute(
            "UPDATE todo_tasks
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

fn item_toggle_active(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let template_id = resolve_recurring_template_id(&conn, payload)?
        .ok_or("仅周期事项支持启停")?;
    template_toggle_active(&json!({
        "id": template_id,
        "active": payload.get("active").and_then(Value::as_bool).unwrap_or(true),
    }))
}

fn item_delete(payload: &Value) -> Result<Value, String> {
    if parse_scope(payload) == SCOPE_FUTURE_INSTANCES {
        let conn = db_conn()?;
        let template_id = resolve_recurring_template_id(&conn, payload)?
            .ok_or("缺少周期事项根记录 id")?;
        return template_toggle_active(&json!({ "id": template_id, "active": false }));
    }

    if parse_item_kind(payload) == SERIES_KIND_RECURRING && is_root_record(payload) {
        let conn = db_conn()?;
        let template_id = resolve_recurring_template_id(&conn, payload)?
            .ok_or("缺少周期事项根记录 id")?;
        return template_delete(&json!({ "id": template_id }));
    }
    task_delete(payload)
}

fn task_list(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let status = parse_string(payload, "status");
    let mut items = Vec::new();

    let mut stmt = if status.is_some() {
        conn.prepare(
            "SELECT t.id, t.title, t.type_id, t.priority, t.description, t.status,
                    t.event_at, t.remind_at, t.snooze_until, t.last_notified_at,
                    t.source_template_id, t.created_at, t.updated_at,
                    ty.name, ty.color, COALESCE(tp.series_kind, 'one_off'), t.pinned
             FROM todo_tasks t
             LEFT JOIN todo_types ty ON ty.id = t.type_id
             LEFT JOIN todo_templates tp ON tp.id = t.source_template_id
             WHERE t.status = ?1
             ORDER BY t.pinned DESC,
                      CASE t.priority WHEN 'P0' THEN 0 WHEN 'P1' THEN 1 WHEN 'P2' THEN 2 ELSE 3 END,
                      COALESCE(t.event_at, t.updated_at),
                      t.id DESC",
        )
    } else {
        conn.prepare(
            "SELECT t.id, t.title, t.type_id, t.priority, t.description, t.status,
                    t.event_at, t.remind_at, t.snooze_until, t.last_notified_at,
                    t.source_template_id, t.created_at, t.updated_at,
                    ty.name, ty.color, COALESCE(tp.series_kind, 'one_off'), t.pinned
             FROM todo_tasks t
             LEFT JOIN todo_types ty ON ty.id = t.type_id
             LEFT JOIN todo_templates tp ON tp.id = t.source_template_id
             ORDER BY t.pinned DESC,
                      CASE t.priority WHEN 'P0' THEN 0 WHEN 'P1' THEN 1 WHEN 'P2' THEN 2 ELSE 3 END,
                      COALESCE(t.event_at, t.updated_at),
                      t.id DESC",
        )
    }
    .map_err(|e| format!("鏌ヨ浠诲姟澶辫触: {e}"))?;

    let mut rows_data: Vec<(i64, Value)> = Vec::new();
    if let Some(status_value) = status {
        let rows = stmt
            .query_map(params![status_value], |row| {
                Ok((row.get::<_, i64>(0)?, row_to_task_json(row)?))
            })
            .map_err(|e| format!("鏄犲皠浠诲姟澶辫触: {e}"))?;
        for row in rows {
            rows_data.push(row.map_err(|e| e.to_string())?);
        }
    } else {
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row_to_task_json(row)?)))
            .map_err(|e| format!("鏄犲皠浠诲姟澶辫触: {e}"))?;
        for row in rows {
            rows_data.push(row.map_err(|e| e.to_string())?);
        }
    }

    for (task_id, mut task) in rows_data {
        let assignees = load_task_assignees(&conn, task_id)?;
        let reminder_summary = load_task_reminder_summary(&conn, task_id)?;
        if let Some(obj) = task.as_object_mut() {
            obj.insert("assignees".to_string(), json!(assignees));
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
            let is_overdue = obj
                .get("status")
                .and_then(Value::as_str)
                .map(|s| s == STATUS_PENDING || s == STATUS_IN_PROGRESS)
                .unwrap_or(false)
                && obj
                    .get("eventAt")
                    .and_then(Value::as_str)
                    .and_then(parse_rfc3339)
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc) < Utc::now())
                    .unwrap_or(false);
            obj.insert("isOverdue".to_string(), json!(is_overdue));
        }
        items.push(task);
    }

    Ok(json!({ "items": items }))
}

fn row_to_task_json(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let item_id = row.get::<_, i64>(0)?;
    let series_id = row.get::<_, Option<i64>>(10)?;
    let series_kind = row.get::<_, String>(15)?;
    let pinned = row.get::<_, i64>(16)? != 0;
    let is_recurring = series_kind == SERIES_KIND_RECURRING;
    let record_role = if is_recurring && series_id.is_some() {
        RECORD_ROLE_OCCURRENCE
    } else {
        RECORD_ROLE_ROOT
    };
    let event_at = row.get::<_, Option<String>>(6)?;
    let display_at = event_at.clone();
    let status = row.get::<_, String>(5)?;
    Ok(json!({
        "id": item_id,
        "title": row.get::<_, String>(1)?,
        "typeId": row.get::<_, Option<i64>>(2)?,
        "priority": row.get::<_, String>(3)?,
        "description": row.get::<_, String>(4)?,
        "kind": series_kind,
        "recordRole": record_role,
        "rootId": series_id.unwrap_or(item_id),
        "pinned": pinned,
        "status": status,
        "eventAt": event_at,
        "reminderPresets": json!([]),
        "snoozeUntil": Value::Null,
        "lastNotifiedAt": Value::Null,
        "recurrence": Value::Null,

        "canEditFuture": series_id.is_some()
            && is_recurring
            && (status == STATUS_PENDING || status == STATUS_IN_PROGRESS),
        "displayAt": display_at,
        "createdAt": row.get::<_, String>(11)?,
        "updatedAt": row.get::<_, String>(12)?,
        "typeName": row.get::<_, Option<String>>(13)?,
        "nextTaskReminderId": Value::Null,
        "nextReminderPreset": Value::Null,
        "typeColor": row.get::<_, Option<String>>(14)?
    }))
}

fn update_task_base_fields(
    conn: &Connection,
    task_id: i64,
    title: &str,
    type_id: Option<i64>,
    priority: &str,
    description: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE todo_tasks
         SET title=?1, type_id=?2, priority=?3, description=?4, updated_at=CURRENT_TIMESTAMP
         WHERE id=?5",
        params![title, type_id, priority, description, task_id],
    )
    .map_err(|e| format!("更新事项基础信息失败: {e}"))?;
    Ok(())
}

fn create_task_record(
    conn: &Connection,
    title: &str,
    type_id: Option<i64>,
    priority: &str,
    description: &str,
    status: &str,
    event_at: Option<&str>,
    assignee_ids: &[i64],
    reminder_presets: &[String],
    source_template_id: Option<i64>,
) -> Result<i64, String> {
    if event_at.is_none() && !reminder_presets.is_empty() {
        return Err("设置提醒前需要先提供事件时间或周期规则".to_string());
    }

    conn.execute(
        "INSERT INTO todo_tasks(title, type_id, priority, description, status, event_at, remind_at, source_template_id)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            title,
            type_id,
            priority,
            description,
            status,
            event_at,
            Option::<String>::None,
            source_template_id,
        ],
    )
    .map_err(|e| format!("创建任务失败: {e}"))?;
    let task_id = conn.last_insert_rowid();
    sync_task_assignees(conn, task_id, assignee_ids)?;
    sync_task_reminders(conn, task_id, event_at, reminder_presets)?;
    Ok(task_id)
}

fn create_recurring_template_record(
    conn: &Connection,
    title: &str,
    type_id: Option<i64>,
    priority: &str,
    description: &str,
    rule_mode: &str,
    rule: &Value,
    cron_expression: &str,
    timezone: &str,
    start_at: &str,
    end_mode: &str,
    end_value: Option<&str>,
    next_occurrence_at: Option<&str>,
    generated_count: i64,
    active: bool,
    assignee_ids: &[i64],
    reminder_presets: &[String],
) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO todo_templates
         (title, type_id, priority, description, rule_mode, rule_json, cron_expression,
          timezone, start_at, end_mode, end_value, next_occurrence_at, generated_count, active, series_kind,
          reminder_offset_minutes)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            title,
            type_id,
            priority,
            description,
            rule_mode,
            serde_json::to_string(rule).map_err(|e| format!("周期规则序列化失败: {e}"))?,
            cron_expression,
            timezone,
            start_at,
            end_mode,
            end_value,
            next_occurrence_at,
            generated_count,
            if active { 1 } else { 0 },
            SERIES_KIND_RECURRING,
            Option::<i64>::None,
        ],
    )
    .map_err(|e| format!("创建周期模板失败: {e}"))?;

    let template_id = conn.last_insert_rowid();
    sync_template_assignees(conn, template_id, assignee_ids)?;
    sync_template_reminders(conn, template_id, reminder_presets)?;
    Ok(template_id)
}

fn delete_template_by_id(conn: &Connection, template_id: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE todo_tasks SET source_template_id=NULL WHERE source_template_id=?1",
        params![template_id],
    )
    .map_err(|e| format!("解绑周期实例失败: {e}"))?;
    conn.execute(
        "DELETE FROM todo_template_assignees WHERE template_id=?1",
        params![template_id],
    )
    .map_err(|e| format!("删除周期执行人失败: {e}"))?;
    conn.execute(
        "DELETE FROM todo_template_reminders WHERE template_id=?1",
        params![template_id],
    )
    .map_err(|e| format!("删除周期提醒失败: {e}"))?;
    conn.execute(
        "DELETE FROM todo_templates WHERE id=?1",
        params![template_id],
    )
    .map_err(|e| format!("删除周期模板失败: {e}"))?;
    Ok(())
}

fn convert_one_off_task_to_recurring(
    conn: &mut Connection,
    payload: &Value,
    task_id: i64,
) -> Result<Value, String> {
    let title = parse_string(payload, "title").ok_or("任务标题不能为空")?;
    let type_id = parse_i64(payload, "typeId");
    let priority = normalize_priority(payload.get("priority").and_then(Value::as_str))?;
    let description = payload
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let recurrence = recurrence_payload(payload);
    let rule_mode = normalize_rule_mode(payload, "simple");
    let rule = recurrence.get("rule").cloned().unwrap_or_else(|| json!({}));
    let cron_expression = resolve_cron_expression(&rule_mode, &rule)?;
    let start_at = parse_start_datetime(payload)?.ok_or("周期事项开始时间不能为空")?;
    let start_at_dt = parse_utc_datetime(&start_at).ok_or("开始时间格式不正确")?;
    let timezone = recurrence
        .get("timezone")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| "local".to_string());
    let (end_mode, end_value) = parse_end_rule(payload)?;
    let reminder_presets = parse_reminder_presets(payload)?.unwrap_or_default();
    let assignee_ids = parse_assignee_ids(payload);
    let next_occurrence = compute_next_occurrence_with_start(
        &cron_expression,
        &timezone,
        Some(&start_at),
        start_at_dt + Duration::seconds(1),
    )?
    .map(|dt| dt.to_rfc3339());

    let tx = conn
        .transaction()
        .map_err(|e| format!("开启事项类型转换事务失败: {e}"))?;
    let template_id = create_recurring_template_record(
        &tx,
        &title,
        type_id,
        &priority,
        &description,
        &rule_mode,
        &rule,
        &cron_expression,
        &timezone,
        &start_at,
        &end_mode,
        end_value.as_deref(),
        next_occurrence.as_deref(),
        1,
        true,
        &assignee_ids,
        &reminder_presets,
    )?;

    update_task_base_fields(&tx, task_id, &title, type_id, &priority, &description)?;
    tx.execute(
        "UPDATE todo_tasks
         SET event_at=?1, source_template_id=?2, remind_at=NULL, snooze_until=NULL, last_notified_at=NULL, updated_at=CURRENT_TIMESTAMP
         WHERE id=?3",
        params![start_at, template_id, task_id],
    )
    .map_err(|e| format!("更新首个周期事项实例失败: {e}"))?;
    sync_task_assignees(&tx, task_id, &assignee_ids)?;
    sync_task_reminders(&tx, task_id, Some(&start_at), &reminder_presets)?;
    tx.commit()
        .map_err(|e| format!("提交事项类型转换失败: {e}"))?;

    generate_recurring_instances(conn, Utc::now())?;

    Ok(json!({ "ok": true, "rootId": template_id, "id": task_id }))
}

fn convert_recurring_item_to_one_off(
    conn: &mut Connection,
    payload: &Value,
    task_id: Option<i64>,
    template_id: i64,
) -> Result<Value, String> {
    let title = parse_string(payload, "title").ok_or("任务标题不能为空")?;
    let type_id = parse_i64(payload, "typeId");
    let priority = normalize_priority(payload.get("priority").and_then(Value::as_str))?;
    let description = payload
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let scope = parse_scope(payload);
    let fallback_event_at = if scope == SCOPE_FUTURE_INSTANCES || task_id.is_none() {
        load_template_anchor_event_at(conn, template_id)?
    } else {
        let task_id = task_id.ok_or("缺少周期事项实例 id")?;
        let snapshot = load_task_snapshot(conn, task_id)?;
        if snapshot.source_template_id.is_none() {
            return Err("当前事项已不是周期实例，无法再改为单次事项".to_string());
        }
        snapshot.event_at
    };
    let next_payload = ensure_event_at_when_missing(payload, fallback_event_at.as_deref())?;
    let event_at = parse_event_datetime(&next_payload, "eventAt")?;
    let reminder_presets = parse_reminder_presets(payload)?.unwrap_or_default();
    let assignee_ids = parse_assignee_ids(payload);
    if event_at.is_none() && !reminder_presets.is_empty() {
        return Err("设置提醒前需要先提供事件时间或周期规则".to_string());
    }

    let tx = conn
        .transaction()
        .map_err(|e| format!("开启事项类型转换事务失败: {e}"))?;
    let detach_current_instance = task_id.is_some() && scope != SCOPE_FUTURE_INSTANCES;
    let resulting_task_id = if let Some(task_id) = task_id.filter(|_| detach_current_instance) {
        update_task_base_fields(&tx, task_id, &title, type_id, &priority, &description)?;
        tx.execute(
            "UPDATE todo_tasks
             SET event_at=?1,
                 source_template_id=NULL,
                 remind_at=NULL,
                 snooze_until=NULL,
                 last_notified_at=NULL,
                 updated_at=CURRENT_TIMESTAMP
             WHERE id=?2",
            params![event_at.as_deref(), task_id],
        )
        .map_err(|e| format!("更新单次事项时间失败: {e}"))?;
        sync_task_assignees(&tx, task_id, &assignee_ids)?;
        sync_task_reminders(&tx, task_id, event_at.as_deref(), &reminder_presets)?;
        task_id
    } else {
        create_task_record(
            &tx,
            &title,
            type_id,
            &priority,
            &description,
            STATUS_PENDING,
            event_at.as_deref(),
            &assignee_ids,
            &reminder_presets,
            None,
        )?
    };

    if !detach_current_instance {
        delete_template_by_id(&tx, template_id)?;
    }
    tx.commit()
        .map_err(|e| format!("提交事项类型转换失败: {e}"))?;

    Ok(json!({ "ok": true, "id": resulting_task_id, "rootId": resulting_task_id }))
}

fn convert_item_kind(
    conn: &mut Connection,
    payload: &Value,
    current_kind: &str,
    task_id: Option<i64>,
    template_id: Option<i64>,
) -> Result<Value, String> {
    let next_kind = parse_item_kind(payload);
    let scope = parse_scope(payload);
    match resolve_item_update_strategy(current_kind, next_kind.as_str(), &scope, is_root_record(payload)) {
        ItemUpdateStrategy::ConvertTaskToRecurring => {
            let task_id = task_id.ok_or("缺少待转换的单次事项 id")?;
            convert_one_off_task_to_recurring(conn, payload, task_id)
        }
        ItemUpdateStrategy::DetachOccurrenceToOneOff
        | ItemUpdateStrategy::ConvertSeriesToOneOff => {
            let template_id = template_id.ok_or("缺少周期事项根记录 id")?;
            convert_recurring_item_to_one_off(conn, payload, task_id, template_id)
        }
        _ => Err("不支持的事项类型转换".to_string()),
    }
}

fn task_create(payload: &Value) -> Result<Value, String> {
    let title = parse_string(payload, "title").ok_or("任务标题不能为空")?;
    let type_id = parse_i64(payload, "typeId");
    let priority = normalize_priority(payload.get("priority").and_then(Value::as_str))?;
    let description = payload
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let event_at = parse_event_datetime(payload, "eventAt")?;
    let reminder_presets = parse_reminder_presets(payload)?
        .unwrap_or_else(|| vec![REMINDER_PRESET_ON_TIME.to_string()]);
    if event_at.is_none() && !reminder_presets.is_empty() {
        return Err("设置提醒前需要先提供事件时间或周期规则".to_string());
    }
    let source_template_id = parse_root_id(payload);
    let assignee_ids = parse_assignee_ids(payload);

    let conn = db_conn()?;
    let id = create_task_record(
        &conn,
        &title,
        type_id,
        &priority,
        &description,
        STATUS_PENDING,
        event_at.as_deref(),
        &assignee_ids,
        &reminder_presets,
        source_template_id,
    )?;
    Ok(json!({
        "ok": true,
        "id": id,
        "rootId": source_template_id.unwrap_or(id),
    }))
}

fn task_update(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缺少任务 id")?;
    let conn = db_conn()?;
    if let Some(title) = parse_string(payload, "title") {
        conn.execute(
            "UPDATE todo_tasks SET title=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![title, id],
        )
        .map_err(|e| format!("更新任务标题失败: {e}"))?;
    }
    if payload.get("typeId").is_some() {
        conn.execute(
            "UPDATE todo_tasks SET type_id=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![parse_i64(payload, "typeId"), id],
        )
        .map_err(|e| format!("更新任务类型失败: {e}"))?;
    }
    if payload.get("priority").is_some() {
        let priority = normalize_priority(payload.get("priority").and_then(Value::as_str))?;
        conn.execute(
            "UPDATE todo_tasks SET priority=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![priority, id],
        )
        .map_err(|e| format!("更新任务优先级失败: {e}"))?;
    }
    if payload.get("description").is_some() {
        let description = payload
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        conn.execute(
            "UPDATE todo_tasks SET description=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![description, id],
        )
        .map_err(|e| format!("更新任务描述失败: {e}"))?;
    }
    let reminder_presets_update = parse_reminder_presets(payload)?;
    if payload.get("eventAt").is_some() || reminder_presets_update.is_some() {
        let current_event_at = load_task_event_at(&conn, id)?;
        let next_event_at = if payload.get("eventAt").is_some() {
            parse_event_datetime(payload, "eventAt")?
        } else {
            current_event_at.clone()
        };
        let next_reminder_presets = if let Some(reminder_presets) = reminder_presets_update {
            reminder_presets
        } else {
            load_task_reminder_configs(&conn, id)?
                .into_iter()
                .map(|config| config.preset)
                .collect::<Vec<_>>()
        };
        if next_event_at.is_none() && !next_reminder_presets.is_empty() {
            return Err("设置提醒前需要先提供事件时间或周期规则".to_string());
        }
        conn.execute(
            "UPDATE todo_tasks
             SET event_at=?1,
                 remind_at=NULL,
                 snooze_until=NULL,
                 last_notified_at=NULL,
                 updated_at=CURRENT_TIMESTAMP
             WHERE id=?2",
            params![next_event_at, id],
        )
        .map_err(|e| format!("更新事项时间失败: {e}"))?;
        sync_task_reminders(&conn, id, next_event_at.as_deref(), &next_reminder_presets)?;
    }
    if payload.get("status").is_some() {
        let next = normalize_status(
            payload
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or(STATUS_PENDING),
        )?;
        let current: String = conn
            .query_row("SELECT status FROM todo_tasks WHERE id=?1", params![id], |row| {
                row.get(0)
            })
            .map_err(|_| "任务不存在".to_string())?;
        if !can_transit(&current, &next) {
            return Err("状态流转不合法".to_string());
        }
        conn.execute(
            "UPDATE todo_tasks SET status=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![next, id],
        )
        .map_err(|e| format!("更新任务状态失败: {e}"))?;
    }
    if payload.get("assigneeIds").is_some() {
        sync_task_assignees(&conn, id, &parse_assignee_ids(payload))?;
    }

    Ok(json!({ "ok": true }))
}

fn task_change_status(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缂哄皯浠诲姟 id")?;
    let next = normalize_status(
        payload
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or(STATUS_PENDING),
    )?;
    let mut conn = db_conn()?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("开启状态更新事务失败: {e}"))?;
    task_change_status_with_conn(&tx, id, &next)?;
    tx.commit().map_err(|e| format!("提交状态更新事务失败: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn task_change_status_with_conn(conn: &Connection, id: i64, next: &str) -> Result<(), String> {
    let current: String = conn
        .query_row("SELECT status FROM todo_tasks WHERE id=?1", params![id], |row| {
            row.get(0)
        })
        .map_err(|_| "任务不存在".to_string())?;
    if !can_transit(&current, next) {
        return Err("鐘舵€佹祦杞笉鍚堟硶".to_string());
    }
    conn.execute(
        "UPDATE todo_tasks
         SET status=?1,
             snooze_until=CASE WHEN ?1 IN ('completed','canceled') THEN NULL ELSE snooze_until END,
             updated_at=CURRENT_TIMESTAMP
         WHERE id=?2",
        params![next, id],
    )
    .map_err(|e| format!("鏇存柊浠诲姟鐘舵€佸け璐? {e}"))?;
    if next == STATUS_COMPLETED || next == STATUS_CANCELED {
        clear_task_reminder_snooze(conn, id)?;
    }
    if next == STATUS_COMPLETED {
        maybe_generate_next_recurring_task_after_completion(conn, id, &current)?;
    }
    Ok(())
}

fn task_snooze(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缂哄皯浠诲姟 id")?;
    let minutes = parse_i64(payload, "minutes").unwrap_or(10).clamp(1, 24 * 60);
    let conn = db_conn()?;
    let status: String = conn
        .query_row("SELECT status FROM todo_tasks WHERE id=?1", params![id], |row| {
            row.get(0)
        })
        .map_err(|_| "任务不存在".to_string())?;
    if status == STATUS_COMPLETED || status == STATUS_CANCELED {
        return Err("已完成或已取消任务不能稍后提醒".to_string());
    }
    let task_reminder_id = resolve_task_reminder_id_for_snooze(
        &conn,
        id,
        parse_i64(payload, "taskReminderId"),
    )?;
    let snooze_until = (Utc::now() + Duration::minutes(minutes)).to_rfc3339();
    conn.execute(
        "UPDATE todo_task_reminders
         SET snooze_until=?1, last_notified_at=NULL, updated_at=CURRENT_TIMESTAMP
         WHERE id=?2 AND task_id=?3",
        params![snooze_until, task_reminder_id, id],
    )
    .map_err(|e| format!("绋嶅悗鎻愰啋澶辫触: {e}"))?;
    Ok(json!({ "ok": true, "snoozeUntil": snooze_until, "taskReminderId": task_reminder_id }))
}

fn task_delete(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缂哄皯浠诲姟 id")?;
    let conn = db_conn()?;
    conn.execute("DELETE FROM todo_task_assignees WHERE task_id=?1", params![id])
        .map_err(|e| format!("鍒犻櫎浠诲姟鎵ц浜哄叧鑱斿け璐? {e}"))?;
    conn.execute("DELETE FROM todo_task_reminders WHERE task_id=?1", params![id])
        .map_err(|e| format!("删除任务提醒失败: {e}"))?;
    conn.execute("DELETE FROM todo_reminder_events WHERE task_id=?1", params![id])
        .map_err(|e| format!("鍒犻櫎浠诲姟鎻愰啋浜嬩欢澶辫触: {e}"))?;
    conn.execute("DELETE FROM todo_tasks WHERE id=?1", params![id])
        .map_err(|e| format!("鍒犻櫎浠诲姟澶辫触: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn normalize_rule_mode(payload: &Value, fallback: &str) -> String {
    recurrence_payload(payload)
        .get("ruleMode")
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .trim()
        .to_lowercase()
}

fn build_simple_cron_expression(rule: &Value) -> Result<String, String> {
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
    let time = rule.get("time").and_then(Value::as_str).unwrap_or("09:00").trim();
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
            // 注意：本项目前端 weekday 语义为 1=周一 ... 7=周日（中文常用习惯）
            // Rust 端使用的 cron crate 对 day-of-week 的数字语义并不一致（1=周日 ... 7=周六），
            // 为避免歧义，这里统一输出英文星期缩写（Mon..Sun）。
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

fn resolve_cron_expression(rule_mode: &str, rule: &Value) -> Result<String, String> {
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

fn parse_end_rule(payload: &Value) -> Result<(String, Option<String>), String> {
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
            let end_payload = json!({ "endValue": recurrence.get("endValue").cloned().unwrap_or(Value::Null) });
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

fn compute_next_occurrence(
    cron_expression: &str,
    timezone: &str,
    after_utc: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, String> {
    let schedule = Schedule::from_str(cron_expression).map_err(|e| format!("周期表达式无效: {e}"))?;

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
            Ok(schedule.after(&tz_after).next().map(|dt| dt.with_timezone(&Utc)))
        }
        Err(_) => {
            let local_after = after_utc.with_timezone(&Local);
            let next = schedule.after(&local_after).next();
            Ok(next.map(|dt| dt.with_timezone(&Utc)))
        }
    }
}

fn sync_template_assignees(conn: &Connection, template_id: i64, ids: &[i64]) -> Result<(), String> {
    conn.execute(
        "DELETE FROM todo_template_assignees WHERE template_id = ?1",
        params![template_id],
    )
    .map_err(|e| format!("清理周期执行人失败: {e}"))?;
    for id in ids {
        conn.execute(
            "INSERT OR IGNORE INTO todo_template_assignees(template_id, assignee_id) VALUES(?1, ?2)",
            params![template_id, id],
        )
        .map_err(|e| format!("保存周期执行人失败: {e}"))?;
    }
    Ok(())
}

fn load_template_assignees(conn: &Connection, template_id: i64) -> Result<Vec<Value>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT a.id, a.name
             FROM todo_template_assignees ta
             JOIN todo_assignees a ON a.id = ta.assignee_id
             WHERE ta.template_id = ?1
             ORDER BY a.name COLLATE NOCASE ASC",
        )
        .map_err(|e| format!("查询周期执行人失败: {e}"))?;
    let rows = stmt
        .query_map(params![template_id], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?
            }))
        })
        .map_err(|e| format!("映射周期执行人失败: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}
fn template_list() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT
                tp.id, tp.title, tp.type_id, tp.priority, tp.description,
                tp.rule_mode, tp.rule_json, tp.cron_expression, tp.timezone,
                tp.start_at, tp.end_mode, tp.end_value, tp.next_occurrence_at,
                tp.generated_count, tp.active, tp.reminder_offset_minutes,
                tp.created_at, tp.updated_at,
                ty.name AS type_name, ty.color AS type_color
             FROM todo_templates tp
             LEFT JOIN todo_types ty ON ty.id = tp.type_id
             WHERE COALESCE(tp.series_kind, 'recurring') = 'recurring'
             ORDER BY tp.active DESC, tp.id DESC",
        )
        .map_err(|e| format!("查询周期模板失败: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                json!({
                    "id": row.get::<_, i64>(0)?,
                    "title": row.get::<_, String>(1)?,
                    "typeId": row.get::<_, Option<i64>>(2)?,
                    "priority": row.get::<_, String>(3)?,
                    "description": row.get::<_, String>(4)?,
                    "ruleMode": row.get::<_, String>(5)?,
                    "rule": serde_json::from_str::<Value>(&row.get::<_, String>(6)?).unwrap_or_else(|_| json!({})),
                    "cronExpression": row.get::<_, String>(7)?,
                    "timezone": row.get::<_, String>(8)?,
                    "startAt": row.get::<_, Option<String>>(9)?,
                    "endMode": row.get::<_, String>(10)?,
                    "endValue": row.get::<_, Option<String>>(11)?,
                    "nextOccurrenceAt": row.get::<_, Option<String>>(12)?,
                    "generatedCount": row.get::<_, i64>(13)?,
                    "active": row.get::<_, i64>(14)? == 1,
                    "reminderPresets": json!([]),
                    "seriesKind": SERIES_KIND_RECURRING,
                    "createdAt": row.get::<_, String>(16)?,
                    "updatedAt": row.get::<_, String>(17)?,
                    "typeName": row.get::<_, Option<String>>(18)?,
                    "typeColor": row.get::<_, Option<String>>(19)?
                }),
            ))
        })
        .map_err(|e| format!("映射周期模板失败: {e}"))?;

    let mut items = Vec::new();
    for row in rows {
        let (template_id, mut item) = row.map_err(|e| e.to_string())?;
        let assignees = load_template_assignees(&conn, template_id)?;
        let reminder_presets = load_template_reminder_configs(&conn, template_id)?
            .into_iter()
            .map(|config| config.preset)
            .collect::<Vec<_>>();
        if let Some(obj) = item.as_object_mut() {
            obj.insert("assignees".to_string(), json!(assignees));
            obj.insert("reminderPresets".to_string(), json!(reminder_presets));
        }
        items.push(item);
    }

    Ok(json!({ "items": items }))
}

fn template_create(payload: &Value) -> Result<Value, String> {
    let title = parse_string(payload, "title").ok_or("周期事件标题不能为空")?;
    let type_id = parse_i64(payload, "typeId");
    let priority = normalize_priority(payload.get("priority").and_then(Value::as_str))?;
    let description = payload
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let recurrence = recurrence_payload(payload);
    let rule_mode = normalize_rule_mode(payload, "simple");
    let rule = recurrence.get("rule").cloned().unwrap_or_else(|| json!({}));
    let cron_expression = resolve_cron_expression(&rule_mode, &rule)?;
    let start_at = parse_start_datetime(payload)?.ok_or("周期事项开始时间不能为空")?;
    let start_at_dt = parse_utc_datetime(&start_at).ok_or("开始时间格式不正确")?;
    let timezone = recurrence
        .get("timezone")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| "local".to_string());
    let (end_mode, end_value) = parse_end_rule(payload)?;
    let reminder_presets = parse_reminder_presets(payload)?
        .unwrap_or_else(|| vec![REMINDER_PRESET_ON_TIME.to_string()]);
    let assignee_ids = parse_assignee_ids(payload);

    let next_occurrence = compute_next_occurrence_with_start(
        &cron_expression,
        &timezone,
        Some(&start_at),
        start_at_dt - Duration::seconds(1),
    )?
        .map(|dt| dt.to_rfc3339());

    let conn = db_conn()?;
    conn.execute(
        "INSERT INTO todo_templates
         (title, type_id, priority, description, rule_mode, rule_json, cron_expression,
          timezone, start_at, end_mode, end_value, next_occurrence_at, generated_count, active, series_kind,
          reminder_offset_minutes)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, 1, ?13, ?14)",
        params![
            title,
            type_id,
            priority,
            description,
            rule_mode,
            serde_json::to_string(&rule).map_err(|e| format!("周期规则序列化失败: {e}"))?,
            cron_expression,
            timezone,
            start_at,
            end_mode,
            end_value,
            next_occurrence,
            SERIES_KIND_RECURRING,
            Option::<i64>::None,
        ],
    )
    .map_err(|e| format!("创建周期模板失败: {e}"))?;

    let id = conn.last_insert_rowid();
    sync_template_assignees(&conn, id, &assignee_ids)?;
    sync_template_reminders(&conn, id, &reminder_presets)?;
    Ok(json!({ "ok": true, "id": id, "rootId": id }))
}

fn template_update(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缺少周期模板 id")?;
    let conn = db_conn()?;
    let recurrence = recurrence_payload(payload);

    if let Some(title) = parse_string(payload, "title") {
        conn.execute(
            "UPDATE todo_templates SET title=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![title, id],
        )
        .map_err(|e| format!("更新任务标题失败: {e}"))?;
    }
    if payload.get("typeId").is_some() {
        conn.execute(
            "UPDATE todo_templates SET type_id=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![parse_i64(payload, "typeId"), id],
        )
        .map_err(|e| format!("更新任务类型失败: {e}"))?;
    }
    if payload.get("priority").is_some() {
        let priority = normalize_priority(payload.get("priority").and_then(Value::as_str))?;
        conn.execute(
            "UPDATE todo_templates SET priority=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![priority, id],
        )
        .map_err(|e| format!("更新周期优先级失败: {e}"))?;
    }
    if payload.get("description").is_some() {
        let description = payload
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        conn.execute(
            "UPDATE todo_templates SET description=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![description, id],
        )
        .map_err(|e| format!("更新周期描述失败: {e}"))?;
    }
    if let Some(reminder_presets) = parse_reminder_presets(payload)? {
        sync_template_reminders(&conn, id, &reminder_presets)?;
    }

    let mut recompute_next = false;
    if has_start_datetime(payload) {
        let start_at = parse_start_datetime(payload)?.ok_or("周期事项开始时间不能为空")?;
        conn.execute(
            "UPDATE todo_templates SET start_at=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![start_at, id],
        )
        .map_err(|e| format!("更新周期开始时间失败: {e}"))?;
        recompute_next = true;
    }
    if payload.get("recurrence").is_some()
        || recurrence.get("ruleMode").is_some()
        || recurrence.get("rule").is_some()
        || recurrence.get("timezone").is_some()
    {
        let current_rule_mode: String = conn
            .query_row(
                "SELECT rule_mode FROM todo_templates WHERE id=?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(|_| "周期模板不存在".to_string())?;
        let current_rule_json: String = conn
            .query_row(
                "SELECT rule_json FROM todo_templates WHERE id=?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(|_| "周期模板不存在".to_string())?;
        let rule_mode = normalize_rule_mode(payload, &current_rule_mode);
        let rule = recurrence.get("rule").cloned().unwrap_or_else(|| {
            serde_json::from_str::<Value>(&current_rule_json).unwrap_or_else(|_| json!({}))
        });
        let cron_expression = resolve_cron_expression(&rule_mode, &rule)?;
        let timezone = recurrence
            .get("timezone")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| {
                conn.query_row(
                    "SELECT timezone FROM todo_templates WHERE id=?1",
                    params![id],
                    |row| row.get(0),
                )
                .unwrap_or_else(|_| "local".to_string())
            });

        conn.execute(
            "UPDATE todo_templates
             SET rule_mode=?1, rule_json=?2, cron_expression=?3, timezone=?4, updated_at=CURRENT_TIMESTAMP
             WHERE id=?5",
            params![
                rule_mode,
                serde_json::to_string(&rule).map_err(|e| format!("周期规则序列化失败: {e}"))?,
                cron_expression,
                timezone,
                id
            ],
        )
        .map_err(|e| format!("更新周期规则失败: {e}"))?;
        recompute_next = true;
    }

    if payload.get("recurrence").is_some() || recurrence.get("endMode").is_some() {
        let (end_mode, end_value) = parse_end_rule(payload)?;
        conn.execute(
            "UPDATE todo_templates SET end_mode=?1, end_value=?2, updated_at=CURRENT_TIMESTAMP WHERE id=?3",
            params![end_mode, end_value, id],
        )
        .map_err(|e| format!("更新周期终止规则失败: {e}"))?;
        recompute_next = true;
    }

    if payload.get("active").is_some() || recurrence.get("active").is_some() {
        let active = payload
            .get("active")
            .and_then(Value::as_bool)
            .or_else(|| recurrence.get("active").and_then(Value::as_bool))
            .unwrap_or(true);
        conn.execute(
            "UPDATE todo_templates
             SET active=?1,
                 next_occurrence_at=CASE WHEN ?1 = 1 THEN next_occurrence_at ELSE NULL END,
                 updated_at=CURRENT_TIMESTAMP
             WHERE id=?2",
            params![if active { 1 } else { 0 }, id],
        )
        .map_err(|e| format!("更新周期状态失败: {e}"))?;
        if active {
            recompute_next = true;
        }
    }

    if payload.get("assigneeIds").is_some() {
        sync_template_assignees(&conn, id, &parse_assignee_ids(payload))?;
    }

    if recompute_next {
        let (cron_expression, timezone, start_at, active): (String, String, Option<String>, i64) = conn
            .query_row(
                "SELECT cron_expression, timezone, start_at, active FROM todo_templates WHERE id=?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(|_| "周期模板不存在".to_string())?;
        if active == 1 {
            let now = Utc::now();
            let next_occurrence = compute_next_occurrence_with_start(
                &cron_expression,
                &timezone,
                start_at.as_deref(),
                now,
            )?
                .map(|dt| dt.to_rfc3339());
            conn.execute(
                "UPDATE todo_templates
                 SET next_occurrence_at=?1, generated_count=0, updated_at=CURRENT_TIMESTAMP
                 WHERE id=?2",
                params![next_occurrence, id],
            )
            .map_err(|e| format!("重算下一次触发失败: {e}"))?;
        }
    }

    Ok(json!({ "ok": true }))
}

fn template_toggle_active(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缺少周期模板 id")?;
    let active = payload
        .get("active")
        .and_then(Value::as_bool)
        .or_else(|| recurrence_payload(payload).get("active").and_then(Value::as_bool))
        .unwrap_or(true);
    let conn = db_conn()?;

    if active {
        let (cron_expression, timezone, start_at): (String, String, Option<String>) = conn
            .query_row(
                "SELECT cron_expression, timezone, start_at FROM todo_templates WHERE id=?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|_| "周期模板不存在".to_string())?;
        let next_occurrence = compute_next_occurrence_with_start(
            &cron_expression,
            &timezone,
            start_at.as_deref(),
            Utc::now(),
        )?
            .map(|dt| dt.to_rfc3339());
        conn.execute(
            "UPDATE todo_templates SET active=1, next_occurrence_at=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![next_occurrence, id],
        )
        .map_err(|e| format!("启用周期模板失败: {e}"))?;
    } else {
        conn.execute(
            "UPDATE todo_templates
             SET active=0,
                 next_occurrence_at=NULL,
                 updated_at=CURRENT_TIMESTAMP
             WHERE id=?1",
            params![id],
        )
        .map_err(|e| format!("鍋滅敤鍛ㄦ湡妯℃澘澶辫触: {e}"))?;
    }

    Ok(json!({ "ok": true }))
}

fn template_delete(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缂哄皯鍛ㄦ湡妯℃澘 id")?;
    let conn = db_conn()?;
    delete_template_by_id(&conn, id)?;
    Ok(json!({ "ok": true }))
}

fn reminder_list_unread(payload: &Value) -> Result<Value, String> {
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
        .map_err(|e| format!("鏌ヨ鎻愰啋涓績澶辫触: {e}"))?;
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
                "createdAt": row.get::<_, String>(7)?,
                "reminderPreset": row.get::<_, Option<String>>(8)?.unwrap_or_else(|| REMINDER_PRESET_NONE.to_string())
            }))
        })
        .map_err(|e| format!("鏄犲皠鎻愰啋涓績澶辫触: {e}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| e.to_string())?);
    }
    Ok(json!({ "items": items }))
}

fn reminder_mark_read(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    if payload.get("all").and_then(Value::as_bool).unwrap_or(false) {
        conn.execute(
            "UPDATE todo_reminder_events SET is_read=1, updated_at=CURRENT_TIMESTAMP WHERE is_read=0",
            [],
        )
        .map_err(|e| format!("鏍囪鍏ㄩ儴宸茶澶辫触: {e}"))?;
        return Ok(json!({ "ok": true }));
    }
    if let Some(id) = parse_i64(payload, "id") {
        conn.execute(
            "UPDATE todo_reminder_events SET is_read=1, updated_at=CURRENT_TIMESTAMP WHERE id=?1",
            params![id],
        )
        .map_err(|e| format!("鏍囪宸茶澶辫触: {e}"))?;
        return Ok(json!({ "ok": true }));
    }
    let ids = payload
        .get("ids")
        .and_then(Value::as_array)
        .ok_or("缂哄皯鎻愰啋浜嬩欢 id")?;
    let values: Vec<i64> = ids.iter().filter_map(Value::as_i64).filter(|id| *id > 0).collect();
    if values.is_empty() {
        return Err("缂哄皯鏈夋晥鎻愰啋浜嬩欢 id".to_string());
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
        .map_err(|e| format!("鎵归噺鏍囪宸茶澶辫触: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn load_active_templates(conn: &Connection) -> Result<Vec<TemplateRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT
                id, title, type_id, priority, description, COALESCE(series_kind, 'recurring'),
                cron_expression, timezone, start_at, end_mode, end_value,
                next_occurrence_at, generated_count, active
             FROM todo_templates
             WHERE active=1 AND COALESCE(series_kind, 'recurring')='recurring'
             ORDER BY id ASC",
        )
        .map_err(|e| format!("查询可用周期模板失败: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(TemplateRow {
                id: row.get(0)?,
                title: row.get(1)?,
                type_id: row.get(2)?,
                priority: row.get(3)?,
                description: row.get(4)?,
                _series_kind: row.get(5)?,
                cron_expression: row.get(6)?,
                timezone: row.get(7)?,
                start_at: row.get(8)?,
                end_mode: row.get(9)?,
                end_value: row.get(10)?,
                next_occurrence_at: row.get(11)?,
                generated_count: row.get(12)?,
                active: row.get::<_, i64>(13)? == 1,
                reminder_configs: Vec::new(),
            })
        })
        .map_err(|e| format!("映射可用周期模板失败: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let mut template = row.map_err(|e| e.to_string())?;
        template.reminder_configs = load_template_reminder_configs(conn, template.id)?;
        out.push(template);
    }
    Ok(out)
}

fn load_template_row(conn: &Connection, template_id: i64) -> Result<Option<TemplateRow>, String> {
    let template = conn
        .query_row(
            "SELECT
                id, title, type_id, priority, description, COALESCE(series_kind, 'recurring'),
                cron_expression, timezone, start_at, end_mode, end_value,
                next_occurrence_at, generated_count, active
             FROM todo_templates
             WHERE id=?1",
            params![template_id],
            |row| {
                Ok(TemplateRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    type_id: row.get(2)?,
                    priority: row.get(3)?,
                    description: row.get(4)?,
                    _series_kind: row.get(5)?,
                    cron_expression: row.get(6)?,
                    timezone: row.get(7)?,
                    start_at: row.get(8)?,
                    end_mode: row.get(9)?,
                    end_value: row.get(10)?,
                    next_occurrence_at: row.get(11)?,
                    generated_count: row.get(12)?,
                    active: row.get::<_, i64>(13)? == 1,
                    reminder_configs: Vec::new(),
                })
            },
        )
        .optional()
        .map_err(|e| format!("查询周期模板失败: {e}"))?;

    let Some(mut template) = template else {
        return Ok(None);
    };

    template.reminder_configs = load_template_reminder_configs(conn, template.id)?;
    Ok(Some(template))
}

fn has_other_actionable_template_task(
    conn: &Connection,
    template_id: i64,
    excluded_task_id: i64,
) -> Result<bool, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM todo_tasks
             WHERE source_template_id=?1
               AND id<>?2
               AND status IN ('pending','in_progress')",
            params![template_id, excluded_task_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("查询周期实例失败: {e}"))?;
    Ok(count > 0)
}

fn has_existing_template_occurrence(
    conn: &Connection,
    template_id: i64,
    excluded_task_id: i64,
    event_at: &str,
) -> Result<bool, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM todo_tasks
             WHERE source_template_id=?1
               AND id<>?2
               AND event_at=?3",
            params![template_id, excluded_task_id, event_at],
            |row| row.get(0),
        )
        .map_err(|e| format!("查询重复周期实例失败: {e}"))?;
    Ok(count > 0)
}

fn persist_template_progress(
    conn: &Connection,
    template_id: i64,
    generated_count: i64,
    next_occurrence: Option<DateTime<Utc>>,
    active: bool,
) -> Result<(), String> {
    conn.execute(
        "UPDATE todo_templates
         SET generated_count=?1,
             next_occurrence_at=?2,
             active=?3,
             updated_at=CURRENT_TIMESTAMP
         WHERE id=?4",
        params![
            generated_count,
            next_occurrence.map(|dt| dt.to_rfc3339()),
            if active { 1 } else { 0 },
            template_id
        ],
    )
    .map_err(|e| format!("更新周期模板进度失败: {e}"))?;
    Ok(())
}

fn maybe_generate_next_recurring_task_after_completion(
    conn: &Connection,
    task_id: i64,
    current_status: &str,
) -> Result<(), String> {
    if current_status == STATUS_COMPLETED || current_status == STATUS_CANCELED {
        return Ok(());
    }

    let snapshot = load_task_snapshot(conn, task_id)?;
    let Some(template_id) = snapshot.source_template_id else {
        return Ok(());
    };

    if has_other_actionable_template_task(conn, template_id, task_id)? {
        return Ok(());
    }

    let Some(template) = load_template_row(conn, template_id)? else {
        return Ok(());
    };
    if !template.active {
        return Ok(());
    }

    let mut next_occurrence = template
        .next_occurrence_at
        .as_deref()
        .and_then(parse_utc_datetime);
    if next_occurrence.is_none() {
        let fallback_after = snapshot
            .event_at
            .as_deref()
            .and_then(parse_utc_datetime)
            .map(|dt| dt + Duration::seconds(1))
            .unwrap_or_else(Utc::now);
        next_occurrence = compute_next_occurrence_with_start(
            &template.cron_expression,
            &template.timezone,
            template.start_at.as_deref(),
            fallback_after,
        )?;
    }

    let Some(next_occurrence) = next_occurrence else {
        persist_template_progress(conn, template.id, template.generated_count, None, false)?;
        return Ok(());
    };

    if should_stop_template(&template, next_occurrence, template.generated_count) {
        persist_template_progress(conn, template.id, template.generated_count, None, false)?;
        return Ok(());
    }

    let next_occurrence_at = next_occurrence.to_rfc3339();
    if has_existing_template_occurrence(conn, template.id, task_id, &next_occurrence_at)? {
        return Ok(());
    }

    create_task_from_template(conn, &template, next_occurrence)?;
    let generated_count = template.generated_count + 1;
    let mut next_after_generated = compute_next_occurrence_with_start(
        &template.cron_expression,
        &template.timezone,
        template.start_at.as_deref(),
        next_occurrence + Duration::seconds(1),
    )?;
    let mut active = template.active;
    if let Some(upcoming_occurrence) = next_after_generated {
        if should_stop_template(&template, upcoming_occurrence, generated_count) {
            next_after_generated = None;
            active = false;
        }
    } else {
        active = false;
    }
    persist_template_progress(
        conn,
        template.id,
        generated_count,
        next_after_generated,
        active,
    )?;
    Ok(())
}

fn should_stop_template(tpl: &TemplateRow, occurrence: DateTime<Utc>, generated_count: i64) -> bool {
    match tpl.end_mode.as_str() {
        "never" => false,
        "until_date" => tpl
            .end_value
            .as_deref()
            .and_then(parse_rfc3339)
            .and_then(|v| DateTime::parse_from_rfc3339(&v).ok())
            .map(|dt| occurrence > dt.with_timezone(&Utc))
            .unwrap_or(false),
        "after_count" => tpl
            .end_value
            .as_deref()
            .and_then(|v| v.parse::<i64>().ok())
            .map(|max| generated_count >= max)
            .unwrap_or(false),
        _ => false,
    }
}

fn create_task_from_template(conn: &Connection, tpl: &TemplateRow, due: DateTime<Utc>) -> Result<i64, String> {
    let event_at = due.to_rfc3339();
    conn.execute(
        "INSERT INTO todo_tasks(title, type_id, priority, description, status, event_at, remind_at, source_template_id)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            tpl.title,
            tpl.type_id,
            tpl.priority,
            tpl.description,
            STATUS_PENDING,
            event_at,
            Option::<String>::None,
            tpl.id
        ],
    )
    .map_err(|e| format!("生成周期任务失败: {e}"))?;
    let task_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT OR IGNORE INTO todo_task_assignees(task_id, assignee_id)
         SELECT ?1, assignee_id FROM todo_template_assignees WHERE template_id = ?2",
        params![task_id, tpl.id],
    )
    .map_err(|e| format!("复制周期执行人失败: {e}"))?;
    let reminder_presets = tpl
        .reminder_configs
        .iter()
        .map(|config| config.preset.clone())
        .collect::<Vec<_>>();
    sync_task_reminders(conn, task_id, Some(&event_at), &reminder_presets)?;
    Ok(task_id)
}

fn generate_recurring_instances(conn: &Connection, now: DateTime<Utc>) -> Result<(), String> {
    let templates = load_active_templates(conn)?;
    for tpl in templates {
        let mut generated_count = tpl.generated_count;
        let mut active = tpl.active;
        let mut next_occurrence = tpl
            .next_occurrence_at
            .as_deref()
            .and_then(parse_rfc3339)
            .and_then(|v| DateTime::parse_from_rfc3339(&v).ok())
            .map(|dt| dt.with_timezone(&Utc));

        if next_occurrence.is_none() {
            next_occurrence = compute_next_occurrence_with_start(
                &tpl.cron_expression,
                &tpl.timezone,
                tpl.start_at.as_deref(),
                now,
            )?;
        }

        let mut tick_count = 0;
        while let Some(current) = next_occurrence {
            if current > now {
                break;
            }
            if should_stop_template(&tpl, current, generated_count) {
                active = false;
                next_occurrence = None;
                break;
            }

            create_task_from_template(conn, &tpl, current)?;
            generated_count += 1;
            tick_count += 1;

            next_occurrence = compute_next_occurrence_with_start(
                &tpl.cron_expression,
                &tpl.timezone,
                tpl.start_at.as_deref(),
                current + Duration::seconds(1),
            )?;

            if tick_count >= 500 {
                break;
            }
        }

        persist_template_progress(conn, tpl.id, generated_count, next_occurrence, active)?;
    }
    Ok(())
}

fn dispatch_due_reminders(conn: &Connection, now: DateTime<Utc>) -> Result<Vec<ReminderDispatch>, String> {
    let now_str = now.to_rfc3339();
    let mut stmt = conn
        .prepare(
            "SELECT
                tr.id,
                t.id,
                tr.reminder_preset,
                t.title,
                t.description,
                t.priority,
                COALESCE(tr.snooze_until, tr.remind_at) AS fire_at
             FROM todo_task_reminders tr
             JOIN todo_tasks t ON t.id = tr.task_id
             WHERE t.status IN ('pending','in_progress')
               AND COALESCE(tr.snooze_until, tr.remind_at) <= ?1
               AND (tr.last_notified_at IS NULL OR tr.last_notified_at < COALESCE(tr.snooze_until, tr.remind_at))
             ORDER BY fire_at ASC, tr.id ASC
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
        let (task_reminder_id, task_id, reminder_preset, title, description, priority, fire_at) =
            row.map_err(|e| e.to_string())?;
        let body = if description.is_empty() {
            String::new()
        } else {
            description
        };
        conn.execute(
            "INSERT INTO todo_reminder_events(task_id, task_reminder_id, title, body, fire_at, is_read, reminder_preset)
             VALUES(?1, ?2, ?3, ?4, ?5, 0, ?6)",
            params![task_id, task_reminder_id, title, body, fire_at, reminder_preset],
        )
        .map_err(|e| format!("写入提醒中心失败: {e}"))?;
        let event_id = conn.last_insert_rowid();
        conn.execute(
            "UPDATE todo_task_reminders
             SET last_notified_at=?1,
                 updated_at=CURRENT_TIMESTAMP,
                 snooze_until=CASE WHEN snooze_until IS NOT NULL AND snooze_until <= ?1 THEN NULL ELSE snooze_until END
             WHERE id=?2",
            params![now.to_rfc3339(), task_reminder_id],
        )
        .map_err(|e| format!("更新任务提醒状态失败: {e}"))?;

        reminders.push(ReminderDispatch {
            event_id,
            task_id,
            task_reminder_id,
            title,
            body,
            fire_at,
            priority,
            reminder_preset,
        });
    }
    Ok(reminders)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn create_test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "
            CREATE TABLE todo_tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                type_id INTEGER DEFAULT NULL,
                priority TEXT NOT NULL DEFAULT 'P2',
                description TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'pending',
                due_at TEXT DEFAULT NULL,
                remind_at TEXT DEFAULT NULL,
                snooze_until TEXT DEFAULT NULL,
                last_notified_at TEXT DEFAULT NULL,
                pinned INTEGER NOT NULL DEFAULT 0,
                source_template_id INTEGER DEFAULT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                event_at TEXT DEFAULT NULL
            );
            CREATE TABLE todo_templates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                type_id INTEGER DEFAULT NULL,
                priority TEXT NOT NULL DEFAULT 'P2',
                description TEXT NOT NULL DEFAULT '',
                rule_mode TEXT NOT NULL DEFAULT 'simple',
                rule_json TEXT NOT NULL DEFAULT '{}',
                cron_expression TEXT NOT NULL DEFAULT '',
                timezone TEXT NOT NULL DEFAULT 'local',
                end_mode TEXT NOT NULL DEFAULT 'never',
                end_value TEXT DEFAULT NULL,
                next_occurrence_at TEXT DEFAULT NULL,
                generated_count INTEGER NOT NULL DEFAULT 0,
                active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                series_kind TEXT NOT NULL DEFAULT 'recurring',
                start_at TEXT DEFAULT NULL,
                reminder_offset_minutes INTEGER DEFAULT NULL
            );
            CREATE TABLE todo_task_assignees (
                task_id INTEGER NOT NULL,
                assignee_id INTEGER NOT NULL,
                UNIQUE(task_id, assignee_id)
            );
            CREATE TABLE todo_template_assignees (
                template_id INTEGER NOT NULL,
                assignee_id INTEGER NOT NULL,
                UNIQUE(template_id, assignee_id)
            );
            CREATE TABLE todo_task_reminders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                reminder_preset TEXT NOT NULL,
                offset_minutes INTEGER NOT NULL,
                remind_at TEXT NOT NULL,
                snooze_until TEXT DEFAULT NULL,
                last_notified_at TEXT DEFAULT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE todo_template_reminders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                template_id INTEGER NOT NULL,
                reminder_preset TEXT NOT NULL,
                offset_minutes INTEGER NOT NULL
            );
            CREATE TABLE todo_reminder_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                task_reminder_id INTEGER DEFAULT NULL,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                fire_at TEXT NOT NULL,
                is_read INTEGER NOT NULL DEFAULT 0,
                reminder_preset TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            ",
        )
        .expect("create todo schema");
        conn
    }

    fn seed_recurring_template(
        conn: &Connection,
        template_id: i64,
        generated_count: i64,
        next_occurrence_at: Option<&str>,
        end_mode: &str,
        end_value: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO todo_templates
             (id, title, priority, description, rule_mode, rule_json, cron_expression, timezone, start_at, end_mode, end_value, next_occurrence_at, generated_count, active, series_kind)
             VALUES(?1, ?2, ?3, ?4, 'simple', '{}', ?5, ?6, ?7, ?8, ?9, ?10, ?11, 1, 'recurring')",
            params![
                template_id,
                "每日晨会",
                "P1",
                "周期事项",
                "0 0 9 * * *",
                "UTC",
                "2026-03-07T09:00:00+00:00",
                end_mode,
                end_value,
                next_occurrence_at,
                generated_count
            ],
        )
        .expect("seed recurring template");
    }

    fn seed_recurring_task(
        conn: &Connection,
        task_id: i64,
        status: &str,
        event_at: &str,
        template_id: i64,
    ) {
        conn.execute(
            "INSERT INTO todo_tasks(id, title, priority, description, status, event_at, source_template_id)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                task_id,
                format!("实例 {task_id}"),
                "P1",
                "已生成实例",
                status,
                event_at,
                template_id
            ],
        )
        .expect("seed recurring task");
    }

    #[test]
    fn simple_daily_cron_should_build() {
        let expr = build_simple_cron_expression(&json!({
            "frequency": "daily",
            "interval": 1,
            "time": "09:30"
        }))
        .expect("daily");
        assert_eq!(expr, "0 30 9 * * *");
    }

    #[test]
    fn simple_weekly_cron_should_build_using_named_weekdays() {
        let expr = build_simple_cron_expression(&json!({
            "frequency": "weekly",
            "interval": 1,
            "time": "09:30",
            "weekdays": [1, 2, 3, 4, 5]
        }))
        .expect("weekly");
        assert_eq!(expr, "0 30 9 * * Mon-Fri");

        let expr = build_simple_cron_expression(&json!({
            "frequency": "weekly",
            "interval": 1,
            "time": "09:30",
            "weekdays": [7]
        }))
        .expect("weekly");
        assert_eq!(expr, "0 30 9 * * Sun");
    }

    #[test]
    fn workday_next_occurrence_should_be_friday_after_thursday() {
        let expr = build_simple_cron_expression(&json!({
            "frequency": "weekly",
            "interval": 1,
            "time": "09:00",
            "weekdays": [1, 2, 3, 4, 5]
        }))
        .expect("weekly");

        let after = DateTime::parse_from_rfc3339("2026-03-12T10:00:00+00:00")
            .expect("after")
            .with_timezone(&Utc);
        let next = compute_next_occurrence_with_start(
            &expr,
            "UTC",
            Some("2026-03-10T09:00:00+00:00"),
            after,
        )
        .expect("next occurrence")
        .expect("occurrence exists");

        assert_eq!(next.to_rfc3339(), "2026-03-13T09:00:00+00:00");
    }

    #[test]
    fn simple_time_should_reject_non_five_minute_step() {
        let error = build_simple_cron_expression(&json!({
            "frequency": "daily",
            "interval": 1,
            "time": "09:07"
        }))
        .expect_err("should reject");
        assert!(error.contains("5 分钟"));
    }

    #[test]
    fn simple_monthly_rule_should_keep_day_31() {
        let expr = build_simple_cron_expression(&json!({
            "frequency": "monthly",
            "interval": 1,
            "time": "09:30",
            "dayOfMonth": 31,
        }))
        .expect("monthly rule");
        assert_eq!(expr, "0 30 9 31 * *");
    }

    #[test]
    fn cron_expression_should_reject_non_five_minute_schedule() {
        let error = resolve_cron_expression("cron", &json!({
            "expression": "3 9 * * *"
        }))
        .expect_err("should reject");
        assert!(error.contains("5 分钟"));
    }

    #[test]
    fn reminder_preset_should_roundtrip() {
        let event_at = Some("2026-03-07T09:30:00+00:00");
        let remind_at = compute_remind_at(event_at, Some(30)).expect("remind");
        assert_eq!(
            derive_reminder_presets(event_at, remind_at.as_deref()),
            vec![REMINDER_PRESET_30M.to_string()]
        );
    }

    #[test]
    fn reminder_requires_event_time() {
        assert!(compute_remind_at(None, Some(5)).is_err());
    }

    #[test]
    fn reminder_presets_should_normalize_multi_select() {
        let presets = parse_reminder_presets(&json!({
            "reminderPresets": ["none", "1d", "0m", "1d", "5m"]
        }))
        .expect("parse")
        .expect("has value");
        assert_eq!(
            presets,
            vec![
                REMINDER_PRESET_ON_TIME.to_string(),
                REMINDER_PRESET_5M.to_string(),
                REMINDER_PRESET_1D.to_string(),
            ]
        );
    }

    #[test]
    fn dispatch_due_reminders_should_include_priority_in_payload() {
        let conn = create_test_conn();
        conn.execute(
            "INSERT INTO todo_tasks(id, title, priority, description, status, event_at)
             VALUES(1, ?1, ?2, ?3, ?4, ?5)",
            params![
                "提醒事项",
                "P0",
                "",
                STATUS_PENDING,
                "2026-03-08T09:00:00+00:00"
            ],
        )
        .expect("seed task");
        conn.execute(
            "INSERT INTO todo_task_reminders(id, task_id, reminder_preset, offset_minutes, remind_at)
             VALUES(11, 1, ?1, 0, ?2)",
            params![REMINDER_PRESET_ON_TIME, "2026-03-08T09:00:00+00:00"],
        )
        .expect("seed reminder");

        let reminders = dispatch_due_reminders(
            &conn,
            DateTime::parse_from_rfc3339("2026-03-08T09:00:00+00:00")
                .expect("parse now")
                .with_timezone(&Utc),
        )
        .expect("dispatch reminders");

        assert_eq!(reminders.len(), 1);
        assert_eq!(reminders[0].priority, "P0");
        assert_eq!(reminders[0].body, "");
    }

    #[test]
    fn status_transition_should_validate() {
        assert!(can_transit(STATUS_PENDING, STATUS_IN_PROGRESS));
        assert!(can_transit(STATUS_IN_PROGRESS, STATUS_COMPLETED));
        assert!(!can_transit(STATUS_COMPLETED, STATUS_PENDING));
    }

    #[test]
    fn sort_item_rows_should_prioritize_pinned_items() {
        let mut items = vec![
            json!({
                "id": 1,
                "pinned": false,
                "priority": "P0",
                "displayAt": "2026-03-08T08:00:00.000Z"
            }),
            json!({
                "id": 2,
                "pinned": true,
                "priority": "P3",
                "displayAt": "2026-03-08T12:00:00.000Z"
            }),
        ];

        sort_item_rows(&mut items);

        assert_eq!(items[0].get("id").and_then(Value::as_i64), Some(2));
    }

    #[test]
    fn item_sort_time_should_ignore_updated_at_fallback() {
        let item = json!({
            "id": 1,
            "displayAt": Value::Null,
            "updatedAt": "2026-03-08T10:00:00.000Z"
        });

        assert_eq!(item_sort_time(&item), "");
    }

    #[test]
    fn task_row_without_event_at_should_not_emit_display_at() {
        let conn = create_test_conn();
        conn.execute(
            "INSERT INTO todo_tasks(
                id, title, priority, description, status,
                created_at, updated_at, event_at, pinned
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                1_i64,
                "无时间事项",
                "P2",
                "",
                STATUS_PENDING,
                "2026-03-08T08:00:00.000Z",
                "2026-03-08T10:00:00.000Z",
                Option::<String>::None,
                0_i64,
            ],
        )
        .expect("insert task");

        let mut stmt = conn
            .prepare(
                "SELECT t.id, t.title, t.type_id, t.priority, t.description, t.status,
                        t.event_at, t.remind_at, t.snooze_until, t.last_notified_at,
                        t.source_template_id, t.created_at, t.updated_at,
                        NULL AS type_name, NULL AS type_color,
                        'one_off' AS series_kind, t.pinned
                 FROM todo_tasks t
                 WHERE t.id = ?1",
            )
            .expect("prepare task row");

        let item = stmt
            .query_row(params![1_i64], |row| row_to_task_json(row))
            .expect("map task row");

        assert!(item.get("eventAt").is_some_and(Value::is_null));
        assert!(item.get("displayAt").is_some_and(Value::is_null));
        assert_eq!(
            item.get("updatedAt").and_then(Value::as_str),
            Some("2026-03-08T10:00:00.000Z")
        );
    }

    #[test]
    fn parse_item_kind_should_support_new_payload_shape() {
        assert_eq!(parse_item_kind(&json!({ "kind": "recurring" })), SERIES_KIND_RECURRING);
        assert_eq!(
            parse_item_kind(&json!({
                "recurrence": {
                    "ruleMode": "simple",
                    "rule": { "frequency": "daily", "interval": 1, "time": "09:00" },
                    "timezone": "local",
                    "endMode": "never",
                    "endValue": null
                }
            })),
            SERIES_KIND_RECURRING
        );
        assert_eq!(parse_item_kind(&json!({ "kind": "one_off" })), SERIES_KIND_ONE_OFF);
    }

    #[test]
    fn parse_end_rule_should_support_nested_recurrence_payload() {
        let (mode, end_value) = parse_end_rule(&json!({
            "recurrence": {
                "endMode": "after_count",
                "endValue": 5
            }
        }))
        .expect("nested recurrence end rule");
        assert_eq!(mode, "after_count");
        assert_eq!(end_value.as_deref(), Some("5"));
    }

    #[test]
    fn resolve_item_update_strategy_should_detach_current_occurrence() {
        assert_eq!(
            resolve_item_update_strategy(
                SERIES_KIND_RECURRING,
                SERIES_KIND_ONE_OFF,
                SCOPE_THIS_INSTANCE,
                false,
            ),
            ItemUpdateStrategy::DetachOccurrenceToOneOff
        );
    }

    #[test]
    fn resolve_item_update_strategy_should_convert_future_series_to_one_off() {
        assert_eq!(
            resolve_item_update_strategy(
                SERIES_KIND_RECURRING,
                SERIES_KIND_ONE_OFF,
                SCOPE_FUTURE_INSTANCES,
                false,
            ),
            ItemUpdateStrategy::ConvertSeriesToOneOff
        );
        assert_eq!(
            resolve_item_update_strategy(
                SERIES_KIND_RECURRING,
                SERIES_KIND_ONE_OFF,
                SCOPE_THIS_INSTANCE,
                true,
            ),
            ItemUpdateStrategy::ConvertSeriesToOneOff
        );
    }

    #[test]
    fn ensure_event_at_when_missing_should_use_start_at_or_fallback() {
        let payload = json!({
            "kind": "one_off",
            "recurrence": {
                "startAt": "2026-03-10T09:30:00+00:00"
            }
        });
        let next = ensure_event_at_when_missing(&payload, None).expect("inject eventAt");
        assert_eq!(
            next.get("eventAt").and_then(Value::as_str),
            Some("2026-03-10T09:30:00+00:00")
        );

        let payload = json!({ "kind": "one_off" });
        let next = ensure_event_at_when_missing(&payload, Some("2026-03-11T10:00:00+00:00"))
            .expect("inject fallback eventAt");
        assert_eq!(
            next.get("eventAt").and_then(Value::as_str),
            Some("2026-03-11T10:00:00+00:00")
        );
    }

    #[test]
    fn ensure_event_at_when_missing_should_respect_explicit_null() {
        let payload = json!({
            "kind": "one_off",
            "eventAt": Value::Null,
            "recurrence": {
                "startAt": "2026-03-10T09:30:00+00:00"
            }
        });
        let next = ensure_event_at_when_missing(&payload, Some("2026-03-11T10:00:00+00:00"))
            .expect("keep explicit null");
        assert!(next.get("eventAt").is_some_and(Value::is_null));
    }

    #[test]
    fn next_occurrence_should_respect_start_time_boundary() {
        let start_at = "2026-03-10T09:30:00+00:00";
        let next = compute_next_occurrence_with_start(
            "0 30 9 * * *",
            "UTC",
            Some(start_at),
            DateTime::parse_from_rfc3339("2026-03-07T00:00:00+00:00")
                .expect("after")
                .with_timezone(&Utc),
        )
        .expect("next occurrence")
        .expect("occurrence exists");

        assert_eq!(next.to_rfc3339(), start_at);
    }

    #[test]
    fn template_root_item_should_map_to_unified_item_shape() {
        let item = template_to_root_item(json!({
            "id": 7,
            "title": "每周例会",
            "typeId": 1,
            "typeName": "会议安排",
            "typeColor": "#e6a23c",
            "priority": "P1",
            "description": "项目例会",
            "ruleMode": "simple",
            "rule": { "frequency": "weekly", "interval": 1, "time": "09:00", "weekdays": [1] },
            "cronExpression": "0 0 9 * * 1",
            "timezone": "local",
            "startAt": "2026-03-08T09:00:00+08:00",
            "endMode": "never",
            "endValue": null,
            "nextOccurrenceAt": "2026-03-09T09:00:00+08:00",
            "generatedCount": 2,
            "active": true,
            "reminderPresets": ["30m"],
            "assignees": [],
            "createdAt": "2026-03-07T08:00:00+08:00",
            "updatedAt": "2026-03-07T08:00:00+08:00"
        }))
        .expect("root item");

        assert_eq!(item.get("kind").and_then(Value::as_str), Some(SERIES_KIND_RECURRING));
        assert_eq!(item.get("recordRole").and_then(Value::as_str), Some(RECORD_ROLE_ROOT));
        assert_eq!(item.get("rootId").and_then(Value::as_i64), Some(7));
        assert_eq!(item.get("pinned").and_then(Value::as_bool), Some(false));
        assert_eq!(
            item.get("recurrence")
                .and_then(Value::as_object)
                .and_then(|recurrence| recurrence.get("startAt"))
                .and_then(Value::as_str),
            Some("2026-03-08T09:00:00+08:00")
        );
        assert_eq!(
            item.get("recurrence")
                .and_then(Value::as_object)
                .and_then(|recurrence| recurrence.get("nextOccurrenceAt"))
                .and_then(Value::as_str),
            Some("2026-03-09T09:00:00+08:00")
        );
        assert_eq!(
            item.get("reminderPresets")
                .and_then(Value::as_array)
                .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>()),
            Some(vec!["30m"])
        );
    }

    #[test]
    fn convert_one_off_task_to_recurring_should_bind_existing_task_without_duplicate() {
        let mut conn = create_test_conn();
        // 使用未来日期，避免测试随当前日期推进而变得不稳定（函数内部会使用 Utc::now() 触发一次调度生成）。
        let start_at = "2099-03-07T09:00:00+00:00";
        let expected_next = "2099-03-08T09:00:00+00:00";
        conn.execute(
            "INSERT INTO todo_tasks(title, type_id, priority, description, status, event_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "单次晨会",
                1,
                "P2",
                "原始单次事项",
                STATUS_PENDING,
                start_at
            ],
        )
        .expect("seed one-off task");

        let payload = json!({
            "id": 1,
            "kind": "recurring",
            "title": "每日晨会",
            "typeId": 2,
            "priority": "P1",
            "description": "转换为周期事项",
            "assigneeIds": [11, 12],
            "reminderPresets": ["0m", "10m"],
            "recurrence": {
                "startAt": start_at,
                "ruleMode": "simple",
                "rule": { "frequency": "daily", "interval": 1, "time": "09:00" },
                "timezone": "UTC",
                "endMode": "never",
                "endValue": null
            }
        });

        let result = convert_one_off_task_to_recurring(&mut conn, &payload, 1).expect("convert");
        let template_id = result.get("rootId").and_then(Value::as_i64).expect("template id");

        let total_tasks: i64 = conn
            .query_row("SELECT COUNT(*) FROM todo_tasks", [], |row| row.get(0))
            .expect("count tasks");
        assert_eq!(total_tasks, 1);

        let bound_template_id: i64 = conn
            .query_row(
                "SELECT source_template_id FROM todo_tasks WHERE id=1",
                [],
                |row| row.get(0),
            )
            .expect("load bound template id");
        assert_eq!(bound_template_id, template_id);

        let generated_count: i64 = conn
            .query_row(
                "SELECT generated_count FROM todo_templates WHERE id=?1",
                params![template_id],
                |row| row.get(0),
            )
            .expect("load generated count");
        let next_occurrence_at: String = conn
            .query_row(
                "SELECT next_occurrence_at FROM todo_templates WHERE id=?1",
                params![template_id],
                |row| row.get(0),
        )
            .expect("load next occurrence");
        assert_eq!(generated_count, 1);
        assert_eq!(next_occurrence_at, expected_next);

        let task_reminders: i64 = conn
            .query_row("SELECT COUNT(*) FROM todo_task_reminders WHERE task_id=1", [], |row| row.get(0))
            .expect("count task reminders");
        let template_reminders: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM todo_template_reminders WHERE template_id=?1",
                params![template_id],
                |row| row.get(0),
            )
            .expect("count template reminders");
        assert_eq!(task_reminders, 2);
        assert_eq!(template_reminders, 2);
    }

    #[test]
    fn convert_recurring_occurrence_to_one_off_should_detach_current_instance() {
        let mut conn = create_test_conn();
        conn.execute(
            "INSERT INTO todo_templates
             (id, title, priority, description, rule_mode, rule_json, cron_expression, timezone, start_at, end_mode, next_occurrence_at, generated_count, active, series_kind)
             VALUES(?1, ?2, ?3, ?4, 'simple', '{}', ?5, ?6, ?7, 'never', ?8, 1, 1, 'recurring')",
            params![
                7,
                "每日晨会",
                "P1",
                "周期事项",
                "0 0 9 * * *",
                "UTC",
                "2026-03-07T09:00:00+00:00",
                "2026-03-08T09:00:00+00:00"
            ],
        )
        .expect("seed template");
        conn.execute(
            "INSERT INTO todo_tasks(title, priority, description, status, event_at, source_template_id)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "2026-03-07 晨会",
                "P1",
                "已生成实例",
                STATUS_PENDING,
                "2026-03-07T09:00:00+00:00",
                7
            ],
        )
        .expect("seed occurrence task");

        let payload = json!({
            "id": 1,
            "rootId": 7,
            "kind": "one_off",
            "scope": "this_instance",
            "title": "独立晨会",
            "priority": "P1",
            "description": "脱离原周期",
            "eventAt": "2026-03-07T09:00:00+00:00",
            "assigneeIds": [],
            "reminderPresets": ["0m"]
        });

        let result = convert_recurring_item_to_one_off(&mut conn, &payload, Some(1), 7)
            .expect("should detach current-instance conversion");
        assert_eq!(result.get("id").and_then(Value::as_i64), Some(1));
        assert_eq!(result.get("rootId").and_then(Value::as_i64), Some(1));

        let task_row: (Option<i64>, String, String) = conn
            .query_row(
                "SELECT source_template_id, title, description FROM todo_tasks WHERE id=1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("load detached task");
        assert_eq!(task_row.0, None);
        assert_eq!(task_row.1, "独立晨会");
        assert_eq!(task_row.2, "脱离原周期");

        let template_exists: i64 = conn
            .query_row("SELECT COUNT(*) FROM todo_templates WHERE id=7", [], |row| row.get(0))
            .expect("count template");
        assert_eq!(template_exists, 1);

        let reminder_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM todo_task_reminders WHERE task_id=1", [], |row| row.get(0))
            .expect("count task reminders");
        assert_eq!(reminder_count, 1);
    }

    #[test]
    fn completing_recurring_task_should_generate_next_instance_when_no_other_open_instance() {
        let conn = create_test_conn();
        seed_recurring_template(
            &conn,
            7,
            1,
            Some("2026-03-08T09:00:00+00:00"),
            "never",
            None,
        );
        seed_recurring_task(&conn, 1, STATUS_PENDING, "2026-03-07T09:00:00+00:00", 7);
        conn.execute(
            "UPDATE todo_tasks SET status=?1 WHERE id=1",
            params![STATUS_COMPLETED],
        )
        .expect("mark current task completed");

        maybe_generate_next_recurring_task_after_completion(&conn, 1, STATUS_PENDING)
            .expect("generate next recurring task");

        let total_tasks: i64 = conn
            .query_row("SELECT COUNT(*) FROM todo_tasks", [], |row| row.get(0))
            .expect("count tasks");
        assert_eq!(total_tasks, 2);

        let next_task: (String, String, i64) = conn
            .query_row(
                "SELECT status, event_at, source_template_id FROM todo_tasks WHERE id<>1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("load generated task");
        assert_eq!(next_task.0, STATUS_PENDING);
        assert_eq!(next_task.1, "2026-03-08T09:00:00+00:00");
        assert_eq!(next_task.2, 7);

        let template_state: (i64, String, i64) = conn
            .query_row(
                "SELECT generated_count, next_occurrence_at, active FROM todo_templates WHERE id=7",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("load template state");
        assert_eq!(template_state.0, 2);
        assert_eq!(template_state.1, "2026-03-09T09:00:00+00:00");
        assert_eq!(template_state.2, 1);
    }

    #[test]
    fn completing_recurring_task_should_not_generate_when_other_open_instance_exists() {
        let conn = create_test_conn();
        seed_recurring_template(
            &conn,
            7,
            2,
            Some("2026-03-09T09:00:00+00:00"),
            "never",
            None,
        );
        seed_recurring_task(&conn, 1, STATUS_PENDING, "2026-03-07T09:00:00+00:00", 7);
        seed_recurring_task(&conn, 2, STATUS_PENDING, "2026-03-08T09:00:00+00:00", 7);
        conn.execute(
            "UPDATE todo_tasks SET status=?1 WHERE id=1",
            params![STATUS_COMPLETED],
        )
        .expect("mark current task completed");

        maybe_generate_next_recurring_task_after_completion(&conn, 1, STATUS_PENDING)
            .expect("skip when open future exists");

        let total_tasks: i64 = conn
            .query_row("SELECT COUNT(*) FROM todo_tasks", [], |row| row.get(0))
            .expect("count tasks");
        assert_eq!(total_tasks, 2);

        let template_state: (i64, String) = conn
            .query_row(
                "SELECT generated_count, next_occurrence_at FROM todo_templates WHERE id=7",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("load template state");
        assert_eq!(template_state.0, 2);
        assert_eq!(template_state.1, "2026-03-09T09:00:00+00:00");
    }

    #[test]
    fn completing_one_off_task_should_not_generate_next_instance() {
        let conn = create_test_conn();
        conn.execute(
            "INSERT INTO todo_tasks(id, title, priority, description, status, event_at)
             VALUES(1, ?1, ?2, ?3, ?4, ?5)",
            params![
                "普通事项",
                "P2",
                "单次事项",
                STATUS_COMPLETED,
                "2026-03-07T09:00:00+00:00"
            ],
        )
        .expect("seed one-off task");

        maybe_generate_next_recurring_task_after_completion(&conn, 1, STATUS_PENDING)
            .expect("one-off task should be ignored");

        let total_tasks: i64 = conn
            .query_row("SELECT COUNT(*) FROM todo_tasks", [], |row| row.get(0))
            .expect("count tasks");
        assert_eq!(total_tasks, 1);
    }

    #[test]
    fn completing_recurring_task_should_stop_template_when_end_limit_reached() {
        let conn = create_test_conn();
        seed_recurring_template(
            &conn,
            7,
            1,
            Some("2026-03-08T09:00:00+00:00"),
            "after_count",
            Some("1"),
        );
        seed_recurring_task(&conn, 1, STATUS_PENDING, "2026-03-07T09:00:00+00:00", 7);
        conn.execute(
            "UPDATE todo_tasks SET status=?1 WHERE id=1",
            params![STATUS_COMPLETED],
        )
        .expect("mark current task completed");

        maybe_generate_next_recurring_task_after_completion(&conn, 1, STATUS_PENDING)
            .expect("respect end limit");

        let total_tasks: i64 = conn
            .query_row("SELECT COUNT(*) FROM todo_tasks", [], |row| row.get(0))
            .expect("count tasks");
        assert_eq!(total_tasks, 1);

        let template_state: (i64, Option<String>, i64) = conn
            .query_row(
                "SELECT generated_count, next_occurrence_at, active FROM todo_templates WHERE id=7",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("load template state");
        assert_eq!(template_state.0, 1);
        assert_eq!(template_state.1, None);
        assert_eq!(template_state.2, 0);
    }

    #[test]
    fn canceling_recurring_task_should_not_generate_next_instance() {
        let conn = create_test_conn();
        seed_recurring_template(
            &conn,
            7,
            1,
            Some("2026-03-08T09:00:00+00:00"),
            "never",
            None,
        );
        seed_recurring_task(&conn, 1, STATUS_PENDING, "2026-03-07T09:00:00+00:00", 7);

        task_change_status_with_conn(&conn, 1, STATUS_CANCELED).expect("cancel recurring task");

        let total_tasks: i64 = conn
            .query_row("SELECT COUNT(*) FROM todo_tasks", [], |row| row.get(0))
            .expect("count tasks");
        assert_eq!(total_tasks, 1);

        let template_state: (i64, String, i64) = conn
            .query_row(
                "SELECT generated_count, next_occurrence_at, active FROM todo_templates WHERE id=7",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("load template state");
        assert_eq!(template_state.0, 1);
        assert_eq!(template_state.1, "2026-03-08T09:00:00+00:00");
        assert_eq!(template_state.2, 1);
    }

    #[test]
    fn repeating_completed_status_should_not_generate_duplicate_instance() {
        let conn = create_test_conn();
        seed_recurring_template(
            &conn,
            7,
            1,
            Some("2026-03-08T09:00:00+00:00"),
            "never",
            None,
        );
        seed_recurring_task(&conn, 1, STATUS_PENDING, "2026-03-07T09:00:00+00:00", 7);

        task_change_status_with_conn(&conn, 1, STATUS_COMPLETED).expect("complete recurring task");
        task_change_status_with_conn(&conn, 1, STATUS_COMPLETED).expect("repeat completed status");

        let total_tasks: i64 = conn
            .query_row("SELECT COUNT(*) FROM todo_tasks", [], |row| row.get(0))
            .expect("count tasks");
        assert_eq!(total_tasks, 2);

        let template_state: (i64, String) = conn
            .query_row(
                "SELECT generated_count, next_occurrence_at FROM todo_templates WHERE id=7",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("load template state");
        assert_eq!(template_state.0, 2);
        assert_eq!(template_state.1, "2026-03-09T09:00:00+00:00");
    }
}

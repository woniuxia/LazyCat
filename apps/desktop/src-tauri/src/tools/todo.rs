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
const SERIES_KIND_ONE_OFF: &str = "one_off";
const SERIES_KIND_RECURRING: &str = "recurring";
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

// ── Structs ───────────────────────────────────────────────

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

struct SeriesRuleRow {
    #[allow(dead_code)]
    series_id: i64,
    rule_mode: String,
    rule_json: String,
    cron_expression: String,
    timezone: String,
    start_at: Option<String>,
    end_mode: String,
    end_value: Option<String>,
    occurrence_index: i64,
    active: bool,
}

struct ReminderConfig {
    preset: String,
    offset_minutes: i64,
}

#[derive(Default)]
struct TaskReminderSummary {
    reminder_presets: Vec<String>,
    snooze_until: Option<String>,
    last_notified_at: Option<String>,
    next_task_reminder_id: Option<i64>,
    next_reminder_preset: Option<String>,
}

// ── Entry points ──────────────────────────────────────────

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
        "open_link" => open_link(payload),
        _ => Err(format!("unsupported todo action: {action}")),
    }
}

pub fn scheduler_tick() -> Result<Vec<ReminderDispatch>, String> {
    let conn = db_conn()?;
    dispatch_due_reminders(&conn, Utc::now())
}

// ── Parse / format utilities ──────────────────────────────

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
    normalize_series_kind(payload.get("seriesKind").and_then(Value::as_str))
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

fn format_db_datetime(raw: &str) -> String {
    parse_rfc3339(raw).unwrap_or_else(|| raw.to_string())
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
    let parsed = DateTime::parse_from_rfc3339(text).map_err(|_| format!("{label}格式不正确"))?;
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
    if let Some(start_at) = recurrence.get("startAt") {
        if !start_at.is_null() {
            return parse_datetime_with_validation(
                &json!({ "startAt": start_at.clone() }),
                "startAt",
                "开始时间",
                true,
            );
        }
    }
    if let Some(start_at) = payload.get("startAt") {
        if !start_at.is_null() {
            return parse_datetime_with_validation(payload, "startAt", "开始时间", true);
        }
    }
    if let Some(event_at) = payload.get("eventAt") {
        if !event_at.is_null() {
            return parse_datetime_with_validation(payload, "eventAt", "开始时间", true);
        }
    }
    Ok(None)
}

fn has_start_datetime(payload: &Value) -> bool {
    payload
        .get("startAt")
        .map(|v| !v.is_null())
        .unwrap_or(false)
        || recurrence_payload(payload)
            .get("startAt")
            .map(|v| !v.is_null())
            .unwrap_or(false)
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

fn parse_links(payload: &Value) -> Option<Vec<Value>> {
    payload.get("links").and_then(Value::as_array).cloned()
}

fn normalize_priority(value: Option<&str>) -> Result<String, String> {
    let p = value.unwrap_or("P2").trim().to_uppercase();
    if PRIORITIES.contains(&p.as_str()) {
        Ok(p)
    } else {
        Err("优先级必须是 P0/P1/P2/P3".to_string())
    }
}

fn normalize_status(value: &str) -> Result<String, String> {
    match value {
        STATUS_PENDING | STATUS_IN_PROGRESS | STATUS_COMPLETED => Ok(value.to_string()),
        _ => Err("状态不合法".to_string()),
    }
}

/// A1 归一化：in_progress 视同 pending
fn normalize_status_a1(status: &str) -> &str {
    if status == STATUS_IN_PROGRESS {
        STATUS_PENDING
    } else {
        status
    }
}

fn is_open_status(status: &str) -> bool {
    status == STATUS_PENDING || status == STATUS_IN_PROGRESS
}

fn can_transit(current: &str, next: &str) -> bool {
    if current == next {
        return true;
    }
    matches!(
        (current, next),
        (STATUS_PENDING, STATUS_IN_PROGRESS)
            | (STATUS_PENDING, STATUS_COMPLETED)
            | (STATUS_IN_PROGRESS, STATUS_PENDING)
            | (STATUS_IN_PROGRESS, STATUS_COMPLETED)
    )
}

fn can_transit_for_kind(current: &str, next: &str, kind: &str) -> bool {
    if current == next {
        return true;
    }
    // recurring: 不允许 done→pending
    if kind == SERIES_KIND_RECURRING
        && current == STATUS_COMPLETED
        && (next == STATUS_PENDING || next == STATUS_IN_PROGRESS)
    {
        return false;
    }
    // one_off: 允许 done→pending (撤销)
    if kind == SERIES_KIND_ONE_OFF
        && current == STATUS_COMPLETED
        && (next == STATUS_PENDING || next == STATUS_IN_PROGRESS)
    {
        return true;
    }
    can_transit(current, next)
}

// ── Sort utilities ────────────────────────────────────────

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

// ── Cron / rule utilities ─────────────────────────────────

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

fn ensure_schedule_granularity(cron_expression: &str) -> Result<(), String> {
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

fn compute_next_occurrence(
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
        Err(_) => {
            let local_after = after_utc.with_timezone(&Local);
            let next = schedule.after(&local_after).next();
            Ok(next.map(|dt| dt.with_timezone(&Utc)))
        }
    }
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

// ── Reminder utilities ────────────────────────────────────

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

fn compute_remind_at(
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

// ── DB helpers for items ──────────────────────────────────

fn sync_item_assignees(conn: &Connection, item_id: i64, ids: &[i64]) -> Result<(), String> {
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

fn load_item_assignees(conn: &Connection, item_id: i64) -> Result<Vec<Value>, String> {
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

fn load_item_links(conn: &Connection, item_id: i64) -> Result<Vec<Value>, String> {
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

fn sync_item_links(conn: &Connection, item_id: i64, links: &[Value]) -> Result<(), String> {
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

fn load_item_reminder_configs(
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

    if !out.is_empty() {
        return Ok(out);
    }

    // Legacy fallback: derive from row-level remind_at
    let legacy = conn
        .query_row(
            "SELECT event_at, remind_at FROM todo_items WHERE id=?1",
            params![item_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
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

fn load_item_reminder_summary(
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

fn sync_item_reminders(
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

fn clear_item_reminder_snooze(conn: &Connection, item_id: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE todo_item_reminders
         SET snooze_until=NULL, updated_at=CURRENT_TIMESTAMP
         WHERE item_id=?1",
        params![item_id],
    )
    .map_err(|e| format!("清理事项稍后提醒失败: {e}"))?;
    Ok(())
}

fn resolve_item_reminder_id_for_snooze(
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

fn load_item_event_at(conn: &Connection, item_id: i64) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT event_at FROM todo_items WHERE id=?1",
        params![item_id],
        |row| row.get(0),
    )
    .map_err(|_| "事项不存在".to_string())
}

// ── Series rule helpers ───────────────────────────────────

fn load_series_rule(conn: &Connection, series_id: i64) -> Result<Option<SeriesRuleRow>, String> {
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

fn should_stop_series(rule: &SeriesRuleRow, occurrence: DateTime<Utc>) -> bool {
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

fn has_other_open_in_series(
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
fn generate_next_item(
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

// ── Type CRUD ─────────────────────────────────────────────

fn type_list() -> Result<Value, String> {
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

fn type_upsert(payload: &Value) -> Result<Value, String> {
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

fn type_delete(payload: &Value) -> Result<Value, String> {
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

fn assignee_list() -> Result<Value, String> {
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
    conn.execute(
        "DELETE FROM todo_item_assignees WHERE assignee_id=?1",
        params![id],
    )
    .map_err(|e| format!("删除事项执行人关联失败: {e}"))?;
    conn.execute("DELETE FROM todo_assignees WHERE id=?1", params![id])
        .map_err(|e| format!("删除执行人失败: {e}"))?;
    Ok(json!({ "ok": true }))
}

// ── item_list ─────────────────────────────────────────────

fn item_list(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let status_filter = parse_string(payload, "status");
    let include_inactive = payload
        .get("includeInactive")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let mut stmt = conn
        .prepare(
            "SELECT i.id, i.title, i.type_id, i.priority, i.description, i.status,
                    i.event_at, i.pinned, i.kind, i.series_id, i.parent_id,
                    i.created_at, i.updated_at,
                    ty.name AS type_name, ty.color AS type_color,
                    sr.rule_mode, sr.rule_json, sr.cron_expression, sr.timezone,
                    sr.start_at, sr.end_mode, sr.end_value, sr.occurrence_index, sr.active
             FROM todo_items i
             LEFT JOIN todo_types ty ON ty.id = i.type_id
             LEFT JOIN todo_series_rules sr ON sr.series_id = i.series_id
             ORDER BY i.id DESC",
        )
        .map_err(|e| format!("查询事项失败: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,             // id
                row.get::<_, String>(1)?,          // title
                row.get::<_, Option<i64>>(2)?,     // type_id
                row.get::<_, String>(3)?,          // priority
                row.get::<_, String>(4)?,          // description
                row.get::<_, String>(5)?,          // status
                row.get::<_, Option<String>>(6)?,  // event_at
                row.get::<_, i64>(7)? != 0,        // pinned
                row.get::<_, String>(8)?,          // kind
                row.get::<_, Option<i64>>(9)?,     // series_id
                row.get::<_, Option<i64>>(10)?,    // parent_id
                row.get::<_, String>(11)?,         // created_at
                row.get::<_, String>(12)?,         // updated_at
                row.get::<_, Option<String>>(13)?, // type_name
                row.get::<_, Option<String>>(14)?, // type_color
                // series rules (nullable)
                row.get::<_, Option<String>>(15)?, // rule_mode
                row.get::<_, Option<String>>(16)?, // rule_json
                row.get::<_, Option<String>>(17)?, // cron_expression
                row.get::<_, Option<String>>(18)?, // timezone
                row.get::<_, Option<String>>(19)?, // start_at
                row.get::<_, Option<String>>(20)?, // end_mode
                row.get::<_, Option<String>>(21)?, // end_value
                row.get::<_, Option<i64>>(22)?,    // occurrence_index
                row.get::<_, Option<i64>>(23)?,    // active (0/1/null)
            ))
        })
        .map_err(|e| format!("映射事项失败: {e}"))?;

    let mut items = Vec::new();
    for row in rows {
        let (
            id,
            title,
            type_id,
            priority,
            description,
            status_raw,
            event_at,
            pinned,
            kind,
            series_id,
            _parent_id,
            created_at,
            updated_at,
            type_name,
            type_color,
            rule_mode,
            rule_json,
            cron_expression,
            timezone,
            start_at,
            end_mode,
            end_value,
            occurrence_index,
            rule_active,
        ) = row.map_err(|e| e.to_string())?;

        // A1 归一化
        let status = normalize_status_a1(&status_raw).to_string();
        let rule_active_bool = rule_active.map(|v| v == 1).unwrap_or(true);

        // includeInactive 过滤
        if !include_inactive
            && kind == SERIES_KIND_RECURRING
            && !rule_active_bool
            && is_open_status(&status_raw)
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
        }

        items.push(item);
    }

    sort_item_rows(&mut items);
    Ok(json!({ "items": items }))
}

// ── item_create ───────────────────────────────────────────

fn item_create(payload: &Value) -> Result<Value, String> {
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
    let reminder_presets = parse_reminder_presets(payload)?
        .unwrap_or_else(|| vec![REMINDER_PRESET_ON_TIME.to_string()]);

    let mut conn = db_conn()?;

    if kind == SERIES_KIND_RECURRING {
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

        let tx = conn
            .transaction()
            .map_err(|e| format!("开启事务失败: {e}"))?;

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

        tx.commit().map_err(|e| format!("提交事务失败: {e}"))?;

        Ok(json!({ "ok": true, "id": item_id, "rootId": item_id }))
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

        conn.execute(
            "INSERT INTO todo_items(title, type_id, priority, description, kind, status, event_at, pinned)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
            params![title, type_id, priority, description, SERIES_KIND_ONE_OFF, STATUS_PENDING, event_at],
        )
        .map_err(|e| format!("创建事项失败: {e}"))?;
        let id = conn.last_insert_rowid();

        sync_item_assignees(&conn, id, &assignee_ids)?;
        if event_at.is_some() {
            sync_item_reminders(&conn, id, event_at.as_deref(), &reminder_presets)?;
        }
        if let Some(links) = parse_links(payload) {
            sync_item_links(&conn, id, &links)?;
        }

        Ok(json!({ "ok": true, "id": id, "rootId": id }))
    }
}

// ── item_update ───────────────────────────────────────────

fn item_update(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缺少事项 id")?;
    let conn = db_conn()?;

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

    Ok(json!({ "ok": true }))
}

fn item_upsert(payload: &Value) -> Result<Value, String> {
    if parse_i64(payload, "id").is_some() {
        item_update(payload)
    } else {
        item_create(payload)
    }
}

// ── item_change_status ────────────────────────────────────

fn item_change_status(payload: &Value) -> Result<Value, String> {
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
             snooze_until=CASE WHEN ?1 IN ('completed') THEN NULL ELSE snooze_until END,
             updated_at=CURRENT_TIMESTAMP
         WHERE id=?2",
        params![next, id],
    )
    .map_err(|e| format!("更新事项状态失败: {e}"))?;

    if next == STATUS_COMPLETED {
        clear_item_reminder_snooze(&conn, id)?;
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

fn item_delete(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("缺少事项 id")?;
    let scope = parse_scope(payload);
    let conn = db_conn()?;

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

    if scope == SCOPE_FUTURE_INSTANCES && kind == SERIES_KIND_RECURRING {
        // 暂停规则，不删除任何项
        if let Some(sid) = series_id {
            conn.execute(
                "UPDATE todo_series_rules SET active=0, updated_at=CURRENT_TIMESTAMP WHERE series_id=?1",
                params![sid],
            )
            .map_err(|e| format!("暂停系列规则失败: {e}"))?;
        }
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

            let has_other_open = has_other_open_in_series(&conn, sid, id)?;

            // 删除该项
            delete_item_by_id(&conn, id)?;

            // 无其它 open 时补生成
            if !has_other_open {
                if let Some(rule) = load_series_rule(&conn, sid)? {
                    if rule.active {
                        // 计算 base_time
                        let now = Utc::now();
                        let event_at_dt = load_item_event_at(&conn, id)
                            .ok()
                            .flatten()
                            .and_then(|s| parse_utc_datetime(&s));
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
                                        params![title, type_id, priority, description, SERIES_KIND_RECURRING, sid, id, STATUS_PENDING, next_event_at],
                                    )
                                    .map_err(|e| format!("补生成事项失败: {e}"))?;
                                    let new_id = conn.last_insert_rowid();

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

fn delete_item_by_id(conn: &Connection, item_id: i64) -> Result<(), String> {
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
    conn.execute("DELETE FROM todo_items WHERE id=?1", params![item_id])
        .map_err(|e| format!("删除事项失败: {e}"))?;
    Ok(())
}

// ── item_toggle_pin / item_snooze / item_toggle_active ────

fn item_toggle_pin(payload: &Value) -> Result<Value, String> {
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

fn item_snooze(payload: &Value) -> Result<Value, String> {
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

fn item_toggle_active(payload: &Value) -> Result<Value, String> {
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

// ── Reminder list / mark read ─────────────────────────────

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

fn reminder_mark_read(payload: &Value) -> Result<Value, String> {
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

fn open_link(payload: &Value) -> Result<Value, String> {
    let url = payload["url"].as_str().ok_or("url 不能为空")?.trim();
    if url.is_empty() {
        return Err("url 不能为空".to_string());
    }
    open::that(url).map_err(|e| format!("打开链接失败: {e}"))?;
    Ok(json!({ "ok": true }))
}

// ── dispatch_due_reminders ────────────────────────────────

fn dispatch_due_reminders(
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

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn create_test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "
            CREATE TABLE todo_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                type_id INTEGER DEFAULT NULL,
                priority TEXT NOT NULL DEFAULT 'P2',
                description TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'pending',
                event_at TEXT DEFAULT NULL,
                pinned INTEGER NOT NULL DEFAULT 0,
                kind TEXT NOT NULL DEFAULT 'one_off',
                parent_id INTEGER DEFAULT NULL,
                series_id INTEGER DEFAULT NULL,
                remind_at TEXT DEFAULT NULL,
                snooze_until TEXT DEFAULT NULL,
                last_notified_at TEXT DEFAULT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE todo_series_rules (
                series_id INTEGER PRIMARY KEY,
                rule_mode TEXT NOT NULL DEFAULT 'simple',
                rule_json TEXT,
                cron_expression TEXT,
                timezone TEXT DEFAULT 'local',
                start_at TEXT DEFAULT NULL,
                end_mode TEXT NOT NULL DEFAULT 'never',
                end_value TEXT DEFAULT NULL,
                occurrence_index INTEGER NOT NULL DEFAULT 1,
                active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE todo_item_assignees (
                item_id INTEGER NOT NULL,
                assignee_id INTEGER NOT NULL,
                UNIQUE(item_id, assignee_id)
            );
            CREATE TABLE todo_item_reminders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                item_id INTEGER NOT NULL,
                reminder_preset TEXT NOT NULL,
                offset_minutes INTEGER NOT NULL,
                remind_at TEXT NOT NULL,
                snooze_until TEXT DEFAULT NULL,
                last_notified_at TEXT DEFAULT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE todo_item_links (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                item_id INTEGER NOT NULL,
                url TEXT NOT NULL,
                title TEXT NOT NULL DEFAULT '',
                sort_order INTEGER NOT NULL DEFAULT 0
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

    fn seed_series_rule(
        conn: &Connection,
        series_id: i64,
        occurrence_index: i64,
        end_mode: &str,
        end_value: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO todo_series_rules
             (series_id, rule_mode, rule_json, cron_expression, timezone, start_at, end_mode, end_value, occurrence_index, active)
             VALUES(?1, 'simple', '{}', ?2, 'UTC', ?3, ?4, ?5, ?6, 1)",
            params![
                series_id,
                "0 0 9 * * *",
                "2026-03-07T09:00:00+00:00",
                end_mode,
                end_value,
                occurrence_index,
            ],
        )
        .expect("seed series rule");
    }

    fn seed_recurring_item(
        conn: &Connection,
        item_id: i64,
        status: &str,
        event_at: &str,
        series_id: i64,
    ) {
        conn.execute(
            "INSERT INTO todo_items(id, title, priority, description, status, event_at, kind, series_id)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                item_id,
                format!("实例 {item_id}"),
                "P1",
                "已生成实例",
                status,
                event_at,
                SERIES_KIND_RECURRING,
                series_id,
            ],
        )
        .expect("seed recurring item");
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
        let error = resolve_cron_expression(
            "cron",
            &json!({
                "expression": "3 9 * * *"
            }),
        )
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
            "INSERT INTO todo_items(id, title, priority, description, status, event_at, kind)
             VALUES(1, ?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "提醒事项",
                "P0",
                "",
                STATUS_PENDING,
                "2026-03-08T09:00:00+00:00",
                SERIES_KIND_ONE_OFF,
            ],
        )
        .expect("seed item");
        conn.execute(
            "INSERT INTO todo_item_reminders(id, item_id, reminder_preset, offset_minutes, remind_at)
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
    fn status_transition_for_kind_should_block_recurring_done_to_pending() {
        assert!(!can_transit_for_kind(
            STATUS_COMPLETED,
            STATUS_PENDING,
            SERIES_KIND_RECURRING
        ));
        assert!(can_transit_for_kind(
            STATUS_COMPLETED,
            STATUS_PENDING,
            SERIES_KIND_ONE_OFF
        ));
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
    fn item_sort_time_should_use_display_at_only() {
        let item = json!({
            "id": 1,
            "displayAt": Value::Null,
            "updatedAt": "2026-03-08T10:00:00.000Z"
        });

        assert_eq!(item_sort_time(&item), "");
    }

    #[test]
    fn parse_item_kind_should_support_payload_shapes() {
        assert_eq!(
            parse_item_kind(&json!({ "kind": "recurring" })),
            SERIES_KIND_RECURRING
        );
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
        assert_eq!(
            parse_item_kind(&json!({ "kind": "one_off" })),
            SERIES_KIND_ONE_OFF
        );
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
    fn completing_recurring_item_should_generate_next_when_no_other_open() {
        let conn = create_test_conn();
        seed_series_rule(&conn, 7, 1, "never", None);
        seed_recurring_item(&conn, 1, STATUS_PENDING, "2026-03-07T09:00:00+00:00", 7);

        // Mark as completed
        conn.execute(
            "UPDATE todo_items SET status=?1 WHERE id=1",
            params![STATUS_COMPLETED],
        )
        .expect("mark completed");

        let next_id = generate_next_item(&conn, 7, 1, true)
            .expect("generate next")
            .expect("should generate");

        // Verify new item
        let (status, event_at, series_id): (String, String, i64) = conn
            .query_row(
                "SELECT status, event_at, series_id FROM todo_items WHERE id=?1",
                params![next_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("load new item");
        assert_eq!(status, STATUS_PENDING);
        assert_eq!(event_at, "2026-03-08T09:00:00+00:00");
        assert_eq!(series_id, 7);

        // Verify occurrence_index incremented
        let idx: i64 = conn
            .query_row(
                "SELECT occurrence_index FROM todo_series_rules WHERE series_id=7",
                [],
                |row| row.get(0),
            )
            .expect("load occurrence_index");
        assert_eq!(idx, 2);
    }

    #[test]
    fn completing_recurring_item_should_not_generate_when_other_open_exists() {
        let conn = create_test_conn();
        seed_series_rule(&conn, 7, 2, "never", None);
        seed_recurring_item(&conn, 1, STATUS_COMPLETED, "2026-03-07T09:00:00+00:00", 7);
        seed_recurring_item(&conn, 2, STATUS_PENDING, "2026-03-08T09:00:00+00:00", 7);

        let result = generate_next_item(&conn, 7, 1, true).expect("no generation");
        assert!(result.is_none());
    }

    #[test]
    fn completing_recurring_item_should_stop_when_end_limit_reached() {
        let conn = create_test_conn();
        seed_series_rule(&conn, 7, 1, "after_count", Some("1"));
        seed_recurring_item(&conn, 1, STATUS_COMPLETED, "2026-03-07T09:00:00+00:00", 7);

        let result = generate_next_item(&conn, 7, 1, true).expect("respect end limit");
        assert!(result.is_none());
    }

    #[test]
    fn should_stop_series_respects_until_date() {
        let rule = SeriesRuleRow {
            series_id: 1,
            rule_mode: "simple".to_string(),
            rule_json: "{}".to_string(),
            cron_expression: "0 0 9 * * *".to_string(),
            timezone: "UTC".to_string(),
            start_at: None,
            end_mode: "until_date".to_string(),
            end_value: Some("2026-03-07T09:00:00+00:00".to_string()),
            occurrence_index: 1,
            active: true,
        };
        let after = DateTime::parse_from_rfc3339("2026-03-08T09:00:00+00:00")
            .expect("parse")
            .with_timezone(&Utc);
        assert!(should_stop_series(&rule, after));
    }

    #[test]
    fn a1_normalization_should_map_in_progress_to_pending() {
        assert_eq!(normalize_status_a1("in_progress"), "pending");
        assert_eq!(normalize_status_a1("pending"), "pending");
        assert_eq!(normalize_status_a1("completed"), "completed");
    }
}

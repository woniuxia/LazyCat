use chrono::{DateTime, NaiveDateTime, Timelike, Utc};
use serde_json::{json, Value};

use super::types::{
    EVENT_TIME_MINUTE_STEP, PRIORITIES, SCOPE_FUTURE_INSTANCES, SCOPE_THIS_INSTANCE,
    SERIES_KIND_ONE_OFF, SERIES_KIND_RECURRING, STATUS_COMPLETED, STATUS_IN_PROGRESS,
    STATUS_PENDING,
};

// ── Parse / format utilities ──────────────────────────────

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

pub(crate) fn recurrence_payload<'a>(payload: &'a Value) -> &'a Value {
    payload
        .get("recurrence")
        .filter(|value| value.is_object())
        .unwrap_or(payload)
}

pub(crate) fn parse_item_kind(payload: &Value) -> String {
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
    SERIES_KIND_ONE_OFF.to_string()
}

pub(crate) fn parse_rfc3339(raw: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).to_rfc3339())
        .or_else(|| {
            NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc).to_rfc3339())
        })
}

pub(crate) fn parse_utc_datetime(raw: &str) -> Option<DateTime<Utc>> {
    parse_rfc3339(raw)
        .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

pub(crate) fn format_db_datetime(raw: &str) -> String {
    parse_rfc3339(raw).unwrap_or_else(|| raw.to_string())
}

pub(crate) fn is_five_minute_datetime(dt: &DateTime<chrono::FixedOffset>) -> bool {
    dt.minute() % EVENT_TIME_MINUTE_STEP == 0 && dt.second() == 0 && dt.nanosecond() == 0
}

pub(crate) fn parse_datetime_with_validation(
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

pub(crate) fn parse_event_datetime(payload: &Value, key: &str) -> Result<Option<String>, String> {
    parse_datetime_with_validation(payload, key, "事件时间", true)
}

pub(crate) fn parse_start_datetime(payload: &Value) -> Result<Option<String>, String> {
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
    Ok(None)
}

pub(crate) fn has_start_datetime(payload: &Value) -> bool {
    payload
        .get("startAt")
        .map(|v| !v.is_null())
        .unwrap_or(false)
        || recurrence_payload(payload)
            .get("startAt")
            .map(|v| !v.is_null())
            .unwrap_or(false)
}

pub(crate) fn parse_scope(payload: &Value) -> String {
    match payload.get("scope").and_then(Value::as_str) {
        Some(SCOPE_FUTURE_INSTANCES) => SCOPE_FUTURE_INSTANCES.to_string(),
        _ => SCOPE_THIS_INSTANCE.to_string(),
    }
}

pub(crate) fn normalize_series_kind(value: Option<&str>) -> String {
    match value.unwrap_or(SERIES_KIND_ONE_OFF) {
        SERIES_KIND_RECURRING => SERIES_KIND_RECURRING.to_string(),
        _ => SERIES_KIND_ONE_OFF.to_string(),
    }
}

pub(crate) fn parse_assignee_ids(payload: &Value) -> Vec<i64> {
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

pub(crate) fn parse_links(payload: &Value) -> Option<Vec<Value>> {
    payload.get("links").and_then(Value::as_array).cloned()
}

pub(crate) fn normalize_priority(value: Option<&str>) -> Result<String, String> {
    let p = value.unwrap_or("P2").trim().to_uppercase();
    if PRIORITIES.contains(&p.as_str()) {
        Ok(p)
    } else {
        Err("优先级必须是 P0/P1/P2/P3".to_string())
    }
}

pub(crate) fn normalize_status(value: &str) -> Result<String, String> {
    match value {
        STATUS_PENDING | STATUS_IN_PROGRESS | STATUS_COMPLETED => Ok(value.to_string()),
        _ => Err("状态不合法".to_string()),
    }
}

/// A1 归一化：in_progress 视同 pending
pub(crate) fn normalize_status_a1(status: &str) -> &str {
    if status == STATUS_IN_PROGRESS {
        STATUS_PENDING
    } else {
        status
    }
}

/// 待处理状态判定；`widget` 模块复用，禁止在其他模块重写。
pub fn is_open_status(status: &str) -> bool {
    status == STATUS_PENDING || status == STATUS_IN_PROGRESS
}

pub(crate) fn can_transit(current: &str, next: &str) -> bool {
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

pub(crate) fn can_transit_for_kind(current: &str, next: &str, kind: &str) -> bool {
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

pub(crate) fn item_priority_rank(item: &Value) -> i32 {
    match item.get("priority").and_then(Value::as_str).unwrap_or("P2") {
        "P0" => 0,
        "P1" => 1,
        "P2" => 2,
        _ => 3,
    }
}

pub(crate) fn item_sort_time(item: &Value) -> String {
    item.get("displayAt")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

pub(crate) fn item_pinned_rank(item: &Value) -> i32 {
    if item.get("pinned").and_then(Value::as_bool).unwrap_or(false) {
        0
    } else {
        1
    }
}

pub(crate) fn sort_item_rows(items: &mut [Value]) {
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

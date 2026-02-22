use chrono::{Datelike, Local, NaiveDate, TimeZone, Utc};
use serde_json::{json, Value};

fn date_diff(payload: &Value) -> Result<Value, String> {
    let start_str = payload["start"].as_str().unwrap_or_default();
    let end_str = payload["end"].as_str().unwrap_or_default();
    let start = NaiveDate::parse_from_str(start_str, "%Y-%m-%d")
        .map_err(|e| format!("起始日期格式错误: {e}"))?;
    let end = NaiveDate::parse_from_str(end_str, "%Y-%m-%d")
        .map_err(|e| format!("结束日期格式错误: {e}"))?;

    let duration = end.signed_duration_since(start);
    let total_days = duration.num_days().unsigned_abs();
    let hours = total_days * 24;
    let minutes = hours * 60;

    // Natural language: X年X月X天
    let (mut y, mut m, d);
    y = 0u32;
    m = 0u32;
    let (earlier, later) = if start <= end { (start, end) } else { (end, start) };
    let mut cursor = earlier;
    while cursor.with_year(cursor.year() + 1).map_or(false, |next| next <= later) {
        y += 1;
        cursor = cursor.with_year(cursor.year() + 1).unwrap();
    }
    while {
        let next_month = if cursor.month() == 12 {
            NaiveDate::from_ymd_opt(cursor.year() + 1, 1, cursor.day().min(28))
        } else {
            let next_m = cursor.month() + 1;
            let max_day = NaiveDate::from_ymd_opt(cursor.year(), next_m, 1)
                .and_then(|d| d.pred_opt())
                .map(|d| d.day())
                .unwrap_or(28);
            NaiveDate::from_ymd_opt(cursor.year(), next_m, cursor.day().min(max_day))
        };
        next_month.map_or(false, |nm| nm <= later)
    } {
        m += 1;
        cursor = if cursor.month() == 12 {
            NaiveDate::from_ymd_opt(cursor.year() + 1, 1, cursor.day().min(28)).unwrap()
        } else {
            let next_m = cursor.month() + 1;
            let max_day = NaiveDate::from_ymd_opt(cursor.year(), next_m, 1)
                .and_then(|d| d.pred_opt())
                .map(|d| d.day())
                .unwrap_or(28);
            NaiveDate::from_ymd_opt(cursor.year(), next_m, cursor.day().min(max_day)).unwrap()
        };
    }
    d = (later - cursor).num_days() as u32;

    let sign = if end < start { "-" } else { "" };
    let natural = format!("{sign}{y}年{m}月{d}天");

    Ok(json!({
        "days": duration.num_days(),
        "hours": duration.num_hours(),
        "minutes": duration.num_minutes(),
        "seconds": duration.num_seconds(),
        "natural": natural,
    }))
}

fn date_add(payload: &Value) -> Result<Value, String> {
    let date_str = payload["date"].as_str().unwrap_or_default();
    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|e| format!("日期格式错误: {e}"))?;

    let add = &payload["add"];
    let days = add["days"].as_i64().unwrap_or(0);
    let hours = add["hours"].as_i64().unwrap_or(0);
    let minutes = add["minutes"].as_i64().unwrap_or(0);

    let dt = date.and_hms_opt(0, 0, 0).unwrap();
    let result = dt
        + chrono::Duration::days(days)
        + chrono::Duration::hours(hours)
        + chrono::Duration::minutes(minutes);

    Ok(json!({
        "result": result.format("%Y-%m-%d").to_string(),
        "resultDatetime": result.format("%Y-%m-%dT%H:%M:%S").to_string(),
    }))
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "timestamp_to_date" => {
            let input = payload["input"].as_i64().unwrap_or_default();
            let ts_ms = if input < 1_000_000_000_000 { input * 1000 } else { input };
            let dt_local = Local
                .timestamp_millis_opt(ts_ms)
                .single()
                .ok_or("invalid timestamp".to_string())?;
            Ok(json!(dt_local.format("%Y-%m-%d %H:%M:%S").to_string()))
        }
        "date_to_timestamp" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let dt = chrono::DateTime::parse_from_rfc3339(input)
                .map(|d| d.with_timezone(&Utc))
                .or_else(|_| {
                    Local
                        .datetime_from_str(input, "%Y-%m-%d %H:%M:%S")
                        .map(|d| d.with_timezone(&Utc))
                })
                .map_err(|e| format!("invalid datetime: {e}"))?;
            Ok(json!({
                "seconds": dt.timestamp(),
                "milliseconds": dt.timestamp_millis()
            }))
        }
        "date_diff" => date_diff(payload),
        "date_add" => date_add(payload),
        _ => Err(format!("unsupported time action: {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn timestamp_to_date_should_support_seconds_and_milliseconds() {
        let out_sec = execute("timestamp_to_date", &json!({ "input": 1_700_000_000_i64 })).expect("sec");
        let out_ms = execute("timestamp_to_date", &json!({ "input": 1_700_000_000_000_i64 })).expect("ms");
        assert!(out_sec.as_str().unwrap_or_default().len() >= 19);
        assert!(out_ms.as_str().unwrap_or_default().len() >= 19);
    }

    #[test]
    fn date_to_timestamp_should_support_rfc3339_and_common_format() {
        let out = execute("date_to_timestamp", &json!({ "input": "2024-01-01T00:00:00Z" })).expect("rfc3339");
        assert!(out["seconds"].as_i64().unwrap_or_default() > 0);
        assert!(out["milliseconds"].as_i64().unwrap_or_default() > 0);

        let out = execute("date_to_timestamp", &json!({ "input": "2024-01-01 00:00:00" })).expect("common format");
        assert!(out["seconds"].as_i64().unwrap_or_default() > 0);
    }

    #[test]
    fn invalid_datetime_should_fail() {
        let err = execute("date_to_timestamp", &json!({ "input": "bad-time" })).expect_err("must fail");
        assert!(err.contains("invalid datetime"));
    }

    #[test]
    fn date_diff_basic() {
        let r = execute("date_diff", &json!({"start": "2026-01-01", "end": "2026-02-21"})).unwrap();
        assert_eq!(r["days"], 51);
        assert!(r["hours"].as_i64().unwrap() > 0);
        assert!(r["natural"].as_str().unwrap().contains("1"));
    }

    #[test]
    fn date_diff_same_day() {
        let r = execute("date_diff", &json!({"start": "2026-01-01", "end": "2026-01-01"})).unwrap();
        assert_eq!(r["days"], 0);
    }

    #[test]
    fn date_add_days() {
        let r = execute("date_add", &json!({"date": "2026-02-21", "add": {"days": 30, "hours": 0, "minutes": 0}})).unwrap();
        assert_eq!(r["result"], "2026-03-23");
    }

    #[test]
    fn date_add_negative() {
        let r = execute("date_add", &json!({"date": "2026-02-21", "add": {"days": -10, "hours": 0, "minutes": 0}})).unwrap();
        assert_eq!(r["result"], "2026-02-11");
    }
}

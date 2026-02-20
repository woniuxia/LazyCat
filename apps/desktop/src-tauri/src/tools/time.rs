use chrono::{Local, TimeZone, Utc};
use serde_json::{json, Value};

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
        _ => Err(format!("unsupported time action: {action}")),
    }
}

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
}

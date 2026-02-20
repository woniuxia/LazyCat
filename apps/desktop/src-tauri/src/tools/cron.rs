use chrono::Local;
use cron::Schedule;
use serde_json::{json, Value};
use std::str::FromStr;

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "generate" => {
            let second = payload["second"].as_str().unwrap_or("0");
            let minute = payload["minute"].as_str().unwrap_or("*");
            let hour = payload["hour"].as_str().unwrap_or("*");
            let day_of_month = payload["dayOfMonth"].as_str().unwrap_or("*");
            let month = payload["month"].as_str().unwrap_or("*");
            let day_of_week = payload["dayOfWeek"].as_str().unwrap_or("*");
            Ok(json!(format!(
                "{second} {minute} {hour} {day_of_month} {month} {day_of_week}"
            )))
        }
        "preview" => {
            let expression = payload["expression"].as_str().unwrap_or("0 * * * * *");
            let count = payload["count"].as_u64().unwrap_or(5) as usize;
            let schedule = Schedule::from_str(expression)
                .map_err(|e| format!("无效的 Cron 表达式: {e}"))?;
            let now = Local::now();
            let times: Vec<String> = schedule
                .after(&now)
                .take(count)
                .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                .collect();
            Ok(json!(times))
        }
        "parse" => {
            let expression = payload["expression"].as_str().unwrap_or("").trim();
            let parts: Vec<&str> = expression.split_whitespace().collect();
            if parts.len() != 6 {
                return Err(format!(
                    "表达式必须包含 6 个字段（秒 分 时 日 月 周），当前 {} 个",
                    parts.len()
                ));
            }
            Schedule::from_str(expression)
                .map_err(|e| format!("无效的 Cron 表达式: {e}"))?;
            Ok(json!({
                "second":     parts[0],
                "minute":     parts[1],
                "hour":       parts[2],
                "dayOfMonth": parts[3],
                "month":      parts[4],
                "dayOfWeek":  parts[5]
            }))
        }
        _ => Err(format!("unsupported cron action: {action}")),
    }
}

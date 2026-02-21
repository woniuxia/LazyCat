use chrono::{DateTime, Local, TimeZone, Utc};
use chrono_tz::Tz;
use cron::Schedule;
use serde_json::{json, Value};
use std::str::FromStr;

#[derive(Debug)]
struct NormalizedCron {
    normalized_expression: String,
    original_field_count: usize,
    warnings: Vec<String>,
    second: String,
    minute: String,
    hour: String,
    day_of_month: String,
    month: String,
    day_of_week: String,
}

enum PreviewTimezone {
    Local,
    Utc,
    Iana(Tz),
}

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
            let expression = payload["expression"].as_str().unwrap_or("0 * * * * *").trim();
            let count = payload["count"].as_u64().unwrap_or(5) as usize;

            let normalized = normalize_expression(expression)?;
            let schedule = parse_schedule(&normalized.normalized_expression)?;

            let now = Local::now();
            let times: Vec<String> = schedule
                .after(&now)
                .take(count)
                .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                .collect();
            Ok(json!(times))
        }
        "preview_v2" => {
            let expression = payload["expression"].as_str().unwrap_or("").trim();
            let count = payload["count"].as_u64().unwrap_or(8) as usize;
            let timezone_input = payload["timezone"].as_str().unwrap_or("local");

            let normalized = normalize_expression(expression)?;
            let schedule = parse_schedule(&normalized.normalized_expression)?;
            let (timezone, timezone_name, timezone_warning) = parse_timezone(timezone_input);

            let items = match timezone {
                PreviewTimezone::Local => collect_preview_items(&schedule, Local::now(), count),
                PreviewTimezone::Utc => collect_preview_items(&schedule, Utc::now(), count),
                PreviewTimezone::Iana(tz) => {
                    collect_preview_items(&schedule, Utc::now().with_timezone(&tz), count)
                }
            };

            let mut warnings = normalized.warnings.clone();
            if let Some(w) = timezone_warning {
                warnings.push(w);
            }

            Ok(json!({
                "normalizedExpression": normalized.normalized_expression,
                "timezone": timezone_name,
                "items": items,
                "warnings": warnings,
            }))
        }
        "normalize" => {
            let expression = payload["expression"].as_str().unwrap_or("").trim();
            let normalized = normalize_expression(expression)?;
            parse_schedule(&normalized.normalized_expression)?;

            Ok(json!({
                "ok": true,
                "normalizedExpression": normalized.normalized_expression,
                "fieldCount": normalized.original_field_count,
                "canonicalFieldCount": 6,
                "parts": {
                    "second": normalized.second,
                    "minute": normalized.minute,
                    "hour": normalized.hour,
                    "dayOfMonth": normalized.day_of_month,
                    "month": normalized.month,
                    "dayOfWeek": normalized.day_of_week,
                },
                "warnings": normalized.warnings,
            }))
        }
        "describe" => {
            let expression = payload["expression"].as_str().unwrap_or("").trim();
            let normalized = normalize_expression(expression)?;
            parse_schedule(&normalized.normalized_expression)?;

            let summary = summarize_expression(&normalized);
            let details = vec![
                format!("秒: {}", describe_field(&normalized.second, "秒")),
                format!("分钟: {}", describe_field(&normalized.minute, "分钟")),
                format!("小时: {}", describe_field(&normalized.hour, "小时")),
                format!("日: {}", describe_field(&normalized.day_of_month, "日")),
                format!("月: {}", describe_field(&normalized.month, "月")),
                format!("周: {}", describe_field(&normalized.day_of_week, "周")),
            ];

            Ok(json!({
                "normalizedExpression": normalized.normalized_expression,
                "summary": summary,
                "details": details,
                "warnings": normalized.warnings,
            }))
        }
        "parse" => {
            let expression = payload["expression"].as_str().unwrap_or("").trim();
            let normalized = normalize_expression(expression)?;
            parse_schedule(&normalized.normalized_expression)?;

            Ok(json!({
                "second": normalized.second,
                "minute": normalized.minute,
                "hour": normalized.hour,
                "dayOfMonth": normalized.day_of_month,
                "month": normalized.month,
                "dayOfWeek": normalized.day_of_week,
                "normalizedExpression": normalized.normalized_expression,
                "warnings": normalized.warnings,
            }))
        }
        _ => Err(format!("unsupported cron action: {action}")),
    }
}

fn normalize_expression(expression: &str) -> Result<NormalizedCron, String> {
    let parts: Vec<&str> = expression.split_whitespace().collect();
    let mut warnings = Vec::new();

    if parts.len() == 7 {
        return Err("当前仅支持 Spring 6 字段（秒 分 时 日 月 周），不支持 year（第 7 字段）。".to_string());
    }

    if parts.len() != 5 && parts.len() != 6 {
        return Err(format!(
            "表达式必须包含 5 或 6 个字段（秒 分 时 日 月 周）。当前为 {} 个字段。",
            parts.len()
        ));
    }

    let original_field_count = parts.len();
    let normalized_parts: Vec<String> = if parts.len() == 5 {
        warnings.push("检测到 5 字段表达式，已自动补齐秒字段为 0。".to_string());
        let mut result = Vec::with_capacity(6);
        result.push("0".to_string());
        result.extend(parts.into_iter().map(|p| p.to_string()));
        result
    } else {
        parts.into_iter().map(|p| p.to_string()).collect()
    };

    Ok(NormalizedCron {
        normalized_expression: normalized_parts.join(" "),
        original_field_count,
        warnings,
        second: normalized_parts[0].clone(),
        minute: normalized_parts[1].clone(),
        hour: normalized_parts[2].clone(),
        day_of_month: normalized_parts[3].clone(),
        month: normalized_parts[4].clone(),
        day_of_week: normalized_parts[5].clone(),
    })
}

fn parse_schedule(expression: &str) -> Result<Schedule, String> {
    Schedule::from_str(expression).map_err(|e| format!("无效的 Cron 表达式: {e}"))
}

fn parse_timezone(input: &str) -> (PreviewTimezone, String, Option<String>) {
    let normalized = input.trim();
    if normalized.is_empty() || normalized.eq_ignore_ascii_case("local") {
        return (PreviewTimezone::Local, "local".to_string(), None);
    }

    if normalized.eq_ignore_ascii_case("utc") {
        return (PreviewTimezone::Utc, "UTC".to_string(), None);
    }

    match normalized.parse::<Tz>() {
        Ok(tz) => (PreviewTimezone::Iana(tz), normalized.to_string(), None),
        Err(_) => (
            PreviewTimezone::Local,
            "local".to_string(),
            Some(format!("无法识别时区 `{normalized}`，已回退到本地时区。")),
        ),
    }
}

fn collect_preview_items<TzType>(schedule: &Schedule, now: DateTime<TzType>, count: usize) -> Vec<Value>
where
    TzType: TimeZone,
    TzType::Offset: std::fmt::Display,
{
    schedule
        .after(&now)
        .take(count)
        .map(|t| {
            json!({
                "iso": t.to_rfc3339(),
                "display": t.format("%Y-%m-%d %H:%M:%S %:z").to_string(),
                "epochMs": t.timestamp_millis(),
            })
        })
        .collect()
}

fn summarize_expression(normalized: &NormalizedCron) -> String {
    if normalized.second == "0"
        && normalized.minute == "*"
        && normalized.hour == "*"
        && normalized.day_of_month == "*"
        && normalized.month == "*"
        && normalized.day_of_week == "*"
    {
        return "每分钟执行一次".to_string();
    }

    if normalized.second == "0"
        && normalized.minute.starts_with("*/")
        && normalized.hour == "*"
        && normalized.day_of_month == "*"
        && normalized.month == "*"
        && normalized.day_of_week == "*"
    {
        let step = normalized.minute.trim_start_matches("*/");
        if !step.is_empty() {
            return format!("每 {step} 分钟执行一次");
        }
    }

    if normalized.second == "0"
        && is_single_number(&normalized.minute)
        && is_single_number(&normalized.hour)
        && normalized.day_of_month == "*"
        && normalized.month == "*"
        && normalized.day_of_week == "*"
    {
        return format!(
            "每天 {:0>2}:{:0>2} 执行",
            normalized.hour, normalized.minute
        );
    }

    if normalized.second == "0"
        && is_single_number(&normalized.minute)
        && is_single_number(&normalized.hour)
        && normalized.day_of_month == "*"
        && normalized.month == "*"
        && normalized.day_of_week == "1-5"
    {
        return format!(
            "工作日 {:0>2}:{:0>2} 执行",
            normalized.hour, normalized.minute
        );
    }

    if normalized.second == "0"
        && is_single_number(&normalized.minute)
        && is_single_number(&normalized.hour)
        && normalized.day_of_month == "1"
        && normalized.month == "*"
        && normalized.day_of_week == "*"
    {
        return format!(
            "每月 1 日 {:0>2}:{:0>2} 执行",
            normalized.hour, normalized.minute
        );
    }

    "按 Cron 表达式执行".to_string()
}

fn describe_field(value: &str, unit: &str) -> String {
    if value == "*" {
        return "任意值".to_string();
    }
    if let Some(step) = value.strip_prefix("*/") {
        return format!("每 {step} {unit}");
    }
    if value.contains(',') {
        return format!("枚举值: {value}");
    }
    if value.contains('-') {
        return format!("范围: {value}");
    }
    if value.contains('/') {
        return format!("步进: {value}");
    }
    format!("固定值: {value}")
}

fn is_single_number(value: &str) -> bool {
    value.chars().all(|c| c.is_ascii_digit()) && !value.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_6_fields_should_pass() {
        let result = normalize_expression("0 */5 * * * *").expect("normalize should pass");
        assert_eq!(result.normalized_expression, "0 */5 * * * *");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn normalize_5_fields_should_add_second() {
        let result = normalize_expression("*/5 * * * *").expect("normalize should pass");
        assert_eq!(result.normalized_expression, "0 */5 * * * *");
        assert_eq!(result.second, "0");
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn normalize_7_fields_should_fail() {
        let err = normalize_expression("0 */5 * * * * 2026").expect_err("should fail");
        assert!(err.contains("不支持 year"));
    }

    #[test]
    fn parse_schedule_invalid_should_fail() {
        let err = parse_schedule("0 61 * * * *").expect_err("should fail");
        assert!(err.contains("无效的 Cron 表达式"));
    }

    #[test]
    fn parse_timezone_should_fallback_to_local() {
        let (_, zone, warning) = parse_timezone("not-a-timezone");
        assert_eq!(zone, "local");
        assert!(warning.is_some());
    }

    #[test]
    fn summarize_common_pattern() {
        let cron = normalize_expression("0 */10 * * * *").expect("normalize");
        assert_eq!(summarize_expression(&cron), "每 10 分钟执行一次");
    }
}

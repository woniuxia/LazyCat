use chrono::{DateTime, Local, TimeZone, Utc};
use chrono_tz::Tz;
use cron::Schedule;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::str::FromStr;

#[derive(Debug)]
struct NormalizedCron {
    normalized_expression: String,
    preview_expression: String,
    original_field_count: usize,
    canonical_field_count: usize,
    warnings: Vec<String>,
    dialect: CronDialect,
    second: String,
    minute: String,
    hour: String,
    day_of_month: String,
    month: String,
    day_of_week: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CronDialect {
    Linux5,
    Spring6,
    Quartz,
}

impl CronDialect {
    fn from_payload(payload: &Value) -> Result<Self, String> {
        match payload["standard"].as_str().unwrap_or("spring6") {
            "linux5" | "linux" | "crontab" => Ok(Self::Linux5),
            "spring6" | "spring" => Ok(Self::Spring6),
            "quartz" | "quartz6" => Ok(Self::Quartz),
            value => Err(format!("不支持的 Cron 目标环境: {value}")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Linux5 => "linux5",
            Self::Spring6 => "spring6",
            Self::Quartz => "quartz",
        }
    }
}

enum PreviewTimezone {
    Local,
    Utc,
    Iana(Tz),
}

const ACTIONS: &[&str] = &[
    "generate",
    "preview",
    "preview_v2",
    "normalize",
    "describe",
    "parse",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported cron action: {action}"));
    }
    match action {
        "generate" => {
            let dialect = CronDialect::from_payload(payload)?;
            let second = payload["second"].as_str().unwrap_or("0");
            let minute = payload["minute"].as_str().unwrap_or("*");
            let hour = payload["hour"].as_str().unwrap_or("*");
            let day_of_month = payload["dayOfMonth"].as_str().unwrap_or("*");
            let month = payload["month"].as_str().unwrap_or("*");
            let day_of_week = payload["dayOfWeek"].as_str().unwrap_or("*");
            let expression =
                format!("{second} {minute} {hour} {day_of_month} {month} {day_of_week}");
            let normalized = normalize_expression(&expression, dialect)?;
            parse_schedule(&normalized.preview_expression)?;
            Ok(json!(normalized.normalized_expression))
        }
        "preview" => {
            let expression = payload["expression"]
                .as_str()
                .unwrap_or("0 * * * * *")
                .trim();
            let count = payload["count"].as_u64().unwrap_or(5) as usize;

            let normalized = normalize_expression(expression, CronDialect::Spring6)?;
            let schedule = parse_schedule(&normalized.preview_expression)?;

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
            let dialect = CronDialect::from_payload(payload)?;

            let normalized = normalize_expression(expression, dialect)?;
            let schedule = parse_schedule(&normalized.preview_expression)?;
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
                "standard": normalized.dialect.as_str(),
                "timezone": timezone_name,
                "items": items,
                "warnings": warnings,
            }))
        }
        "normalize" => {
            let expression = payload["expression"].as_str().unwrap_or("").trim();
            let dialect = CronDialect::from_payload(payload)?;
            let normalized = normalize_expression(expression, dialect)?;
            parse_schedule(&normalized.preview_expression)?;

            Ok(json!({
                "ok": true,
                "normalizedExpression": normalized.normalized_expression,
                "fieldCount": normalized.original_field_count,
                "canonicalFieldCount": normalized.canonical_field_count,
                "standard": normalized.dialect.as_str(),
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
            let dialect = CronDialect::from_payload(payload)?;
            let normalized = normalize_expression(expression, dialect)?;
            parse_schedule(&normalized.preview_expression)?;

            let summary = summarize_expression(&normalized);
            let mut details = Vec::with_capacity(normalized.canonical_field_count);
            if normalized.dialect != CronDialect::Linux5 {
                details.push(format!("秒: {}", describe_field(&normalized.second, "秒")));
            }
            details.extend([
                format!("分钟: {}", describe_field(&normalized.minute, "分钟")),
                format!("小时: {}", describe_field(&normalized.hour, "小时")),
                format!("日: {}", describe_field(&normalized.day_of_month, "日")),
                format!("月: {}", describe_field(&normalized.month, "月")),
                format!("周: {}", describe_field(&normalized.day_of_week, "周")),
            ]);

            Ok(json!({
                "normalizedExpression": normalized.normalized_expression,
                "standard": normalized.dialect.as_str(),
                "summary": summary,
                "details": details,
                "warnings": normalized.warnings,
            }))
        }
        "parse" => {
            let expression = payload["expression"].as_str().unwrap_or("").trim();
            let dialect = CronDialect::from_payload(payload)?;
            let normalized = normalize_expression(expression, dialect)?;
            parse_schedule(&normalized.preview_expression)?;

            Ok(json!({
                "second": normalized.second,
                "minute": normalized.minute,
                "hour": normalized.hour,
                "dayOfMonth": normalized.day_of_month,
                "month": normalized.month,
                "dayOfWeek": normalized.day_of_week,
                "normalizedExpression": normalized.normalized_expression,
                "standard": normalized.dialect.as_str(),
                "warnings": normalized.warnings,
            }))
        }
        _ => Err(format!("unsupported cron action: {action}")),
    }
}

fn normalize_expression(expression: &str, dialect: CronDialect) -> Result<NormalizedCron, String> {
    let parts: Vec<&str> = expression.split_whitespace().collect();
    let mut warnings = Vec::new();

    if parts.len() == 7 {
        return Err("当前不支持包含 year 的 7 字段表达式，请使用 5 或 6 字段。".to_string());
    }

    if parts.len() != 5 && parts.len() != 6 {
        return Err(format!(
            "表达式必须包含 5 或 6 个字段。当前为 {} 个字段。",
            parts.len()
        ));
    }

    let original_field_count = parts.len();
    let mut normalized_parts: Vec<String> = if parts.len() == 5 {
        let mut result = Vec::with_capacity(6);
        result.push("0".to_string());
        result.extend(parts.into_iter().map(|p| p.to_string()));
        result
    } else {
        parts.into_iter().map(|p| p.to_string()).collect()
    };

    validate_supported_syntax(&normalized_parts[3], &normalized_parts[5])?;
    match dialect {
        CronDialect::Linux5 => {
            if normalized_parts[0] != "0" {
                return Err(
                    "Linux Crontab 不支持秒字段；请将秒设为 0，或改用 Spring / Quartz。"
                        .to_string(),
                );
            }
            if normalized_parts.iter().any(|part| part.contains('?')) {
                return Err("Linux Crontab 不支持 `?`，请使用 `*`。".to_string());
            }
            if original_field_count == 6 {
                warnings.push("已移除秒字段，输出 Linux Crontab 5 字段表达式。".to_string());
            }
        }
        CronDialect::Spring6 => {
            if original_field_count == 5 {
                warnings.push("已补齐秒字段 0，输出 Spring 6 字段表达式。".to_string());
            }
        }
        CronDialect::Quartz => {
            if original_field_count == 5 {
                warnings.push("已补齐秒字段 0，输出 Quartz 6 字段表达式。".to_string());
            }
            normalize_quartz_day_fields(&mut normalized_parts)?;
        }
    }

    if !is_any(&normalized_parts[3]) && !is_any(&normalized_parts[5]) {
        warnings.push(
            "日和周同时指定在不同 Cron 实现中的组合语义不一致，建议只指定其中一项。".to_string(),
        );
    }

    let preview_expression = [
        normalized_parts[0].clone(),
        normalized_parts[1].clone(),
        normalized_parts[2].clone(),
        normalized_parts[3].replace('?', "*"),
        normalized_parts[4].clone(),
        day_of_week_for_preview(&normalized_parts[5], dialect)?,
    ]
    .join(" ");
    let normalized_expression = match dialect {
        CronDialect::Linux5 => normalized_parts[1..].join(" "),
        CronDialect::Spring6 | CronDialect::Quartz => normalized_parts.join(" "),
    };

    Ok(NormalizedCron {
        normalized_expression,
        preview_expression,
        original_field_count,
        canonical_field_count: if dialect == CronDialect::Linux5 { 5 } else { 6 },
        warnings,
        dialect,
        second: normalized_parts[0].clone(),
        minute: normalized_parts[1].clone(),
        hour: normalized_parts[2].clone(),
        day_of_month: normalized_parts[3].clone(),
        month: normalized_parts[4].clone(),
        day_of_week: normalized_parts[5].clone(),
    })
}

fn validate_supported_syntax(day_of_month: &str, day_of_week: &str) -> Result<(), String> {
    let dom = day_of_month.to_ascii_uppercase();
    let dow = day_of_week.to_ascii_uppercase();
    let has_special_dom = dom == "L" || dom == "LW" || dom.ends_with('W') || dom.contains("L-");
    let has_special_dow = dow.contains('#') || dow.ends_with('L');
    if has_special_dom || has_special_dow {
        return Err(
            "当前兼容生成器暂不支持 L、W、# 特殊语法，请改用明确的日期或星期。".to_string(),
        );
    }
    Ok(())
}

fn normalize_quartz_day_fields(parts: &mut [String]) -> Result<(), String> {
    let dom = parts[3].as_str();
    let dow = parts[5].as_str();
    let dom_unspecified = dom == "?";
    let dow_unspecified = dow == "?";
    let dom_wildcard = dom == "*";
    let dow_wildcard = dow == "*";

    if dom_unspecified && dow_unspecified {
        return Err("Quartz 的日和周字段不能同时为 `?`。".to_string());
    }
    if !dom_unspecified && !dow_unspecified && !dom_wildcard && !dow_wildcard {
        return Err("Quartz 不能同时指定日和周；请将其中一项设为 `?`。".to_string());
    }
    if dom_wildcard && dow_wildcard {
        parts[5] = "?".to_string();
    } else if !dom_wildcard && !dom_unspecified && dow_wildcard {
        parts[5] = "?".to_string();
    } else if !dow_wildcard && !dow_unspecified && dom_wildcard {
        parts[3] = "?".to_string();
    }
    Ok(())
}

fn is_any(value: &str) -> bool {
    value == "*" || value == "?"
}

fn day_of_week_for_preview(value: &str, dialect: CronDialect) -> Result<String, String> {
    let value = value.replace('?', "*");
    let has_alpha = value.chars().any(|ch| ch.is_ascii_alphabetic());
    let has_digit = value.chars().any(|ch| ch.is_ascii_digit());
    if has_alpha && has_digit {
        return Err("周字段请统一使用英文缩写或数字，不要混用两种星期编号规则。".to_string());
    }
    if value == "*" || has_alpha {
        return Ok(value);
    }

    let (min, max, shift) = match dialect {
        CronDialect::Quartz => (1_u32, 7_u32, false),
        CronDialect::Linux5 | CronDialect::Spring6 => (0_u32, 7_u32, true),
    };
    let mut mapped = BTreeSet::new();
    for segment in value.split(',') {
        let (base, step) = match segment.split_once('/') {
            Some((base, step)) => {
                let step = step
                    .parse::<u32>()
                    .map_err(|_| "周字段步长无效".to_string())?;
                if step == 0 {
                    return Err("周字段步长不能为 0。".to_string());
                }
                (base, step)
            }
            None => (segment, 1),
        };
        let (start, end) = if base == "*" {
            (min, if shift { 6 } else { max })
        } else if let Some((start, end)) = base.split_once('-') {
            let start = parse_dow_number(start, min, max)?;
            let end = parse_dow_number(end, min, max)?;
            if start > end {
                return Err("周字段范围起始值不能大于结束值。".to_string());
            }
            (start, end)
        } else {
            let start = parse_dow_number(base, min, max)?;
            let end = if segment.contains('/') { max } else { start };
            (start, end)
        };

        let mut current = start;
        while current <= end {
            let preview_value = if shift {
                if current == 0 || current == 7 {
                    1
                } else {
                    current + 1
                }
            } else {
                current
            };
            mapped.insert(preview_value);
            current = current.saturating_add(step);
            if current == u32::MAX {
                break;
            }
        }
    }
    Ok(mapped
        .into_iter()
        .map(|item| item.to_string())
        .collect::<Vec<_>>()
        .join(","))
}

fn parse_dow_number(value: &str, min: u32, max: u32) -> Result<u32, String> {
    let number = value
        .parse::<u32>()
        .map_err(|_| "周字段格式无效".to_string())?;
    if !(min..=max).contains(&number) {
        return Err(format!("周字段数字必须在 {min}-{max} 范围内。"));
    }
    Ok(number)
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

fn collect_preview_items<TzType>(
    schedule: &Schedule,
    now: DateTime<TzType>,
    count: usize,
) -> Vec<Value>
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
        && is_any(&normalized.day_of_month)
        && normalized.month == "*"
        && is_any(&normalized.day_of_week)
    {
        return "每分钟执行一次".to_string();
    }

    if normalized.second == "0"
        && normalized.minute.starts_with("*/")
        && normalized.hour == "*"
        && is_any(&normalized.day_of_month)
        && normalized.month == "*"
        && is_any(&normalized.day_of_week)
    {
        let step = normalized.minute.trim_start_matches("*/");
        if !step.is_empty() {
            return format!("每 {step} 分钟执行一次");
        }
    }

    if normalized.second == "0"
        && is_single_number(&normalized.minute)
        && is_single_number(&normalized.hour)
        && is_any(&normalized.day_of_month)
        && normalized.month == "*"
        && is_any(&normalized.day_of_week)
    {
        return format!(
            "每天 {:0>2}:{:0>2} 执行",
            normalized.hour, normalized.minute
        );
    }

    if normalized.second == "0"
        && is_single_number(&normalized.minute)
        && is_single_number(&normalized.hour)
        && is_any(&normalized.day_of_month)
        && normalized.month == "*"
        && is_sun_to_thu_day_of_week(&normalized.day_of_week)
    {
        return format!(
            "周日-周四 {:0>2}:{:0>2} 执行",
            normalized.hour, normalized.minute
        );
    }

    if normalized.second == "0"
        && is_single_number(&normalized.minute)
        && is_single_number(&normalized.hour)
        && is_any(&normalized.day_of_month)
        && normalized.month == "*"
        && is_workday_day_of_week(&normalized.day_of_week)
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
        && is_any(&normalized.day_of_week)
    {
        return format!(
            "每月 1 日 {:0>2}:{:0>2} 执行",
            normalized.hour, normalized.minute
        );
    }

    "按 Cron 表达式执行".to_string()
}

fn is_workday_day_of_week(value: &str) -> bool {
    let normalized = value.trim().replace(' ', "").to_lowercase();
    matches!(
        normalized.as_str(),
        // 数字语义下的工作日：2=周一 ... 6=周五
        "2-6" | "2,3,4,5,6"
            // 推荐写法（无歧义）
            | "mon-fri"
            | "mon,tue,wed,thu,fri"
            | "mon,tues,wed,thu,fri"
    )
}

fn is_sun_to_thu_day_of_week(value: &str) -> bool {
    let normalized = value.trim().replace(' ', "").to_lowercase();
    matches!(normalized.as_str(), "1-5" | "1,2,3,4,5")
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
        let result = normalize_expression("0 */5 * * * *", CronDialect::Spring6)
            .expect("normalize should pass");
        assert_eq!(result.normalized_expression, "0 */5 * * * *");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn normalize_5_fields_should_add_second() {
        let result = normalize_expression("*/5 * * * *", CronDialect::Spring6)
            .expect("normalize should pass");
        assert_eq!(result.normalized_expression, "0 */5 * * * *");
        assert_eq!(result.second, "0");
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn normalize_7_fields_should_fail() {
        let err = normalize_expression("0 */5 * * * * 2026", CronDialect::Spring6)
            .expect_err("should fail");
        assert!(err.contains("7 字段"));
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
        let cron = normalize_expression("0 */10 * * * *", CronDialect::Spring6).expect("normalize");
        assert_eq!(summarize_expression(&cron), "每 10 分钟执行一次");
    }

    #[test]
    fn linux_should_emit_five_fields() {
        let cron =
            normalize_expression("0 30 9 * * Mon-Fri", CronDialect::Linux5).expect("normalize");
        assert_eq!(cron.normalized_expression, "30 9 * * Mon-Fri");
        assert_eq!(cron.preview_expression, "0 30 9 * * Mon-Fri");

        let numeric =
            normalize_expression("0 30 9 * * 1-5", CronDialect::Linux5).expect("normalize");
        assert_eq!(numeric.preview_expression, "0 30 9 * * 2,3,4,5,6");
    }

    #[test]
    fn linux_should_reject_non_zero_seconds() {
        let err =
            normalize_expression("*/30 * * * * *", CronDialect::Linux5).expect_err("should fail");
        assert!(err.contains("不支持秒字段"));
    }

    #[test]
    fn quartz_should_add_question_mark() {
        let cron =
            normalize_expression("0 0 9 * * Mon-Fri", CronDialect::Quartz).expect("normalize");
        assert_eq!(cron.normalized_expression, "0 0 9 ? * Mon-Fri");
        assert_eq!(cron.preview_expression, "0 0 9 * * Mon-Fri");
    }

    #[test]
    fn quartz_should_reject_both_day_fields() {
        let err =
            normalize_expression("0 0 9 1 * Mon", CronDialect::Quartz).expect_err("should fail");
        assert!(err.contains("不能同时指定日和周"));
    }

    #[test]
    fn unsupported_special_syntax_should_fail_explicitly() {
        let err =
            normalize_expression("0 0 23 L * *", CronDialect::Spring6).expect_err("should fail");
        assert!(err.contains("L、W、#"));
    }
}

use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

use super::types::{KeyValueRow, PreparedBody};

pub(crate) const MAX_TIMEOUT_MS: u64 = 120_000;
pub(crate) const MIN_TIMEOUT_MS: u64 = 100;
pub(crate) const MAX_RESPONSE_BODY_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const MAX_HISTORY_BODY_PREVIEW_BYTES: usize = 64 * 1024;
pub(crate) const MAX_HISTORY_ROWS: i64 = 200;
pub(crate) const MAX_HISTORY_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const MAX_HISTORY_NOTE_CHARS: usize = 2000;

pub(crate) fn validate_variable_name(name: &str) -> bool {
    let len = name.chars().count();
    if len == 0 || len > 64 {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub(crate) fn extract_variable_names(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut rest = input;
    while let Some(start) = rest.find("{{") {
        let after_start = &rest[start + 2..];
        if let Some(end) = after_start.find("}}") {
            let name = after_start[..end].trim().to_string();
            if seen.insert(name.clone()) {
                out.push(name);
            }
            rest = &after_start[end + 2..];
        } else {
            break;
        }
    }
    out
}

pub(crate) fn resolve_template(
    input: &str,
    vars: &HashMap<String, String>,
) -> Result<String, String> {
    let names = extract_variable_names(input);
    let mut missing = Vec::new();
    for name in &names {
        if !validate_variable_name(name) || !vars.contains_key(name) {
            missing.push(name.clone());
        }
    }
    if !missing.is_empty() {
        missing.sort();
        missing.dedup();
        return Err(format!("未解析变量: {}", missing.join(", ")));
    }

    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        if let Some(end) = after_start.find("}}") {
            let name = after_start[..end].trim();
            if let Some(value) = vars.get(name) {
                output.push_str(value);
            }
            rest = &after_start[end + 2..];
        } else {
            output.push_str(&rest[start..]);
            rest = "";
        }
    }
    output.push_str(rest);
    Ok(output)
}

pub(crate) fn is_absolute_http_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

pub(crate) fn append_query_rows(mut final_url: String, query: &[KeyValueRow]) -> String {
    let enabled: Vec<String> = query
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
        .map(|row| {
            format!(
                "{}={}",
                urlencoding::encode(row.key.trim()),
                urlencoding::encode(row.value.as_str())
            )
        })
        .collect();
    if enabled.is_empty() {
        return final_url;
    }
    let separator = if final_url.contains('?') { "&" } else { "?" };
    final_url.push_str(separator);
    final_url.push_str(&enabled.join("&"));
    final_url
}

pub(crate) fn build_final_url(
    base_url: &str,
    raw_url: &str,
    query: &[KeyValueRow],
) -> Result<String, String> {
    let url = raw_url.trim();
    if url.is_empty() {
        return Err("请求 URL 不能为空".to_string());
    }
    let combined = if is_absolute_http_url(url) {
        url.to_string()
    } else {
        let base = base_url.trim();
        if base.is_empty() {
            return Err("相对 URL 需要配置 BASE_URL".to_string());
        }
        if !is_absolute_http_url(base) {
            return Err("BASE_URL 只支持 http 或 https".to_string());
        }
        format!(
            "{}/{}",
            base.trim_end_matches('/'),
            url.trim_start_matches('/')
        )
    };
    if !is_absolute_http_url(&combined) {
        return Err("只支持 http 和 https 协议".to_string());
    }
    Ok(append_query_rows(combined, query))
}

pub(crate) fn has_header(headers: &[KeyValueRow], name: &str) -> bool {
    headers
        .iter()
        .any(|row| row.enabled && row.key.eq_ignore_ascii_case(name))
}

pub(crate) fn prepare_request_body(
    body_type: &str,
    body: &str,
    form: &[KeyValueRow],
    headers: &[KeyValueRow],
) -> Result<PreparedBody, String> {
    match body_type {
        "none" => Ok(PreparedBody {
            body: None,
            content_type: None,
        }),
        "json" => {
            serde_json::from_str::<Value>(body).map_err(|e| format!("JSON Body 格式错误: {e}"))?;
            Ok(PreparedBody {
                body: Some(body.as_bytes().to_vec()),
                content_type: if has_header(headers, "Content-Type") {
                    None
                } else {
                    Some("application/json".to_string())
                },
            })
        }
        "text" => Ok(PreparedBody {
            body: Some(body.as_bytes().to_vec()),
            content_type: if has_header(headers, "Content-Type") {
                None
            } else {
                Some("text/plain; charset=utf-8".to_string())
            },
        }),
        "form-urlencoded" => {
            let encoded: Vec<String> = form
                .iter()
                .filter(|row| row.enabled && !row.key.trim().is_empty())
                .map(|row| {
                    format!(
                        "{}={}",
                        urlencoding::encode(row.key.trim()),
                        urlencoding::encode(row.value.as_str())
                    )
                })
                .collect();
            Ok(PreparedBody {
                body: Some(encoded.join("&").into_bytes()),
                content_type: if has_header(headers, "Content-Type") {
                    None
                } else {
                    Some("application/x-www-form-urlencoded".to_string())
                },
            })
        }
        other => Err(format!("unsupported body type: {other}")),
    }
}

pub(crate) fn clamp_timeout_ms(value: u64) -> u64 {
    value.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS)
}

pub(crate) fn parse_i64(payload: &Value, key: &str) -> Result<i64, String> {
    payload[key]
        .as_i64()
        .ok_or_else(|| format!("{key} must be an integer"))
}

pub(crate) fn parse_ordered_ids(payload: &Value) -> Result<Vec<i64>, String> {
    let arr = payload["orderedIds"]
        .as_array()
        .ok_or_else(|| "orderedIds must be an array".to_string())?;
    let mut ids = Vec::with_capacity(arr.len());
    let mut seen = HashSet::new();
    for item in arr {
        let id = item
            .as_i64()
            .ok_or_else(|| "orderedIds must contain integers".to_string())?;
        if !seen.insert(id) {
            return Err("排序列表包含重复项".to_string());
        }
        ids.push(id);
    }
    Ok(ids)
}

pub(crate) fn parse_name(payload: &Value, key: &str) -> Result<String, String> {
    let value = payload[key].as_str().unwrap_or_default().trim().to_string();
    if value.is_empty() {
        return Err(format!("{key} 不能为空"));
    }
    Ok(value)
}

pub(crate) fn serialize_limited_json<T: Serialize>(
    value: &T,
    max_bytes: usize,
    message: &str,
) -> Result<String, String> {
    let serialized =
        serde_json::to_string(value).map_err(|e| format!("serialize snapshot failed: {e}"))?;
    if serialized.len() > max_bytes {
        return Err(message.to_string());
    }
    Ok(serialized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_variable_name_accepts_expected_names() {
        assert!(validate_variable_name("TOKEN"));
        assert!(validate_variable_name("org_id"));
        assert!(validate_variable_name("x-api-key"));
        assert!(!validate_variable_name(""));
        assert!(!validate_variable_name("a.b"));
        assert!(!validate_variable_name(&"a".repeat(65)));
    }

    #[test]
    fn resolve_template_reports_missing_variables() {
        let mut vars = std::collections::HashMap::new();
        vars.insert("TOKEN".to_string(), "abc".to_string());
        let err = resolve_template("Bearer {{TOKEN}} {{ORG_ID}}", &vars).expect_err("missing");
        assert!(err.contains("ORG_ID"));
    }

    #[test]
    fn resolve_template_replaces_variables_with_inner_whitespace() {
        let mut vars = std::collections::HashMap::new();
        vars.insert("TOKEN".to_string(), "abc".to_string());
        let resolved = resolve_template("Bearer {{ TOKEN }}", &vars).expect("resolve");
        assert_eq!(resolved, "Bearer abc");
    }

    #[test]
    fn build_final_url_joins_base_url_and_query_rows() {
        let query = vec![
            KeyValueRow {
                enabled: true,
                key: "page".into(),
                value: "1".into(),
            },
            KeyValueRow {
                enabled: false,
                key: "skip".into(),
                value: "x".into(),
            },
        ];
        let out = build_final_url("http://127.0.0.1:8080/", "api/users", &query).expect("url");
        assert_eq!(out, "http://127.0.0.1:8080/api/users?page=1");
    }

    #[test]
    fn build_final_url_rejects_relative_url_without_base() {
        let err = build_final_url("", "/api/users", &[]).expect_err("base required");
        assert!(err.contains("BASE_URL"));
    }

    #[test]
    fn prepare_request_body_validates_json_and_form_encoding() {
        let json_body = prepare_request_body("json", "{\"ok\":true}", &[], &[]).expect("json body");
        assert_eq!(json_body.content_type.as_deref(), Some("application/json"));
        assert_eq!(
            String::from_utf8(json_body.body.unwrap()).unwrap(),
            "{\"ok\":true}"
        );

        let form = vec![
            KeyValueRow {
                enabled: true,
                key: "a b".into(),
                value: "1+2".into(),
            },
            KeyValueRow {
                enabled: false,
                key: "skip".into(),
                value: "x".into(),
            },
        ];
        let form_body = prepare_request_body("form-urlencoded", "", &form, &[]).expect("form body");
        assert_eq!(
            form_body.content_type.as_deref(),
            Some("application/x-www-form-urlencoded")
        );
        assert_eq!(
            String::from_utf8(form_body.body.unwrap()).unwrap(),
            "a%20b=1%2B2"
        );

        let err = prepare_request_body("json", "{", &[], &[]).expect_err("bad json");
        assert!(err.contains("JSON"));
    }
}

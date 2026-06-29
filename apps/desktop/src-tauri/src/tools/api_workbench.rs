use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::time::{Duration, Instant};

pub(crate) const API_WORKBENCH_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS api_workbench_collections (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  active_environment_id INTEGER,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY(active_environment_id) REFERENCES api_workbench_environments(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_api_workbench_collections_sort
  ON api_workbench_collections(sort_order, id);

CREATE TABLE IF NOT EXISTS api_workbench_folders (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  collection_id INTEGER NOT NULL,
  parent_id INTEGER,
  name TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY(collection_id) REFERENCES api_workbench_collections(id) ON DELETE CASCADE,
  FOREIGN KEY(parent_id) REFERENCES api_workbench_folders(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_api_workbench_folders_collection
  ON api_workbench_folders(collection_id, parent_id, sort_order);

CREATE TABLE IF NOT EXISTS api_workbench_requests (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  collection_id INTEGER NOT NULL,
  folder_id INTEGER,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  method TEXT NOT NULL DEFAULT 'GET',
  url TEXT NOT NULL DEFAULT '',
  query_json TEXT NOT NULL DEFAULT '[]',
  headers_json TEXT NOT NULL DEFAULT '[]',
  body_type TEXT NOT NULL DEFAULT 'none',
  body_text TEXT NOT NULL DEFAULT '',
  form_json TEXT NOT NULL DEFAULT '[]',
  timeout_ms INTEGER NOT NULL DEFAULT 10000,
  example_response_json TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY(collection_id) REFERENCES api_workbench_collections(id) ON DELETE CASCADE,
  FOREIGN KEY(folder_id) REFERENCES api_workbench_folders(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_api_workbench_requests_collection
  ON api_workbench_requests(collection_id, folder_id, sort_order);

CREATE TABLE IF NOT EXISTS api_workbench_environments (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  collection_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(collection_id, name),
  FOREIGN KEY(collection_id) REFERENCES api_workbench_collections(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS api_workbench_environment_variables (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  environment_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  value TEXT NOT NULL DEFAULT '',
  is_secret INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(environment_id, name),
  FOREIGN KEY(environment_id) REFERENCES api_workbench_environments(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_api_workbench_env_vars_environment
  ON api_workbench_environment_variables(environment_id, sort_order);

CREATE TABLE IF NOT EXISTS api_workbench_global_variables (
  name TEXT PRIMARY KEY,
  value TEXT NOT NULL DEFAULT '',
  is_secret INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS api_workbench_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  collection_id INTEGER,
  environment_id INTEGER,
  request_id INTEGER,
  name TEXT NOT NULL DEFAULT '',
  method TEXT NOT NULL,
  url TEXT NOT NULL,
  final_url TEXT NOT NULL,
  status INTEGER,
  duration_ms INTEGER NOT NULL,
  ok INTEGER NOT NULL,
  error TEXT,
  response_content_type TEXT NOT NULL DEFAULT '',
  response_size INTEGER NOT NULL DEFAULT 0,
  response_body_preview TEXT NOT NULL DEFAULT '',
  response_body_truncated INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY(collection_id) REFERENCES api_workbench_collections(id) ON DELETE SET NULL,
  FOREIGN KEY(environment_id) REFERENCES api_workbench_environments(id) ON DELETE SET NULL,
  FOREIGN KEY(request_id) REFERENCES api_workbench_requests(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_api_workbench_history_created
  ON api_workbench_history(created_at DESC);
"#;

const MAX_TIMEOUT_MS: u64 = 120_000;
const MIN_TIMEOUT_MS: u64 = 100;
const DEFAULT_TIMEOUT_MS: u64 = 10_000;
const MAX_RESPONSE_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_HISTORY_BODY_PREVIEW_BYTES: usize = 64 * 1024;
const MAX_HISTORY_ROWS: i64 = 200;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct KeyValueRow {
    enabled: bool,
    key: String,
    value: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestDraft {
    method: String,
    url: String,
    query: Vec<KeyValueRow>,
    headers: Vec<KeyValueRow>,
    body_type: String,
    body: String,
    form: Vec<KeyValueRow>,
    timeout_ms: u64,
}

#[derive(Debug, Clone)]
struct PreparedBody {
    body: Option<Vec<u8>>,
    content_type: Option<String>,
}

fn validate_variable_name(name: &str) -> bool {
    let len = name.chars().count();
    if len == 0 || len > 64 {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn extract_variable_names(input: &str) -> Vec<String> {
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

fn resolve_template(input: &str, vars: &HashMap<String, String>) -> Result<String, String> {
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
    let mut output = input.to_string();
    for name in names {
        if let Some(value) = vars.get(&name) {
            output = output.replace(&format!("{{{{{name}}}}}"), value);
        }
    }
    Ok(output)
}

fn is_absolute_http_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn append_query_rows(mut final_url: String, query: &[KeyValueRow]) -> String {
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

fn build_final_url(base_url: &str, raw_url: &str, query: &[KeyValueRow]) -> Result<String, String> {
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
        format!("{}/{}", base.trim_end_matches('/'), url.trim_start_matches('/'))
    };
    if !is_absolute_http_url(&combined) {
        return Err("只支持 http 和 https 协议".to_string());
    }
    Ok(append_query_rows(combined, query))
}

fn has_header(headers: &[KeyValueRow], name: &str) -> bool {
    headers
        .iter()
        .any(|row| row.enabled && row.key.eq_ignore_ascii_case(name))
}

fn prepare_request_body(
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

fn clamp_timeout_ms(value: u64) -> u64 {
    value.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS)
}

pub fn execute(action: &str, _payload: &Value) -> Result<Value, String> {
    match action {
        "list" => Ok(json!({ "collections": [], "history": [] })),
        _ => Err(format!("unsupported api_workbench action: {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::json;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open memory db");
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .expect("foreign keys");
        conn.execute_batch(API_WORKBENCH_SCHEMA_SQL)
            .expect("schema");
        conn
    }

    #[test]
    fn api_workbench_schema_creates_core_tables() {
        let conn = test_conn();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN (
                    'api_workbench_collections',
                    'api_workbench_folders',
                    'api_workbench_requests',
                    'api_workbench_environments',
                    'api_workbench_environment_variables',
                    'api_workbench_global_variables',
                    'api_workbench_history'
                )",
                [],
                |row| row.get(0),
            )
            .expect("table count");
        assert_eq!(count, 7);
    }

    #[test]
    fn api_workbench_execute_rejects_unknown_action() {
        let err = execute("missing_action", &json!({})).expect_err("unknown action");
        assert!(err.contains("unsupported api_workbench action"));
    }

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
        let json_body =
            prepare_request_body("json", "{\"ok\":true}", &[], &[]).expect("json body");
        assert_eq!(
            json_body.content_type.as_deref(),
            Some("application/json")
        );
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
        let form_body =
            prepare_request_body("form-urlencoded", "", &form, &[]).expect("form body");
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

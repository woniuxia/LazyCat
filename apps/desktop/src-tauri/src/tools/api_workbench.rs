use super::helpers::db_conn;
use rusqlite::{params, Connection};
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

fn parse_i64(payload: &Value, key: &str) -> Result<i64, String> {
    payload[key]
        .as_i64()
        .ok_or_else(|| format!("{key} must be an integer"))
}

fn parse_name(payload: &Value, key: &str) -> Result<String, String> {
    let value = payload[key].as_str().unwrap_or_default().trim().to_string();
    if value.is_empty() {
        return Err(format!("{key} 不能为空"));
    }
    Ok(value)
}

fn collection_create_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let name = parse_name(payload, "name")?;
    let description = payload["description"].as_str().unwrap_or_default().trim();
    let next_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM api_workbench_collections",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO api_workbench_collections(name, description, sort_order)
         VALUES(?1, ?2, ?3)",
        params![name, description, next_order],
    )
    .map_err(|e| format!("create api collection failed: {e}"))?;
    let collection_id = conn.last_insert_rowid();
    let env = environment_save_with_conn(
        conn,
        &json!({
            "collectionId": collection_id,
            "name": "开发",
            "variables": [{ "name": "BASE_URL", "value": "", "isSecret": false }]
        }),
    )?;
    let active_environment_id = env["id"].as_i64().ok_or("environment id missing")?;
    conn.execute(
        "UPDATE api_workbench_collections
         SET active_environment_id=?1, updated_at=CURRENT_TIMESTAMP
         WHERE id=?2",
        params![active_environment_id, collection_id],
    )
    .map_err(|e| format!("set active environment failed: {e}"))?;
    Ok(json!({
        "id": collection_id,
        "name": name,
        "description": description,
        "activeEnvironmentId": active_environment_id,
        "sortOrder": next_order
    }))
}

fn collection_set_active_environment_with_conn(
    conn: &Connection,
    payload: &Value,
) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let environment_id = parse_i64(payload, "environmentId")?;
    let owner: i64 = conn
        .query_row(
            "SELECT collection_id FROM api_workbench_environments WHERE id=?1",
            [environment_id],
            |row| row.get(0),
        )
        .map_err(|_| "环境不存在".to_string())?;
    if owner != collection_id {
        return Err("环境不属于当前集合".to_string());
    }
    let affected = conn
        .execute(
            "UPDATE api_workbench_collections
             SET active_environment_id=?1, updated_at=CURRENT_TIMESTAMP
             WHERE id=?2",
            params![environment_id, collection_id],
        )
        .map_err(|e| format!("set active environment failed: {e}"))?;
    if affected == 0 {
        return Err("集合不存在".to_string());
    }
    Ok(json!({ "ok": true, "activeEnvironmentId": environment_id }))
}

fn parse_variable_rows(payload: &Value) -> Result<Vec<KeyValueRow>, String> {
    let rows = payload["variables"].as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();
    for item in rows {
        let name = item["name"].as_str().unwrap_or_default().trim();
        if !validate_variable_name(name) {
            return Err(format!("变量名无效: {name}"));
        }
        out.push(KeyValueRow {
            enabled: true,
            key: name.to_string(),
            value: item["value"].as_str().unwrap_or_default().to_string(),
        });
    }
    Ok(out)
}

fn environment_save_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let name = parse_name(payload, "name")?;
    let id = payload["id"].as_i64();
    let env_id = if let Some(id) = id {
        let affected = conn
            .execute(
                "UPDATE api_workbench_environments
                 SET name=?1, updated_at=CURRENT_TIMESTAMP
                 WHERE id=?2 AND collection_id=?3",
                params![name, id, collection_id],
            )
            .map_err(|e| format!("update environment failed: {e}"))?;
        if affected == 0 {
            return Err("环境不存在".to_string());
        }
        id
    } else {
        let next_order: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1
                 FROM api_workbench_environments WHERE collection_id=?1",
                [collection_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO api_workbench_environments(collection_id, name, sort_order)
             VALUES(?1, ?2, ?3)",
            params![collection_id, name, next_order],
        )
        .map_err(|e| format!("create environment failed: {e}"))?;
        conn.last_insert_rowid()
    };

    let mut rows = parse_variable_rows(payload)?;
    if !rows.iter().any(|row| row.key == "BASE_URL") {
        rows.insert(
            0,
            KeyValueRow {
                enabled: true,
                key: "BASE_URL".into(),
                value: "".into(),
            },
        );
    }
    conn.execute(
        "DELETE FROM api_workbench_environment_variables WHERE environment_id=?1",
        [env_id],
    )
    .map_err(|e| format!("replace environment variables failed: {e}"))?;
    for (idx, row) in rows.iter().enumerate() {
        conn.execute(
            "INSERT INTO api_workbench_environment_variables(environment_id, name, value, is_secret, sort_order)
             VALUES(?1, ?2, ?3, 0, ?4)",
            params![env_id, row.key, row.value, idx as i64],
        )
        .map_err(|e| format!("save environment variable failed: {e}"))?;
    }
    Ok(json!({ "id": env_id, "collectionId": collection_id, "name": name }))
}

fn environment_delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    let collection_id: i64 = conn
        .query_row(
            "SELECT collection_id FROM api_workbench_environments WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .map_err(|_| "环境不存在".to_string())?;
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM api_workbench_environments WHERE collection_id=?1",
            [collection_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("count environments failed: {e}"))?;
    if count <= 1 {
        return Err("不能删除集合内最后一个环境".to_string());
    }
    conn.execute("DELETE FROM api_workbench_environments WHERE id=?1", [id])
        .map_err(|e| format!("delete environment failed: {e}"))?;
    let next_active: i64 = conn
        .query_row(
            "SELECT id FROM api_workbench_environments
             WHERE collection_id=?1 ORDER BY sort_order ASC, id ASC LIMIT 1",
            [collection_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("pick active environment failed: {e}"))?;
    conn.execute(
        "UPDATE api_workbench_collections
         SET active_environment_id=?1, updated_at=CURRENT_TIMESTAMP
         WHERE id=?2 AND (active_environment_id IS NULL OR active_environment_id=?3)",
        params![next_active, collection_id, id],
    )
    .map_err(|e| format!("switch active environment failed: {e}"))?;
    Ok(json!({ "ok": true, "activeEnvironmentId": next_active }))
}

fn global_variables_save_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let rows = parse_variable_rows(payload)?;
    if rows.iter().any(|row| row.key == "BASE_URL") {
        return Err("全局变量不能使用 BASE_URL".to_string());
    }
    conn.execute("DELETE FROM api_workbench_global_variables", [])
        .map_err(|e| format!("clear global variables failed: {e}"))?;
    for (idx, row) in rows.iter().enumerate() {
        conn.execute(
            "INSERT INTO api_workbench_global_variables(name, value, is_secret, sort_order)
             VALUES(?1, ?2, 0, ?3)",
            params![row.key, row.value, idx as i64],
        )
        .map_err(|e| format!("save global variable failed: {e}"))?;
    }
    Ok(json!({ "ok": true }))
}

fn folder_create_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let name = parse_name(payload, "name")?;
    let parent_id = payload["parentId"].as_i64();
    let next_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1
             FROM api_workbench_folders
             WHERE collection_id=?1 AND parent_id IS ?2",
            params![collection_id, parent_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO api_workbench_folders(collection_id, parent_id, name, sort_order)
         VALUES(?1, ?2, ?3, ?4)",
        params![collection_id, parent_id, name, next_order],
    )
    .map_err(|e| format!("create folder failed: {e}"))?;
    Ok(json!({
        "id": conn.last_insert_rowid(),
        "collectionId": collection_id,
        "parentId": parent_id,
        "name": name
    }))
}

fn folder_update_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    let name = parse_name(payload, "name")?;
    let affected = conn
        .execute(
            "UPDATE api_workbench_folders SET name=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![name, id],
        )
        .map_err(|e| format!("update folder failed: {e}"))?;
    if affected == 0 {
        return Err("文件夹不存在".to_string());
    }
    Ok(json!({ "ok": true }))
}

fn folder_delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    conn.execute("DELETE FROM api_workbench_folders WHERE id=?1", [id])
        .map_err(|e| format!("delete folder failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn parse_draft(payload: &Value) -> Result<RequestDraft, String> {
    serde_json::from_value(payload["draft"].clone()).map_err(|e| format!("请求草稿格式错误: {e}"))
}

fn request_save_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let folder_id = payload["folderId"].as_i64();
    let name = parse_name(payload, "name")?;
    let description = payload["description"].as_str().unwrap_or_default().trim();
    let draft = parse_draft(payload)?;
    let query_json = serde_json::to_string(&draft.query).map_err(|e| e.to_string())?;
    let headers_json = serde_json::to_string(&draft.headers).map_err(|e| e.to_string())?;
    let form_json = serde_json::to_string(&draft.form).map_err(|e| e.to_string())?;
    let id = payload["id"].as_i64();
    if let Some(id) = id {
        let affected = conn
            .execute(
                "UPDATE api_workbench_requests
                 SET folder_id=?1, name=?2, description=?3, method=?4, url=?5,
                     query_json=?6, headers_json=?7, body_type=?8, body_text=?9,
                     form_json=?10, timeout_ms=?11, updated_at=CURRENT_TIMESTAMP
                 WHERE id=?12 AND collection_id=?13",
                params![
                    folder_id,
                    name,
                    description,
                    draft.method,
                    draft.url,
                    query_json,
                    headers_json,
                    draft.body_type,
                    draft.body,
                    form_json,
                    clamp_timeout_ms(draft.timeout_ms) as i64,
                    id,
                    collection_id
                ],
            )
            .map_err(|e| format!("update request failed: {e}"))?;
        if affected == 0 {
            return Err("接口不存在".to_string());
        }
        Ok(json!({ "id": id, "ok": true }))
    } else {
        let next_order: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1
                 FROM api_workbench_requests
                 WHERE collection_id=?1 AND folder_id IS ?2",
                params![collection_id, folder_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO api_workbench_requests(
                collection_id, folder_id, name, description, method, url,
                query_json, headers_json, body_type, body_text, form_json,
                timeout_ms, sort_order
             )
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                collection_id,
                folder_id,
                name,
                description,
                draft.method,
                draft.url,
                query_json,
                headers_json,
                draft.body_type,
                draft.body,
                form_json,
                clamp_timeout_ms(draft.timeout_ms) as i64,
                next_order
            ],
        )
        .map_err(|e| format!("create request failed: {e}"))?;
        Ok(json!({ "id": conn.last_insert_rowid(), "ok": true }))
    }
}

fn request_get_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    conn.query_row(
        "SELECT id, collection_id, folder_id, name, description, method, url,
                query_json, headers_json, body_type, body_text, form_json, timeout_ms,
                example_response_json, sort_order, created_at, updated_at
         FROM api_workbench_requests WHERE id=?1",
        [id],
        |row| {
            let query_json: String = row.get(7)?;
            let headers_json: String = row.get(8)?;
            let form_json: String = row.get(11)?;
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "collectionId": row.get::<_, i64>(1)?,
                "folderId": row.get::<_, Option<i64>>(2)?,
                "name": row.get::<_, String>(3)?,
                "description": row.get::<_, String>(4)?,
                "draft": {
                    "method": row.get::<_, String>(5)?,
                    "url": row.get::<_, String>(6)?,
                    "query": serde_json::from_str::<Value>(&query_json).unwrap_or_else(|_| json!([])),
                    "headers": serde_json::from_str::<Value>(&headers_json).unwrap_or_else(|_| json!([])),
                    "bodyType": row.get::<_, String>(9)?,
                    "body": row.get::<_, String>(10)?,
                    "form": serde_json::from_str::<Value>(&form_json).unwrap_or_else(|_| json!([])),
                    "timeoutMs": row.get::<_, i64>(12)?
                },
                "exampleResponse": row.get::<_, Option<String>>(13)?,
                "sortOrder": row.get::<_, i64>(14)?,
                "createdAt": row.get::<_, String>(15)?,
                "updatedAt": row.get::<_, String>(16)?
            }))
        },
    )
    .map_err(|_| "接口不存在".to_string())
}

fn request_delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    conn.execute("DELETE FROM api_workbench_requests WHERE id=?1", [id])
        .map_err(|e| format!("delete request failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn collection_update_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    let name = parse_name(payload, "name")?;
    let description = payload["description"].as_str().unwrap_or_default().trim();
    let affected = conn
        .execute(
            "UPDATE api_workbench_collections
             SET name=?1, description=?2, updated_at=CURRENT_TIMESTAMP WHERE id=?3",
            params![name, description, id],
        )
        .map_err(|e| format!("update collection failed: {e}"))?;
    if affected == 0 {
        return Err("集合不存在".to_string());
    }
    Ok(json!({ "ok": true }))
}

fn collection_delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    conn.execute("DELETE FROM api_workbench_collections WHERE id=?1", [id])
        .map_err(|e| format!("delete collection failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn environment_list_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let mut stmt = conn
        .prepare(
            "SELECT id, collection_id, name, sort_order, created_at, updated_at
             FROM api_workbench_environments
             WHERE collection_id=?1 ORDER BY sort_order ASC, id ASC",
        )
        .map_err(|e| format!("prepare environment list failed: {e}"))?;
    let items = stmt
        .query_map([collection_id], |row| {
            let env_id = row.get::<_, i64>(0)?;
            let mut var_stmt = conn.prepare(
                "SELECT name, value, is_secret, sort_order
                 FROM api_workbench_environment_variables
                 WHERE environment_id=?1 ORDER BY sort_order ASC, id ASC",
            )?;
            let variables = var_stmt
                .query_map([env_id], |var_row| {
                    Ok(json!({
                        "name": var_row.get::<_, String>(0)?,
                        "value": var_row.get::<_, String>(1)?,
                        "isSecret": var_row.get::<_, i64>(2)? != 0,
                        "sortOrder": var_row.get::<_, i64>(3)?
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(json!({
                "id": env_id,
                "collectionId": row.get::<_, i64>(1)?,
                "name": row.get::<_, String>(2)?,
                "sortOrder": row.get::<_, i64>(3)?,
                "createdAt": row.get::<_, String>(4)?,
                "updatedAt": row.get::<_, String>(5)?,
                "variables": variables
            }))
        })
        .map_err(|e| format!("list environments failed: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read environments failed: {e}"))?;
    Ok(json!({ "items": items }))
}

fn global_variables_list_with_conn(conn: &Connection) -> Result<Value, String> {
    let mut stmt = conn
        .prepare(
            "SELECT name, value, is_secret, sort_order
             FROM api_workbench_global_variables ORDER BY sort_order ASC, name ASC",
        )
        .map_err(|e| format!("prepare global variables failed: {e}"))?;
    let items = stmt
        .query_map([], |row| {
            Ok(json!({
                "name": row.get::<_, String>(0)?,
                "value": row.get::<_, String>(1)?,
                "isSecret": row.get::<_, i64>(2)? != 0,
                "sortOrder": row.get::<_, i64>(3)?
            }))
        })
        .map_err(|e| format!("list global variables failed: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read global variables failed: {e}"))?;
    Ok(json!({ "items": items }))
}

fn action_list_with_conn(conn: &Connection) -> Result<Value, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, description, active_environment_id, sort_order, created_at, updated_at
             FROM api_workbench_collections ORDER BY sort_order ASC, id ASC",
        )
        .map_err(|e| format!("prepare collection list failed: {e}"))?;
    let collections = stmt
        .query_map([], |row| {
            let collection_id = row.get::<_, i64>(0)?;

            let mut folder_stmt = conn.prepare(
                "SELECT id, collection_id, parent_id, name, sort_order
                 FROM api_workbench_folders
                 WHERE collection_id=?1 ORDER BY parent_id ASC, sort_order ASC, id ASC",
            )?;
            let folders = folder_stmt
                .query_map([collection_id], |folder_row| {
                    Ok(json!({
                        "id": folder_row.get::<_, i64>(0)?,
                        "collectionId": folder_row.get::<_, i64>(1)?,
                        "parentId": folder_row.get::<_, Option<i64>>(2)?,
                        "name": folder_row.get::<_, String>(3)?,
                        "sortOrder": folder_row.get::<_, i64>(4)?
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            let mut request_stmt = conn.prepare(
                "SELECT id, collection_id, folder_id, name, method, url, sort_order
                 FROM api_workbench_requests
                 WHERE collection_id=?1 ORDER BY folder_id ASC, sort_order ASC, id ASC",
            )?;
            let requests = request_stmt
                .query_map([collection_id], |request_row| {
                    Ok(json!({
                        "id": request_row.get::<_, i64>(0)?,
                        "collectionId": request_row.get::<_, i64>(1)?,
                        "folderId": request_row.get::<_, Option<i64>>(2)?,
                        "name": request_row.get::<_, String>(3)?,
                        "method": request_row.get::<_, String>(4)?,
                        "url": request_row.get::<_, String>(5)?,
                        "sortOrder": request_row.get::<_, i64>(6)?
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            Ok(json!({
                "id": collection_id,
                "name": row.get::<_, String>(1)?,
                "description": row.get::<_, String>(2)?,
                "activeEnvironmentId": row.get::<_, Option<i64>>(3)?,
                "sortOrder": row.get::<_, i64>(4)?,
                "createdAt": row.get::<_, String>(5)?,
                "updatedAt": row.get::<_, String>(6)?,
                "folders": folders,
                "requests": requests
            }))
        })
        .map_err(|e| format!("list collections failed: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read collections failed: {e}"))?;
    Ok(json!({ "collections": collections, "history": [] }))
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    match action {
        "list" => action_list_with_conn(&conn),
        "collection_create" => collection_create_with_conn(&conn, payload),
        "collection_update" => collection_update_with_conn(&conn, payload),
        "collection_set_active_environment" => {
            collection_set_active_environment_with_conn(&conn, payload)
        }
        "collection_delete" => collection_delete_with_conn(&conn, payload),
        "folder_create" => folder_create_with_conn(&conn, payload),
        "folder_update" => folder_update_with_conn(&conn, payload),
        "folder_delete" => folder_delete_with_conn(&conn, payload),
        "request_get" => request_get_with_conn(&conn, payload),
        "request_save" => request_save_with_conn(&conn, payload),
        "request_delete" => request_delete_with_conn(&conn, payload),
        "environment_list" => environment_list_with_conn(&conn, payload),
        "environment_save" => environment_save_with_conn(&conn, payload),
        "environment_delete" => environment_delete_with_conn(&conn, payload),
        "global_variables_list" => global_variables_list_with_conn(&conn),
        "global_variables_save" => global_variables_save_with_conn(&conn, payload),
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

    #[test]
    fn collection_create_initializes_default_environment_and_base_url() {
        let conn = test_conn();
        let result = collection_create_with_conn(
            &conn,
            &json!({ "name": "Demo", "description": "desc" }),
        )
        .expect("create");
        let collection_id = result["id"].as_i64().expect("collection id");
        let active_environment_id = result["activeEnvironmentId"].as_i64().expect("env id");
        assert!(collection_id > 0);
        assert!(active_environment_id > 0);

        let base_url_count: i64 = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM api_workbench_environment_variables
                 WHERE environment_id=?1 AND name='BASE_URL'",
                [active_environment_id],
                |row| row.get(0),
            )
            .expect("base url count");
        assert_eq!(base_url_count, 1);
    }

    #[test]
    fn collection_set_active_environment_requires_same_collection() {
        let conn = test_conn();
        let a = collection_create_with_conn(&conn, &json!({ "name": "A" })).expect("a");
        let b = collection_create_with_conn(&conn, &json!({ "name": "B" })).expect("b");
        let a_id = a["id"].as_i64().unwrap();
        let b_env_id = b["activeEnvironmentId"].as_i64().unwrap();
        let err = collection_set_active_environment_with_conn(
            &conn,
            &json!({ "collectionId": a_id, "environmentId": b_env_id }),
        )
        .expect_err("must reject");
        assert!(err.contains("不属于当前集合"));
    }

    #[test]
    fn environment_delete_switches_active_environment_and_rejects_last_one() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let first_env_id = c["activeEnvironmentId"].as_i64().unwrap();
        let second = environment_save_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "name": "Test", "variables": [] }),
        )
        .expect("second env");
        let second_env_id = second["id"].as_i64().unwrap();

        environment_delete_with_conn(&conn, &json!({ "id": first_env_id })).expect("delete first");
        let active: i64 = conn
            .query_row(
                "SELECT active_environment_id FROM api_workbench_collections WHERE id=?1",
                [collection_id],
                |row| row.get(0),
            )
            .expect("active");
        assert_eq!(active, second_env_id);

        let err = environment_delete_with_conn(&conn, &json!({ "id": second_env_id }))
            .expect_err("reject last");
        assert!(err.contains("最后一个环境"));
    }

    #[test]
    fn global_variables_reject_base_url() {
        let conn = test_conn();
        let err = global_variables_save_with_conn(
            &conn,
            &json!({ "variables": [{ "name": "BASE_URL", "value": "http://x", "isSecret": false }] }),
        )
        .expect_err("reject");
        assert!(err.contains("BASE_URL"));
    }

    #[test]
    fn request_save_and_get_round_trips_draft_json() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let saved = request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": null,
                "name": "List users",
                "description": "Fetch users",
                "draft": {
                    "method": "GET",
                    "url": "/api/users",
                    "query": [{ "enabled": true, "key": "page", "value": "1" }],
                    "headers": [],
                    "bodyType": "none",
                    "body": "",
                    "form": [],
                    "timeoutMs": 10000
                }
            }),
        )
        .expect("save");
        let request_id = saved["id"].as_i64().unwrap();
        let detail = request_get_with_conn(&conn, &json!({ "id": request_id })).expect("get");
        assert_eq!(detail["name"], "List users");
        assert_eq!(detail["draft"]["url"], "/api/users");
        assert_eq!(detail["draft"]["query"][0]["key"], "page");
    }

    #[test]
    fn action_list_returns_collections_with_folders_and_requests() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let folder = folder_create_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "name": "Users" }),
        )
        .expect("folder");
        request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": folder["id"].as_i64().unwrap(),
                "name": "List users",
                "draft": {
                    "method": "GET",
                    "url": "/api/users",
                    "query": [],
                    "headers": [],
                    "bodyType": "none",
                    "body": "",
                    "form": [],
                    "timeoutMs": 10000
                }
            }),
        )
        .expect("request");

        let list = action_list_with_conn(&conn).expect("list");
        assert_eq!(list["collections"][0]["name"], "Demo");
        assert_eq!(list["collections"][0]["folders"][0]["name"], "Users");
        assert_eq!(list["collections"][0]["requests"][0]["name"], "List users");
    }
}

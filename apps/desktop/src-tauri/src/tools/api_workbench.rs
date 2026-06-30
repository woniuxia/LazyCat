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
  request_snapshot_json TEXT,
  executed_request_snapshot_json TEXT,
  replayed_from_history_id INTEGER,
  pinned INTEGER NOT NULL DEFAULT 0,
  note TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY(collection_id) REFERENCES api_workbench_collections(id) ON DELETE SET NULL,
  FOREIGN KEY(environment_id) REFERENCES api_workbench_environments(id) ON DELETE SET NULL,
  FOREIGN KEY(request_id) REFERENCES api_workbench_requests(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_api_workbench_history_created
  ON api_workbench_history(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_api_workbench_history_pinned_created
  ON api_workbench_history(pinned, created_at DESC, id DESC);
"#;

const MAX_TIMEOUT_MS: u64 = 120_000;
const MIN_TIMEOUT_MS: u64 = 100;
const MAX_RESPONSE_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_HISTORY_BODY_PREVIEW_BYTES: usize = 64 * 1024;
const MAX_HISTORY_ROWS: i64 = 200;
const MAX_HISTORY_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;
const MAX_HISTORY_NOTE_CHARS: usize = 2000;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct KeyValueRow {
    enabled: bool,
    key: String,
    value: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecutedRequestSnapshot {
    method: String,
    final_url: String,
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

fn ensure_api_workbench_history_columns(conn: &Connection) -> Result<(), String> {
    let columns = [
        ("request_snapshot_json", "TEXT"),
        ("executed_request_snapshot_json", "TEXT"),
        ("replayed_from_history_id", "INTEGER"),
        ("pinned", "INTEGER NOT NULL DEFAULT 0"),
        ("note", "TEXT NOT NULL DEFAULT ''"),
    ];
    for (name, ty) in columns {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('api_workbench_history') WHERE name=?1",
                [name],
                |row| row.get(0),
            )
            .map_err(|e| format!("inspect api history schema failed: {e}"))?;
        if exists == 0 {
            conn.execute(
                &format!("ALTER TABLE api_workbench_history ADD COLUMN {name} {ty}"),
                [],
            )
            .map_err(|e| format!("migrate api history column {name} failed: {e}"))?;
        }
    }
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_api_workbench_history_pinned_created
         ON api_workbench_history(pinned, created_at DESC, id DESC)",
        [],
    )
    .map_err(|e| format!("create api history pinned index failed: {e}"))?;
    Ok(())
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

fn parse_ordered_ids(payload: &Value) -> Result<Vec<i64>, String> {
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
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM api_workbench_folders WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .map_err(|e| format!("check folder failed: {e}"))?;
    if exists == 0 {
        return Err("文件夹不存在".to_string());
    }

    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("delete folder begin: {e}"))?;
    tx.execute(
        "WITH RECURSIVE descendants(id) AS (
            SELECT id FROM api_workbench_folders WHERE id=?1
            UNION ALL
            SELECT f.id FROM api_workbench_folders f
            JOIN descendants d ON f.parent_id=d.id
        )
        UPDATE api_workbench_requests
        SET folder_id=NULL, updated_at=CURRENT_TIMESTAMP
        WHERE folder_id IN (SELECT id FROM descendants)",
        [id],
    )
    .map_err(|e| format!("unassign folder requests failed: {e}"))?;
    tx.execute("DELETE FROM api_workbench_folders WHERE id=?1", [id])
        .map_err(|e| format!("delete folder failed: {e}"))?;
    tx.commit()
        .map_err(|e| format!("delete folder commit: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn next_folder_sort_order(
    conn: &Connection,
    collection_id: i64,
    parent_id: Option<i64>,
) -> Result<i64, String> {
    conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1
         FROM api_workbench_folders
         WHERE collection_id=?1 AND parent_id IS ?2",
        params![collection_id, parent_id],
        |row| row.get(0),
    )
    .map_err(|e| format!("query next folder order failed: {e}"))
}

fn next_request_sort_order(
    conn: &Connection,
    collection_id: i64,
    folder_id: Option<i64>,
) -> Result<i64, String> {
    conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1
         FROM api_workbench_requests
         WHERE collection_id=?1 AND folder_id IS ?2",
        params![collection_id, folder_id],
        |row| row.get(0),
    )
    .map_err(|e| format!("query next request order failed: {e}"))
}

fn folder_is_descendant(
    conn: &Connection,
    folder_id: i64,
    possible_descendant_id: i64,
) -> Result<bool, String> {
    let count: i64 = conn
        .query_row(
            "WITH RECURSIVE descendants(id) AS (
                SELECT id FROM api_workbench_folders WHERE parent_id=?1
                UNION ALL
                SELECT f.id FROM api_workbench_folders f
                JOIN descendants d ON f.parent_id=d.id
            )
            SELECT COUNT(*) FROM descendants WHERE id=?2",
            params![folder_id, possible_descendant_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("check descendants failed: {e}"))?;
    Ok(count > 0)
}

fn folder_move_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    let target_parent_id = payload["targetParentId"].as_i64();
    if target_parent_id == Some(id) {
        return Err("不能移动到自己".to_string());
    }

    let collection_id: i64 = conn
        .query_row(
            "SELECT collection_id FROM api_workbench_folders WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .map_err(|_| "文件夹不存在".to_string())?;
    if let Some(parent_id) = target_parent_id {
        let owner: i64 = conn
            .query_row(
                "SELECT collection_id FROM api_workbench_folders WHERE id=?1",
                [parent_id],
                |row| row.get(0),
            )
            .map_err(|_| "目标文件夹不存在".to_string())?;
        if owner != collection_id {
            return Err("目标文件夹不属于当前集合".to_string());
        }
        if folder_is_descendant(conn, id, parent_id)? {
            return Err("不能移动到自己的子文件夹".to_string());
        }
    }

    let next_order = next_folder_sort_order(conn, collection_id, target_parent_id)?;
    conn.execute(
        "UPDATE api_workbench_folders
         SET parent_id=?1, sort_order=?2, updated_at=CURRENT_TIMESTAMP
         WHERE id=?3",
        params![target_parent_id, next_order, id],
    )
    .map_err(|e| format!("move folder failed: {e}"))?;
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

fn request_move_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    let target_folder_id = payload["targetFolderId"].as_i64();
    let collection_id: i64 = conn
        .query_row(
            "SELECT collection_id FROM api_workbench_requests WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .map_err(|_| "接口不存在".to_string())?;
    if let Some(folder_id) = target_folder_id {
        let owner: i64 = conn
            .query_row(
                "SELECT collection_id FROM api_workbench_folders WHERE id=?1",
                [folder_id],
                |row| row.get(0),
            )
            .map_err(|_| "目标文件夹不存在".to_string())?;
        if owner != collection_id {
            return Err("目标文件夹不属于当前集合".to_string());
        }
    }

    let next_order = next_request_sort_order(conn, collection_id, target_folder_id)?;
    conn.execute(
        "UPDATE api_workbench_requests
         SET folder_id=?1, sort_order=?2, updated_at=CURRENT_TIMESTAMP
         WHERE id=?3",
        params![target_folder_id, next_order, id],
    )
    .map_err(|e| format!("move request failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn folder_reorder_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let parent_id = payload["parentId"].as_i64();
    let ordered_ids = parse_ordered_ids(payload)?;
    let existing: Vec<i64> = {
        let mut stmt = conn
            .prepare(
                "SELECT id FROM api_workbench_folders
                 WHERE collection_id=?1 AND parent_id IS ?2
                 ORDER BY sort_order ASC, id ASC",
            )
            .map_err(|e| format!("prepare folder reorder failed: {e}"))?;
        let rows = stmt
            .query_map(params![collection_id, parent_id], |row| row.get(0))
            .map_err(|e| format!("query folder reorder failed: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("collect folder reorder failed: {e}"))?;
        rows
    };
    let expected: HashSet<i64> = existing.iter().copied().collect();
    let actual: HashSet<i64> = ordered_ids.iter().copied().collect();
    if expected != actual || existing.len() != ordered_ids.len() {
        return Err("排序列表不完整".to_string());
    }

    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("folder reorder begin: {e}"))?;
    for (idx, id) in ordered_ids.iter().enumerate() {
        tx.execute(
            "UPDATE api_workbench_folders SET sort_order=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![idx as i64, id],
        )
        .map_err(|e| format!("update folder order failed: {e}"))?;
    }
    tx.commit()
        .map_err(|e| format!("folder reorder commit: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn request_reorder_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let folder_id = payload["folderId"].as_i64();
    let ordered_ids = parse_ordered_ids(payload)?;
    let existing: Vec<i64> = {
        let mut stmt = conn
            .prepare(
                "SELECT id FROM api_workbench_requests
                 WHERE collection_id=?1 AND folder_id IS ?2
                 ORDER BY sort_order ASC, id ASC",
            )
            .map_err(|e| format!("prepare request reorder failed: {e}"))?;
        let rows = stmt
            .query_map(params![collection_id, folder_id], |row| row.get(0))
            .map_err(|e| format!("query request reorder failed: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("collect request reorder failed: {e}"))?;
        rows
    };
    let expected: HashSet<i64> = existing.iter().copied().collect();
    let actual: HashSet<i64> = ordered_ids.iter().copied().collect();
    if expected != actual || existing.len() != ordered_ids.len() {
        return Err("排序列表不完整".to_string());
    }

    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("request reorder begin: {e}"))?;
    for (idx, id) in ordered_ids.iter().enumerate() {
        tx.execute(
            "UPDATE api_workbench_requests SET sort_order=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![idx as i64, id],
        )
        .map_err(|e| format!("update request order failed: {e}"))?;
    }
    tx.commit()
        .map_err(|e| format!("request reorder commit: {e}"))?;
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
    let history = history_list_with_conn(conn)?["items"].clone();
    Ok(json!({ "collections": collections, "history": history }))
}

fn load_variables(
    conn: &Connection,
    environment_id: i64,
) -> Result<(HashMap<String, String>, String), String> {
    let mut vars = HashMap::new();
    let mut stmt = conn
        .prepare("SELECT name, value FROM api_workbench_global_variables ORDER BY sort_order ASC")
        .map_err(|e| format!("prepare global variables failed: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("query global variables failed: {e}"))?;
    for row in rows {
        let (name, value) = row.map_err(|e| e.to_string())?;
        vars.insert(name, value);
    }

    let mut base_url = String::new();
    let mut env_stmt = conn
        .prepare(
            "SELECT name, value FROM api_workbench_environment_variables
             WHERE environment_id=?1 ORDER BY sort_order ASC",
        )
        .map_err(|e| format!("prepare environment variables failed: {e}"))?;
    let env_rows = env_stmt
        .query_map([environment_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("query environment variables failed: {e}"))?;
    for row in env_rows {
        let (name, value) = row.map_err(|e| e.to_string())?;
        if name == "BASE_URL" {
            base_url = value.clone();
        }
        vars.insert(name, value);
    }
    Ok((vars, base_url))
}

fn resolve_rows(
    rows: &[KeyValueRow],
    vars: &HashMap<String, String>,
) -> Result<Vec<KeyValueRow>, String> {
    let mut out = Vec::new();
    for row in rows {
        if !row.enabled {
            continue;
        }
        out.push(KeyValueRow {
            enabled: true,
            key: resolve_template(&row.key, vars)?,
            value: resolve_template(&row.value, vars)?,
        });
    }
    Ok(out)
}

fn prepare_api_workbench_request(
    draft: &RequestDraft,
    vars: &HashMap<String, String>,
    base_url: &str,
) -> Result<ExecutedRequestSnapshot, String> {
    let resolved_url = resolve_template(&draft.url, vars)?;
    let resolved_query = resolve_rows(&draft.query, vars)?;
    let mut resolved_headers = resolve_rows(&draft.headers, vars)?;
    let resolved_body = if matches!(draft.body_type.as_str(), "json" | "text") {
        resolve_template(&draft.body, vars)?
    } else {
        String::new()
    };
    let resolved_form = if draft.body_type == "form-urlencoded" {
        resolve_rows(&draft.form, vars)?
    } else {
        Vec::new()
    };
    let final_url = build_final_url(base_url, &resolved_url, &resolved_query)?;
    let prepared = prepare_request_body(
        &draft.body_type,
        &resolved_body,
        &resolved_form,
        &resolved_headers,
    )?;
    if let Some(content_type) = prepared.content_type {
        resolved_headers.push(KeyValueRow {
            enabled: true,
            key: "Content-Type".to_string(),
            value: content_type,
        });
    }
    Ok(ExecutedRequestSnapshot {
        method: draft.method.clone(),
        final_url,
        headers: resolved_headers,
        body_type: draft.body_type.clone(),
        body: resolved_body,
        form: resolved_form,
        timeout_ms: clamp_timeout_ms(draft.timeout_ms),
    })
}

fn execute_api_workbench_request(snapshot: &ExecutedRequestSnapshot) -> Result<Value, String> {
    let prepared = prepare_request_body(
        &snapshot.body_type,
        &snapshot.body,
        &snapshot.form,
        &snapshot.headers,
    )?;
    let draft_for_timeout = RequestDraft {
        method: snapshot.method.clone(),
        url: snapshot.final_url.clone(),
        query: Vec::new(),
        headers: snapshot.headers.clone(),
        body_type: snapshot.body_type.clone(),
        body: snapshot.body.clone(),
        form: snapshot.form.clone(),
        timeout_ms: snapshot.timeout_ms,
    };
    execute_http_request(
        &draft_for_timeout,
        &snapshot.final_url,
        &snapshot.headers,
        prepared,
    )
}

fn serialize_limited_json<T: Serialize>(
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

fn execute_http_request(
    draft: &RequestDraft,
    final_url: &str,
    headers: &[KeyValueRow],
    prepared: PreparedBody,
) -> Result<Value, String> {
    let started = Instant::now();
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_millis(clamp_timeout_ms(draft.timeout_ms)))
        .redirects(0)
        .build();
    let method = draft.method.to_ascii_uppercase();
    let mut request = match method.as_str() {
        "GET" => agent.get(final_url),
        "POST" => agent.post(final_url),
        "PUT" => agent.put(final_url),
        "PATCH" => agent.request("PATCH", final_url),
        "DELETE" => agent.delete(final_url),
        "HEAD" => agent.head(final_url),
        "OPTIONS" => agent.request("OPTIONS", final_url),
        _ => return Err(format!("unsupported method: {method}")),
    };
    let mut request_headers = headers.to_vec();
    for row in headers {
        if row.enabled && !row.key.trim().is_empty() {
            request = request.set(row.key.trim(), row.value.as_str());
        }
    }
    if let Some(content_type) = prepared.content_type.as_deref() {
        request = request.set("Content-Type", content_type);
        request_headers.push(KeyValueRow {
            enabled: true,
            key: "Content-Type".to_string(),
            value: content_type.to_string(),
        });
    }

    let result = if let Some(body) = prepared.body {
        request.send_bytes(&body)
    } else {
        request.call()
    };
    let duration_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(resp) => response_to_json(final_url, duration_ms, resp, None, &request_headers),
        Err(ureq::Error::Status(_, resp)) => {
            response_to_json(final_url, duration_ms, resp, None, &request_headers)
        }
        Err(err) => Ok(json!({
            "finalUrl": final_url,
            "status": null,
            "statusText": "",
            "ok": false,
            "durationMs": duration_ms,
            "requestHeaders": request_headers,
            "responseHeaders": [],
            "bodyText": "",
            "bodySize": 0,
            "bodyTruncated": false,
            "contentType": "",
            "error": err.to_string()
        })),
    }
}

fn response_to_json(
    final_url: &str,
    duration_ms: u64,
    resp: ureq::Response,
    forced_error: Option<String>,
    request_headers: &[KeyValueRow],
) -> Result<Value, String> {
    let status = resp.status();
    let status_text = resp.status_text().to_string();
    let content_type = resp.header("Content-Type").unwrap_or("").to_string();
    let response_headers: Vec<Value> = resp
        .headers_names()
        .into_iter()
        .map(|key| {
            let value = resp.header(&key).unwrap_or("").to_string();
            json!({ "enabled": true, "key": key, "value": value })
        })
        .collect();
    let mut reader = resp.into_reader().take((MAX_RESPONSE_BODY_BYTES + 1) as u64);
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read response body failed: {e}"))?;
    let body_truncated = bytes.len() > MAX_RESPONSE_BODY_BYTES;
    if body_truncated {
        bytes.truncate(MAX_RESPONSE_BODY_BYTES);
    }
    let body_size = bytes.len();
    let body_text = String::from_utf8_lossy(&bytes).to_string();
    Ok(json!({
        "finalUrl": final_url,
        "status": status,
        "statusText": status_text,
        "ok": (200..300).contains(&status),
        "durationMs": duration_ms,
        "requestHeaders": request_headers,
        "responseHeaders": response_headers,
        "bodyText": body_text,
        "bodySize": body_size,
        "bodyTruncated": body_truncated,
        "contentType": content_type,
        "error": forced_error
    }))
}

struct HistoryInsert {
    collection_id: Option<i64>,
    environment_id: Option<i64>,
    request_id: Option<i64>,
    name: String,
    method: String,
    url: String,
    final_url: String,
    status: Option<i64>,
    duration_ms: u64,
    ok: bool,
    error: Option<String>,
    response_content_type: String,
    response_size: usize,
    response_body_preview: String,
    response_body_truncated: bool,
    request_snapshot_json: Option<String>,
    executed_request_snapshot_json: Option<String>,
    replayed_from_history_id: Option<i64>,
    pinned: bool,
    note: String,
}

fn truncate_to_max_bytes(input: &str, max: usize) -> String {
    if input.len() <= max {
        return input.to_string();
    }
    let mut end = 0;
    for (idx, _) in input.char_indices() {
        if idx > max {
            break;
        }
        end = idx;
    }
    input[..end].to_string()
}

fn insert_history_with_conn(conn: &Connection, item: &HistoryInsert) -> Result<(), String> {
    let preview_too_large = item.response_body_preview.len() > MAX_HISTORY_BODY_PREVIEW_BYTES;
    let preview = truncate_to_max_bytes(
        &item.response_body_preview,
        MAX_HISTORY_BODY_PREVIEW_BYTES,
    );
    conn.execute(
        "INSERT INTO api_workbench_history(
            collection_id, environment_id, request_id, name, method, url, final_url,
            status, duration_ms, ok, error, response_content_type, response_size,
            response_body_preview, response_body_truncated, request_snapshot_json,
            executed_request_snapshot_json, replayed_from_history_id, pinned, note
         )
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        params![
            item.collection_id,
            item.environment_id,
            item.request_id,
            item.name,
            item.method,
            item.url,
            item.final_url,
            item.status,
            item.duration_ms as i64,
            if item.ok { 1 } else { 0 },
            item.error,
            item.response_content_type,
            item.response_size as i64,
            preview,
            if item.response_body_truncated || preview_too_large { 1 } else { 0 },
            item.request_snapshot_json,
            item.executed_request_snapshot_json,
            item.replayed_from_history_id,
            if item.pinned { 1 } else { 0 },
            item.note
        ],
    )
    .map_err(|e| format!("insert history failed: {e}"))?;
    conn.execute(
        "DELETE FROM api_workbench_history
         WHERE pinned=0
           AND id NOT IN (
            SELECT id FROM api_workbench_history
            WHERE pinned=0
            ORDER BY created_at DESC, id DESC
            LIMIT ?1
         )",
        [MAX_HISTORY_ROWS],
    )
    .map_err(|e| format!("trim history failed: {e}"))?;
    Ok(())
}

fn send_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = payload["collectionId"].as_i64();
    let environment_id = parse_i64(payload, "environmentId")?;
    let request_id = payload["requestId"].as_i64();
    let draft: RequestDraft = serde_json::from_value(payload["draft"].clone())
        .map_err(|e| format!("请求草稿格式错误: {e}"))?;

    if let Some(collection_id) = collection_id {
        let env_owner: i64 = conn
            .query_row(
                "SELECT collection_id FROM api_workbench_environments WHERE id=?1",
                [environment_id],
                |row| row.get(0),
            )
            .map_err(|_| "环境不存在".to_string())?;
        if env_owner != collection_id {
            return Err("环境不属于当前集合".to_string());
        }
        if let Some(request_id) = request_id {
            let request_owner: i64 = conn
                .query_row(
                    "SELECT collection_id FROM api_workbench_requests WHERE id=?1",
                    [request_id],
                    |row| row.get(0),
                )
                .map_err(|_| "接口不存在".to_string())?;
            if request_owner != collection_id {
                return Err("接口不属于当前集合".to_string());
            }
        }
    }

    let (vars, base_url) = load_variables(conn, environment_id)?;
    let executed_snapshot = prepare_api_workbench_request(&draft, &vars, &base_url)?;
    let result = execute_api_workbench_request(&executed_snapshot)?;
    let request_snapshot_json =
        serialize_limited_json(&draft, MAX_HISTORY_SNAPSHOT_BYTES, "请求快照体积超过限制")?;
    let executed_snapshot_json = serialize_limited_json(
        &executed_snapshot,
        MAX_HISTORY_SNAPSHOT_BYTES,
        "执行快照体积超过限制",
    )?;
    insert_history_with_conn(
        conn,
        &HistoryInsert {
            collection_id,
            environment_id: Some(environment_id),
            request_id,
            name: payload["name"].as_str().unwrap_or_default().to_string(),
            method: draft.method.clone(),
            url: draft.url.clone(),
            final_url: executed_snapshot.final_url.clone(),
            status: result["status"].as_i64(),
            duration_ms: result["durationMs"].as_u64().unwrap_or(0),
            ok: result["ok"].as_bool().unwrap_or(false),
            error: result["error"].as_str().map(|s| s.to_string()),
            response_content_type: result["contentType"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            response_size: result["bodySize"].as_u64().unwrap_or(0) as usize,
            response_body_preview: result["bodyText"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            response_body_truncated: result["bodyTruncated"].as_bool().unwrap_or(false),
            request_snapshot_json: Some(request_snapshot_json),
            executed_request_snapshot_json: Some(executed_snapshot_json),
            replayed_from_history_id: None,
            pinned: false,
            note: String::new(),
        },
    )?;
    Ok(result)
}

fn parse_export_shell(payload: &Value) -> Result<&'static str, String> {
    match payload["targetShell"].as_str().unwrap_or("powershell") {
        "powershell" => Ok("powershell"),
        "bash" => Ok("bash"),
        other => Err(format!("unsupported shell: {other}")),
    }
}

fn quote_curl_arg(shell: &str, value: &str) -> Result<String, String> {
    if value.contains('\n') || value.contains('\r') {
        return Err("cURL 导出暂不支持包含换行的 Header 或 Body".to_string());
    }
    match shell {
        "powershell" => Ok(format!("'{}'", value.replace('\'', "''"))),
        "bash" => Ok(format!("'{}'", value.replace('\'', "'\\''"))),
        _ => Err(format!("unsupported shell: {shell}")),
    }
}

fn export_curl_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let environment_id = parse_i64(payload, "environmentId")?;
    let shell = parse_export_shell(payload)?;
    let draft: RequestDraft = serde_json::from_value(payload["draft"].clone())
        .map_err(|e| format!("请求草稿格式错误: {e}"))?;

    let env_owner: i64 = conn
        .query_row(
            "SELECT collection_id FROM api_workbench_environments WHERE id=?1",
            [environment_id],
            |row| row.get(0),
        )
        .map_err(|_| "环境不存在".to_string())?;
    if env_owner != collection_id {
        return Err("环境不属于当前集合".to_string());
    }

    let (vars, base_url) = load_variables(conn, environment_id)?;
    let resolved_url = resolve_template(&draft.url, &vars)?;
    let resolved_query = resolve_rows(&draft.query, &vars)?;
    let resolved_headers = resolve_rows(&draft.headers, &vars)?;
    let resolved_body = if matches!(draft.body_type.as_str(), "json" | "text") {
        resolve_template(&draft.body, &vars)?
    } else {
        String::new()
    };
    let resolved_form = if draft.body_type == "form-urlencoded" {
        resolve_rows(&draft.form, &vars)?
    } else {
        Vec::new()
    };
    let final_url = build_final_url(&base_url, &resolved_url, &resolved_query)?;
    let prepared = prepare_request_body(
        &draft.body_type,
        &resolved_body,
        &resolved_form,
        &resolved_headers,
    )?;

    let method = draft.method.to_ascii_uppercase();
    let mut parts = vec![
        "curl".to_string(),
        "-X".to_string(),
        method,
        quote_curl_arg(shell, &final_url)?,
    ];
    for header in resolved_headers
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
    {
        parts.push("-H".to_string());
        parts.push(quote_curl_arg(
            shell,
            &format!("{}: {}", header.key.trim(), header.value),
        )?);
    }
    if let Some(content_type) = prepared.content_type.as_deref() {
        parts.push("-H".to_string());
        parts.push(quote_curl_arg(shell, &format!("Content-Type: {content_type}"))?);
    }
    if let Some(body) = prepared.body {
        if !body.is_empty() {
            let body_text = String::from_utf8_lossy(&body);
            parts.push("--data-raw".to_string());
            parts.push(quote_curl_arg(shell, &body_text)?);
        }
    }

    Ok(json!({ "shell": shell, "command": parts.join(" ") }))
}

fn history_save_request_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let history_id = parse_i64(payload, "historyId")?;
    let collection_id = parse_i64(payload, "collectionId")?;
    let folder_id = payload["folderId"].as_i64();
    let name = parse_name(payload, "name")?;

    conn.query_row(
        "SELECT id FROM api_workbench_collections WHERE id=?1",
        [collection_id],
        |row| row.get::<_, i64>(0),
    )
    .map_err(|_| "集合不存在".to_string())?;
    if let Some(folder_id) = folder_id {
        let owner: i64 = conn
            .query_row(
                "SELECT collection_id FROM api_workbench_folders WHERE id=?1",
                [folder_id],
                |row| row.get(0),
            )
            .map_err(|_| "目标文件夹不存在".to_string())?;
        if owner != collection_id {
            return Err("目标文件夹不属于当前集合".to_string());
        }
    }

    let history = conn
        .query_row(
            "SELECT method, url, final_url, status, duration_ms, created_at
             FROM api_workbench_history WHERE id=?1",
            [history_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .map_err(|_| "历史记录不存在".to_string())?;

    let description = format!(
        "来源历史记录：状态 {}，耗时 {}ms，最终 URL：{}，创建时间：{}",
        history
            .3
            .map(|status| status.to_string())
            .unwrap_or_else(|| "ERR".to_string()),
        history.4,
        history.2,
        history.5
    );
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
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, '[]', '[]', 'none', '', '[]', 10000, ?7)",
        params![
            collection_id,
            folder_id,
            name,
            description,
            history.0,
            history.1,
            next_order
        ],
    )
    .map_err(|e| format!("save history as request failed: {e}"))?;
    Ok(json!({ "id": conn.last_insert_rowid() }))
}

fn request_save_example_response_with_conn(
    conn: &Connection,
    payload: &Value,
) -> Result<Value, String> {
    let request_id = parse_i64(payload, "requestId")?;
    let collection_id = parse_i64(payload, "collectionId")?;
    let owner: i64 = conn
        .query_row(
            "SELECT collection_id FROM api_workbench_requests WHERE id=?1",
            [request_id],
            |row| row.get(0),
        )
        .map_err(|_| "接口不存在".to_string())?;
    if owner != collection_id {
        return Err("接口不属于当前集合".to_string());
    }
    let response = payload
        .get("response")
        .ok_or_else(|| "response is required".to_string())?;
    let serialized = serde_json::to_string(response).map_err(|e| format!("示例响应格式错误: {e}"))?;
    if serialized.len() > MAX_RESPONSE_BODY_BYTES {
        return Err("示例响应体积超过限制".to_string());
    }
    conn.execute(
        "UPDATE api_workbench_requests
         SET example_response_json=?1, updated_at=CURRENT_TIMESTAMP
         WHERE id=?2 AND collection_id=?3",
        params![serialized, request_id, collection_id],
    )
    .map_err(|e| format!("save example response failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn history_list_with_conn(conn: &Connection) -> Result<Value, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, collection_id, environment_id, request_id, name, method, url, final_url,
                    status, duration_ms, ok, error, response_content_type, response_size,
                    response_body_preview, response_body_truncated, created_at
             FROM api_workbench_history ORDER BY created_at DESC, id DESC LIMIT ?1",
        )
        .map_err(|e| format!("prepare history failed: {e}"))?;
    let rows = stmt
        .query_map([MAX_HISTORY_ROWS], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "collectionId": row.get::<_, Option<i64>>(1)?,
                "environmentId": row.get::<_, Option<i64>>(2)?,
                "requestId": row.get::<_, Option<i64>>(3)?,
                "name": row.get::<_, String>(4)?,
                "method": row.get::<_, String>(5)?,
                "url": row.get::<_, String>(6)?,
                "finalUrl": row.get::<_, String>(7)?,
                "status": row.get::<_, Option<i64>>(8)?,
                "durationMs": row.get::<_, i64>(9)?,
                "ok": row.get::<_, i64>(10)? == 1,
                "error": row.get::<_, Option<String>>(11)?,
                "contentType": row.get::<_, String>(12)?,
                "bodySize": row.get::<_, i64>(13)?,
                "bodyPreview": row.get::<_, String>(14)?,
                "bodyTruncated": row.get::<_, i64>(15)? == 1,
                "createdAt": row.get::<_, String>(16)?
            }))
        })
        .map_err(|e| format!("query history failed: {e}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| e.to_string())?);
    }
    Ok(json!({ "items": items }))
}

fn history_clear_with_conn(conn: &Connection) -> Result<Value, String> {
    conn.execute("DELETE FROM api_workbench_history", [])
        .map_err(|e| format!("clear history failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization" | "cookie" | "x-api-key" | "x-auth-token"
    )
}

fn markdown_escape(text: &str) -> String {
    text.replace('|', "\\|")
}

fn render_header_lines(headers: &[KeyValueRow]) -> String {
    let mut lines = Vec::new();
    for header in headers
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
    {
        let value = if is_sensitive_header(&header.key) {
            "******".to_string()
        } else {
            header.value.clone()
        };
        lines.push(format!(
            "- {}: {}",
            markdown_escape(&header.key),
            markdown_escape(&value)
        ));
    }
    if lines.is_empty() {
        "- 无".to_string()
    } else {
        lines.join("\n")
    }
}

fn render_request_markdown(item: &Value) -> String {
    let name = item["name"].as_str().unwrap_or("未命名接口");
    let description = item["description"].as_str().unwrap_or("");
    let draft = &item["draft"];
    let method = draft["method"].as_str().unwrap_or("GET");
    let url = draft["url"].as_str().unwrap_or("");
    let headers: Vec<KeyValueRow> =
        serde_json::from_value(draft["headers"].clone()).unwrap_or_default();
    let body_type = draft["bodyType"].as_str().unwrap_or("none");
    let body = draft["body"].as_str().unwrap_or("");
    let mut out = String::new();
    out.push_str(&format!("### {name}\n\n"));
    if !description.is_empty() {
        out.push_str(description);
        out.push_str("\n\n");
    }
    out.push_str(&format!("`{method} {url}`\n\n"));
    out.push_str("#### Headers\n\n");
    out.push_str(&render_header_lines(&headers));
    out.push_str("\n\n");
    out.push_str("#### Body\n\n");
    if body_type == "none" || body.trim().is_empty() {
        out.push_str("无\n\n");
    } else {
        out.push_str(&format!("```{body_type}\n{body}\n```\n\n"));
    }
    if let Some(example) = item["exampleResponse"].as_str() {
        if let Ok(example) = serde_json::from_str::<Value>(example) {
            out.push_str("#### 示例响应\n\n");
            let status = example["status"]
                .as_i64()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "ERR".to_string());
            let status_text = example["statusText"].as_str().unwrap_or_default();
            let content_type = example["contentType"].as_str().unwrap_or_default();
            let body_text = example["bodyText"].as_str().unwrap_or_default();
            let truncated = example["bodyTruncated"].as_bool().unwrap_or(false);
            out.push_str(&format!("`{status} {status_text}`\n\n"));
            if !content_type.is_empty() {
                out.push_str(&format!("- Content-Type: `{}`\n\n", markdown_escape(content_type)));
            }
            if body_text.trim().is_empty() {
                out.push_str("无响应体\n\n");
            } else {
                out.push_str(&format!("```text\n{body_text}\n```\n\n"));
            }
            if truncated {
                out.push_str("> 响应体已截断。\n\n");
            }
        }
    }
    out
}

fn export_markdown_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let collection = conn
        .query_row(
            "SELECT name, description FROM api_workbench_collections WHERE id=?1",
            [collection_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(|_| "集合不存在".to_string())?;
    let mut markdown = String::new();
    markdown.push_str(&format!("# {}\n\n", collection.0));
    if !collection.1.is_empty() {
        markdown.push_str(&collection.1);
        markdown.push_str("\n\n");
    }

    markdown.push_str("## 环境变量\n\n");
    let mut var_stmt = conn
        .prepare(
            "SELECT e.name, v.name
             FROM api_workbench_environments e
             LEFT JOIN api_workbench_environment_variables v ON v.environment_id=e.id
             WHERE e.collection_id=?1
             ORDER BY e.sort_order ASC, e.id ASC, v.sort_order ASC",
        )
        .map_err(|e| format!("prepare vars failed: {e}"))?;
    let var_rows = var_stmt
        .query_map([collection_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(|e| format!("query vars failed: {e}"))?;
    for row in var_rows {
        let (env_name, var_name) = row.map_err(|e| e.to_string())?;
        if let Some(var_name) = var_name {
            markdown.push_str(&format!("- {}: `{}`\n", env_name, var_name));
        }
    }
    markdown.push_str("\n## 接口\n\n");

    let mut stmt = conn
        .prepare(
            "SELECT id FROM api_workbench_requests
             WHERE collection_id=?1 ORDER BY folder_id IS NOT NULL, folder_id, sort_order, id",
        )
        .map_err(|e| format!("prepare requests failed: {e}"))?;
    let ids = stmt
        .query_map([collection_id], |row| row.get::<_, i64>(0))
        .map_err(|e| format!("query requests failed: {e}"))?;
    for id in ids {
        let request = request_get_with_conn(conn, &json!({ "id": id.map_err(|e| e.to_string())? }))?;
        markdown.push_str(&render_request_markdown(&request));
    }

    let file_name = format!("{}-api.md", collection.0.trim().replace(' ', "-"));
    Ok(json!({ "fileName": file_name, "markdown": markdown }))
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    ensure_api_workbench_history_columns(&conn)?;
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
        "folder_move" => folder_move_with_conn(&conn, payload),
        "folder_reorder" => folder_reorder_with_conn(&conn, payload),
        "request_get" => request_get_with_conn(&conn, payload),
        "request_save" => request_save_with_conn(&conn, payload),
        "request_delete" => request_delete_with_conn(&conn, payload),
        "request_move" => request_move_with_conn(&conn, payload),
        "request_reorder" => request_reorder_with_conn(&conn, payload),
        "send" => send_with_conn(&conn, payload),
        "export_curl" => export_curl_with_conn(&conn, payload),
        "history_save_request" => history_save_request_with_conn(&conn, payload),
        "request_save_example_response" => request_save_example_response_with_conn(&conn, payload),
        "history_list" => history_list_with_conn(&conn),
        "history_clear" => history_clear_with_conn(&conn),
        "export_markdown" => export_markdown_with_conn(&conn, payload),
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
    fn export_curl_resolves_variables_and_quotes_for_powershell() {
        let conn = test_conn();
        let collection = collection_create_with_conn(
            &conn,
            &json!({ "name": "Demo", "description": "" }),
        )
        .expect("collection");
        let collection_id = collection["id"].as_i64().unwrap();
        let environment_id = collection["activeEnvironmentId"].as_i64().unwrap();
        environment_save_with_conn(
            &conn,
            &json!({
                "id": environment_id,
                "collectionId": collection_id,
                "name": "开发",
                "variables": [
                    { "name": "BASE_URL", "value": "http://127.0.0.1:8080", "isSecret": false },
                    { "name": "TOKEN", "value": "abc'123", "isSecret": false }
                ]
            }),
        )
        .expect("environment");

        let exported = export_curl_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "environmentId": environment_id,
                "targetShell": "powershell",
                "draft": {
                    "method": "POST",
                    "url": "/api/users",
                    "query": [{ "enabled": true, "key": "page", "value": "1" }],
                    "headers": [{ "enabled": true, "key": "Authorization", "value": "Bearer {{ TOKEN }}" }],
                    "bodyType": "json",
                    "body": "{\"name\":\"Tom\"}",
                    "form": [],
                    "timeoutMs": 10000
                }
            }),
        )
        .expect("export");

        assert_eq!(exported["shell"], "powershell");
        let command = exported["command"].as_str().unwrap();
        assert!(command.contains("curl -X POST 'http://127.0.0.1:8080/api/users?page=1'"));
        assert!(command.contains("-H 'Authorization: Bearer abc''123'"));
        assert!(command.contains("--data-raw '{\"name\":\"Tom\"}'"));
    }

    #[test]
    fn export_curl_rejects_multiline_values() {
        let conn = test_conn();
        let collection = collection_create_with_conn(
            &conn,
            &json!({ "name": "Demo", "description": "" }),
        )
        .expect("collection");
        let collection_id = collection["id"].as_i64().unwrap();
        let environment_id = collection["activeEnvironmentId"].as_i64().unwrap();

        let err = export_curl_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "environmentId": environment_id,
                "targetShell": "powershell",
                "draft": {
                    "method": "POST",
                    "url": "http://127.0.0.1:8080/api/users",
                    "query": [],
                    "headers": [],
                    "bodyType": "text",
                    "body": "line1\nline2",
                    "form": [],
                    "timeoutMs": 10000
                }
            }),
        )
        .expect_err("newline");

        assert!(err.contains("换行"));
    }

    #[test]
    fn history_save_request_creates_request_from_available_history_fields() {
        let conn = test_conn();
        let collection = collection_create_with_conn(
            &conn,
            &json!({ "name": "Demo", "description": "" }),
        )
        .expect("collection");
        let collection_id = collection["id"].as_i64().unwrap();
        insert_history_with_conn(
            &conn,
            &HistoryInsert {
                collection_id: Some(collection_id),
                environment_id: None,
                request_id: None,
                name: "".into(),
                method: "POST".into(),
                url: "/api/users".into(),
                final_url: "http://127.0.0.1:8080/api/users".into(),
                status: Some(201),
                duration_ms: 23,
                ok: true,
                error: None,
                response_content_type: "application/json".into(),
                response_size: 11,
                response_body_preview: "{\"ok\":true}".into(),
                response_body_truncated: false,
                request_snapshot_json: None,
                executed_request_snapshot_json: None,
                replayed_from_history_id: None,
                pinned: false,
                note: String::new(),
            },
        )
        .expect("history");

        let saved = history_save_request_with_conn(
            &conn,
            &json!({
                "historyId": 1,
                "collectionId": collection_id,
                "folderId": null,
                "name": "POST /api/users"
            }),
        )
        .expect("save request");
        let detail = request_get_with_conn(&conn, &json!({ "id": saved["id"] })).expect("detail");

        assert_eq!(detail["name"], "POST /api/users");
        assert_eq!(detail["draft"]["method"], "POST");
        assert_eq!(detail["draft"]["url"], "/api/users");
        assert_eq!(detail["draft"]["headers"], json!([]));
        assert_eq!(detail["draft"]["bodyType"], "none");
        assert!(detail["description"].as_str().unwrap().contains("201"));
        assert!(detail["description"]
            .as_str()
            .unwrap()
            .contains("http://127.0.0.1:8080/api/users"));
    }

    #[test]
    fn request_save_example_response_updates_request_and_markdown() {
        let conn = test_conn();
        let collection = collection_create_with_conn(
            &conn,
            &json!({ "name": "Demo", "description": "" }),
        )
        .expect("collection");
        let collection_id = collection["id"].as_i64().unwrap();
        let saved = request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": null,
                "name": "Health",
                "description": "",
                "draft": {
                    "method": "GET",
                    "url": "/health",
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
        let request_id = saved["id"].as_i64().unwrap();

        request_save_example_response_with_conn(
            &conn,
            &json!({
                "requestId": request_id,
                "collectionId": collection_id,
                "response": {
                    "status": 200,
                    "statusText": "OK",
                    "contentType": "application/json",
                    "headers": [{ "enabled": true, "key": "Content-Type", "value": "application/json" }],
                    "bodyText": "{\"ok\":true}",
                    "bodySize": 11,
                    "bodyTruncated": false,
                    "savedAt": "2026-06-30T10:00:00+08:00"
                }
            }),
        )
        .expect("example");

        let detail = request_get_with_conn(&conn, &json!({ "id": request_id })).expect("detail");
        assert!(detail["exampleResponse"]
            .as_str()
            .unwrap()
            .contains("\"status\":200"));
        let markdown =
            export_markdown_with_conn(&conn, &json!({ "collectionId": collection_id }))
                .expect("markdown");
        let markdown = markdown["markdown"].as_str().unwrap();
        assert!(markdown.contains("#### 示例响应"));
        assert!(markdown.contains("`200 OK`"));
        assert!(markdown.contains("{\"ok\":true}"));
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
    fn send_requires_environment_and_request_to_match_collection() {
        let conn = test_conn();
        let a = collection_create_with_conn(&conn, &json!({ "name": "A" })).expect("a");
        let b = collection_create_with_conn(&conn, &json!({ "name": "B" })).expect("b");
        let a_id = a["id"].as_i64().unwrap();
        let b_id = b["id"].as_i64().unwrap();
        let b_env_id = b["activeEnvironmentId"].as_i64().unwrap();
        let saved = request_save_with_conn(
            &conn,
            &json!({
                "collectionId": a_id,
                "folderId": null,
                "name": "A request",
                "draft": {
                    "method": "GET",
                    "url": "http://127.0.0.1",
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
        let request_id = saved["id"].as_i64().unwrap();

        let err = send_with_conn(
            &conn,
            &json!({
                "collectionId": a_id,
                "environmentId": b_env_id,
                "requestId": request_id,
                "draft": {
                    "method": "GET",
                    "url": "http://127.0.0.1",
                    "query": [],
                    "headers": [],
                    "bodyType": "none",
                    "body": "",
                    "form": [],
                    "timeoutMs": 100
                }
            }),
        )
        .expect_err("environment must match");
        assert!(err.contains("环境不属于当前集合"));

        let err = send_with_conn(
            &conn,
            &json!({
                "collectionId": b_id,
                "environmentId": b_env_id,
                "requestId": request_id,
                "draft": {
                    "method": "GET",
                    "url": "http://127.0.0.1",
                    "query": [],
                    "headers": [],
                    "bodyType": "none",
                    "body": "",
                    "form": [],
                    "timeoutMs": 100
                }
            }),
        )
        .expect_err("request must match");
        assert!(err.contains("接口不属于当前集合"));
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

    #[test]
    fn folder_delete_preserves_descendant_requests_as_unassigned() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let parent = folder_create_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "name": "Parent" }),
        )
        .expect("parent");
        let parent_id = parent["id"].as_i64().unwrap();
        let child = folder_create_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "parentId": parent_id, "name": "Child" }),
        )
        .expect("child");
        let child_id = child["id"].as_i64().unwrap();

        let saved = request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": child_id,
                "name": "Child request",
                "draft": {
                    "method": "GET",
                    "url": "/x",
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
        let request_id = saved["id"].as_i64().unwrap();

        folder_delete_with_conn(&conn, &json!({ "id": parent_id })).expect("delete");

        let folder_id: Option<i64> = conn
            .query_row(
                "SELECT folder_id FROM api_workbench_requests WHERE id=?1",
                [request_id],
                |row| row.get(0),
            )
            .expect("request remains");
        assert_eq!(folder_id, None);
    }

    #[test]
    fn folder_delete_reports_missing_folder() {
        let conn = test_conn();
        let err = folder_delete_with_conn(&conn, &json!({ "id": 999 })).expect_err("missing");
        assert!(err.contains("文件夹不存在"));
    }

    #[test]
    fn request_move_moves_between_folder_and_unassigned() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let folder = folder_create_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "name": "Users" }),
        )
        .expect("folder");
        let folder_id = folder["id"].as_i64().unwrap();
        let saved = request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": null,
                "name": "Health",
                "draft": {
                    "method": "GET",
                    "url": "/health",
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
        let request_id = saved["id"].as_i64().unwrap();

        request_move_with_conn(&conn, &json!({ "id": request_id, "targetFolderId": folder_id }))
            .expect("move to folder");
        let in_folder: Option<i64> = conn
            .query_row(
                "SELECT folder_id FROM api_workbench_requests WHERE id=?1",
                [request_id],
                |row| row.get(0),
            )
            .expect("folder id");
        assert_eq!(in_folder, Some(folder_id));

        request_move_with_conn(&conn, &json!({ "id": request_id, "targetFolderId": null }))
            .expect("move to unassigned");
        let unassigned: Option<i64> = conn
            .query_row(
                "SELECT folder_id FROM api_workbench_requests WHERE id=?1",
                [request_id],
                |row| row.get(0),
            )
            .expect("folder id");
        assert_eq!(unassigned, None);
    }

    #[test]
    fn folder_move_rejects_self_and_descendant_targets() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let parent = folder_create_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "name": "Parent" }),
        )
        .expect("parent");
        let parent_id = parent["id"].as_i64().unwrap();
        let child = folder_create_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "parentId": parent_id, "name": "Child" }),
        )
        .expect("child");
        let child_id = child["id"].as_i64().unwrap();

        let err =
            folder_move_with_conn(&conn, &json!({ "id": parent_id, "targetParentId": parent_id }))
                .expect_err("self");
        assert!(err.contains("自己"));

        let err =
            folder_move_with_conn(&conn, &json!({ "id": parent_id, "targetParentId": child_id }))
                .expect_err("descendant");
        assert!(err.contains("子文件夹"));
    }

    #[test]
    fn folder_reorder_requires_complete_sibling_ids() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let a = folder_create_with_conn(&conn, &json!({ "collectionId": collection_id, "name": "A" }))
            .expect("a");
        let b = folder_create_with_conn(&conn, &json!({ "collectionId": collection_id, "name": "B" }))
            .expect("b");
        let a_id = a["id"].as_i64().unwrap();
        let b_id = b["id"].as_i64().unwrap();

        let err = folder_reorder_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "parentId": null, "orderedIds": [b_id] }),
        )
        .expect_err("incomplete");
        assert!(err.contains("不完整"));

        folder_reorder_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "parentId": null, "orderedIds": [b_id, a_id] }),
        )
        .expect("reorder");
        let names: Vec<String> = {
            let mut stmt = conn
                .prepare(
                    "SELECT name FROM api_workbench_folders WHERE collection_id=?1 AND parent_id IS NULL ORDER BY sort_order ASC",
                )
                .unwrap();
            stmt.query_map([collection_id], |row| row.get(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(names, vec!["B", "A"]);
    }

    #[test]
    fn request_reorder_rejects_duplicate_ids() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let first = request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": null,
                "name": "First",
                "draft": { "method": "GET", "url": "/1", "query": [], "headers": [], "bodyType": "none", "body": "", "form": [], "timeoutMs": 10000 }
            }),
        )
        .expect("first");
        let second = request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": null,
                "name": "Second",
                "draft": { "method": "GET", "url": "/2", "query": [], "headers": [], "bodyType": "none", "body": "", "form": [], "timeoutMs": 10000 }
            }),
        )
        .expect("second");
        let first_id = first["id"].as_i64().unwrap();
        let second_id = second["id"].as_i64().unwrap();

        let err = request_reorder_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "folderId": null, "orderedIds": [first_id, first_id] }),
        )
        .expect_err("duplicate");
        assert!(err.contains("重复"));

        request_reorder_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "folderId": null, "orderedIds": [second_id, first_id] }),
        )
        .expect("reorder");
    }

    #[test]
    fn action_list_includes_recent_history() {
        let conn = test_conn();
        insert_history_with_conn(
            &conn,
            &HistoryInsert {
                collection_id: None,
                environment_id: None,
                request_id: None,
                name: "history".into(),
                method: "GET".into(),
                url: "http://127.0.0.1".into(),
                final_url: "http://127.0.0.1".into(),
                status: Some(200),
                duration_ms: 1,
                ok: true,
                error: None,
                response_content_type: "text/plain".into(),
                response_size: 2,
                response_body_preview: "ok".into(),
                response_body_truncated: false,
                request_snapshot_json: None,
                executed_request_snapshot_json: None,
                replayed_from_history_id: None,
                pinned: false,
                note: String::new(),
            },
        )
        .expect("history");

        let list = action_list_with_conn(&conn).expect("list");
        assert_eq!(list["history"][0]["name"], "history");
    }

    #[test]
    fn send_returns_http_302_without_following_redirect() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(
                    b"HTTP/1.1 302 Found\r\nLocation: /next\r\nContent-Length: 8\r\n\r\nredirect",
                );
            }
        });

        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let environment_id = c["activeEnvironmentId"].as_i64().unwrap();
        environment_save_with_conn(
            &conn,
            &json!({
                "id": environment_id,
                "collectionId": collection_id,
                "name": "开发",
                "variables": [{ "name": "BASE_URL", "value": format!("http://127.0.0.1:{port}"), "isSecret": false }]
            }),
        )
        .expect("env");

        let result = send_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "environmentId": environment_id,
                "draft": {
                    "method": "GET",
                    "url": "/redirect",
                    "query": [],
                    "headers": [],
                    "bodyType": "none",
                    "body": "",
                    "form": [],
                    "timeoutMs": 10000
                }
            }),
        )
        .expect("send");
        assert_eq!(result["status"], 302);
        assert_eq!(result["ok"], false);
        assert_eq!(result["bodyText"], "redirect");
        assert_eq!(result["responseHeaders"][0]["key"].is_string(), true);
    }

    #[test]
    fn send_ignores_inactive_body_fields_when_resolving_variables() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
            }
        });

        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let environment_id = c["activeEnvironmentId"].as_i64().unwrap();

        let result = send_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "environmentId": environment_id,
                "draft": {
                    "method": "GET",
                    "url": format!("http://127.0.0.1:{port}"),
                    "query": [],
                    "headers": [],
                    "bodyType": "none",
                    "body": "{{MISSING_BODY_VAR}}",
                    "form": [{ "enabled": true, "key": "unused", "value": "{{MISSING_FORM_VAR}}" }],
                    "timeoutMs": 10000
                }
            }),
        )
        .expect("send");
        assert_eq!(result["status"], 200);
    }

    #[test]
    fn send_writes_request_and_executed_snapshots() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nok",
                );
            }
        });

        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let environment_id = c["activeEnvironmentId"].as_i64().unwrap();
        environment_save_with_conn(
            &conn,
            &json!({
                "id": environment_id,
                "collectionId": collection_id,
                "name": "开发",
                "variables": [
                    { "name": "BASE_URL", "value": format!("http://127.0.0.1:{port}"), "isSecret": false },
                    { "name": "TOKEN", "value": "abc", "isSecret": false }
                ]
            }),
        )
        .expect("env");

        send_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "environmentId": environment_id,
                "name": "Login",
                "draft": {
                    "method": "POST",
                    "url": "/login",
                    "query": [{ "enabled": true, "key": "token", "value": "{{TOKEN}}" }],
                    "headers": [{ "enabled": true, "key": "X-Token", "value": "{{TOKEN}}" }],
                    "bodyType": "json",
                    "body": "{\"token\":\"{{TOKEN}}\"}",
                    "form": [{ "enabled": true, "key": "unused", "value": "{{TOKEN}}" }],
                    "timeoutMs": 10000
                }
            }),
        )
        .expect("send");

        let (request_snapshot, executed_snapshot): (String, String) = conn
            .query_row(
                "SELECT request_snapshot_json, executed_request_snapshot_json FROM api_workbench_history ORDER BY id DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("history snapshots");
        let request: Value =
            serde_json::from_str(&request_snapshot).expect("request snapshot json");
        let executed: Value =
            serde_json::from_str(&executed_snapshot).expect("executed snapshot json");

        assert_eq!(request["url"], "/login");
        assert_eq!(request["headers"][0]["value"], "{{TOKEN}}");
        assert_eq!(request["form"][0]["value"], "{{TOKEN}}");
        assert!(executed["finalUrl"]
            .as_str()
            .unwrap()
            .contains("/login?token=abc"));
        assert_eq!(executed["headers"][0]["value"], "abc");
        assert_eq!(executed["body"], "{\"token\":\"abc\"}");
        assert_eq!(executed["form"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn send_writes_history_and_trims_to_limit() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let environment_id = c["activeEnvironmentId"].as_i64().unwrap();
        insert_history_with_conn(
            &conn,
            &HistoryInsert {
                collection_id: Some(collection_id),
                environment_id: Some(environment_id),
                request_id: None,
                name: "x".into(),
                method: "GET".into(),
                url: "/x".into(),
                final_url: "http://127.0.0.1/x".into(),
                status: Some(200),
                duration_ms: 1,
                ok: true,
                error: None,
                response_content_type: "text/plain".into(),
                response_size: 2,
                response_body_preview: "ok".into(),
                response_body_truncated: false,
                request_snapshot_json: None,
                executed_request_snapshot_json: None,
                replayed_from_history_id: None,
                pinned: false,
                note: String::new(),
            },
        )
        .expect("history");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM api_workbench_history", [], |row| {
                row.get(0)
            })
            .expect("count");
        assert_eq!(count, 1);
    }

    #[test]
    fn export_markdown_redacts_sensitive_headers_and_hides_variable_values() {
        let conn = test_conn();
        let c = collection_create_with_conn(
            &conn,
            &json!({ "name": "Demo", "description": "API docs" }),
        )
        .expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": null,
                "name": "Auth",
                "description": "Login",
                "draft": {
                    "method": "POST",
                    "url": "/api/login",
                    "query": [],
                    "headers": [{ "enabled": true, "key": "Authorization", "value": "Bearer secret" }],
                    "bodyType": "json",
                    "body": "{\"name\":\"demo\"}",
                    "form": [],
                    "timeoutMs": 10000
                }
            }),
        )
        .expect("request");
        let result =
            export_markdown_with_conn(&conn, &json!({ "collectionId": collection_id }))
                .expect("export");
        let markdown = result["markdown"].as_str().unwrap();
        assert!(markdown.contains("# Demo"));
        assert!(markdown.contains("POST /api/login"));
        assert!(markdown.contains("Authorization: ******"));
        assert!(!markdown.contains("Bearer secret"));
        assert!(markdown.contains("BASE_URL"));
    }
}

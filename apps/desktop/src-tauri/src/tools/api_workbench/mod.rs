use super::helpers::db_conn;
#[cfg(not(test))]
use super::helpers::get_data_dir;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

mod helpers;
mod types;

use helpers::*;
use types::*;

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
  response_body_storage TEXT NOT NULL DEFAULT 'text',
  response_body_file_path TEXT NOT NULL DEFAULT '',
  response_body_file_name TEXT NOT NULL DEFAULT '',
  response_body_extension TEXT NOT NULL DEFAULT '',
  response_body_hash TEXT NOT NULL DEFAULT '',
  response_preview_error TEXT,
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
"#;

fn ensure_api_workbench_history_columns(conn: &Connection) -> Result<(), String> {
    let columns = [
        ("request_snapshot_json", "TEXT"),
        ("executed_request_snapshot_json", "TEXT"),
        ("replayed_from_history_id", "INTEGER"),
        ("pinned", "INTEGER NOT NULL DEFAULT 0"),
        ("note", "TEXT NOT NULL DEFAULT ''"),
        ("response_body_storage", "TEXT NOT NULL DEFAULT 'text'"),
        ("response_body_file_path", "TEXT NOT NULL DEFAULT ''"),
        ("response_body_file_name", "TEXT NOT NULL DEFAULT ''"),
        ("response_body_extension", "TEXT NOT NULL DEFAULT ''"),
        ("response_body_hash", "TEXT NOT NULL DEFAULT ''"),
        ("response_preview_error", "TEXT"),
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
    let mut seen_names = HashSet::new();
    for item in rows {
        let name = item["name"].as_str().unwrap_or_default().trim();
        if !validate_variable_name(name) {
            return Err(format!("变量名无效: {name}"));
        }
        if !seen_names.insert(name.to_string()) {
            return Err(format!("变量名重复: {name}"));
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
    let history = history_list_with_conn(conn, &json!({}))?["items"].clone();
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

#[cfg(test)]
fn get_api_workbench_response_cache_dir() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir()
        .join("lazycat-api-workbench-tests")
        .join("api-workbench")
        .join("response-cache");
    fs::create_dir_all(&dir).map_err(|e| format!("create response cache dir failed: {e}"))?;
    Ok(dir)
}

#[cfg(not(test))]
fn get_api_workbench_response_cache_dir() -> Result<PathBuf, String> {
    let dir = get_data_dir()?.join("api-workbench").join("response-cache");
    fs::create_dir_all(&dir).map_err(|e| format!("create response cache dir failed: {e}"))?;
    Ok(dir)
}

fn normalized_mime(content_type: &str) -> String {
    content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

fn extension_from_mime(mime: &str) -> Option<&'static str> {
    match mime {
        "application/json" => Some("json"),
        "text/html" | "application/xhtml+xml" => Some("html"),
        "text/plain" => Some("txt"),
        "text/css" => Some("css"),
        "text/csv" => Some("csv"),
        "application/xml" | "text/xml" => Some("xml"),
        "application/javascript" | "text/javascript" => Some("js"),
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        "application/pdf" => Some("pdf"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => Some("docx"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => Some("xlsx"),
        "application/vnd.ms-excel" => Some("xls"),
        "application/vnd.oasis.opendocument.spreadsheet" => Some("ods"),
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => Some("pptx"),
        "application/vnd.ms-powerpoint" => Some("ppt"),
        _ => None,
    }
}

fn extension_from_url(final_url: &str) -> Option<String> {
    let parsed = url::Url::parse(final_url).ok()?;
    let segment = parsed.path_segments()?.next_back()?;
    let (_, ext) = segment.rsplit_once('.')?;
    let ext = ext
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    if ext.is_empty() || ext.len() > 12 {
        None
    } else {
        Some(ext)
    }
}

fn filename_from_content_disposition(content_disposition: &str) -> Option<String> {
    for part in content_disposition.split(';') {
        let trimmed = part.trim();
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("filename*=") {
            let value = trimmed.split_once('=')?.1.trim().trim_matches('"');
            if let Some((_, encoded)) = value.rsplit_once("''") {
                return Some(sanitize_file_name(&urlencoding::decode(encoded).ok()?));
            }
        }
        if lower.starts_with("filename=") {
            let value = trimmed.split_once('=')?.1.trim().trim_matches('"');
            return Some(sanitize_file_name(value));
        }
    }
    None
}

fn sanitize_file_name(input: &str) -> String {
    let name = input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let name = name.trim_matches('.').trim_matches('_').to_string();
    if name.is_empty() {
        "response".to_string()
    } else {
        name.chars().take(96).collect()
    }
}

fn extension_from_file_name(file_name: &str) -> Option<String> {
    let (_, ext) = file_name.rsplit_once('.')?;
    let ext = ext
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    if ext.is_empty() || ext.len() > 12 {
        None
    } else {
        Some(ext)
    }
}

fn extension_from_bytes(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("jpg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("gif");
    }
    if bytes.starts_with(b"%PDF-") {
        return Some("pdf");
    }
    if bytes.starts_with(b"PK\x03\x04") {
        return Some("zip");
    }
    None
}

fn looks_textual_mime(mime: &str) -> bool {
    mime.starts_with("text/")
        || matches!(
            mime,
            "application/json"
                | "application/xml"
                | "application/xhtml+xml"
                | "application/javascript"
                | "application/x-www-form-urlencoded"
                | "image/svg+xml"
        )
        || mime.ends_with("+json")
        || mime.ends_with("+xml")
}

fn looks_binary_mime(mime: &str) -> bool {
    mime.starts_with("image/")
        || matches!(
            mime,
            "application/pdf"
                | "application/octet-stream"
                | "application/zip"
                | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                | "application/vnd.openxmlformats-officedocument.presentationml.presentation"
                | "application/vnd.ms-excel"
                | "application/vnd.ms-powerpoint"
                | "application/vnd.oasis.opendocument.spreadsheet"
        )
}

fn classify_response_storage(mime: &str, bytes: &[u8]) -> &'static str {
    if bytes.is_empty() {
        return "empty";
    }
    if looks_textual_mime(mime) {
        return "text";
    }
    if looks_binary_mime(mime) || extension_from_bytes(bytes).is_some() {
        return "file";
    }
    if std::str::from_utf8(bytes).is_ok() {
        "text"
    } else {
        "file"
    }
}

fn persist_response_cache_file(
    cache_dir: &Path,
    bytes: &[u8],
    file_name_hint: Option<String>,
    extension_hint: Option<String>,
) -> Result<(String, String, String, String), String> {
    fs::create_dir_all(cache_dir).map_err(|e| format!("create response cache dir failed: {e}"))?;
    let cache_dir = cache_dir
        .canonicalize()
        .map_err(|e| format!("resolve response cache dir failed: {e}"))?;
    let hash = blake3::hash(bytes).to_hex().to_string();
    let hash_prefix = &hash[..16];
    let extension = extension_hint.unwrap_or_else(|| "bin".to_string());
    let display_name = file_name_hint.unwrap_or_else(|| format!("response.{extension}"));
    let timestamp = chrono::Local::now().format("%Y%m%d%H%M%S");
    let file_name = format!("{timestamp}-{hash_prefix}.{extension}");
    let target = cache_dir.join(&file_name);
    fs::write(&target, bytes).map_err(|e| format!("write response cache failed: {e}"))?;
    let canonical_target = target
        .canonicalize()
        .map_err(|e| format!("resolve response cache file failed: {e}"))?;
    if !canonical_target.starts_with(&cache_dir) {
        let _ = fs::remove_file(&canonical_target);
        return Err("response cache path escaped cache dir".to_string());
    }
    Ok((
        canonical_target.to_string_lossy().to_string(),
        display_name,
        extension,
        hash,
    ))
}

fn build_response_body_payload(
    final_url: &str,
    content_type: &str,
    content_disposition: &str,
    bytes: Vec<u8>,
    body_truncated: bool,
) -> ResponseBodyPayload {
    let body_size = bytes.len();
    let mime = normalized_mime(content_type);
    let storage = classify_response_storage(&mime, &bytes);
    if storage == "empty" {
        return ResponseBodyPayload {
            body_text: String::new(),
            body_size,
            body_truncated,
            body_storage: "empty".to_string(),
            body_file_path: String::new(),
            body_file_name: String::new(),
            body_extension: String::new(),
            body_hash: String::new(),
            body_preview_error: None,
        };
    }
    if storage == "text" {
        return ResponseBodyPayload {
            body_text: String::from_utf8_lossy(&bytes).to_string(),
            body_size,
            body_truncated,
            body_storage: "text".to_string(),
            body_file_path: String::new(),
            body_file_name: String::new(),
            body_extension: String::new(),
            body_hash: String::new(),
            body_preview_error: None,
        };
    }
    if body_truncated {
        return ResponseBodyPayload {
            body_text: String::new(),
            body_size,
            body_truncated,
            body_storage: "truncated-binary".to_string(),
            body_file_path: String::new(),
            body_file_name: String::new(),
            body_extension: String::new(),
            body_hash: String::new(),
            body_preview_error: Some("二进制响应已截断，未生成预览缓存".to_string()),
        };
    }

    let file_name_hint = filename_from_content_disposition(content_disposition);
    let extension_hint = file_name_hint
        .as_deref()
        .and_then(extension_from_file_name)
        .or_else(|| extension_from_url(final_url))
        .or_else(|| extension_from_mime(&mime).map(str::to_string))
        .or_else(|| extension_from_bytes(&bytes).map(str::to_string))
        .unwrap_or_else(|| "bin".to_string());
    match get_api_workbench_response_cache_dir().and_then(|dir| {
        persist_response_cache_file(&dir, &bytes, file_name_hint, Some(extension_hint))
    }) {
        Ok((path, display_name, extension, hash)) => ResponseBodyPayload {
            body_text: String::new(),
            body_size,
            body_truncated,
            body_storage: "file".to_string(),
            body_file_path: path,
            body_file_name: display_name,
            body_extension: extension,
            body_hash: hash,
            body_preview_error: None,
        },
        Err(error) => ResponseBodyPayload {
            body_text: String::new(),
            body_size,
            body_truncated,
            body_storage: "file".to_string(),
            body_file_path: String::new(),
            body_file_name: String::new(),
            body_extension: String::new(),
            body_hash: String::new(),
            body_preview_error: Some(error),
        },
    }
}

#[derive(Debug, Clone)]
struct HistoryCacheRef {
    file_path: String,
}

fn validate_response_cache_file_path(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("cache path is empty".to_string());
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err("cache path must be absolute".to_string());
    }
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err("cache path contains `..`".to_string());
        }
    }
    #[cfg(windows)]
    {
        if trimmed.starts_with(r"\\.\") {
            return Err("device namespace path not allowed".to_string());
        }
    }
    let cache_dir = get_api_workbench_response_cache_dir()?
        .canonicalize()
        .map_err(|e| format!("resolve response cache dir failed: {e}"))?;
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("resolve response cache file failed: {e}"))?;
    if !canonical.starts_with(cache_dir) {
        return Err("cache path is outside response cache dir".to_string());
    }
    Ok(canonical)
}

fn remove_response_cache_file_if_safe(file_path: &str) -> Result<(), String> {
    if file_path.trim().is_empty() {
        return Ok(());
    }
    if !PathBuf::from(file_path).exists() {
        return Ok(());
    }
    let canonical = validate_response_cache_file_path(file_path)?;
    fs::remove_file(&canonical).map_err(|e| format!("remove response cache failed: {e}"))
}

fn collect_history_cache_refs(
    conn: &Connection,
    where_sql: &str,
) -> Result<Vec<HistoryCacheRef>, String> {
    let sql = format!("SELECT response_body_file_path FROM api_workbench_history {where_sql}");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("prepare history cache refs failed: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(HistoryCacheRef {
                file_path: row.get::<_, String>(0)?,
            })
        })
        .map_err(|e| format!("query history cache refs failed: {e}"))?;
    let mut refs = Vec::new();
    for row in rows {
        let item = row.map_err(|e| e.to_string())?;
        if !item.file_path.trim().is_empty() {
            refs.push(item);
        }
    }
    Ok(refs)
}

fn cleanup_unreferenced_history_cache_files(conn: &Connection, refs: &[HistoryCacheRef]) {
    let mut seen = HashSet::new();
    for item in refs {
        if item.file_path.trim().is_empty() || !seen.insert(item.file_path.clone()) {
            continue;
        }
        let remaining: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM api_workbench_history WHERE response_body_file_path=?1",
                params![item.file_path],
                |row| row.get(0),
            )
            .unwrap_or(1);
        if remaining == 0 {
            let _ = remove_response_cache_file_if_safe(&item.file_path);
        }
    }
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
            "bodyStorage": "empty",
            "bodyFilePath": "",
            "bodyFileName": "",
            "bodyExtension": "",
            "bodyHash": "",
            "bodyPreviewError": null,
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
    let content_disposition = resp.header("Content-Disposition").unwrap_or("").to_string();
    let response_headers: Vec<Value> = resp
        .headers_names()
        .into_iter()
        .map(|key| {
            let value = resp.header(&key).unwrap_or("").to_string();
            json!({ "enabled": true, "key": key, "value": value })
        })
        .collect();
    let mut reader = resp
        .into_reader()
        .take((MAX_RESPONSE_BODY_BYTES + 1) as u64);
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read response body failed: {e}"))?;
    let body_truncated = bytes.len() > MAX_RESPONSE_BODY_BYTES;
    if body_truncated {
        bytes.truncate(MAX_RESPONSE_BODY_BYTES);
    }
    let body = build_response_body_payload(
        final_url,
        &content_type,
        &content_disposition,
        bytes,
        body_truncated,
    );
    Ok(json!({
        "finalUrl": final_url,
        "status": status,
        "statusText": status_text,
        "ok": (200..300).contains(&status),
        "durationMs": duration_ms,
        "requestHeaders": request_headers,
        "responseHeaders": response_headers,
        "bodyText": body.body_text,
        "bodySize": body.body_size,
        "bodyTruncated": body.body_truncated,
        "contentType": content_type,
        "bodyStorage": body.body_storage,
        "bodyFilePath": body.body_file_path,
        "bodyFileName": body.body_file_name,
        "bodyExtension": body.body_extension,
        "bodyHash": body.body_hash,
        "bodyPreviewError": body.body_preview_error,
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
    response_body_storage: String,
    response_body_file_path: String,
    response_body_file_name: String,
    response_body_extension: String,
    response_body_hash: String,
    response_preview_error: Option<String>,
    request_snapshot_json: Option<String>,
    executed_request_snapshot_json: Option<String>,
    replayed_from_history_id: Option<i64>,
    pinned: bool,
    note: String,
}

impl Default for HistoryInsert {
    fn default() -> Self {
        Self {
            collection_id: None,
            environment_id: None,
            request_id: None,
            name: String::new(),
            method: "GET".to_string(),
            url: String::new(),
            final_url: String::new(),
            status: None,
            duration_ms: 0,
            ok: false,
            error: None,
            response_content_type: String::new(),
            response_size: 0,
            response_body_preview: String::new(),
            response_body_truncated: false,
            response_body_storage: "text".to_string(),
            response_body_file_path: String::new(),
            response_body_file_name: String::new(),
            response_body_extension: String::new(),
            response_body_hash: String::new(),
            response_preview_error: None,
            request_snapshot_json: None,
            executed_request_snapshot_json: None,
            replayed_from_history_id: None,
            pinned: false,
            note: String::new(),
        }
    }
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

fn insert_history_with_conn(conn: &Connection, item: &HistoryInsert) -> Result<i64, String> {
    let preview_too_large = item.response_body_preview.len() > MAX_HISTORY_BODY_PREVIEW_BYTES;
    let preview =
        truncate_to_max_bytes(&item.response_body_preview, MAX_HISTORY_BODY_PREVIEW_BYTES);
    conn.execute(
        "INSERT INTO api_workbench_history(
            collection_id, environment_id, request_id, name, method, url, final_url,
            status, duration_ms, ok, error, response_content_type, response_size,
            response_body_preview, response_body_truncated, request_snapshot_json,
            executed_request_snapshot_json, replayed_from_history_id, pinned, note,
            response_body_storage, response_body_file_path, response_body_file_name,
            response_body_extension, response_body_hash, response_preview_error
         )
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
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
            item.note,
            item.response_body_storage,
            item.response_body_file_path,
            item.response_body_file_name,
            item.response_body_extension,
            item.response_body_hash,
            item.response_preview_error,
        ],
    )
    .map_err(|e| format!("insert history failed: {e}"))?;
    let id = conn.last_insert_rowid();
    let trimmed_cache_refs = collect_history_cache_refs(
        conn,
        &format!(
            "WHERE pinned=0
           AND id NOT IN (
            SELECT id FROM api_workbench_history
            WHERE pinned=0
            ORDER BY created_at DESC, id DESC
            LIMIT {MAX_HISTORY_ROWS}
         )"
        ),
    )?;
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
    cleanup_unreferenced_history_cache_files(conn, &trimmed_cache_refs);
    Ok(id)
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
            response_body_preview: result["bodyText"].as_str().unwrap_or_default().to_string(),
            response_body_truncated: result["bodyTruncated"].as_bool().unwrap_or(false),
            response_body_storage: result["bodyStorage"].as_str().unwrap_or("text").to_string(),
            response_body_file_path: result["bodyFilePath"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            response_body_file_name: result["bodyFileName"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            response_body_extension: result["bodyExtension"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            response_body_hash: result["bodyHash"].as_str().unwrap_or_default().to_string(),
            response_preview_error: result["bodyPreviewError"].as_str().map(|s| s.to_string()),
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
        parts.push(quote_curl_arg(
            shell,
            &format!("Content-Type: {content_type}"),
        )?);
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
            "SELECT method, url, final_url, status, duration_ms, created_at, request_snapshot_json
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
                    row.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .map_err(|_| "历史记录不存在".to_string())?;
    let draft = if let Some(raw) = history.6.as_deref() {
        serde_json::from_str::<RequestDraft>(raw).map_err(|_| "历史快照已损坏".to_string())?
    } else {
        RequestDraft {
            method: history.0.clone(),
            url: history.1.clone(),
            query: Vec::new(),
            headers: Vec::new(),
            body_type: "none".to_string(),
            body: String::new(),
            form: Vec::new(),
            timeout_ms: 10000,
        }
    };
    let query_json = serde_json::to_string(&draft.query).map_err(|e| e.to_string())?;
    let headers_json = serde_json::to_string(&draft.headers).map_err(|e| e.to_string())?;
    let form_json = serde_json::to_string(&draft.form).map_err(|e| e.to_string())?;

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
    let mut response = payload
        .get("response")
        .cloned()
        .ok_or_else(|| "response is required".to_string())?;
    sanitize_example_response(&mut response);
    let serialized =
        serde_json::to_string(&response).map_err(|e| format!("示例响应格式错误: {e}"))?;
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

fn sanitize_example_response(response: &mut Value) {
    let Some(obj) = response.as_object_mut() else {
        return;
    };
    let storage = obj
        .get("bodyStorage")
        .and_then(|value| value.as_str())
        .unwrap_or("text")
        .to_string();
    if matches!(storage.as_str(), "file" | "truncated-binary") {
        obj.remove("bodyFilePath");
        obj.remove("bodyHash");
        let file_name = obj
            .get("bodyFileName")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let extension = obj
            .get("bodyExtension")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let body_size = obj
            .get("bodySize")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        let summary = if storage == "truncated-binary" {
            format!("二进制响应已截断，仅保存元信息摘要（{body_size} bytes）。")
        } else if file_name.is_empty() {
            format!("二进制响应，仅保存元信息摘要（{body_size} bytes，{extension}）。")
        } else {
            format!("二进制响应，仅保存元信息摘要（{file_name}，{body_size} bytes）。")
        };
        obj.insert("bodyText".to_string(), Value::String(summary));
    }
}

fn history_row_json(
    row: &rusqlite::Row<'_>,
    include_request_snapshot: bool,
) -> rusqlite::Result<Value> {
    let request_snapshot_json: Option<String> = row.get(23)?;
    let executed_snapshot_json: Option<String> = row.get(24)?;
    let mut value = json!({
        "id": row.get::<_, i64>(0)?,
        "collectionId": row.get::<_, Option<i64>>(1)?,
        "environmentId": row.get::<_, Option<i64>>(2)?,
        "requestId": row.get::<_, Option<i64>>(3)?,
        "replayedFromHistoryId": row.get::<_, Option<i64>>(25)?,
        "name": row.get::<_, String>(4)?,
        "note": row.get::<_, String>(26)?,
        "pinned": row.get::<_, i64>(27)? == 1,
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
        "bodyStorage": row.get::<_, String>(16)?,
        "bodyFilePath": row.get::<_, String>(17)?,
        "bodyFileName": row.get::<_, String>(18)?,
        "bodyExtension": row.get::<_, String>(19)?,
        "bodyHash": row.get::<_, String>(20)?,
        "bodyPreviewError": row.get::<_, Option<String>>(21)?,
        "createdAt": row.get::<_, String>(22)?,
        "hasRequestSnapshot": request_snapshot_json.is_some(),
        "hasExecutedRequestSnapshot": executed_snapshot_json.is_some()
    });
    if include_request_snapshot {
        value["requestSnapshot"] = match request_snapshot_json {
            Some(raw) => serde_json::from_str(&raw).unwrap_or(Value::Null),
            None => Value::Null,
        };
    }
    Ok(value)
}

fn history_get_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let history_id = parse_i64(payload, "historyId")?;
    let detail = conn
        .query_row(
            "SELECT id, collection_id, environment_id, request_id, name, method, url, final_url,
                    status, duration_ms, ok, error, response_content_type, response_size,
                    response_body_preview, response_body_truncated,
                    response_body_storage, response_body_file_path, response_body_file_name,
                    response_body_extension, response_body_hash, response_preview_error,
                    created_at,
                    request_snapshot_json, executed_request_snapshot_json, replayed_from_history_id,
                    note, pinned
             FROM api_workbench_history WHERE id=?1",
            [history_id],
            |row| history_row_json(row, true),
        )
        .map_err(|_| "历史记录不存在".to_string())?;
    if detail["hasRequestSnapshot"].as_bool().unwrap_or(false)
        && detail["requestSnapshot"].is_null()
    {
        return Err("历史快照已损坏".to_string());
    }
    Ok(detail)
}

fn history_replay_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let history_id = parse_i64(payload, "historyId")?;
    let (raw, name): (Option<String>, String) = conn
        .query_row(
            "SELECT executed_request_snapshot_json, name FROM api_workbench_history WHERE id=?1",
            [history_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| "历史记录不存在".to_string())?;
    let raw = raw.ok_or_else(|| "旧历史缺少执行快照，请载入后手动发送".to_string())?;
    let snapshot: ExecutedRequestSnapshot =
        serde_json::from_str(&raw).map_err(|_| "历史快照已损坏".to_string())?;
    let result = execute_api_workbench_request(&snapshot)?;
    let history_record_id = insert_history_with_conn(
        conn,
        &HistoryInsert {
            collection_id: None,
            environment_id: None,
            request_id: None,
            name,
            method: snapshot.method.clone(),
            url: snapshot.final_url.clone(),
            final_url: snapshot.final_url.clone(),
            status: result["status"].as_i64(),
            duration_ms: result["durationMs"].as_u64().unwrap_or(0),
            ok: result["ok"].as_bool().unwrap_or(false),
            error: result["error"].as_str().map(|s| s.to_string()),
            response_content_type: result["contentType"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            response_size: result["bodySize"].as_u64().unwrap_or(0) as usize,
            response_body_preview: result["bodyText"].as_str().unwrap_or_default().to_string(),
            response_body_truncated: result["bodyTruncated"].as_bool().unwrap_or(false),
            response_body_storage: result["bodyStorage"].as_str().unwrap_or("text").to_string(),
            response_body_file_path: result["bodyFilePath"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            response_body_file_name: result["bodyFileName"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            response_body_extension: result["bodyExtension"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            response_body_hash: result["bodyHash"].as_str().unwrap_or_default().to_string(),
            response_preview_error: result["bodyPreviewError"].as_str().map(|s| s.to_string()),
            request_snapshot_json: None,
            executed_request_snapshot_json: Some(raw),
            replayed_from_history_id: Some(history_id),
            pinned: false,
            note: String::new(),
        },
    )?;
    let mut out = result;
    out["historyId"] = json!(history_record_id);
    Ok(out)
}

fn history_list_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let query = payload["query"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_string();
    let pinned_only = payload["pinnedOnly"].as_bool().unwrap_or(false);
    let limit = payload["limit"]
        .as_i64()
        .unwrap_or(MAX_HISTORY_ROWS)
        .clamp(1, MAX_HISTORY_ROWS);
    let pattern = format!(
        "%{}%",
        query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let sql = r#"
SELECT id, collection_id, environment_id, request_id, name, method, url, final_url,
       status, duration_ms, ok, error, response_content_type, response_size,
       response_body_preview, response_body_truncated,
       response_body_storage, response_body_file_path, response_body_file_name,
       response_body_extension, response_body_hash, response_preview_error,
       created_at,
       request_snapshot_json, executed_request_snapshot_json, replayed_from_history_id,
       note, pinned
FROM api_workbench_history
WHERE (?1 = 0 OR pinned = 1)
  AND (
    ?2 = ''
    OR name LIKE ?3 ESCAPE '\'
    OR note LIKE ?3 ESCAPE '\'
    OR method LIKE ?3 ESCAPE '\'
    OR url LIKE ?3 ESCAPE '\'
    OR final_url LIKE ?3 ESCAPE '\'
    OR CAST(status AS TEXT) LIKE ?3 ESCAPE '\'
    OR COALESCE(error, '') LIKE ?3 ESCAPE '\'
    OR response_content_type LIKE ?3 ESCAPE '\'
  )
ORDER BY created_at DESC, id DESC
LIMIT ?4"#;
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("prepare history failed: {e}"))?;
    let rows = stmt
        .query_map(
            params![if pinned_only { 1 } else { 0 }, query, pattern, limit],
            |row| history_row_json(row, false),
        )
        .map_err(|e| format!("query history failed: {e}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| e.to_string())?);
    }
    Ok(json!({ "items": items }))
}

fn history_update_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    let name = payload["name"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_string();
    let note = payload["note"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_string();
    if note.chars().count() > MAX_HISTORY_NOTE_CHARS {
        return Err("历史备注超过 2000 字符".to_string());
    }
    let pinned = payload["pinned"]
        .as_bool()
        .ok_or_else(|| "pinned must be a boolean".to_string())?;
    let changed = conn
        .execute(
            "UPDATE api_workbench_history SET name=?1, note=?2, pinned=?3 WHERE id=?4",
            params![name, note, if pinned { 1 } else { 0 }, id],
        )
        .map_err(|e| format!("update history failed: {e}"))?;
    if changed == 0 {
        return Err("历史记录不存在".to_string());
    }
    Ok(json!({ "ok": true }))
}

fn history_clear_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let include_pinned = payload["includePinned"].as_bool().unwrap_or(false);
    let cache_refs = if include_pinned {
        collect_history_cache_refs(conn, "")?
    } else {
        collect_history_cache_refs(conn, "WHERE pinned=0")?
    };
    let sql = if include_pinned {
        "DELETE FROM api_workbench_history"
    } else {
        "DELETE FROM api_workbench_history WHERE pinned=0"
    };
    conn.execute(sql, [])
        .map_err(|e| format!("clear history failed: {e}"))?;
    cleanup_unreferenced_history_cache_files(conn, &cache_refs);
    Ok(json!({ "ok": true }))
}

fn response_preview_office_with_conn(_conn: &Connection, payload: &Value) -> Result<Value, String> {
    let file_path = payload["filePath"]
        .as_str()
        .ok_or_else(|| "filePath is required".to_string())?;
    let kind = payload["kind"]
        .as_str()
        .ok_or_else(|| "kind is required".to_string())?;
    let path = validate_response_cache_file_path(file_path)?;
    match kind {
        "word" => preview_word_file(&path),
        "sheet" => preview_sheet_file(&path, payload),
        "slides" => preview_slides_file(&path),
        other => Err(format!("unsupported office preview kind: {other}")),
    }
}

fn response_cache_open_with_conn(_conn: &Connection, payload: &Value) -> Result<Value, String> {
    let file_path = payload["filePath"]
        .as_str()
        .ok_or_else(|| "filePath is required".to_string())?;
    let path = validate_response_cache_file_path(file_path)?;
    open::that(path).map_err(|e| format!("open response cache failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn response_cache_reveal_with_conn(_conn: &Connection, payload: &Value) -> Result<Value, String> {
    let file_path = payload["filePath"]
        .as_str()
        .ok_or_else(|| "filePath is required".to_string())?;
    let path = validate_response_cache_file_path(file_path)?;
    reveal_response_cache_file(&path)?;
    Ok(json!({ "ok": true }))
}

#[cfg(windows)]
fn reveal_response_cache_file(path: &Path) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn()
        .map_err(|e| format!("explorer launch failed: {e}"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn reveal_response_cache_file(path: &Path) -> Result<(), String> {
    std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn()
        .map_err(|e| format!("open -R failed: {e}"))?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn reveal_response_cache_file(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "cache file has no parent".to_string())?;
    std::process::Command::new("xdg-open")
        .arg(parent)
        .spawn()
        .map_err(|e| format!("xdg-open failed: {e}"))?;
    Ok(())
}

fn path_extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn preview_sheet_file(path: &Path, payload: &Value) -> Result<Value, String> {
    let ext = path_extension(path);
    if ext == "csv" {
        return preview_csv_file(path, payload);
    }
    preview_workbook_file(path, payload)
}

fn preview_csv_file(path: &Path, payload: &Value) -> Result<Value, String> {
    let offset = payload["offset"].as_u64().unwrap_or(0) as usize;
    let limit = payload["limit"].as_u64().unwrap_or(200).clamp(1, 200) as usize;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(path)
        .map_err(|e| format!("无法打开 CSV 文件: {e}"))?;
    let mut rows = Vec::new();
    let mut total_rows = 0usize;
    let mut column_count = 0usize;
    for record in reader.records() {
        let record = record.map_err(|e| format!("无法读取 CSV 文件: {e}"))?;
        if total_rows >= offset && rows.len() < limit {
            let row: Vec<String> = record.iter().map(|cell| cell.to_string()).collect();
            column_count = column_count.max(row.len());
            rows.push(row);
        }
        total_rows += 1;
    }
    Ok(json!({
        "kind": "sheet",
        "sheetNames": ["CSV"],
        "activeSheet": "CSV",
        "offset": offset,
        "limit": limit,
        "totalRows": total_rows,
        "totalColumns": column_count,
        "truncated": total_rows > offset + rows.len(),
        "rows": rows
    }))
}

fn api_workbench_cell_to_string(cell: &calamine::Data) -> String {
    match cell {
        calamine::Data::Empty => String::new(),
        calamine::Data::String(value) => value.trim().to_string(),
        calamine::Data::Float(value) => format!("{value}"),
        calamine::Data::Int(value) => format!("{value}"),
        calamine::Data::Bool(value) => value.to_string(),
        calamine::Data::DateTime(value) => value.to_string(),
        _ => String::new(),
    }
}

fn preview_workbook_file(path: &Path, payload: &Value) -> Result<Value, String> {
    use calamine::Reader;

    let offset = payload["offset"].as_u64().unwrap_or(0) as usize;
    let limit = payload["limit"].as_u64().unwrap_or(200).clamp(1, 200) as usize;
    let mut workbook =
        calamine::open_workbook_auto(path).map_err(|e| format!("无法打开表格文件: {e}"))?;
    let sheet_names = workbook.sheet_names().to_vec();
    let active_sheet = payload["sheetName"]
        .as_str()
        .filter(|name| sheet_names.iter().any(|item| item == *name))
        .map(str::to_string)
        .or_else(|| sheet_names.first().cloned())
        .ok_or_else(|| "表格文件没有工作表".to_string())?;
    let range = workbook
        .worksheet_range(&active_sheet)
        .map_err(|e| format!("无法读取工作表: {e}"))?;
    let total_rows = range.height();
    let total_columns = range.width();
    let rows: Vec<Vec<String>> = range
        .rows()
        .skip(offset)
        .take(limit)
        .map(|row| row.iter().map(api_workbench_cell_to_string).collect())
        .collect();
    Ok(json!({
        "kind": "sheet",
        "sheetNames": sheet_names,
        "activeSheet": active_sheet,
        "offset": offset,
        "limit": limit,
        "totalRows": total_rows,
        "totalColumns": total_columns,
        "truncated": total_rows > offset + rows.len(),
        "rows": rows
    }))
}

fn extract_xml_texts(xml: &str, max_chars: usize) -> Vec<String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut texts = Vec::new();
    let mut used_chars = 0usize;
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Text(event)) => {
                let text = String::from_utf8_lossy(event.as_ref()).trim().to_string();
                if text.is_empty() {
                    continue;
                }
                used_chars += text.chars().count();
                if used_chars > max_chars {
                    break;
                }
                texts.push(text);
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    texts
}

fn read_zip_text_entry(
    archive: &mut zip::ZipArchive<fs::File>,
    name: &str,
) -> Result<String, String> {
    let mut entry = archive
        .by_name(name)
        .map_err(|e| format!("无法读取 OpenXML 内容: {e}"))?;
    let mut xml = String::new();
    entry
        .read_to_string(&mut xml)
        .map_err(|e| format!("无法读取 OpenXML 文本: {e}"))?;
    Ok(xml)
}

fn preview_word_file(path: &Path) -> Result<Value, String> {
    let ext = path_extension(path);
    if ext != "docx" {
        return Ok(json!({
            "kind": "word",
            "paragraphs": [],
            "tables": [],
            "imageCount": 0,
            "truncated": false,
            "unsupported": true,
            "message": "该格式暂不支持基础预览"
        }));
    }
    let file = fs::File::open(path).map_err(|e| format!("无法打开 Word 文件: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("无法读取 docx 文件: {e}"))?;
    let xml = read_zip_text_entry(&mut archive, "word/document.xml")?;
    let paragraphs = extract_xml_texts(&xml, 200_000);
    let image_count = (0..archive.len())
        .filter_map(|idx| {
            archive
                .by_index(idx)
                .ok()
                .map(|entry| entry.name().to_string())
        })
        .filter(|name| name.starts_with("word/media/"))
        .count();
    Ok(json!({
        "kind": "word",
        "paragraphs": paragraphs,
        "tables": [],
        "imageCount": image_count,
        "truncated": false,
        "unsupported": false
    }))
}

fn slide_sort_key(name: &str) -> i64 {
    name.rsplit_once("slide")
        .and_then(|(_, tail)| tail.split('.').next())
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(i64::MAX)
}

fn preview_slides_file(path: &Path) -> Result<Value, String> {
    let ext = path_extension(path);
    if ext != "pptx" {
        return Ok(json!({
            "kind": "slides",
            "slides": [],
            "truncated": false,
            "unsupported": true,
            "message": "该格式暂不支持基础预览"
        }));
    }
    let file = fs::File::open(path).map_err(|e| format!("无法打开 PowerPoint 文件: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("无法读取 pptx 文件: {e}"))?;
    let mut slide_names = Vec::new();
    let mut image_count = 0usize;
    for idx in 0..archive.len() {
        let entry = archive
            .by_index(idx)
            .map_err(|e| format!("无法读取 pptx 条目: {e}"))?;
        let name = entry.name().to_string();
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            slide_names.push(name);
        } else if name.starts_with("ppt/media/") {
            image_count += 1;
        }
    }
    slide_names.sort_by_key(|name| slide_sort_key(name));
    let mut slides = Vec::new();
    let mut truncated = false;
    for (index, name) in slide_names.iter().enumerate() {
        if index >= 100 {
            truncated = true;
            break;
        }
        let xml = read_zip_text_entry(&mut archive, name)?;
        let texts = extract_xml_texts(&xml, 100_000);
        let title = texts
            .first()
            .cloned()
            .unwrap_or_else(|| format!("幻灯片 {}", index + 1));
        slides.push(json!({
            "index": index + 1,
            "title": title,
            "texts": texts,
            "notes": [],
            "imageCount": image_count
        }));
    }
    Ok(json!({
        "kind": "slides",
        "slides": slides,
        "truncated": truncated,
        "unsupported": false
    }))
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
                out.push_str(&format!(
                    "- Content-Type: `{}`\n\n",
                    markdown_escape(content_type)
                ));
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
        let request =
            request_get_with_conn(conn, &json!({ "id": id.map_err(|e| e.to_string())? }))?;
        markdown.push_str(&render_request_markdown(&request));
    }

    let file_name = format!("{}-api.md", collection.0.trim().replace(' ', "-"));
    Ok(json!({ "fileName": file_name, "markdown": markdown }))
}

fn is_supported_api_workbench_action(action: &str) -> bool {
    matches!(
        action,
        "list"
            | "collection_create"
            | "collection_update"
            | "collection_set_active_environment"
            | "collection_delete"
            | "folder_create"
            | "folder_update"
            | "folder_delete"
            | "folder_move"
            | "folder_reorder"
            | "request_get"
            | "request_save"
            | "request_delete"
            | "request_move"
            | "request_reorder"
            | "send"
            | "export_curl"
            | "history_save_request"
            | "request_save_example_response"
            | "history_list"
            | "history_get"
            | "history_replay"
            | "history_update"
            | "history_clear"
            | "response_preview_office"
            | "response_cache_open"
            | "response_cache_reveal"
            | "export_markdown"
            | "environment_list"
            | "environment_save"
            | "environment_delete"
            | "global_variables_list"
            | "global_variables_save"
    )
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !is_supported_api_workbench_action(action) {
        return Err(format!("unsupported api_workbench action: {action}"));
    }
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
        "history_list" => history_list_with_conn(&conn, payload),
        "history_get" => history_get_with_conn(&conn, payload),
        "history_replay" => history_replay_with_conn(&conn, payload),
        "history_update" => history_update_with_conn(&conn, payload),
        "history_clear" => history_clear_with_conn(&conn, payload),
        "response_preview_office" => response_preview_office_with_conn(&conn, payload),
        "response_cache_open" => response_cache_open_with_conn(&conn, payload),
        "response_cache_reveal" => response_cache_reveal_with_conn(&conn, payload),
        "export_markdown" => export_markdown_with_conn(&conn, payload),
        "environment_list" => environment_list_with_conn(&conn, payload),
        "environment_save" => environment_save_with_conn(&conn, payload),
        "environment_delete" => environment_delete_with_conn(&conn, payload),
        "global_variables_list" => global_variables_list_with_conn(&conn),
        "global_variables_save" => global_variables_save_with_conn(&conn, payload),
        _ => unreachable!("api workbench action checked before dispatch"),
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
    fn api_workbench_schema_handles_existing_history_without_pinned_column() {
        let conn = Connection::open_in_memory().expect("open memory db");
        conn.execute_batch(
            "CREATE TABLE api_workbench_history (
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
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .expect("old api history schema");

        conn.execute_batch(API_WORKBENCH_SCHEMA_SQL)
            .expect("api workbench schema should allow old history table");
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

    #[test]
    fn export_curl_resolves_variables_and_quotes_for_powershell() {
        let conn = test_conn();
        let collection =
            collection_create_with_conn(&conn, &json!({ "name": "Demo", "description": "" }))
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
        let collection =
            collection_create_with_conn(&conn, &json!({ "name": "Demo", "description": "" }))
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
        let collection =
            collection_create_with_conn(&conn, &json!({ "name": "Demo", "description": "" }))
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
                ..HistoryInsert::default()
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
    fn history_get_returns_request_snapshot_for_loading() {
        let conn = test_conn();
        let request_snapshot = json!({
            "method": "POST",
            "url": "/login",
            "query": [{ "enabled": true, "key": "a", "value": "1" }],
            "headers": [{ "enabled": true, "key": "X-A", "value": "b" }],
            "bodyType": "json",
            "body": "{\"ok\":true}",
            "form": [],
            "timeoutMs": 15000
        });
        insert_history_with_conn(
            &conn,
            &HistoryInsert {
                collection_id: None,
                environment_id: None,
                request_id: None,
                name: "Login".into(),
                method: "POST".into(),
                url: "/login".into(),
                final_url: "http://127.0.0.1/login".into(),
                status: Some(200),
                duration_ms: 10,
                ok: true,
                error: None,
                response_content_type: "application/json".into(),
                response_size: 2,
                response_body_preview: "{}".into(),
                response_body_truncated: false,
                request_snapshot_json: Some(request_snapshot.to_string()),
                executed_request_snapshot_json: None,
                replayed_from_history_id: None,
                pinned: false,
                note: String::new(),
                ..HistoryInsert::default()
            },
        )
        .expect("history");
        let id: i64 = conn
            .query_row("SELECT id FROM api_workbench_history", [], |row| row.get(0))
            .unwrap();

        let detail = history_get_with_conn(&conn, &json!({ "historyId": id })).expect("detail");
        assert_eq!(detail["requestSnapshot"]["headers"][0]["key"], "X-A");
        assert_eq!(detail["hasRequestSnapshot"], true);
        assert_eq!(detail["hasExecutedRequestSnapshot"], false);
    }

    #[test]
    fn history_replay_uses_executed_snapshot_without_environment() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 2048];
                let size = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..size]);
                assert!(req.contains("GET /replay?token=abc HTTP/1.1"));
                assert!(req.contains("X-Token: abc"));
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nreplay");
            }
        });

        let conn = test_conn();
        let executed_snapshot = json!({
            "method": "GET",
            "finalUrl": format!("http://127.0.0.1:{port}/replay?token=abc"),
            "headers": [{ "enabled": true, "key": "X-Token", "value": "abc" }],
            "bodyType": "none",
            "body": "",
            "form": [],
            "timeoutMs": 10000
        });
        insert_history_with_conn(
            &conn,
            &HistoryInsert {
                collection_id: None,
                environment_id: None,
                request_id: None,
                name: "Replay".into(),
                method: "GET".into(),
                url: "/replay".into(),
                final_url: format!("http://127.0.0.1:{port}/replay?token=abc"),
                status: Some(200),
                duration_ms: 1,
                ok: true,
                error: None,
                response_content_type: "text/plain".into(),
                response_size: 6,
                response_body_preview: "replay".into(),
                response_body_truncated: false,
                request_snapshot_json: None,
                executed_request_snapshot_json: Some(executed_snapshot.to_string()),
                replayed_from_history_id: None,
                pinned: false,
                note: String::new(),
                ..HistoryInsert::default()
            },
        )
        .expect("history");
        let id: i64 = conn
            .query_row("SELECT id FROM api_workbench_history", [], |row| row.get(0))
            .unwrap();

        let result = history_replay_with_conn(&conn, &json!({ "historyId": id })).expect("replay");
        assert_eq!(result["status"], 200);
        assert_eq!(result["bodyText"], "replay");
        assert!(result["historyId"].as_i64().unwrap() > id);
        let parent: i64 = conn
            .query_row(
                "SELECT replayed_from_history_id FROM api_workbench_history WHERE id=?1",
                [result["historyId"].as_i64().unwrap()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(parent, id);
    }

    #[test]
    fn history_save_request_uses_request_snapshot_when_available() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let request_snapshot = json!({
            "method": "PATCH",
            "url": "/users/1",
            "query": [{ "enabled": true, "key": "expand", "value": "roles" }],
            "headers": [{ "enabled": true, "key": "X-Token", "value": "{{TOKEN}}" }],
            "bodyType": "json",
            "body": "{\"name\":\"demo\"}",
            "form": [],
            "timeoutMs": 12000
        });
        insert_history_with_conn(
            &conn,
            &HistoryInsert {
                collection_id: Some(collection_id),
                environment_id: None,
                request_id: None,
                name: "Patch user".into(),
                method: "PATCH".into(),
                url: "/users/1".into(),
                final_url: "http://127.0.0.1/users/1?expand=roles".into(),
                status: Some(200),
                duration_ms: 7,
                ok: true,
                error: None,
                response_content_type: "application/json".into(),
                response_size: 2,
                response_body_preview: "{}".into(),
                response_body_truncated: false,
                request_snapshot_json: Some(request_snapshot.to_string()),
                executed_request_snapshot_json: None,
                replayed_from_history_id: None,
                pinned: false,
                note: String::new(),
                ..HistoryInsert::default()
            },
        )
        .expect("history");
        let history_id: i64 = conn
            .query_row("SELECT id FROM api_workbench_history", [], |row| row.get(0))
            .unwrap();

        let saved = history_save_request_with_conn(
            &conn,
            &json!({ "historyId": history_id, "collectionId": collection_id, "folderId": null, "name": "Saved" }),
        )
        .expect("save");
        let detail = request_get_with_conn(&conn, &json!({ "id": saved["id"].as_i64().unwrap() }))
            .expect("detail");
        assert_eq!(detail["draft"]["method"], "PATCH");
        assert_eq!(detail["draft"]["headers"][0]["value"], "{{TOKEN}}");
        assert_eq!(detail["draft"]["timeoutMs"], 12000);
    }

    #[test]
    fn history_update_allows_empty_name_and_validates_note_length() {
        let conn = test_conn();
        insert_history_with_conn(
            &conn,
            &HistoryInsert {
                collection_id: None,
                environment_id: None,
                request_id: None,
                name: "Old".into(),
                method: "GET".into(),
                url: "/x".into(),
                final_url: "/x".into(),
                status: Some(200),
                duration_ms: 1,
                ok: true,
                error: None,
                response_content_type: String::new(),
                response_size: 0,
                response_body_preview: String::new(),
                response_body_truncated: false,
                request_snapshot_json: None,
                executed_request_snapshot_json: None,
                replayed_from_history_id: None,
                pinned: false,
                note: String::new(),
                ..HistoryInsert::default()
            },
        )
        .expect("history");
        let id: i64 = conn
            .query_row("SELECT id FROM api_workbench_history", [], |row| row.get(0))
            .unwrap();

        history_update_with_conn(
            &conn,
            &json!({ "id": id, "name": "", "note": "keep", "pinned": true }),
        )
        .expect("update");
        let (name, note, pinned): (String, String, i64) = conn
            .query_row(
                "SELECT name, note, pinned FROM api_workbench_history WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(name, "");
        assert_eq!(note, "keep");
        assert_eq!(pinned, 1);

        let long_note = "x".repeat(MAX_HISTORY_NOTE_CHARS + 1);
        let err = history_update_with_conn(
            &conn,
            &json!({ "id": id, "name": "", "note": long_note, "pinned": true }),
        )
        .expect_err("long note");
        assert!(err.contains("备注"));
    }

    #[test]
    fn history_clear_preserves_pinned_by_default() {
        let conn = test_conn();
        for (name, pinned) in [("keep", true), ("drop", false)] {
            insert_history_with_conn(
                &conn,
                &HistoryInsert {
                    collection_id: None,
                    environment_id: None,
                    request_id: None,
                    name: name.into(),
                    method: "GET".into(),
                    url: format!("/{name}"),
                    final_url: format!("/{name}"),
                    status: Some(200),
                    duration_ms: 1,
                    ok: true,
                    error: None,
                    response_content_type: String::new(),
                    response_size: 0,
                    response_body_preview: String::new(),
                    response_body_truncated: false,
                    request_snapshot_json: None,
                    executed_request_snapshot_json: None,
                    replayed_from_history_id: None,
                    pinned,
                    note: String::new(),
                    ..HistoryInsert::default()
                },
            )
            .expect("history");
        }

        history_clear_with_conn(&conn, &json!({ "includePinned": false })).expect("clear");
        let names: Vec<String> = conn
            .prepare("SELECT name FROM api_workbench_history ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(names, vec!["keep"]);

        history_clear_with_conn(&conn, &json!({ "includePinned": true })).expect("clear all");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM api_workbench_history", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn history_list_filters_search_and_pinned() {
        let conn = test_conn();
        insert_history_with_conn(
            &conn,
            &HistoryInsert {
                collection_id: None,
                environment_id: None,
                request_id: None,
                name: "Login ok".into(),
                method: "POST".into(),
                url: "/login".into(),
                final_url: "http://127.0.0.1/login".into(),
                status: Some(200),
                duration_ms: 1,
                ok: true,
                error: None,
                response_content_type: "application/json".into(),
                response_size: 2,
                response_body_preview: "{}".into(),
                response_body_truncated: false,
                request_snapshot_json: Some("{} ".trim().to_string()),
                executed_request_snapshot_json: Some("{} ".trim().to_string()),
                replayed_from_history_id: None,
                pinned: true,
                note: "admin token".into(),
                ..HistoryInsert::default()
            },
        )
        .expect("history");
        insert_history_with_conn(
            &conn,
            &HistoryInsert {
                collection_id: None,
                environment_id: None,
                request_id: None,
                name: "Health".into(),
                method: "GET".into(),
                url: "/health".into(),
                final_url: "http://127.0.0.1/health".into(),
                status: Some(500),
                duration_ms: 1,
                ok: false,
                error: Some("boom".into()),
                response_content_type: "text/plain".into(),
                response_size: 4,
                response_body_preview: "fail".into(),
                response_body_truncated: false,
                request_snapshot_json: None,
                executed_request_snapshot_json: None,
                replayed_from_history_id: None,
                pinned: false,
                note: String::new(),
                ..HistoryInsert::default()
            },
        )
        .expect("history");

        let pinned = history_list_with_conn(
            &conn,
            &json!({ "query": "token", "pinnedOnly": true, "limit": 200 }),
        )
        .expect("list");
        assert_eq!(pinned["items"].as_array().unwrap().len(), 1);
        assert_eq!(pinned["items"][0]["name"], "Login ok");
        assert_eq!(pinned["items"][0]["hasRequestSnapshot"], true);
        assert_eq!(pinned["items"][0]["hasExecutedRequestSnapshot"], true);
    }

    #[test]
    fn request_save_example_response_updates_request_and_markdown() {
        let conn = test_conn();
        let collection =
            collection_create_with_conn(&conn, &json!({ "name": "Demo", "description": "" }))
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
        let markdown = export_markdown_with_conn(&conn, &json!({ "collectionId": collection_id }))
            .expect("markdown");
        let markdown = markdown["markdown"].as_str().unwrap();
        assert!(markdown.contains("#### 示例响应"));
        assert!(markdown.contains("`200 OK`"));
        assert!(markdown.contains("{\"ok\":true}"));
    }

    #[test]
    fn request_save_example_response_omits_binary_cache_path() {
        let conn = test_conn();
        let collection =
            collection_create_with_conn(&conn, &json!({ "name": "Demo", "description": "" }))
                .expect("collection");
        let collection_id = collection["id"].as_i64().unwrap();
        let saved = request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": null,
                "name": "Download",
                "description": "",
                "draft": {
                    "method": "GET",
                    "url": "/download",
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
                    "contentType": "application/pdf",
                    "headers": [],
                    "bodyText": "",
                    "bodySize": 128,
                    "bodyTruncated": false,
                    "bodyStorage": "file",
                    "bodyFilePath": "C:/should/not/be/saved.pdf",
                    "bodyFileName": "report.pdf",
                    "bodyExtension": "pdf",
                    "bodyHash": "abc",
                    "savedAt": "2026-07-01T00:00:00.000Z"
                }
            }),
        )
        .expect("example");

        let raw: String = conn
            .query_row(
                "SELECT example_response_json FROM api_workbench_requests WHERE id=?1",
                [request_id],
                |row| row.get(0),
            )
            .expect("example");
        let example: Value = serde_json::from_str(&raw).expect("example json");
        assert_eq!(example["bodyStorage"], "file");
        assert!(example.get("bodyFilePath").is_none());
        assert!(example.get("bodyHash").is_none());
        assert!(example["bodyText"].as_str().unwrap().contains("二进制响应"));
    }

    #[test]
    fn collection_create_initializes_default_environment_and_base_url() {
        let conn = test_conn();
        let result =
            collection_create_with_conn(&conn, &json!({ "name": "Demo", "description": "desc" }))
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
    fn environment_save_rejects_duplicate_variable_names() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let environment_id = c["activeEnvironmentId"].as_i64().unwrap();

        let err = environment_save_with_conn(
            &conn,
            &json!({
                "id": environment_id,
                "collectionId": collection_id,
                "name": "开发",
                "variables": [
                    { "name": "TOKEN", "value": "a", "isSecret": false },
                    { "name": " TOKEN ", "value": "b", "isSecret": false }
                ]
            }),
        )
        .expect_err("duplicate variable");

        assert!(err.contains("变量名重复: TOKEN"));
        assert!(!err.contains("UNIQUE constraint"));
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

        request_move_with_conn(
            &conn,
            &json!({ "id": request_id, "targetFolderId": folder_id }),
        )
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

        let err = folder_move_with_conn(
            &conn,
            &json!({ "id": parent_id, "targetParentId": parent_id }),
        )
        .expect_err("self");
        assert!(err.contains("自己"));

        let err = folder_move_with_conn(
            &conn,
            &json!({ "id": parent_id, "targetParentId": child_id }),
        )
        .expect_err("descendant");
        assert!(err.contains("子文件夹"));
    }

    #[test]
    fn folder_reorder_requires_complete_sibling_ids() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let a = folder_create_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "name": "A" }),
        )
        .expect("a");
        let b = folder_create_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "name": "B" }),
        )
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
                ..HistoryInsert::default()
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
    fn send_caches_binary_response_without_lossy_body_text() {
        use std::fs;
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let body = vec![
            0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0xff, 0x00, 0x80,
        ];
        let expected_body = body.clone();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&body);
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
                    "url": "/image.png",
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

        assert_eq!(result["bodyStorage"], "file");
        assert_eq!(result["bodyText"], "");
        assert_eq!(result["bodyExtension"], "png");
        let file_path = result["bodyFilePath"].as_str().expect("cache path");
        assert!(file_path.contains("api-workbench"));
        assert_eq!(fs::read(file_path).expect("read cache"), expected_body);
    }

    #[test]
    fn send_does_not_cache_truncated_binary_response() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let body = vec![0x7fu8; MAX_RESPONSE_BODY_BYTES + 8];
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&body);
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
                    "url": "/large.bin",
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

        assert_eq!(result["bodyStorage"], "truncated-binary");
        assert_eq!(result["bodyTruncated"], true);
        assert_eq!(result["bodyFilePath"], "");
        assert_eq!(result["bodyHash"], "");
    }

    #[test]
    fn send_writes_binary_cache_reference_to_history() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let body = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 1, 2, 3];
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&body);
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
                    "url": "/image.png",
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

        let history_id: i64 = conn
            .query_row("SELECT id FROM api_workbench_history", [], |row| row.get(0))
            .expect("history id");
        let detail = history_get_with_conn(&conn, &json!({ "historyId": history_id }))
            .expect("history detail");

        assert_eq!(detail["bodyStorage"], "file");
        assert_eq!(detail["bodyFilePath"], result["bodyFilePath"]);
        assert_eq!(detail["bodyExtension"], "png");
        assert_eq!(detail["bodyHash"], result["bodyHash"]);
    }

    #[test]
    fn history_clear_removes_unreferenced_response_cache() {
        use std::fs;
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let body = vec![0x25, b'P', b'D', b'F', b'-', b'1'];
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/pdf\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&body);
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
                    "url": "/report.pdf",
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
        let file_path = result["bodyFilePath"]
            .as_str()
            .expect("cache path")
            .to_string();
        assert!(fs::metadata(&file_path).is_ok());

        history_clear_with_conn(&conn, &json!({ "includePinned": true })).expect("clear history");

        assert!(fs::metadata(&file_path).is_err());
    }

    #[test]
    fn history_clear_keeps_cache_still_referenced_by_pinned_history() {
        use std::fs;

        let conn = test_conn();
        let cache_dir = get_api_workbench_response_cache_dir().expect("cache dir");
        let (file_path, file_name, extension, hash) =
            persist_response_cache_file(&cache_dir, b"same-cache", None, Some("bin".into()))
                .expect("cache file");

        for pinned in [true, false] {
            insert_history_with_conn(
                &conn,
                &HistoryInsert {
                    name: if pinned { "keep".into() } else { "drop".into() },
                    method: "GET".into(),
                    url: "/file".into(),
                    final_url: "/file".into(),
                    status: Some(200),
                    ok: true,
                    response_body_storage: "file".into(),
                    response_body_file_path: file_path.clone(),
                    response_body_file_name: file_name.clone(),
                    response_body_extension: extension.clone(),
                    response_body_hash: hash.clone(),
                    pinned,
                    ..HistoryInsert::default()
                },
            )
            .expect("history");
        }

        history_clear_with_conn(&conn, &json!({ "includePinned": false })).expect("clear unpinned");
        assert!(fs::metadata(&file_path).is_ok());

        history_clear_with_conn(&conn, &json!({ "includePinned": true })).expect("clear all");
        assert!(fs::metadata(&file_path).is_err());
    }

    #[test]
    fn history_clear_deletes_same_hash_cache_when_path_is_unreferenced() {
        use std::fs;

        let conn = test_conn();
        let cache_dir = get_api_workbench_response_cache_dir().expect("cache dir");
        let bytes = b"same-bytes";
        let (drop_path, drop_name, extension, hash) =
            persist_response_cache_file(&cache_dir, bytes, None, Some("bin".into()))
                .expect("cache file");
        let keep_path = cache_dir.join("same-hash-different-path.bin");
        fs::write(&keep_path, bytes).expect("keep cache file");
        let keep_path = keep_path
            .canonicalize()
            .expect("canonical keep path")
            .to_string_lossy()
            .to_string();

        for (name, path, pinned) in [
            ("drop", drop_path.clone(), false),
            ("keep", keep_path.clone(), true),
        ] {
            insert_history_with_conn(
                &conn,
                &HistoryInsert {
                    name: name.into(),
                    method: "GET".into(),
                    url: "/file".into(),
                    final_url: "/file".into(),
                    status: Some(200),
                    ok: true,
                    response_body_storage: "file".into(),
                    response_body_file_path: path,
                    response_body_file_name: drop_name.clone(),
                    response_body_extension: extension.clone(),
                    response_body_hash: hash.clone(),
                    pinned,
                    ..HistoryInsert::default()
                },
            )
            .expect("history");
        }

        history_clear_with_conn(&conn, &json!({ "includePinned": false })).expect("clear unpinned");

        assert!(fs::metadata(&drop_path).is_err());
        assert!(fs::metadata(&keep_path).is_ok());
    }

    #[test]
    fn insert_history_trims_unpinned_cache_files() {
        use std::fs;

        let conn = test_conn();
        let cache_dir = get_api_workbench_response_cache_dir().expect("cache dir");
        let mut first_path = String::new();

        for idx in 0..(MAX_HISTORY_ROWS + 1) {
            let bytes = format!("cache-{idx}");
            let (file_path, file_name, extension, hash) =
                persist_response_cache_file(&cache_dir, bytes.as_bytes(), None, Some("bin".into()))
                    .expect("cache file");
            if idx == 0 {
                first_path = file_path.clone();
            }
            insert_history_with_conn(
                &conn,
                &HistoryInsert {
                    name: format!("history-{idx}"),
                    method: "GET".into(),
                    url: format!("/{idx}"),
                    final_url: format!("/{idx}"),
                    status: Some(200),
                    ok: true,
                    response_body_storage: "file".into(),
                    response_body_file_path: file_path,
                    response_body_file_name: file_name,
                    response_body_extension: extension,
                    response_body_hash: hash,
                    pinned: false,
                    ..HistoryInsert::default()
                },
            )
            .expect("history");
        }

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM api_workbench_history", [], |row| {
                row.get(0)
            })
            .expect("count");
        assert_eq!(count, MAX_HISTORY_ROWS);
        assert!(fs::metadata(&first_path).is_err());
    }

    #[test]
    fn response_cache_path_validation_rejects_outside_file() {
        let outside = std::env::temp_dir().join("lazycat-api-workbench-outside.bin");
        fs::write(&outside, b"outside").expect("outside file");

        let err = validate_response_cache_file_path(&outside.to_string_lossy())
            .expect_err("outside path");
        assert!(err.contains("outside"));
    }

    #[test]
    fn response_preview_office_rejects_outside_cache_path() {
        let conn = test_conn();
        let outside = std::env::temp_dir().join("lazycat-api-workbench-outside.docx");
        fs::write(&outside, b"outside").expect("outside file");

        let err = response_preview_office_with_conn(
            &conn,
            &json!({ "filePath": outside.to_string_lossy(), "kind": "word" }),
        )
        .expect_err("outside path");
        assert!(err.contains("outside"));
    }

    #[test]
    fn response_preview_office_reads_csv_sheet() {
        let conn = test_conn();
        let cache_dir = get_api_workbench_response_cache_dir().expect("cache dir");
        let (file_path, _, _, _) = persist_response_cache_file(
            &cache_dir,
            b"name,age\nAlice,30\nBob,31\n",
            Some("users.csv".into()),
            Some("csv".into()),
        )
        .expect("cache file");

        let preview = response_preview_office_with_conn(
            &conn,
            &json!({ "filePath": file_path, "kind": "sheet" }),
        )
        .expect("preview");

        assert_eq!(preview["kind"], "sheet");
        assert_eq!(preview["sheetNames"], json!(["CSV"]));
        assert_eq!(preview["rows"][0], json!(["name", "age"]));
        assert_eq!(preview["rows"][1], json!(["Alice", "30"]));
    }

    fn write_openxml_zip(path: &Path, entries: &[(&str, &str)]) {
        let file = fs::File::create(path).expect("zip file");
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default();
        for (name, content) in entries {
            zip.start_file(*name, options).expect("zip entry");
            std::io::Write::write_all(&mut zip, content.as_bytes()).expect("zip content");
        }
        zip.finish().expect("finish zip");
    }

    #[test]
    fn response_preview_office_extracts_docx_text() {
        let conn = test_conn();
        let cache_dir = get_api_workbench_response_cache_dir().expect("cache dir");
        let source = cache_dir.join("sample.docx");
        write_openxml_zip(
            &source,
            &[(
                "word/document.xml",
                r#"<w:document xmlns:w="w"><w:body><w:p><w:r><w:t>API 文档</w:t></w:r></w:p><w:p><w:r><w:t>响应预览</w:t></w:r></w:p></w:body></w:document>"#,
            )],
        );
        let source_bytes = fs::read(&source).expect("docx bytes");
        let (file_path, _, _, _) = persist_response_cache_file(
            &cache_dir,
            &source_bytes,
            Some("sample.docx".into()),
            Some("docx".into()),
        )
        .expect("cache file");

        let preview = response_preview_office_with_conn(
            &conn,
            &json!({ "filePath": file_path, "kind": "word" }),
        )
        .expect("preview");

        assert_eq!(preview["kind"], "word");
        assert_eq!(preview["paragraphs"][0], "API 文档");
        assert_eq!(preview["paragraphs"][1], "响应预览");
    }

    #[test]
    fn response_preview_office_extracts_pptx_slides() {
        let conn = test_conn();
        let cache_dir = get_api_workbench_response_cache_dir().expect("cache dir");
        let source = cache_dir.join("deck.pptx");
        write_openxml_zip(
            &source,
            &[(
                "ppt/slides/slide1.xml",
                r#"<p:sld xmlns:a="a" xmlns:p="p"><p:cSld><p:spTree><a:t>标题</a:t><a:t>要点一</a:t></p:spTree></p:cSld></p:sld>"#,
            )],
        );
        let source_bytes = fs::read(&source).expect("pptx bytes");
        let (file_path, _, _, _) = persist_response_cache_file(
            &cache_dir,
            &source_bytes,
            Some("deck.pptx".into()),
            Some("pptx".into()),
        )
        .expect("cache file");

        let preview = response_preview_office_with_conn(
            &conn,
            &json!({ "filePath": file_path, "kind": "slides" }),
        )
        .expect("preview");

        assert_eq!(preview["kind"], "slides");
        assert_eq!(preview["slides"][0]["title"], "标题");
        assert_eq!(preview["slides"][0]["texts"][1], "要点一");
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
                ..HistoryInsert::default()
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
        let result = export_markdown_with_conn(&conn, &json!({ "collectionId": collection_id }))
            .expect("export");
        let markdown = result["markdown"].as_str().unwrap();
        assert!(markdown.contains("# Demo"));
        assert!(markdown.contains("POST /api/login"));
        assert!(markdown.contains("Authorization: ******"));
        assert!(!markdown.contains("Bearer secret"));
        assert!(markdown.contains("BASE_URL"));
    }
}

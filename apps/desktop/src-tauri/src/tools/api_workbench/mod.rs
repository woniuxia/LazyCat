use super::helpers::db_conn;
use rusqlite::{params, Connection};
use serde_json::{json, Value};

mod collection;
mod environment;
mod executor;
mod export;
mod folder;
mod helpers;
mod request;
mod response;
mod types;

use collection::*;
use environment::*;
use executor::*;
use export::*;
use folder::*;
use helpers::*;
use request::*;
use response::*;
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
pub(crate) fn test_conn() -> Connection {
    let conn = Connection::open_in_memory().expect("open memory db");
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .expect("foreign keys");
    conn.execute_batch(API_WORKBENCH_SCHEMA_SQL)
        .expect("schema");
    conn
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::json;

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

}

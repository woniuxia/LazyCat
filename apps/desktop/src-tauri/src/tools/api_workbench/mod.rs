use super::helpers::db_conn;
use rusqlite::Connection;
use serde_json::{json, Value};

mod collection;
mod environment;
mod executor;
mod export;
mod folder;
mod helpers;
mod history;
mod request;
mod response;
mod types;

use collection::*;
use environment::*;
use executor::*;
use export::*;
use folder::*;
use history::*;
use request::*;
use response::*;

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

const ACTIONS: &[&str] = &[
    "list",
    "collection_create",
    "collection_update",
    "collection_set_active_environment",
    "collection_delete",
    "folder_create",
    "folder_update",
    "folder_delete",
    "folder_move",
    "folder_reorder",
    "request_get",
    "request_save",
    "request_delete",
    "request_move",
    "request_reorder",
    "send",
    "export_curl",
    "history_save_request",
    "request_save_example_response",
    "history_list",
    "history_get",
    "history_replay",
    "history_update",
    "history_clear",
    "response_preview_office",
    "response_cache_open",
    "response_cache_reveal",
    "export_markdown",
    "environment_list",
    "environment_save",
    "environment_delete",
    "global_variables_list",
    "global_variables_save",
];

pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
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


}

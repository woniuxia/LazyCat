mod bindings;
mod definitions;
mod dispatches;

use rusqlite::Connection;
use serde_json::{json, Value};

const ACTIONS: &[&str] = &["definition_list", "target_list"];

pub(crate) const ACTION_CENTER_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS action_bindings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    trigger_type TEXT NOT NULL,
    trigger_id TEXT NOT NULL,
    action_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(trigger_type, trigger_id)
);
CREATE TABLE IF NOT EXISTS action_dispatches (
    id TEXT PRIMARY KEY,
    binding_id INTEGER NULL REFERENCES action_bindings(id) ON DELETE SET NULL,
    trigger_type TEXT NOT NULL,
    trigger_id TEXT NOT NULL,
    trigger_event_id TEXT NOT NULL,
    action_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending_confirmation','running','succeeded','failed','cancelled')),
    external_run_id TEXT NULL,
    result_code TEXT NULL,
    error TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    started_at TEXT NULL,
    finished_at TEXT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_action_dispatches_one_active_binding
ON action_dispatches(binding_id)
WHERE binding_id IS NOT NULL AND status IN ('pending_confirmation','running');
CREATE UNIQUE INDEX IF NOT EXISTS idx_action_dispatches_external_run
ON action_dispatches(external_run_id) WHERE external_run_id IS NOT NULL;
"#;

pub(crate) fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(ACTION_CENTER_SCHEMA_SQL)
        .map_err(|error| format!("create action center schema failed: {error}"))
}

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "definition_list" => Ok(json!({
            "definitions": definitions::all_definitions(),
        })),
        "target_list" => {
            let action_type = payload
                .get("actionType")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or("actionType 不能为空")?;
            let conn = super::helpers::db_conn()?;
            Ok(json!({
                "targets": definitions::list_targets(&conn, action_type)?,
            }))
        }
        _ => Err(format!("unsupported action_center action: {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn action_center_schema_creates_binding_and_dispatch_tables() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema(&conn).unwrap();

        for table in ["action_bindings", "action_dispatches"] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "missing table {table}");
        }
    }

    #[test]
    fn action_center_schema_has_active_dispatch_uniqueness() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema(&conn).unwrap();
        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name='idx_action_dispatches_one_active_binding'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(sql.contains("pending_confirmation"));
        assert!(sql.contains("running"));
    }
}

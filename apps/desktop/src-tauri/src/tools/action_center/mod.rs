mod atomic_actions;
mod bindings;
mod combinations;
mod definitions;
mod dispatches;

use rusqlite::Connection;
use serde_json::{json, Value};

pub(crate) use bindings::{
    apply_todo_binding_patch, attach_todo_binding_summaries, delete_todo_binding,
    ensure_todo_can_become_recurring, parse_binding_patch, todo_reminder_action_summary,
    BindingPatch,
};
pub(crate) use dispatches::{associate_release_package_run, recover_interrupted_dispatches};
#[cfg(not(test))]
pub(crate) use dispatches::finish_release_package_run;

const ACTIONS: &[&str] = &[
    "definition_list",
    "target_list",
    "binding_get",
    "dispatch",
    "dispatch_cancel",
    "dispatch_latest",
];

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
        .map_err(|error| format!("create action center schema failed: {error}"))?;
    combinations::ensure_schema(conn)
}

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported action_center action: {action}"));
    }
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
        "binding_get" => bindings::binding_get(payload),
        "dispatch" => Err("action_center dispatch requires app context".into()),
        "dispatch_cancel" => dispatches::dispatch_cancel(payload),
        "dispatch_latest" => dispatches::dispatch_latest(payload),
        _ => unreachable!("action center action whitelist and dispatcher must stay in sync"),
    }
}

pub fn execute_with_app(
    action: &str,
    payload: &Value,
    app: &tauri::AppHandle,
) -> Result<Value, String> {
    if action == "dispatch" {
        dispatches::dispatch_with_app(app, payload)
    } else {
        execute(action, payload)
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

        for table in [
            "action_bindings",
            "action_dispatches",
            "action_combinations",
            "action_combination_steps",
            "action_combination_runs",
            "action_combination_run_steps",
        ] {
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

    #[test]
    fn action_center_schema_has_global_active_combination_run_uniqueness() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema(&conn).unwrap();
        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name='idx_action_combination_runs_one_active'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(sql.contains("pending"));
        assert!(sql.contains("running"));
    }
}

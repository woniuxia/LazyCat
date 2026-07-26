mod atomic_actions;
mod bindings;
mod combination_executor;
mod combination_runs;
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
#[cfg(not(test))]
pub(crate) use dispatches::finish_release_package_run;
pub(crate) use dispatches::{associate_release_package_run, recover_interrupted_dispatches};

const ACTIONS: &[&str] = &[
    "definition_list",
    "target_list",
    "binding_get",
    "dispatch",
    "dispatch_cancel",
    "dispatch_latest",
    "combination_definition_list",
    "combination_target_list",
    "combination_list",
    "combination_get",
    "combination_save",
    "combination_delete",
    "combination_run",
    "combination_run_get",
    "combination_run_list",
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

fn parse_combination_id(payload: &Value) -> Result<i64, String> {
    payload
        .get("combinationId")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| "combinationId 必须是正整数".to_string())
}

fn parse_combination_run_id(payload: &Value) -> Result<&str, String> {
    payload
        .get("runId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|run_id| !run_id.is_empty())
        .ok_or_else(|| "runId 不能为空".to_string())
}

fn combination_detail_value(conn: &Connection, combination_id: i64) -> Result<Value, String> {
    let detail = combinations::get_with_conn(conn, combination_id)?;
    let mut value = serde_json::to_value(&detail)
        .map_err(|error| format!("serialize combination detail failed: {error}"))?;
    let step_values = value
        .get_mut("steps")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "serialized combination detail has no steps".to_string())?;
    for (step, step_value) in detail.steps.iter().zip(step_values) {
        let snapshot =
            atomic_actions::snapshot_target_with_conn(conn, &step.action_type, &step.target_id);
        let object = step_value
            .as_object_mut()
            .ok_or_else(|| "serialized combination step is not an object".to_string())?;
        object.insert("targetLabel".into(), Value::String(snapshot.target_label));
        object.insert(
            "available".into(),
            Value::Bool(snapshot.validation_error.is_none()),
        );
        if let Some(reason) = snapshot.validation_error {
            object.insert("unavailableReason".into(), Value::String(reason));
        }
    }
    Ok(value)
}

fn execute_combination_with_conn<F>(
    action: &str,
    payload: &Value,
    conn: &mut Connection,
    validate_target: F,
) -> Result<Value, String>
where
    F: Fn(&Connection, &str, &str) -> Result<(), String>,
{
    match action {
        "combination_definition_list" => Ok(json!({
            "definitions": definitions::combination_definitions(),
        })),
        "combination_target_list" => {
            let action_type = payload
                .get("actionType")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or("actionType 不能为空")?;
            Ok(json!({
                "targets": atomic_actions::list_targets_with_conn(conn, action_type)?,
            }))
        }
        "combination_list" => Ok(json!({
            "combinations": combinations::list_with_conn(conn)?,
        })),
        "combination_get" => combination_detail_value(conn, parse_combination_id(payload)?),
        "combination_save" => {
            let input: combinations::CombinationSaveInput = serde_json::from_value(payload.clone())
                .map_err(|error| format!("组合动作参数无效: {error}"))?;
            let id = combinations::save_with_conn(conn, input, validate_target)?;
            Ok(json!({ "id": id }))
        }
        "combination_delete" => {
            combinations::delete_with_conn(conn, parse_combination_id(payload)?)?;
            Ok(json!({ "deleted": true }))
        }
        "combination_run" => Err("action_center combination_run requires app context".into()),
        "combination_run_get" => serde_json::to_value(combination_runs::get_run_with_conn(
            conn,
            parse_combination_run_id(payload)?,
        )?)
        .map_err(|error| format!("serialize combination run failed: {error}")),
        "combination_run_list" => Ok(json!({
            "runs": combination_runs::list_runs_with_conn(
                conn,
                parse_combination_id(payload)?,
            )?,
        })),
        _ => Err(format!("unsupported combination action: {action}")),
    }
}

pub(crate) fn recover_interrupted_combination_runs() -> Result<usize, String> {
    let conn = super::helpers::db_conn()?;
    combination_runs::recover_interrupted_with_conn(&conn)
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported action_center action: {action}"));
    }
    if action == "combination_target_list" {
        let action_type = payload
            .get("actionType")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or("actionType 不能为空")?;
        return Ok(json!({
            "targets": atomic_actions::list_targets(action_type)?,
        }));
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
        action if action.starts_with("combination_") => {
            let mut conn = super::helpers::db_conn()?;
            execute_combination_with_conn(
                action,
                payload,
                &mut conn,
                |validation_conn, action_type, target_id| {
                    atomic_actions::validate_target_with_conn(
                        validation_conn,
                        action_type,
                        target_id,
                    )
                    .map(|_| ())
                },
            )
        }
        _ => unreachable!("action center action whitelist and dispatcher must stay in sync"),
    }
}

pub fn execute_with_app(
    action: &str,
    payload: &Value,
    app: &tauri::AppHandle,
) -> Result<Value, String> {
    match action {
        "dispatch" => dispatches::dispatch_with_app(app, payload),
        "combination_run" => serde_json::to_value(combination_runs::start_with_app(
            app,
            parse_combination_id(payload)?,
        )?)
        .map_err(|error| format!("serialize started combination run failed: {error}")),
        _ => execute(action, payload),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::action_center::{
        atomic_actions::AtomicTargetSnapshot, combination_executor::RunTerminalStatus,
        combinations::ExecutionMode,
    };
    use rusqlite::Connection;

    fn combination_test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE hosts_profiles (
                 id INTEGER PRIMARY KEY,
                 name TEXT NOT NULL,
                 content TEXT NOT NULL,
                 enabled INTEGER NOT NULL DEFAULT 0,
                 sort_order INTEGER NOT NULL DEFAULT 0
             );",
        )
        .unwrap();
        ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO hosts_profiles(id, name, content, enabled, sort_order)
             VALUES (1, '开发 Hosts', '127.0.0.1 dev.local', 1, 0)",
            [],
        )
        .unwrap();
        conn
    }

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

    #[test]
    fn combination_run_rejects_missing_positive_id() {
        for payload in [
            json!({}),
            json!({ "combinationId": 0 }),
            json!({ "combinationId": -1 }),
            json!({ "combinationId": "1" }),
        ] {
            assert!(parse_combination_id(&payload).is_err());
        }
        assert_eq!(
            parse_combination_id(&json!({ "combinationId": 7 })).unwrap(),
            7
        );
    }

    #[test]
    fn combination_run_id_rejects_missing_or_blank_value() {
        for payload in [json!({}), json!({ "runId": "" }), json!({ "runId": "  " })] {
            assert!(parse_combination_run_id(&payload).is_err());
        }
        assert_eq!(
            parse_combination_run_id(&json!({ "runId": " run-7 " })).unwrap(),
            "run-7"
        );
    }

    #[test]
    fn combination_actions_are_in_supported_action_contract() {
        for action in [
            "combination_definition_list",
            "combination_target_list",
            "combination_list",
            "combination_get",
            "combination_save",
            "combination_delete",
            "combination_run",
            "combination_run_get",
            "combination_run_list",
        ] {
            assert!(supported_actions().contains(&action));
        }
    }

    #[test]
    fn combination_crud_dispatches_with_current_target_state() {
        let mut conn = combination_test_conn();
        let saved = execute_combination_with_conn(
            "combination_save",
            &json!({
                "name": "开发环境",
                "executionMode": "serial",
                "steps": [{
                    "actionType": "hosts.activate",
                    "targetId": "1",
                }],
            }),
            &mut conn,
            |_, _, _| Ok(()),
        )
        .unwrap();
        let combination_id = saved["id"].as_i64().unwrap();

        let listed =
            execute_combination_with_conn("combination_list", &json!({}), &mut conn, |_, _, _| {
                Ok(())
            })
            .unwrap();
        assert_eq!(listed["combinations"][0]["id"], combination_id);

        let detail = execute_combination_with_conn(
            "combination_get",
            &json!({ "combinationId": combination_id }),
            &mut conn,
            |_, _, _| Ok(()),
        )
        .unwrap();
        assert_eq!(detail["steps"][0]["targetLabel"], "开发 Hosts");
        assert_eq!(detail["steps"][0]["available"], true);
        assert!(detail["steps"][0].get("unavailableReason").is_none());

        conn.execute("DELETE FROM hosts_profiles WHERE id=1", [])
            .unwrap();
        let unavailable = execute_combination_with_conn(
            "combination_get",
            &json!({ "combinationId": combination_id }),
            &mut conn,
            |_, _, _| Ok(()),
        )
        .unwrap();
        assert_eq!(unavailable["steps"][0]["targetLabel"], "1");
        assert_eq!(unavailable["steps"][0]["available"], false);
        assert!(unavailable["steps"][0]["unavailableReason"]
            .as_str()
            .unwrap()
            .contains("不存在"));

        execute_combination_with_conn(
            "combination_delete",
            &json!({ "combinationId": combination_id }),
            &mut conn,
            |_, _, _| Ok(()),
        )
        .unwrap();
        assert!(execute_combination_with_conn(
            "combination_get",
            &json!({ "combinationId": combination_id }),
            &mut conn,
            |_, _, _| Ok(()),
        )
        .is_err());
    }

    #[test]
    fn combination_run_detail_and_history_dispatch_without_executing_actions() {
        let mut conn = combination_test_conn();
        let combination_id = combinations::save_with_conn(
            &mut conn,
            combinations::CombinationSaveInput {
                id: None,
                name: "运行历史".into(),
                execution_mode: ExecutionMode::Serial,
                steps: vec![combinations::CombinationStepInput {
                    action_type: "hosts.activate".into(),
                    target_id: "1".into(),
                }],
            },
            |_, _, _| Ok(()),
        )
        .unwrap();
        let run = combination_runs::create_run_with_conn(
            &mut conn,
            combination_id,
            |action_type, target_id| AtomicTargetSnapshot {
                action_label: action_type.into(),
                target_label: target_id.into(),
                validation_error: None,
            },
        )
        .unwrap();
        combination_runs::persist_step_finished_with_conn(
            &conn,
            &combination_executor::ExecutedStep {
                run_step_id: run.steps[0].id,
                sort_order: 0,
                status: combination_executor::StepTerminalStatus::Succeeded,
                result_code: Some("test".into()),
                message: None,
            },
        )
        .unwrap();
        combination_runs::finish_run_with_conn(&conn, &run.id, RunTerminalStatus::Succeeded)
            .unwrap();

        let detail = execute_combination_with_conn(
            "combination_run_get",
            &json!({ "runId": run.id }),
            &mut conn,
            |_, _, _| Ok(()),
        )
        .unwrap();
        assert_eq!(detail["status"], "succeeded");

        let history = execute_combination_with_conn(
            "combination_run_list",
            &json!({ "combinationId": combination_id }),
            &mut conn,
            |_, _, _| Ok(()),
        )
        .unwrap();
        assert_eq!(history["runs"].as_array().unwrap().len(), 1);
        assert_eq!(history["runs"][0]["id"], detail["id"]);
    }
}

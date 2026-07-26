use std::collections::HashSet;
#[cfg(test)]
use std::{
    sync::Mutex,
    thread::{self, ThreadId},
};

use rusqlite::{params, Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};

const COMBINATION_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS action_combinations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    execution_mode TEXT NOT NULL CHECK(execution_mode IN ('serial','parallel')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS action_combination_steps (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    combination_id INTEGER NOT NULL REFERENCES action_combinations(id) ON DELETE CASCADE,
    action_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(combination_id, sort_order),
    UNIQUE(combination_id, action_type, target_id)
);
CREATE TABLE IF NOT EXISTS action_combination_runs (
    id TEXT PRIMARY KEY,
    combination_id INTEGER NULL REFERENCES action_combinations(id) ON DELETE SET NULL,
    combination_name TEXT NOT NULL,
    execution_mode TEXT NOT NULL CHECK(execution_mode IN ('serial','parallel')),
    status TEXT NOT NULL CHECK(status IN ('pending','running','succeeded','partially_succeeded','failed')),
    result_code TEXT NULL,
    error TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    started_at TEXT NULL,
    finished_at TEXT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_action_combination_runs_one_active
ON action_combination_runs((1))
WHERE status IN ('pending','running');
CREATE TABLE IF NOT EXISTS action_combination_run_steps (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL REFERENCES action_combination_runs(id) ON DELETE CASCADE,
    source_step_id INTEGER NULL,
    action_type TEXT NOT NULL,
    action_label TEXT NOT NULL,
    target_id TEXT NOT NULL,
    target_label TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('pending','running','succeeded','already_satisfied','failed')),
    result_code TEXT NULL,
    message TEXT NULL,
    started_at TEXT NULL,
    finished_at TEXT NULL,
    UNIQUE(run_id, sort_order)
);
"#;

pub(super) fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(COMBINATION_SCHEMA_SQL)
        .map_err(|error| format!("create action combination schema failed: {error}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExecutionMode {
    Serial,
    Parallel,
}

impl ExecutionMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Serial => "serial",
            Self::Parallel => "parallel",
        }
    }

    fn from_db(value: &str) -> Result<Self, String> {
        match value {
            "serial" => Ok(Self::Serial),
            "parallel" => Ok(Self::Parallel),
            _ => Err(format!(
                "invalid combination execution mode in database: {value}"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CombinationStepInput {
    pub(crate) action_type: String,
    pub(crate) target_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CombinationSaveInput {
    pub(crate) id: Option<i64>,
    pub(crate) name: String,
    pub(crate) execution_mode: ExecutionMode,
    pub(crate) steps: Vec<CombinationStepInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CombinationSummary {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) execution_mode: ExecutionMode,
    pub(crate) step_count: i64,
    pub(crate) latest_run_status: Option<String>,
    pub(crate) updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CombinationStep {
    pub(crate) id: i64,
    pub(crate) action_type: String,
    pub(crate) target_id: String,
    pub(crate) sort_order: i64,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CombinationDetail {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) execution_mode: ExecutionMode,
    pub(crate) steps: Vec<CombinationStep>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

struct NormalizedCombinationSaveInput {
    id: Option<i64>,
    name: String,
    execution_mode: ExecutionMode,
    steps: Vec<CombinationStepInput>,
}

fn normalize_input<F>(
    input: CombinationSaveInput,
    validate_target: F,
) -> Result<NormalizedCombinationSaveInput, String>
where
    F: Fn(&str, &str) -> Result<(), String>,
{
    let name = input.name.trim();
    if name.is_empty() {
        return Err("combination name cannot be empty".into());
    }
    if input.steps.is_empty() {
        return Err("combination steps cannot be empty".into());
    }

    let mut unique_targets = HashSet::with_capacity(input.steps.len());
    let mut steps = Vec::with_capacity(input.steps.len());
    for step in input.steps {
        let action_type = step.action_type.trim();
        let target_id = step.target_id.trim();
        if action_type.is_empty() {
            return Err("combination action type cannot be empty".into());
        }
        if target_id.is_empty() {
            return Err("combination target id cannot be empty".into());
        }
        validate_target(action_type, target_id)?;
        if !unique_targets.insert((action_type.to_owned(), target_id.to_owned())) {
            return Err(format!(
                "duplicate combination target: {action_type}/{target_id}"
            ));
        }
        steps.push(CombinationStepInput {
            action_type: action_type.to_owned(),
            target_id: target_id.to_owned(),
        });
    }

    Ok(NormalizedCombinationSaveInput {
        id: input.id,
        name: name.to_owned(),
        execution_mode: input.execution_mode,
        steps,
    })
}

pub(crate) fn save_with_conn<F>(
    conn: &mut Connection,
    input: CombinationSaveInput,
    validate_target: F,
) -> Result<i64, String>
where
    F: Fn(&str, &str) -> Result<(), String>,
{
    let input = normalize_input(input, validate_target)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("begin combination save transaction failed: {error}"))?;

    let combination_id = match input.id {
        Some(id) => {
            let changed = tx
                .execute(
                    "UPDATE action_combinations
                     SET name=?1, execution_mode=?2, updated_at=CURRENT_TIMESTAMP
                     WHERE id=?3",
                    params![input.name, input.execution_mode.as_str(), id],
                )
                .map_err(|error| format!("update combination failed: {error}"))?;
            if changed == 0 {
                return Err(format!("combination not found: {id}"));
            }
            tx.execute(
                "DELETE FROM action_combination_steps WHERE combination_id=?1",
                [id],
            )
            .map_err(|error| format!("replace combination steps failed: {error}"))?;
            id
        }
        None => {
            tx.execute(
                "INSERT INTO action_combinations(name, execution_mode)
                 VALUES (?1, ?2)",
                params![input.name, input.execution_mode.as_str()],
            )
            .map_err(|error| format!("create combination failed: {error}"))?;
            tx.last_insert_rowid()
        }
    };

    for (sort_order, step) in input.steps.into_iter().enumerate() {
        let sort_order =
            i64::try_from(sort_order).map_err(|_| "combination has too many steps".to_string())?;
        tx.execute(
            "INSERT INTO action_combination_steps
             (combination_id, action_type, target_id, sort_order)
             VALUES (?1, ?2, ?3, ?4)",
            params![combination_id, step.action_type, step.target_id, sort_order],
        )
        .map_err(|error| format!("save combination step failed: {error}"))?;
    }

    #[cfg(test)]
    run_thread_hook(&SAVE_BEFORE_COMMIT_HOOK);
    tx.commit()
        .map_err(|error| format!("commit combination save failed: {error}"))?;
    Ok(combination_id)
}

pub(crate) fn list_with_conn(conn: &Connection) -> Result<Vec<CombinationSummary>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT c.id,
                    c.name,
                    c.execution_mode,
                    (SELECT COUNT(*)
                     FROM action_combination_steps s
                     WHERE s.combination_id=c.id) AS step_count,
                    (SELECT r.status
                     FROM action_combination_runs r
                     WHERE r.combination_id=c.id
                     ORDER BY r.created_at DESC, r.rowid DESC
                     LIMIT 1) AS latest_run_status,
                    c.updated_at
             FROM action_combinations c
             ORDER BY c.updated_at DESC, c.id DESC",
        )
        .map_err(|error| format!("prepare combination list failed: {error}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .map_err(|error| format!("query combinations failed: {error}"))?;

    let mut summaries = Vec::new();
    for row in rows {
        let (id, name, mode, step_count, latest_run_status, updated_at) =
            row.map_err(|error| format!("read combination summary failed: {error}"))?;
        summaries.push(CombinationSummary {
            id,
            name,
            execution_mode: ExecutionMode::from_db(&mode)?,
            step_count,
            latest_run_status,
            updated_at,
        });
    }
    Ok(summaries)
}

pub(crate) fn get_with_conn(conn: &Connection, id: i64) -> Result<CombinationDetail, String> {
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.name, c.execution_mode, c.created_at, c.updated_at,
                    s.id, s.action_type, s.target_id, s.sort_order, s.created_at, s.updated_at
             FROM action_combinations c
             LEFT JOIN action_combination_steps s ON s.combination_id=c.id
             WHERE c.id=?1
             ORDER BY s.sort_order ASC",
        )
        .map_err(|error| format!("prepare combination detail failed: {error}"))?;
    let mut rows = stmt
        .query([id])
        .map_err(|error| format!("query combination detail failed: {error}"))?;
    let first = rows
        .next()
        .map_err(|error| format!("read combination detail failed: {error}"))?
        .ok_or_else(|| format!("combination not found: {id}"))?;
    #[cfg(test)]
    run_thread_hook(&DETAIL_READ_HOOK);

    let combination_id = first
        .get(0)
        .map_err(|error| format!("read combination id failed: {error}"))?;
    let name = first
        .get(1)
        .map_err(|error| format!("read combination name failed: {error}"))?;
    let mode: String = first
        .get(2)
        .map_err(|error| format!("read combination execution mode failed: {error}"))?;
    let created_at = first
        .get(3)
        .map_err(|error| format!("read combination created time failed: {error}"))?;
    let updated_at = first
        .get(4)
        .map_err(|error| format!("read combination updated time failed: {error}"))?;
    let mut steps = Vec::new();
    if let Some(step) = combination_step_from_joined_row(first)
        .map_err(|error| format!("read combination step failed: {error}"))?
    {
        steps.push(step);
    }
    while let Some(row) = rows
        .next()
        .map_err(|error| format!("read combination detail failed: {error}"))?
    {
        if let Some(step) = combination_step_from_joined_row(row)
            .map_err(|error| format!("read combination step failed: {error}"))?
        {
            steps.push(step);
        }
    }

    Ok(CombinationDetail {
        id: combination_id,
        name,
        execution_mode: ExecutionMode::from_db(&mode)?,
        steps,
        created_at,
        updated_at,
    })
}

#[cfg(test)]
type ThreadHook = (ThreadId, Box<dyn FnOnce() + Send>);

#[cfg(test)]
static DETAIL_READ_HOOK: Mutex<Option<ThreadHook>> = Mutex::new(None);

#[cfg(test)]
static DELETE_BEFORE_WRITE_HOOK: Mutex<Option<ThreadHook>> = Mutex::new(None);

#[cfg(test)]
static SAVE_BEFORE_COMMIT_HOOK: Mutex<Option<ThreadHook>> = Mutex::new(None);

#[cfg(test)]
fn install_thread_hook(
    hook_slot: &Mutex<Option<ThreadHook>>,
    thread_id: ThreadId,
    hook: impl FnOnce() + Send + 'static,
) {
    let mut slot = hook_slot.lock().unwrap();
    assert!(slot.is_none(), "thread hook already installed");
    *slot = Some((thread_id, Box::new(hook)));
}

#[cfg(test)]
fn run_thread_hook(hook_slot: &Mutex<Option<ThreadHook>>) {
    let hook = {
        let mut slot = hook_slot.lock().unwrap();
        match slot.as_ref() {
            Some((thread_id, _)) if *thread_id == thread::current().id() => {
                slot.take().map(|(_, hook)| hook)
            }
            _ => None,
        }
    };
    if let Some(hook) = hook {
        hook();
    }
}

fn combination_step_from_joined_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<Option<CombinationStep>> {
    let Some(id) = row.get::<_, Option<i64>>(5)? else {
        return Ok(None);
    };
    Ok(Some(CombinationStep {
        id,
        action_type: row.get(6)?,
        target_id: row.get(7)?,
        sort_order: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    }))
}

pub(crate) fn delete_with_conn(conn: &Connection, id: i64) -> Result<(), String> {
    #[cfg(test)]
    run_thread_hook(&DELETE_BEFORE_WRITE_HOOK);
    let changed = conn
        .execute(
            "DELETE FROM action_combinations
             WHERE id=?1
               AND NOT EXISTS (
                   SELECT 1
                   FROM action_combination_runs
                   WHERE combination_id=?1 AND status IN ('pending','running')
               )",
            [id],
        )
        .map_err(|error| format!("delete combination failed: {error}"))?;
    if changed == 1 {
        return Ok(());
    }

    let exists = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM action_combinations WHERE id=?1)",
            [id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|error| format!("check combination after delete failed: {error}"))?;
    if exists {
        Err(format!("combination has an active run: {id}"))
    } else {
        Err(format!("combination not found: {id}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{Connection, OptionalExtension};
    use serde_json::json;
    use std::{
        ffi::OsString,
        fs,
        path::{Path, PathBuf},
        sync::mpsc,
        thread,
    };
    use uuid::Uuid;

    struct TempDatabase {
        path: PathBuf,
    }

    impl TempDatabase {
        fn new() -> (Self, Connection) {
            let path = std::env::temp_dir().join(format!(
                "lazycat-action-combinations-{}.sqlite",
                Uuid::new_v4()
            ));
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA busy_timeout = 5000;",
            )
            .unwrap();
            super::super::ensure_schema(&conn).unwrap();
            (Self { path }, conn)
        }

        fn connect(&self) -> Connection {
            let conn = Connection::open(&self.path).unwrap();
            conn.execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA busy_timeout = 5000;",
            )
            .unwrap();
            conn
        }
    }

    impl Drop for TempDatabase {
        fn drop(&mut self) {
            for path in [
                self.path.clone(),
                sidecar_path(&self.path, "-wal"),
                sidecar_path(&self.path, "-shm"),
            ] {
                match fs::remove_file(&path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        panic!("remove temporary sqlite file {}: {error}", path.display())
                    }
                }
            }
        }
    }

    fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
        let mut value = OsString::from(path.as_os_str());
        value.push(suffix);
        PathBuf::from(value)
    }

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        super::super::ensure_schema(&conn).unwrap();
        conn
    }

    fn input(
        id: Option<i64>,
        name: &str,
        execution_mode: ExecutionMode,
        steps: &[(&str, &str)],
    ) -> CombinationSaveInput {
        CombinationSaveInput {
            id,
            name: name.into(),
            execution_mode,
            steps: steps
                .iter()
                .map(|(action_type, target_id)| CombinationStepInput {
                    action_type: (*action_type).into(),
                    target_id: (*target_id).into(),
                })
                .collect(),
        }
    }

    fn validate_known_target(action_type: &str, target_id: &str) -> Result<(), String> {
        match (action_type, target_id) {
            ("release_package", "1" | "2") | ("browser_profile", "work") => Ok(()),
            _ => Err(format!("unknown target: {action_type}/{target_id}")),
        }
    }

    fn save(conn: &mut Connection, input: CombinationSaveInput) -> Result<i64, String> {
        save_with_conn(conn, input, validate_known_target)
    }

    #[test]
    fn serde_uses_expected_field_and_enum_casing() {
        let value = serde_json::to_value(input(
            None,
            "release",
            ExecutionMode::Parallel,
            &[("release_package", "1")],
        ))
        .unwrap();
        assert_eq!(
            value,
            json!({
                "id": null,
                "name": "release",
                "executionMode": "parallel",
                "steps": [{
                    "actionType": "release_package",
                    "targetId": "1",
                }],
            })
        );
    }

    #[test]
    fn creates_and_gets_steps_in_input_order() {
        let mut conn = test_conn();
        let id = save(
            &mut conn,
            input(
                None,
                "  weekday release  ",
                ExecutionMode::Serial,
                &[(" browser_profile ", " work "), ("release_package", "1")],
            ),
        )
        .unwrap();

        let detail = get_with_conn(&conn, id).unwrap();
        assert_eq!(detail.name, "weekday release");
        assert_eq!(detail.execution_mode, ExecutionMode::Serial);
        assert_eq!(detail.steps.len(), 2);
        assert_eq!(detail.steps[0].action_type, "browser_profile");
        assert_eq!(detail.steps[0].target_id, "work");
        assert_eq!(detail.steps[0].sort_order, 0);
        assert_eq!(detail.steps[1].action_type, "release_package");
        assert_eq!(detail.steps[1].target_id, "1");
        assert_eq!(detail.steps[1].sort_order, 1);
    }

    #[test]
    fn update_replaces_steps_and_execution_mode() {
        let mut conn = test_conn();
        let id = save(
            &mut conn,
            input(
                None,
                "old",
                ExecutionMode::Serial,
                &[("release_package", "1"), ("release_package", "2")],
            ),
        )
        .unwrap();

        let returned_id = save(
            &mut conn,
            input(
                Some(id),
                "new",
                ExecutionMode::Parallel,
                &[("browser_profile", "work")],
            ),
        )
        .unwrap();

        assert_eq!(returned_id, id);
        let detail = get_with_conn(&conn, id).unwrap();
        assert_eq!(detail.name, "new");
        assert_eq!(detail.execution_mode, ExecutionMode::Parallel);
        assert_eq!(detail.steps.len(), 1);
        assert_eq!(detail.steps[0].action_type, "browser_profile");
        assert_eq!(detail.steps[0].target_id, "work");
    }

    #[test]
    fn rejects_empty_name_and_steps_without_writing() {
        for invalid in [
            input(
                None,
                "   ",
                ExecutionMode::Serial,
                &[("release_package", "1")],
            ),
            input(None, "empty", ExecutionMode::Serial, &[]),
            input(None, "blank action", ExecutionMode::Serial, &[(" ", "1")]),
            input(
                None,
                "blank target",
                ExecutionMode::Serial,
                &[("release_package", " ")],
            ),
        ] {
            let mut conn = test_conn();
            assert!(save(&mut conn, invalid).is_err());
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM action_combinations", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
    }

    #[test]
    fn rejects_unknown_and_duplicate_targets_without_partial_write() {
        let mut conn = test_conn();
        let unknown = input(
            None,
            "unknown",
            ExecutionMode::Serial,
            &[("release_package", "1"), ("release_package", "404")],
        );
        assert!(save(&mut conn, unknown)
            .unwrap_err()
            .contains("unknown target"));

        let duplicate = input(
            None,
            "duplicate",
            ExecutionMode::Serial,
            &[(" release_package ", "1"), ("release_package", " 1 ")],
        );
        assert!(save(&mut conn, duplicate)
            .unwrap_err()
            .contains("duplicate"));
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM action_combinations", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn rejected_update_preserves_existing_configuration() {
        let mut conn = test_conn();
        let id = save(
            &mut conn,
            input(
                None,
                "stable",
                ExecutionMode::Serial,
                &[("release_package", "1")],
            ),
        )
        .unwrap();

        let error = save(
            &mut conn,
            input(
                Some(id),
                "invalid",
                ExecutionMode::Parallel,
                &[("release_package", "404")],
            ),
        )
        .unwrap_err();

        assert!(error.contains("unknown target"));
        let detail = get_with_conn(&conn, id).unwrap();
        assert_eq!(detail.name, "stable");
        assert_eq!(detail.execution_mode, ExecutionMode::Serial);
        assert_eq!(detail.steps[0].target_id, "1");
    }

    #[test]
    fn step_insert_failure_rolls_back_combination_and_replaced_steps() {
        let mut conn = test_conn();
        let id = save(
            &mut conn,
            input(
                None,
                "stable",
                ExecutionMode::Serial,
                &[("release_package", "1")],
            ),
        )
        .unwrap();
        conn.execute_batch(
            "CREATE TRIGGER reject_second_target
             BEFORE INSERT ON action_combination_steps
             WHEN NEW.target_id='2'
             BEGIN
                 SELECT RAISE(ABORT, 'injected step failure');
             END;",
        )
        .unwrap();

        let error = save(
            &mut conn,
            input(
                Some(id),
                "must roll back",
                ExecutionMode::Parallel,
                &[("browser_profile", "work"), ("release_package", "2")],
            ),
        )
        .unwrap_err();

        assert!(error.contains("injected step failure"));
        let detail = get_with_conn(&conn, id).unwrap();
        assert_eq!(detail.name, "stable");
        assert_eq!(detail.execution_mode, ExecutionMode::Serial);
        assert_eq!(detail.steps.len(), 1);
        assert_eq!(detail.steps[0].target_id, "1");
    }

    #[test]
    fn missing_update_get_and_delete_are_explicit_errors() {
        let mut conn = test_conn();
        assert!(save(
            &mut conn,
            input(
                Some(999),
                "missing",
                ExecutionMode::Serial,
                &[("release_package", "1")],
            ),
        )
        .unwrap_err()
        .contains("not found"));
        assert!(get_with_conn(&conn, 999).unwrap_err().contains("not found"));
        assert!(delete_with_conn(&conn, 999)
            .unwrap_err()
            .contains("not found"));
    }

    #[test]
    fn delete_cascades_editable_steps_and_preserves_run_snapshot() {
        let mut conn = test_conn();
        let id = save(
            &mut conn,
            input(
                None,
                "snapshot source",
                ExecutionMode::Serial,
                &[("release_package", "1")],
            ),
        )
        .unwrap();
        let source_step_id: i64 = conn
            .query_row(
                "SELECT id FROM action_combination_steps WHERE combination_id=?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO action_combination_runs
             (id, combination_id, combination_name, execution_mode, status)
             VALUES ('run-finished', ?1, 'snapshot source', 'serial', 'succeeded')",
            [id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO action_combination_run_steps
             (run_id, source_step_id, action_type, action_label, target_id, target_label,
              sort_order, status, result_code)
             VALUES ('run-finished', ?1, 'release_package', 'Release package', '1',
                     'Project one', 0, 'succeeded', 'started')",
            [source_step_id],
        )
        .unwrap();

        delete_with_conn(&conn, id).unwrap();

        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM action_combination_steps WHERE combination_id=?1",
                [id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            0
        );
        let snapshot: (Option<i64>, String, String) = conn
            .query_row(
                "SELECT combination_id, combination_name, execution_mode
                 FROM action_combination_runs WHERE id='run-finished'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(snapshot, (None, "snapshot source".into(), "serial".into()));
        let retained_step: (Option<i64>, String, String) = conn
            .query_row(
                "SELECT source_step_id, action_label, target_label
                 FROM action_combination_run_steps
                 WHERE run_id='run-finished'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            retained_step,
            (
                Some(source_step_id),
                "Release package".into(),
                "Project one".into()
            )
        );
    }

    #[test]
    fn delete_rejects_combination_with_active_run() {
        let mut conn = test_conn();
        let id = save(
            &mut conn,
            input(
                None,
                "active",
                ExecutionMode::Serial,
                &[("release_package", "1")],
            ),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO action_combination_runs
             (id, combination_id, combination_name, execution_mode, status)
             VALUES ('run-active', ?1, 'active', 'serial', 'running')",
            [id],
        )
        .unwrap();

        assert!(delete_with_conn(&conn, id)
            .unwrap_err()
            .contains("active run"));
        assert!(get_with_conn(&conn, id).is_ok());
    }

    #[test]
    fn concurrent_active_run_commit_prevents_delete() {
        let (database, mut setup_conn) = TempDatabase::new();
        let id = save(
            &mut setup_conn,
            input(
                None,
                "concurrent delete",
                ExecutionMode::Serial,
                &[("release_package", "1")],
            ),
        )
        .unwrap();
        drop(setup_conn);

        let run_conn = database.connect();
        let delete_conn = database.connect();
        let (delete_start_tx, delete_start_rx) = mpsc::sync_channel(0);
        let (before_delete_tx, before_delete_rx) = mpsc::sync_channel(0);
        let (continue_delete_tx, continue_delete_rx) = mpsc::sync_channel(0);
        let (delete_result_tx, delete_result_rx) = mpsc::sync_channel(0);
        let delete_thread = thread::spawn(move || {
            delete_start_rx.recv().unwrap();
            delete_result_tx
                .send(delete_with_conn(&delete_conn, id))
                .unwrap();
        });
        install_thread_hook(
            &DELETE_BEFORE_WRITE_HOOK,
            delete_thread.thread().id(),
            move || {
                before_delete_tx.send(()).unwrap();
                continue_delete_rx.recv().unwrap();
            },
        );
        delete_start_tx.send(()).unwrap();
        before_delete_rx.recv().unwrap();

        run_conn
            .execute(
                "INSERT INTO action_combination_runs
                 (id, combination_id, combination_name, execution_mode, status)
                 VALUES ('concurrent-run', ?1, 'concurrent delete', 'serial', 'pending')",
                [id],
            )
            .unwrap();
        continue_delete_tx.send(()).unwrap();
        let delete_error = delete_result_rx.recv().unwrap().unwrap_err();
        delete_thread.join().unwrap();

        assert!(delete_error.contains("active run"));
        let verify_conn = database.connect();
        assert!(get_with_conn(&verify_conn, id).is_ok());
        let linked_combination_id: Option<i64> = verify_conn
            .query_row(
                "SELECT combination_id FROM action_combination_runs
                 WHERE id='concurrent-run'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(linked_combination_id, Some(id));
    }

    #[test]
    fn concurrent_replacement_cannot_tear_combination_detail() {
        let (database, mut setup_conn) = TempDatabase::new();
        let id = save(
            &mut setup_conn,
            input(
                None,
                "old",
                ExecutionMode::Serial,
                &[("release_package", "1"), ("release_package", "2")],
            ),
        )
        .unwrap();
        drop(setup_conn);

        let mut writer_conn = database.connect();
        let (writer_start_tx, writer_start_rx) = mpsc::sync_channel(0);
        let (replacement_ready_tx, replacement_ready_rx) = mpsc::sync_channel(0);
        let (commit_tx, commit_rx) = mpsc::sync_channel(0);
        let writer_thread = thread::spawn(move || {
            writer_start_rx.recv().unwrap();
            save(
                &mut writer_conn,
                input(
                    Some(id),
                    "new",
                    ExecutionMode::Parallel,
                    &[("browser_profile", "work")],
                ),
            )
        });
        install_thread_hook(
            &SAVE_BEFORE_COMMIT_HOOK,
            writer_thread.thread().id(),
            move || {
                replacement_ready_tx.send(()).unwrap();
                commit_rx.recv().unwrap();
            },
        );
        writer_start_tx.send(()).unwrap();
        replacement_ready_rx.recv().unwrap();

        let reader_conn = database.connect();
        let (reader_start_tx, reader_start_rx) = mpsc::sync_channel(0);
        let (first_row_tx, first_row_rx) = mpsc::sync_channel(0);
        let (continue_tx, continue_rx) = mpsc::sync_channel(0);
        let reader_thread = thread::spawn(move || {
            reader_start_rx.recv().unwrap();
            get_with_conn(&reader_conn, id)
        });
        install_thread_hook(&DETAIL_READ_HOOK, reader_thread.thread().id(), move || {
            first_row_tx.send(()).unwrap();
            continue_rx.recv().unwrap();
        });
        reader_start_tx.send(()).unwrap();
        first_row_rx.recv().unwrap();

        commit_tx.send(()).unwrap();
        assert_eq!(writer_thread.join().unwrap().unwrap(), id);
        continue_tx.send(()).unwrap();
        let during_commit = reader_thread.join().unwrap().unwrap();
        assert_eq!(during_commit.name, "old");
        assert_eq!(during_commit.execution_mode, ExecutionMode::Serial);
        assert_eq!(
            during_commit
                .steps
                .iter()
                .map(|step| step.target_id.as_str())
                .collect::<Vec<_>>(),
            ["1", "2"]
        );

        let after_commit = get_with_conn(&database.connect(), id).unwrap();
        assert_eq!(after_commit.name, "new");
        assert_eq!(after_commit.execution_mode, ExecutionMode::Parallel);
        assert_eq!(after_commit.steps.len(), 1);
        assert_eq!(after_commit.steps[0].target_id, "work");
    }

    #[test]
    fn only_one_pending_or_running_combination_run_is_allowed_globally() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO action_combination_runs
             (id, combination_name, execution_mode, status)
             VALUES ('run-1', 'first', 'serial', 'pending')",
            [],
        )
        .unwrap();
        assert!(conn
            .execute(
                "INSERT INTO action_combination_runs
                 (id, combination_name, execution_mode, status)
                 VALUES ('run-2', 'second', 'parallel', 'running')",
                [],
            )
            .is_err());
        conn.execute(
            "UPDATE action_combination_runs SET status='succeeded' WHERE id='run-1'",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO action_combination_runs
             (id, combination_name, execution_mode, status)
             VALUES ('run-2', 'second', 'parallel', 'running')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn run_step_schema_uses_result_code_snapshot_column() {
        let conn = test_conn();
        let mut stmt = conn
            .prepare("PRAGMA table_info(action_combination_run_steps)")
            .unwrap();
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(columns.iter().any(|column| column == "result_code"));
        assert!(!columns.iter().any(|column| column == "result"));
    }

    #[test]
    fn list_includes_step_count_and_latest_run_status() {
        let mut conn = test_conn();
        let first = save(
            &mut conn,
            input(
                None,
                "first",
                ExecutionMode::Serial,
                &[("release_package", "1"), ("release_package", "2")],
            ),
        )
        .unwrap();
        let second = save(
            &mut conn,
            input(
                None,
                "second",
                ExecutionMode::Parallel,
                &[("browser_profile", "work")],
            ),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO action_combination_runs
             (id, combination_id, combination_name, execution_mode, status, created_at)
             VALUES ('run-old', ?1, 'first', 'serial', 'succeeded', '2026-01-01 00:00:00')",
            [first],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO action_combination_runs
             (id, combination_id, combination_name, execution_mode, status, created_at)
             VALUES ('run-latest', ?1, 'first', 'serial', 'failed', '2026-01-02 00:00:00')",
            [first],
        )
        .unwrap();

        let summaries = list_with_conn(&conn).unwrap();
        let first_summary = summaries.iter().find(|item| item.id == first).unwrap();
        assert_eq!(first_summary.step_count, 2);
        assert_eq!(first_summary.latest_run_status.as_deref(), Some("failed"));
        let second_summary = summaries.iter().find(|item| item.id == second).unwrap();
        assert_eq!(second_summary.step_count, 1);
        assert_eq!(second_summary.latest_run_status, None);
    }

    #[test]
    fn list_ignores_snapshots_from_deleted_combinations() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO action_combination_runs
             (id, combination_name, execution_mode, status)
             VALUES ('detached', 'deleted', 'serial', 'failed')",
            [],
        )
        .unwrap();
        assert!(list_with_conn(&conn).unwrap().is_empty());
        let detached: Option<i64> = conn
            .query_row(
                "SELECT combination_id FROM action_combination_runs WHERE id='detached'",
                [],
                |row| row.get(0),
            )
            .optional()
            .unwrap()
            .flatten();
        assert_eq!(detached, None);
    }
}

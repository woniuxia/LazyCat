use super::{
    atomic_actions::{
        snapshot_target, AtomicActionExecutor, AtomicTargetSnapshot, RegisteredAtomicActionExecutor,
    },
    combination_executor::{
        aggregate_status, execute_plan_with_observer, ExecutedStep, ExecutionObserver, PlannedStep,
        RunTerminalStatus, StepTerminalStatus,
    },
    combinations::ExecutionMode,
};
use crate::events::EVENT_ACTION_CENTER_COMBINATION_RUN_UPDATED;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use std::{
    any::Any,
    io,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Arc, Mutex, OnceLock},
    thread,
};
use tauri::Emitter;
use uuid::Uuid;

const INTERRUPTED_ERROR: &str = "组合动作运行因应用中断而失败";
const INTERRUPTED_STEP_MESSAGE: &str = "组合动作步骤因应用中断而失败";

type ConnectionFactory = Arc<dyn Fn() -> Result<Connection, String> + Send + Sync>;
type EventEmitter = Arc<dyn Fn(&str, &str) -> Result<(), String> + Send + Sync>;
type BackgroundTask = Box<dyn FnOnce() + Send + 'static>;

static ACTIVE_RUN_SLOT: OnceLock<Mutex<Option<ActiveRunSlot>>> = OnceLock::new();

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CombinationRunDetail {
    pub(crate) id: String,
    pub(crate) combination_id: Option<i64>,
    pub(crate) combination_name: String,
    pub(crate) execution_mode: ExecutionMode,
    pub(crate) status: String,
    pub(crate) result_code: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) created_at: String,
    pub(crate) started_at: Option<String>,
    pub(crate) finished_at: Option<String>,
    pub(crate) steps: Vec<CombinationRunStep>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CombinationRunStep {
    pub(crate) id: i64,
    pub(crate) action_type: String,
    pub(crate) action_label: String,
    pub(crate) target_id: String,
    pub(crate) target_label: String,
    pub(crate) sort_order: i64,
    pub(crate) status: String,
    pub(crate) result_code: Option<String>,
    pub(crate) message: Option<String>,
    pub(crate) started_at: Option<String>,
    pub(crate) finished_at: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CombinationRunUpdatedPayload {
    run_id: String,
    status: String,
}

fn execution_mode_from_db(value: &str) -> Result<ExecutionMode, String> {
    match value {
        "serial" => Ok(ExecutionMode::Serial),
        "parallel" => Ok(ExecutionMode::Parallel),
        _ => Err(format!(
            "invalid combination execution mode in database: {value}"
        )),
    }
}

fn execution_mode_as_str(mode: ExecutionMode) -> &'static str {
    match mode {
        ExecutionMode::Serial => "serial",
        ExecutionMode::Parallel => "parallel",
    }
}

fn step_terminal_status_as_str(status: StepTerminalStatus) -> &'static str {
    match status {
        StepTerminalStatus::Succeeded => "succeeded",
        StepTerminalStatus::AlreadySatisfied => "already_satisfied",
        StepTerminalStatus::Failed => "failed",
    }
}

fn run_terminal_status_as_str(status: RunTerminalStatus) -> &'static str {
    match status {
        RunTerminalStatus::Succeeded => "succeeded",
        RunTerminalStatus::PartiallySucceeded => "partially_succeeded",
        RunTerminalStatus::Failed => "failed",
    }
}

fn active_run_error_with_conn(
    conn: &Connection,
    run_id: Option<&str>,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT combination_name, status
         FROM action_combination_runs
         WHERE status IN ('pending','running')
           AND (?1 IS NULL OR id=?1)
         ORDER BY created_at ASC, id ASC
         LIMIT 1",
        [run_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )
    .optional()
    .map(|active| {
        active.map(|(combination_name, status)| {
            format!("已有组合动作正在运行: {combination_name} ({status})")
        })
    })
    .map_err(|error| format!("read active combination run failed: {error}"))
}

#[cfg(test)]
pub(crate) fn create_run_with_conn<F>(
    conn: &mut Connection,
    combination_id: i64,
    snapshot_target: F,
) -> Result<CombinationRunDetail, String>
where
    F: Fn(&str, &str) -> AtomicTargetSnapshot,
{
    let run_id = Uuid::new_v4().to_string();
    create_run_and_plan_with_conn(conn, combination_id, &run_id, snapshot_target)
        .map(|(run, _)| run)
}

fn create_run_and_plan_with_conn<F>(
    conn: &mut Connection,
    combination_id: i64,
    run_id: &str,
    snapshot_target: F,
) -> Result<(CombinationRunDetail, Vec<PlannedStep>), String>
where
    F: Fn(&str, &str) -> AtomicTargetSnapshot,
{
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("begin combination run transaction failed: {error}"))?;
    let combination = tx
        .query_row(
            "SELECT name, execution_mode FROM action_combinations WHERE id=?1",
            [combination_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| format!("read combination for run failed: {error}"))?
        .ok_or_else(|| format!("combination not found: {combination_id}"))?;
    let execution_mode = execution_mode_from_db(&combination.1)?;

    let source_steps = {
        let mut stmt = tx
            .prepare(
                "SELECT id, action_type, target_id, sort_order
                 FROM action_combination_steps
                 WHERE combination_id=?1
                 ORDER BY sort_order ASC, id ASC",
            )
            .map_err(|error| format!("prepare combination run steps failed: {error}"))?;
        let rows = stmt
            .query_map([combination_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(|error| format!("query combination run steps failed: {error}"))?;
        let mut steps = Vec::new();
        for row in rows {
            steps.push(row.map_err(|error| format!("read combination run step failed: {error}"))?);
        }
        steps
    };
    if source_steps.is_empty() {
        return Err(format!("combination has no steps: {combination_id}"));
    }

    if let Err(error) = tx.execute(
        "INSERT INTO action_combination_runs
         (id, combination_id, combination_name, execution_mode, status, created_at)
         VALUES (?1, ?2, ?3, ?4, 'pending', STRFTIME('%Y-%m-%d %H:%M:%f', 'now'))",
        params![
            run_id,
            combination_id,
            combination.0,
            execution_mode_as_str(execution_mode)
        ],
    ) {
        if let Some(active_error) = active_run_error_with_conn(&tx, None)? {
            return Err(active_error);
        }
        return Err(format!("create combination run failed: {error}"));
    }

    let mut plan = Vec::with_capacity(source_steps.len());
    for (source_step_id, action_type, target_id, sort_order) in source_steps {
        let snapshot = snapshot_target(&action_type, &target_id);
        tx.execute(
            "INSERT INTO action_combination_run_steps
             (run_id, source_step_id, action_type, action_label, target_id, target_label,
              sort_order, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending')",
            params![
                run_id,
                source_step_id,
                action_type,
                snapshot.action_label,
                target_id,
                snapshot.target_label,
                sort_order
            ],
        )
        .map_err(|error| format!("create combination run step failed: {error}"))?;
        plan.push(PlannedStep {
            run_step_id: tx.last_insert_rowid(),
            action_type,
            target_id,
            sort_order,
            validation_error: snapshot.validation_error,
        });
    }

    let detail = get_run_with_conn(&tx, run_id)?;
    tx.commit()
        .map_err(|error| format!("commit combination run failed: {error}"))?;
    Ok((detail, plan))
}

pub(crate) fn get_run_with_conn(
    conn: &Connection,
    run_id: &str,
) -> Result<CombinationRunDetail, String> {
    let row = conn
        .query_row(
            "SELECT id, combination_id, combination_name, execution_mode, status,
                    result_code, error, created_at, started_at, finished_at
             FROM action_combination_runs
             WHERE id=?1",
            [run_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("read combination run failed: {error}"))?
        .ok_or_else(|| format!("combination run not found: {run_id}"))?;
    Ok(CombinationRunDetail {
        id: row.0,
        combination_id: row.1,
        combination_name: row.2,
        execution_mode: execution_mode_from_db(&row.3)?,
        status: row.4,
        result_code: row.5,
        error: row.6,
        created_at: row.7,
        started_at: row.8,
        finished_at: row.9,
        steps: load_run_steps(conn, run_id)?,
    })
}

fn load_run_steps(conn: &Connection, run_id: &str) -> Result<Vec<CombinationRunStep>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, action_type, action_label, target_id, target_label, sort_order,
                    status, result_code, message, started_at, finished_at
             FROM action_combination_run_steps
             WHERE run_id=?1
             ORDER BY sort_order ASC, id ASC",
        )
        .map_err(|error| format!("prepare combination run detail steps failed: {error}"))?;
    let rows = stmt
        .query_map([run_id], |row| {
            Ok(CombinationRunStep {
                id: row.get(0)?,
                action_type: row.get(1)?,
                action_label: row.get(2)?,
                target_id: row.get(3)?,
                target_label: row.get(4)?,
                sort_order: row.get(5)?,
                status: row.get(6)?,
                result_code: row.get(7)?,
                message: row.get(8)?,
                started_at: row.get(9)?,
                finished_at: row.get(10)?,
            })
        })
        .map_err(|error| format!("query combination run detail steps failed: {error}"))?;
    let mut steps = Vec::new();
    for row in rows {
        steps.push(
            row.map_err(|error| format!("read combination run detail step failed: {error}"))?,
        );
    }
    Ok(steps)
}

pub(crate) fn list_runs_with_conn(
    conn: &Connection,
    combination_id: i64,
) -> Result<Vec<CombinationRunDetail>, String> {
    let ids = {
        let mut stmt = conn
            .prepare(
                "SELECT id
                 FROM action_combination_runs
                 WHERE combination_id=?1
                 ORDER BY created_at DESC, id DESC
                 LIMIT 20",
            )
            .map_err(|error| format!("prepare combination run list failed: {error}"))?;
        let rows = stmt
            .query_map([combination_id], |row| row.get::<_, String>(0))
            .map_err(|error| format!("query combination run list failed: {error}"))?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(row.map_err(|error| format!("read combination run list failed: {error}"))?);
        }
        ids
    };
    ids.into_iter()
        .map(|run_id| get_run_with_conn(conn, &run_id))
        .collect()
}

pub(crate) fn persist_step_started_with_conn(
    conn: &Connection,
    run_step_id: i64,
) -> Result<(), String> {
    let status = step_status_with_conn(conn, run_step_id)?;
    match status.as_str() {
        "pending" => {
            conn.execute(
                "UPDATE action_combination_run_steps
                 SET status='running', started_at=CURRENT_TIMESTAMP
                 WHERE id=?1 AND status='pending'",
                [run_step_id],
            )
            .map_err(|error| format!("persist combination step start failed: {error}"))?;
            Ok(())
        }
        "running" => Ok(()),
        _ => Err(format!(
            "组合动作步骤已是终态，不能重新开始: {run_step_id}/{status}"
        )),
    }
}

pub(crate) fn persist_step_finished_with_conn(
    conn: &Connection,
    result: &ExecutedStep,
) -> Result<(), String> {
    let terminal_status = step_terminal_status_as_str(result.status);
    let existing = conn
        .query_row(
            "SELECT status, result_code, message
             FROM action_combination_run_steps WHERE id=?1",
            [result.run_step_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|error| format!("read combination step before finish failed: {error}"))?
        .ok_or_else(|| format!("combination run step not found: {}", result.run_step_id))?;
    if matches!(
        existing.0.as_str(),
        "succeeded" | "already_satisfied" | "failed"
    ) {
        if existing.0 == terminal_status
            && existing.1 == result.result_code
            && existing.2 == result.message
        {
            return Ok(());
        }
        return Err(format!(
            "组合动作步骤已是不同终态: {}/{}",
            result.run_step_id, existing.0
        ));
    }

    conn.execute(
        "UPDATE action_combination_run_steps
         SET status=?1, result_code=?2, message=?3,
             started_at=COALESCE(started_at, CURRENT_TIMESTAMP),
             finished_at=CURRENT_TIMESTAMP
         WHERE id=?4 AND status IN ('pending','running')",
        params![
            terminal_status,
            result.result_code,
            result.message,
            result.run_step_id
        ],
    )
    .map_err(|error| format!("persist combination step finish failed: {error}"))?;
    Ok(())
}

fn step_status_with_conn(conn: &Connection, run_step_id: i64) -> Result<String, String> {
    conn.query_row(
        "SELECT status FROM action_combination_run_steps WHERE id=?1",
        [run_step_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| format!("read combination run step status failed: {error}"))?
    .ok_or_else(|| format!("combination run step not found: {run_step_id}"))
}

#[cfg(test)]
pub(crate) fn finish_run_with_conn(
    conn: &Connection,
    run_id: &str,
    status: RunTerminalStatus,
) -> Result<(), String> {
    finish_run_with_result(conn, run_id, status, None, None)
}

fn finish_run_with_result(
    conn: &Connection,
    run_id: &str,
    status: RunTerminalStatus,
    result_code: Option<&str>,
    error: Option<&str>,
) -> Result<(), String> {
    let target_status = run_terminal_status_as_str(status);
    let current = conn
        .query_row(
            "SELECT status, result_code, error FROM action_combination_runs WHERE id=?1",
            [run_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|db_error| format!("read combination run before finish failed: {db_error}"))?
        .ok_or_else(|| format!("combination run not found: {run_id}"))?;
    if matches!(
        current.0.as_str(),
        "succeeded" | "partially_succeeded" | "failed"
    ) {
        if current.0 == target_status
            && current.1.as_deref() == result_code
            && current.2.as_deref() == error
        {
            return Ok(());
        }
        return Err(format!("组合动作运行已是不同终态: {run_id}/{}", current.0));
    }

    conn.execute(
        "UPDATE action_combination_runs
         SET status=?1, result_code=?2, error=?3, finished_at=CURRENT_TIMESTAMP
         WHERE id=?4 AND status IN ('pending','running')",
        params![target_status, result_code, error, run_id],
    )
    .map_err(|db_error| format!("finish combination run failed: {db_error}"))?;
    Ok(())
}

fn finalize_run_with_results(
    conn: &Connection,
    run_id: &str,
    results: &[ExecutedStep],
) -> Result<RunTerminalStatus, String> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|error| format!("begin combination result reconciliation failed: {error}"))?;
    for result in results {
        persist_step_finished_with_conn(&tx, result)?;
    }
    let status = aggregate_status(results);
    finish_run_with_result(&tx, run_id, status, None, None)?;
    tx.commit()
        .map_err(|error| format!("commit combination result reconciliation failed: {error}"))?;
    Ok(status)
}

fn fail_run_and_unfinished_steps_with_conn(
    conn: &Connection,
    run_id: &str,
    result_code: &str,
    error: &str,
) -> Result<(), String> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|db_error| format!("begin combination failure finalization failed: {db_error}"))?;
    tx.execute(
        "UPDATE action_combination_run_steps
         SET status='failed', result_code=?1, message=?2,
             started_at=COALESCE(started_at, CURRENT_TIMESTAMP),
             finished_at=CURRENT_TIMESTAMP
         WHERE run_id=?3 AND status IN ('pending','running')",
        params![result_code, error, run_id],
    )
    .map_err(|db_error| format!("finish unfinished combination steps failed: {db_error}"))?;
    finish_run_with_result(
        &tx,
        run_id,
        RunTerminalStatus::Failed,
        Some(result_code),
        Some(error),
    )?;
    tx.commit()
        .map_err(|db_error| format!("commit combination failure finalization failed: {db_error}"))
}

pub(crate) fn recover_interrupted_with_conn(conn: &Connection) -> Result<usize, String> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|error| format!("begin interrupted combination recovery failed: {error}"))?;
    tx.execute(
        "UPDATE action_combination_run_steps
         SET status='failed', result_code='interrupted', message=?1,
             finished_at=CURRENT_TIMESTAMP
         WHERE status IN ('pending','running')
           AND run_id IN (
               SELECT id FROM action_combination_runs
               WHERE status IN ('pending','running')
           )",
        [INTERRUPTED_STEP_MESSAGE],
    )
    .map_err(|error| format!("recover interrupted combination steps failed: {error}"))?;
    let recovered = tx
        .execute(
            "UPDATE action_combination_runs
             SET status='failed', result_code='interrupted', error=?1,
                 finished_at=CURRENT_TIMESTAMP
             WHERE status IN ('pending','running')",
            [INTERRUPTED_ERROR],
        )
        .map_err(|error| format!("recover interrupted combination runs failed: {error}"))?;
    tx.commit()
        .map_err(|error| format!("commit interrupted combination recovery failed: {error}"))?;
    Ok(recovered)
}

fn mark_run_started_with_conn(conn: &Connection, run_id: &str) -> Result<(), String> {
    let status = conn
        .query_row(
            "SELECT status FROM action_combination_runs WHERE id=?1",
            [run_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("read combination run before start failed: {error}"))?
        .ok_or_else(|| format!("combination run not found: {run_id}"))?;
    match status.as_str() {
        "pending" => {
            conn.execute(
                "UPDATE action_combination_runs
                 SET status='running', started_at=CURRENT_TIMESTAMP
                 WHERE id=?1 AND status='pending'",
                [run_id],
            )
            .map_err(|error| format!("mark combination run started failed: {error}"))?;
            Ok(())
        }
        "running" => Ok(()),
        _ => Err(format!("组合动作运行已是终态: {run_id}/{status}")),
    }
}

struct DatabaseRunObserver {
    run_id: String,
    conn_factory: ConnectionFactory,
    emitter: EventEmitter,
    first_error: Mutex<Option<String>>,
}

impl DatabaseRunObserver {
    fn new(run_id: String, conn_factory: ConnectionFactory, emitter: EventEmitter) -> Self {
        Self {
            run_id,
            conn_factory,
            emitter,
            first_error: Mutex::new(None),
        }
    }

    fn record_error(&self, error: String) {
        let mut first_error = self
            .first_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if first_error.is_none() {
            *first_error = Some(error);
        }
    }

    fn take_error(&self) -> Option<String> {
        self.first_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

impl ExecutionObserver for DatabaseRunObserver {
    fn step_started(&self, run_step_id: i64) {
        let result = (self.conn_factory)()
            .and_then(|conn| persist_step_started_with_conn(&conn, run_step_id));
        match result {
            Ok(()) => emit_safely(&self.emitter, &self.run_id, "running"),
            Err(error) => self.record_error(error),
        }
    }

    fn step_finished(&self, result: &ExecutedStep) {
        let persisted =
            (self.conn_factory)().and_then(|conn| persist_step_finished_with_conn(&conn, result));
        match persisted {
            Ok(()) => emit_safely(&self.emitter, &self.run_id, "running"),
            Err(error) => self.record_error(error),
        }
    }
}

struct ActiveRunSlot {
    run_id: String,
    combination_name: String,
}

struct ActiveRunGuard {
    run_id: String,
}

impl ActiveRunGuard {
    fn acquire(conn: &Connection, run_id: &str, combination_name: &str) -> Result<Self, String> {
        let mut slot = active_run_slot()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(active) = slot.as_ref() {
            let persisted = conn
                .query_row(
                    "SELECT combination_name, status
                     FROM action_combination_runs
                     WHERE id=?1",
                    [&active.run_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()
                .map_err(|error| format!("read active combination run failed: {error}"))?;
            match persisted {
                Some((persisted_name, status))
                    if matches!(status.as_str(), "pending" | "running") =>
                {
                    return Err(format!("已有组合动作正在运行: {persisted_name} ({status})"));
                }
                None => {
                    return Err(format!(
                        "已有组合动作正在运行: {} (pending)",
                        active.combination_name
                    ));
                }
                Some(_) => {}
            }
        }
        *slot = Some(ActiveRunSlot {
            run_id: run_id.to_string(),
            combination_name: combination_name.to_string(),
        });
        Ok(Self {
            run_id: run_id.to_string(),
        })
    }
}

impl Drop for ActiveRunGuard {
    fn drop(&mut self) {
        let mut slot = active_run_slot()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if slot.as_ref().map(|active| active.run_id.as_str()) == Some(&self.run_id) {
            *slot = None;
        }
    }
}

fn active_run_slot() -> &'static Mutex<Option<ActiveRunSlot>> {
    ACTIVE_RUN_SLOT.get_or_init(|| Mutex::new(None))
}

fn coordinate_run(
    run_id: &str,
    execution_mode: ExecutionMode,
    plan: Vec<PlannedStep>,
    executor: Arc<dyn AtomicActionExecutor>,
    conn_factory: ConnectionFactory,
    emitter: EventEmitter,
) -> Result<(), String> {
    let conn = conn_factory()?;
    mark_run_started_with_conn(&conn, run_id)?;
    emit_safely(&emitter, run_id, "running");

    let observer = Arc::new(DatabaseRunObserver::new(
        run_id.to_string(),
        conn_factory.clone(),
        emitter.clone(),
    ));
    let results = execute_plan_with_observer(
        execution_mode,
        plan,
        executor,
        observer.clone() as Arc<dyn ExecutionObserver>,
    );
    let observer_error = observer.take_error();
    let conn = conn_factory()?;
    let status = finalize_run_with_results(&conn, run_id, &results)?;
    if let Some(error) = observer_error {
        eprintln!("reconciled combination run after observer persistence error {run_id}: {error}");
    }
    emit_safely(&emitter, run_id, run_terminal_status_as_str(status));
    Ok(())
}

fn start_with_dependencies<F, S>(
    conn: &mut Connection,
    combination_id: i64,
    snapshot_target: F,
    executor: Arc<dyn AtomicActionExecutor>,
    conn_factory: ConnectionFactory,
    emitter: EventEmitter,
    spawner: S,
) -> Result<CombinationRunDetail, String>
where
    F: Fn(&str, &str) -> AtomicTargetSnapshot,
    S: FnOnce(BackgroundTask) -> io::Result<()>,
{
    let run_id = Uuid::new_v4().to_string();
    let combination_name = conn
        .query_row(
            "SELECT name FROM action_combinations WHERE id=?1",
            [combination_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("read combination for run failed: {error}"))?
        .ok_or_else(|| format!("combination not found: {combination_id}"))?;
    let guard = Arc::new(ActiveRunGuard::acquire(conn, &run_id, &combination_name)?);
    let (run, plan) =
        create_run_and_plan_with_conn(conn, combination_id, &run_id, snapshot_target)?;

    let background_run_id = run_id.clone();
    let background_factory = conn_factory.clone();
    let background_emitter = emitter.clone();
    let background_guard = guard.clone();
    let task: BackgroundTask = Box::new(move || {
        let _guard = background_guard;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            coordinate_run(
                &background_run_id,
                run.execution_mode,
                plan,
                executor,
                background_factory.clone(),
                background_emitter.clone(),
            )
        }));
        match outcome {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                best_effort_fail_run(
                    &background_factory,
                    &background_emitter,
                    &background_run_id,
                    "coordination_failed",
                    &error,
                );
            }
            Err(payload) => {
                let error = format!("组合动作执行线程 panic: {}", panic_message(payload));
                best_effort_fail_run(
                    &background_factory,
                    &background_emitter,
                    &background_run_id,
                    "executor_panicked",
                    &error,
                );
            }
        }
    });

    if let Err(error) = spawner(task) {
        let message = format!("启动组合动作后台线程失败: {error}");
        fail_run_and_unfinished_steps_with_conn(conn, &run_id, "start_failed", &message)?;
        emit_safely(&emitter, &run_id, "failed");
        return Err(message);
    }
    get_run_with_conn(conn, &run_id)
}

fn best_effort_fail_run(
    conn_factory: &ConnectionFactory,
    emitter: &EventEmitter,
    run_id: &str,
    result_code: &str,
    error: &str,
) {
    match conn_factory()
        .and_then(|conn| fail_run_and_unfinished_steps_with_conn(&conn, run_id, result_code, error))
    {
        Ok(()) => emit_safely(emitter, run_id, "failed"),
        Err(db_error) => {
            eprintln!("failed to persist combination run failure {run_id}: {db_error}")
        }
    }
}

fn emit_safely(emitter: &EventEmitter, run_id: &str, status: &str) {
    let emitted = catch_unwind(AssertUnwindSafe(|| emitter(run_id, status)));
    match emitted {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            eprintln!("failed to emit combination run update {run_id}/{status}: {error}")
        }
        Err(payload) => eprintln!(
            "combination run update emitter panicked {run_id}/{status}: {}",
            panic_message(payload)
        ),
    }
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".into()
    }
}

pub(crate) fn start_with_app(
    app: &tauri::AppHandle,
    combination_id: i64,
    notify_on_completion: bool,
) -> Result<CombinationRunDetail, String> {
    let mut conn = crate::tools::helpers::db_conn()?;
    let event_app = app.clone();
    let emitter: EventEmitter = Arc::new(move |run_id, status| {
        let event_result = event_app
            .emit(
                EVENT_ACTION_CENTER_COMBINATION_RUN_UPDATED,
                CombinationRunUpdatedPayload {
                    run_id: run_id.to_string(),
                    status: status.to_string(),
                },
            )
            .map_err(|error| format!("emit combination run update failed: {error}"));
        if notify_on_completion && matches!(status, "succeeded" | "partially_succeeded" | "failed")
        {
            let conn = crate::tools::helpers::db_conn()?;
            let run = get_run_with_conn(&conn, run_id)?;
            if let Some(notification) =
                crate::global_notification::build_action_combination_notification(&run)
            {
                crate::global_notification::show_notifications(&event_app, vec![notification]);
            }
        }
        event_result
    });
    start_with_dependencies(
        &mut conn,
        combination_id,
        snapshot_target,
        Arc::new(RegisteredAtomicActionExecutor),
        Arc::new(crate::tools::helpers::db_conn),
        emitter,
        |task| {
            thread::Builder::new()
                .name("action-center-combination-run".into())
                .spawn(task)
                .map(|_| ())
        },
    )
}

#[cfg(test)]
fn clear_active_run_slot_for_test() {
    *active_run_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
}

#[cfg(test)]
fn active_run_slot_contains_for_test(run_id: &str) -> bool {
    active_run_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .map(|active| active.run_id.as_str())
        == Some(run_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::action_center::{
        atomic_actions::{
            AtomicActionExecutor, AtomicStepSuccess, AtomicStepSuccessStatus, AtomicTargetSnapshot,
        },
        combination_executor::{ExecutedStep, RunTerminalStatus, StepTerminalStatus},
        combinations::ExecutionMode,
    };
    use rusqlite::{params, Connection};
    use std::{
        collections::HashMap,
        io,
        path::PathBuf,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc, Mutex,
        },
    };
    use uuid::Uuid;

    static COORDINATOR_TEST_LOCK: Mutex<()> = Mutex::new(());

    struct TempDatabase {
        path: PathBuf,
    }

    impl TempDatabase {
        fn new() -> (Self, Connection) {
            let path = std::env::temp_dir().join(format!(
                "lazycat-combination-runs-{}.sqlite",
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

        fn factory(&self) -> ConnectionFactory {
            let path = self.path.clone();
            Arc::new(move || {
                let conn = Connection::open(&path)
                    .map_err(|error| format!("open test database failed: {error}"))?;
                conn.execute_batch(
                    "PRAGMA foreign_keys = ON;
                     PRAGMA busy_timeout = 5000;",
                )
                .map_err(|error| format!("configure test database failed: {error}"))?;
                Ok(conn)
            })
        }
    }

    impl Drop for TempDatabase {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_file(format!("{}-wal", self.path.display()));
            let _ = std::fs::remove_file(format!("{}-shm", self.path.display()));
        }
    }

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        super::super::ensure_schema(&conn).unwrap();
        conn
    }

    fn seed_combination(
        conn: &Connection,
        name: &str,
        mode: ExecutionMode,
        steps: &[(&str, &str)],
    ) -> i64 {
        let mode = match mode {
            ExecutionMode::Serial => "serial",
            ExecutionMode::Parallel => "parallel",
        };
        conn.execute(
            "INSERT INTO action_combinations(name, execution_mode) VALUES (?1, ?2)",
            params![name, mode],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        for (sort_order, (action_type, target_id)) in steps.iter().enumerate() {
            conn.execute(
                "INSERT INTO action_combination_steps
                 (combination_id, action_type, target_id, sort_order)
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, action_type, target_id, sort_order as i64],
            )
            .unwrap();
        }
        id
    }

    fn snapshot(action_type: &str, target_id: &str) -> AtomicTargetSnapshot {
        AtomicTargetSnapshot {
            action_label: format!("action:{action_type}"),
            target_label: format!("target:{target_id}"),
            validation_error: None,
        }
    }

    #[test]
    fn run_keeps_combination_and_step_snapshots_after_sources_change() {
        let mut conn = test_conn();
        let combination_id = seed_combination(
            &conn,
            "晨间启动",
            ExecutionMode::Parallel,
            &[("hosts.activate", "7"), ("request_forward.start", "9")],
        );

        let run = create_run_with_conn(&mut conn, combination_id, snapshot).unwrap();
        conn.execute(
            "UPDATE action_combinations SET name='已改名', execution_mode='serial' WHERE id=?1",
            [combination_id],
        )
        .unwrap();
        conn.execute(
            "DELETE FROM action_combination_steps WHERE combination_id=?1",
            [combination_id],
        )
        .unwrap();

        let loaded = get_run_with_conn(&conn, &run.id).unwrap();
        assert_eq!(loaded.combination_name, "晨间启动");
        assert_eq!(loaded.execution_mode, ExecutionMode::Parallel);
        assert_eq!(loaded.steps.len(), 2);
        assert_eq!(
            (
                loaded.steps[0].action_label.as_str(),
                loaded.steps[0].target_id.as_str(),
                loaded.steps[0].target_label.as_str(),
                loaded.steps[0].sort_order,
            ),
            ("action:hosts.activate", "7", "target:7", 0)
        );
        assert_eq!(
            (
                loaded.steps[1].action_label.as_str(),
                loaded.steps[1].target_id.as_str(),
                loaded.steps[1].target_label.as_str(),
                loaded.steps[1].sort_order,
            ),
            ("action:request_forward.start", "9", "target:9", 1)
        );
    }

    #[test]
    fn create_rejects_another_active_run_globally() {
        let mut conn = test_conn();
        let first = seed_combination(
            &conn,
            "first",
            ExecutionMode::Serial,
            &[("hosts.activate", "1")],
        );
        let second = seed_combination(
            &conn,
            "second",
            ExecutionMode::Serial,
            &[("hosts.activate", "2")],
        );

        let active = create_run_with_conn(&mut conn, first, snapshot).unwrap();
        let pending_error = create_run_with_conn(&mut conn, first, snapshot).unwrap_err();
        assert!(pending_error.contains("first"), "{pending_error}");
        assert!(pending_error.contains("pending"), "{pending_error}");

        mark_run_started_with_conn(&conn, &active.id).unwrap();
        let running_error = create_run_with_conn(&mut conn, second, snapshot).unwrap_err();
        assert!(running_error.contains("first"), "{running_error}");
        assert!(running_error.contains("running"), "{running_error}");

        let run_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM action_combination_runs", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(run_count, 1);
    }

    #[test]
    fn active_slot_rejection_reports_database_combination_name_and_status() {
        let _serial = COORDINATOR_TEST_LOCK.lock().unwrap();
        clear_active_run_slot_for_test();
        let mut conn = test_conn();
        let combination_id = seed_combination(
            &conn,
            "正在初始化开发环境",
            ExecutionMode::Serial,
            &[("hosts.activate", "1")],
        );
        let run = create_run_with_conn(&mut conn, combination_id, snapshot).unwrap();
        let guard = ActiveRunGuard::acquire(&conn, &run.id, &run.combination_name).unwrap();
        mark_run_started_with_conn(&conn, &run.id).unwrap();

        let error = ActiveRunGuard::acquire(&conn, "new-run", "new combination")
            .err()
            .expect("active slot must reject another run");

        assert!(error.contains("正在初始化开发环境"), "{error}");
        assert!(error.contains("running"), "{error}");
        drop(guard);
        clear_active_run_slot_for_test();
    }

    #[test]
    fn active_slot_rejects_another_run_while_first_is_only_reserved() {
        let _serial = COORDINATOR_TEST_LOCK.lock().unwrap();
        clear_active_run_slot_for_test();
        let conn = test_conn();
        let guard = ActiveRunGuard::acquire(&conn, "reserved-run", "first combination").unwrap();

        let error = ActiveRunGuard::acquire(&conn, "second-run", "second combination")
            .err()
            .expect("reserved slot must reject another run before database insertion");

        assert!(error.contains("first combination"), "{error}");
        assert!(error.contains("pending"), "{error}");
        assert!(active_run_slot_contains_for_test("reserved-run"));
        drop(guard);
        assert!(!active_run_slot_contains_for_test("reserved-run"));
        clear_active_run_slot_for_test();
    }

    #[test]
    fn create_rejects_missing_or_empty_combination_but_keeps_validation_error_in_plan() {
        let mut conn = test_conn();
        let missing = create_run_with_conn(&mut conn, 999, snapshot).unwrap_err();
        assert!(missing.contains("combination not found"), "{missing}");

        let empty = seed_combination(&conn, "empty", ExecutionMode::Serial, &[]);
        let empty_error = create_run_with_conn(&mut conn, empty, snapshot).unwrap_err();
        assert!(empty_error.contains("has no steps"), "{empty_error}");

        let invalid = seed_combination(
            &conn,
            "invalid target",
            ExecutionMode::Serial,
            &[("hosts.activate", "missing")],
        );
        let run_id = Uuid::new_v4().to_string();
        let (run, plan) =
            create_run_and_plan_with_conn(&mut conn, invalid, &run_id, |action, target| {
                AtomicTargetSnapshot {
                    action_label: action.into(),
                    target_label: target.into(),
                    validation_error: Some("目标不存在".into()),
                }
            })
            .unwrap();
        assert_eq!(run.status, "pending");
        assert_eq!(plan[0].validation_error.as_deref(), Some("目标不存在"));
    }

    #[test]
    fn step_progress_and_aggregate_status_are_persisted() {
        let mut conn = test_conn();
        let combination_id = seed_combination(
            &conn,
            "mixed",
            ExecutionMode::Serial,
            &[
                ("hosts.activate", "1"),
                ("hosts.activate", "2"),
                ("hosts.activate", "3"),
            ],
        );
        let run = create_run_with_conn(&mut conn, combination_id, snapshot).unwrap();
        let results = [
            ExecutedStep {
                run_step_id: run.steps[0].id,
                sort_order: 0,
                status: StepTerminalStatus::Succeeded,
                result_code: Some("changed".into()),
                message: Some("done".into()),
            },
            ExecutedStep {
                run_step_id: run.steps[1].id,
                sort_order: 1,
                status: StepTerminalStatus::Failed,
                result_code: Some("denied".into()),
                message: Some("no permission".into()),
            },
            ExecutedStep {
                run_step_id: run.steps[2].id,
                sort_order: 2,
                status: StepTerminalStatus::AlreadySatisfied,
                result_code: Some("unchanged".into()),
                message: None,
            },
        ];

        for result in &results {
            persist_step_started_with_conn(&conn, result.run_step_id).unwrap();
            let running = get_run_with_conn(&conn, &run.id).unwrap();
            let step = running
                .steps
                .iter()
                .find(|step| step.id == result.run_step_id)
                .unwrap();
            assert_eq!(step.status, "running");
            assert!(step.started_at.is_some());
            persist_step_finished_with_conn(&conn, result).unwrap();
        }
        finish_run_with_conn(
            &conn,
            &run.id,
            crate::tools::action_center::combination_executor::aggregate_status(&results),
        )
        .unwrap();

        let loaded = get_run_with_conn(&conn, &run.id).unwrap();
        assert_eq!(loaded.status, "partially_succeeded");
        assert!(loaded.finished_at.is_some());
        assert_eq!(
            loaded
                .steps
                .iter()
                .map(|step| (
                    step.status.as_str(),
                    step.result_code.as_deref(),
                    step.message.as_deref()
                ))
                .collect::<Vec<_>>(),
            vec![
                ("succeeded", Some("changed"), Some("done")),
                ("failed", Some("denied"), Some("no permission")),
                ("already_satisfied", Some("unchanged"), None),
            ]
        );
        assert!(loaded.steps.iter().all(|step| step.finished_at.is_some()));
    }

    #[test]
    fn recovery_fails_interrupted_runs_and_only_non_terminal_steps() {
        let mut conn = test_conn();
        let combination_id = seed_combination(
            &conn,
            "recover",
            ExecutionMode::Serial,
            &[("hosts.activate", "1"), ("hosts.activate", "2")],
        );
        let run = create_run_with_conn(&mut conn, combination_id, snapshot).unwrap();
        persist_step_started_with_conn(&conn, run.steps[0].id).unwrap();
        persist_step_finished_with_conn(
            &conn,
            &ExecutedStep {
                run_step_id: run.steps[0].id,
                sort_order: 0,
                status: StepTerminalStatus::Succeeded,
                result_code: Some("kept".into()),
                message: Some("completed before crash".into()),
            },
        )
        .unwrap();
        conn.execute(
            "UPDATE action_combination_runs
             SET status='running', started_at=CURRENT_TIMESTAMP WHERE id=?1",
            [&run.id],
        )
        .unwrap();

        recover_interrupted_with_conn(&conn).unwrap();

        let recovered = get_run_with_conn(&conn, &run.id).unwrap();
        assert_eq!(recovered.status, "failed");
        assert_eq!(recovered.result_code.as_deref(), Some("interrupted"));
        assert!(recovered.error.as_deref().unwrap().contains("中断"));
        assert!(recovered.finished_at.is_some());
        assert_eq!(recovered.steps[0].status, "succeeded");
        assert_eq!(recovered.steps[0].result_code.as_deref(), Some("kept"));
        assert_eq!(
            recovered.steps[0].message.as_deref(),
            Some("completed before crash")
        );
        assert_eq!(recovered.steps[1].status, "failed");
        assert_eq!(
            recovered.steps[1].result_code.as_deref(),
            Some("interrupted")
        );
        assert!(recovered.steps[1]
            .message
            .as_deref()
            .unwrap()
            .contains("中断"));
        assert!(recovered.steps[1].finished_at.is_some());
    }

    #[test]
    fn list_returns_latest_twenty_runs_for_only_requested_combination() {
        let mut conn = test_conn();
        let wanted = seed_combination(
            &conn,
            "wanted",
            ExecutionMode::Serial,
            &[("hosts.activate", "1")],
        );
        let other = seed_combination(
            &conn,
            "other",
            ExecutionMode::Serial,
            &[("hosts.activate", "2")],
        );
        for _ in 0..25 {
            let run = create_run_with_conn(&mut conn, wanted, snapshot).unwrap();
            conn.execute(
                "UPDATE action_combination_runs
                 SET status='succeeded', created_at='2026-01-01 00:00:00',
                     finished_at=CURRENT_TIMESTAMP
                 WHERE id=?1",
                [&run.id],
            )
            .unwrap();
        }
        let other_run = create_run_with_conn(&mut conn, other, snapshot).unwrap();
        conn.execute(
            "UPDATE action_combination_runs
             SET status='succeeded', finished_at=CURRENT_TIMESTAMP WHERE id=?1",
            [&other_run.id],
        )
        .unwrap();

        let runs = list_runs_with_conn(&conn, wanted).unwrap();
        assert_eq!(runs.len(), 20);
        assert!(runs.iter().all(|run| run.combination_id == Some(wanted)));
        assert!(runs.windows(2).all(|pair| pair[0].id > pair[1].id));
    }

    #[test]
    fn new_runs_store_subsecond_creation_time() {
        let mut conn = test_conn();
        let combination_id = seed_combination(
            &conn,
            "precise time",
            ExecutionMode::Serial,
            &[("hosts.activate", "1")],
        );

        let run = create_run_with_conn(&mut conn, combination_id, snapshot).unwrap();

        assert!(
            run.created_at.contains('.'),
            "created_at={}",
            run.created_at
        );
    }

    struct FakeExecutor {
        results: HashMap<String, Result<AtomicStepSuccess, String>>,
    }

    impl AtomicActionExecutor for FakeExecutor {
        fn execute(
            &self,
            _action_type: &str,
            target_id: &str,
        ) -> Result<AtomicStepSuccess, String> {
            self.results.get(target_id).unwrap().clone()
        }
    }

    fn success(status: AtomicStepSuccessStatus) -> Result<AtomicStepSuccess, String> {
        Ok(AtomicStepSuccess {
            status,
            result_code: Some("fake".into()),
            message: Some("fake result".into()),
        })
    }

    fn inline_spawner(task: BackgroundTask) -> io::Result<()> {
        task();
        Ok(())
    }

    #[test]
    fn start_rejects_occupied_process_slot_before_creating_run() {
        let _serial = COORDINATOR_TEST_LOCK.lock().unwrap();
        clear_active_run_slot_for_test();
        let (database, mut conn) = TempDatabase::new();
        let first = seed_combination(
            &conn,
            "first running combination",
            ExecutionMode::Serial,
            &[("hosts.activate", "1")],
        );
        let second = seed_combination(
            &conn,
            "second combination",
            ExecutionMode::Serial,
            &[("hosts.activate", "2")],
        );
        let first_run = create_run_with_conn(&mut conn, first, snapshot).unwrap();
        let guard =
            ActiveRunGuard::acquire(&conn, &first_run.id, &first_run.combination_name).unwrap();
        conn.execute("DROP INDEX idx_action_combination_runs_one_active", [])
            .unwrap();
        let executor = Arc::new(FakeExecutor {
            results: HashMap::from([("2".into(), success(AtomicStepSuccessStatus::Succeeded))]),
        });

        let error = start_with_dependencies(
            &mut conn,
            second,
            snapshot,
            executor,
            database.factory(),
            Arc::new(|_, _| Ok(())),
            |_task| panic!("occupied slot must reject before spawning"),
        )
        .unwrap_err();

        assert!(error.contains("first running combination"), "{error}");
        assert!(error.contains("pending"), "{error}");
        let run_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM action_combination_runs", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(run_count, 1, "slot rejection must not create a second run");
        drop(guard);
        clear_active_run_slot_for_test();
    }

    #[test]
    fn coordinator_persists_fake_results_and_ignores_event_errors() {
        let _serial = COORDINATOR_TEST_LOCK.lock().unwrap();
        clear_active_run_slot_for_test();
        let (database, mut conn) = TempDatabase::new();
        let combination_id = seed_combination(
            &conn,
            "coordinator",
            ExecutionMode::Serial,
            &[
                ("hosts.activate", "ok"),
                ("hosts.activate", "bad"),
                ("hosts.activate", "same"),
            ],
        );
        let executor = Arc::new(FakeExecutor {
            results: HashMap::from([
                ("ok".into(), success(AtomicStepSuccessStatus::Succeeded)),
                ("bad".into(), Err("fake failure".into())),
                (
                    "same".into(),
                    success(AtomicStepSuccessStatus::AlreadySatisfied),
                ),
            ]),
        });

        let started = start_with_dependencies(
            &mut conn,
            combination_id,
            snapshot,
            executor,
            database.factory(),
            Arc::new(|_, _| Err("event unavailable".into())),
            inline_spawner,
        )
        .unwrap();

        let run = get_run_with_conn(&conn, &started.id).unwrap();
        assert_eq!(run.status, "partially_succeeded");
        assert_eq!(
            run.steps
                .iter()
                .map(|step| step.status.as_str())
                .collect::<Vec<_>>(),
            vec!["succeeded", "failed", "already_satisfied"]
        );
        assert!(run.steps.iter().all(|step| step.finished_at.is_some()));
        assert!(!active_run_slot_contains_for_test(&started.id));
    }

    #[test]
    fn step_events_keep_run_status_running_until_final_aggregate() {
        let _serial = COORDINATOR_TEST_LOCK.lock().unwrap();
        clear_active_run_slot_for_test();
        let (database, mut conn) = TempDatabase::new();
        let combination_id = seed_combination(
            &conn,
            "event status",
            ExecutionMode::Serial,
            &[("hosts.activate", "1")],
        );
        let executor = Arc::new(FakeExecutor {
            results: HashMap::from([("1".into(), success(AtomicStepSuccessStatus::Succeeded))]),
        });
        let statuses = Arc::new(Mutex::new(Vec::new()));
        let emitter: EventEmitter = Arc::new({
            let statuses = statuses.clone();
            move |_, status| {
                statuses.lock().unwrap().push(status.to_string());
                Ok(())
            }
        });

        start_with_dependencies(
            &mut conn,
            combination_id,
            snapshot,
            executor,
            database.factory(),
            emitter,
            inline_spawner,
        )
        .unwrap();

        assert_eq!(
            *statuses.lock().unwrap(),
            vec!["running", "running", "running", "succeeded"]
        );
    }

    #[test]
    fn spawn_failure_marks_run_failed_and_releases_slot() {
        let _serial = COORDINATOR_TEST_LOCK.lock().unwrap();
        clear_active_run_slot_for_test();
        let (database, mut conn) = TempDatabase::new();
        let combination_id = seed_combination(
            &conn,
            "spawn fail",
            ExecutionMode::Serial,
            &[("hosts.activate", "1")],
        );
        let executor = Arc::new(FakeExecutor {
            results: HashMap::from([("1".into(), success(AtomicStepSuccessStatus::Succeeded))]),
        });

        let error = start_with_dependencies(
            &mut conn,
            combination_id,
            snapshot,
            executor,
            database.factory(),
            Arc::new(|_, _| Ok(())),
            |_task| Err(io::Error::other("injected spawn failure")),
        )
        .unwrap_err();
        assert!(error.contains("injected spawn failure"), "{error}");

        let run_id: String = conn
            .query_row(
                "SELECT id FROM action_combination_runs
                 ORDER BY created_at DESC, id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let failed = get_run_with_conn(&conn, &run_id).unwrap();
        assert_eq!(failed.status, "failed");
        assert_eq!(failed.result_code.as_deref(), Some("start_failed"));
        assert!(failed
            .error
            .as_deref()
            .unwrap()
            .contains("injected spawn failure"));
        assert!(failed.steps.iter().all(|step| step.status == "failed"));
        assert!(failed
            .steps
            .iter()
            .all(|step| step.result_code.as_deref() == Some("start_failed")));
        assert!(!active_run_slot_contains_for_test(&run_id));
    }

    #[test]
    fn spawn_failure_keeps_slot_reserved_until_failure_is_persisted() {
        let _serial = COORDINATOR_TEST_LOCK.lock().unwrap();
        clear_active_run_slot_for_test();
        let (database, mut conn) = TempDatabase::new();
        let combination_id = seed_combination(
            &conn,
            "spawn failure reservation",
            ExecutionMode::Serial,
            &[("hosts.activate", "1")],
        );
        let executor = Arc::new(FakeExecutor {
            results: HashMap::from([("1".into(), success(AtomicStepSuccessStatus::Succeeded))]),
        });
        let contender_conn = (database.factory())().unwrap();
        let contender_rejected = Arc::new(AtomicBool::new(false));
        let contender_error = Arc::new(Mutex::new(None));

        let error = start_with_dependencies(
            &mut conn,
            combination_id,
            snapshot,
            executor,
            database.factory(),
            Arc::new(|_, _| Ok(())),
            {
                let contender_rejected = contender_rejected.clone();
                let contender_error = contender_error.clone();
                move |task| {
                    drop(task);
                    match ActiveRunGuard::acquire(
                        &contender_conn,
                        "contender-run",
                        "contender combination",
                    ) {
                        Ok(guard) => drop(guard),
                        Err(error) => {
                            contender_rejected.store(true, Ordering::SeqCst);
                            *contender_error.lock().unwrap() = Some(error);
                        }
                    }
                    Err(io::Error::other("injected spawn failure"))
                }
            },
        )
        .unwrap_err();

        assert!(error.contains("injected spawn failure"), "{error}");
        assert!(contender_rejected.load(Ordering::SeqCst));
        let contender_error = contender_error.lock().unwrap().clone().unwrap();
        assert!(
            contender_error.contains("spawn failure reservation"),
            "{contender_error}"
        );
        assert!(contender_error.contains("pending"), "{contender_error}");

        let next_guard = ActiveRunGuard::acquire(&conn, "next-run", "next combination").unwrap();
        drop(next_guard);
        clear_active_run_slot_for_test();
    }

    #[test]
    fn observer_write_error_is_reconciled_from_execution_results() {
        let _serial = COORDINATOR_TEST_LOCK.lock().unwrap();
        clear_active_run_slot_for_test();
        let (database, mut conn) = TempDatabase::new();
        let combination_id = seed_combination(
            &conn,
            "observer retry",
            ExecutionMode::Serial,
            &[("hosts.activate", "1")],
        );
        let executor = Arc::new(FakeExecutor {
            results: HashMap::from([("1".into(), success(AtomicStepSuccessStatus::Succeeded))]),
        });
        let real_factory = database.factory();
        let attempts = Arc::new(AtomicUsize::new(0));
        let flaky_factory: ConnectionFactory = Arc::new({
            let attempts = attempts.clone();
            move || {
                if attempts.fetch_add(1, Ordering::SeqCst) == 2 {
                    Err("injected observer finish error".into())
                } else {
                    real_factory()
                }
            }
        });

        let started = start_with_dependencies(
            &mut conn,
            combination_id,
            snapshot,
            executor,
            flaky_factory,
            Arc::new(|_, _| Ok(())),
            inline_spawner,
        )
        .unwrap();

        let run = get_run_with_conn(&conn, &started.id).unwrap();
        assert_eq!(run.status, "succeeded");
        assert_eq!(run.steps[0].status, "succeeded");
        assert_eq!(run.steps[0].result_code.as_deref(), Some("fake"));
    }

    #[test]
    fn terminal_database_run_can_replace_stale_memory_slot() {
        let _serial = COORDINATOR_TEST_LOCK.lock().unwrap();
        clear_active_run_slot_for_test();
        let (database, mut conn) = TempDatabase::new();
        let first = seed_combination(
            &conn,
            "first",
            ExecutionMode::Serial,
            &[("hosts.activate", "1")],
        );
        let second = seed_combination(
            &conn,
            "second",
            ExecutionMode::Serial,
            &[("hosts.activate", "2")],
        );
        let first_run = create_run_with_conn(&mut conn, first, snapshot).unwrap();
        let stale_guard =
            ActiveRunGuard::acquire(&conn, &first_run.id, &first_run.combination_name).unwrap();
        finish_run_with_conn(&conn, &first_run.id, RunTerminalStatus::Succeeded).unwrap();
        let executor = Arc::new(FakeExecutor {
            results: HashMap::from([("2".into(), success(AtomicStepSuccessStatus::Succeeded))]),
        });

        let second_run = start_with_dependencies(
            &mut conn,
            second,
            snapshot,
            executor,
            database.factory(),
            Arc::new(|_, _| Ok(())),
            inline_spawner,
        )
        .unwrap();

        assert_eq!(
            get_run_with_conn(&conn, &second_run.id).unwrap().status,
            "succeeded"
        );
        drop(stale_guard);
        assert!(!active_run_slot_contains_for_test(&second_run.id));
    }

    #[test]
    fn coordinator_releases_slot_after_event_panic_and_allows_next_run() {
        let _serial = COORDINATOR_TEST_LOCK.lock().unwrap();
        clear_active_run_slot_for_test();
        let (database, mut conn) = TempDatabase::new();
        let first = seed_combination(
            &conn,
            "panic event",
            ExecutionMode::Serial,
            &[("hosts.activate", "1")],
        );
        let second = seed_combination(
            &conn,
            "next",
            ExecutionMode::Serial,
            &[("hosts.activate", "2")],
        );
        let executor = Arc::new(FakeExecutor {
            results: HashMap::from([
                ("1".into(), success(AtomicStepSuccessStatus::Succeeded)),
                ("2".into(), success(AtomicStepSuccessStatus::Succeeded)),
            ]),
        });

        let first_run = start_with_dependencies(
            &mut conn,
            first,
            snapshot,
            executor.clone(),
            database.factory(),
            Arc::new(|_, _| panic!("event panic")),
            inline_spawner,
        )
        .unwrap();
        assert_eq!(
            get_run_with_conn(&conn, &first_run.id).unwrap().status,
            "succeeded"
        );

        let second_run = start_with_dependencies(
            &mut conn,
            second,
            snapshot,
            executor,
            database.factory(),
            Arc::new(|_, _| Ok(())),
            inline_spawner,
        )
        .unwrap();
        assert_eq!(
            get_run_with_conn(&conn, &second_run.id).unwrap().status,
            "succeeded"
        );
        assert!(!active_run_slot_contains_for_test(&second_run.id));
    }

    struct PanickingExecutor;

    impl AtomicActionExecutor for PanickingExecutor {
        fn execute(
            &self,
            _action_type: &str,
            _target_id: &str,
        ) -> Result<AtomicStepSuccess, String> {
            panic!("executor panic")
        }
    }

    #[test]
    fn coordinator_releases_slot_after_executor_panic_and_database_error() {
        let _serial = COORDINATOR_TEST_LOCK.lock().unwrap();
        clear_active_run_slot_for_test();
        let (database, mut conn) = TempDatabase::new();
        let executor_panic = seed_combination(
            &conn,
            "executor panic",
            ExecutionMode::Serial,
            &[("hosts.activate", "1")],
        );
        let database_error = seed_combination(
            &conn,
            "database error",
            ExecutionMode::Serial,
            &[("hosts.activate", "2")],
        );
        let after_error = seed_combination(
            &conn,
            "after error",
            ExecutionMode::Serial,
            &[("hosts.activate", "3")],
        );

        let panic_run = start_with_dependencies(
            &mut conn,
            executor_panic,
            snapshot,
            Arc::new(PanickingExecutor),
            database.factory(),
            Arc::new(|_, _| Ok(())),
            inline_spawner,
        )
        .unwrap();
        assert_eq!(
            get_run_with_conn(&conn, &panic_run.id).unwrap().status,
            "failed"
        );
        assert!(!active_run_slot_contains_for_test(&panic_run.id));

        let real_factory = database.factory();
        let attempts = Arc::new(AtomicUsize::new(0));
        let failing_factory: ConnectionFactory = Arc::new({
            let attempts = attempts.clone();
            move || {
                if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err("injected database error".into())
                } else {
                    real_factory()
                }
            }
        });
        let executor = Arc::new(FakeExecutor {
            results: HashMap::from([
                ("2".into(), success(AtomicStepSuccessStatus::Succeeded)),
                ("3".into(), success(AtomicStepSuccessStatus::Succeeded)),
            ]),
        });
        let failed_run = start_with_dependencies(
            &mut conn,
            database_error,
            snapshot,
            executor.clone(),
            failing_factory,
            Arc::new(|_, _| Ok(())),
            inline_spawner,
        )
        .unwrap();
        let failed = get_run_with_conn(&conn, &failed_run.id).unwrap();
        assert_eq!(failed.status, "failed");
        assert_eq!(failed.result_code.as_deref(), Some("coordination_failed"));
        assert!(failed
            .error
            .as_deref()
            .unwrap()
            .contains("injected database error"));
        assert!(!active_run_slot_contains_for_test(&failed_run.id));

        let next = start_with_dependencies(
            &mut conn,
            after_error,
            snapshot,
            executor,
            database.factory(),
            Arc::new(|_, _| Ok(())),
            inline_spawner,
        )
        .unwrap();
        assert_eq!(
            get_run_with_conn(&conn, &next.id).unwrap().status,
            "succeeded"
        );
    }

    #[test]
    fn update_event_payload_serializes_camel_case() {
        assert_eq!(
            serde_json::to_value(CombinationRunUpdatedPayload {
                run_id: "run-1".into(),
                status: "running".into(),
            })
            .unwrap(),
            serde_json::json!({
                "runId": "run-1",
                "status": "running",
            })
        );
    }

    #[test]
    fn conflicting_terminal_rewrite_is_rejected() {
        let mut conn = test_conn();
        let combination_id = seed_combination(
            &conn,
            "terminal",
            ExecutionMode::Serial,
            &[("hosts.activate", "1")],
        );
        let run = create_run_with_conn(&mut conn, combination_id, snapshot).unwrap();
        finish_run_with_conn(&conn, &run.id, RunTerminalStatus::Succeeded).unwrap();
        finish_run_with_conn(&conn, &run.id, RunTerminalStatus::Succeeded).unwrap();
        let error = finish_run_with_conn(&conn, &run.id, RunTerminalStatus::Failed).unwrap_err();
        assert!(error.contains("终态"), "{error}");
        assert_eq!(
            get_run_with_conn(&conn, &run.id).unwrap().status,
            "succeeded"
        );
    }
}

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::Emitter;
use uuid::Uuid;

use super::bindings::validate_binding_target;
use super::definitions::definition;

const STATUS_PENDING_CONFIRMATION: &str = "pending_confirmation";
const STATUS_RUNNING: &str = "running";
const STATUS_SUCCEEDED: &str = "succeeded";
const STATUS_FAILED: &str = "failed";
const STATUS_CANCELLED: &str = "cancelled";

#[derive(Clone, Debug)]
pub(crate) struct CreateDispatchRequest {
    pub trigger_type: String,
    pub trigger_id: String,
    pub trigger_event_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DispatchSummary {
    pub id: String,
    pub trigger_type: String,
    pub trigger_id: String,
    pub action_type: String,
    pub target_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActionDispatchRequest {
    pub dispatch_id: String,
    pub action_type: String,
    pub target_tool_id: String,
    pub target_id: String,
}

fn can_transition(current: &str, next: &str) -> bool {
    matches!(
        (current, next),
        (STATUS_PENDING_CONFIRMATION, STATUS_RUNNING)
            | (STATUS_PENDING_CONFIRMATION, STATUS_FAILED)
            | (STATUS_PENDING_CONFIRMATION, STATUS_CANCELLED)
            | (STATUS_RUNNING, STATUS_SUCCEEDED)
            | (STATUS_RUNNING, STATUS_FAILED)
            | (STATUS_RUNNING, STATUS_CANCELLED)
    )
}

fn load_dispatch_summary(
    conn: &Connection,
    dispatch_id: &str,
) -> Result<Option<DispatchSummary>, String> {
    conn.query_row(
        "SELECT id, trigger_type, trigger_id, action_type, target_id, status,
                result_code, error, created_at, started_at, finished_at
         FROM action_dispatches
         WHERE id=?1",
        [dispatch_id],
        |row| {
            Ok(DispatchSummary {
                id: row.get(0)?,
                trigger_type: row.get(1)?,
                trigger_id: row.get(2)?,
                action_type: row.get(3)?,
                target_id: row.get(4)?,
                status: row.get(5)?,
                result_code: row.get(6)?,
                error: row.get(7)?,
                created_at: row.get(8)?,
                started_at: row.get(9)?,
                finished_at: row.get(10)?,
            })
        },
    )
    .optional()
    .map_err(|error| format!("查询动作派发失败: {error}"))
}

pub(crate) fn create_dispatch_with_conn(
    conn: &mut Connection,
    request: &CreateDispatchRequest,
) -> Result<DispatchSummary, String> {
    if request.trigger_type != "todo_item" {
        return Err("暂不支持该动作触发类型".into());
    }
    let todo_id = request
        .trigger_id
        .parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or("任务 id 不合法")?;

    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("开启动作派发事务失败: {error}"))?;
    let (todo_status, todo_kind): (String, String) = tx
        .query_row(
            "SELECT status, kind FROM todo_items WHERE id=?1",
            [todo_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| format!("查询触发任务失败: {error}"))?
        .ok_or("任务不存在")?;
    if todo_status == "completed" {
        return Err("已完成任务不能执行动作".into());
    }
    if todo_kind != "one_off" {
        return Err("周期事项暂不支持执行动作".into());
    }

    let (binding_id, action_type, target_id): (i64, String, String) = tx
        .query_row(
            "SELECT id, action_type, target_id
             FROM action_bindings
             WHERE trigger_type='todo_item' AND trigger_id=?1 AND enabled=1",
            [&request.trigger_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| format!("查询动作绑定失败: {error}"))?
        .ok_or("任务未绑定执行动作")?;
    let action =
        definition(&action_type).ok_or_else(|| format!("动作类型不存在: {action_type}"))?;
    if !action.trigger_types.contains(&"todo_item") {
        return Err("该动作不支持由事项触发".into());
    }
    validate_binding_target(&tx, &action_type, &target_id)?;

    let has_active: bool = tx
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM action_dispatches
                WHERE binding_id=?1 AND status IN ('pending_confirmation','running')
             )",
            [binding_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("检查活动动作失败: {error}"))?;
    if has_active {
        return Err("已有待确认或进行中的动作".into());
    }

    let trigger_event_id = match request.trigger_event_id.as_deref() {
        Some(value) => {
            let event_id = value
                .parse::<i64>()
                .ok()
                .filter(|id| *id > 0)
                .ok_or("提醒事件 id 不合法")?;
            let event_todo_id = tx
                .query_row(
                    "SELECT task_id FROM todo_reminder_events WHERE id=?1",
                    [event_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(|error| format!("查询提醒事件失败: {error}"))?;
            if event_todo_id != Some(todo_id) {
                return Err("提醒事件与当前任务不匹配".into());
            }
            value.to_string()
        }
        None => Uuid::new_v4().to_string(),
    };
    let dispatch_id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO action_dispatches(
            id, binding_id, trigger_type, trigger_id, trigger_event_id,
            action_type, target_id, status
         ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending_confirmation')",
        params![
            dispatch_id,
            binding_id,
            request.trigger_type,
            request.trigger_id,
            trigger_event_id,
            action_type,
            target_id,
        ],
    )
    .map_err(|error| {
        if error
            .to_string()
            .contains("idx_action_dispatches_one_active_binding")
        {
            "已有待确认或进行中的动作".to_string()
        } else {
            format!("创建动作派发失败: {error}")
        }
    })?;
    if let Some(value) = request.trigger_event_id.as_deref() {
        tx.execute(
            "UPDATE todo_reminder_events SET is_read=1 WHERE id=?1 AND task_id=?2",
            params![
                value.parse::<i64>().expect("validated reminder event id"),
                todo_id
            ],
        )
        .map_err(|error| format!("更新提醒事件失败: {error}"))?;
    }
    let dispatch = load_dispatch_summary(&tx, &dispatch_id)?.ok_or("创建动作派发后无法读取记录")?;
    tx.commit()
        .map_err(|error| format!("提交动作派发事务失败: {error}"))?;
    Ok(dispatch)
}

#[cfg(test)]
pub(crate) fn transition_with_conn(
    conn: &mut Connection,
    dispatch_id: &str,
    next: &str,
    result_code: Option<&str>,
    error: Option<&str>,
    expected_run_id: Option<&str>,
) -> Result<bool, String> {
    transition_from_with_conn(
        conn,
        dispatch_id,
        next,
        result_code,
        error,
        expected_run_id,
        None,
    )
}

fn transition_from_with_conn(
    conn: &mut Connection,
    dispatch_id: &str,
    next: &str,
    result_code: Option<&str>,
    error: Option<&str>,
    expected_run_id: Option<&str>,
    required_current: Option<&str>,
) -> Result<bool, String> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|cause| format!("开启动作状态事务失败: {cause}"))?;
    let current = tx
        .query_row(
            "SELECT status, external_run_id FROM action_dispatches WHERE id=?1",
            [dispatch_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map_err(|cause| format!("查询动作状态失败: {cause}"))?
        .ok_or("动作派发不存在")?;
    if expected_run_id.is_some() && current.1.as_deref() != expected_run_id {
        return Ok(false);
    }
    if required_current.is_some() && Some(current.0.as_str()) != required_current {
        return Ok(false);
    }
    if !can_transition(&current.0, next) {
        return Ok(false);
    }
    let terminal = matches!(next, STATUS_SUCCEEDED | STATUS_FAILED | STATUS_CANCELLED);
    let changed = tx
        .execute(
            "UPDATE action_dispatches
             SET status=?1,
                 result_code=?2,
                 error=?3,
                 started_at=CASE
                    WHEN ?1='running' THEN COALESCE(started_at, CURRENT_TIMESTAMP)
                    ELSE started_at
                 END,
                 finished_at=CASE
                    WHEN ?4=1 THEN CURRENT_TIMESTAMP
                    ELSE finished_at
                 END
             WHERE id=?5 AND status=?6",
            params![
                next,
                result_code,
                error,
                terminal as i64,
                dispatch_id,
                current.0
            ],
        )
        .map_err(|cause| format!("更新动作状态失败: {cause}"))?;
    tx.commit()
        .map_err(|cause| format!("提交动作状态事务失败: {cause}"))?;
    Ok(changed == 1)
}

pub(crate) fn stop_pending_with_conn(
    conn: &mut Connection,
    dispatch_id: &str,
    outcome: &str,
    error: Option<&str>,
) -> Result<(), String> {
    if !matches!(outcome, STATUS_CANCELLED | STATUS_FAILED) {
        return Err("outcome 只允许 cancelled 或 failed".into());
    }
    let normalized_error = error.map(str::trim).filter(|value| !value.is_empty());
    if outcome == STATUS_FAILED && normalized_error.is_none() {
        return Err("failed outcome 必须提供 error".into());
    }
    let changed = transition_from_with_conn(
        conn,
        dispatch_id,
        outcome,
        Some(outcome),
        normalized_error,
        None,
        Some(STATUS_PENDING_CONFIRMATION),
    )?;
    if !changed {
        return Err("动作已进入运行态或已结束，不能通过确认取消接口停止".into());
    }
    Ok(())
}

pub(crate) fn associate_release_package_run_with_conn(
    conn: &mut Connection,
    dispatch_id: &str,
    run_id: &str,
    environment_id: i64,
) -> Result<(), String> {
    let dispatch_id = dispatch_id.trim();
    let run_id = run_id.trim();
    if dispatch_id.is_empty() {
        return Err("动作派发 id 不能为空".into());
    }
    if run_id.is_empty() {
        return Err("上线包 run id 不能为空".into());
    }
    if environment_id <= 0 {
        return Err("上线包环境 id 不合法".into());
    }

    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("开启上线包动作关联事务失败: {error}"))?;
    let (status, action_type, target_id, external_run_id): (
        String,
        String,
        String,
        Option<String>,
    ) = tx
        .query_row(
            "SELECT status, action_type, target_id, external_run_id
             FROM action_dispatches WHERE id=?1",
            [dispatch_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| format!("查询上线包动作派发失败: {error}"))?
        .ok_or("动作派发不存在")?;
    if status != STATUS_PENDING_CONFIRMATION {
        return Err("动作派发不在待确认状态".into());
    }
    if action_type != "release_package.run" {
        return Err("动作派发不是上线包打包动作".into());
    }
    if target_id != environment_id.to_string() {
        return Err("动作派发目标与上线包环境不匹配".into());
    }
    if external_run_id.is_some() {
        return Err("动作派发已关联上线包运行".into());
    }

    tx.execute(
        "UPDATE action_dispatches
         SET status='running', external_run_id=?1, started_at=CURRENT_TIMESTAMP,
             result_code=NULL, error=NULL
         WHERE id=?2 AND status='pending_confirmation' AND external_run_id IS NULL",
        params![run_id, dispatch_id],
    )
    .map_err(|error| format!("关联上线包运行失败: {error}"))?;
    tx.commit()
        .map_err(|error| format!("提交上线包动作关联失败: {error}"))?;
    Ok(())
}

pub(crate) fn finish_release_package_run_with_conn(
    conn: &mut Connection,
    run_id: &str,
    result_code: &str,
) -> Result<bool, String> {
    let run_id = run_id.trim();
    if run_id.is_empty() {
        return Err("上线包 run id 不能为空".into());
    }
    let dispatch_status = match result_code {
        "succeeded" => STATUS_SUCCEEDED,
        "cancelled" => STATUS_CANCELLED,
        "partially_succeeded" | "package_succeeded_upload_failed" | "failed" => STATUS_FAILED,
        "upload_succeeded_command_failed" => STATUS_FAILED,
        _ => return Err(format!("未知的上线包终态: {result_code}")),
    };

    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("开启上线包动作终态事务失败: {error}"))?;
    let dispatch: Option<(String, String, String)> = tx
        .query_row(
            "SELECT id, trigger_type, trigger_id
             FROM action_dispatches
             WHERE external_run_id=?1 AND status='running'
               AND action_type='release_package.run'",
            [run_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| format!("查询上线包动作运行失败: {error}"))?;
    let Some((dispatch_id, trigger_type, trigger_id)) = dispatch else {
        return Ok(false);
    };

    let changed = tx
        .execute(
            "UPDATE action_dispatches
             SET status=?1, result_code=?2, error=NULL, finished_at=CURRENT_TIMESTAMP
             WHERE id=?3 AND status='running' AND external_run_id=?4
               AND action_type='release_package.run'",
            params![dispatch_status, result_code, dispatch_id, run_id],
        )
        .map_err(|error| format!("更新上线包动作终态失败: {error}"))?;
    if changed != 1 {
        return Ok(false);
    }

    if result_code == "succeeded" && trigger_type == "todo_item" {
        let todo_id = trigger_id
            .parse::<i64>()
            .ok()
            .filter(|id| *id > 0)
            .ok_or("动作派发中的任务 id 不合法")?;
        let todo_exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM todo_items WHERE id=?1)",
                [todo_id],
                |row| row.get(0),
            )
            .map_err(|error| format!("查询动作关联任务失败: {error}"))?;
        if todo_exists {
            crate::tools::todo::change_item_status_with_conn(&tx, todo_id, "completed")?;
        }
    }

    tx.commit()
        .map_err(|error| format!("提交上线包动作终态失败: {error}"))?;
    Ok(true)
}

pub(crate) fn associate_release_package_run(
    dispatch_id: &str,
    run_id: &str,
    environment_id: i64,
) -> Result<(), String> {
    let mut conn = super::super::helpers::db_conn()?;
    associate_release_package_run_with_conn(&mut conn, dispatch_id, run_id, environment_id)
}

#[cfg(not(test))]
pub(crate) fn finish_release_package_run(
    run_id: &str,
    result_code: &str,
) -> Result<bool, String> {
    let mut conn = super::super::helpers::db_conn()?;
    finish_release_package_run_with_conn(&mut conn, run_id, result_code)
}

pub(crate) fn latest_dispatch_with_conn(
    conn: &Connection,
    trigger_type: &str,
    trigger_id: &str,
) -> Result<Option<DispatchSummary>, String> {
    conn.query_row(
        "SELECT id, trigger_type, trigger_id, action_type, target_id, status,
                result_code, error, created_at, started_at, finished_at
         FROM action_dispatches
         WHERE trigger_type=?1 AND trigger_id=?2
         ORDER BY created_at DESC, rowid DESC
         LIMIT 1",
        params![trigger_type, trigger_id],
        |row| {
            Ok(DispatchSummary {
                id: row.get(0)?,
                trigger_type: row.get(1)?,
                trigger_id: row.get(2)?,
                action_type: row.get(3)?,
                target_id: row.get(4)?,
                status: row.get(5)?,
                result_code: row.get(6)?,
                error: row.get(7)?,
                created_at: row.get(8)?,
                started_at: row.get(9)?,
                finished_at: row.get(10)?,
            })
        },
    )
    .optional()
    .map_err(|error| format!("查询最近动作派发失败: {error}"))
}

pub(crate) fn recover_interrupted_with_conn(conn: &mut Connection) -> Result<usize, String> {
    conn.execute(
        "UPDATE action_dispatches
         SET status='failed',
             result_code='interrupted',
             error='应用重启，动作执行已中断',
             finished_at=CURRENT_TIMESTAMP
         WHERE status IN ('pending_confirmation','running')",
        [],
    )
    .map_err(|error| format!("恢复中断动作失败: {error}"))
}

fn parse_required_string<'a>(payload: &'a Value, key: &str) -> Result<&'a str, String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{key} 不能为空"))
}

fn parse_create_request(payload: &Value) -> Result<CreateDispatchRequest, String> {
    let trigger_type = parse_required_string(payload, "triggerType")?.to_string();
    let trigger_id = parse_required_string(payload, "triggerId")?.to_string();
    let trigger_event_id = match payload.get("triggerEventId") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if !value.trim().is_empty() => Some(value.trim().to_string()),
        Some(_) => return Err("triggerEventId 必须是非空字符串".into()),
    };
    Ok(CreateDispatchRequest {
        trigger_type,
        trigger_id,
        trigger_event_id,
    })
}

pub(crate) fn dispatch_with_app(app: &tauri::AppHandle, payload: &Value) -> Result<Value, String> {
    let request = parse_create_request(payload)?;
    let mut conn = super::super::helpers::db_conn()?;
    let dispatch = create_dispatch_with_conn(&mut conn, &request)?;
    let action = definition(&dispatch.action_type)
        .ok_or_else(|| format!("动作类型不存在: {}", dispatch.action_type))?;
    let intent = ActionDispatchRequest {
        dispatch_id: dispatch.id.clone(),
        action_type: dispatch.action_type.clone(),
        target_tool_id: action.target_tool_id.to_string(),
        target_id: dispatch.target_id.clone(),
    };
    if let Err(error) =
        crate::navigate_main_window_to_tool(app, &intent.target_tool_id).and_then(|_| {
            app.emit(crate::events::EVENT_ACTION_CENTER_DISPATCH_REQUEST, &intent)
                .map_err(|cause| cause.to_string())
        })
    {
        fail_pending_dispatch(&dispatch.id, &error)?;
        return Err(error);
    }
    Ok(json!(dispatch))
}

pub(crate) fn dispatch_cancel(payload: &Value) -> Result<Value, String> {
    let dispatch_id = parse_required_string(payload, "dispatchId")?;
    let outcome = parse_required_string(payload, "outcome")?;
    let error = match payload.get("error") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.as_str()),
        Some(_) => return Err("error 必须是字符串".into()),
    };
    let mut conn = super::super::helpers::db_conn()?;
    stop_pending_with_conn(&mut conn, dispatch_id, outcome, error)?;
    Ok(json!({ "ok": true }))
}

pub(crate) fn dispatch_latest(payload: &Value) -> Result<Value, String> {
    let trigger_type = parse_required_string(payload, "triggerType")?;
    let trigger_id = parse_required_string(payload, "triggerId")?;
    let conn = super::super::helpers::db_conn()?;
    Ok(json!({
        "dispatch": latest_dispatch_with_conn(&conn, trigger_type, trigger_id)?,
    }))
}

pub(crate) fn fail_pending_dispatch(dispatch_id: &str, error: &str) -> Result<(), String> {
    let mut conn = super::super::helpers::db_conn()?;
    stop_pending_with_conn(&mut conn, dispatch_id, STATUS_FAILED, Some(error))
}

pub(crate) fn recover_interrupted_dispatches() -> Result<usize, String> {
    let mut conn = super::super::helpers::db_conn()?;
    recover_interrupted_with_conn(&mut conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection, OptionalExtension};

    fn seeded_action_conn() -> (Connection, i64, i64) {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE todo_items (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                status TEXT NOT NULL,
                kind TEXT NOT NULL,
                series_id INTEGER,
                completed_at TEXT,
                snooze_until TEXT,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE TABLE todo_reminder_events (
                id INTEGER PRIMARY KEY,
                task_id INTEGER NOT NULL,
                is_read INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE TABLE todo_item_reminders (
                id INTEGER PRIMARY KEY,
                item_id INTEGER NOT NULL,
                snooze_until TEXT,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             INSERT INTO todo_items(id, title, status, kind)
             VALUES(1, '发布客户门户', 'pending', 'one_off'),
                   (2, '其他任务', 'pending', 'one_off');",
        )
        .unwrap();
        crate::tools::release_package::ensure_schema(&conn).unwrap();
        crate::tools::action_center::ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO release_package_projects(
                id, name, frontend_project_path, backend_project_path
             ) VALUES(7, '客户门户', '', '')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO release_package_environments(project_id, environment)
             VALUES(7, 'test')",
            [],
        )
        .unwrap();
        let test_environment_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO release_package_environments(project_id, environment, output_root)
             VALUES(7, 'production', 'D:\\releases')",
            [],
        )
        .unwrap();
        let production_environment_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO action_bindings(
                id, trigger_type, trigger_id, action_type, target_id, enabled
             ) VALUES(1, 'todo_item', '1', 'release_package.run', ?1, 1)",
            [production_environment_id.to_string()],
        )
        .unwrap();
        (conn, test_environment_id, production_environment_id)
    }

    fn manual_request(todo_id: i64) -> CreateDispatchRequest {
        CreateDispatchRequest {
            trigger_type: "todo_item".into(),
            trigger_id: todo_id.to_string(),
            trigger_event_id: None,
        }
    }

    fn reminder_request(todo_id: i64, event_id: i64) -> CreateDispatchRequest {
        CreateDispatchRequest {
            trigger_type: "todo_item".into(),
            trigger_id: todo_id.to_string(),
            trigger_event_id: Some(event_id.to_string()),
        }
    }

    fn seed_reminder_event(conn: &Connection, event_id: i64, todo_id: i64) {
        conn.execute(
            "INSERT INTO todo_reminder_events(id, task_id, is_read) VALUES(?1, ?2, 0)",
            params![event_id, todo_id],
        )
        .unwrap();
    }

    fn seed_dispatch(
        conn: &Connection,
        id: &str,
        status: &str,
        run_id: Option<&str>,
        environment_id: i64,
    ) {
        conn.execute(
            "INSERT INTO action_dispatches(
                id, binding_id, trigger_type, trigger_id, trigger_event_id,
                action_type, target_id, status, external_run_id
             ) VALUES(?1, 1, 'todo_item', '1', ?1,
                      'release_package.run', ?2, ?3, ?4)",
            params![id, environment_id.to_string(), status, run_id],
        )
        .unwrap();
    }

    fn dispatch_state(conn: &Connection, id: &str) -> (String, Option<String>, Option<String>) {
        conn.query_row(
            "SELECT status, result_code, error FROM action_dispatches WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
    }

    fn dispatch_run_state(conn: &Connection, id: &str) -> (String, Option<String>, Option<String>) {
        conn.query_row(
            "SELECT status, external_run_id, started_at FROM action_dispatches WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
    }

    fn todo_state(conn: &Connection, id: i64) -> (String, Option<String>) {
        conn.query_row(
            "SELECT status, completed_at FROM todo_items WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
    }

    #[test]
    fn same_binding_can_only_have_one_active_dispatch() {
        let (mut conn, _, _) = seeded_action_conn();
        let first = create_dispatch_with_conn(&mut conn, &manual_request(1)).unwrap();
        let error = create_dispatch_with_conn(&mut conn, &manual_request(1)).unwrap_err();

        assert!(error.contains("已有待确认或进行中的动作"));
        assert_eq!(first.status, "pending_confirmation");
    }

    #[test]
    fn reminder_event_must_belong_to_the_trigger_todo() {
        let (mut conn, _, _) = seeded_action_conn();
        seed_reminder_event(&conn, 41, 2);

        let error = create_dispatch_with_conn(&mut conn, &reminder_request(1, 41)).unwrap_err();

        assert_eq!(error, "提醒事件与当前任务不匹配");
    }

    #[test]
    fn successful_reminder_dispatch_marks_event_read() {
        let (mut conn, _, _) = seeded_action_conn();
        seed_reminder_event(&conn, 41, 1);

        create_dispatch_with_conn(&mut conn, &reminder_request(1, 41)).unwrap();

        let is_read: i64 = conn
            .query_row(
                "SELECT is_read FROM todo_reminder_events WHERE id=41",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(is_read, 1);
    }

    #[test]
    fn invalid_target_and_completed_todo_are_rejected() {
        let (mut conn, test_environment_id, production_environment_id) = seeded_action_conn();
        conn.execute("DELETE FROM release_package_projects WHERE id=7", [])
            .unwrap();
        assert!(create_dispatch_with_conn(&mut conn, &manual_request(1))
            .unwrap_err()
            .contains("上线包配置不存在"));

        conn.execute(
            "INSERT INTO release_package_projects(
                id, name, frontend_project_path, backend_project_path
             ) VALUES(7, '客户门户', '', '')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO release_package_environments(id, project_id, environment)
             VALUES(?1, 7, 'test')",
            [test_environment_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO release_package_environments(id, project_id, environment, output_root)
             VALUES(?1, 7, 'production', 'D:\\releases')",
            [production_environment_id],
        )
        .unwrap();
        conn.execute("UPDATE todo_items SET status='completed' WHERE id=1", [])
            .unwrap();
        assert!(create_dispatch_with_conn(&mut conn, &manual_request(1))
            .unwrap_err()
            .contains("已完成"));
    }

    #[test]
    fn pending_dispatch_can_end_as_cancelled_or_failed_but_not_running_via_cancel_api() {
        let (mut conn, _, _) = seeded_action_conn();
        let dispatch = create_dispatch_with_conn(&mut conn, &manual_request(1)).unwrap();

        stop_pending_with_conn(&mut conn, &dispatch.id, "failed", Some("页面有未保存配置"))
            .unwrap();

        assert_eq!(
            dispatch_state(&conn, &dispatch.id),
            (
                "failed".into(),
                Some("failed".into()),
                Some("页面有未保存配置".into())
            )
        );
        assert!(stop_pending_with_conn(&mut conn, &dispatch.id, "running", None).is_err());

        let (mut running_conn, _, _) = seeded_action_conn();
        let running_dispatch =
            create_dispatch_with_conn(&mut running_conn, &manual_request(1)).unwrap();
        assert!(transition_with_conn(
            &mut running_conn,
            &running_dispatch.id,
            "running",
            None,
            None,
            None,
        )
        .unwrap());
        assert!(
            stop_pending_with_conn(&mut running_conn, &running_dispatch.id, "cancelled", None,)
                .is_err()
        );
        assert_eq!(
            dispatch_state(&running_conn, &running_dispatch.id).0,
            "running"
        );
    }

    #[test]
    fn repeated_terminal_and_wrong_run_id_do_not_rewrite_dispatch() {
        let (mut conn, _, production_environment_id) = seeded_action_conn();
        seed_dispatch(
            &conn,
            "running",
            "running",
            Some("run-1"),
            production_environment_id,
        );

        assert!(!transition_with_conn(
            &mut conn,
            "running",
            "succeeded",
            Some("succeeded"),
            None,
            Some("other-run"),
        )
        .unwrap());
        assert!(transition_with_conn(
            &mut conn,
            "running",
            "failed",
            Some("failed"),
            Some("首次失败"),
            Some("run-1"),
        )
        .unwrap());
        assert!(!transition_with_conn(
            &mut conn,
            "running",
            "cancelled",
            Some("cancelled"),
            Some("重复改写"),
            Some("run-1"),
        )
        .unwrap());
        assert_eq!(
            dispatch_state(&conn, "running"),
            (
                "failed".into(),
                Some("failed".into()),
                Some("首次失败".into())
            )
        );
    }

    #[test]
    fn release_package_run_association_requires_matching_pending_dispatch() {
        let (mut conn, test_environment_id, production_environment_id) = seeded_action_conn();
        let dispatch = create_dispatch_with_conn(&mut conn, &manual_request(1)).unwrap();

        assert_eq!(
            associate_release_package_run_with_conn(
                &mut conn,
                &dispatch.id,
                "run-wrong-target",
                test_environment_id,
            )
            .unwrap_err(),
            "动作派发目标与上线包环境不匹配"
        );
        assert_eq!(dispatch_run_state(&conn, &dispatch.id).0, "pending_confirmation");

        conn.execute(
            "UPDATE action_dispatches SET action_type='browser_profile.open' WHERE id=?1",
            [&dispatch.id],
        )
        .unwrap();
        assert!(associate_release_package_run_with_conn(
            &mut conn,
            &dispatch.id,
            "run-wrong-action",
            production_environment_id,
        )
        .unwrap_err()
        .contains("动作"));
        conn.execute(
            "UPDATE action_dispatches SET action_type='release_package.run' WHERE id=?1",
            [&dispatch.id],
        )
        .unwrap();

        associate_release_package_run_with_conn(
            &mut conn,
            &dispatch.id,
            "run-1",
            production_environment_id,
        )
        .unwrap();
        let state = dispatch_run_state(&conn, &dispatch.id);
        assert_eq!(state.0, "running");
        assert_eq!(state.1.as_deref(), Some("run-1"));
        assert!(state.2.is_some());
        assert!(associate_release_package_run_with_conn(
            &mut conn,
            &dispatch.id,
            "run-2",
            production_environment_id,
        )
        .is_err());
    }

    #[test]
    fn only_full_release_package_success_completes_the_todo() {
        for (result_code, expected_dispatch, expected_todo) in [
            ("succeeded", "succeeded", "completed"),
            ("partially_succeeded", "failed", "pending"),
            ("package_succeeded_upload_failed", "failed", "pending"),
            ("upload_succeeded_command_failed", "failed", "pending"),
            ("failed", "failed", "pending"),
            ("cancelled", "cancelled", "pending"),
        ] {
            let (mut conn, _, production_environment_id) = seeded_action_conn();
            let dispatch = create_dispatch_with_conn(&mut conn, &manual_request(1)).unwrap();
            associate_release_package_run_with_conn(
                &mut conn,
                &dispatch.id,
                "run-1",
                production_environment_id,
            )
            .unwrap();

            assert!(finish_release_package_run_with_conn(&mut conn, "run-1", result_code)
                .unwrap());
            assert_eq!(dispatch_state(&conn, &dispatch.id).0, expected_dispatch);
            assert_eq!(todo_state(&conn, 1).0, expected_todo);
        }
    }

    #[test]
    fn wrong_or_repeated_run_terminal_is_idempotent() {
        let (mut conn, _, production_environment_id) = seeded_action_conn();
        let dispatch = create_dispatch_with_conn(&mut conn, &manual_request(1)).unwrap();
        associate_release_package_run_with_conn(
            &mut conn,
            &dispatch.id,
            "run-1",
            production_environment_id,
        )
        .unwrap();

        assert!(!finish_release_package_run_with_conn(&mut conn, "other-run", "succeeded")
            .unwrap());
        assert!(finish_release_package_run_with_conn(&mut conn, "run-1", "succeeded").unwrap());
        let completed_at = todo_state(&conn, 1).1;
        assert!(completed_at.is_some());
        assert!(!finish_release_package_run_with_conn(&mut conn, "run-1", "failed").unwrap());
        assert_eq!(todo_state(&conn, 1).1, completed_at);
    }

    #[test]
    fn release_package_terminal_ignores_other_action_runs() {
        let (mut conn, _, production_environment_id) = seeded_action_conn();
        let dispatch = create_dispatch_with_conn(&mut conn, &manual_request(1)).unwrap();
        associate_release_package_run_with_conn(
            &mut conn,
            &dispatch.id,
            "run-1",
            production_environment_id,
        )
        .unwrap();
        conn.execute(
            "UPDATE action_dispatches SET action_type='development_environment.start' WHERE id=?1",
            [&dispatch.id],
        )
        .unwrap();

        assert!(!finish_release_package_run_with_conn(&mut conn, "run-1", "succeeded").unwrap());
        assert_eq!(dispatch_state(&conn, &dispatch.id).0, "running");
        assert_eq!(todo_state(&conn, 1).0, "pending");
    }

    #[test]
    fn success_preserves_manual_completion_and_tolerates_deleted_todo() {
        let (mut completed_conn, _, production_environment_id) = seeded_action_conn();
        let dispatch = create_dispatch_with_conn(&mut completed_conn, &manual_request(1)).unwrap();
        associate_release_package_run_with_conn(
            &mut completed_conn,
            &dispatch.id,
            "run-1",
            production_environment_id,
        )
        .unwrap();
        completed_conn
            .execute(
                "UPDATE todo_items SET status='completed', completed_at='2026-07-25 12:00:00' WHERE id=1",
                [],
            )
            .unwrap();
        finish_release_package_run_with_conn(&mut completed_conn, "run-1", "succeeded")
            .unwrap();
        assert_eq!(
            todo_state(&completed_conn, 1),
            ("completed".into(), Some("2026-07-25 12:00:00".into()))
        );

        let (mut deleted_conn, _, production_environment_id) = seeded_action_conn();
        let deleted_dispatch =
            create_dispatch_with_conn(&mut deleted_conn, &manual_request(1)).unwrap();
        associate_release_package_run_with_conn(
            &mut deleted_conn,
            &deleted_dispatch.id,
            "run-2",
            production_environment_id,
        )
        .unwrap();
        deleted_conn.execute("DELETE FROM todo_items WHERE id=1", []).unwrap();
        assert!(finish_release_package_run_with_conn(&mut deleted_conn, "run-2", "succeeded")
            .unwrap());
        assert_eq!(dispatch_state(&deleted_conn, &deleted_dispatch.id).0, "succeeded");
    }

    #[test]
    fn deleting_binding_keeps_historical_dispatch() {
        let (mut conn, _, _) = seeded_action_conn();
        let dispatch = create_dispatch_with_conn(&mut conn, &manual_request(1)).unwrap();
        stop_pending_with_conn(&mut conn, &dispatch.id, "cancelled", None).unwrap();

        conn.execute("DELETE FROM action_bindings WHERE id=1", [])
            .unwrap();

        let binding_id: Option<i64> = conn
            .query_row(
                "SELECT binding_id FROM action_dispatches WHERE id=?1",
                [&dispatch.id],
                |row| row.get(0),
            )
            .optional()
            .unwrap()
            .flatten();
        assert_eq!(binding_id, None);
    }

    #[test]
    fn startup_recovery_marks_orphaned_active_dispatches_interrupted() {
        let (mut conn, _, production_environment_id) = seeded_action_conn();
        seed_dispatch(
            &conn,
            "pending",
            "pending_confirmation",
            None,
            production_environment_id,
        );
        conn.execute(
            "UPDATE action_dispatches SET status='failed', result_code='old'
             WHERE id='pending'",
            [],
        )
        .unwrap();
        seed_dispatch(
            &conn,
            "pending-2",
            "pending_confirmation",
            None,
            production_environment_id,
        );
        conn.execute(
            "UPDATE action_dispatches SET binding_id=NULL WHERE id='pending-2'",
            [],
        )
        .unwrap();
        seed_dispatch(
            &conn,
            "running",
            "running",
            Some("run-1"),
            production_environment_id,
        );

        assert_eq!(recover_interrupted_with_conn(&mut conn).unwrap(), 2);
        for id in ["pending-2", "running"] {
            let (status, result_code, error) = dispatch_state(&conn, id);
            assert_eq!(status, "failed");
            assert_eq!(result_code.as_deref(), Some("interrupted"));
            assert!(error.unwrap().contains("应用重启"));
        }
        assert_eq!(
            dispatch_state(&conn, "pending"),
            ("failed".into(), Some("old".into()), None)
        );
    }
}

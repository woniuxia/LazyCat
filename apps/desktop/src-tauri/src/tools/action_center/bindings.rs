use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Map, Value};

use super::definitions::{definition, RELEASE_PACKAGE_RUN};

const TODO_TRIGGER_TYPE: &str = "todo_item";
const ONE_OFF_KIND: &str = "one_off";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BindingPatch {
    Preserve,
    Remove,
    Set {
        action_type: String,
        target_id: String,
    },
}

#[derive(Clone, Debug)]
struct BindingRow {
    id: i64,
    action_type: String,
    target_id: String,
}

struct BindingPresentation {
    action_label: String,
    target_label: String,
    available: bool,
    unavailable_reason: Option<String>,
}

pub(crate) fn parse_binding_patch(payload: &Value) -> Result<BindingPatch, String> {
    match payload.get("actionBinding") {
        None => Ok(BindingPatch::Preserve),
        Some(Value::Null) => Ok(BindingPatch::Remove),
        Some(Value::Object(value)) => {
            let action_type = value
                .get("actionType")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or("actionBinding.actionType 不能为空")?;
            let target_id = value
                .get("targetId")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or("actionBinding.targetId 不能为空")?;
            Ok(BindingPatch::Set {
                action_type: action_type.into(),
                target_id: target_id.into(),
            })
        }
        Some(_) => Err("actionBinding 必须是对象或 null".into()),
    }
}

fn load_todo_binding(conn: &Connection, item_id: i64) -> Result<Option<BindingRow>, String> {
    conn.query_row(
        "SELECT id, action_type, target_id
         FROM action_bindings
         WHERE trigger_type=?1 AND trigger_id=?2",
        params![TODO_TRIGGER_TYPE, item_id.to_string()],
        |row| {
            Ok(BindingRow {
                id: row.get(0)?,
                action_type: row.get(1)?,
                target_id: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(|error| format!("查询事项动作绑定失败: {error}"))
}

fn has_active_dispatch(conn: &Connection, binding_id: i64) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM action_dispatches
            WHERE binding_id=?1 AND status IN ('pending_confirmation','running')
         )",
        [binding_id],
        |row| row.get(0),
    )
    .map_err(|error| format!("检查事项动作执行状态失败: {error}"))
}

pub(crate) fn validate_binding_target(
    conn: &Connection,
    action_type: &str,
    target_id: &str,
) -> Result<(), String> {
    let action = definition(action_type).ok_or_else(|| format!("动作类型不存在: {action_type}"))?;
    if !action.trigger_types.contains(&TODO_TRIGGER_TYPE) {
        return Err("该动作不支持由事项触发".into());
    }

    match action_type {
        RELEASE_PACKAGE_RUN => {
            let project_id = target_id
                .parse::<i64>()
                .ok()
                .filter(|id| *id > 0)
                .ok_or("上线包配置不存在")?;
            if super::super::release_package::load_action_target_label(conn, project_id)?.is_none()
            {
                return Err("上线包配置不存在".into());
            }
            Ok(())
        }
        _ => Err(format!("动作目标适配器不存在: {action_type}")),
    }
}

pub(crate) fn apply_todo_binding_patch(
    conn: &Connection,
    item_id: i64,
    item_kind: &str,
    patch: BindingPatch,
    _is_create: bool,
) -> Result<(), String> {
    let existing = load_todo_binding(conn, item_id)?;

    match patch {
        BindingPatch::Preserve => {
            if item_kind != ONE_OFF_KIND && existing.is_some() {
                return Err("周期事项暂不支持执行动作".into());
            }
            Ok(())
        }
        BindingPatch::Remove => {
            let Some(binding) = existing else {
                return Ok(());
            };
            if has_active_dispatch(conn, binding.id)? {
                return Err("动作正在执行，暂不能修改绑定".into());
            }
            conn.execute("DELETE FROM action_bindings WHERE id=?1", [binding.id])
                .map_err(|error| format!("解除事项动作绑定失败: {error}"))?;
            Ok(())
        }
        BindingPatch::Set {
            action_type,
            target_id,
        } => {
            if item_kind != ONE_OFF_KIND {
                return Err("周期事项暂不支持执行动作".into());
            }
            validate_binding_target(conn, &action_type, &target_id)?;
            if let Some(binding) = &existing {
                if binding.action_type == action_type && binding.target_id == target_id {
                    return Ok(());
                }
                if has_active_dispatch(conn, binding.id)? {
                    return Err("动作正在执行，暂不能修改绑定".into());
                }
            }
            conn.execute(
                "INSERT INTO action_bindings(
                    trigger_type, trigger_id, action_type, target_id, enabled
                 ) VALUES(?1, ?2, ?3, ?4, 1)
                 ON CONFLICT(trigger_type, trigger_id) DO UPDATE SET
                    action_type=excluded.action_type,
                    target_id=excluded.target_id,
                    enabled=1,
                    updated_at=CURRENT_TIMESTAMP",
                params![
                    TODO_TRIGGER_TYPE,
                    item_id.to_string(),
                    action_type,
                    target_id
                ],
            )
            .map_err(|error| format!("保存事项动作绑定失败: {error}"))?;
            Ok(())
        }
    }
}

pub(crate) fn ensure_todo_can_become_recurring(
    conn: &Connection,
    item_id: i64,
    patch: &BindingPatch,
) -> Result<(), String> {
    if matches!(patch, BindingPatch::Set { .. }) || load_todo_binding(conn, item_id)?.is_some() {
        return Err("请先解除动作绑定，再改为周期事项".into());
    }
    Ok(())
}

fn binding_presentation(
    conn: &Connection,
    binding: &BindingRow,
) -> Result<BindingPresentation, String> {
    let action = definition(&binding.action_type);
    let action_label = action
        .as_ref()
        .map(|value| value.label)
        .unwrap_or(binding.action_type.as_str())
        .to_string();

    let (target_label, available, unavailable_reason) = match binding.action_type.as_str() {
        RELEASE_PACKAGE_RUN => {
            let project_id = binding.target_id.parse::<i64>().ok().filter(|id| *id > 0);
            let label = match project_id {
                Some(id) => super::super::release_package::load_action_target_label(conn, id)?,
                None => None,
            };
            match label {
                Some(label) => (label, true, None),
                None => (
                    format!("配置 #{}", binding.target_id),
                    false,
                    Some("上线包配置不存在".to_string()),
                ),
            }
        }
        _ => (
            binding.target_id.clone(),
            false,
            Some("动作定义不存在".to_string()),
        ),
    };

    Ok(BindingPresentation {
        action_label,
        target_label,
        available,
        unavailable_reason,
    })
}

fn binding_summary(conn: &Connection, binding: BindingRow) -> Result<Value, String> {
    let presentation = binding_presentation(conn, &binding)?;

    let mut summary = Map::new();
    summary.insert("id".into(), json!(binding.id));
    summary.insert("actionType".into(), json!(binding.action_type));
    summary.insert("actionLabel".into(), json!(presentation.action_label));
    summary.insert("targetId".into(), json!(binding.target_id));
    summary.insert("targetLabel".into(), json!(presentation.target_label));
    summary.insert("available".into(), json!(presentation.available));
    if let Some(reason) = presentation.unavailable_reason {
        summary.insert("unavailableReason".into(), json!(reason));
    }
    Ok(Value::Object(summary))
}

pub(crate) fn todo_reminder_action_summary(
    conn: &Connection,
    item_id: i64,
) -> Result<Option<crate::tools::todo::ReminderActionSummary>, String> {
    let binding = conn
        .query_row(
            "SELECT id, action_type, target_id
             FROM action_bindings
             WHERE trigger_type='todo_item' AND trigger_id=?1 AND enabled=1",
            [item_id.to_string()],
            |row| {
                Ok(BindingRow {
                    id: row.get(0)?,
                    action_type: row.get(1)?,
                    target_id: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("查询提醒动作绑定失败: {error}"))?;
    let Some(binding) = binding else {
        return Ok(None);
    };
    let presentation = binding_presentation(conn, &binding)?;
    let active_dispatch_status = conn
        .query_row(
            "SELECT status
             FROM action_dispatches
             WHERE binding_id=?1 AND status IN ('pending_confirmation','running')
             ORDER BY created_at DESC
             LIMIT 1",
            [binding.id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("查询提醒动作状态失败: {error}"))?;

    Ok(Some(crate::tools::todo::ReminderActionSummary {
        binding_id: binding.id,
        action_type: binding.action_type,
        action_label: presentation.action_label,
        target_label: presentation.target_label,
        available: presentation.available,
        unavailable_reason: presentation.unavailable_reason,
        active_dispatch_status,
    }))
}

pub(crate) fn get_binding_summary(
    conn: &Connection,
    trigger_type: &str,
    trigger_id: &str,
) -> Result<Option<Value>, String> {
    let binding = conn
        .query_row(
            "SELECT id, action_type, target_id
             FROM action_bindings
             WHERE trigger_type=?1 AND trigger_id=?2 AND enabled=1",
            params![trigger_type, trigger_id],
            |row| {
                Ok(BindingRow {
                    id: row.get(0)?,
                    action_type: row.get(1)?,
                    target_id: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("查询动作绑定失败: {error}"))?;
    binding
        .map(|value| binding_summary(conn, value))
        .transpose()
}

pub(crate) fn attach_todo_binding_summaries(
    conn: &Connection,
    items: &mut [Value],
) -> Result<(), String> {
    for item in items {
        let Some(item_id) = item.get("id").and_then(Value::as_i64) else {
            continue;
        };
        let summary = get_binding_summary(conn, TODO_TRIGGER_TYPE, &item_id.to_string())?;
        if let Some(object) = item.as_object_mut() {
            object.insert("actionBinding".into(), summary.unwrap_or(Value::Null));
        }
    }
    Ok(())
}

pub(crate) fn delete_todo_binding(conn: &Connection, item_id: i64) -> Result<(), String> {
    let Some(binding) = load_todo_binding(conn, item_id)? else {
        return Ok(());
    };
    if has_active_dispatch(conn, binding.id)? {
        return Err("动作正在执行，暂不能删除事项".into());
    }
    conn.execute("DELETE FROM action_bindings WHERE id=?1", [binding.id])
        .map_err(|error| format!("删除事项动作绑定失败: {error}"))?;
    Ok(())
}

pub(crate) fn binding_get(payload: &Value) -> Result<Value, String> {
    let trigger_type = payload
        .get("triggerType")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("triggerType 不能为空")?;
    let trigger_id = payload
        .get("triggerId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("triggerId 不能为空")?;
    let conn = super::super::helpers::db_conn()?;
    Ok(json!({
        "binding": get_binding_summary(&conn, trigger_type, trigger_id)?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};
    use serde_json::json;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::tools::release_package::ensure_schema(&conn).unwrap();
        crate::tools::action_center::ensure_schema(&conn).unwrap();
        conn.execute_batch(
            "CREATE TABLE todo_items (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                kind TEXT NOT NULL
            );
            INSERT INTO todo_items(id, title, kind) VALUES(1, '发布', 'one_off');",
        )
        .unwrap();
        conn
    }

    fn seed_project(conn: &Connection, id: i64, name: &str) {
        conn.execute(
            "INSERT INTO release_package_projects(
                id, name, output_root, frontend_project_path, frontend_build_command,
                frontend_artifact_path, frontend_artifact_mode, backend_project_path,
                backend_build_command, backend_artifact_path
             ) VALUES (?1, ?2, '', '', '', '', 'copy_directory', '', '', '')",
            params![id, name],
        )
        .unwrap();
    }

    fn binding_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM action_bindings", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn binding_patch_preserves_missing_and_distinguishes_null() {
        assert!(matches!(
            parse_binding_patch(&json!({})).unwrap(),
            BindingPatch::Preserve
        ));
        assert!(matches!(
            parse_binding_patch(&json!({ "actionBinding": null })).unwrap(),
            BindingPatch::Remove
        ));
        assert!(matches!(
            parse_binding_patch(&json!({
                "actionBinding": {
                    "actionType": " release_package.run ",
                    "targetId": " 7 "
                }
            }))
            .unwrap(),
            BindingPatch::Set { action_type, target_id }
                if action_type == "release_package.run" && target_id == "7"
        ));
    }

    #[test]
    fn binding_patch_create_update_and_remove_follow_three_state_semantics() {
        let conn = test_conn();
        seed_project(&conn, 7, "客户门户");
        seed_project(&conn, 8, "管理后台");

        apply_todo_binding_patch(&conn, 1, "one_off", BindingPatch::Preserve, true).unwrap();
        assert_eq!(binding_count(&conn), 0);

        apply_todo_binding_patch(
            &conn,
            1,
            "one_off",
            BindingPatch::Set {
                action_type: "release_package.run".into(),
                target_id: "7".into(),
            },
            false,
        )
        .unwrap();
        apply_todo_binding_patch(&conn, 1, "one_off", BindingPatch::Preserve, false).unwrap();
        let target_id: String = conn
            .query_row(
                "SELECT target_id FROM action_bindings WHERE trigger_type='todo_item' AND trigger_id='1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(target_id, "7");

        apply_todo_binding_patch(
            &conn,
            1,
            "one_off",
            BindingPatch::Set {
                action_type: "release_package.run".into(),
                target_id: "8".into(),
            },
            false,
        )
        .unwrap();
        apply_todo_binding_patch(&conn, 1, "one_off", BindingPatch::Remove, false).unwrap();
        assert_eq!(binding_count(&conn), 0);
    }

    #[test]
    fn active_dispatch_blocks_binding_replacement_and_removal() {
        let conn = test_conn();
        seed_project(&conn, 7, "客户门户");
        seed_project(&conn, 8, "管理后台");
        apply_todo_binding_patch(
            &conn,
            1,
            "one_off",
            BindingPatch::Set {
                action_type: "release_package.run".into(),
                target_id: "7".into(),
            },
            false,
        )
        .unwrap();
        let binding_id: i64 = conn
            .query_row("SELECT id FROM action_bindings", [], |row| row.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO action_dispatches(
                id, binding_id, trigger_type, trigger_id, trigger_event_id,
                action_type, target_id, status
             ) VALUES('dispatch-1', ?1, 'todo_item', '1', 'manual-1',
                      'release_package.run', '7', 'running')",
            [binding_id],
        )
        .unwrap();

        let replace_error = apply_todo_binding_patch(
            &conn,
            1,
            "one_off",
            BindingPatch::Set {
                action_type: "release_package.run".into(),
                target_id: "8".into(),
            },
            false,
        )
        .unwrap_err();
        assert!(replace_error.contains("正在执行"));
        let remove_error =
            apply_todo_binding_patch(&conn, 1, "one_off", BindingPatch::Remove, false).unwrap_err();
        assert!(remove_error.contains("正在执行"));
    }

    #[test]
    fn deleted_target_is_attached_as_unavailable_summary() {
        let conn = test_conn();
        seed_project(&conn, 7, "客户门户");
        apply_todo_binding_patch(
            &conn,
            1,
            "one_off",
            BindingPatch::Set {
                action_type: "release_package.run".into(),
                target_id: "7".into(),
            },
            false,
        )
        .unwrap();
        conn.execute("DELETE FROM release_package_projects WHERE id=7", [])
            .unwrap();
        let mut items = vec![json!({ "id": 1, "title": "发布" })];

        attach_todo_binding_summaries(&conn, &mut items).unwrap();

        assert_eq!(items[0]["actionBinding"]["available"], false);
        assert_eq!(
            items[0]["actionBinding"]["unavailableReason"],
            "上线包配置不存在"
        );
        assert_eq!(items[0]["actionBinding"]["targetLabel"], "配置 #7");
    }
}

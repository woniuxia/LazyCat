use rusqlite::Connection;
use serde::Serialize;

pub(crate) const RELEASE_PACKAGE_RUN: &str = "release_package.run";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActionDefinition {
    pub action_type: &'static str,
    pub label: &'static str,
    pub trigger_types: &'static [&'static str],
    pub target_kind: &'static str,
    pub target_tool_id: &'static str,
    pub execution_mode: &'static str,
    pub completion_policy: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActionTargetOption {
    pub id: String,
    pub label: String,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
}

pub(crate) fn definition(action_type: &str) -> Option<ActionDefinition> {
    match action_type {
        RELEASE_PACKAGE_RUN => Some(ActionDefinition {
            action_type: RELEASE_PACKAGE_RUN,
            label: "开始打包",
            trigger_types: &["todo_item"],
            target_kind: "release_package_project",
            target_tool_id: "release-package",
            execution_mode: "open_and_confirm",
            completion_policy: "on_succeeded",
        }),
        _ => None,
    }
}

pub(crate) fn all_definitions() -> Vec<ActionDefinition> {
    vec![definition(RELEASE_PACKAGE_RUN).expect("registered release package action")]
}

pub(crate) fn list_targets(
    conn: &Connection,
    action_type: &str,
) -> Result<Vec<ActionTargetOption>, String> {
    if definition(action_type).is_none() {
        return Err(format!("动作类型不存在: {action_type}"));
    }
    match action_type {
        RELEASE_PACKAGE_RUN => {
            super::super::release_package::list_action_target_rows(conn).map(|rows| {
                rows.into_iter()
                    .map(|(id, label)| ActionTargetOption {
                        id: id.to_string(),
                        label,
                        available: true,
                        unavailable_reason: None,
                    })
                    .collect()
            })
        }
        _ => unreachable!("definition registry and target adapter must stay in sync"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::tools::release_package::ensure_schema(&conn).unwrap();
        conn
    }

    fn seed_release_project(conn: &Connection, id: i64, name: &str) {
        conn.execute(
            "INSERT INTO release_package_projects(
                id, name, output_root, frontend_project_path, frontend_build_command,
                frontend_artifact_path, frontend_artifact_mode, backend_project_path,
                backend_build_command, backend_artifact_path
             ) VALUES (?1, ?2, '', '', '', '', 'copy_directory', '', '', '')",
            rusqlite::params![id, name],
        )
        .unwrap();
    }

    #[test]
    fn release_package_definition_is_registered() {
        let definition = definition("release_package.run").expect("registered action");
        assert_eq!(definition.trigger_types, &["todo_item"]);
        assert_eq!(definition.target_kind, "release_package_project");
        assert_eq!(definition.target_tool_id, "release-package");
        assert_eq!(definition.execution_mode, "open_and_confirm");
        assert_eq!(definition.completion_policy, "on_succeeded");
    }

    #[test]
    fn release_package_targets_only_return_saved_projects() {
        let conn = test_conn();
        seed_release_project(&conn, 7, "客户门户");
        assert_eq!(
            list_targets(&conn, "release_package.run").unwrap(),
            vec![ActionTargetOption {
                id: "7".into(),
                label: "客户门户".into(),
                available: true,
                unavailable_reason: None,
            }]
        );
    }
}

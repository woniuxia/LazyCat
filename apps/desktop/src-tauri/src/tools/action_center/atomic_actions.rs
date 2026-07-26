use rusqlite::Connection;

use super::definitions::{
    definition, ActionTargetOption, BROWSER_PROFILE_LAUNCH, HOSTS_ACTIVATE, REQUEST_FORWARD_START,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AtomicTargetSnapshot {
    pub action_label: String,
    pub target_label: String,
    pub validation_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AtomicStepSuccessStatus {
    Succeeded,
    AlreadySatisfied,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AtomicStepSuccess {
    pub status: AtomicStepSuccessStatus,
    pub result_code: Option<String>,
    pub message: Option<String>,
}

impl AtomicStepSuccess {
    pub(crate) fn succeeded(message: Option<String>) -> Self {
        Self {
            status: AtomicStepSuccessStatus::Succeeded,
            result_code: None,
            message,
        }
    }

    pub(crate) fn from_changed(changed: bool) -> Self {
        if changed {
            Self::succeeded(None)
        } else {
            Self {
                status: AtomicStepSuccessStatus::AlreadySatisfied,
                result_code: None,
                message: None,
            }
        }
    }
}

pub(crate) trait AtomicActionExecutor: Send + Sync + 'static {
    fn execute(&self, action_type: &str, target_id: &str) -> Result<AtomicStepSuccess, String>;
}

pub(crate) fn normalize_atomic_failure(
    action_type: &str,
    error: String,
) -> (Option<String>, String) {
    if action_type == REQUEST_FORWARD_START {
        if let Some(error) = crate::tools::request_forward::decode_action_error(&error) {
            return (Some(error.result_code), error.message);
        }
    }
    (None, error)
}

pub(crate) struct RegisteredAtomicActionExecutor;

impl AtomicActionExecutor for RegisteredAtomicActionExecutor {
    fn execute(&self, action_type: &str, target_id: &str) -> Result<AtomicStepSuccess, String> {
        match action_type {
            HOSTS_ACTIVATE => crate::tools::hosts::activate_action_target(target_id)
                .map(AtomicStepSuccess::from_changed),
            BROWSER_PROFILE_LAUNCH => {
                crate::tools::browser_profiles::launch_action_target(target_id)
                    .map(AtomicStepSuccess::succeeded)
            }
            REQUEST_FORWARD_START => crate::tools::request_forward::start_action_target(target_id)
                .map(AtomicStepSuccess::from_changed),
            _ => Err(format!("组合动作类型不存在: {action_type}")),
        }
    }
}

type BrowserActionTarget = (String, String, bool, Option<String>);
type BrowserTargetLoader = fn() -> Result<Vec<BrowserActionTarget>, String>;

fn composable_definition(
    action_type: &str,
) -> Result<super::definitions::ActionDefinition, String> {
    let definition =
        definition(action_type).ok_or_else(|| format!("动作类型不存在: {action_type}"))?;
    if !definition.supports_combination {
        return Err(format!("动作不支持组合: {action_type}"));
    }
    Ok(definition)
}

fn target_option(
    id: String,
    label: String,
    available: bool,
    unavailable_reason: Option<String>,
) -> ActionTargetOption {
    ActionTargetOption {
        id,
        label,
        available,
        unavailable_reason,
    }
}

fn parse_numeric_target_id(target_id: &str) -> Result<i64, String> {
    target_id
        .parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| format!("目标 ID 无效: {target_id}"))
}

pub(super) fn list_targets_with_conn(
    conn: &Connection,
    action_type: &str,
) -> Result<Vec<ActionTargetOption>, String> {
    composable_definition(action_type)?;
    match action_type {
        HOSTS_ACTIVATE => crate::tools::hosts::list_action_targets_with_conn(conn).map(|rows| {
            rows.into_iter()
                .map(|(id, label)| target_option(id, label, true, None))
                .collect()
        }),
        BROWSER_PROFILE_LAUNCH => {
            crate::tools::browser_profiles::list_action_targets().map(|rows| {
                rows.into_iter()
                    .map(|(id, label, available, reason)| {
                        target_option(id, label, available, reason)
                    })
                    .collect()
            })
        }
        REQUEST_FORWARD_START => crate::tools::request_forward::list_action_targets_with_conn(conn)
            .map(|rows| {
                rows.into_iter()
                    .map(|(id, label)| target_option(id, label, true, None))
                    .collect()
            }),
        _ => unreachable!("combination action registry and target adapters must stay in sync"),
    }
}

pub(crate) fn list_targets(action_type: &str) -> Result<Vec<ActionTargetOption>, String> {
    composable_definition(action_type)?;
    if action_type == BROWSER_PROFILE_LAUNCH {
        return crate::tools::browser_profiles::list_action_targets().map(|rows| {
            rows.into_iter()
                .map(|(id, label, available, reason)| target_option(id, label, available, reason))
                .collect()
        });
    }
    let conn = crate::tools::helpers::db_conn()?;
    list_targets_with_conn(&conn, action_type)
}

fn resolve_target_with_conn(
    conn: &Connection,
    action_type: &str,
    target_id: &str,
    load_browser_targets: BrowserTargetLoader,
) -> Result<Option<ActionTargetOption>, String> {
    composable_definition(action_type)?;
    match action_type {
        HOSTS_ACTIVATE => {
            let id = parse_numeric_target_id(target_id)?;
            crate::tools::hosts::load_action_target_with_conn(conn, id).map(|target| {
                target.map(|(label, _)| target_option(id.to_string(), label, true, None))
            })
        }
        BROWSER_PROFILE_LAUNCH => {
            let expected = crate::tools::browser_profiles::decode_action_target(target_id)?;
            for (id, label, available, unavailable_reason) in load_browser_targets()? {
                let target = target_option(id, label, available, unavailable_reason);
                if crate::tools::browser_profiles::decode_action_target(&target.id)? == expected {
                    return Ok(Some(target));
                }
            }
            Ok(None)
        }
        REQUEST_FORWARD_START => {
            let id = parse_numeric_target_id(target_id)?;
            crate::tools::request_forward::load_action_target_with_conn(conn, id)
                .map(|target| target.map(|label| target_option(id.to_string(), label, true, None)))
        }
        _ => unreachable!("combination action registry and target adapters must stay in sync"),
    }
}

pub(super) fn validate_target_with_conn(
    conn: &Connection,
    action_type: &str,
    target_id: &str,
) -> Result<ActionTargetOption, String> {
    validate_target_with_conn_using_browser_targets(
        conn,
        action_type,
        target_id,
        crate::tools::browser_profiles::list_action_targets,
    )
}

fn validate_target_with_conn_using_browser_targets(
    conn: &Connection,
    action_type: &str,
    target_id: &str,
    load_browser_targets: BrowserTargetLoader,
) -> Result<ActionTargetOption, String> {
    let target = resolve_target_with_conn(conn, action_type, target_id, load_browser_targets)?
        .ok_or_else(|| format!("目标不存在: {target_id}"))?;
    if !target.available {
        return Err(target
            .unavailable_reason
            .clone()
            .unwrap_or_else(|| format!("目标不可用: {target_id}")));
    }
    Ok(target)
}

pub(super) fn snapshot_target_with_conn(
    conn: &Connection,
    action_type: &str,
    target_id: &str,
) -> AtomicTargetSnapshot {
    snapshot_target_with_conn_using_browser_targets(
        conn,
        action_type,
        target_id,
        crate::tools::browser_profiles::list_action_targets,
    )
}

fn snapshot_target_with_conn_using_browser_targets(
    conn: &Connection,
    action_type: &str,
    target_id: &str,
    load_browser_targets: BrowserTargetLoader,
) -> AtomicTargetSnapshot {
    let action_label = definition(action_type)
        .map(|definition| definition.label.to_string())
        .unwrap_or_else(|| action_type.to_string());
    match resolve_target_with_conn(conn, action_type, target_id, load_browser_targets) {
        Ok(Some(target)) => AtomicTargetSnapshot {
            action_label,
            target_label: target.label,
            validation_error: if target.available {
                None
            } else {
                Some(
                    target
                        .unavailable_reason
                        .unwrap_or_else(|| format!("目标不可用: {target_id}")),
                )
            },
        },
        Ok(None) => AtomicTargetSnapshot {
            action_label,
            target_label: target_id.to_string(),
            validation_error: Some(format!("目标不存在: {target_id}")),
        },
        Err(error) => AtomicTargetSnapshot {
            action_label,
            target_label: target_id.to_string(),
            validation_error: Some(error),
        },
    }
}

pub(crate) fn snapshot_target(action_type: &str, target_id: &str) -> AtomicTargetSnapshot {
    match crate::tools::helpers::db_conn() {
        Ok(conn) => snapshot_target_with_conn(&conn, action_type, target_id),
        Err(error) => AtomicTargetSnapshot {
            action_label: definition(action_type)
                .map(|definition| definition.label.to_string())
                .unwrap_or_else(|| action_type.to_string()),
            target_label: target_id.to_string(),
            validation_error: Some(error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::action_center::definitions::{
        all_definitions, combination_definitions, definition, BROWSER_PROFILE_LAUNCH,
        HOSTS_ACTIVATE, RELEASE_PACKAGE_RUN, REQUEST_FORWARD_START,
    };
    use rusqlite::{params, Connection};

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE hosts_profiles (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                content TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 0,
                sort_order INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .unwrap();
        crate::tools::helpers::ensure_request_forward_schema_for_test(&conn).unwrap();
        conn
    }

    fn seed_hosts(conn: &Connection, id: i64, name: &str) {
        conn.execute(
            "INSERT INTO hosts_profiles(id, name, content) VALUES (?1, ?2, '')",
            params![id, name],
        )
        .unwrap();
    }

    fn seed_forward(conn: &Connection, id: i64, name: &str) {
        conn.execute(
            "INSERT INTO request_forward_rules(
                id, name, protocol, bind_host, listen_port, target_url, target_host,
                target_port, capture_http_headers, capture_http_body, auto_start,
                created_at, updated_at
             ) VALUES (?1, ?2, 'http', '127.0.0.1', 8080, 'http://127.0.0.1:3000',
                NULL, NULL, 1, 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            params![id, name],
        )
        .unwrap();
    }

    #[test]
    fn only_registered_safe_actions_are_composable() {
        assert_eq!(
            combination_definitions()
                .iter()
                .map(|item| item.action_type)
                .collect::<Vec<_>>(),
            vec![
                HOSTS_ACTIVATE,
                BROWSER_PROFILE_LAUNCH,
                REQUEST_FORWARD_START
            ],
        );
        assert!(
            !definition("release_package.run")
                .unwrap()
                .supports_combination
        );
        assert!(definition("shell.run").is_none());
    }

    #[test]
    fn todo_definition_list_still_excludes_manual_combination_atoms() {
        assert_eq!(
            all_definitions()
                .iter()
                .map(|item| item.action_type)
                .collect::<Vec<_>>(),
            vec![RELEASE_PACKAGE_RUN],
        );
    }

    #[test]
    fn browser_target_key_round_trips_without_delimiter_assumptions() {
        let key =
            crate::tools::browser_profiles::encode_action_target("edge", "Profile 1").unwrap();
        assert_eq!(
            crate::tools::browser_profiles::decode_action_target(&key).unwrap(),
            ("edge".to_string(), "Profile 1".to_string()),
        );
    }

    #[test]
    fn hosts_and_forward_targets_use_stable_numeric_ids() {
        let conn = test_conn();
        seed_hosts(&conn, 7, "开发");
        seed_forward(&conn, 12, "本地 API");
        assert_eq!(
            crate::tools::hosts::list_action_targets_with_conn(&conn).unwrap()[0].0,
            "7"
        );
        assert_eq!(
            crate::tools::request_forward::list_action_targets_with_conn(&conn).unwrap()[0].0,
            "12"
        );
    }

    #[test]
    fn lists_and_validates_registered_database_targets() {
        let conn = test_conn();
        seed_hosts(&conn, 7, "开发");
        seed_forward(&conn, 12, "本地 API");

        let hosts = list_targets_with_conn(&conn, HOSTS_ACTIVATE).unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].id, "7");
        assert_eq!(hosts[0].label, "开发");
        assert!(hosts[0].available);

        let forward = validate_target_with_conn(&conn, REQUEST_FORWARD_START, "12").unwrap();
        assert_eq!(forward.label, "本地 API");
        assert!(forward.available);
    }

    #[test]
    fn rejects_unknown_actions_and_invalid_or_missing_targets() {
        let conn = test_conn();
        assert!(list_targets_with_conn(&conn, "shell.run")
            .unwrap_err()
            .contains("动作类型不存在"));
        assert!(
            validate_target_with_conn(&conn, HOSTS_ACTIVATE, "not-an-id")
                .unwrap_err()
                .contains("目标 ID 无效")
        );
        assert!(validate_target_with_conn(&conn, HOSTS_ACTIVATE, "7")
            .unwrap_err()
            .contains("目标不存在"));
    }

    #[test]
    fn snapshots_valid_missing_and_unknown_targets_without_hiding_identity() {
        let conn = test_conn();
        seed_hosts(&conn, 7, "开发");

        assert_eq!(
            snapshot_target_with_conn(&conn, HOSTS_ACTIVATE, "7"),
            AtomicTargetSnapshot {
                action_label: "切换 Hosts".into(),
                target_label: "开发".into(),
                validation_error: None,
            }
        );

        let missing = snapshot_target_with_conn(&conn, HOSTS_ACTIVATE, "8");
        assert_eq!(missing.action_label, "切换 Hosts");
        assert_eq!(missing.target_label, "8");
        assert!(missing.validation_error.unwrap().contains("目标不存在"));

        let unknown = snapshot_target_with_conn(&conn, "shell.run", "script-1");
        assert_eq!(unknown.action_label, "shell.run");
        assert_eq!(unknown.target_label, "script-1");
        assert!(unknown.validation_error.unwrap().contains("动作类型不存在"));
    }

    #[test]
    fn unavailable_browser_target_is_rejected_and_snapshotted_with_identity() {
        fn unavailable_browser_targets(
        ) -> Result<Vec<(String, String, bool, Option<String>)>, String> {
            Ok(vec![(
                crate::tools::browser_profiles::encode_action_target("edge", "Profile 1")?,
                "Edge · 工作".into(),
                false,
                Some("未找到 msedge.exe，无法启动该浏览器身份".into()),
            )])
        }

        let conn = test_conn();
        let target_id =
            crate::tools::browser_profiles::encode_action_target("edge", "Profile 1").unwrap();

        assert_eq!(
            validate_target_with_conn_using_browser_targets(
                &conn,
                BROWSER_PROFILE_LAUNCH,
                &target_id,
                unavailable_browser_targets,
            )
            .unwrap_err(),
            "未找到 msedge.exe，无法启动该浏览器身份"
        );
        assert_eq!(
            snapshot_target_with_conn_using_browser_targets(
                &conn,
                BROWSER_PROFILE_LAUNCH,
                &target_id,
                unavailable_browser_targets,
            ),
            AtomicTargetSnapshot {
                action_label: "启动浏览器身份".into(),
                target_label: "Edge · 工作".into(),
                validation_error: Some("未找到 msedge.exe，无法启动该浏览器身份".into()),
            }
        );
    }

    #[test]
    fn atomic_executor_maps_domain_boolean_to_step_outcome() {
        assert_eq!(
            AtomicStepSuccess::from_changed(false),
            AtomicStepSuccess {
                status: AtomicStepSuccessStatus::AlreadySatisfied,
                result_code: None,
                message: None,
            }
        );
        assert_eq!(
            AtomicStepSuccess::from_changed(true),
            AtomicStepSuccess {
                status: AtomicStepSuccessStatus::Succeeded,
                result_code: None,
                message: None,
            }
        );
    }
}

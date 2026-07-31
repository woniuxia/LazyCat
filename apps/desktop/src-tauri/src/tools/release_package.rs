use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
#[cfg(test)]
use std::{
    sync::Mutex,
    thread::{self, ThreadId},
};

use super::helpers::db_conn;
use super::release_package_archive::{default_folder_name, validate_folder_name};
pub use super::release_package_model::{
    PrepareResult, ReleasePackageEnvironmentConfig, ReleasePackageEnvironmentKind,
    ReleasePackageProjectConfig, ReleasePackageType, ReleaseTarget,
};
#[cfg(test)]
use super::release_package_remote::consume_probe;
use super::release_package_remote::run_command_preflight;
use super::release_package_remote::{
    classify_trust, consume_probe_for_environment, discard_preflight, discard_probe,
    issue_preflight, load_probe, probe_host, run_remote_preflight, store_probe,
    validate_remote_dir, validate_remote_file, AuthSecret, HostTrust, PreflightBinding,
    ProbeSnapshot, RemoteEndpoint, RemoteTarget,
};
use zeroize::Zeroizing;

const LEGACY_OUTPUT_ROOT_KEY: &str = "release_package.output_root";
pub const RELEASE_PACKAGE_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS release_package_projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    frontend_project_path TEXT NOT NULL,
    backend_project_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS release_package_environments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES release_package_projects(id) ON DELETE CASCADE,
    environment TEXT NOT NULL CHECK (environment IN ('test', 'production')),
    output_root TEXT NOT NULL DEFAULT '',
    package_type TEXT NOT NULL DEFAULT 'local_archive' CHECK (package_type IN ('local_archive', 'server_upload')),
    frontend_expected_branch TEXT NOT NULL DEFAULT 'master',
    frontend_build_command TEXT NOT NULL DEFAULT '',
    frontend_success_keyword TEXT NOT NULL DEFAULT '',
    frontend_post_upload_command TEXT NOT NULL DEFAULT '',
    frontend_artifact_path TEXT NOT NULL DEFAULT '',
    frontend_artifact_mode TEXT NOT NULL DEFAULT 'copy_directory' CHECK (frontend_artifact_mode IN ('copy_directory', 'zip_directory')),
    backend_expected_branch TEXT NOT NULL DEFAULT 'master',
    backend_build_command TEXT NOT NULL DEFAULT '',
    backend_success_keyword TEXT NOT NULL DEFAULT '',
    backend_post_upload_command TEXT NOT NULL DEFAULT '',
    backend_artifact_path TEXT NOT NULL DEFAULT '',
    ssh_host TEXT NOT NULL DEFAULT '',
    ssh_port INTEGER NOT NULL DEFAULT 22,
    ssh_username TEXT NOT NULL DEFAULT '',
    ssh_auth_type TEXT NOT NULL DEFAULT 'password' CHECK (ssh_auth_type IN ('password', 'private_key')),
    vault_entry_id INTEGER NULL,
    ssh_private_key_path TEXT NOT NULL DEFAULT '',
    frontend_remote_dir TEXT NOT NULL DEFAULT '',
    backend_remote_path TEXT NOT NULL DEFAULT '',
    health_check_enabled INTEGER NOT NULL DEFAULT 0 CHECK (health_check_enabled IN (0, 1)),
    health_check_url TEXT NOT NULL DEFAULT '',
    health_check_max_retries INTEGER NOT NULL DEFAULT 6 CHECK (health_check_max_retries BETWEEN 0 AND 60),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(project_id, environment)
);
CREATE TABLE IF NOT EXISTS release_package_known_hosts (
    host TEXT NOT NULL,
    port INTEGER NOT NULL,
    key_type TEXT NOT NULL,
    fingerprint_sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(host, port)
);
"#;

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>, String> {
    let mut query = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| format!("inspect release package table {table} failed: {error}"))?;
    let columns = query
        .query_map([], |row| row.get(1))
        .map_err(|error| format!("query release package table {table} failed: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read release package table {table} failed: {error}"))?;
    Ok(columns)
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1
         )",
        [table],
        |row| row.get(0),
    )
    .map_err(|error| format!("inspect release package table {table} failed: {error}"))
}

fn add_legacy_column_if_missing(
    transaction: &Transaction<'_>,
    column: &str,
    definition: &str,
) -> Result<(), String> {
    if table_columns(transaction, "release_package_projects")?
        .iter()
        .any(|name| name == column)
    {
        return Ok(());
    }
    transaction
        .execute_batch(&format!(
            "ALTER TABLE release_package_projects ADD COLUMN {column} {definition}"
        ))
        .map_err(|error| format!("migrate release package column {column} failed: {error}"))
}

fn add_environment_column_if_missing(
    transaction: &Transaction<'_>,
    column: &str,
    definition: &str,
) -> Result<(), String> {
    if table_columns(transaction, "release_package_environments")?
        .iter()
        .any(|name| name == column)
    {
        return Ok(());
    }
    transaction
        .execute_batch(&format!(
            "ALTER TABLE release_package_environments ADD COLUMN {column} {definition}"
        ))
        .map_err(|error| {
            format!("migrate release package environment column {column} failed: {error}")
        })
}

fn legacy_release_project_has_production_environment(
    transaction: &Transaction<'_>,
    target_id: &str,
) -> Result<bool, rusqlite::Error> {
    let Ok(project_id) = target_id.parse::<i64>() else {
        return Ok(false);
    };
    if project_id <= 0 {
        return Ok(false);
    }
    transaction.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM release_package_environment_migration_map
             WHERE project_id=?1
         )",
        [project_id],
        |row| row.get(0),
    )
}

fn migrate_legacy_schema(conn: &Connection) -> Result<(), String> {
    let transaction = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
        .map_err(|error| format!("begin release package schema migration failed: {error}"))?;

    for (column, definition) in [
        ("output_root", "TEXT NOT NULL DEFAULT ''"),
        ("upload_enabled", "INTEGER NOT NULL DEFAULT 0"),
        ("ssh_host", "TEXT NOT NULL DEFAULT ''"),
        ("ssh_port", "INTEGER NOT NULL DEFAULT 22"),
        ("ssh_username", "TEXT NOT NULL DEFAULT ''"),
        ("ssh_auth_type", "TEXT NOT NULL DEFAULT 'password'"),
        ("vault_entry_id", "INTEGER NULL"),
        ("ssh_private_key_path", "TEXT NOT NULL DEFAULT ''"),
        ("frontend_remote_dir", "TEXT NOT NULL DEFAULT ''"),
        ("backend_remote_path", "TEXT NOT NULL DEFAULT ''"),
        ("frontend_success_keyword", "TEXT NOT NULL DEFAULT ''"),
        ("backend_success_keyword", "TEXT NOT NULL DEFAULT ''"),
        ("frontend_post_upload_command", "TEXT NOT NULL DEFAULT ''"),
        ("backend_post_upload_command", "TEXT NOT NULL DEFAULT ''"),
    ] {
        add_legacy_column_if_missing(&transaction, column, definition)?;
    }
    if !table_columns(&transaction, "release_package_projects")?
        .iter()
        .any(|name| name == "package_type")
    {
        transaction
            .execute_batch(
                "ALTER TABLE release_package_projects
                 ADD COLUMN package_type TEXT NOT NULL DEFAULT 'local_archive'
                 CHECK (package_type IN ('local_archive', 'server_upload'));
                 UPDATE release_package_projects
                 SET package_type = CASE
                     WHEN upload_enabled = 1 THEN 'server_upload'
                     ELSE 'local_archive'
                 END;",
            )
            .map_err(|error| format!("migrate release package type failed: {error}"))?;
    }

    transaction
        .execute_batch(
            "ALTER TABLE release_package_projects
             RENAME TO release_package_projects_legacy;",
        )
        .map_err(|error| format!("rename legacy release package projects failed: {error}"))?;
    transaction
        .execute_batch(RELEASE_PACKAGE_SCHEMA_SQL)
        .map_err(|error| format!("create release package environment schema failed: {error}"))?;
    transaction
        .execute(
            "INSERT INTO release_package_projects(
                 id, name, frontend_project_path, backend_project_path, created_at, updated_at
             )
             SELECT id, name, frontend_project_path, backend_project_path, created_at, updated_at
             FROM release_package_projects_legacy",
            [],
        )
        .map_err(|error| format!("migrate release package projects failed: {error}"))?;
    transaction
        .execute(
            "INSERT INTO release_package_environments(
                 project_id, environment, output_root, package_type,
                 frontend_build_command, frontend_success_keyword,
                 frontend_post_upload_command, frontend_artifact_path, frontend_artifact_mode,
                 backend_build_command, backend_success_keyword,
                 backend_post_upload_command, backend_artifact_path,
                 ssh_host, ssh_port, ssh_username, ssh_auth_type, vault_entry_id,
                 ssh_private_key_path, frontend_remote_dir, backend_remote_path,
                 created_at, updated_at
             )
             SELECT id, 'production', output_root, package_type,
                    frontend_build_command, frontend_success_keyword,
                    frontend_post_upload_command, frontend_artifact_path,
                    frontend_artifact_mode, backend_build_command, backend_success_keyword,
                    backend_post_upload_command, backend_artifact_path,
                    ssh_host, ssh_port, ssh_username, ssh_auth_type, vault_entry_id,
                    ssh_private_key_path, frontend_remote_dir, backend_remote_path,
                    created_at, updated_at
             FROM release_package_projects_legacy",
            [],
        )
        .map_err(|error| {
            format!("migrate production release package environments failed: {error}")
        })?;
    transaction
        .execute(
            "INSERT INTO release_package_environments(project_id, environment)
             SELECT id, 'test' FROM release_package_projects",
            [],
        )
        .map_err(|error| format!("create test release package environments failed: {error}"))?;

    let invalid_environment_count: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM release_package_projects project
             WHERE (
                 SELECT COUNT(*) FROM release_package_environments environment
                 WHERE environment.project_id=project.id
             ) <> 2",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("verify release package environment count failed: {error}"))?;
    if invalid_environment_count != 0 {
        return Err(format!(
            "release package environment migration invalid: {invalid_environment_count} projects do not have two environments"
        ));
    }
    let foreign_key_violation_count: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM pragma_foreign_key_check('release_package_environments')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| {
            format!("verify release package environment foreign keys failed: {error}")
        })?;
    if foreign_key_violation_count != 0 {
        return Err(format!(
            "release package environment migration has {foreign_key_violation_count} foreign key violations"
        ));
    }

    transaction
        .execute_batch(
            "CREATE TEMP TABLE release_package_environment_migration_map (
                 project_id INTEGER PRIMARY KEY,
                 production_environment_id INTEGER NOT NULL
             );
             INSERT INTO release_package_environment_migration_map(
                 project_id, production_environment_id
             )
             SELECT project_id, id FROM release_package_environments
             WHERE environment='production';",
        )
        .map_err(|error| {
            format!("create release package environment migration map failed: {error}")
        })?;

    if table_exists(&transaction, "action_bindings")? {
        let binding_targets = transaction
            .prepare(
                "SELECT target_id FROM action_bindings
                 WHERE action_type='release_package.run'",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|error| {
                format!("validate release package action bindings before migration failed: {error}")
            })?;
        let mut invalid_binding_count = 0;
        for target_id in binding_targets {
            let valid = legacy_release_project_has_production_environment(&transaction, &target_id)
                .map_err(|error| {
                    format!(
                        "validate release package action bindings before migration failed: {error}"
                    )
                })?;
            invalid_binding_count += i64::from(!valid);
        }
        if invalid_binding_count != 0 {
            return Err(format!(
                "release package action binding migration invalid: {invalid_binding_count} active bindings have no environment"
            ));
        }
        transaction
            .execute(
                "UPDATE action_bindings
                 SET target_id = (
                     SELECT CAST(production_environment_id AS TEXT)
                     FROM release_package_environment_migration_map migration
                     WHERE migration.project_id=CAST(action_bindings.target_id AS INTEGER)
                 )
                 WHERE action_type='release_package.run'",
                [],
            )
            .map_err(|error| format!("migrate release package action bindings failed: {error}"))?;
        let invalid_binding_count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM action_bindings binding
                 WHERE binding.action_type='release_package.run'
                   AND (
                       binding.target_id=''
                       OR NOT EXISTS (
                           SELECT 1 FROM release_package_environments environment
                           WHERE CAST(environment.id AS TEXT)=binding.target_id
                       )
                   )",
                [],
                |row| row.get(0),
            )
            .map_err(|error| format!("verify release package action bindings failed: {error}"))?;
        if invalid_binding_count != 0 {
            return Err(format!(
                "release package action binding migration invalid: {invalid_binding_count} active bindings have no environment"
            ));
        }
    }

    if table_exists(&transaction, "action_dispatches")? {
        let dispatch_targets = transaction
            .prepare(
                "SELECT target_id FROM action_dispatches
                 WHERE action_type='release_package.run'
                   AND status IN ('pending_confirmation','running')",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|error| {
                format!(
                    "validate release package action dispatches before migration failed: {error}"
                )
            })?;
        let mut invalid_dispatch_count = 0;
        for target_id in dispatch_targets {
            let valid = legacy_release_project_has_production_environment(&transaction, &target_id)
                .map_err(|error| {
                    format!(
                        "validate release package action dispatches before migration failed: {error}"
                    )
                })?;
            invalid_dispatch_count += i64::from(!valid);
        }
        if invalid_dispatch_count != 0 {
            return Err(format!(
                "release package action dispatch migration invalid: {invalid_dispatch_count} active dispatches have no environment"
            ));
        }
        transaction
            .execute(
                "UPDATE action_dispatches
                 SET target_id = (
                     SELECT CAST(production_environment_id AS TEXT)
                     FROM release_package_environment_migration_map migration
                     WHERE migration.project_id=CAST(action_dispatches.target_id AS INTEGER)
                 )
                 WHERE action_type='release_package.run'
                   AND status IN ('pending_confirmation','running')",
                [],
            )
            .map_err(|error| {
                format!("migrate release package action dispatches failed: {error}")
            })?;
        let invalid_dispatch_count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM action_dispatches dispatch
                 WHERE dispatch.action_type='release_package.run'
                   AND dispatch.status IN ('pending_confirmation','running')
                   AND (
                       dispatch.target_id=''
                       OR NOT EXISTS (
                           SELECT 1 FROM release_package_environments environment
                           WHERE CAST(environment.id AS TEXT)=dispatch.target_id
                       )
                   )",
                [],
                |row| row.get(0),
            )
            .map_err(|error| format!("verify release package action dispatches failed: {error}"))?;
        if invalid_dispatch_count != 0 {
            return Err(format!(
                "release package action dispatch migration invalid: {invalid_dispatch_count} active dispatches have no environment"
            ));
        }
    }

    transaction
        .execute_batch(
            "DROP TABLE release_package_environment_migration_map;
             DROP TABLE release_package_projects_legacy;",
        )
        .map_err(|error| format!("finish release package schema migration failed: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("commit release package schema migration failed: {error}"))
}

pub fn ensure_schema(conn: &Connection) -> Result<(), String> {
    let legacy_schema = table_columns(conn, "release_package_projects")?
        .iter()
        .any(|column| column == "frontend_build_command");
    if legacy_schema {
        return migrate_legacy_schema(conn);
    }
    conn.execute_batch(RELEASE_PACKAGE_SCHEMA_SQL)
        .map_err(|error| format!("create release package schema failed: {error}"))?;
    let transaction =
        Transaction::new_unchecked(conn, TransactionBehavior::Immediate).map_err(|error| {
            format!("begin release package branch schema migration failed: {error}")
        })?;
    add_environment_column_if_missing(
        &transaction,
        "frontend_expected_branch",
        "TEXT NOT NULL DEFAULT 'master'",
    )?;
    add_environment_column_if_missing(
        &transaction,
        "health_check_enabled",
        "INTEGER NOT NULL DEFAULT 0 CHECK (health_check_enabled IN (0, 1))",
    )?;
    add_environment_column_if_missing(
        &transaction,
        "health_check_url",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_environment_column_if_missing(
        &transaction,
        "health_check_max_retries",
        "INTEGER NOT NULL DEFAULT 6 CHECK (health_check_max_retries BETWEEN 0 AND 60)",
    )?;
    add_environment_column_if_missing(
        &transaction,
        "backend_expected_branch",
        "TEXT NOT NULL DEFAULT 'master'",
    )?;
    transaction
        .commit()
        .map_err(|error| format!("commit release package branch schema migration failed: {error}"))
}

pub(crate) fn migrate_legacy_output_root(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "UPDATE release_package_environments
         SET output_root = (
             SELECT value FROM user_settings
             WHERE key = ?1
         )
         WHERE environment = 'production'
           AND TRIM(output_root) = ''
           AND EXISTS (
               SELECT 1 FROM user_settings
               WHERE key = ?1 AND TRIM(value) <> ''
           )",
        [LEGACY_OUTPUT_ROOT_KEY],
    )
    .map_err(|error| format!("migrate release package output root failed: {error}"))?;
    Ok(())
}

const ACTIONS: &[&str] = &[
    "project_list",
    "project_create",
    "project_update",
    "project_delete",
    "prepare",
    "branch_check",
    "target_check",
    "remote_probe",
    "host_trust",
    "remote_preflight",
    "remote_discard",
    "command_retry_prepare",
    "command_retry_preflight",
    "command_retry_start",
    "start",
    "upload_retry",
    "cancel",
];

fn require_package_type(
    project: &ReleasePackageEnvironmentConfig,
    expected: ReleasePackageType,
    action: &str,
) -> Result<(), String> {
    if project.package_type != expected {
        return Err(format!(
            "{action} only supports {} projects",
            expected.as_str()
        ));
    }
    Ok(())
}

pub(crate) struct ReleasePackageActionTargetRow {
    pub id: i64,
    pub label: String,
    pub available: bool,
    pub unavailable_reason: Option<String>,
}

#[derive(Debug)]
enum GitHead {
    Branch(String),
    Detached(String),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BranchCheck {
    target: ReleaseTarget,
    expected_branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detached_commit: Option<String>,
    matches: bool,
}

fn git_output(project_path: &str, args: &[&str]) -> Result<std::process::Output, String> {
    Command::new("git")
        .arg("-C")
        .arg(project_path)
        .args(args)
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                "未找到 Git，无法执行生产分支检查".to_string()
            } else {
                format!("无法执行 Git：{error}")
            }
        })
}

fn inspect_git_head(project_path: &str, target_label: &str) -> Result<GitHead, String> {
    let repository = git_output(project_path, &["rev-parse", "--is-inside-work-tree"])?;
    if !repository.status.success() || String::from_utf8_lossy(&repository.stdout).trim() != "true"
    {
        return Err(format!(
            "{target_label}工程目录不是 Git 工作区：{project_path}"
        ));
    }

    let branch = git_output(
        project_path,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
    )?;
    if branch.status.success() {
        let value = String::from_utf8_lossy(&branch.stdout).trim().to_string();
        if !value.is_empty() {
            return Ok(GitHead::Branch(value));
        }
    }

    let commit = git_output(project_path, &["rev-parse", "--short", "HEAD"])?;
    if commit.status.success() {
        let value = String::from_utf8_lossy(&commit.stdout).trim().to_string();
        if !value.is_empty() {
            return Ok(GitHead::Detached(value));
        }
    }
    let detail = String::from_utf8_lossy(&branch.stderr).trim().to_string();
    Err(if detail.is_empty() {
        format!("无法读取{target_label}工程当前 Git 分支")
    } else {
        format!("无法读取{target_label}工程当前 Git 分支：{detail}")
    })
}

fn inspect_production_branches(
    project: &ReleasePackageEnvironmentConfig,
    targets: &[ReleaseTarget],
) -> Result<Vec<BranchCheck>, String> {
    let mut checks = Vec::with_capacity(targets.len());
    for target in targets {
        let (label, project_path, expected_branch) = match target {
            ReleaseTarget::Frontend => (
                "前端",
                project.frontend_project_path.as_str(),
                project.frontend_expected_branch.as_str(),
            ),
            ReleaseTarget::Backend => (
                "后端",
                project.backend_project_path.as_str(),
                project.backend_expected_branch.as_str(),
            ),
        };
        let head = inspect_git_head(project_path, label)?;
        let (current_branch, detached_commit, matches) = match head {
            GitHead::Branch(branch) => {
                let matches = branch == expected_branch;
                (Some(branch), None, matches)
            }
            GitHead::Detached(commit) => (None, Some(commit), false),
        };
        checks.push(BranchCheck {
            target: *target,
            expected_branch: expected_branch.to_string(),
            current_branch,
            detached_commit,
            matches,
        });
    }
    Ok(checks)
}

fn validate_production_branches(
    project: &ReleasePackageEnvironmentConfig,
    targets: &[ReleaseTarget],
) -> Result<(), String> {
    if project.environment != ReleasePackageEnvironmentKind::Production {
        return Ok(());
    }
    for check in inspect_production_branches(project, targets)? {
        let label = match check.target {
            ReleaseTarget::Frontend => "前端",
            ReleaseTarget::Backend => "后端",
        };
        if let Some(branch) = check.current_branch.filter(|_| !check.matches) {
            return Err(format!(
                "{label}生产分支不匹配：当前为 {branch}，要求为 {}",
                check.expected_branch
            ));
        }
        if let Some(commit) = check.detached_commit {
            return Err(format!(
                "{label}工程当前处于 detached HEAD：{commit}，生产打包要求分支 {}",
                check.expected_branch
            ));
        }
    }
    Ok(())
}

struct ProjectPayload {
    name: String,
    frontend_project_path: String,
    backend_project_path: String,
}

#[derive(Clone)]
struct EnvironmentPayload {
    output_root: String,
    package_type: ReleasePackageType,
    frontend_expected_branch: String,
    frontend_build_command: String,
    frontend_success_keyword: String,
    frontend_post_upload_command: String,
    frontend_artifact_path: String,
    frontend_artifact_mode: String,
    backend_expected_branch: String,
    backend_build_command: String,
    backend_success_keyword: String,
    backend_post_upload_command: String,
    backend_artifact_path: String,
    ssh_host: String,
    ssh_port: u16,
    ssh_username: String,
    ssh_auth_type: String,
    vault_entry_id: Option<i64>,
    ssh_private_key_path: String,
    frontend_remote_dir: String,
    backend_remote_path: String,
    health_check_enabled: bool,
    health_check_url: String,
    health_check_max_retries: u32,
}
fn required_string(payload: &Value, key: &str) -> Result<String, String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("{key} is required"))
}

fn optional_string(payload: &Value, key: &str) -> Result<String, String> {
    match payload.get(key) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(value)) => Ok(value.trim().to_owned()),
        Some(_) => Err(format!("{key} must be a string")),
    }
}

fn optional_port(payload: &Value, key: &str, default: u16) -> Result<u16, String> {
    match payload.get(key) {
        None => Ok(default),
        Some(value) => value
            .as_u64()
            .filter(|port| (1..=u16::MAX as u64).contains(port))
            .map(|port| port as u16)
            .ok_or_else(|| format!("{key} must be between 1 and 65535")),
    }
}

fn optional_i64(payload: &Value, key: &str) -> Result<Option<i64>, String> {
    match payload.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_i64()
            .filter(|id| *id > 0)
            .map(Some)
            .ok_or_else(|| format!("{key} must be a positive integer")),
    }
}

fn optional_bool(payload: &Value, key: &str, default: bool) -> Result<bool, String> {
    match payload.get(key) {
        None => Ok(default),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(format!("{key} must be a boolean")),
    }
}

fn optional_bounded_u32(payload: &Value, key: &str, default: u32, max: u32) -> Result<u32, String> {
    match payload.get(key) {
        None => Ok(default),
        Some(value) => value
            .as_u64()
            .filter(|number| *number <= u64::from(max))
            .map(|number| number as u32)
            .ok_or_else(|| format!("{key} must be between 0 and {max}")),
    }
}
fn parse_targets(value: &Value) -> Result<Vec<ReleaseTarget>, String> {
    let values = value.as_array().ok_or("targets is required")?;
    if values.is_empty() {
        return Err("请至少选择前端包或后端包".into());
    }
    let mut targets = Vec::with_capacity(values.len());
    for value in values {
        let target = match value.as_str() {
            Some("frontend") => ReleaseTarget::Frontend,
            Some("backend") => ReleaseTarget::Backend,
            _ => return Err("targets 只能包含 frontend 或 backend".into()),
        };
        if targets.contains(&target) {
            return Err("targets 不能包含重复项".into());
        }
        targets.push(target);
    }
    Ok(targets)
}

fn parse_overwrite_existing(payload: &Value) -> Result<bool, String> {
    match payload.get("overwriteExisting") {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err("overwriteExisting must be a boolean".into()),
    }
}

#[derive(Debug)]
enum ReleaseStartInput {
    LocalArchive {
        folder_name: String,
        overwrite_existing: bool,
    },
    ServerUpload {
        preflight_token: String,
        overwrite_remote_targets: Vec<ReleaseTarget>,
    },
}

fn parse_start_input(
    package_type: ReleasePackageType,
    payload: &Value,
) -> Result<ReleaseStartInput, String> {
    if payload.get("mode").is_some() {
        return Err("mode is no longer supported".into());
    }
    match package_type {
        ReleasePackageType::LocalArchive => {
            if payload.get("preflightToken").is_some()
                || payload.get("overwriteRemoteTargets").is_some()
            {
                return Err("local_archive cannot include server upload parameters".into());
            }
            Ok(ReleaseStartInput::LocalArchive {
                folder_name: required_string(payload, "folderName")?,
                overwrite_existing: parse_overwrite_existing(payload)?,
            })
        }
        ReleasePackageType::ServerUpload => {
            if payload.get("folderName").is_some() || payload.get("overwriteExisting").is_some() {
                return Err("server_upload cannot include local archive parameters".into());
            }
            let overwrite_remote_targets = match payload.get("overwriteRemoteTargets") {
                None => Vec::new(),
                Some(Value::Array(values)) if values.is_empty() => Vec::new(),
                Some(value) => parse_targets(value)?,
            };
            Ok(ReleaseStartInput::ServerUpload {
                preflight_token: required_string(payload, "preflightToken")?,
                overwrite_remote_targets,
            })
        }
    }
}

fn validate_start_confirmation(
    environment: ReleasePackageEnvironmentKind,
    payload: &Value,
) -> Result<(), String> {
    match environment {
        ReleasePackageEnvironmentKind::Test if payload.get("productionConfirmed").is_some() => {
            Err("测试环境启动不能携带生产确认参数".into())
        }
        ReleasePackageEnvironmentKind::Test => Ok(()),
        ReleasePackageEnvironmentKind::Production
            if matches!(payload.get("productionConfirmed"), Some(Value::Bool(true))) =>
        {
            Ok(())
        }
        ReleasePackageEnvironmentKind::Production => Err("生产环境发布需要明确确认".into()),
    }
}

fn parse_action_dispatch_id(payload: &Value) -> Result<Option<String>, String> {
    match payload.get("actionDispatchId") {
        None => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => {
            Ok(Some(value.trim().to_string()))
        }
        Some(_) => Err("actionDispatchId must be a non-empty string".into()),
    }
}

fn parse_common_project_payload(payload: &Value) -> Result<ProjectPayload, String> {
    let project = payload
        .get("project")
        .filter(|value| value.is_object())
        .ok_or("project is required")?;
    let project = ProjectPayload {
        name: required_string(project, "name")?,
        frontend_project_path: required_string(project, "frontendProjectPath")?,
        backend_project_path: required_string(project, "backendProjectPath")?,
    };
    validate_folder_name(&project.name)?;
    Ok(project)
}

fn parse_environment_kind(payload: &Value) -> Result<ReleasePackageEnvironmentKind, String> {
    ReleasePackageEnvironmentKind::parse(&required_string(payload, "environment")?)
}

fn parse_environment_fields(payload: &Value) -> Result<EnvironmentPayload, String> {
    let mut ssh_auth_type = optional_string(payload, "sshAuthType")?;
    if ssh_auth_type.is_empty() {
        ssh_auth_type = "password".into();
    }
    if !matches!(ssh_auth_type.as_str(), "password" | "private_key") {
        return Err("sshAuthType must be password or private_key".into());
    }
    let ssh_port = if ssh_auth_type == "private_key" {
        optional_port(payload, "sshPort", 22)?
    } else {
        22
    };

    let environment = EnvironmentPayload {
        output_root: optional_string(payload, "outputRoot")?,
        package_type: ReleasePackageType::parse(&required_string(payload, "packageType")?)?,
        frontend_expected_branch: required_string(payload, "frontendExpectedBranch")?,
        frontend_build_command: required_string(payload, "frontendBuildCommand")?,
        frontend_success_keyword: optional_string(payload, "frontendSuccessKeyword")?,
        frontend_post_upload_command: optional_string(payload, "frontendPostUploadCommand")?,
        frontend_artifact_path: required_string(payload, "frontendArtifactPath")?,
        frontend_artifact_mode: required_string(payload, "frontendArtifactMode")?,
        backend_expected_branch: required_string(payload, "backendExpectedBranch")?,
        backend_build_command: required_string(payload, "backendBuildCommand")?,
        backend_success_keyword: optional_string(payload, "backendSuccessKeyword")?,
        backend_post_upload_command: optional_string(payload, "backendPostUploadCommand")?,
        backend_artifact_path: required_string(payload, "backendArtifactPath")?,
        ssh_host: optional_string(payload, "sshHost")?,
        ssh_port,
        ssh_username: optional_string(payload, "sshUsername")?,
        ssh_auth_type,
        vault_entry_id: optional_i64(payload, "vaultEntryId")?,
        ssh_private_key_path: optional_string(payload, "sshPrivateKeyPath")?,
        frontend_remote_dir: optional_string(payload, "frontendRemoteDir")?,
        backend_remote_path: optional_string(payload, "backendRemotePath")?,
        health_check_enabled: optional_bool(payload, "healthCheckEnabled", false)?,
        health_check_url: optional_string(payload, "healthCheckUrl")?,
        health_check_max_retries: optional_bounded_u32(payload, "healthCheckMaxRetries", 6, 60)?,
    };
    validate_environment_payload(&environment)?;
    Ok(environment)
}

fn parse_environment_payload(payload: &Value) -> Result<EnvironmentPayload, String> {
    let environment = payload
        .get("environmentConfig")
        .filter(|value| value.is_object())
        .ok_or("environmentConfig is required")?;
    parse_environment_fields(environment)
}

fn validate_environment_payload(environment: &EnvironmentPayload) -> Result<(), String> {
    if environment.package_type == ReleasePackageType::LocalArchive
        && environment.output_root.is_empty()
    {
        return Err("outputRoot is required for local_archive".into());
    }
    if environment.package_type == ReleasePackageType::ServerUpload {
        if environment.ssh_auth_type == "password" {
            if environment.vault_entry_id.is_none() {
                return Err("vaultEntryId is required for password authentication".into());
            }
        } else {
            if environment.ssh_host.is_empty() {
                return Err("sshHost is required for private_key authentication".into());
            }
            if environment.ssh_username.is_empty() {
                return Err("sshUsername is required for private_key authentication".into());
            }
            if environment.ssh_private_key_path.is_empty() {
                return Err("sshPrivateKeyPath is required for private_key authentication".into());
            }
        }
        if !environment.frontend_remote_dir.starts_with('/')
            || environment.frontend_remote_dir == "/"
        {
            return Err("frontendRemoteDir must be an absolute Linux path".into());
        }
        if !environment.backend_remote_path.starts_with('/')
            || environment.backend_remote_path == "/"
        {
            return Err("backendRemotePath must be an absolute Linux path".into());
        }
        if environment.health_check_enabled {
            let url = url::Url::parse(&environment.health_check_url)
                .map_err(|_| "healthCheckUrl must be a valid HTTP URL")?;
            if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
                return Err("healthCheckUrl must be a valid HTTP URL".into());
            }
        }
    }
    if !matches!(
        environment.frontend_artifact_mode.as_str(),
        "copy_directory" | "zip_directory"
    ) {
        return Err("frontendArtifactMode must be copy_directory or zip_directory".into());
    }
    Ok(())
}

fn validate_vault_binding(
    conn: &Connection,
    environment: &EnvironmentPayload,
) -> Result<(), String> {
    if environment.package_type != ReleasePackageType::ServerUpload
        || environment.ssh_auth_type != "password"
    {
        return Ok(());
    }
    let entry_id = environment
        .vault_entry_id
        .ok_or("vaultEntryId is required for password authentication")?;
    super::vault::server_credential_metadata(conn, entry_id)?;
    Ok(())
}

fn project_record_from_row(row: &Row<'_>) -> rusqlite::Result<ReleasePackageProjectConfig> {
    Ok(ReleasePackageProjectConfig {
        id: row.get(0)?,
        name: row.get(1)?,
        frontend_project_path: row.get(2)?,
        backend_project_path: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

struct RawEnvironmentConfig {
    id: i64,
    project_id: i64,
    project_name: String,
    environment: String,
    output_root: String,
    package_type: String,
    frontend_project_path: String,
    frontend_expected_branch: String,
    frontend_build_command: String,
    frontend_success_keyword: String,
    frontend_post_upload_command: String,
    frontend_artifact_path: String,
    frontend_artifact_mode: String,
    backend_project_path: String,
    backend_expected_branch: String,
    backend_build_command: String,
    backend_success_keyword: String,
    backend_post_upload_command: String,
    backend_artifact_path: String,
    ssh_host: String,
    ssh_port: i64,
    ssh_username: String,
    ssh_auth_type: String,
    vault_entry_id: Option<i64>,
    ssh_private_key_path: String,
    frontend_remote_dir: String,
    backend_remote_path: String,
    health_check_enabled: bool,
    health_check_url: String,
    health_check_max_retries: i64,
    created_at: String,
    updated_at: String,
}

fn raw_environment_from_row(row: &Row<'_>) -> rusqlite::Result<RawEnvironmentConfig> {
    Ok(RawEnvironmentConfig {
        id: row.get(0)?,
        project_id: row.get(1)?,
        project_name: row.get(2)?,
        environment: row.get(3)?,
        output_root: row.get(4)?,
        package_type: row.get(5)?,
        frontend_project_path: row.get(6)?,
        frontend_expected_branch: row.get(7)?,
        frontend_build_command: row.get(8)?,
        frontend_success_keyword: row.get(9)?,
        frontend_post_upload_command: row.get(10)?,
        frontend_artifact_path: row.get(11)?,
        frontend_artifact_mode: row.get(12)?,
        backend_project_path: row.get(13)?,
        backend_expected_branch: row.get(14)?,
        backend_build_command: row.get(15)?,
        backend_success_keyword: row.get(16)?,
        backend_post_upload_command: row.get(17)?,
        backend_artifact_path: row.get(18)?,
        ssh_host: row.get(19)?,
        ssh_port: row.get(20)?,
        ssh_username: row.get(21)?,
        ssh_auth_type: row.get(22)?,
        vault_entry_id: row.get(23)?,
        ssh_private_key_path: row.get(24)?,
        frontend_remote_dir: row.get(25)?,
        backend_remote_path: row.get(26)?,
        health_check_enabled: row.get(27)?,
        health_check_url: row.get(28)?,
        health_check_max_retries: row.get(29)?,
        created_at: row.get(30)?,
        updated_at: row.get(31)?,
    })
}

fn environment_payload_from_config(
    environment: &ReleasePackageEnvironmentConfig,
) -> EnvironmentPayload {
    EnvironmentPayload {
        output_root: environment.output_root.clone(),
        package_type: environment.package_type,
        frontend_expected_branch: environment.frontend_expected_branch.clone(),
        frontend_build_command: environment.frontend_build_command.clone(),
        frontend_success_keyword: environment.frontend_success_keyword.clone(),
        frontend_post_upload_command: environment.frontend_post_upload_command.clone(),
        frontend_artifact_path: environment.frontend_artifact_path.clone(),
        frontend_artifact_mode: environment.frontend_artifact_mode.clone(),
        backend_expected_branch: environment.backend_expected_branch.clone(),
        backend_build_command: environment.backend_build_command.clone(),
        backend_success_keyword: environment.backend_success_keyword.clone(),
        backend_post_upload_command: environment.backend_post_upload_command.clone(),
        backend_artifact_path: environment.backend_artifact_path.clone(),
        ssh_host: environment.ssh_host.clone(),
        ssh_port: environment.ssh_port,
        ssh_username: environment.ssh_username.clone(),
        ssh_auth_type: environment.ssh_auth_type.clone(),
        vault_entry_id: environment.vault_entry_id,
        ssh_private_key_path: environment.ssh_private_key_path.clone(),
        frontend_remote_dir: environment.frontend_remote_dir.clone(),
        backend_remote_path: environment.backend_remote_path.clone(),
        health_check_enabled: environment.health_check_enabled,
        health_check_url: environment.health_check_url.clone(),
        health_check_max_retries: environment.health_check_max_retries,
    }
}

fn environment_from_raw(
    raw: RawEnvironmentConfig,
) -> Result<ReleasePackageEnvironmentConfig, String> {
    let environment = ReleasePackageEnvironmentKind::parse(&raw.environment)?;
    let package_type = ReleasePackageType::parse(&raw.package_type)?;
    if !matches!(
        raw.frontend_artifact_mode.as_str(),
        "copy_directory" | "zip_directory"
    ) {
        return Err("上线包环境的前端产物模式无效".into());
    }
    if !matches!(raw.ssh_auth_type.as_str(), "password" | "private_key") {
        return Err("上线包环境的 SSH 认证方式无效".into());
    }
    let ssh_port = u16::try_from(raw.ssh_port)
        .ok()
        .filter(|port| *port > 0)
        .ok_or("上线包环境的 SSH 端口无效")?;
    let health_check_max_retries = u32::try_from(raw.health_check_max_retries)
        .ok()
        .filter(|retries| *retries <= 60)
        .ok_or("上线包环境的健康检查最多重试次数无效")?;
    let mut result = ReleasePackageEnvironmentConfig {
        id: raw.id,
        project_id: raw.project_id,
        project_name: raw.project_name,
        environment,
        configured: false,
        output_root: raw.output_root,
        package_type,
        frontend_project_path: raw.frontend_project_path,
        frontend_expected_branch: raw.frontend_expected_branch,
        frontend_build_command: raw.frontend_build_command,
        frontend_success_keyword: raw.frontend_success_keyword,
        frontend_post_upload_command: raw.frontend_post_upload_command,
        frontend_artifact_path: raw.frontend_artifact_path,
        frontend_artifact_mode: raw.frontend_artifact_mode,
        backend_project_path: raw.backend_project_path,
        backend_expected_branch: raw.backend_expected_branch,
        backend_build_command: raw.backend_build_command,
        backend_success_keyword: raw.backend_success_keyword,
        backend_post_upload_command: raw.backend_post_upload_command,
        backend_artifact_path: raw.backend_artifact_path,
        ssh_host: raw.ssh_host,
        ssh_port,
        ssh_username: raw.ssh_username,
        ssh_auth_type: raw.ssh_auth_type,
        vault_entry_id: raw.vault_entry_id,
        ssh_private_key_path: raw.ssh_private_key_path,
        frontend_remote_dir: raw.frontend_remote_dir,
        backend_remote_path: raw.backend_remote_path,
        health_check_enabled: raw.health_check_enabled,
        health_check_url: raw.health_check_url,
        health_check_max_retries,
        created_at: raw.created_at,
        updated_at: raw.updated_at,
    };
    result.configured =
        validate_environment_payload(&environment_payload_from_config(&result)).is_ok();
    Ok(result)
}

const ENVIRONMENT_SELECT: &str = "SELECT environment.id, environment.project_id, project.name,
            environment.environment, environment.output_root, environment.package_type,
            project.frontend_project_path, environment.frontend_expected_branch,
            environment.frontend_build_command,
            environment.frontend_success_keyword, environment.frontend_post_upload_command,
            environment.frontend_artifact_path, environment.frontend_artifact_mode,
            project.backend_project_path, environment.backend_expected_branch,
            environment.backend_build_command,
            environment.backend_success_keyword, environment.backend_post_upload_command,
            environment.backend_artifact_path, environment.ssh_host, environment.ssh_port,
            environment.ssh_username, environment.ssh_auth_type, environment.vault_entry_id,
            environment.ssh_private_key_path, environment.frontend_remote_dir,
            environment.backend_remote_path, environment.health_check_enabled,
            environment.health_check_url, environment.health_check_max_retries,
            environment.created_at, environment.updated_at
     FROM release_package_environments environment
     JOIN release_package_projects project ON project.id=environment.project_id";

fn load_environment(
    conn: &Connection,
    environment_id: i64,
) -> Result<ReleasePackageEnvironmentConfig, String> {
    let sql = format!("{ENVIRONMENT_SELECT} WHERE environment.id=?1");
    let raw = conn
        .query_row(&sql, [environment_id], raw_environment_from_row)
        .optional()
        .map_err(|error| format!("load release package environment failed: {error}"))?
        .ok_or_else(|| "release package environment not found".to_string())?;
    environment_from_raw(raw)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectListItem {
    #[serde(flatten)]
    project: ReleasePackageProjectConfig,
    environments: Vec<ReleasePackageEnvironmentConfig>,
}

#[cfg(test)]
type ThreadHook = (ThreadId, Box<dyn FnOnce() + Send>);

#[cfg(test)]
static PROJECT_LIST_AFTER_PROJECTS_HOOK: Mutex<Option<ThreadHook>> = Mutex::new(None);

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

fn load_project_list_items(conn: &Connection) -> Result<Vec<ProjectListItem>, String> {
    let transaction =
        Transaction::new_unchecked(conn, TransactionBehavior::Deferred).map_err(|error| {
            format!("begin release package project list transaction failed: {error}")
        })?;
    let mut project_statement = transaction
        .prepare(
            "SELECT id, name, frontend_project_path, backend_project_path, created_at, updated_at
             FROM release_package_projects
             ORDER BY name COLLATE NOCASE ASC, id ASC",
        )
        .map_err(|error| format!("prepare release package project list failed: {error}"))?;
    let projects = project_statement
        .query_map([], project_record_from_row)
        .map_err(|error| format!("query release package projects failed: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read release package project failed: {error}"))?;
    drop(project_statement);
    #[cfg(test)]
    run_thread_hook(&PROJECT_LIST_AFTER_PROJECTS_HOOK);

    let mut environment_statement = transaction
        .prepare(&format!(
            "{ENVIRONMENT_SELECT} ORDER BY environment.project_id, environment.id"
        ))
        .map_err(|error| format!("prepare release package environment list failed: {error}"))?;
    let raw_environments = environment_statement
        .query_map([], raw_environment_from_row)
        .map_err(|error| format!("query release package environments failed: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read release package environment failed: {error}"))?;
    drop(environment_statement);
    let mut grouped: HashMap<i64, Vec<ReleasePackageEnvironmentConfig>> = HashMap::new();
    for raw in raw_environments {
        let environment =
            environment_from_raw(raw).map_err(|_| "上线包项目环境配置不完整".to_string())?;
        grouped
            .entry(environment.project_id)
            .or_default()
            .push(environment);
    }

    let mut items = Vec::with_capacity(projects.len());
    for project in projects {
        let environments = grouped.remove(&project.id).unwrap_or_default();
        let mut test = Vec::new();
        let mut production = Vec::new();
        for environment in environments {
            match environment.environment {
                ReleasePackageEnvironmentKind::Test => test.push(environment),
                ReleasePackageEnvironmentKind::Production => production.push(environment),
            }
        }
        if test.len() != 1 || production.len() != 1 {
            return Err("上线包项目环境配置不完整".into());
        }
        items.push(ProjectListItem {
            project,
            environments: vec![test.pop().unwrap(), production.pop().unwrap()],
        });
    }
    if !grouped.is_empty() {
        return Err("上线包项目环境配置不完整".into());
    }
    transaction.commit().map_err(|error| {
        format!("commit release package project list transaction failed: {error}")
    })?;
    Ok(items)
}

fn project_list_with_conn(conn: &Connection) -> Result<Value, String> {
    Ok(json!({ "projects": load_project_list_items(conn)? }))
}

fn action_target_label(project_name: &str, environment: ReleasePackageEnvironmentKind) -> String {
    let environment_label = match environment {
        ReleasePackageEnvironmentKind::Test => "测试环境",
        ReleasePackageEnvironmentKind::Production => "生产环境",
    };
    format!("{project_name} · {environment_label}")
}

fn action_target_row_from_environment(
    environment: ReleasePackageEnvironmentConfig,
) -> ReleasePackageActionTargetRow {
    ReleasePackageActionTargetRow {
        id: environment.id,
        label: action_target_label(&environment.project_name, environment.environment),
        available: environment.configured,
        unavailable_reason: (!environment.configured).then(|| "环境配置不完整".to_string()),
    }
}

pub(crate) fn list_action_target_rows(
    conn: &Connection,
) -> Result<Vec<ReleasePackageActionTargetRow>, String> {
    let projects = load_project_list_items(conn)?;
    let mut rows = Vec::with_capacity(projects.len() * 2);
    for project in projects {
        for environment in project.environments {
            rows.push(action_target_row_from_environment(environment));
        }
    }
    Ok(rows)
}

pub(crate) fn load_action_target_row(
    conn: &Connection,
    environment_id: i64,
) -> Result<Option<ReleasePackageActionTargetRow>, String> {
    let sql = format!("{ENVIRONMENT_SELECT} WHERE environment.id=?1");
    let raw = conn
        .query_row(&sql, [environment_id], raw_environment_from_row)
        .optional()
        .map_err(|error| format!("load release package action target failed: {error}"))?;
    raw.map(|raw| {
        environment_from_raw(raw)
            .map(action_target_row_from_environment)
            .map_err(|_| "上线包项目环境配置不完整".to_string())
    })
    .transpose()
}

pub(crate) fn load_action_target_label(
    conn: &Connection,
    environment_id: i64,
) -> Result<Option<String>, String> {
    Ok(load_action_target_row(conn, environment_id)?.map(|row| row.label))
}

fn project_create_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let project = parse_common_project_payload(payload)?;
    let environment_kind = parse_environment_kind(payload)?;
    let environment = parse_environment_payload(payload)?;
    let transaction = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
        .map_err(|error| format!("begin release package project create failed: {error}"))?;
    validate_vault_binding(&transaction, &environment)?;
    transaction
        .execute(
            "INSERT INTO release_package_projects(
                name, frontend_project_path, backend_project_path
             ) VALUES (?1, ?2, ?3)",
            params![
                project.name,
                project.frontend_project_path,
                project.backend_project_path,
            ],
        )
        .map_err(|error| format!("create release package project failed: {error}"))?;
    let project_id = transaction.last_insert_rowid();
    transaction
        .execute(
            "INSERT INTO release_package_environments(
                project_id, environment, output_root, package_type,
                frontend_expected_branch, frontend_build_command, frontend_success_keyword,
                frontend_post_upload_command, frontend_artifact_path, frontend_artifact_mode,
                backend_expected_branch, backend_build_command, backend_success_keyword,
                backend_post_upload_command, backend_artifact_path,
                ssh_host, ssh_port, ssh_username, ssh_auth_type, vault_entry_id,
                ssh_private_key_path, frontend_remote_dir, backend_remote_path,
                health_check_enabled, health_check_url, health_check_max_retries
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21,
                ?22, ?23, ?24, ?25, ?26
             )",
            params![
                project_id,
                environment_kind.as_str(),
                environment.output_root,
                environment.package_type.as_str(),
                environment.frontend_expected_branch,
                environment.frontend_build_command,
                environment.frontend_success_keyword,
                environment.frontend_post_upload_command,
                environment.frontend_artifact_path,
                environment.frontend_artifact_mode,
                environment.backend_expected_branch,
                environment.backend_build_command,
                environment.backend_success_keyword,
                environment.backend_post_upload_command,
                environment.backend_artifact_path,
                environment.ssh_host,
                environment.ssh_port,
                environment.ssh_username,
                environment.ssh_auth_type,
                environment.vault_entry_id,
                environment.ssh_private_key_path,
                environment.frontend_remote_dir,
                environment.backend_remote_path,
                environment.health_check_enabled,
                environment.health_check_url,
                environment.health_check_max_retries,
            ],
        )
        .map_err(|error| format!("create release package environment failed: {error}"))?;
    let environment_id = transaction.last_insert_rowid();
    let blank_kind = match environment_kind {
        ReleasePackageEnvironmentKind::Test => ReleasePackageEnvironmentKind::Production,
        ReleasePackageEnvironmentKind::Production => ReleasePackageEnvironmentKind::Test,
    };
    transaction
        .execute(
            "INSERT INTO release_package_environments(project_id, environment) VALUES (?1, ?2)",
            params![project_id, blank_kind.as_str()],
        )
        .map_err(|error| format!("create blank release package environment failed: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("commit release package project create failed: {error}"))?;
    Ok(json!({ "id": project_id, "environmentId": environment_id }))
}

fn required_positive_id(payload: &Value, key: &str) -> Result<i64, String> {
    payload
        .get(key)
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| format!("{key} must be a positive integer"))
}

fn project_update_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = required_positive_id(payload, "id")?;
    let environment_id = required_positive_id(payload, "environmentId")?;
    let project = parse_common_project_payload(payload)?;
    let environment_kind = parse_environment_kind(payload)?;
    let environment = parse_environment_payload(payload)?;
    let transaction = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
        .map_err(|error| format!("begin release package project update failed: {error}"))?;
    let owner = transaction
        .query_row(
            "SELECT project_id, environment FROM release_package_environments WHERE id=?1",
            [environment_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| format!("load release package environment owner failed: {error}"))?;
    let Some((owner_project_id, stored_environment)) = owner else {
        return Err("上线包环境不属于当前项目".into());
    };
    if owner_project_id != id {
        return Err("上线包环境不属于当前项目".into());
    }
    if stored_environment != environment_kind.as_str() {
        return Err("上线包环境与提交环境不一致".into());
    }
    validate_vault_binding(&transaction, &environment)?;
    let changed = transaction
        .execute(
            "UPDATE release_package_projects SET
                name=?1, frontend_project_path=?2, backend_project_path=?3,
                updated_at=CURRENT_TIMESTAMP
             WHERE id=?4",
            params![
                project.name,
                project.frontend_project_path,
                project.backend_project_path,
                id,
            ],
        )
        .map_err(|error| format!("update release package project failed: {error}"))?;
    if changed == 0 {
        return Err("release package project not found".into());
    }
    transaction
        .execute(
            "UPDATE release_package_environments SET
                output_root=?1, package_type=?2,
                frontend_expected_branch=?3, frontend_build_command=?4,
                frontend_success_keyword=?5, frontend_post_upload_command=?6,
                frontend_artifact_path=?7, frontend_artifact_mode=?8,
                backend_expected_branch=?9, backend_build_command=?10,
                backend_success_keyword=?11, backend_post_upload_command=?12,
                backend_artifact_path=?13, ssh_host=?14, ssh_port=?15,
                ssh_username=?16, ssh_auth_type=?17, vault_entry_id=?18,
                ssh_private_key_path=?19, frontend_remote_dir=?20,
                backend_remote_path=?21, health_check_enabled=?22,
                health_check_url=?23, health_check_max_retries=?24,
                updated_at=CURRENT_TIMESTAMP
             WHERE id=?25",
            params![
                environment.output_root,
                environment.package_type.as_str(),
                environment.frontend_expected_branch,
                environment.frontend_build_command,
                environment.frontend_success_keyword,
                environment.frontend_post_upload_command,
                environment.frontend_artifact_path,
                environment.frontend_artifact_mode,
                environment.backend_expected_branch,
                environment.backend_build_command,
                environment.backend_success_keyword,
                environment.backend_post_upload_command,
                environment.backend_artifact_path,
                environment.ssh_host,
                environment.ssh_port,
                environment.ssh_username,
                environment.ssh_auth_type,
                environment.vault_entry_id,
                environment.ssh_private_key_path,
                environment.frontend_remote_dir,
                environment.backend_remote_path,
                environment.health_check_enabled,
                environment.health_check_url,
                environment.health_check_max_retries,
                environment_id,
            ],
        )
        .map_err(|error| format!("update release package environment failed: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("commit release package project update failed: {error}"))?;
    Ok(json!({ "id": id, "environmentId": environment_id }))
}

fn project_delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = required_positive_id(payload, "id")?;
    let changed = conn
        .execute("DELETE FROM release_package_projects WHERE id=?1", [id])
        .map_err(|e| format!("delete release package project failed: {e}"))?;
    if changed == 0 {
        return Err("release package project not found".into());
    }
    Ok(json!({ "ok": true }))
}

fn validate_run_inputs(
    project: &ReleasePackageEnvironmentConfig,
    folder_name: &str,
    targets: &[ReleaseTarget],
    overwrite_existing: bool,
) -> Result<(), String> {
    let output_root = PathBuf::from(&project.output_root);
    if !output_root.is_dir() {
        return Err("归档根目录不存在或不是文件夹".into());
    }
    validate_project_directories(project, targets)?;
    let final_path = output_root.join(folder_name);
    if final_path.exists() {
        if !final_path.is_dir() {
            return Err("目标归档路径已存在且不是文件夹".into());
        }
        if !overwrite_existing {
            return Err("目标归档目录已存在".into());
        }
    }
    Ok(())
}

fn validate_project_directories(
    project: &ReleasePackageEnvironmentConfig,
    targets: &[ReleaseTarget],
) -> Result<(), String> {
    if targets.contains(&ReleaseTarget::Frontend)
        && !PathBuf::from(&project.frontend_project_path).is_dir()
    {
        return Err("前端工程目录不存在或不是文件夹".into());
    }
    if targets.contains(&ReleaseTarget::Backend)
        && !PathBuf::from(&project.backend_project_path).is_dir()
    {
        return Err("后端工程目录不存在或不是文件夹".into());
    }
    Ok(())
}

fn prepare_with_conn(
    conn: &Connection,
    environment_id: i64,
    today: NaiveDate,
) -> Result<Value, String> {
    let project = load_environment(conn, environment_id)?;
    if project.package_type == ReleasePackageType::ServerUpload {
        return Ok(json!({ "packageType": "server_upload" }));
    }
    if project.output_root.trim().is_empty() {
        return Err("请先为当前项目配置归档根目录".into());
    }
    let output_root = project.output_root.clone();
    let folder_name = default_folder_name(today, &project.project_name);
    validate_folder_name(&folder_name)?;
    let archive_path = PathBuf::from(&output_root)
        .join(&folder_name)
        .to_string_lossy()
        .into_owned();
    serde_json::to_value(PrepareResult {
        package_type: project.package_type,
        default_folder_name: folder_name,
        output_root,
        archive_path,
        frontend_artifact_mode: project.frontend_artifact_mode,
    })
    .map_err(|e| format!("serialize release package prepare result failed: {e}"))
}

fn branch_check_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let environment_id = required_positive_id(payload, "environmentId")?;
    let project = load_environment(conn, environment_id)?;
    if project.environment != ReleasePackageEnvironmentKind::Production {
        return Err("仅生产环境需要执行分支检查".into());
    }
    let targets = parse_targets(payload.get("targets").unwrap_or(&Value::Null))?;
    validate_project_directories(&project, &targets)?;
    let checks = inspect_production_branches(&project, &targets)?;
    Ok(json!({ "checks": checks }))
}

fn target_check_with_conn(
    conn: &Connection,
    environment_id: i64,
    folder_name: &str,
) -> Result<Value, String> {
    let project = load_environment(conn, environment_id)?;
    require_package_type(&project, ReleasePackageType::LocalArchive, "target_check")?;
    validate_folder_name(folder_name)?;
    let output_root = PathBuf::from(&project.output_root);
    if !output_root.is_dir() {
        return Err("归档根目录不存在或不是文件夹".into());
    }
    let archive_path = output_root.join(folder_name);
    if archive_path.exists() && !archive_path.is_dir() {
        return Err("目标归档路径已存在且不是文件夹".into());
    }
    Ok(json!({
        "archivePath": archive_path.to_string_lossy().into_owned(),
        "exists": archive_path.is_dir(),
    }))
}

fn known_host_with_conn(
    conn: &Connection,
    endpoint: &RemoteEndpoint,
) -> Result<Option<(String, String)>, String> {
    conn.query_row(
        "SELECT key_type, fingerprint_sha256
         FROM release_package_known_hosts
         WHERE host=?1 AND port=?2",
        params![endpoint.host, endpoint.port],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .map_err(|error| format!("读取 SSH 主机信任记录失败：{error}"))
}

fn probe_result_with_conn(conn: &Connection, snapshot: ProbeSnapshot) -> Result<Value, String> {
    let previous = known_host_with_conn(conn, &snapshot.endpoint)?;
    let trust = classify_trust(
        previous
            .as_ref()
            .map(|(_, fingerprint)| fingerprint.as_str()),
        &snapshot.fingerprint_sha256,
    );
    let probe_token = store_probe(snapshot.clone())?;
    let mut result = json!({
        "probeToken": probe_token,
        "host": snapshot.endpoint.host,
        "port": snapshot.endpoint.port,
        "keyType": snapshot.key_type,
        "fingerprintSha256": snapshot.fingerprint_sha256,
        "trust": trust,
    });
    if let Some((_, fingerprint)) = previous {
        if trust == HostTrust::Changed {
            result["previousFingerprintSha256"] = json!(fingerprint);
        }
    }
    Ok(result)
}

fn validate_upload_project(project: &ReleasePackageEnvironmentConfig) -> Result<(), String> {
    if !matches!(project.ssh_auth_type.as_str(), "password" | "private_key") {
        return Err("不支持的 SSH 认证方式".into());
    }
    if project.ssh_auth_type == "password" {
        if project.vault_entry_id.is_none() {
            return Err("密码认证必须绑定密码库服务器凭据".into());
        }
    } else {
        if project.ssh_port == 0 {
            return Err("SSH 端口必须在 1 到 65535 之间".into());
        }
        if project.ssh_host.trim().is_empty() || project.ssh_username.trim().is_empty() {
            return Err("SSH 服务器地址和用户名不能为空".into());
        }
        if project.ssh_private_key_path.trim().is_empty() {
            return Err("私钥认证必须配置 SSH 私钥文件".into());
        }
    }
    validate_remote_dir(&project.frontend_remote_dir)?;
    validate_remote_file(&project.backend_remote_path)?;
    Ok(())
}

struct UploadEndpoint {
    endpoint: RemoteEndpoint,
    vault_entry_id: Option<i64>,
}

fn upload_endpoint_with_conn(
    conn: &Connection,
    project: &ReleasePackageEnvironmentConfig,
) -> Result<UploadEndpoint, String> {
    if project.ssh_auth_type == "password" {
        let entry_id = project.vault_entry_id.ok_or("vault_entry_id_missing")?;
        let metadata = super::vault::server_credential_metadata(conn, entry_id)?;
        super::vault::require_unlocked(conn)?;
        return Ok(UploadEndpoint {
            endpoint: RemoteEndpoint {
                host: metadata.address.to_ascii_lowercase(),
                port: metadata.port,
                username: metadata.account,
            },
            vault_entry_id: Some(metadata.entry_id),
        });
    }

    Ok(UploadEndpoint {
        endpoint: RemoteEndpoint {
            host: project.ssh_host.trim().to_ascii_lowercase(),
            port: project.ssh_port,
            username: project.ssh_username.clone(),
        },
        vault_entry_id: None,
    })
}

fn remote_probe_with_conn(conn: &Connection, environment_id: i64) -> Result<Value, String> {
    let project = load_environment(conn, environment_id)?;
    require_package_type(&project, ReleasePackageType::ServerUpload, "remote_probe")?;
    validate_upload_project(&project)?;
    let upload = upload_endpoint_with_conn(conn, &project)?;
    let snapshot = probe_host(environment_id, &upload.endpoint)?;
    probe_result_with_conn(conn, snapshot)
}

fn trust_host_with_conn(
    conn: &Connection,
    snapshot: &ProbeSnapshot,
    replace_existing: bool,
) -> Result<(), String> {
    let previous = known_host_with_conn(conn, &snapshot.endpoint)?;
    if let Some((_, fingerprint)) = &previous {
        if fingerprint != &snapshot.fingerprint_sha256 && !replace_existing {
            return Err("SSH 主机指纹已变化，必须显式确认重新信任".into());
        }
    }
    conn.execute(
        "INSERT INTO release_package_known_hosts(
            host, port, key_type, fingerprint_sha256
         ) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(host, port) DO UPDATE SET
            key_type=excluded.key_type,
            fingerprint_sha256=excluded.fingerprint_sha256,
            updated_at=CURRENT_TIMESTAMP",
        params![
            snapshot.endpoint.host,
            snapshot.endpoint.port,
            snapshot.key_type,
            snapshot.fingerprint_sha256,
        ],
    )
    .map_err(|error| format!("保存 SSH 主机信任记录失败：{error}"))?;
    Ok(())
}

fn host_trust_with_conn(
    conn: &Connection,
    environment_id: i64,
    probe_token: &str,
    replace_existing: bool,
) -> Result<Value, String> {
    let project = load_environment(conn, environment_id)?;
    require_package_type(&project, ReleasePackageType::ServerUpload, "host_trust")?;
    let snapshot = consume_probe_for_environment(probe_token, environment_id)?;
    trust_host_with_conn(conn, &snapshot, replace_existing)?;
    let next_token = store_probe(snapshot.clone())?;
    Ok(json!({
        "probeToken": next_token,
        "host": snapshot.endpoint.host,
        "port": snapshot.endpoint.port,
        "keyType": snapshot.key_type,
        "fingerprintSha256": snapshot.fingerprint_sha256,
        "trust": HostTrust::Trusted,
    }))
}
fn remote_targets(targets: &[ReleaseTarget]) -> Vec<RemoteTarget> {
    targets
        .iter()
        .map(|target| match target {
            ReleaseTarget::Frontend => RemoteTarget::Frontend,
            ReleaseTarget::Backend => RemoteTarget::Backend,
        })
        .collect()
}

fn parse_private_key_auth_secret(payload: &Value) -> Result<AuthSecret, String> {
    if payload.get("password").is_some() {
        return Err("私钥认证不能提交密码".into());
    }
    let passphrase = match payload.get("privateKeyPassphrase") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if value.is_empty() => None,
        Some(Value::String(value)) => Some(Zeroizing::new(value.clone())),
        Some(_) => return Err("privateKeyPassphrase must be a string".into()),
    };
    Ok(AuthSecret::PrivateKeyPassphrase(passphrase))
}

fn preflight_binding(
    project: &ReleasePackageEnvironmentConfig,
    upload: &UploadEndpoint,
    targets: &[ReleaseTarget],
) -> PreflightBinding {
    PreflightBinding {
        environment_id: project.id,
        project_id: project.project_id,
        environment: project.environment,
        endpoint: upload.endpoint.clone(),
        auth_type: project.ssh_auth_type.clone(),
        vault_entry_id: upload.vault_entry_id,
        private_key_path: project.ssh_private_key_path.clone(),
        targets: remote_targets(targets),
        command_retry_token: None,
        frontend_remote_dir: project.frontend_remote_dir.clone(),
        backend_remote_path: project.backend_remote_path.clone(),
    }
}

fn remote_preflight_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let environment_id = required_positive_id(payload, "environmentId")?;
    let project = load_environment(conn, environment_id)?;
    require_package_type(
        &project,
        ReleasePackageType::ServerUpload,
        "remote_preflight",
    )?;
    let targets = parse_targets(payload.get("targets").unwrap_or(&Value::Null))?;
    let probe_token = payload["probeToken"]
        .as_str()
        .ok_or("probeToken is required")?;
    validate_upload_project(&project)?;
    let upload = upload_endpoint_with_conn(conn, &project)?;
    let binding = preflight_binding(&project, &upload, &targets);
    let probe = load_probe(probe_token)?;
    if probe.environment_id != environment_id {
        return Err("SSH 探测令牌与当前环境不匹配".into());
    }
    if probe.endpoint != binding.endpoint {
        return Err("SSH 探测令牌与当前项目服务器配置不匹配".into());
    }
    let known = known_host_with_conn(conn, &binding.endpoint)?
        .ok_or_else(|| "请先确认并信任 SSH 主机指纹".to_string())?;
    if known.0 != probe.key_type || known.1 != probe.fingerprint_sha256 {
        return Err("SSH 主机指纹未受信任或已变化".into());
    }
    let secret = if project.ssh_auth_type == "password" {
        if payload.get("password").is_some() || payload.get("privateKeyPassphrase").is_some() {
            return Err("密码库认证不接受前端认证秘密".into());
        }
        let entry_id = project.vault_entry_id.ok_or("vault_entry_id_missing")?;
        let credential = super::vault::resolve_server_credential(conn, entry_id)?;
        let _metadata = credential.metadata;
        AuthSecret::Password(credential.password)
    } else {
        parse_private_key_auth_secret(payload)?
    };
    let checks = run_remote_preflight(&binding, &known.1, &secret)?;
    let issued = issue_preflight(binding, known.1, secret, &checks)?;
    Ok(json!({
        "preflightToken": issued.token,
        "expiresAt": issued.expires_at.to_rfc3339(),
        "targets": checks,
    }))
}

fn optional_token<'a>(payload: &'a Value, key: &str) -> Result<Option<&'a str>, String> {
    match payload.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.is_empty() => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(format!("{key} must be a string")),
    }
}

fn command_retry_prepare_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let environment_id = required_positive_id(payload, "environmentId")?;
    let retry_token = payload["retryToken"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or("retryToken is required")?;
    let project = load_environment(conn, environment_id)?;
    require_package_type(
        &project,
        ReleasePackageType::ServerUpload,
        "command_retry_prepare",
    )?;
    let prepared =
        super::release_package_runtime::prepare_command_retry(retry_token, environment_id)?;
    let snapshot = probe_host(environment_id, &prepared.binding.endpoint)?;
    let mut result = probe_result_with_conn(conn, snapshot)?;
    result["targets"] = json!(prepared.targets);
    result["authType"] = json!(prepared.binding.auth_type);
    result["username"] = json!(prepared.binding.endpoint.username);
    Ok(result)
}

fn command_retry_preflight_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let environment_id = required_positive_id(payload, "environmentId")?;
    let retry_token = payload["retryToken"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or("retryToken is required")?;
    let probe_token = payload["probeToken"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or("probeToken is required")?;
    let project = load_environment(conn, environment_id)?;
    require_package_type(
        &project,
        ReleasePackageType::ServerUpload,
        "command_retry_preflight",
    )?;
    let prepared =
        super::release_package_runtime::prepare_command_retry(retry_token, environment_id)?;
    let probe = load_probe(probe_token)?;
    if probe.environment_id != environment_id {
        return Err("SSH 探测令牌与当前环境不匹配".into());
    }
    if probe.endpoint != prepared.binding.endpoint {
        return Err("SSH 探测令牌与命令重试服务器不匹配".into());
    }
    let known = known_host_with_conn(conn, &prepared.binding.endpoint)?
        .ok_or_else(|| "请先确认并信任 SSH 主机指纹".to_string())?;
    if known.0 != probe.key_type || known.1 != probe.fingerprint_sha256 {
        return Err("SSH 主机指纹未受信任或已变化".into());
    }
    let secret = if prepared.binding.auth_type == "password" {
        if payload.get("password").is_some() || payload.get("privateKeyPassphrase").is_some() {
            return Err("密码库认证不接受前端认证秘密".into());
        }
        let entry_id = prepared
            .binding
            .vault_entry_id
            .ok_or("vault_entry_id_missing")?;
        AuthSecret::Password(super::vault::resolve_server_credential(conn, entry_id)?.password)
    } else {
        parse_private_key_auth_secret(payload)?
    };
    run_command_preflight(
        &prepared.binding.endpoint,
        &prepared.binding.private_key_path,
        &known.1,
        &secret,
    )?;
    let binding = PreflightBinding {
        environment_id,
        project_id: project.project_id,
        environment: project.environment,
        endpoint: prepared.binding.endpoint,
        auth_type: prepared.binding.auth_type,
        vault_entry_id: prepared.binding.vault_entry_id,
        private_key_path: prepared.binding.private_key_path,
        targets: remote_targets(&prepared.targets),
        command_retry_token: Some(retry_token.to_string()),
        frontend_remote_dir: String::new(),
        backend_remote_path: String::new(),
    };
    let issued = issue_preflight(binding, known.1, secret, &[])?;
    Ok(json!({ "authToken": issued.token, "expiresAt": issued.expires_at.to_rfc3339() }))
}
fn remote_discard(payload: &Value) -> Result<Value, String> {
    let preflight_token = optional_token(payload, "preflightToken")?;
    let probe_token = optional_token(payload, "probeToken")?;
    if let Some(token) = preflight_token {
        discard_preflight(token)?;
    }
    if let Some(token) = probe_token {
        discard_probe(token)?;
    }
    Ok(json!({ "ok": true }))
}
#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported release_package action: {action}"));
    }
    if matches!(action, "start" | "upload_retry" | "cancel") {
        return Err("release_package action requires app context".into());
    }
    if action == "remote_discard" {
        return remote_discard(payload);
    }
    let conn = db_conn()?;
    match action {
        "project_list" => project_list_with_conn(&conn),
        "project_create" => project_create_with_conn(&conn, payload),
        "project_update" => project_update_with_conn(&conn, payload),
        "project_delete" => project_delete_with_conn(&conn, payload),
        "prepare" => {
            let id = required_positive_id(payload, "environmentId")?;
            prepare_with_conn(&conn, id, Local::now().date_naive())
        }
        "branch_check" => branch_check_with_conn(&conn, payload),
        "target_check" => {
            let id = required_positive_id(payload, "environmentId")?;
            let folder_name = payload["folderName"]
                .as_str()
                .ok_or("folderName is required")?;
            target_check_with_conn(&conn, id, folder_name)
        }
        "remote_probe" => {
            let id = required_positive_id(payload, "environmentId")?;
            remote_probe_with_conn(&conn, id)
        }
        "host_trust" => {
            let environment_id = required_positive_id(payload, "environmentId")?;
            let probe_token = payload["probeToken"]
                .as_str()
                .ok_or("probeToken is required")?;
            let replace_existing = match payload.get("replaceExisting") {
                None => false,
                Some(Value::Bool(value)) => *value,
                Some(_) => return Err("replaceExisting must be a boolean".into()),
            };
            host_trust_with_conn(&conn, environment_id, probe_token, replace_existing)
        }
        "remote_preflight" => remote_preflight_with_conn(&conn, payload),
        "command_retry_preflight" => command_retry_preflight_with_conn(&conn, payload),
        "command_retry_prepare" => command_retry_prepare_with_conn(&conn, payload),
        _ => unreachable!(),
    }
}

pub fn execute_with_app(
    action: &str,
    payload: &Value,
    app: &tauri::AppHandle,
) -> Result<Value, String> {
    match action {
        "start" => {
            let environment_id = required_positive_id(payload, "environmentId")?;
            let action_dispatch_id = parse_action_dispatch_id(payload)?;
            let conn = db_conn()?;
            let project = load_environment(&conn, environment_id)?;
            validate_start_confirmation(project.environment, payload)?;
            let targets = parse_targets(payload.get("targets").unwrap_or(&Value::Null))?;
            let start_input = parse_start_input(project.package_type, payload)?;
            validate_project_directories(&project, &targets)?;
            validate_production_branches(&project, &targets)?;
            match start_input {
                ReleaseStartInput::LocalArchive {
                    folder_name,
                    overwrite_existing,
                } => {
                    validate_folder_name(&folder_name)?;
                    validate_run_inputs(&project, &folder_name, &targets, overwrite_existing)?;
                    let output_root = PathBuf::from(&project.output_root);
                    super::release_package_runtime::start(
                        app,
                        project,
                        targets,
                        super::release_package_runtime::RuntimeStartRequest::LocalArchive {
                            output_root,
                            folder_name,
                            overwrite_existing,
                        },
                        action_dispatch_id,
                    )
                }
                ReleaseStartInput::ServerUpload {
                    preflight_token,
                    overwrite_remote_targets,
                } => {
                    validate_upload_project(&project)?;
                    let upload = upload_endpoint_with_conn(&conn, &project)?;
                    let binding = preflight_binding(&project, &upload, &targets);
                    let deploy_authorization =
                        super::release_package_runtime::consume_deploy_authorization(
                            &preflight_token,
                            &binding,
                            &overwrite_remote_targets,
                        )?;
                    super::release_package_runtime::start(
                        app,
                        project,
                        targets,
                        super::release_package_runtime::RuntimeStartRequest::ServerUpload {
                            deploy_authorization,
                        },
                        action_dispatch_id,
                    )
                }
            }
        }
        "upload_retry" => {
            let environment_id = required_positive_id(payload, "environmentId")?;
            let conn = db_conn()?;
            let project = load_environment(&conn, environment_id)?;
            require_package_type(&project, ReleasePackageType::ServerUpload, "upload_retry")?;
            let retry_token = payload["retryToken"]
                .as_str()
                .filter(|token| !token.is_empty())
                .ok_or("retryToken is required")?;
            let preflight_token = payload["preflightToken"]
                .as_str()
                .filter(|token| !token.is_empty())
                .ok_or("preflightToken is required")?;
            let overwrite_remote_targets = match payload.get("overwriteRemoteTargets") {
                None => Vec::new(),
                Some(Value::Array(values)) if values.is_empty() => Vec::new(),
                Some(value) => parse_targets(value)?,
            };
            let targets =
                super::release_package_runtime::retry_targets(retry_token, environment_id)?;
            validate_upload_project(&project)?;
            let upload = upload_endpoint_with_conn(&conn, &project)?;
            let binding = preflight_binding(&project, &upload, &targets);
            let deploy_authorization =
                super::release_package_runtime::consume_deploy_authorization(
                    preflight_token,
                    &binding,
                    &overwrite_remote_targets,
                )?;
            super::release_package_runtime::upload_retry(
                app,
                project,
                retry_token,
                deploy_authorization,
            )
        }
        "command_retry_start" => {
            let environment_id = required_positive_id(payload, "environmentId")?;
            let retry_token = payload["retryToken"]
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or("retryToken is required")?;
            let auth_token = payload["authToken"]
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or("authToken is required")?;
            let conn = db_conn()?;
            let project = load_environment(&conn, environment_id)?;
            require_package_type(
                &project,
                ReleasePackageType::ServerUpload,
                "command_retry_start",
            )?;
            let prepared =
                super::release_package_runtime::prepare_command_retry(retry_token, environment_id)?;
            let binding = PreflightBinding {
                environment_id,
                project_id: project.project_id,
                environment: project.environment,
                endpoint: prepared.binding.endpoint,
                auth_type: prepared.binding.auth_type,
                vault_entry_id: prepared.binding.vault_entry_id,
                private_key_path: prepared.binding.private_key_path,
                targets: remote_targets(&prepared.targets),
                command_retry_token: Some(retry_token.to_string()),
                frontend_remote_dir: String::new(),
                backend_remote_path: String::new(),
            };
            super::release_package_runtime::command_retry(
                app,
                project,
                retry_token,
                auth_token,
                binding,
            )
        }
        "cancel" => {
            let run_id = payload["runId"].as_str().ok_or("runId is required")?;
            super::release_package_runtime::cancel(run_id)
        }
        _ => execute(action, payload),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rusqlite::Connection;
    use serde_json::{json, Value};
    use std::{
        ffi::OsString,
        fs,
        path::{Path, PathBuf},
        thread,
    };
    use uuid::Uuid;

    #[test]
    fn project_config_serializes_only_project_fields() {
        let project = ReleasePackageProjectConfig {
            id: 7,
            name: "Customer portal".into(),
            frontend_project_path: r"D:\work\web".into(),
            backend_project_path: r"D:\work\server".into(),
            created_at: "2026-07-28 10:00:00".into(),
            updated_at: "2026-07-28 11:00:00".into(),
        };

        let serialized = serde_json::to_value(project).unwrap();
        assert_eq!(
            serialized,
            json!({
                "id": 7,
                "name": "Customer portal",
                "frontendProjectPath": r"D:\work\web",
                "backendProjectPath": r"D:\work\server",
                "createdAt": "2026-07-28 10:00:00",
                "updatedAt": "2026-07-28 11:00:00"
            })
        );
        assert!(serialized.get("packageType").is_none());
        assert!(serialized.get("outputRoot").is_none());
    }

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(RELEASE_PACKAGE_SCHEMA_SQL).unwrap();
        conn
    }

    #[test]
    fn legacy_output_root_migrates_only_to_blank_production_environments() {
        let conn = test_conn();
        conn.execute_batch(
            "CREATE TABLE user_settings (
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );
             INSERT INTO user_settings(key, value)
             VALUES ('release_package.output_root', 'D:\\legacy-releases');
             INSERT INTO release_package_projects(
                 id, name, frontend_project_path, backend_project_path
             ) VALUES
                 (1, 'blank', '', ''),
                 (2, 'configured', '', '');
             INSERT INTO release_package_environments(
                 project_id, environment, output_root
             ) VALUES
                 (1, 'test', ''),
                 (1, 'production', ''),
                 (2, 'test', ''),
                 (2, 'production', 'D:\\configured');",
        )
        .unwrap();

        migrate_legacy_output_root(&conn).unwrap();

        let output_root = |project_id, environment| {
            conn.query_row(
                "SELECT output_root FROM release_package_environments
                 WHERE project_id=?1 AND environment=?2",
                params![project_id, environment],
                |row| row.get::<_, String>(0),
            )
            .unwrap()
        };
        assert_eq!(output_root(1, "production"), r"D:\legacy-releases");
        assert_eq!(output_root(1, "test"), "");
        assert_eq!(output_root(2, "production"), r"D:\configured");
        assert_eq!(output_root(2, "test"), "");
    }

    struct TempDatabase {
        path: PathBuf,
    }

    impl TempDatabase {
        fn new() -> (Self, Connection) {
            let path = std::env::temp_dir()
                .join(format!("lazycat-release-package-{}.sqlite", Uuid::new_v4()));
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA busy_timeout = 5000;",
            )
            .unwrap();
            ensure_schema(&conn).unwrap();
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

    fn seed_legacy_release_package_schema(conn: &Connection) {
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE release_package_projects (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL,
                 output_root TEXT NOT NULL,
                 package_type TEXT NOT NULL DEFAULT 'local_archive',
                 frontend_project_path TEXT NOT NULL,
                 frontend_build_command TEXT NOT NULL,
                 frontend_success_keyword TEXT NOT NULL DEFAULT '',
                 frontend_post_upload_command TEXT NOT NULL DEFAULT '',
                 frontend_artifact_path TEXT NOT NULL,
                 frontend_artifact_mode TEXT NOT NULL,
                 backend_project_path TEXT NOT NULL,
                 backend_build_command TEXT NOT NULL,
                 backend_success_keyword TEXT NOT NULL DEFAULT '',
                 backend_post_upload_command TEXT NOT NULL DEFAULT '',
                 backend_artifact_path TEXT NOT NULL,
                 upload_enabled INTEGER NOT NULL DEFAULT 0,
                 ssh_host TEXT NOT NULL DEFAULT '',
                 ssh_port INTEGER NOT NULL DEFAULT 22,
                 ssh_username TEXT NOT NULL DEFAULT '',
                 ssh_auth_type TEXT NOT NULL DEFAULT 'password',
                 vault_entry_id INTEGER NULL,
                 ssh_private_key_path TEXT NOT NULL DEFAULT '',
                 frontend_remote_dir TEXT NOT NULL DEFAULT '',
                 backend_remote_path TEXT NOT NULL DEFAULT '',
                 created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                 updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE TABLE release_package_known_hosts (
                 host TEXT NOT NULL,
                 port INTEGER NOT NULL,
                 key_type TEXT NOT NULL,
                 fingerprint_sha256 TEXT NOT NULL,
                 created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                 updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                 PRIMARY KEY(host, port)
             );
             INSERT INTO release_package_known_hosts(
                 host, port, key_type, fingerprint_sha256, created_at, updated_at
             ) VALUES (
                 'deploy.example.internal', 2222, 'ssh-ed25519', 'SHA256:legacy',
                 '2026-07-01 08:00:00', '2026-07-02 09:00:00'
             );
             CREATE TABLE action_bindings (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 action_type TEXT NOT NULL,
                 target_id TEXT NOT NULL,
                 enabled INTEGER NOT NULL DEFAULT 1
             );
             CREATE TABLE action_dispatches (
                 id TEXT PRIMARY KEY,
                 action_type TEXT NOT NULL,
                 target_id TEXT NOT NULL,
                 status TEXT NOT NULL
             );",
        )
        .unwrap();
    }

    fn seed_legacy_release_project(conn: &Connection, id: i64) {
        conn.execute(
            "INSERT INTO release_package_projects(
                 id, name, output_root, package_type,
                 frontend_project_path, frontend_build_command, frontend_success_keyword,
                 frontend_post_upload_command, frontend_artifact_path, frontend_artifact_mode,
                 backend_project_path, backend_build_command, backend_success_keyword,
                 backend_post_upload_command, backend_artifact_path, upload_enabled,
                 ssh_host, ssh_port, ssh_username, ssh_auth_type, vault_entry_id,
                 ssh_private_key_path, frontend_remote_dir, backend_remote_path,
                 created_at, updated_at
             ) VALUES (
                 ?1, '客户门户', 'D:\\releases', 'server_upload',
                 'D:\\work\\web', 'pnpm build', 'Build completed',
                 'cd /srv/web && ./reload.sh', 'dist', 'zip_directory',
                 'D:\\work\\server', 'mvn package -Pprod', 'BUILD SUCCESS',
                 'systemctl restart portal', 'target/portal.jar', 0,
                 'deploy.example.internal', 2222, 'deploy', 'password', 17,
                 'C:\\keys\\legacy', '/srv/portal/web', '/srv/portal/app.jar',
                 '2026-07-03 10:00:00', '2026-07-04 11:00:00'
             )",
            [id],
        )
        .unwrap();
    }

    fn environment_project_payload(environment: &str) -> Value {
        json!({
            "project": {
                "name": "客户门户",
                "frontendProjectPath": r"D:\work\web",
                "backendProjectPath": r"D:\work\server"
            },
            "environment": environment,
            "environmentConfig": {
                "packageType": "server_upload",
                "outputRoot": r"D:\releases",
                "frontendExpectedBranch": "master",
                "frontendBuildCommand": "pnpm build",
                "frontendSuccessKeyword": "  Build completed  ",
                "frontendPostUploadCommand": "\n  cd /srv/web\n  ./reload.sh\n",
                "frontendArtifactPath": "dist",
                "frontendArtifactMode": "copy_directory",
                "backendExpectedBranch": "master",
                "backendBuildCommand": "mvn clean package -Pprod",
                "backendSuccessKeyword": "  BUILD SUCCESS  ",
                "backendPostUploadCommand": "\n  systemctl restart portal\n",
                "backendArtifactPath": r"target\portal.jar",
                "sshHost": "deploy.example.internal",
                "sshPort": 2222,
                "sshUsername": "deploy",
                "sshAuthType": "private_key",
                "vaultEntryId": null,
                "sshPrivateKeyPath": r"C:\Users\tester\.ssh\lazycat",
                "frontendRemoteDir": "/srv/portal/web",
                "backendRemotePath": "/srv/portal/app.jar",
                "healthCheckEnabled": true,
                "healthCheckUrl": "https://portal.example.com/health",
                "healthCheckMaxRetries": 4
            }
        })
    }

    fn environment_id(conn: &Connection, project_id: i64, environment: &str) -> i64 {
        conn.query_row(
            "SELECT id FROM release_package_environments
             WHERE project_id=?1 AND environment=?2",
            params![project_id, environment],
            |row| row.get(0),
        )
        .unwrap()
    }

    #[test]
    fn project_list_uses_one_snapshot_during_concurrent_delete() {
        let (database, reader_conn) = TempDatabase::new();
        let project_id =
            project_create_with_conn(&reader_conn, &environment_project_payload("production"))
                .unwrap()["id"]
                .as_i64()
                .unwrap();
        let writer_conn = database.connect();
        install_thread_hook(
            &PROJECT_LIST_AFTER_PROJECTS_HOOK,
            thread::current().id(),
            move || {
                thread::spawn(move || {
                    writer_conn
                        .execute(
                            "DELETE FROM release_package_projects WHERE id=?1",
                            [project_id],
                        )
                        .unwrap();
                })
                .join()
                .unwrap();
            },
        );

        let listed = project_list_with_conn(&reader_conn).unwrap();

        assert_eq!(listed["projects"][0]["id"], project_id);
        assert_eq!(
            listed["projects"][0]["environments"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(project_list_with_conn(&reader_conn).unwrap()["projects"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn project_crud_environment_create_test_and_list_fixed_environments() {
        let conn = test_conn();
        let created =
            project_create_with_conn(&conn, &environment_project_payload("test")).unwrap();
        let project_id = created["id"].as_i64().unwrap();
        let selected_environment_id = created["environmentId"].as_i64().unwrap();

        let stored = conn
            .prepare(
                "SELECT id, environment FROM release_package_environments
                 WHERE project_id=?1 ORDER BY environment",
            )
            .unwrap()
            .query_map([project_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(stored.len(), 2);
        assert!(stored
            .iter()
            .any(|(id, kind)| *id == selected_environment_id && kind == "test"));

        let listed = project_list_with_conn(&conn).unwrap();
        let project = &listed["projects"][0];
        assert_eq!(project["name"], "客户门户");
        assert_eq!(project["frontendProjectPath"], r"D:\work\web");
        assert_eq!(project["backendProjectPath"], r"D:\work\server");
        let environments = project["environments"].as_array().unwrap();
        assert_eq!(environments.len(), 2);
        assert_eq!(environments[0]["environment"], "test");
        assert_eq!(environments[0]["configured"], true);
        assert_eq!(environments[0]["id"], selected_environment_id);
        assert_eq!(environments[0]["frontendSuccessKeyword"], "Build completed");
        assert_eq!(environments[1]["environment"], "production");
        assert_eq!(environments[1]["configured"], false);
    }

    #[test]
    fn project_crud_environment_create_production_marks_only_production_configured() {
        let conn = test_conn();
        let created =
            project_create_with_conn(&conn, &environment_project_payload("production")).unwrap();
        let listed = project_list_with_conn(&conn).unwrap();
        let environments = listed["projects"][0]["environments"].as_array().unwrap();

        assert_eq!(environments[0]["environment"], "test");
        assert_eq!(environments[0]["configured"], false);
        assert_eq!(environments[1]["environment"], "production");
        assert_eq!(environments[1]["configured"], true);
        assert_eq!(environments[1]["id"], created["environmentId"]);
    }

    #[test]
    fn project_crud_environment_rejects_cross_project_update_without_changes() {
        let conn = test_conn();
        let first = project_create_with_conn(&conn, &environment_project_payload("test")).unwrap();
        let mut second_payload = environment_project_payload("test");
        second_payload["project"]["name"] = json!("管理后台");
        let second = project_create_with_conn(&conn, &second_payload).unwrap();
        let first_id = first["id"].as_i64().unwrap();
        let second_environment_id = second["environmentId"].as_i64().unwrap();

        let mut update = environment_project_payload("test");
        update["id"] = json!(first_id);
        update["environmentId"] = json!(second_environment_id);
        update["project"]["name"] = json!("不应写入");
        update["environmentConfig"]["frontendBuildCommand"] = json!("invalid mutation");

        assert_eq!(
            project_update_with_conn(&conn, &update).unwrap_err(),
            "上线包环境不属于当前项目"
        );
        assert_eq!(
            conn.query_row(
                "SELECT name FROM release_package_projects WHERE id=?1",
                [first_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "客户门户"
        );
        assert_eq!(
            conn.query_row(
                "SELECT frontend_build_command FROM release_package_environments WHERE id=?1",
                [second_environment_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "pnpm build"
        );
    }

    #[test]
    fn project_crud_environment_update_is_atomic_and_keeps_other_environment() {
        let conn = test_conn();
        let created =
            project_create_with_conn(&conn, &environment_project_payload("test")).unwrap();
        let project_id = created["id"].as_i64().unwrap();
        let test_environment_id = created["environmentId"].as_i64().unwrap();
        let production_environment_id = environment_id(&conn, project_id, "production");
        let mut production = environment_project_payload("production");
        production["id"] = json!(project_id);
        production["environmentId"] = json!(production_environment_id);
        production["project"]["name"] = json!("客户门户 Pro");
        production["project"]["frontendProjectPath"] = json!(r"D:\work\web-pro");
        production["environmentConfig"]["frontendBuildCommand"] = json!("pnpm build:prod");

        let updated = project_update_with_conn(&conn, &production).unwrap();
        assert_eq!(
            updated,
            json!({
                "id": project_id,
                "environmentId": production_environment_id
            })
        );
        assert_eq!(
            conn.query_row(
                "SELECT name || '|' || frontend_project_path
                 FROM release_package_projects WHERE id=?1",
                [project_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            r"客户门户 Pro|D:\work\web-pro"
        );
        assert_eq!(
            conn.query_row(
                "SELECT frontend_build_command FROM release_package_environments WHERE id=?1",
                [production_environment_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "pnpm build:prod"
        );
        assert_eq!(
            conn.query_row(
                "SELECT frontend_build_command FROM release_package_environments WHERE id=?1",
                [test_environment_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "pnpm build"
        );

        conn.execute_batch(
            "CREATE TRIGGER reject_release_environment_update
             BEFORE UPDATE ON release_package_environments
             BEGIN SELECT RAISE(ABORT, 'forced environment failure'); END;",
        )
        .unwrap();
        production["project"]["name"] = json!("事务不应部分提交");
        production["environmentConfig"]["frontendBuildCommand"] = json!("also not saved");
        assert!(project_update_with_conn(&conn, &production).is_err());
        assert_eq!(
            conn.query_row(
                "SELECT name FROM release_package_projects WHERE id=?1",
                [project_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "客户门户 Pro"
        );
        assert_eq!(
            conn.query_row(
                "SELECT frontend_build_command FROM release_package_environments WHERE id=?1",
                [production_environment_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "pnpm build:prod"
        );
    }

    #[test]
    fn project_crud_environment_create_rolls_back_project_when_environment_insert_fails() {
        let conn = test_conn();
        conn.execute_batch(
            "CREATE TRIGGER reject_release_environment_insert
             BEFORE INSERT ON release_package_environments
             BEGIN SELECT RAISE(ABORT, 'forced environment failure'); END;",
        )
        .unwrap();

        assert!(
            project_create_with_conn(&conn, &environment_project_payload("test"))
                .unwrap_err()
                .contains("forced environment failure")
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM release_package_projects", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            0
        );
    }

    #[test]
    fn project_crud_environment_delete_project_cascades_both_environments() {
        let conn = test_conn();
        conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
        let created =
            project_create_with_conn(&conn, &environment_project_payload("test")).unwrap();
        let project_id = created["id"].as_i64().unwrap();

        project_delete_with_conn(&conn, &json!({ "id": project_id })).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM release_package_environments WHERE project_id=?1",
                [project_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn project_crud_environment_list_rejects_incomplete_duplicate_and_unknown_without_repair() {
        let missing = test_conn();
        let created =
            project_create_with_conn(&missing, &environment_project_payload("test")).unwrap();
        let project_id = created["id"].as_i64().unwrap();
        missing
            .execute(
                "DELETE FROM release_package_environments
                 WHERE project_id=?1 AND environment='production'",
                [project_id],
            )
            .unwrap();
        assert_eq!(
            project_list_with_conn(&missing).unwrap_err(),
            "上线包项目环境配置不完整"
        );
        assert_eq!(
            missing
                .query_row(
                    "SELECT COUNT(*) FROM release_package_environments WHERE project_id=?1",
                    [project_id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );

        for corrupt_kind in ["test", "staging"] {
            let conn = Connection::open_in_memory().unwrap();
            conn.execute_batch(
                "CREATE TABLE release_package_projects (
                    id INTEGER PRIMARY KEY, name TEXT NOT NULL,
                    frontend_project_path TEXT NOT NULL, backend_project_path TEXT NOT NULL,
                    created_at TEXT NOT NULL, updated_at TEXT NOT NULL
                 );
                 CREATE TABLE release_package_environments AS
                 SELECT 0 AS id, 0 AS project_id, '' AS environment, '' AS output_root,
                    '' AS package_type, '' AS frontend_expected_branch,
                    '' AS frontend_build_command,
                    '' AS frontend_success_keyword, '' AS frontend_post_upload_command,
                    '' AS frontend_artifact_path, '' AS frontend_artifact_mode,
                    '' AS backend_expected_branch, '' AS backend_build_command,
                    '' AS backend_success_keyword,
                    '' AS backend_post_upload_command, '' AS backend_artifact_path,
                    '' AS ssh_host, 22 AS ssh_port, '' AS ssh_username,
                    '' AS ssh_auth_type, NULL AS vault_entry_id, '' AS ssh_private_key_path,
                    '' AS frontend_remote_dir, '' AS backend_remote_path,
                    0 AS health_check_enabled, '' AS health_check_url,
                    6 AS health_check_max_retries,
                    '' AS created_at, '' AS updated_at WHERE 0;
                 INSERT INTO release_package_projects VALUES
                    (1, '损坏项目', 'D:\\web', 'D:\\server', 'created', 'updated');
                 INSERT INTO release_package_environments
                 SELECT 1, 1, 'test', '', 'local_archive', 'master', '', '', '', '',
                    'copy_directory', 'master', '', '', '', '', '', 22, '', 'password', NULL,
                    '', '', '', 0, '', 6, 'created', 'updated';
                 INSERT INTO release_package_environments
                 SELECT 2, 1, 'production', '', 'local_archive', 'master', '', '', '', '',
                    'copy_directory', 'master', '', '', '', '', '', 22, '', 'password', NULL,
                    '', '', '', 0, '', 6, 'created', 'updated';",
            )
            .unwrap();
            conn.execute(
                "UPDATE release_package_environments SET environment=?1 WHERE id=2",
                [corrupt_kind],
            )
            .unwrap();
            assert_eq!(
                project_list_with_conn(&conn).unwrap_err(),
                "上线包项目环境配置不完整"
            );
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM release_package_environments",
                    [],
                    |row| { row.get::<_, i64>(0) }
                )
                .unwrap(),
                2
            );
        }
    }

    #[test]
    fn schema_migrates_each_legacy_project_to_production_and_blank_test() {
        let conn = Connection::open_in_memory().unwrap();
        seed_legacy_release_package_schema(&conn);
        seed_legacy_release_project(&conn, 42);
        conn.execute_batch(
            "INSERT INTO action_bindings(action_type, target_id) VALUES
                 ('release_package.run', '+42'),
                 ('hosts.activate', '42');
             INSERT INTO action_dispatches(id, action_type, target_id, status) VALUES
                 ('pending', 'release_package.run', '00042', 'pending_confirmation'),
                 ('running', 'release_package.run', '42', 'running'),
                 ('succeeded', 'release_package.run', '42', 'succeeded'),
                 ('failed', 'release_package.run', '42', 'failed'),
                 ('other', 'hosts.activate', '42', 'running');",
        )
        .unwrap();

        ensure_schema(&conn).unwrap();

        let project_columns = conn
            .prepare("PRAGMA table_info(release_package_projects)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            project_columns,
            [
                "id",
                "name",
                "frontend_project_path",
                "backend_project_path",
                "created_at",
                "updated_at",
            ]
        );
        let project: (String, String, String, String, String) = conn
            .query_row(
                "SELECT name, frontend_project_path, backend_project_path, created_at, updated_at
                 FROM release_package_projects WHERE id=42",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            project,
            (
                "客户门户".into(),
                r"D:\work\web".into(),
                r"D:\work\server".into(),
                "2026-07-03 10:00:00".into(),
                "2026-07-04 11:00:00".into(),
            )
        );

        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM release_package_environments WHERE project_id=42",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            2
        );
        let production_id: i64 = conn
            .query_row(
                "SELECT id FROM release_package_environments
                 WHERE project_id=42 AND environment='production'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(production_id > 0);
        conn.query_row(
            "SELECT output_root, package_type,
                    frontend_build_command, frontend_success_keyword,
                    frontend_post_upload_command, frontend_artifact_path,
                    frontend_artifact_mode, backend_build_command, backend_success_keyword,
                    backend_post_upload_command, backend_artifact_path,
                    ssh_host, ssh_port, ssh_username, ssh_auth_type, vault_entry_id,
                    ssh_private_key_path, frontend_remote_dir, backend_remote_path,
                    created_at, updated_at
             FROM release_package_environments WHERE id=?1",
            [production_id],
            |row| {
                assert_eq!(row.get::<_, String>(0)?, r"D:\releases");
                assert_eq!(row.get::<_, String>(1)?, "server_upload");
                assert_eq!(row.get::<_, String>(2)?, "pnpm build");
                assert_eq!(row.get::<_, String>(3)?, "Build completed");
                assert_eq!(row.get::<_, String>(4)?, "cd /srv/web && ./reload.sh");
                assert_eq!(row.get::<_, String>(5)?, "dist");
                assert_eq!(row.get::<_, String>(6)?, "zip_directory");
                assert_eq!(row.get::<_, String>(7)?, "mvn package -Pprod");
                assert_eq!(row.get::<_, String>(8)?, "BUILD SUCCESS");
                assert_eq!(row.get::<_, String>(9)?, "systemctl restart portal");
                assert_eq!(row.get::<_, String>(10)?, "target/portal.jar");
                assert_eq!(row.get::<_, String>(11)?, "deploy.example.internal");
                assert_eq!(row.get::<_, i64>(12)?, 2222);
                assert_eq!(row.get::<_, String>(13)?, "deploy");
                assert_eq!(row.get::<_, String>(14)?, "password");
                assert_eq!(row.get::<_, Option<i64>>(15)?, Some(17));
                assert_eq!(row.get::<_, String>(16)?, r"C:\keys\legacy");
                assert_eq!(row.get::<_, String>(17)?, "/srv/portal/web");
                assert_eq!(row.get::<_, String>(18)?, "/srv/portal/app.jar");
                assert_eq!(row.get::<_, String>(19)?, "2026-07-03 10:00:00");
                assert_eq!(row.get::<_, String>(20)?, "2026-07-04 11:00:00");
                Ok(())
            },
        )
        .unwrap();

        let test_id: i64 = conn
            .query_row(
                "SELECT id FROM release_package_environments
                 WHERE project_id=42 AND environment='test'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(test_id > 0);
        let blank_test_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM release_package_environments
                 WHERE id=?1 AND output_root='' AND package_type='local_archive'
                   AND frontend_build_command='' AND frontend_success_keyword=''
                   AND frontend_post_upload_command='' AND frontend_artifact_path=''
                   AND frontend_artifact_mode='copy_directory'
                   AND backend_build_command='' AND backend_success_keyword=''
                   AND backend_post_upload_command='' AND backend_artifact_path=''
                   AND ssh_host='' AND ssh_port=22 AND ssh_username=''
                   AND ssh_auth_type='password' AND vault_entry_id IS NULL
                   AND ssh_private_key_path='' AND frontend_remote_dir=''
                   AND backend_remote_path=''",
                [test_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(blank_test_count, 1);

        let foreign_key_target: String = conn
            .query_row(
                "PRAGMA foreign_key_list(release_package_environments)",
                [],
                |row| row.get(2),
            )
            .unwrap();
        assert_eq!(foreign_key_target, "release_package_projects");
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            0
        );

        let mapped_target = production_id.to_string();
        assert_eq!(
            conn.query_row(
                "SELECT target_id FROM action_bindings WHERE action_type='release_package.run'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            mapped_target
        );
        assert_eq!(
            conn.query_row(
                "SELECT target_id FROM action_bindings WHERE action_type='hosts.activate'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "42"
        );
        for dispatch_id in ["pending", "running"] {
            assert_eq!(
                conn.query_row(
                    "SELECT target_id FROM action_dispatches WHERE id=?1",
                    [dispatch_id],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
                production_id.to_string()
            );
        }
        for dispatch_id in ["succeeded", "failed"] {
            assert_eq!(
                conn.query_row(
                    "SELECT target_id FROM action_dispatches WHERE id=?1",
                    [dispatch_id],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
                "42"
            );
        }
        assert_eq!(
            conn.query_row(
                "SELECT
                    (SELECT COUNT(*) FROM action_bindings b
                     JOIN release_package_environments e ON CAST(e.id AS TEXT)=b.target_id
                     WHERE b.action_type='release_package.run')
                    +
                    (SELECT COUNT(*) FROM action_dispatches d
                     JOIN release_package_environments e ON CAST(e.id AS TEXT)=d.target_id
                     WHERE d.action_type='release_package.run'
                       AND d.status IN ('pending_confirmation','running'))",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            3
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM release_package_known_hosts
                 WHERE host='deploy.example.internal' AND port=2222
                   AND fingerprint_sha256='SHA256:legacy'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            1
        );

        ensure_schema(&conn).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM release_package_environments WHERE project_id=42",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            2
        );
        assert!(conn
            .execute(
                "INSERT INTO release_package_environments(project_id, environment)
                 VALUES (42, 'production')",
                [],
            )
            .is_err());

        conn.execute("DELETE FROM release_package_projects WHERE id=42", [])
            .unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM release_package_environments WHERE project_id=42",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn schema_adds_default_branches_to_existing_environment_tables() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE release_package_projects (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL,
                 frontend_project_path TEXT NOT NULL,
                 backend_project_path TEXT NOT NULL,
                 created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                 updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE TABLE release_package_environments (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 project_id INTEGER NOT NULL,
                 environment TEXT NOT NULL
             );
             INSERT INTO release_package_environments(project_id, environment)
             VALUES (1, 'production');",
        )
        .unwrap();

        ensure_schema(&conn).unwrap();

        let branches = conn
            .query_row(
                "SELECT frontend_expected_branch, backend_expected_branch
                 FROM release_package_environments WHERE id=1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .unwrap();
        assert_eq!(branches, ("master".into(), "master".into()));
        ensure_schema(&conn).unwrap();
        assert_eq!(
            table_columns(&conn, "release_package_environments")
                .unwrap()
                .iter()
                .filter(|column| column.ends_with("_expected_branch"))
                .count(),
            2
        );
    }

    #[test]
    fn schema_environment_kind_accepts_only_fixed_values() {
        assert_eq!(
            ReleasePackageEnvironmentKind::parse("test").unwrap(),
            ReleasePackageEnvironmentKind::Test
        );
        assert_eq!(
            ReleasePackageEnvironmentKind::parse("production").unwrap(),
            ReleasePackageEnvironmentKind::Production
        );
        assert_eq!(
            ReleasePackageEnvironmentKind::parse("staging")
                .err()
                .unwrap(),
            "上线包环境无效"
        );
        assert_eq!(ReleasePackageEnvironmentKind::Test.as_str(), "test");
        assert_eq!(
            ReleasePackageEnvironmentKind::Production.as_str(),
            "production"
        );
        assert_eq!(
            serde_json::to_value(ReleasePackageEnvironmentKind::Production).unwrap(),
            json!("production")
        );
    }

    #[test]
    fn schema_new_environment_table_rejects_staging_via_check_constraint() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO release_package_projects(name, frontend_project_path, backend_project_path)
             VALUES ('客户门户', 'D:\\work\\web', 'D:\\work\\server')",
            [],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO release_package_environments(project_id, environment)
             VALUES (1, 'staging')",
            [],
        );

        assert!(result.is_err());
    }

    #[test]
    fn schema_rolls_back_when_a_binding_target_overflows_i64() {
        let conn = Connection::open_in_memory().unwrap();
        seed_legacy_release_package_schema(&conn);
        seed_legacy_release_project(&conn, i64::MAX);
        conn.execute(
            "INSERT INTO action_bindings(action_type, target_id)
             VALUES ('release_package.run', '9223372036854775808')",
            [],
        )
        .unwrap();

        let error = ensure_schema(&conn).err().unwrap();
        assert!(error.contains("active bindings have no environment"));
        let project_columns = table_columns(&conn, "release_package_projects").unwrap();
        assert!(project_columns
            .iter()
            .any(|column| column == "frontend_build_command"));
        assert!(!table_exists(&conn, "release_package_environments").unwrap());
        assert!(!table_exists(&conn, "release_package_projects_legacy").unwrap());
        assert_eq!(
            conn.query_row(
                "SELECT target_id FROM action_bindings WHERE action_type='release_package.run'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "9223372036854775808"
        );
    }

    #[test]
    fn schema_rolls_back_when_an_active_dispatch_target_is_zero() {
        let conn = Connection::open_in_memory().unwrap();
        seed_legacy_release_package_schema(&conn);
        seed_legacy_release_project(&conn, 0);
        conn.execute(
            "INSERT INTO action_dispatches(id, action_type, target_id, status)
             VALUES ('zero', 'release_package.run', '0', 'running')",
            [],
        )
        .unwrap();

        let error = ensure_schema(&conn).err().unwrap();
        assert!(error.contains("active dispatches have no environment"));
        let project_columns = table_columns(&conn, "release_package_projects").unwrap();
        assert!(project_columns
            .iter()
            .any(|column| column == "frontend_build_command"));
        assert!(!table_exists(&conn, "release_package_environments").unwrap());
        assert!(!table_exists(&conn, "release_package_projects_legacy").unwrap());
        assert_eq!(
            conn.query_row(
                "SELECT target_id FROM action_dispatches WHERE id='zero'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "0"
        );
    }

    #[test]
    fn schema_rolls_back_when_an_invalid_binding_target_collides_with_an_environment_id() {
        let conn = Connection::open_in_memory().unwrap();
        seed_legacy_release_package_schema(&conn);
        seed_legacy_release_project(&conn, 42);
        conn.execute(
            "INSERT INTO action_bindings(action_type, target_id)
             VALUES ('release_package.run', '1')",
            [],
        )
        .unwrap();

        let error = ensure_schema(&conn).err().unwrap();
        assert!(error.contains("active bindings have no environment"));
        let project_columns = table_columns(&conn, "release_package_projects").unwrap();
        assert!(project_columns
            .iter()
            .any(|column| column == "frontend_build_command"));
        assert!(!table_exists(&conn, "release_package_environments").unwrap());
        assert!(!table_exists(&conn, "release_package_projects_legacy").unwrap());
        assert_eq!(
            conn.query_row(
                "SELECT target_id FROM action_bindings WHERE action_type='release_package.run'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "1"
        );
    }

    #[test]
    fn schema_rolls_back_when_an_invalid_dispatch_target_collides_with_an_environment_id() {
        let conn = Connection::open_in_memory().unwrap();
        seed_legacy_release_package_schema(&conn);
        seed_legacy_release_project(&conn, 42);
        conn.execute(
            "INSERT INTO action_dispatches(id, action_type, target_id, status)
             VALUES ('collision', 'release_package.run', '1', 'pending_confirmation')",
            [],
        )
        .unwrap();

        let error = ensure_schema(&conn).err().unwrap();
        assert!(error.contains("active dispatches have no environment"));
        let project_columns = table_columns(&conn, "release_package_projects").unwrap();
        assert!(project_columns
            .iter()
            .any(|column| column == "frontend_build_command"));
        assert!(!table_exists(&conn, "release_package_environments").unwrap());
        assert!(!table_exists(&conn, "release_package_projects_legacy").unwrap());
        assert_eq!(
            conn.query_row(
                "SELECT target_id FROM action_dispatches WHERE id='collision'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "1"
        );
    }

    #[test]
    fn password_project_requires_vault_entry_but_private_key_keeps_host_and_username() {
        let mut password = environment_project_payload("production");
        password["environmentConfig"]["sshAuthType"] = json!("password");
        password["environmentConfig"]["vaultEntryId"] = Value::Null;
        password["environmentConfig"]["sshHost"] = json!("");
        password["environmentConfig"]["sshUsername"] = json!("");
        assert_eq!(
            parse_environment_payload(&password).err().unwrap(),
            "vaultEntryId is required for password authentication"
        );

        let private_key =
            parse_environment_payload(&environment_project_payload("production")).unwrap();
        assert_eq!(private_key.vault_entry_id, None);
        assert_eq!(private_key.frontend_success_keyword, "Build completed");
        assert_eq!(private_key.backend_success_keyword, "BUILD SUCCESS");
        assert_eq!(
            private_key.frontend_post_upload_command,
            "cd /srv/web\n  ./reload.sh"
        );
        assert_eq!(
            private_key.backend_post_upload_command,
            "systemctl restart portal"
        );
    }

    #[test]
    fn project_port_is_only_validated_for_private_key_authentication() {
        let conn = test_conn();
        conn.execute_batch(
            "CREATE TABLE vault_entries (
                id INTEGER PRIMARY KEY, category TEXT NOT NULL, plain_fields TEXT
            );
            INSERT INTO vault_entries(id, category, plain_fields)
            VALUES (17, 'server', '{\"address\":\"10.0.0.8\",\"port\":2200,\"account\":\"deploy\"}');",
        )
        .unwrap();
        let mut password = environment_project_payload("production");
        password["environmentConfig"]["sshAuthType"] = json!("password");
        password["environmentConfig"]["vaultEntryId"] = json!(17);
        password["environmentConfig"]["sshPort"] = json!(0);

        let parsed_password = parse_environment_payload(&password).unwrap();
        assert_eq!(parsed_password.ssh_port, 22);
        let mut password_project_payload = environment_project_payload("production");
        password_project_payload["environmentConfig"]["sshAuthType"] = json!("password");
        password_project_payload["environmentConfig"]["vaultEntryId"] = json!(17);
        password_project_payload["environmentConfig"]["sshPort"] = json!(0);
        let password_id = project_create_with_conn(&conn, &password_project_payload).unwrap()["id"]
            .as_i64()
            .unwrap();
        let mut password_project =
            load_environment(&conn, environment_id(&conn, password_id, "production")).unwrap();
        password_project.ssh_port = 0;
        assert!(validate_upload_project(&password_project).is_ok());

        let mut invalid_private_key = environment_project_payload("production");
        invalid_private_key["environmentConfig"]["sshPort"] = json!(0);
        assert_eq!(
            parse_environment_payload(&invalid_private_key)
                .err()
                .unwrap(),
            "sshPort must be between 1 and 65535"
        );

        let private_key_id =
            project_create_with_conn(&conn, &environment_project_payload("production")).unwrap()
                ["id"]
                .as_i64()
                .unwrap();
        let mut private_key_project =
            load_environment(&conn, environment_id(&conn, private_key_id, "production")).unwrap();
        private_key_project.ssh_port = 0;
        assert_eq!(
            validate_upload_project(&private_key_project).err().unwrap(),
            "SSH 端口必须在 1 到 65535 之间"
        );
    }

    #[test]
    fn password_project_round_trips_only_the_vault_entry_id() {
        let conn = test_conn();
        conn.execute_batch(
            "CREATE TABLE vault_entries (
                id INTEGER PRIMARY KEY, category TEXT NOT NULL, plain_fields TEXT
            );
            INSERT INTO vault_entries(id, category, plain_fields)
            VALUES (17, 'server', '{\"address\":\"10.0.0.8\",\"account\":\"deploy\"}');",
        )
        .unwrap();
        let mut input = environment_project_payload("production");
        input["environmentConfig"]["sshAuthType"] = json!("password");
        input["environmentConfig"]["vaultEntryId"] = json!(17);
        input["environmentConfig"]["sshHost"] = json!("");
        input["environmentConfig"]["sshUsername"] = json!("");

        let created = project_create_with_conn(&conn, &input).unwrap();
        let saved = load_environment(&conn, created["environmentId"].as_i64().unwrap()).unwrap();
        assert_eq!(saved.vault_entry_id, Some(17));
        let listed = project_list_with_conn(&conn).unwrap();
        let production = &listed["projects"][0]["environments"][1];
        assert_eq!(production["vaultEntryId"], 17);
        assert!(production.get("password").is_none());
        assert!(production.get("privateKeyPassphrase").is_none());
    }

    #[test]
    fn password_preflight_uses_bound_vault_credential_and_rejects_password_payload() {
        let conn = test_conn();
        conn.execute_batch(
            "CREATE TABLE user_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO user_settings(key, value) VALUES
                ('vault_activity_lock_enabled', 'false'),
                ('vault_system_idle_lock_enabled', 'false');
            CREATE TABLE vault_entries (
                id INTEGER PRIMARY KEY, category TEXT NOT NULL, plain_fields TEXT,
                iv TEXT NOT NULL, encrypted_blob TEXT NOT NULL
            );",
        )
        .unwrap();
        super::super::vault::insert_test_server_entry(
            &conn,
            11,
            "deploy.example",
            2200,
            "deploy",
            "secret",
        );
        let mut input = environment_project_payload("production");
        input["environmentConfig"]["sshAuthType"] = json!("password");
        input["environmentConfig"]["vaultEntryId"] = json!(11);
        input["environmentConfig"]["sshHost"] = json!("");
        input["environmentConfig"]["sshUsername"] = json!("");
        let project_id = project_create_with_conn(&conn, &input).unwrap()["id"]
            .as_i64()
            .unwrap();
        super::super::vault::install_test_session([7u8; 32]);

        let endpoint = upload_endpoint_with_conn(
            &conn,
            &load_environment(&conn, environment_id(&conn, project_id, "production")).unwrap(),
        )
        .unwrap();
        assert_eq!(endpoint.endpoint.host, "deploy.example");
        assert_eq!(endpoint.endpoint.port, 2200);
        assert_eq!(endpoint.endpoint.username, "deploy");

        let error = parse_private_key_auth_secret(&json!({ "password": "injected" }))
            .err()
            .unwrap();
        assert_eq!(error, "私钥认证不能提交密码");
        super::super::vault::force_lock();
    }

    #[test]
    fn private_key_endpoint_keeps_using_the_project_port() {
        let conn = test_conn();
        let environment_id =
            project_create_with_conn(&conn, &environment_project_payload("production")).unwrap()
                ["environmentId"]
                .as_i64()
                .unwrap();

        let endpoint =
            upload_endpoint_with_conn(&conn, &load_environment(&conn, environment_id).unwrap())
                .unwrap();

        assert_eq!(endpoint.endpoint.host, "deploy.example.internal");
        assert_eq!(endpoint.endpoint.port, 2222);
        assert_eq!(endpoint.endpoint.username, "deploy");
        assert_eq!(endpoint.vault_entry_id, None);
    }

    #[test]
    fn schema_migrates_existing_projects_and_never_persists_passwords() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE release_package_projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                output_root TEXT NOT NULL,
                frontend_project_path TEXT NOT NULL,
                frontend_build_command TEXT NOT NULL,
                frontend_artifact_path TEXT NOT NULL,
                frontend_artifact_mode TEXT NOT NULL,
                backend_project_path TEXT NOT NULL,
                backend_build_command TEXT NOT NULL,
                backend_artifact_path TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            INSERT INTO release_package_projects(
                name, output_root, frontend_project_path, frontend_build_command,
                frontend_artifact_path, frontend_artifact_mode, backend_project_path,
                backend_build_command, backend_artifact_path
            ) VALUES ('portal', 'D:\\release', 'D:\\web', 'pnpm build', 'dist',
                      'copy_directory', 'D:\\server', 'mvn package', 'target/app.jar');",
        )
        .unwrap();

        ensure_schema(&conn).unwrap();
        ensure_schema(&conn).unwrap();
        let migrated: (
            String,
            i64,
            String,
            Option<i64>,
            String,
            String,
            String,
            String,
        ) = conn
            .query_row(
                "SELECT package_type, ssh_port, ssh_auth_type, vault_entry_id,
                        frontend_success_keyword, backend_success_keyword,
                        frontend_post_upload_command, backend_post_upload_command
                 FROM release_package_environments
                 WHERE project_id=1 AND environment='production'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            migrated,
            (
                "local_archive".into(),
                22,
                "password".into(),
                None,
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            )
        );
        let environment_columns: Vec<String> = conn
            .prepare("PRAGMA table_info(release_package_environments)")
            .unwrap()
            .query_map([], |row| row.get(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(!environment_columns
            .iter()
            .any(|column| column == "password"));
        assert!(!environment_columns
            .iter()
            .any(|column| column == "private_key_passphrase"));
    }

    #[test]
    fn schema_migrates_legacy_upload_flag_to_package_type_once() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE release_package_projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                output_root TEXT NOT NULL,
                frontend_project_path TEXT NOT NULL,
                frontend_build_command TEXT NOT NULL,
                frontend_artifact_path TEXT NOT NULL,
                frontend_artifact_mode TEXT NOT NULL,
                backend_project_path TEXT NOT NULL,
                backend_build_command TEXT NOT NULL,
                backend_artifact_path TEXT NOT NULL,
                upload_enabled INTEGER NOT NULL DEFAULT 0,
                ssh_host TEXT NOT NULL DEFAULT '',
                ssh_port INTEGER NOT NULL DEFAULT 22,
                ssh_username TEXT NOT NULL DEFAULT '',
                ssh_auth_type TEXT NOT NULL DEFAULT 'password',
                ssh_private_key_path TEXT NOT NULL DEFAULT '',
                frontend_remote_dir TEXT NOT NULL DEFAULT '',
                backend_remote_path TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            INSERT INTO release_package_projects(
                name, output_root, frontend_project_path, frontend_build_command,
                frontend_artifact_path, frontend_artifact_mode, backend_project_path,
                backend_build_command, backend_artifact_path, upload_enabled
            ) VALUES
                ('local', 'D:\\release', 'D:\\web', 'pnpm build', 'dist',
                 'copy_directory', 'D:\\server', 'mvn package', 'target/app.jar', 0),
                ('upload', '', 'D:\\web', 'pnpm build', 'dist',
                 'copy_directory', 'D:\\server', 'mvn package', 'target/app.jar', 1);",
        )
        .unwrap();

        ensure_schema(&conn).unwrap();
        let local: String = conn
            .query_row(
                "SELECT package_type FROM release_package_environments
                 WHERE project_id=1 AND environment='production'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let upload: String = conn
            .query_row(
                "SELECT package_type FROM release_package_environments
                 WHERE project_id=2 AND environment='production'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(local, "local_archive");
        assert_eq!(upload, "server_upload");

        conn.execute(
            "UPDATE release_package_environments
             SET package_type='local_archive'
             WHERE project_id=2 AND environment='production'",
            [],
        )
        .unwrap();
        ensure_schema(&conn).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT package_type FROM release_package_environments
                 WHERE project_id=2 AND environment='production'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "local_archive"
        );
    }

    #[test]
    fn project_validation_depends_on_package_type() {
        let mut upload = environment_project_payload("production");
        upload["environmentConfig"]["packageType"] = json!("server_upload");
        upload["environmentConfig"]["outputRoot"] = json!("");
        assert!(parse_environment_payload(&upload).is_ok());

        upload["environmentConfig"]["sshHost"] = json!("");
        assert_eq!(
            parse_environment_payload(&upload).err().unwrap(),
            "sshHost is required for private_key authentication"
        );

        let mut local = environment_project_payload("production");
        local["environmentConfig"]["packageType"] = json!("local_archive");
        local["environmentConfig"]["sshHost"] = json!("");
        local["environmentConfig"]["sshUsername"] = json!("");
        local["environmentConfig"]["frontendRemoteDir"] = json!("");
        local["environmentConfig"]["backendRemotePath"] = json!("");
        assert!(parse_environment_payload(&local).is_ok());

        local["environmentConfig"]["outputRoot"] = json!("");
        assert_eq!(
            parse_environment_payload(&local).err().unwrap(),
            "outputRoot is required for local_archive"
        );
    }
    #[test]
    fn trusted_host_requires_explicit_replacement_when_fingerprint_changes() {
        let conn = test_conn();
        let probe = ProbeSnapshot {
            environment_id: 1,
            endpoint: RemoteEndpoint {
                host: "deploy.example.internal".into(),
                port: 22,
                username: "deploy".into(),
            },
            key_type: "ed25519".into(),
            fingerprint_sha256: "SHA256:first".into(),
        };
        trust_host_with_conn(&conn, &probe, false).unwrap();
        trust_host_with_conn(&conn, &probe, false).unwrap();

        let changed_probe = ProbeSnapshot {
            fingerprint_sha256: "SHA256:changed".into(),
            ..probe
        };
        assert!(trust_host_with_conn(&conn, &changed_probe, false).is_err());
        trust_host_with_conn(&conn, &changed_probe, true).unwrap();
        let stored = known_host_with_conn(&conn, &changed_probe.endpoint)
            .unwrap()
            .unwrap();
        assert_eq!(stored.1, "SHA256:changed");
    }
    #[test]
    fn host_trust_rotates_the_probe_token_and_rejects_reuse() {
        let conn = test_conn();
        let environment_id =
            project_create_with_conn(&conn, &environment_project_payload("production")).unwrap()
                ["environmentId"]
                .as_i64()
                .unwrap();
        let snapshot = ProbeSnapshot {
            environment_id,
            endpoint: RemoteEndpoint {
                host: "server.example.internal".into(),
                port: 22,
                username: "deploy".into(),
            },
            key_type: "ed25519".into(),
            fingerprint_sha256: "SHA256:new".into(),
        };
        let old_token = store_probe(snapshot.clone()).unwrap();
        let result = host_trust_with_conn(&conn, environment_id, &old_token, false).unwrap();
        let next_token = result["probeToken"].as_str().unwrap();
        assert_ne!(next_token, old_token);
        assert!(host_trust_with_conn(&conn, environment_id, &old_token, false).is_err());
        assert_eq!(consume_probe(next_token).unwrap(), snapshot);
    }

    #[test]
    fn host_trust_environment_mismatch_does_not_consume_the_probe_token() {
        let conn = test_conn();
        let created =
            project_create_with_conn(&conn, &environment_project_payload("test")).unwrap();
        let project_id = created["id"].as_i64().unwrap();
        let test_environment_id = created["environmentId"].as_i64().unwrap();
        let production_environment_id = environment_id(&conn, project_id, "production");
        let mut production = environment_project_payload("production");
        production["id"] = json!(project_id);
        production["environmentId"] = json!(production_environment_id);
        project_update_with_conn(&conn, &production).unwrap();

        let snapshot = ProbeSnapshot {
            environment_id: test_environment_id,
            endpoint: RemoteEndpoint {
                host: "server.example.internal".into(),
                port: 22,
                username: "deploy".into(),
            },
            key_type: "ed25519".into(),
            fingerprint_sha256: "SHA256:test".into(),
        };
        let probe_token = store_probe(snapshot).unwrap();

        assert_eq!(
            host_trust_with_conn(&conn, production_environment_id, &probe_token, false)
                .unwrap_err(),
            "SSH 探测令牌与当前环境不匹配"
        );
        assert!(host_trust_with_conn(&conn, test_environment_id, &probe_token, false).is_ok());
    }

    #[test]
    fn local_archive_cannot_trust_a_host_and_does_not_consume_the_probe_token() {
        let conn = test_conn();
        let mut local = environment_project_payload("production");
        local["environmentConfig"]["packageType"] = json!("local_archive");
        let environment_id = project_create_with_conn(&conn, &local).unwrap()["environmentId"]
            .as_i64()
            .unwrap();
        let snapshot = ProbeSnapshot {
            environment_id,
            endpoint: RemoteEndpoint {
                host: "server.example.internal".into(),
                port: 22,
                username: "deploy".into(),
            },
            key_type: "ed25519".into(),
            fingerprint_sha256: "SHA256:local".into(),
        };
        let probe_token = store_probe(snapshot.clone()).unwrap();

        assert!(
            host_trust_with_conn(&conn, environment_id, &probe_token, false)
                .unwrap_err()
                .contains("server_upload")
        );
        assert_eq!(consume_probe(&probe_token).unwrap(), snapshot);
    }

    #[test]
    fn private_key_authentication_payload_rejects_passwords() {
        assert!(matches!(
            parse_private_key_auth_secret(&json!({ "privateKeyPassphrase": "secret" })).unwrap(),
            AuthSecret::PrivateKeyPassphrase(Some(_))
        ));
        assert!(parse_private_key_auth_secret(&json!({ "password": "wrong" })).is_err());
    }
    #[test]
    fn project_crud_round_trip() {
        let conn = test_conn();
        let created =
            project_create_with_conn(&conn, &environment_project_payload("production")).unwrap();
        let id = created["id"].as_i64().unwrap();
        let environment_id = created["environmentId"].as_i64().unwrap();
        let listed = project_list_with_conn(&conn).unwrap();
        assert_eq!(listed["projects"][0]["name"], "客户门户");
        let production = &listed["projects"][0]["environments"][1];
        assert_eq!(production["sshHost"], "deploy.example.internal");
        assert_eq!(production["sshPort"], 2222);
        assert_eq!(production["frontendSuccessKeyword"], "Build completed");
        assert_eq!(production["backendSuccessKeyword"], "BUILD SUCCESS");
        assert_eq!(production["healthCheckEnabled"], true);
        assert_eq!(production["healthCheckUrl"], "https://portal.example.com/health");
        assert_eq!(production["healthCheckMaxRetries"], 4);
        assert_eq!(
            production["frontendPostUploadCommand"],
            "cd /srv/web\n  ./reload.sh"
        );
        assert_eq!(
            production["backendPostUploadCommand"],
            "systemctl restart portal"
        );
        assert!(production.get("password").is_none());
        assert!(production.get("privateKeyPassphrase").is_none());
        let mut update = environment_project_payload("production");
        update["id"] = json!(id);
        update["environmentId"] = json!(environment_id);
        update["project"]["name"] = json!("客户门户 Pro");
        update["environmentConfig"]["backendPostUploadCommand"] =
            json!("systemctl restart portal-pro");
        update["environmentConfig"]["healthCheckMaxRetries"] = json!(8);
        project_update_with_conn(&conn, &update).unwrap();
        let updated = load_environment(&conn, environment_id).unwrap();
        assert_eq!(updated.project_name, "客户门户 Pro");
        assert_eq!(
            updated.backend_post_upload_command,
            "systemctl restart portal-pro"
        );
        assert!(updated.health_check_enabled);
        assert_eq!(updated.health_check_url, "https://portal.example.com/health");
        assert_eq!(updated.health_check_max_retries, 8);
        project_delete_with_conn(&conn, &json!({ "id": id })).unwrap();
        assert!(load_environment(&conn, environment_id).is_err());
    }

    #[test]
    fn prepare_uses_project_output_root_and_inclusive_thursday() {
        let conn = test_conn();
        let mut project = environment_project_payload("production");
        project["environmentConfig"]["packageType"] = json!("local_archive");
        let environment_id = project_create_with_conn(&conn, &project).unwrap()["environmentId"]
            .as_i64()
            .unwrap();
        let out = prepare_with_conn(
            &conn,
            environment_id,
            NaiveDate::from_ymd_opt(2026, 7, 23).unwrap(),
        )
        .unwrap();
        assert_eq!(out["packageType"], "local_archive");
        assert_eq!(out["defaultFolderName"], "20260723-客户门户");
        assert_eq!(out["archivePath"], r"D:\releases\20260723-客户门户");
    }

    #[test]
    fn prepare_returns_a_discriminated_result_without_archive_for_upload() {
        let conn = test_conn();
        let mut project = environment_project_payload("production");
        project["environmentConfig"]["outputRoot"] = json!("");
        let environment_id = project_create_with_conn(&conn, &project).unwrap()["environmentId"]
            .as_i64()
            .unwrap();

        let out = prepare_with_conn(
            &conn,
            environment_id,
            NaiveDate::from_ymd_opt(2026, 7, 23).unwrap(),
        )
        .unwrap();

        assert_eq!(out, json!({ "packageType": "server_upload" }));
    }

    #[test]
    fn run_targets_must_be_known_unique_and_non_empty() {
        assert_eq!(
            parse_targets(&json!(["frontend", "backend"])).unwrap(),
            vec![ReleaseTarget::Frontend, ReleaseTarget::Backend]
        );
        assert!(parse_targets(&json!([])).unwrap_err().contains("至少选择"));
        assert!(parse_targets(&json!(["frontend", "frontend"])).is_err());
        assert!(parse_targets(&json!(["mobile"])).is_err());
    }

    #[test]
    fn start_input_parses_only_parameters_for_the_project_type() {
        let local = parse_start_input(
            ReleasePackageType::LocalArchive,
            &json!({ "folderName": "release", "overwriteExisting": true }),
        )
        .unwrap();
        assert!(matches!(
            local,
            ReleaseStartInput::LocalArchive {
                folder_name,
                overwrite_existing: true,
            } if folder_name == "release"
        ));

        let upload = parse_start_input(
            ReleasePackageType::ServerUpload,
            &json!({
                "preflightToken": "token",
                "overwriteRemoteTargets": ["frontend"]
            }),
        )
        .unwrap();
        assert!(matches!(
            upload,
            ReleaseStartInput::ServerUpload {
                preflight_token,
                overwrite_remote_targets,
            } if preflight_token == "token"
                && overwrite_remote_targets == vec![ReleaseTarget::Frontend]
        ));

        assert!(parse_start_input(
            ReleasePackageType::ServerUpload,
            &json!({ "overwriteRemoteTargets": [] }),
        )
        .unwrap_err()
        .contains("preflightToken"));
        assert!(parse_start_input(
            ReleasePackageType::ServerUpload,
            &json!({
                "preflightToken": "token",
                "overwriteRemoteTargets": ["frontend", "frontend"]
            }),
        )
        .is_err());
    }

    #[test]
    fn production_start_requires_explicit_confirmation() {
        assert!(
            validate_start_confirmation(ReleasePackageEnvironmentKind::Test, &json!({})).is_ok()
        );
        assert_eq!(
            validate_start_confirmation(
                ReleasePackageEnvironmentKind::Test,
                &json!({ "productionConfirmed": false })
            )
            .unwrap_err(),
            "测试环境启动不能携带生产确认参数"
        );
        assert_eq!(
            validate_start_confirmation(ReleasePackageEnvironmentKind::Production, &json!({}))
                .unwrap_err(),
            "生产环境发布需要明确确认"
        );
        assert_eq!(
            validate_start_confirmation(
                ReleasePackageEnvironmentKind::Production,
                &json!({ "productionConfirmed": true })
            ),
            Ok(())
        );
        assert_eq!(
            validate_start_confirmation(
                ReleasePackageEnvironmentKind::Production,
                &json!({ "productionConfirmed": "true" })
            )
            .unwrap_err(),
            "生产环境发布需要明确确认"
        );
    }

    fn init_git_repository(path: &Path, branch: &str) {
        fs::create_dir_all(path).unwrap();
        let output = std::process::Command::new("git")
            .args(["init", "-b", branch])
            .arg(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn production_branch_check_uses_only_selected_targets_and_reports_mismatch() {
        let root = std::env::temp_dir().join(format!(
            "lazycat-release-branch-test-{}",
            uuid::Uuid::new_v4()
        ));
        let frontend = root.join("frontend");
        let backend = root.join("backend");
        init_git_repository(&frontend, "master");
        init_git_repository(&backend, "release");

        let conn = test_conn();
        let mut payload = environment_project_payload("production");
        payload["project"]["frontendProjectPath"] = json!(frontend.to_string_lossy());
        payload["project"]["backendProjectPath"] = json!(backend.to_string_lossy());
        let environment_id = project_create_with_conn(&conn, &payload).unwrap()["environmentId"]
            .as_i64()
            .unwrap();
        let project = load_environment(&conn, environment_id).unwrap();

        let frontend_only =
            inspect_production_branches(&project, &[ReleaseTarget::Frontend]).unwrap();
        assert_eq!(frontend_only.len(), 1);
        assert!(frontend_only[0].matches);
        assert_eq!(frontend_only[0].current_branch.as_deref(), Some("master"));
        assert!(validate_production_branches(&project, &[ReleaseTarget::Frontend]).is_ok());

        let result = branch_check_with_conn(
            &conn,
            &json!({
                "environmentId": environment_id,
                "targets": ["frontend", "backend"]
            }),
        )
        .unwrap();
        assert_eq!(result["checks"][0]["matches"], true);
        assert_eq!(result["checks"][1]["currentBranch"], "release");
        assert_eq!(result["checks"][1]["expectedBranch"], "master");
        assert_eq!(result["checks"][1]["matches"], false);
        assert_eq!(
            validate_production_branches(
                &project,
                &[ReleaseTarget::Frontend, ReleaseTarget::Backend]
            )
            .unwrap_err(),
            "后端生产分支不匹配：当前为 release，要求为 master"
        );

        let mut test_project = project;
        test_project.environment = ReleasePackageEnvironmentKind::Test;
        assert!(validate_production_branches(
            &test_project,
            &[ReleaseTarget::Frontend, ReleaseTarget::Backend]
        )
        .is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn production_branch_check_rejects_non_git_directories() {
        let root = std::env::temp_dir().join(format!(
            "lazycat-release-non-git-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        let error = inspect_git_head(root.to_string_lossy().as_ref(), "前端").unwrap_err();
        assert!(error.contains("前端工程目录不是 Git 工作区"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn start_checks_production_confirmation_before_starting_the_runtime() {
        let source = include_str!("release_package.rs");
        let execute = source.find("pub fn execute_with_app(").unwrap();
        let start = source[execute..].find("\"start\" => {").unwrap() + execute;
        let end = source[start..].find("\"upload_retry\" => {").unwrap() + start;
        let start_action = &source[start..end];

        let confirmation = start_action.find("validate_start_confirmation(").unwrap();
        let branch_check = start_action.find("validate_production_branches(").unwrap();
        let runtime_start = start_action
            .find("super::release_package_runtime::start(")
            .unwrap();
        let consume_upload_token = start_action.find("consume_deploy_authorization(").unwrap();
        assert!(confirmation < branch_check);
        assert!(branch_check < runtime_start);
        assert!(branch_check < consume_upload_token);
    }

    #[test]
    fn action_dispatch_id_is_optional_but_strict() {
        assert_eq!(parse_action_dispatch_id(&json!({})).unwrap(), None);
        assert_eq!(
            parse_action_dispatch_id(&json!({ "actionDispatchId": "dispatch-1" })).unwrap(),
            Some("dispatch-1".into())
        );
        assert!(parse_action_dispatch_id(&json!({ "actionDispatchId": "" })).is_err());
        assert!(parse_action_dispatch_id(&json!({ "actionDispatchId": 1 })).is_err());
        assert!(parse_action_dispatch_id(&json!({ "actionDispatchId": null })).is_err());
    }

    #[test]
    fn start_input_rejects_parameters_from_the_other_package_type() {
        assert!(parse_start_input(
            ReleasePackageType::LocalArchive,
            &json!({ "folderName": "release", "preflightToken": "token" }),
        )
        .is_err());
        assert!(parse_start_input(
            ReleasePackageType::ServerUpload,
            &json!({ "folderName": "release", "preflightToken": "token" }),
        )
        .is_err());
    }

    #[test]
    fn start_input_rejects_the_obsolete_mode_parameter() {
        assert!(parse_start_input(
            ReleasePackageType::LocalArchive,
            &json!({ "folderName": "release", "mode": "package_only" }),
        )
        .is_err());
        assert!(parse_start_input(
            ReleasePackageType::ServerUpload,
            &json!({ "preflightToken": "token", "mode": "package_and_upload" }),
        )
        .is_err());
    }

    #[test]
    fn type_specific_actions_reject_the_other_package_type() {
        let conn = test_conn();
        let upload_environment_id =
            project_create_with_conn(&conn, &environment_project_payload("production")).unwrap()
                ["environmentId"]
                .as_i64()
                .unwrap();
        assert!(
            target_check_with_conn(&conn, upload_environment_id, "release")
                .unwrap_err()
                .contains("local_archive")
        );

        let mut local = environment_project_payload("production");
        local["environmentConfig"]["packageType"] = json!("local_archive");
        local["environmentConfig"]["sshHost"] = json!("");
        let local_environment_id = project_create_with_conn(&conn, &local).unwrap()
            ["environmentId"]
            .as_i64()
            .unwrap();
        assert!(remote_probe_with_conn(&conn, local_environment_id)
            .unwrap_err()
            .contains("server_upload"));
        assert!(remote_preflight_with_conn(
            &conn,
            &json!({
                "environmentId": local_environment_id,
                "targets": ["frontend"],
                "probeToken": "unused"
            }),
        )
        .unwrap_err()
        .contains("server_upload"));
    }

    #[test]
    fn server_upload_config_is_validated_independently() {
        let conn = test_conn();
        let id = project_create_with_conn(&conn, &environment_project_payload("production"))
            .unwrap()["id"]
            .as_i64()
            .unwrap();
        let project = load_environment(&conn, environment_id(&conn, id, "production")).unwrap();

        assert!(validate_upload_project(&project).is_ok());
    }

    #[test]
    fn overwrite_requires_an_explicit_boolean_and_controls_existing_targets() {
        assert!(!parse_overwrite_existing(&json!({})).unwrap());
        assert!(parse_overwrite_existing(&json!({ "overwriteExisting": true })).unwrap());
        assert!(parse_overwrite_existing(&json!({ "overwriteExisting": "true" })).is_err());

        let root = std::env::temp_dir().join(format!(
            "lazycat-release-overwrite-input-test-{}",
            uuid::Uuid::new_v4()
        ));
        let backend = root.join("backend");
        let output = root.join("output");
        fs::create_dir_all(&backend).unwrap();
        fs::create_dir_all(output.join("release")).unwrap();
        let project = ReleasePackageEnvironmentConfig {
            id: 1,
            project_id: 1,
            project_name: "test".into(),
            environment: ReleasePackageEnvironmentKind::Test,
            configured: true,
            output_root: output.to_string_lossy().into_owned(),
            package_type: ReleasePackageType::LocalArchive,
            frontend_project_path: root.join("missing-frontend").to_string_lossy().into_owned(),
            frontend_expected_branch: "master".into(),
            frontend_build_command: "exit 0".into(),
            frontend_success_keyword: String::new(),
            frontend_post_upload_command: String::new(),
            frontend_artifact_path: "dist".into(),
            frontend_artifact_mode: "copy_directory".into(),
            backend_project_path: backend.to_string_lossy().into_owned(),
            backend_expected_branch: "master".into(),
            backend_build_command: "exit 0".into(),
            backend_success_keyword: String::new(),
            backend_post_upload_command: String::new(),
            backend_artifact_path: "app.jar".into(),
            ssh_host: String::new(),
            ssh_port: 22,
            ssh_username: String::new(),
            ssh_auth_type: "password".into(),
            vault_entry_id: None,
            ssh_private_key_path: String::new(),
            frontend_remote_dir: String::new(),
            backend_remote_path: String::new(),
            health_check_enabled: false,
            health_check_url: String::new(),
            health_check_max_retries: 6,
            created_at: String::new(),
            updated_at: String::new(),
        };

        assert!(
            validate_run_inputs(&project, "release", &[ReleaseTarget::Backend], false,).is_err()
        );
        assert!(validate_run_inputs(&project, "release", &[ReleaseTarget::Backend], true,).is_ok());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn target_check_reports_existing_directory_and_rejects_file() {
        assert!(supported_actions().contains(&"target_check"));
        let root = std::env::temp_dir().join(format!(
            "lazycat-release-target-check-test-{}",
            uuid::Uuid::new_v4()
        ));
        let output = root.join("output");
        fs::create_dir_all(&output).unwrap();
        let conn = test_conn();
        let mut project = environment_project_payload("production");
        project["environmentConfig"]["packageType"] = json!("local_archive");
        project["environmentConfig"]["outputRoot"] = json!(output.to_string_lossy());
        let environment_id = project_create_with_conn(&conn, &project).unwrap()["environmentId"]
            .as_i64()
            .unwrap();

        let missing = target_check_with_conn(&conn, environment_id, "release").unwrap();
        assert_eq!(missing["exists"], false);
        assert_eq!(
            missing["archivePath"],
            output.join("release").to_string_lossy().as_ref()
        );

        fs::create_dir(output.join("release")).unwrap();
        let existing = target_check_with_conn(&conn, environment_id, "release").unwrap();
        assert_eq!(existing["exists"], true);

        fs::remove_dir(output.join("release")).unwrap();
        fs::write(output.join("release"), "file").unwrap();
        assert!(target_check_with_conn(&conn, environment_id, "release")
            .unwrap_err()
            .contains("不是文件夹"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn remote_discard_action_revokes_both_token_types_idempotently() {
        assert!(supported_actions().contains(&"remote_discard"));
        let snapshot = ProbeSnapshot {
            environment_id: 7,
            endpoint: RemoteEndpoint {
                host: "server.example.internal".into(),
                port: 22,
                username: "deploy".into(),
            },
            key_type: "ed25519".into(),
            fingerprint_sha256: "SHA256:key".into(),
        };
        let probe_token = store_probe(snapshot).unwrap();
        let binding = PreflightBinding {
            environment_id: 7,
            project_id: 7,
            environment: ReleasePackageEnvironmentKind::Test,
            endpoint: RemoteEndpoint {
                host: "server.example.internal".into(),
                port: 22,
                username: "deploy".into(),
            },
            auth_type: "private_key".into(),
            vault_entry_id: None,
            private_key_path: r"C:\Users\tester\.ssh\lazycat".into(),
            targets: vec![RemoteTarget::Backend],
            command_retry_token: None,
            frontend_remote_dir: "/srv/app/web".into(),
            backend_remote_path: "/srv/app/app.jar".into(),
        };
        let preflight = issue_preflight(
            binding.clone(),
            "SHA256:key".into(),
            AuthSecret::PrivateKeyPassphrase(Some(Zeroizing::new("secret".into()))),
            &[],
        )
        .unwrap();
        let payload = json!({
            "probeToken": probe_token,
            "preflightToken": preflight.token,
        });

        remote_discard(&payload).unwrap();

        assert!(consume_probe(payload["probeToken"].as_str().unwrap()).is_err());
        assert!(crate::tools::release_package_remote::consume_preflight(
            payload["preflightToken"].as_str().unwrap(),
            &binding,
        )
        .is_err());
        remote_discard(&payload).unwrap();
    }

    #[test]
    fn run_validation_only_checks_selected_project_directory() {
        let root = std::env::temp_dir().join(format!(
            "lazycat-release-input-test-{}",
            uuid::Uuid::new_v4()
        ));
        let backend = root.join("backend");
        let output = root.join("output");
        fs::create_dir_all(&backend).unwrap();
        fs::create_dir_all(&output).unwrap();
        let project = ReleasePackageEnvironmentConfig {
            id: 1,
            project_id: 1,
            project_name: "test".into(),
            environment: ReleasePackageEnvironmentKind::Test,
            configured: true,
            output_root: output.to_string_lossy().into_owned(),
            package_type: ReleasePackageType::LocalArchive,
            frontend_project_path: root.join("missing-frontend").to_string_lossy().into_owned(),
            frontend_expected_branch: "master".into(),
            frontend_build_command: "exit 0".into(),
            frontend_success_keyword: String::new(),
            frontend_post_upload_command: String::new(),
            frontend_artifact_path: "dist".into(),
            frontend_artifact_mode: "copy_directory".into(),
            backend_project_path: backend.to_string_lossy().into_owned(),
            backend_expected_branch: "master".into(),
            backend_build_command: "exit 0".into(),
            backend_success_keyword: String::new(),
            backend_post_upload_command: String::new(),
            backend_artifact_path: "app.jar".into(),
            ssh_host: String::new(),
            ssh_port: 22,
            ssh_username: String::new(),
            ssh_auth_type: "password".into(),
            vault_entry_id: None,
            ssh_private_key_path: String::new(),
            frontend_remote_dir: String::new(),
            backend_remote_path: String::new(),
            health_check_enabled: false,
            health_check_url: String::new(),
            health_check_max_retries: 6,
            created_at: String::new(),
            updated_at: String::new(),
        };

        let result = validate_run_inputs(&project, "release", &[ReleaseTarget::Backend], false);
        let _ = fs::remove_dir_all(&root);

        assert!(result.is_ok());
    }
}

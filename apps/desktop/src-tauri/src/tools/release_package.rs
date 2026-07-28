use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;

use super::helpers::db_conn;
use super::release_package_archive::{default_folder_name, validate_folder_name};
use super::release_package_remote::run_command_preflight;
use super::release_package_remote::{
    classify_trust, consume_probe, discard_preflight, discard_probe, issue_preflight, load_probe,
    probe_host, run_remote_preflight, store_probe, validate_remote_dir, validate_remote_file,
    AuthSecret, HostTrust, PreflightBinding, ProbeSnapshot, RemoteEndpoint, RemoteTarget,
};
use zeroize::Zeroizing;

pub const LEGACY_OUTPUT_ROOT_KEY: &str = "release_package.output_root";
pub const RELEASE_PACKAGE_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS release_package_projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    output_root TEXT NOT NULL,
    package_type TEXT NOT NULL DEFAULT 'local_archive' CHECK (package_type IN ('local_archive', 'server_upload')),
    frontend_project_path TEXT NOT NULL,
    frontend_build_command TEXT NOT NULL,
    frontend_success_keyword TEXT NOT NULL DEFAULT '',
    frontend_post_upload_command TEXT NOT NULL DEFAULT '',
    frontend_artifact_path TEXT NOT NULL,
    frontend_artifact_mode TEXT NOT NULL CHECK (frontend_artifact_mode IN ('copy_directory', 'zip_directory')),
    backend_project_path TEXT NOT NULL,
    backend_build_command TEXT NOT NULL,
    backend_success_keyword TEXT NOT NULL DEFAULT '',
    backend_post_upload_command TEXT NOT NULL DEFAULT '',
    backend_artifact_path TEXT NOT NULL,
    upload_enabled INTEGER NOT NULL DEFAULT 0,
    ssh_host TEXT NOT NULL DEFAULT '',
    ssh_port INTEGER NOT NULL DEFAULT 22,
    ssh_username TEXT NOT NULL DEFAULT '',
    ssh_auth_type TEXT NOT NULL DEFAULT 'password' CHECK (ssh_auth_type IN ('password', 'private_key')),
    vault_entry_id INTEGER NULL,
    ssh_private_key_path TEXT NOT NULL DEFAULT '',
    frontend_remote_dir TEXT NOT NULL DEFAULT '',
    backend_remote_path TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
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

pub fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(RELEASE_PACKAGE_SCHEMA_SQL)
        .map_err(|error| format!("create release package schema failed: {error}"))?;
    for (column, statement) in [
        ("output_root", "ALTER TABLE release_package_projects ADD COLUMN output_root TEXT NOT NULL DEFAULT ''"),
        ("upload_enabled", "ALTER TABLE release_package_projects ADD COLUMN upload_enabled INTEGER NOT NULL DEFAULT 0"),
        ("ssh_host", "ALTER TABLE release_package_projects ADD COLUMN ssh_host TEXT NOT NULL DEFAULT ''"),
        ("ssh_port", "ALTER TABLE release_package_projects ADD COLUMN ssh_port INTEGER NOT NULL DEFAULT 22"),
        ("ssh_username", "ALTER TABLE release_package_projects ADD COLUMN ssh_username TEXT NOT NULL DEFAULT ''"),
        ("ssh_auth_type", "ALTER TABLE release_package_projects ADD COLUMN ssh_auth_type TEXT NOT NULL DEFAULT 'password'"),
        ("vault_entry_id", "ALTER TABLE release_package_projects ADD COLUMN vault_entry_id INTEGER NULL"),
        ("ssh_private_key_path", "ALTER TABLE release_package_projects ADD COLUMN ssh_private_key_path TEXT NOT NULL DEFAULT ''"),
        ("frontend_remote_dir", "ALTER TABLE release_package_projects ADD COLUMN frontend_remote_dir TEXT NOT NULL DEFAULT ''"),
        ("backend_remote_path", "ALTER TABLE release_package_projects ADD COLUMN backend_remote_path TEXT NOT NULL DEFAULT ''"),
        ("frontend_success_keyword", "ALTER TABLE release_package_projects ADD COLUMN frontend_success_keyword TEXT NOT NULL DEFAULT ''"),
        ("backend_success_keyword", "ALTER TABLE release_package_projects ADD COLUMN backend_success_keyword TEXT NOT NULL DEFAULT ''"),
        ("frontend_post_upload_command", "ALTER TABLE release_package_projects ADD COLUMN frontend_post_upload_command TEXT NOT NULL DEFAULT ''"),
        ("backend_post_upload_command", "ALTER TABLE release_package_projects ADD COLUMN backend_post_upload_command TEXT NOT NULL DEFAULT ''"),
    ] {
        let mut query = conn
            .prepare("PRAGMA table_info(release_package_projects)")
            .map_err(|error| format!("inspect release package schema failed: {error}"))?;
        let columns = query
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|error| format!("query release package schema failed: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("read release package schema failed: {error}"))?;
        let exists = columns.iter().any(|name| name == column);
        if !exists {
            conn.execute_batch(statement).map_err(|error| {
                format!("migrate release package column {column} failed: {error}")
            })?;
        }
    }
    let package_type_exists = conn
        .prepare("PRAGMA table_info(release_package_projects)")
        .and_then(|mut query| {
            query
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(|error| format!("inspect release package type schema failed: {error}"))?
        .iter()
        .any(|name| name == "package_type");
    if !package_type_exists {
        conn.execute_batch(
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
    Ok(())
}

const ACTIONS: &[&str] = &[
    "project_list",
    "project_create",
    "project_update",
    "project_delete",
    "prepare",
    "target_check",
    "remote_probe",
    "host_trust",
    "remote_preflight",
    "remote_discard",
    "command_retry_prepare",
    "command_retry_preflight",
    "start",
    "upload_retry",
    "cancel",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleasePackageType {
    LocalArchive,
    ServerUpload,
}

impl ReleasePackageType {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "local_archive" => Ok(Self::LocalArchive),
            "server_upload" => Ok(Self::ServerUpload),
            _ => Err("packageType must be local_archive or server_upload".into()),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::LocalArchive => "local_archive",
            Self::ServerUpload => "server_upload",
        }
    }
}

fn require_package_type(
    project: &ReleasePackageProjectConfig,
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackageProjectConfig {
    pub id: i64,
    pub name: String,
    pub output_root: String,
    pub package_type: ReleasePackageType,
    pub frontend_project_path: String,
    pub frontend_build_command: String,
    pub frontend_success_keyword: String,
    pub frontend_post_upload_command: String,
    pub frontend_artifact_path: String,
    pub frontend_artifact_mode: String,
    pub backend_project_path: String,
    pub backend_build_command: String,
    pub backend_success_keyword: String,
    pub backend_post_upload_command: String,
    pub backend_artifact_path: String,
    pub ssh_host: String,
    pub ssh_port: u16,
    pub ssh_username: String,
    pub ssh_auth_type: String,
    pub vault_entry_id: Option<i64>,
    pub ssh_private_key_path: String,
    pub frontend_remote_dir: String,
    pub backend_remote_path: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareResult {
    pub package_type: ReleasePackageType,
    pub default_folder_name: String,
    pub output_root: String,
    pub archive_path: String,
    pub frontend_artifact_mode: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseTarget {
    Frontend,
    Backend,
}

struct ProjectPayload {
    name: String,
    output_root: String,
    package_type: ReleasePackageType,
    frontend_project_path: String,
    frontend_build_command: String,
    frontend_success_keyword: String,
    frontend_post_upload_command: String,
    frontend_artifact_path: String,
    frontend_artifact_mode: String,
    backend_project_path: String,
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

fn parse_action_dispatch_id(payload: &Value) -> Result<Option<String>, String> {
    match payload.get("actionDispatchId") {
        None => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => {
            Ok(Some(value.trim().to_string()))
        }
        Some(_) => Err("actionDispatchId must be a non-empty string".into()),
    }
}

fn parse_project_payload(payload: &Value) -> Result<ProjectPayload, String> {
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

    let project = ProjectPayload {
        name: required_string(payload, "name")?,
        output_root: optional_string(payload, "outputRoot")?,
        package_type: ReleasePackageType::parse(&required_string(payload, "packageType")?)?,
        frontend_project_path: required_string(payload, "frontendProjectPath")?,
        frontend_build_command: required_string(payload, "frontendBuildCommand")?,
        frontend_success_keyword: optional_string(payload, "frontendSuccessKeyword")?,
        frontend_post_upload_command: optional_string(payload, "frontendPostUploadCommand")?,
        frontend_artifact_path: required_string(payload, "frontendArtifactPath")?,
        frontend_artifact_mode: required_string(payload, "frontendArtifactMode")?,
        backend_project_path: required_string(payload, "backendProjectPath")?,
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
    };
    validate_folder_name(&project.name)?;
    if project.package_type == ReleasePackageType::LocalArchive && project.output_root.is_empty() {
        return Err("outputRoot is required for local_archive".into());
    }
    if project.package_type == ReleasePackageType::ServerUpload {
        if project.ssh_auth_type == "password" {
            if project.vault_entry_id.is_none() {
                return Err("vaultEntryId is required for password authentication".into());
            }
        } else {
            if project.ssh_host.is_empty() {
                return Err("sshHost is required for private_key authentication".into());
            }
            if project.ssh_username.is_empty() {
                return Err("sshUsername is required for private_key authentication".into());
            }
            if project.ssh_private_key_path.is_empty() {
                return Err("sshPrivateKeyPath is required for private_key authentication".into());
            }
        }
        if !project.frontend_remote_dir.starts_with('/') || project.frontend_remote_dir == "/" {
            return Err("frontendRemoteDir must be an absolute Linux path".into());
        }
        if !project.backend_remote_path.starts_with('/') || project.backend_remote_path == "/" {
            return Err("backendRemotePath must be an absolute Linux path".into());
        }
    }
    if !matches!(
        project.frontend_artifact_mode.as_str(),
        "copy_directory" | "zip_directory"
    ) {
        return Err("frontendArtifactMode must be copy_directory or zip_directory".into());
    }
    Ok(project)
}

fn validate_vault_binding(conn: &Connection, project: &ProjectPayload) -> Result<(), String> {
    if project.package_type != ReleasePackageType::ServerUpload
        || project.ssh_auth_type != "password"
    {
        return Ok(());
    }
    let entry_id = project
        .vault_entry_id
        .ok_or("vaultEntryId is required for password authentication")?;
    super::vault::server_credential_metadata(conn, entry_id)?;
    Ok(())
}

fn project_from_row(row: &Row<'_>) -> rusqlite::Result<ReleasePackageProjectConfig> {
    let package_type = ReleasePackageType::parse(&row.get::<_, String>(10)?)
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(ReleasePackageProjectConfig {
        id: row.get(0)?,
        name: row.get(1)?,
        output_root: row.get(2)?,
        frontend_project_path: row.get(3)?,
        frontend_build_command: row.get(4)?,
        frontend_artifact_path: row.get(5)?,
        frontend_artifact_mode: row.get(6)?,
        backend_project_path: row.get(7)?,
        backend_build_command: row.get(8)?,
        backend_artifact_path: row.get(9)?,
        package_type,
        ssh_host: row.get(11)?,
        ssh_port: row.get(12)?,
        ssh_username: row.get(13)?,
        ssh_auth_type: row.get(14)?,
        vault_entry_id: row.get(15)?,
        ssh_private_key_path: row.get(16)?,
        frontend_remote_dir: row.get(17)?,
        backend_remote_path: row.get(18)?,
        frontend_success_keyword: row.get(19)?,
        backend_success_keyword: row.get(20)?,
        frontend_post_upload_command: row.get(21)?,
        backend_post_upload_command: row.get(22)?,
        created_at: row.get(23)?,
        updated_at: row.get(24)?,
    })
}

fn project_list_with_conn(conn: &Connection) -> Result<Value, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, output_root, frontend_project_path, frontend_build_command, frontend_artifact_path,
                    frontend_artifact_mode, backend_project_path, backend_build_command,
                    backend_artifact_path, package_type, ssh_host, ssh_port, ssh_username, ssh_auth_type,
                    vault_entry_id, ssh_private_key_path, frontend_remote_dir, backend_remote_path,
                    frontend_success_keyword, backend_success_keyword, frontend_post_upload_command,
                    backend_post_upload_command, created_at, updated_at
             FROM release_package_projects
             ORDER BY name COLLATE NOCASE ASC, id ASC",
        )
        .map_err(|e| format!("prepare release package project list failed: {e}"))?;
    let projects = stmt
        .query_map([], project_from_row)
        .map_err(|e| format!("query release package projects failed: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read release package project failed: {e}"))?;
    Ok(json!({ "projects": projects }))
}

pub(crate) fn list_action_target_rows(conn: &Connection) -> Result<Vec<(i64, String)>, String> {
    let mut statement = conn
        .prepare(
            "SELECT id, name
             FROM release_package_projects
             ORDER BY name COLLATE NOCASE ASC, id ASC",
        )
        .map_err(|error| format!("prepare release package action targets failed: {error}"))?;
    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|error| format!("query release package action targets failed: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read release package action target failed: {error}"))?;
    Ok(rows)
}

pub(crate) fn load_action_target_label(
    conn: &Connection,
    id: i64,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT name FROM release_package_projects WHERE id=?1",
        [id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| format!("load release package action target failed: {error}"))
}

fn project_create_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let project = parse_project_payload(payload)?;
    validate_vault_binding(conn, &project)?;
    conn.execute(
        "INSERT INTO release_package_projects(
            name, output_root, frontend_project_path, frontend_build_command, frontend_artifact_path,
            frontend_artifact_mode, backend_project_path, backend_build_command, backend_artifact_path,
            package_type, ssh_host, ssh_port, ssh_username, ssh_auth_type, vault_entry_id,
            ssh_private_key_path, frontend_remote_dir, backend_remote_path,
            frontend_success_keyword, backend_success_keyword, frontend_post_upload_command,
            backend_post_upload_command
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
        params![
            project.name,
            project.output_root,
            project.frontend_project_path,
            project.frontend_build_command,
            project.frontend_artifact_path,
            project.frontend_artifact_mode,
            project.backend_project_path,
            project.backend_build_command,
            project.backend_artifact_path,
            project.package_type.as_str(),
            project.ssh_host,
            project.ssh_port,
            project.ssh_username,
            project.ssh_auth_type,
            project.vault_entry_id,
            project.ssh_private_key_path,
            project.frontend_remote_dir,
            project.backend_remote_path,
            project.frontend_success_keyword,
            project.backend_success_keyword,
            project.frontend_post_upload_command,
            project.backend_post_upload_command,
        ],
    )
    .map_err(|e| format!("create release package project failed: {e}"))?;
    Ok(json!({ "id": conn.last_insert_rowid() }))
}

fn project_update_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let project = parse_project_payload(payload)?;
    validate_vault_binding(conn, &project)?;
    let changed = conn
        .execute(
            "UPDATE release_package_projects SET
                name=?1, output_root=?2, frontend_project_path=?3, frontend_build_command=?4,
                frontend_artifact_path=?5, frontend_artifact_mode=?6,
                backend_project_path=?7, backend_build_command=?8, backend_artifact_path=?9,
                package_type=?10, ssh_host=?11, ssh_port=?12, ssh_username=?13,
                ssh_auth_type=?14, vault_entry_id=?15, ssh_private_key_path=?16,
                frontend_remote_dir=?17, backend_remote_path=?18,
                frontend_success_keyword=?19, backend_success_keyword=?20,
                frontend_post_upload_command=?21, backend_post_upload_command=?22,
                updated_at=CURRENT_TIMESTAMP
             WHERE id=?23",
            params![
                project.name,
                project.output_root,
                project.frontend_project_path,
                project.frontend_build_command,
                project.frontend_artifact_path,
                project.frontend_artifact_mode,
                project.backend_project_path,
                project.backend_build_command,
                project.backend_artifact_path,
                project.package_type.as_str(),
                project.ssh_host,
                project.ssh_port,
                project.ssh_username,
                project.ssh_auth_type,
                project.vault_entry_id,
                project.ssh_private_key_path,
                project.frontend_remote_dir,
                project.backend_remote_path,
                project.frontend_success_keyword,
                project.backend_success_keyword,
                project.frontend_post_upload_command,
                project.backend_post_upload_command,
                id,
            ],
        )
        .map_err(|e| format!("update release package project failed: {e}"))?;
    if changed == 0 {
        return Err("release package project not found".into());
    }
    Ok(json!({ "id": id }))
}

fn project_delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let changed = conn
        .execute("DELETE FROM release_package_projects WHERE id=?1", [id])
        .map_err(|e| format!("delete release package project failed: {e}"))?;
    if changed == 0 {
        return Err("release package project not found".into());
    }
    Ok(json!({ "ok": true }))
}

pub(crate) fn load_project(
    conn: &Connection,
    id: i64,
) -> Result<ReleasePackageProjectConfig, String> {
    conn.query_row(
        "SELECT id, name, output_root, frontend_project_path, frontend_build_command, frontend_artifact_path,
                frontend_artifact_mode, backend_project_path, backend_build_command,
                backend_artifact_path, package_type, ssh_host, ssh_port, ssh_username, ssh_auth_type,
                vault_entry_id, ssh_private_key_path, frontend_remote_dir, backend_remote_path,
                frontend_success_keyword, backend_success_keyword, frontend_post_upload_command,
                backend_post_upload_command, created_at, updated_at
         FROM release_package_projects
         WHERE id=?1",
        [id],
        project_from_row,
    )
    .optional()
    .map_err(|e| format!("load release package project failed: {e}"))?
    .ok_or_else(|| "release package project not found".into())
}

fn validate_run_inputs(
    project: &ReleasePackageProjectConfig,
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
    project: &ReleasePackageProjectConfig,
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
    project_id: i64,
    today: NaiveDate,
) -> Result<Value, String> {
    let project = load_project(conn, project_id)?;
    if project.package_type == ReleasePackageType::ServerUpload {
        return Ok(json!({ "packageType": "server_upload" }));
    }
    if project.output_root.trim().is_empty() {
        return Err("请先为当前项目配置归档根目录".into());
    }
    let output_root = project.output_root.clone();
    let folder_name = default_folder_name(today, &project.name);
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

fn target_check_with_conn(
    conn: &Connection,
    project_id: i64,
    folder_name: &str,
) -> Result<Value, String> {
    let project = load_project(conn, project_id)?;
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

fn validate_upload_project(project: &ReleasePackageProjectConfig) -> Result<(), String> {
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
    project: &ReleasePackageProjectConfig,
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

fn remote_probe_with_conn(conn: &Connection, project_id: i64) -> Result<Value, String> {
    let project = load_project(conn, project_id)?;
    require_package_type(&project, ReleasePackageType::ServerUpload, "remote_probe")?;
    validate_upload_project(&project)?;
    let upload = upload_endpoint_with_conn(conn, &project)?;
    let snapshot = probe_host(&upload.endpoint)?;
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
    project_id: i64,
    probe_token: &str,
    replace_existing: bool,
) -> Result<Value, String> {
    let project = load_project(conn, project_id)?;
    require_package_type(&project, ReleasePackageType::ServerUpload, "host_trust")?;
    let snapshot = consume_probe(probe_token)?;
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
    project: &ReleasePackageProjectConfig,
    upload: &UploadEndpoint,
    targets: &[ReleaseTarget],
) -> PreflightBinding {
    PreflightBinding {
        project_id: project.id,
        endpoint: upload.endpoint.clone(),
        auth_type: project.ssh_auth_type.clone(),
        vault_entry_id: upload.vault_entry_id,
        private_key_path: project.ssh_private_key_path.clone(),
        targets: remote_targets(targets),
        frontend_remote_dir: project.frontend_remote_dir.clone(),
        backend_remote_path: project.backend_remote_path.clone(),
    }
}

fn remote_preflight_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let project_id = payload["projectId"]
        .as_i64()
        .ok_or("projectId is required")?;
    let project = load_project(conn, project_id)?;
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
    let project_id = payload["projectId"]
        .as_i64()
        .ok_or("projectId is required")?;
    let retry_token = payload["retryToken"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or("retryToken is required")?;
    let project = load_project(conn, project_id)?;
    require_package_type(
        &project,
        ReleasePackageType::ServerUpload,
        "command_retry_prepare",
    )?;
    let prepared = super::release_package_runtime::prepare_command_retry(retry_token, project_id)?;
    let snapshot = probe_host(&prepared.binding.endpoint)?;
    let mut result = probe_result_with_conn(conn, snapshot)?;
    result["targets"] = json!(prepared.targets);
    result["authType"] = json!(prepared.binding.auth_type);
    Ok(result)
}

fn command_retry_preflight_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let project_id = payload["projectId"]
        .as_i64()
        .ok_or("projectId is required")?;
    let retry_token = payload["retryToken"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or("retryToken is required")?;
    let probe_token = payload["probeToken"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or("probeToken is required")?;
    let project = load_project(conn, project_id)?;
    require_package_type(
        &project,
        ReleasePackageType::ServerUpload,
        "command_retry_preflight",
    )?;
    let prepared = super::release_package_runtime::prepare_command_retry(retry_token, project_id)?;
    let probe = load_probe(probe_token)?;
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
        project_id,
        endpoint: prepared.binding.endpoint,
        auth_type: prepared.binding.auth_type,
        vault_entry_id: prepared.binding.vault_entry_id,
        private_key_path: prepared.binding.private_key_path,
        targets: remote_targets(&prepared.targets),
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
            let id = payload["projectId"]
                .as_i64()
                .ok_or("projectId is required")?;
            prepare_with_conn(&conn, id, Local::now().date_naive())
        }
        "target_check" => {
            let id = payload["projectId"]
                .as_i64()
                .ok_or("projectId is required")?;
            let folder_name = payload["folderName"]
                .as_str()
                .ok_or("folderName is required")?;
            target_check_with_conn(&conn, id, folder_name)
        }
        "remote_probe" => {
            let id = payload["projectId"]
                .as_i64()
                .ok_or("projectId is required")?;
            remote_probe_with_conn(&conn, id)
        }
        "host_trust" => {
            let project_id = payload["projectId"]
                .as_i64()
                .ok_or("projectId is required")?;
            let probe_token = payload["probeToken"]
                .as_str()
                .ok_or("probeToken is required")?;
            let replace_existing = match payload.get("replaceExisting") {
                None => false,
                Some(Value::Bool(value)) => *value,
                Some(_) => return Err("replaceExisting must be a boolean".into()),
            };
            host_trust_with_conn(&conn, project_id, probe_token, replace_existing)
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
            let project_id = payload["projectId"]
                .as_i64()
                .ok_or("projectId is required")?;
            let action_dispatch_id = parse_action_dispatch_id(payload)?;
            let conn = db_conn()?;
            let project = load_project(&conn, project_id)?;
            let targets = parse_targets(payload.get("targets").unwrap_or(&Value::Null))?;
            match parse_start_input(project.package_type, payload)? {
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
                    validate_project_directories(&project, &targets)?;
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
            let project_id = payload["projectId"]
                .as_i64()
                .ok_or("projectId is required")?;
            let conn = db_conn()?;
            let project = load_project(&conn, project_id)?;
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
            let targets = super::release_package_runtime::retry_targets(retry_token, project_id)?;
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
    use std::fs;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(RELEASE_PACKAGE_SCHEMA_SQL).unwrap();
        conn
    }

    fn payload() -> Value {
        json!({
            "name": "客户门户",
            "outputRoot": r"D:\releases",
            "packageType": "server_upload",
            "frontendProjectPath": r"D:\work\web",
            "frontendBuildCommand": "pnpm build",
            "frontendSuccessKeyword": "  Build completed  ",
            "frontendPostUploadCommand": "\n  cd /srv/web\n  ./reload.sh\n",
            "frontendArtifactPath": "dist",
            "frontendArtifactMode": "copy_directory",
            "backendProjectPath": r"D:\work\server",
            "backendBuildCommand": "mvn clean package -Pprod",
            "backendSuccessKeyword": "  BUILD SUCCESS  ",
            "backendPostUploadCommand": "\n  systemctl restart portal\n",
            "backendArtifactPath": r"target\portal.jar",
            "sshHost": "deploy.example.internal",
            "sshPort": 2222,
            "sshUsername": "deploy",
            "sshAuthType": "private_key",
            "sshPrivateKeyPath": r"C:\Users\tester\.ssh\lazycat",
            "frontendRemoteDir": "/srv/portal/web",
            "backendRemotePath": "/srv/portal/app.jar"
        })
    }

    #[test]
    fn schema_migrates_vault_entry_id_and_project_round_trips_it() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE release_package_projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL,
                output_root TEXT NOT NULL, frontend_project_path TEXT NOT NULL,
                frontend_build_command TEXT NOT NULL, frontend_artifact_path TEXT NOT NULL,
                frontend_artifact_mode TEXT NOT NULL, backend_project_path TEXT NOT NULL,
                backend_build_command TEXT NOT NULL, backend_artifact_path TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .unwrap();

        ensure_schema(&conn).unwrap();

        let columns = conn
            .prepare("PRAGMA table_info(release_package_projects)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(columns.contains(&"vault_entry_id".to_string()));
        for column in [
            "frontend_success_keyword",
            "backend_success_keyword",
            "frontend_post_upload_command",
            "backend_post_upload_command",
        ] {
            assert!(columns.contains(&column.to_string()));
        }
    }

    #[test]
    fn password_project_requires_vault_entry_but_private_key_keeps_host_and_username() {
        let mut password = payload();
        password["sshAuthType"] = json!("password");
        password["vaultEntryId"] = Value::Null;
        password["sshHost"] = json!("");
        password["sshUsername"] = json!("");
        assert_eq!(
            parse_project_payload(&password).err().unwrap(),
            "vaultEntryId is required for password authentication"
        );

        let private_key = parse_project_payload(&payload()).unwrap();
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
        let mut password = payload();
        password["sshAuthType"] = json!("password");
        password["vaultEntryId"] = json!(17);
        password["sshPort"] = json!(0);

        let parsed_password = parse_project_payload(&password).unwrap();
        assert_eq!(parsed_password.ssh_port, 22);
        let password_id = project_create_with_conn(&conn, &password).unwrap()["id"]
            .as_i64()
            .unwrap();
        let mut password_project = load_project(&conn, password_id).unwrap();
        password_project.ssh_port = 0;
        assert!(validate_upload_project(&password_project).is_ok());

        let mut invalid_private_key = payload();
        invalid_private_key["sshPort"] = json!(0);
        assert_eq!(
            parse_project_payload(&invalid_private_key).err().unwrap(),
            "sshPort must be between 1 and 65535"
        );

        let private_key_id = project_create_with_conn(&conn, &payload()).unwrap()["id"]
            .as_i64()
            .unwrap();
        let mut private_key_project = load_project(&conn, private_key_id).unwrap();
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
        let mut input = payload();
        input["sshAuthType"] = json!("password");
        input["vaultEntryId"] = json!(17);
        input["sshHost"] = json!("");
        input["sshUsername"] = json!("");

        let id = project_create_with_conn(&conn, &input).unwrap()["id"]
            .as_i64()
            .unwrap();
        let saved = load_project(&conn, id).unwrap();
        assert_eq!(saved.vault_entry_id, Some(17));
        let listed = project_list_with_conn(&conn).unwrap();
        assert_eq!(listed["projects"][0]["vaultEntryId"], 17);
        assert!(listed["projects"][0].get("password").is_none());
        assert!(listed["projects"][0].get("privateKeyPassphrase").is_none());
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
        let mut input = payload();
        input["sshAuthType"] = json!("password");
        input["vaultEntryId"] = json!(11);
        input["sshHost"] = json!("");
        input["sshUsername"] = json!("");
        let project_id = project_create_with_conn(&conn, &input).unwrap()["id"]
            .as_i64()
            .unwrap();
        super::super::vault::install_test_session([7u8; 32]);

        let endpoint =
            upload_endpoint_with_conn(&conn, &load_project(&conn, project_id).unwrap()).unwrap();
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
        let project_id = project_create_with_conn(&conn, &payload()).unwrap()["id"]
            .as_i64()
            .unwrap();

        let endpoint =
            upload_endpoint_with_conn(&conn, &load_project(&conn, project_id).unwrap()).unwrap();

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
        let projects = project_list_with_conn(&conn).unwrap();
        let project = &projects["projects"][0];
        assert_eq!(project["packageType"], "local_archive");
        assert_eq!(project["sshPort"], 22);
        assert_eq!(project["sshAuthType"], "password");
        assert_eq!(project["frontendSuccessKeyword"], "");
        assert_eq!(project["backendSuccessKeyword"], "");
        assert_eq!(project["frontendPostUploadCommand"], "");
        assert_eq!(project["backendPostUploadCommand"], "");
        assert!(project.get("password").is_none());
        assert!(project.get("privateKeyPassphrase").is_none());
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
        let local = load_project(&conn, 1).unwrap();
        let upload = load_project(&conn, 2).unwrap();
        assert_eq!(local.package_type, ReleasePackageType::LocalArchive);
        assert_eq!(upload.package_type, ReleasePackageType::ServerUpload);

        conn.execute(
            "UPDATE release_package_projects SET package_type='local_archive' WHERE id=2",
            [],
        )
        .unwrap();
        ensure_schema(&conn).unwrap();
        assert_eq!(
            load_project(&conn, 2).unwrap().package_type,
            ReleasePackageType::LocalArchive
        );
    }

    #[test]
    fn project_validation_depends_on_package_type() {
        let mut upload = payload();
        upload["packageType"] = json!("server_upload");
        upload["outputRoot"] = json!("");
        assert!(parse_project_payload(&upload).is_ok());

        upload["sshHost"] = json!("");
        assert_eq!(
            parse_project_payload(&upload).err().unwrap(),
            "sshHost is required for private_key authentication"
        );

        let mut local = payload();
        local["packageType"] = json!("local_archive");
        local["sshHost"] = json!("");
        local["sshUsername"] = json!("");
        local["frontendRemoteDir"] = json!("");
        local["backendRemotePath"] = json!("");
        assert!(parse_project_payload(&local).is_ok());

        local["outputRoot"] = json!("");
        assert_eq!(
            parse_project_payload(&local).err().unwrap(),
            "outputRoot is required for local_archive"
        );
    }
    #[test]
    fn trusted_host_requires_explicit_replacement_when_fingerprint_changes() {
        let conn = test_conn();
        let probe = ProbeSnapshot {
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
        let project_id = project_create_with_conn(&conn, &payload()).unwrap()["id"]
            .as_i64()
            .unwrap();
        let snapshot = ProbeSnapshot {
            endpoint: RemoteEndpoint {
                host: "server.example.internal".into(),
                port: 22,
                username: "deploy".into(),
            },
            key_type: "ed25519".into(),
            fingerprint_sha256: "SHA256:new".into(),
        };
        let old_token = store_probe(snapshot.clone()).unwrap();
        let result = host_trust_with_conn(&conn, project_id, &old_token, false).unwrap();
        let next_token = result["probeToken"].as_str().unwrap();
        assert_ne!(next_token, old_token);
        assert!(host_trust_with_conn(&conn, project_id, &old_token, false).is_err());
        assert_eq!(consume_probe(next_token).unwrap(), snapshot);
    }

    #[test]
    fn local_archive_cannot_trust_a_host_and_does_not_consume_the_probe_token() {
        let conn = test_conn();
        let mut local = payload();
        local["packageType"] = json!("local_archive");
        let project_id = project_create_with_conn(&conn, &local).unwrap()["id"]
            .as_i64()
            .unwrap();
        let snapshot = ProbeSnapshot {
            endpoint: RemoteEndpoint {
                host: "server.example.internal".into(),
                port: 22,
                username: "deploy".into(),
            },
            key_type: "ed25519".into(),
            fingerprint_sha256: "SHA256:local".into(),
        };
        let probe_token = store_probe(snapshot.clone()).unwrap();

        assert!(host_trust_with_conn(&conn, project_id, &probe_token, false)
            .unwrap_err()
            .contains("server_upload"));
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
        let created = project_create_with_conn(&conn, &payload()).unwrap();
        let id = created["id"].as_i64().unwrap();
        let listed = project_list_with_conn(&conn).unwrap();
        assert_eq!(listed["projects"][0]["name"], "客户门户");
        assert_eq!(listed["projects"][0]["sshHost"], "deploy.example.internal");
        assert_eq!(listed["projects"][0]["sshPort"], 2222);
        assert_eq!(
            listed["projects"][0]["frontendSuccessKeyword"],
            "Build completed"
        );
        assert_eq!(
            listed["projects"][0]["backendSuccessKeyword"],
            "BUILD SUCCESS"
        );
        assert_eq!(
            listed["projects"][0]["frontendPostUploadCommand"],
            "cd /srv/web\n  ./reload.sh"
        );
        assert_eq!(
            listed["projects"][0]["backendPostUploadCommand"],
            "systemctl restart portal"
        );
        assert!(listed["projects"][0].get("password").is_none());
        assert!(listed["projects"][0].get("privateKeyPassphrase").is_none());
        let mut update = payload();
        update["id"] = json!(id);
        update["name"] = json!("客户门户 Pro");
        update["backendPostUploadCommand"] = json!("systemctl restart portal-pro");
        project_update_with_conn(&conn, &update).unwrap();
        let updated = load_project(&conn, id).unwrap();
        assert_eq!(updated.name, "客户门户 Pro");
        assert_eq!(
            updated.backend_post_upload_command,
            "systemctl restart portal-pro"
        );
        project_delete_with_conn(&conn, &json!({ "id": id })).unwrap();
        assert!(load_project(&conn, id).is_err());
    }

    #[test]
    fn prepare_uses_project_output_root_and_inclusive_thursday() {
        let conn = test_conn();
        let mut project = payload();
        project["packageType"] = json!("local_archive");
        let id = project_create_with_conn(&conn, &project).unwrap()["id"]
            .as_i64()
            .unwrap();
        let out =
            prepare_with_conn(&conn, id, NaiveDate::from_ymd_opt(2026, 7, 23).unwrap()).unwrap();
        assert_eq!(out["packageType"], "local_archive");
        assert_eq!(out["defaultFolderName"], "20260723-客户门户");
        assert_eq!(out["archivePath"], r"D:\releases\20260723-客户门户");
    }

    #[test]
    fn prepare_returns_a_discriminated_result_without_archive_for_upload() {
        let conn = test_conn();
        let mut project = payload();
        project["outputRoot"] = json!("");
        let id = project_create_with_conn(&conn, &project).unwrap()["id"]
            .as_i64()
            .unwrap();

        let out =
            prepare_with_conn(&conn, id, NaiveDate::from_ymd_opt(2026, 7, 23).unwrap()).unwrap();

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
        let upload_id = project_create_with_conn(&conn, &payload()).unwrap()["id"]
            .as_i64()
            .unwrap();
        assert!(target_check_with_conn(&conn, upload_id, "release")
            .unwrap_err()
            .contains("local_archive"));

        let mut local = payload();
        local["packageType"] = json!("local_archive");
        local["sshHost"] = json!("");
        let local_id = project_create_with_conn(&conn, &local).unwrap()["id"]
            .as_i64()
            .unwrap();
        assert!(remote_probe_with_conn(&conn, local_id)
            .unwrap_err()
            .contains("server_upload"));
        assert!(remote_preflight_with_conn(
            &conn,
            &json!({
                "projectId": local_id,
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
        let id = project_create_with_conn(&conn, &payload()).unwrap()["id"]
            .as_i64()
            .unwrap();
        let project = load_project(&conn, id).unwrap();

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
        let project = ReleasePackageProjectConfig {
            id: 1,
            name: "test".into(),
            output_root: output.to_string_lossy().into_owned(),
            package_type: ReleasePackageType::LocalArchive,
            frontend_project_path: root.join("missing-frontend").to_string_lossy().into_owned(),
            frontend_build_command: "exit 0".into(),
            frontend_success_keyword: String::new(),
            frontend_post_upload_command: String::new(),
            frontend_artifact_path: "dist".into(),
            frontend_artifact_mode: "copy_directory".into(),
            backend_project_path: backend.to_string_lossy().into_owned(),
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
        let mut project = payload();
        project["packageType"] = json!("local_archive");
        project["outputRoot"] = json!(output.to_string_lossy());
        let id = project_create_with_conn(&conn, &project).unwrap()["id"]
            .as_i64()
            .unwrap();

        let missing = target_check_with_conn(&conn, id, "release").unwrap();
        assert_eq!(missing["exists"], false);
        assert_eq!(
            missing["archivePath"],
            output.join("release").to_string_lossy().as_ref()
        );

        fs::create_dir(output.join("release")).unwrap();
        let existing = target_check_with_conn(&conn, id, "release").unwrap();
        assert_eq!(existing["exists"], true);

        fs::remove_dir(output.join("release")).unwrap();
        fs::write(output.join("release"), "file").unwrap();
        assert!(target_check_with_conn(&conn, id, "release")
            .unwrap_err()
            .contains("不是文件夹"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn remote_discard_action_revokes_both_token_types_idempotently() {
        assert!(supported_actions().contains(&"remote_discard"));
        let snapshot = ProbeSnapshot {
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
            project_id: 7,
            endpoint: RemoteEndpoint {
                host: "server.example.internal".into(),
                port: 22,
                username: "deploy".into(),
            },
            auth_type: "private_key".into(),
            vault_entry_id: None,
            private_key_path: r"C:\Users\tester\.ssh\lazycat".into(),
            targets: vec![RemoteTarget::Backend],
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
        let project = ReleasePackageProjectConfig {
            id: 1,
            name: "test".into(),
            output_root: output.to_string_lossy().into_owned(),
            package_type: ReleasePackageType::LocalArchive,
            frontend_project_path: root.join("missing-frontend").to_string_lossy().into_owned(),
            frontend_build_command: "exit 0".into(),
            frontend_success_keyword: String::new(),
            frontend_post_upload_command: String::new(),
            frontend_artifact_path: "dist".into(),
            frontend_artifact_mode: "copy_directory".into(),
            backend_project_path: backend.to_string_lossy().into_owned(),
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
            created_at: String::new(),
            updated_at: String::new(),
        };

        let result = validate_run_inputs(&project, "release", &[ReleaseTarget::Backend], false);
        let _ = fs::remove_dir_all(&root);

        assert!(result.is_ok());
    }
}

use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;

use super::helpers::db_conn;
use super::release_package_archive::{default_folder_name, validate_folder_name};
use super::release_package_remote::{
    classify_trust, consume_probe, issue_preflight, load_probe, probe_host, run_remote_preflight,
    store_probe, validate_remote_dir, validate_remote_file, AuthSecret, HostTrust,
    PreflightBinding, ProbeSnapshot, RemoteEndpoint, RemoteTarget,
};
use zeroize::Zeroizing;

pub const LEGACY_OUTPUT_ROOT_KEY: &str = "release_package.output_root";
pub const RELEASE_PACKAGE_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS release_package_projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    output_root TEXT NOT NULL,
    frontend_project_path TEXT NOT NULL,
    frontend_build_command TEXT NOT NULL,
    frontend_artifact_path TEXT NOT NULL,
    frontend_artifact_mode TEXT NOT NULL CHECK (frontend_artifact_mode IN ('copy_directory', 'zip_directory')),
    backend_project_path TEXT NOT NULL,
    backend_build_command TEXT NOT NULL,
    backend_artifact_path TEXT NOT NULL,
    upload_enabled INTEGER NOT NULL DEFAULT 0,
    ssh_host TEXT NOT NULL DEFAULT '',
    ssh_port INTEGER NOT NULL DEFAULT 22,
    ssh_username TEXT NOT NULL DEFAULT '',
    ssh_auth_type TEXT NOT NULL DEFAULT 'password' CHECK (ssh_auth_type IN ('password', 'private_key')),
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
        ("ssh_private_key_path", "ALTER TABLE release_package_projects ADD COLUMN ssh_private_key_path TEXT NOT NULL DEFAULT ''"),
        ("frontend_remote_dir", "ALTER TABLE release_package_projects ADD COLUMN frontend_remote_dir TEXT NOT NULL DEFAULT ''"),
        ("backend_remote_path", "ALTER TABLE release_package_projects ADD COLUMN backend_remote_path TEXT NOT NULL DEFAULT ''"),
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
    "start",
    "cancel",
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackageProjectConfig {
    pub id: i64,
    pub name: String,
    pub output_root: String,
    pub frontend_project_path: String,
    pub frontend_build_command: String,
    pub frontend_artifact_path: String,
    pub frontend_artifact_mode: String,
    pub backend_project_path: String,
    pub backend_build_command: String,
    pub backend_artifact_path: String,
    pub upload_enabled: bool,
    pub ssh_host: String,
    pub ssh_port: u16,
    pub ssh_username: String,
    pub ssh_auth_type: String,
    pub ssh_private_key_path: String,
    pub frontend_remote_dir: String,
    pub backend_remote_path: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareResult {
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
    frontend_project_path: String,
    frontend_build_command: String,
    frontend_artifact_path: String,
    frontend_artifact_mode: String,
    backend_project_path: String,
    backend_build_command: String,
    backend_artifact_path: String,
    upload_enabled: bool,
    ssh_host: String,
    ssh_port: u16,
    ssh_username: String,
    ssh_auth_type: String,
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

fn optional_bool(payload: &Value, key: &str, default: bool) -> Result<bool, String> {
    match payload.get(key) {
        None => Ok(default),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(format!("{key} must be a boolean")),
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

fn parse_project_payload(payload: &Value) -> Result<ProjectPayload, String> {
    let mut project = ProjectPayload {
        name: required_string(payload, "name")?,
        output_root: required_string(payload, "outputRoot")?,
        frontend_project_path: required_string(payload, "frontendProjectPath")?,
        frontend_build_command: required_string(payload, "frontendBuildCommand")?,
        frontend_artifact_path: required_string(payload, "frontendArtifactPath")?,
        frontend_artifact_mode: required_string(payload, "frontendArtifactMode")?,
        backend_project_path: required_string(payload, "backendProjectPath")?,
        backend_build_command: required_string(payload, "backendBuildCommand")?,
        backend_artifact_path: required_string(payload, "backendArtifactPath")?,
        upload_enabled: optional_bool(payload, "uploadEnabled", false)?,
        ssh_host: optional_string(payload, "sshHost")?,
        ssh_port: optional_port(payload, "sshPort", 22)?,
        ssh_username: optional_string(payload, "sshUsername")?,
        ssh_auth_type: optional_string(payload, "sshAuthType")?,
        ssh_private_key_path: optional_string(payload, "sshPrivateKeyPath")?,
        frontend_remote_dir: optional_string(payload, "frontendRemoteDir")?,
        backend_remote_path: optional_string(payload, "backendRemotePath")?,
    };
    validate_folder_name(&project.name)?;
    if project.ssh_auth_type.is_empty() {
        project.ssh_auth_type = "password".into();
    }
    if !matches!(project.ssh_auth_type.as_str(), "password" | "private_key") {
        return Err("sshAuthType must be password or private_key".into());
    }
    if project.upload_enabled {
        if project.ssh_host.is_empty() {
            return Err("sshHost is required when upload is enabled".into());
        }
        if project.ssh_username.is_empty() {
            return Err("sshUsername is required when upload is enabled".into());
        }
        if project.ssh_auth_type == "private_key" && project.ssh_private_key_path.is_empty() {
            return Err("sshPrivateKeyPath is required for private_key authentication".into());
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

fn project_from_row(row: &Row<'_>) -> rusqlite::Result<ReleasePackageProjectConfig> {
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
        upload_enabled: row.get(10)?,
        ssh_host: row.get(11)?,
        ssh_port: row.get(12)?,
        ssh_username: row.get(13)?,
        ssh_auth_type: row.get(14)?,
        ssh_private_key_path: row.get(15)?,
        frontend_remote_dir: row.get(16)?,
        backend_remote_path: row.get(17)?,
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
    })
}

fn project_list_with_conn(conn: &Connection) -> Result<Value, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, output_root, frontend_project_path, frontend_build_command, frontend_artifact_path,
                    frontend_artifact_mode, backend_project_path, backend_build_command,
                    backend_artifact_path, upload_enabled, ssh_host, ssh_port, ssh_username, ssh_auth_type,
                    ssh_private_key_path, frontend_remote_dir, backend_remote_path, created_at, updated_at
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

fn project_create_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let project = parse_project_payload(payload)?;
    conn.execute(
        "INSERT INTO release_package_projects(
            name, output_root, frontend_project_path, frontend_build_command, frontend_artifact_path,
            frontend_artifact_mode, backend_project_path, backend_build_command, backend_artifact_path,
            upload_enabled, ssh_host, ssh_port, ssh_username, ssh_auth_type,
            ssh_private_key_path, frontend_remote_dir, backend_remote_path
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
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
            project.upload_enabled,
            project.ssh_host,
            project.ssh_port,
            project.ssh_username,
            project.ssh_auth_type,
            project.ssh_private_key_path,
            project.frontend_remote_dir,
            project.backend_remote_path,
        ],
    )
    .map_err(|e| format!("create release package project failed: {e}"))?;
    Ok(json!({ "id": conn.last_insert_rowid() }))
}

fn project_update_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let project = parse_project_payload(payload)?;
    let changed = conn
        .execute(
            "UPDATE release_package_projects SET
                name=?1, output_root=?2, frontend_project_path=?3, frontend_build_command=?4,
                frontend_artifact_path=?5, frontend_artifact_mode=?6,
                backend_project_path=?7, backend_build_command=?8, backend_artifact_path=?9,
                upload_enabled=?10, ssh_host=?11, ssh_port=?12, ssh_username=?13,
                ssh_auth_type=?14, ssh_private_key_path=?15, frontend_remote_dir=?16,
                backend_remote_path=?17, updated_at=CURRENT_TIMESTAMP
             WHERE id=?18",
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
                project.upload_enabled,
                project.ssh_host,
                project.ssh_port,
                project.ssh_username,
                project.ssh_auth_type,
                project.ssh_private_key_path,
                project.frontend_remote_dir,
                project.backend_remote_path,
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
                backend_artifact_path, upload_enabled, ssh_host, ssh_port, ssh_username, ssh_auth_type,
                    ssh_private_key_path, frontend_remote_dir, backend_remote_path, created_at, updated_at
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

fn prepare_with_conn(
    conn: &Connection,
    project_id: i64,
    today: NaiveDate,
) -> Result<Value, String> {
    let project = load_project(conn, project_id)?;
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
    validate_folder_name(folder_name)?;
    let project = load_project(conn, project_id)?;
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

fn remote_probe_with_conn(conn: &Connection, project_id: i64) -> Result<Value, String> {
    let project = load_project(conn, project_id)?;
    if !project.upload_enabled {
        return Err("当前项目未启用服务器上传".into());
    }
    validate_remote_dir(&project.frontend_remote_dir)?;
    validate_remote_file(&project.backend_remote_path)?;
    let endpoint = RemoteEndpoint {
        host: project.ssh_host.trim().to_ascii_lowercase(),
        port: project.ssh_port,
        username: project.ssh_username,
    };
    if endpoint.host.is_empty() || endpoint.username.trim().is_empty() {
        return Err("SSH 服务器地址和用户名不能为空".into());
    }
    let snapshot = probe_host(&endpoint)?;
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
    probe_token: &str,
    replace_existing: bool,
) -> Result<Value, String> {
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

fn parse_auth_secret(
    project: &ReleasePackageProjectConfig,
    payload: &Value,
) -> Result<AuthSecret, String> {
    match project.ssh_auth_type.as_str() {
        "password" => {
            if payload.get("privateKeyPassphrase").is_some() {
                return Err("密码认证不能提交私钥口令".into());
            }
            let password = payload
                .get("password")
                .and_then(Value::as_str)
                .ok_or("password is required for password authentication")?;
            Ok(AuthSecret::Password(Zeroizing::new(password.to_string())))
        }
        "private_key" => {
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
        _ => Err("不支持的 SSH 认证方式".into()),
    }
}

fn preflight_binding(
    project: &ReleasePackageProjectConfig,
    targets: &[ReleaseTarget],
) -> PreflightBinding {
    PreflightBinding {
        project_id: project.id,
        endpoint: RemoteEndpoint {
            host: project.ssh_host.trim().to_ascii_lowercase(),
            port: project.ssh_port,
            username: project.ssh_username.clone(),
        },
        auth_type: project.ssh_auth_type.clone(),
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
    let targets = parse_targets(payload.get("targets").unwrap_or(&Value::Null))?;
    let probe_token = payload["probeToken"]
        .as_str()
        .ok_or("probeToken is required")?;
    let project = load_project(conn, project_id)?;
    if !project.upload_enabled {
        return Err("当前项目未启用服务器上传".into());
    }
    let binding = preflight_binding(&project, &targets);
    let probe = load_probe(probe_token)?;
    if probe.endpoint != binding.endpoint {
        return Err("SSH 探测令牌与当前项目服务器配置不匹配".into());
    }
    let known = known_host_with_conn(conn, &binding.endpoint)?
        .ok_or_else(|| "请先确认并信任 SSH 主机指纹".to_string())?;
    if known.0 != probe.key_type || known.1 != probe.fingerprint_sha256 {
        return Err("SSH 主机指纹未受信任或已变化".into());
    }
    let secret = parse_auth_secret(&project, payload)?;
    let checks = run_remote_preflight(&binding, &known.1, &secret)?;
    let issued = issue_preflight(binding, secret, &checks)?;
    Ok(json!({
        "preflightToken": issued.token,
        "expiresAt": issued.expires_at.to_rfc3339(),
        "targets": checks,
    }))
}
#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported release_package action: {action}"));
    }
    if matches!(action, "start" | "cancel") {
        return Err("release_package action requires app context".into());
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
            let probe_token = payload["probeToken"]
                .as_str()
                .ok_or("probeToken is required")?;
            let replace_existing = match payload.get("replaceExisting") {
                None => false,
                Some(Value::Bool(value)) => *value,
                Some(_) => return Err("replaceExisting must be a boolean".into()),
            };
            host_trust_with_conn(&conn, probe_token, replace_existing)
        }
        "remote_preflight" => remote_preflight_with_conn(&conn, payload),
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
            let folder_name = payload["folderName"]
                .as_str()
                .ok_or("folderName is required")?
                .to_string();
            validate_folder_name(&folder_name)?;
            let conn = db_conn()?;
            let project = load_project(&conn, project_id)?;
            let output_root = project.output_root.clone();
            let targets = parse_targets(payload.get("targets").unwrap_or(&Value::Null))?;
            let overwrite_existing = parse_overwrite_existing(payload)?;
            validate_run_inputs(&project, &folder_name, &targets, overwrite_existing)?;
            super::release_package_runtime::start(
                app,
                project,
                output_root.into(),
                folder_name,
                targets,
                overwrite_existing,
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
            "frontendProjectPath": r"D:\work\web",
            "frontendBuildCommand": "pnpm build",
            "frontendArtifactPath": "dist",
            "frontendArtifactMode": "copy_directory",
            "backendProjectPath": r"D:\work\server",
            "backendBuildCommand": "mvn clean package -Pprod",
            "backendArtifactPath": r"target\portal.jar",
            "uploadEnabled": true,
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
        let projects = project_list_with_conn(&conn).unwrap();
        let project = &projects["projects"][0];
        assert_eq!(project["uploadEnabled"], false);
        assert_eq!(project["sshPort"], 22);
        assert_eq!(project["sshAuthType"], "password");
        assert!(project.get("password").is_none());
        assert!(project.get("privateKeyPassphrase").is_none());
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
        let result = host_trust_with_conn(&conn, &old_token, false).unwrap();
        let next_token = result["probeToken"].as_str().unwrap();
        assert_ne!(next_token, old_token);
        assert!(host_trust_with_conn(&conn, &old_token, false).is_err());
        assert_eq!(consume_probe(next_token).unwrap(), snapshot);
    }
    #[test]
    fn authentication_payload_must_match_the_configured_mode() {
        let conn = test_conn();
        let id = project_create_with_conn(&conn, &payload()).unwrap()["id"]
            .as_i64()
            .unwrap();
        let mut project = load_project(&conn, id).unwrap();

        project.ssh_auth_type = "password".into();
        assert!(matches!(
            parse_auth_secret(&project, &json!({ "password": "secret" })).unwrap(),
            AuthSecret::Password(_)
        ));
        assert!(parse_auth_secret(
            &project,
            &json!({ "password": "secret", "privateKeyPassphrase": "wrong" })
        )
        .is_err());

        project.ssh_auth_type = "private_key".into();
        assert!(matches!(
            parse_auth_secret(&project, &json!({ "privateKeyPassphrase": "secret" })).unwrap(),
            AuthSecret::PrivateKeyPassphrase(Some(_))
        ));
        assert!(parse_auth_secret(&project, &json!({ "password": "wrong" })).is_err());
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
        assert!(listed["projects"][0].get("password").is_none());
        assert!(listed["projects"][0].get("privateKeyPassphrase").is_none());
        let mut update = payload();
        update["id"] = json!(id);
        update["name"] = json!("客户门户 Pro");
        project_update_with_conn(&conn, &update).unwrap();
        assert_eq!(load_project(&conn, id).unwrap().name, "客户门户 Pro");
        project_delete_with_conn(&conn, &json!({ "id": id })).unwrap();
        assert!(load_project(&conn, id).is_err());
    }

    #[test]
    fn prepare_uses_project_output_root_and_inclusive_thursday() {
        let conn = test_conn();
        let id = project_create_with_conn(&conn, &payload()).unwrap()["id"]
            .as_i64()
            .unwrap();
        let out =
            prepare_with_conn(&conn, id, NaiveDate::from_ymd_opt(2026, 7, 23).unwrap()).unwrap();
        assert_eq!(out["defaultFolderName"], "20260723-客户门户");
        assert_eq!(out["archivePath"], r"D:\releases\20260723-客户门户");
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
            frontend_project_path: root.join("missing-frontend").to_string_lossy().into_owned(),
            frontend_build_command: "exit 0".into(),
            frontend_artifact_path: "dist".into(),
            frontend_artifact_mode: "copy_directory".into(),
            backend_project_path: backend.to_string_lossy().into_owned(),
            backend_build_command: "exit 0".into(),
            backend_artifact_path: "app.jar".into(),
            upload_enabled: false,
            ssh_host: String::new(),
            ssh_port: 22,
            ssh_username: String::new(),
            ssh_auth_type: "password".into(),
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
            frontend_project_path: root.join("missing-frontend").to_string_lossy().into_owned(),
            frontend_build_command: "exit 0".into(),
            frontend_artifact_path: "dist".into(),
            frontend_artifact_mode: "copy_directory".into(),
            backend_project_path: backend.to_string_lossy().into_owned(),
            backend_build_command: "exit 0".into(),
            backend_artifact_path: "app.jar".into(),
            upload_enabled: false,
            ssh_host: String::new(),
            ssh_port: 22,
            ssh_username: String::new(),
            ssh_auth_type: "password".into(),
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

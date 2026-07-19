use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;

use super::helpers::db_conn;
use super::release_package_archive::{default_folder_name, validate_folder_name};

pub const OUTPUT_ROOT_KEY: &str = "release_package.output_root";
pub const RELEASE_PACKAGE_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS release_package_projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    frontend_project_path TEXT NOT NULL,
    frontend_build_command TEXT NOT NULL,
    frontend_artifact_path TEXT NOT NULL,
    frontend_artifact_mode TEXT NOT NULL CHECK (frontend_artifact_mode IN ('copy_directory', 'zip_directory')),
    backend_project_path TEXT NOT NULL,
    backend_build_command TEXT NOT NULL,
    backend_artifact_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
"#;

const ACTIONS: &[&str] = &[
    "project_list",
    "project_create",
    "project_update",
    "project_delete",
    "prepare",
    "start",
    "cancel",
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePackageProjectConfig {
    pub id: i64,
    pub name: String,
    pub frontend_project_path: String,
    pub frontend_build_command: String,
    pub frontend_artifact_path: String,
    pub frontend_artifact_mode: String,
    pub backend_project_path: String,
    pub backend_build_command: String,
    pub backend_artifact_path: String,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseTarget {
    Frontend,
    Backend,
}

struct ProjectPayload {
    name: String,
    frontend_project_path: String,
    frontend_build_command: String,
    frontend_artifact_path: String,
    frontend_artifact_mode: String,
    backend_project_path: String,
    backend_build_command: String,
    backend_artifact_path: String,
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

fn parse_project_payload(payload: &Value) -> Result<ProjectPayload, String> {
    let project = ProjectPayload {
        name: required_string(payload, "name")?,
        frontend_project_path: required_string(payload, "frontendProjectPath")?,
        frontend_build_command: required_string(payload, "frontendBuildCommand")?,
        frontend_artifact_path: required_string(payload, "frontendArtifactPath")?,
        frontend_artifact_mode: required_string(payload, "frontendArtifactMode")?,
        backend_project_path: required_string(payload, "backendProjectPath")?,
        backend_build_command: required_string(payload, "backendBuildCommand")?,
        backend_artifact_path: required_string(payload, "backendArtifactPath")?,
    };
    validate_folder_name(&project.name)?;
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
        frontend_project_path: row.get(2)?,
        frontend_build_command: row.get(3)?,
        frontend_artifact_path: row.get(4)?,
        frontend_artifact_mode: row.get(5)?,
        backend_project_path: row.get(6)?,
        backend_build_command: row.get(7)?,
        backend_artifact_path: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn project_list_with_conn(conn: &Connection) -> Result<Value, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, frontend_project_path, frontend_build_command, frontend_artifact_path,
                    frontend_artifact_mode, backend_project_path, backend_build_command,
                    backend_artifact_path, created_at, updated_at
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
            name, frontend_project_path, frontend_build_command, frontend_artifact_path,
            frontend_artifact_mode, backend_project_path, backend_build_command, backend_artifact_path
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            project.name,
            project.frontend_project_path,
            project.frontend_build_command,
            project.frontend_artifact_path,
            project.frontend_artifact_mode,
            project.backend_project_path,
            project.backend_build_command,
            project.backend_artifact_path,
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
                name=?1, frontend_project_path=?2, frontend_build_command=?3,
                frontend_artifact_path=?4, frontend_artifact_mode=?5,
                backend_project_path=?6, backend_build_command=?7, backend_artifact_path=?8,
                updated_at=CURRENT_TIMESTAMP
             WHERE id=?9",
            params![
                project.name,
                project.frontend_project_path,
                project.frontend_build_command,
                project.frontend_artifact_path,
                project.frontend_artifact_mode,
                project.backend_project_path,
                project.backend_build_command,
                project.backend_artifact_path,
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
        "SELECT id, name, frontend_project_path, frontend_build_command, frontend_artifact_path,
                frontend_artifact_mode, backend_project_path, backend_build_command,
                backend_artifact_path, created_at, updated_at
         FROM release_package_projects
         WHERE id=?1",
        [id],
        project_from_row,
    )
    .optional()
    .map_err(|e| format!("load release package project failed: {e}"))?
    .ok_or_else(|| "release package project not found".into())
}

pub(crate) fn load_output_root(conn: &Connection) -> Result<String, String> {
    let output_root = conn
        .query_row(
            "SELECT value FROM user_settings WHERE key=?1",
            [OUTPUT_ROOT_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| format!("load release package output root failed: {e}"))?
        .ok_or("release package output root is required")?;
    if output_root.trim().is_empty() {
        return Err("release package output root is required".into());
    }
    Ok(output_root)
}

fn validate_run_inputs(
    project: &ReleasePackageProjectConfig,
    output_root: &str,
    folder_name: &str,
    targets: &[ReleaseTarget],
) -> Result<(), String> {
    let output_root = PathBuf::from(output_root);
    if !output_root.is_dir() {
        return Err("全局归档根目录不存在或不是文件夹".into());
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
    if output_root.join(folder_name).exists() {
        return Err("目标归档目录已存在".into());
    }
    Ok(())
}

fn prepare_with_conn(
    conn: &Connection,
    project_id: i64,
    today: NaiveDate,
) -> Result<Value, String> {
    let project = load_project(conn, project_id)?;
    let output_root = load_output_root(conn)?;
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
            let output_root = load_output_root(&conn)?;
            let targets = parse_targets(payload.get("targets").unwrap_or(&Value::Null))?;
            validate_run_inputs(&project, &output_root, &folder_name, &targets)?;
            super::release_package_runtime::start(app, project, output_root.into(), folder_name)
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
        conn.execute_batch("CREATE TABLE user_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);").unwrap();
        conn
    }

    fn payload() -> Value {
        json!({
            "name": "客户门户",
            "frontendProjectPath": r"D:\work\web",
            "frontendBuildCommand": "pnpm build",
            "frontendArtifactPath": "dist",
            "frontendArtifactMode": "copy_directory",
            "backendProjectPath": r"D:\work\server",
            "backendBuildCommand": "mvn clean package -Pprod",
            "backendArtifactPath": r"target\portal.jar"
        })
    }

    #[test]
    fn project_crud_round_trip() {
        let conn = test_conn();
        let created = project_create_with_conn(&conn, &payload()).unwrap();
        let id = created["id"].as_i64().unwrap();
        let listed = project_list_with_conn(&conn).unwrap();
        assert_eq!(listed["projects"][0]["name"], "客户门户");
        let mut update = payload();
        update["id"] = json!(id);
        update["name"] = json!("客户门户 Pro");
        project_update_with_conn(&conn, &update).unwrap();
        assert_eq!(load_project(&conn, id).unwrap().name, "客户门户 Pro");
        project_delete_with_conn(&conn, &json!({ "id": id })).unwrap();
        assert!(load_project(&conn, id).is_err());
    }

    #[test]
    fn prepare_uses_global_output_root_and_inclusive_thursday() {
        let conn = test_conn();
        let id = project_create_with_conn(&conn, &payload()).unwrap()["id"]
            .as_i64()
            .unwrap();
        conn.execute(
            "INSERT INTO user_settings(key, value) VALUES (?1, ?2)",
            [OUTPUT_ROOT_KEY, r"D:\releases"],
        )
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
            frontend_project_path: root.join("missing-frontend").to_string_lossy().into_owned(),
            frontend_build_command: "exit 0".into(),
            frontend_artifact_path: "dist".into(),
            frontend_artifact_mode: "copy_directory".into(),
            backend_project_path: backend.to_string_lossy().into_owned(),
            backend_build_command: "exit 0".into(),
            backend_artifact_path: "app.jar".into(),
            created_at: String::new(),
            updated_at: String::new(),
        };

        let result = validate_run_inputs(
            &project,
            output.to_string_lossy().as_ref(),
            "release",
            &[ReleaseTarget::Backend],
        );
        let _ = fs::remove_dir_all(&root);

        assert!(result.is_ok());
    }
}

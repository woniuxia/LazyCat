use super::helpers::db_conn;
#[cfg(not(test))]
use super::helpers::get_data_dir;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(crate) const API_MOCK_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS api_mock_projects (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  host TEXT NOT NULL DEFAULT '127.0.0.1',
  port INTEGER NOT NULL,
  enabled_cors_default INTEGER NOT NULL DEFAULT 1,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_api_mock_projects_sort
  ON api_mock_projects(sort_order, id);

CREATE TABLE IF NOT EXISTS api_mock_files (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  original_name TEXT NOT NULL,
  stored_path TEXT NOT NULL,
  content_type TEXT NOT NULL DEFAULT '',
  size INTEGER NOT NULL DEFAULT 0,
  hash TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_api_mock_files_hash
  ON api_mock_files(hash);

CREATE TABLE IF NOT EXISTS api_mock_routes (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  method TEXT NOT NULL,
  path_pattern TEXT NOT NULL,
  status_code INTEGER NOT NULL DEFAULT 200,
  response_kind TEXT NOT NULL DEFAULT 'static_body',
  content_type TEXT NOT NULL DEFAULT 'application/json; charset=utf-8',
  headers_json TEXT NOT NULL DEFAULT '[]',
  body_text TEXT NOT NULL DEFAULT '',
  file_id INTEGER,
  cors_json TEXT NOT NULL DEFAULT '{}',
  enabled INTEGER NOT NULL DEFAULT 1,
  delay_ms INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0,
  source_request_id INTEGER,
  source_snapshot_json TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY(project_id) REFERENCES api_mock_projects(id) ON DELETE CASCADE,
  FOREIGN KEY(file_id) REFERENCES api_mock_files(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_api_mock_routes_project
  ON api_mock_routes(project_id, enabled, sort_order, id);
"#;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct HeaderRow {
    enabled: bool,
    key: String,
    value: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CorsConfig {
    enabled: bool,
    allow_origin: String,
    allow_methods: Vec<String>,
    allow_headers: String,
    expose_headers: String,
    allow_credentials: bool,
    max_age_seconds: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MockFileSnapshot {
    id: i64,
    original_name: String,
    stored_path: String,
    content_type: String,
    size: i64,
    hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MockRouteSnapshot {
    id: i64,
    name: String,
    method: String,
    path_pattern: String,
    status_code: u16,
    response_kind: String,
    content_type: String,
    headers: Vec<HeaderRow>,
    body_text: String,
    file: Option<MockFileSnapshot>,
    cors: CorsConfig,
    delay_ms: i64,
    sort_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportedMockFile {
    original_name: String,
    stored_path: String,
    content_type: String,
    size: i64,
    hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestLogEntry {
    id: u64,
    time: String,
    method: String,
    path: String,
    status: u16,
    route_id: Option<i64>,
    route_name: Option<String>,
    duration_ms: u128,
    error: Option<String>,
}

struct RunningMockService {
    host: String,
    port: u16,
    started_at: String,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    route_snapshot: Arc<Vec<MockRouteSnapshot>>,
    logs: Arc<Mutex<VecDeque<RequestLogEntry>>>,
    last_error: Arc<Mutex<Option<String>>>,
}

impl RunningMockService {
    fn snapshot_signature(&self) -> String {
        build_route_signature(&self.route_snapshot)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RouteSpecificity {
    Wildcard = 0,
    Param = 1,
    Exact = 2,
}

fn running_services() -> &'static Mutex<HashMap<i64, RunningMockService>> {
    static SERVICES: OnceLock<Mutex<HashMap<i64, RunningMockService>>> = OnceLock::new();
    SERVICES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn default_cors_config() -> CorsConfig {
    CorsConfig {
        enabled: true,
        allow_origin: "*".into(),
        allow_methods: Vec::new(),
        allow_headers: "*".into(),
        expose_headers: String::new(),
        allow_credentials: false,
        max_age_seconds: Some(600),
    }
}

fn validate_method(method: &str) -> Result<(), String> {
    match method {
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS" => Ok(()),
        _ => Err(format!("unsupported method: {method}")),
    }
}

fn parse_required_i64(payload: &Value, key: &str) -> Result<i64, String> {
    payload[key]
        .as_i64()
        .ok_or_else(|| format!("{key} is required"))
}

fn parse_optional_i64(payload: &Value, key: &str) -> Option<i64> {
    payload[key].as_i64()
}

fn parse_string(payload: &Value, key: &str) -> Option<String> {
    payload[key].as_str().map(|value| value.trim().to_string())
}

fn validate_host(host: &str) -> Result<(), String> {
    match host {
        "127.0.0.1" | "0.0.0.0" => Ok(()),
        _ => Err("host must be 127.0.0.1 or 0.0.0.0".into()),
    }
}

fn validate_port(port: i64) -> Result<(), String> {
    if (1..=65535).contains(&port) {
        Ok(())
    } else {
        Err("port must be between 1 and 65535".into())
    }
}

fn validate_status_code(status_code: i64) -> Result<u16, String> {
    if (100..=599).contains(&status_code) {
        Ok(status_code as u16)
    } else {
        Err("statusCode must be between 100 and 599".into())
    }
}

fn validate_response_kind(kind: &str) -> Result<(), String> {
    match kind {
        "static_body" | "file" => Ok(()),
        _ => Err(format!("unsupported response kind: {kind}")),
    }
}

const MAX_MOCK_DELAY_MS: i64 = 60_000;
const MAX_CONCURRENT_MOCK_CONNECTIONS: usize = 64;
const MOCK_DELAY_SLICE_MS: u64 = 100;
const OVER_LIMIT_LOG_ERROR: &str = "并发超限";

fn validate_delay_ms(delay_ms: i64) -> Result<i64, String> {
    if (0..=MAX_MOCK_DELAY_MS).contains(&delay_ms) {
        Ok(delay_ms)
    } else {
        Err(format!("delayMs must be between 0 and {MAX_MOCK_DELAY_MS}"))
    }
}

/// RAII 计数守卫：连接线程无论正常返回还是提前 return，计数都会递减。
struct ConnectionGuard {
    counter: Arc<AtomicUsize>,
    count: usize,
}

impl ConnectionGuard {
    fn acquire(counter: &Arc<AtomicUsize>) -> Self {
        let count = counter.fetch_add(1, Ordering::AcqRel) + 1;
        Self {
            counter: Arc::clone(counter),
            count,
        }
    }

    fn over_limit(&self) -> bool {
        self.count > MAX_CONCURRENT_MOCK_CONNECTIONS
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::AcqRel);
    }
}

/// 分片 sleep，每片检查 stop 信号；返回 false 表示服务正在停止，应放弃响应。
fn sleep_with_stop(total_ms: u64, stop: &AtomicBool) -> bool {
    let mut remaining = total_ms;
    while remaining > 0 {
        if stop.load(Ordering::Acquire) {
            return false;
        }
        let slice = remaining.min(MOCK_DELAY_SLICE_MS);
        thread::sleep(Duration::from_millis(slice));
        remaining -= slice;
    }
    !stop.load(Ordering::Acquire)
}

fn validate_path_pattern(pattern: &str) -> Result<(), String> {
    let value = pattern.trim();
    if !value.starts_with('/') {
        return Err("path pattern must start with /".into());
    }
    if value.len() > 1 && value.ends_with('/') {
        return Err("path pattern must not end with /".into());
    }

    let segments: Vec<&str> = value.split('/').skip(1).collect();
    for (index, segment) in segments.iter().enumerate() {
        if segment.is_empty() {
            return Err("path pattern contains empty segment".into());
        }
        if *segment == "*" {
            if index != segments.len() - 1 {
                return Err("wildcard must be the last complete segment".into());
            }
            continue;
        }
        if segment.contains('*') {
            return Err("wildcard must be a complete segment".into());
        }
        if let Some(name) = segment.strip_prefix(':') {
            if !is_valid_param_name(name) {
                return Err("invalid path parameter name".into());
            }
            continue;
        }
        if segment.contains(':') {
            return Err("path parameter must occupy a complete segment".into());
        }
    }
    Ok(())
}

fn is_valid_param_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn route_specificity(pattern: &str) -> RouteSpecificity {
    if pattern.split('/').any(|segment| segment == "*") {
        return RouteSpecificity::Wildcard;
    }
    if pattern.split('/').any(|segment| segment.starts_with(':')) {
        return RouteSpecificity::Param;
    }
    RouteSpecificity::Exact
}

fn path_matches(pattern: &str, path: &str) -> bool {
    let pattern_segments: Vec<&str> = pattern.trim_matches('/').split('/').collect();
    let path_segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    if pattern == "/" {
        return path == "/";
    }
    for (index, pattern_segment) in pattern_segments.iter().enumerate() {
        if *pattern_segment == "*" {
            return true;
        }
        let Some(path_segment) = path_segments.get(index) else {
            return false;
        };
        if pattern_segment.starts_with(':') {
            continue;
        }
        if pattern_segment != path_segment {
            return false;
        }
    }
    pattern_segments.len() == path_segments.len()
}

fn select_route<'a>(
    routes: &'a [MockRouteSnapshot],
    method: &str,
    path: &str,
) -> Option<&'a MockRouteSnapshot> {
    routes
        .iter()
        .filter(|route| route.method == method && path_matches(&route.path_pattern, path))
        .max_by_key(|route| {
            (
                route_specificity(&route.path_pattern),
                std::cmp::Reverse(route.sort_order),
                std::cmp::Reverse(route.id),
            )
        })
}

fn parse_cors_config(value: &Value) -> Result<CorsConfig, String> {
    let mut config = if value.is_null() || value.as_object().map(|obj| obj.is_empty()).unwrap_or(false) {
        default_cors_config()
    } else {
        serde_json::from_value::<CorsConfig>(value.clone())
            .map_err(|e| format!("invalid cors config: {e}"))?
    };
    config.allow_origin = config.allow_origin.trim().to_string();
    config.allow_headers = config.allow_headers.trim().to_string();
    config.expose_headers = config.expose_headers.trim().to_string();
    for method in &config.allow_methods {
        validate_method(method)?;
    }
    if config.enabled && config.allow_credentials && config.allow_origin == "*" {
        return Err("Allow-Origin cannot be * when credentials are allowed".into());
    }
    if let Some(max_age) = config.max_age_seconds {
        if max_age < 0 {
            return Err("CORS max age must be non-negative".into());
        }
    }
    if config.allow_origin.is_empty() {
        config.allow_origin = "*".into();
    }
    if config.allow_headers.is_empty() {
        config.allow_headers = "*".into();
    }
    Ok(config)
}

fn build_cors_headers(config: &CorsConfig, route_method: &str) -> Vec<HeaderRow> {
    if !config.enabled {
        return Vec::new();
    }
    let allow_methods = if config.allow_methods.is_empty() {
        route_method.to_string()
    } else {
        config.allow_methods.join(", ")
    };
    let mut headers = vec![
        header("Access-Control-Allow-Origin", &config.allow_origin),
        header("Access-Control-Allow-Methods", &allow_methods),
        header("Access-Control-Allow-Headers", &config.allow_headers),
    ];
    if !config.expose_headers.is_empty() {
        headers.push(header("Access-Control-Expose-Headers", &config.expose_headers));
    }
    if config.allow_credentials {
        headers.push(header("Access-Control-Allow-Credentials", "true"));
    }
    if let Some(max_age) = config.max_age_seconds {
        headers.push(header("Access-Control-Max-Age", &max_age.to_string()));
    }
    headers
}

fn header(key: &str, value: &str) -> HeaderRow {
    HeaderRow {
        enabled: true,
        key: key.to_string(),
        value: value.to_string(),
    }
}

fn header_value(headers: &[HeaderRow], key: &str) -> Option<String> {
    headers
        .iter()
        .find(|row| row.key.eq_ignore_ascii_case(key))
        .map(|row| row.value.clone())
}

fn request_header_value(request: &str, key: &str) -> Option<String> {
    request.lines().skip(1).find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case(key) {
            Some(value.trim().to_string())
        } else {
            None
        }
    })
}

fn parse_headers(value: &Value) -> Result<Vec<HeaderRow>, String> {
    if value.is_null() {
        return Ok(Vec::new());
    }
    let rows = serde_json::from_value::<Vec<HeaderRow>>(value.clone())
        .map_err(|e| format!("invalid headers: {e}"))?;
    Ok(rows
        .into_iter()
        .filter(|row| row.enabled)
        .map(|row| HeaderRow {
            enabled: true,
            key: row.key.trim().to_string(),
            value: row.value,
        })
        .filter(|row| !row.key.is_empty())
        .collect())
}

#[cfg(test)]
fn get_api_mock_files_dir() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir()
        .join("lazycat-api-mock-tests")
        .join("api-mock")
        .join("files");
    fs::create_dir_all(&dir).map_err(|e| format!("create api mock files dir failed: {e}"))?;
    dir.canonicalize()
        .map_err(|e| format!("canonicalize api mock files dir failed: {e}"))
}

#[cfg(not(test))]
fn get_api_mock_files_dir() -> Result<PathBuf, String> {
    let dir = get_data_dir()?.join("api-mock").join("files");
    fs::create_dir_all(&dir).map_err(|e| format!("create api mock files dir failed: {e}"))?;
    dir.canonicalize()
        .map_err(|e| format!("canonicalize api mock files dir failed: {e}"))
}

fn import_file_copy(source: &Path, content_type: &str) -> Result<ImportedMockFile, String> {
    let bytes = fs::read(source).map_err(|e| format!("read source file failed: {e}"))?;
    let hash = blake3::hash(&bytes).to_hex().to_string();
    let original_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file")
        .to_string();
    let extension = source.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    let stored_name = if extension.is_empty() {
        hash.clone()
    } else {
        format!("{hash}.{extension}")
    };
    let target = get_api_mock_files_dir()?.join(stored_name);
    if !target.exists() {
        fs::write(&target, &bytes).map_err(|e| format!("write api mock file copy failed: {e}"))?;
    }
    let stored_path = validate_stored_file_path(&target.to_string_lossy())?;
    Ok(ImportedMockFile {
        original_name,
        stored_path: stored_path.to_string_lossy().to_string(),
        content_type: content_type.trim().to_string(),
        size: bytes.len() as i64,
        hash,
    })
}

fn validate_stored_file_path(path: &str) -> Result<PathBuf, String> {
    let files_dir = get_api_mock_files_dir()?
        .canonicalize()
        .map_err(|e| format!("canonicalize api mock files dir failed: {e}"))?;
    let file_path = PathBuf::from(path)
        .canonicalize()
        .map_err(|e| format!("canonicalize api mock file failed: {e}"))?;
    if !file_path.starts_with(&files_dir) {
        return Err("api mock file path is outside controlled directory".into());
    }
    Ok(file_path)
}

fn project_exists(conn: &Connection, id: i64) -> Result<bool, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM api_mock_projects WHERE id=?1",
        [id],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .map_err(|e| format!("check api mock project failed: {e}"))
}

fn file_exists(conn: &Connection, id: i64) -> Result<bool, String> {
    conn.query_row("SELECT COUNT(*) FROM api_mock_files WHERE id=?1", [id], |row| {
        row.get::<_, i64>(0)
    })
    .map(|count| count > 0)
    .map_err(|e| format!("check api mock file failed: {e}"))
}

fn runtime_summary_json(project_id: i64, current_signature: Option<String>) -> Value {
    let services = match running_services().lock() {
        Ok(services) => services,
        Err(_) => {
            return json!({
                "running": false,
                "restartRequired": false,
                "lastError": "runtime registry lock failed",
                "startedAt": null,
                "snapshot": null
            });
        }
    };
    if let Some(service) = services.get(&project_id) {
        let last_error = service.last_error.lock().ok().and_then(|value| value.clone());
        let snapshot_signature = service.snapshot_signature();
        let restart_required = current_signature
            .as_ref()
            .map(|signature| signature != &snapshot_signature)
            .unwrap_or(false);
        return json!({
            "running": true,
            "restartRequired": restart_required,
            "lastError": last_error,
            "startedAt": service.started_at,
            "snapshot": {
                "host": service.host,
                "port": service.port,
                "routeSignature": snapshot_signature
            }
        });
    }
    json!({
        "running": false,
        "restartRequired": false,
        "lastError": null,
        "startedAt": null,
        "snapshot": null
    })
}

fn project_create_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let name = parse_string(payload, "name")
        .filter(|value| !value.is_empty())
        .ok_or("name is required")?;
    let description = parse_string(payload, "description").unwrap_or_default();
    let host = parse_string(payload, "host").unwrap_or_else(|| "127.0.0.1".into());
    validate_host(&host)?;
    let port = parse_required_i64(payload, "port")?;
    validate_port(port)?;
    let enabled_cors_default = payload["enabledCorsDefault"].as_bool().unwrap_or(true);
    let sort_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM api_mock_projects",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("query next project order failed: {e}"))?;
    conn.execute(
        "INSERT INTO api_mock_projects(name, description, host, port, enabled_cors_default, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![name, description, host, port, enabled_cors_default as i64, sort_order],
    )
    .map_err(|e| format!("create api mock project failed: {e}"))?;
    Ok(json!({ "id": conn.last_insert_rowid() }))
}

fn project_update_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_required_i64(payload, "id")?;
    if !project_exists(conn, id)? {
        return Err("api mock project not found".into());
    }
    let name = parse_string(payload, "name")
        .filter(|value| !value.is_empty())
        .ok_or("name is required")?;
    let description = parse_string(payload, "description").unwrap_or_default();
    let host = parse_string(payload, "host").unwrap_or_else(|| "127.0.0.1".into());
    validate_host(&host)?;
    let port = parse_required_i64(payload, "port")?;
    validate_port(port)?;
    let enabled_cors_default = payload["enabledCorsDefault"].as_bool().unwrap_or(true);
    conn.execute(
        "UPDATE api_mock_projects
         SET name=?1, description=?2, host=?3, port=?4, enabled_cors_default=?5, updated_at=CURRENT_TIMESTAMP
         WHERE id=?6",
        params![name, description, host, port, enabled_cors_default as i64, id],
    )
    .map_err(|e| format!("update api mock project failed: {e}"))?;
    Ok(json!({ "id": id }))
}

fn project_list_with_conn(conn: &Connection) -> Result<Value, String> {
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.name, p.description, p.host, p.port, p.enabled_cors_default, p.sort_order,
                    COUNT(r.id) AS route_count,
                    COALESCE(SUM(CASE WHEN r.enabled=1 THEN 1 ELSE 0 END), 0) AS enabled_route_count
             FROM api_mock_projects p
             LEFT JOIN api_mock_routes r ON r.project_id=p.id
             GROUP BY p.id
             ORDER BY p.sort_order ASC, p.id ASC",
        )
        .map_err(|e| format!("prepare api mock project list failed: {e}"))?;
    let mut projects = stmt
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            Ok(json!({
                "id": id,
                "name": row.get::<_, String>(1)?,
                "description": row.get::<_, String>(2)?,
                "host": row.get::<_, String>(3)?,
                "port": row.get::<_, i64>(4)?,
                "enabledCorsDefault": row.get::<_, i64>(5)? != 0,
                "sortOrder": row.get::<_, i64>(6)?,
                "routeCount": row.get::<_, i64>(7)?,
                "enabledRouteCount": row.get::<_, i64>(8)?,
                "runtime": runtime_summary_json(id, None)
            }))
        })
        .map_err(|e| format!("query api mock projects failed: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read api mock project failed: {e}"))?;
    for project in &mut projects {
        if let Some(id) = project["id"].as_i64() {
            let signature = load_enabled_route_snapshots(conn, id)
                .ok()
                .map(|routes| build_route_signature(&routes));
            project["runtime"] = runtime_summary_json(id, signature);
        }
    }
    Ok(json!({ "projects": projects }))
}

fn project_reorder_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let ids = payload["ids"].as_array().ok_or("ids is required")?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("begin api mock project reorder failed: {e}"))?;
    for (index, id) in ids.iter().enumerate() {
        let id = id.as_i64().ok_or("ids must contain numbers")?;
        tx.execute(
            "UPDATE api_mock_projects SET sort_order=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![index as i64, id],
        )
        .map_err(|e| format!("update api mock project order failed: {e}"))?;
    }
    tx.commit()
        .map_err(|e| format!("commit api mock project reorder failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn collect_project_file_ids(conn: &Connection, project_id: i64) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT file_id FROM api_mock_routes WHERE project_id=?1 AND file_id IS NOT NULL")
        .map_err(|e| format!("prepare project file ids failed: {e}"))?;
    let ids = stmt
        .query_map([project_id], |row| row.get(0))
        .map_err(|e| format!("query project file ids failed: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read project file ids failed: {e}"))?;
    Ok(ids)
}

fn cleanup_file_ids(conn: &Connection, file_ids: &[i64]) -> Vec<String> {
    let mut warnings = Vec::new();
    for file_id in file_ids {
        let ref_count = conn
            .query_row(
                "SELECT COUNT(*) FROM api_mock_routes WHERE file_id=?1",
                [file_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(1);
        if ref_count > 0 {
            continue;
        }
        let stored_path = conn
            .query_row("SELECT stored_path FROM api_mock_files WHERE id=?1", [file_id], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .unwrap_or(None);
        if let Some(stored_path) = stored_path {
            let path_ref_count = conn
                .query_row(
                    "SELECT COUNT(*)
                     FROM api_mock_routes r
                     INNER JOIN api_mock_files f ON f.id=r.file_id
                     WHERE f.stored_path=?1 AND f.id<>?2",
                    params![stored_path, file_id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap_or(0);
            match validate_stored_file_path(&stored_path) {
                Ok(path) => {
                    if path_ref_count == 0 && path.exists() {
                        if let Err(err) = fs::remove_file(&path) {
                            warnings.push(format!("delete file copy failed: {err}"));
                        }
                    }
                }
                Err(err) => warnings.push(err),
            }
        }
        if let Err(err) = conn.execute("DELETE FROM api_mock_files WHERE id=?1", [file_id]) {
            warnings.push(format!("delete file record failed: {err}"));
        }
    }
    warnings
}

fn project_delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_required_i64(payload, "id")?;
    stop_runtime_service(id)?;
    let file_ids = collect_project_file_ids(conn, id)?;
    conn.execute("DELETE FROM api_mock_projects WHERE id=?1", [id])
        .map_err(|e| format!("delete api mock project failed: {e}"))?;
    Ok(json!({ "ok": true, "warnings": cleanup_file_ids(conn, &file_ids) }))
}

fn route_select_sql(where_sql: &str) -> String {
    format!(
        "SELECT r.id, r.project_id, r.name, r.method, r.path_pattern, r.status_code,
                r.response_kind, r.content_type, r.headers_json, r.body_text, r.cors_json,
                r.enabled, r.sort_order,
                f.id, f.original_name, f.content_type, f.size, f.hash, f.created_at,
                r.delay_ms
         FROM api_mock_routes r
         LEFT JOIN api_mock_files f ON f.id=r.file_id
         {where_sql}"
    )
}

fn route_row_to_json(row: &rusqlite::Row<'_>, include_detail: bool) -> rusqlite::Result<Value> {
    let id: i64 = row.get(0)?;
    let file_id: Option<i64> = row.get(13)?;
    let file_json = file_id.map(|id| {
        json!({
            "id": id,
            "originalName": row.get::<_, Option<String>>(14).ok().flatten().unwrap_or_default(),
            "contentType": row.get::<_, Option<String>>(15).ok().flatten().unwrap_or_default(),
            "size": row.get::<_, Option<i64>>(16).ok().flatten().unwrap_or_default(),
            "hash": row.get::<_, Option<String>>(17).ok().flatten().unwrap_or_default(),
            "createdAt": row.get::<_, Option<String>>(18).ok().flatten().unwrap_or_default()
        })
    });
    let headers_json: String = row.get(8)?;
    let body_text: String = row.get(9)?;
    let cors_json: String = row.get(10)?;
    let mut value = json!({
        "id": id,
        "projectId": row.get::<_, i64>(1)?,
        "name": row.get::<_, String>(2)?,
        "method": row.get::<_, String>(3)?,
        "pathPattern": row.get::<_, String>(4)?,
        "statusCode": row.get::<_, i64>(5)?,
        "responseKind": row.get::<_, String>(6)?,
        "contentType": row.get::<_, String>(7)?,
        "enabled": row.get::<_, i64>(11)? != 0,
        "sortOrder": row.get::<_, i64>(12)?,
        "delayMs": row.get::<_, i64>(19)?,
        "file": file_json
    });
    if include_detail {
        value["headers"] = serde_json::from_str::<Value>(&headers_json).unwrap_or_else(|_| json!([]));
        value["bodyText"] = json!(body_text);
        value["cors"] = serde_json::from_str::<Value>(&cors_json).unwrap_or_else(|_| json!({}));
    }
    Ok(value)
}

fn route_list_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let project_id = parse_required_i64(payload, "projectId")?;
    let sql = route_select_sql("WHERE r.project_id=?1 ORDER BY r.sort_order ASC, r.id ASC");
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("prepare api mock route list failed: {e}"))?;
    let routes = stmt
        .query_map([project_id], |row| route_row_to_json(row, false))
        .map_err(|e| format!("query api mock route list failed: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read api mock route failed: {e}"))?;
    Ok(json!({ "routes": routes }))
}

fn route_get_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_required_i64(payload, "id")?;
    let sql = route_select_sql("WHERE r.id=?1");
    let route = conn
        .query_row(&sql, [id], |row| route_row_to_json(row, true))
        .optional()
        .map_err(|e| format!("query api mock route detail failed: {e}"))?
        .ok_or_else(|| "api mock route not found".to_string())?;
    Ok(json!({ "route": route }))
}

fn route_save_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_optional_i64(payload, "id");
    let project_id = parse_required_i64(payload, "projectId")?;
    if !project_exists(conn, project_id)? {
        return Err("api mock project not found".into());
    }
    let name = parse_string(payload, "name")
        .filter(|value| !value.is_empty())
        .ok_or("name is required")?;
    let method = parse_string(payload, "method").unwrap_or_else(|| "GET".into());
    validate_method(&method)?;
    let path_pattern = parse_string(payload, "pathPattern").ok_or("pathPattern is required")?;
    validate_path_pattern(&path_pattern)?;
    let status_code = validate_status_code(payload["statusCode"].as_i64().unwrap_or(200))?;
    let response_kind = parse_string(payload, "responseKind").unwrap_or_else(|| "static_body".into());
    validate_response_kind(&response_kind)?;
    let content_type = parse_string(payload, "contentType")
        .unwrap_or_else(|| "application/json; charset=utf-8".into());
    let headers = parse_headers(&payload["headers"])?;
    let headers_json = serde_json::to_string(&headers).map_err(|e| format!("serialize headers failed: {e}"))?;
    let body_text = payload["bodyText"].as_str().unwrap_or_default().to_string();
    let cors = parse_cors_config(&payload["cors"])?;
    let cors_json = serde_json::to_string(&cors).map_err(|e| format!("serialize cors failed: {e}"))?;
    let enabled = payload["enabled"].as_bool().unwrap_or(true);
    let delay_ms = validate_delay_ms(payload["delayMs"].as_i64().unwrap_or(0))?;
    let file_id = parse_optional_i64(payload, "fileId");
    if response_kind == "file" {
        let file_id = file_id.ok_or("fileId is required for file response")?;
        if !file_exists(conn, file_id)? {
            return Err("api mock file not found".into());
        }
    }
    let old_file_id = if let Some(id) = id {
        conn.query_row("SELECT file_id FROM api_mock_routes WHERE id=?1", [id], |row| row.get(0))
            .optional()
            .map_err(|e| format!("query old route file failed: {e}"))?
            .flatten()
    } else {
        None
    };
    let route_id = if let Some(id) = id {
        let affected = conn.execute(
            "UPDATE api_mock_routes
             SET project_id=?1, name=?2, method=?3, path_pattern=?4, status_code=?5,
                 response_kind=?6, content_type=?7, headers_json=?8, body_text=?9,
                 file_id=?10, cors_json=?11, enabled=?12, delay_ms=?13, updated_at=CURRENT_TIMESTAMP
             WHERE id=?14",
            params![
                project_id,
                name,
                method,
                path_pattern,
                status_code as i64,
                response_kind,
                content_type,
                headers_json,
                body_text,
                file_id,
                cors_json,
                enabled as i64,
                delay_ms,
                id
            ],
        )
        .map_err(|e| format!("update api mock route failed: {e}"))?;
        if affected == 0 {
            return Err("api mock route not found".into());
        }
        id
    } else {
        let sort_order: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM api_mock_routes WHERE project_id=?1",
                [project_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("query next route order failed: {e}"))?;
        conn.execute(
            "INSERT INTO api_mock_routes(
                project_id, name, method, path_pattern, status_code, response_kind, content_type,
                headers_json, body_text, file_id, cors_json, enabled, delay_ms, sort_order
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                project_id,
                name,
                method,
                path_pattern,
                status_code as i64,
                response_kind,
                content_type,
                headers_json,
                body_text,
                file_id,
                cors_json,
                enabled as i64,
                delay_ms,
                sort_order
            ],
        )
        .map_err(|e| format!("create api mock route failed: {e}"))?;
        conn.last_insert_rowid()
    };
    let warnings = if old_file_id.is_some() && old_file_id != file_id {
        cleanup_file_ids(conn, &old_file_id.into_iter().collect::<Vec<_>>())
    } else {
        Vec::new()
    };
    Ok(json!({ "id": route_id, "warnings": warnings }))
}

fn route_reorder_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let project_id = parse_required_i64(payload, "projectId")?;
    let ids = payload["ids"].as_array().ok_or("ids is required")?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("begin api mock route reorder failed: {e}"))?;
    for (index, id) in ids.iter().enumerate() {
        let id = id.as_i64().ok_or("ids must contain numbers")?;
        tx.execute(
            "UPDATE api_mock_routes SET sort_order=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2 AND project_id=?3",
            params![index as i64, id, project_id],
        )
        .map_err(|e| format!("update api mock route order failed: {e}"))?;
    }
    tx.commit()
        .map_err(|e| format!("commit api mock route reorder failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn route_toggle_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_required_i64(payload, "id")?;
    let enabled = payload["enabled"].as_bool().ok_or("enabled is required")?;
    let affected = conn
        .execute(
            "UPDATE api_mock_routes SET enabled=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![enabled as i64, id],
        )
        .map_err(|e| format!("toggle api mock route failed: {e}"))?;
    if affected == 0 {
        return Err("api mock route not found".into());
    }
    Ok(json!({ "ok": true }))
}

fn route_delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_required_i64(payload, "id")?;
    let file_id: Option<i64> = conn
        .query_row("SELECT file_id FROM api_mock_routes WHERE id=?1", [id], |row| row.get(0))
        .optional()
        .map_err(|e| format!("query route file before delete failed: {e}"))?
        .flatten();
    conn.execute("DELETE FROM api_mock_routes WHERE id=?1", [id])
        .map_err(|e| format!("delete api mock route failed: {e}"))?;
    Ok(json!({ "ok": true, "warnings": cleanup_file_ids(conn, &file_id.into_iter().collect::<Vec<_>>()) }))
}

fn file_import_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let path = payload["path"].as_str().ok_or("path is required")?;
    let content_type = payload["contentType"].as_str().unwrap_or_default();
    let imported = import_file_copy(Path::new(path), content_type)?;
    conn.execute(
        "INSERT INTO api_mock_files(original_name, stored_path, content_type, size, hash)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            imported.original_name,
            imported.stored_path,
            imported.content_type,
            imported.size,
            imported.hash
        ],
    )
    .map_err(|e| format!("insert api mock file failed: {e}"))?;
    Ok(json!({
        "file": {
            "id": conn.last_insert_rowid(),
            "originalName": imported.original_name,
            "storedPath": imported.stored_path,
            "contentType": imported.content_type,
            "size": imported.size,
            "hash": imported.hash
        }
    }))
}

fn build_route_signature(routes: &[MockRouteSnapshot]) -> String {
    let mut parts = routes
        .iter()
        .map(|route| {
            let headers = serde_json::to_string(&route.headers).unwrap_or_default();
            let cors = serde_json::to_string(&route.cors).unwrap_or_default();
            let file_hash = route.file.as_ref().map(|file| file.hash.as_str()).unwrap_or("");
            let body_hash = blake3::hash(route.body_text.as_bytes()).to_hex().to_string();
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
                route.id,
                route.method,
                route.path_pattern,
                route.status_code,
                route.response_kind,
                route.content_type,
                headers,
                cors,
                file_hash,
                body_hash,
                route.delay_ms,
                route.sort_order,
            )
        })
        .collect::<Vec<_>>();
    parts.sort();
    blake3::hash(parts.join("|").as_bytes()).to_hex().to_string()
}

fn load_enabled_route_snapshots(conn: &Connection, project_id: i64) -> Result<Vec<MockRouteSnapshot>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT r.id, r.name, r.method, r.path_pattern, r.status_code, r.response_kind,
                    r.content_type, r.headers_json, r.body_text, r.cors_json, r.sort_order,
                    f.id, f.original_name, f.stored_path, f.content_type, f.size, f.hash,
                    r.delay_ms
             FROM api_mock_routes r
             LEFT JOIN api_mock_files f ON f.id=r.file_id
             WHERE r.project_id=?1 AND r.enabled=1
             ORDER BY r.sort_order ASC, r.id ASC",
        )
        .map_err(|e| format!("prepare route snapshot failed: {e}"))?;
    let routes = stmt
        .query_map([project_id], |row| {
            let headers_json: String = row.get(7)?;
            let cors_json: String = row.get(9)?;
            let file_id: Option<i64> = row.get(11)?;
            let file = file_id.map(|id| MockFileSnapshot {
                id,
                original_name: row.get::<_, Option<String>>(12).ok().flatten().unwrap_or_default(),
                stored_path: row.get::<_, Option<String>>(13).ok().flatten().unwrap_or_default(),
                content_type: row.get::<_, Option<String>>(14).ok().flatten().unwrap_or_default(),
                size: row.get::<_, Option<i64>>(15).ok().flatten().unwrap_or_default(),
                hash: row.get::<_, Option<String>>(16).ok().flatten().unwrap_or_default(),
            });
            let headers = serde_json::from_str::<Vec<HeaderRow>>(&headers_json).unwrap_or_default();
            let cors = serde_json::from_str::<CorsConfig>(&cors_json).unwrap_or_else(|_| default_cors_config());
            Ok(MockRouteSnapshot {
                id: row.get(0)?,
                name: row.get(1)?,
                method: row.get(2)?,
                path_pattern: row.get(3)?,
                status_code: row.get::<_, i64>(4)? as u16,
                response_kind: row.get(5)?,
                content_type: row.get(6)?,
                headers,
                body_text: row.get(8)?,
                file,
                cors,
                delay_ms: row.get(17)?,
                sort_order: row.get(10)?,
            })
        })
        .map_err(|e| format!("query route snapshot failed: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read route snapshot failed: {e}"))?;
    Ok(routes)
}

fn read_project_endpoint(conn: &Connection, project_id: i64) -> Result<(String, u16), String> {
    let (host, port): (String, i64) = conn
        .query_row(
            "SELECT host, port FROM api_mock_projects WHERE id=?1",
            [project_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("query api mock project endpoint failed: {e}"))?
        .ok_or_else(|| "api mock project not found".to_string())?;
    validate_host(&host)?;
    validate_port(port)?;
    Ok((host, port as u16))
}

fn service_start_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let project_id = parse_required_i64(payload, "projectId")?;
    let (host, port) = read_project_endpoint(conn, project_id)?;
    let routes = load_enabled_route_snapshots(conn, project_id)?;
    if routes.is_empty() {
        return Err("api mock project has no enabled route".into());
    }
    let mut services = running_services()
        .lock()
        .map_err(|e| format!("runtime registry lock failed: {e}"))?;
    if services.contains_key(&project_id) {
        return Err("api mock service is already running".into());
    }
    let listener = TcpListener::bind(format!("{host}:{port}"))
        .map_err(|e| format!("bind {host}:{port} failed: {e}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("set listener nonblocking failed: {e}"))?;

    let stop = Arc::new(AtomicBool::new(false));
    let logs = Arc::new(Mutex::new(VecDeque::new()));
    let last_error = Arc::new(Mutex::new(None));
    let routes = Arc::new(routes);
    let thread_stop = Arc::clone(&stop);
    let thread_logs = Arc::clone(&logs);
    let thread_error = Arc::clone(&last_error);
    let thread_routes = Arc::clone(&routes);
    let handle = thread::spawn(move || {
        run_http_service(listener, thread_routes, thread_stop, thread_logs, thread_error);
    });
    let service = RunningMockService {
        host,
        port,
        started_at: chrono::Utc::now().to_rfc3339(),
        stop,
        handle: Some(handle),
        route_snapshot: routes,
        logs,
        last_error,
    };
    services.insert(project_id, service);
    drop(services);
    Ok(json!({ "status": runtime_summary_json(project_id, None) }))
}

fn stop_runtime_service(project_id: i64) -> Result<(), String> {
    let mut service = {
        let mut services = running_services()
            .lock()
            .map_err(|e| format!("runtime registry lock failed: {e}"))?;
        services.remove(&project_id)
    };
    if let Some(service) = service.as_mut() {
        service.stop.store(true, Ordering::Release);
        if let Some(handle) = service.handle.take() {
            handle
                .join()
                .map_err(|_| "api mock service thread panicked".to_string())?;
        }
    }
    Ok(())
}

fn service_stop_with_conn(_conn: &Connection, payload: &Value) -> Result<Value, String> {
    let project_id = parse_required_i64(payload, "projectId")?;
    stop_runtime_service(project_id)?;
    Ok(json!({ "status": runtime_summary_json(project_id, None) }))
}

fn service_status_with_conn(_conn: &Connection, payload: &Value) -> Result<Value, String> {
    let project_id = parse_required_i64(payload, "projectId")?;
    Ok(json!({ "status": runtime_summary_json(project_id, None) }))
}

fn request_logs_with_conn(_conn: &Connection, payload: &Value) -> Result<Value, String> {
    let project_id = parse_required_i64(payload, "projectId")?;
    let services = running_services()
        .lock()
        .map_err(|e| format!("runtime registry lock failed: {e}"))?;
    let logs = services
        .get(&project_id)
        .and_then(|service| service.logs.lock().ok().map(|logs| logs.iter().cloned().collect::<Vec<_>>()))
        .unwrap_or_default();
    Ok(json!({ "logs": logs }))
}

fn request_logs_clear_with_conn(_conn: &Connection, payload: &Value) -> Result<Value, String> {
    let project_id = parse_required_i64(payload, "projectId")?;
    let services = running_services()
        .lock()
        .map_err(|e| format!("runtime registry lock failed: {e}"))?;
    if let Some(service) = services.get(&project_id) {
        if let Ok(mut logs) = service.logs.lock() {
            logs.clear();
        }
    }
    Ok(json!({ "ok": true }))
}

fn run_http_service(
    listener: TcpListener,
    routes: Arc<Vec<MockRouteSnapshot>>,
    stop: Arc<AtomicBool>,
    logs: Arc<Mutex<VecDeque<RequestLogEntry>>>,
    last_error: Arc<Mutex<Option<String>>>,
) {
    let active_connections = Arc::new(AtomicUsize::new(0));
    let mut log_id: u64 = 0;
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => {
                log_id = log_id.saturating_add(1);
                let conn_routes = Arc::clone(&routes);
                let conn_stop = Arc::clone(&stop);
                let conn_logs = Arc::clone(&logs);
                let conn_error = Arc::clone(&last_error);
                let conn_active = Arc::clone(&active_connections);
                let conn_log_id = log_id;
                // 每连接一线程：延迟路由不阻塞其他请求，也不阻塞 stop 的 join。
                thread::spawn(move || {
                    let guard = ConnectionGuard::acquire(&conn_active);
                    if let Err(err) =
                        handle_http_stream(stream, &conn_routes, &conn_logs, conn_log_id, &conn_stop, guard.over_limit())
                    {
                        if let Ok(mut last_error) = conn_error.lock() {
                            *last_error = Some(err);
                        }
                    }
                    drop(guard);
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(err) => {
                if let Ok(mut last_error) = last_error.lock() {
                    *last_error = Some(format!("accept failed: {err}"));
                }
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

fn handle_http_stream(
    mut stream: TcpStream,
    routes: &[MockRouteSnapshot],
    logs: &Arc<Mutex<VecDeque<RequestLogEntry>>>,
    log_id: u64,
    stop: &AtomicBool,
    over_limit: bool,
) -> Result<(), String> {
    let start = Instant::now();
    // Windows 下 accept 出的 socket 继承监听端非阻塞模式，连接线程内改回阻塞读并限时。
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut buffer = [0_u8; 8192];
    let size = stream.read(&mut buffer).map_err(|e| format!("read request failed: {e}"))?;
    let request = String::from_utf8_lossy(&buffer[..size]);
    let request_line = request.lines().next().ok_or("missing request line")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let raw_path = parts.next().unwrap_or("/");
    let path = raw_path.split('?').next().unwrap_or("/").to_string();

    if over_limit {
        let response = build_response(503, Vec::new(), Vec::new(), b"Service Unavailable".to_vec(), false);
        let bytes = response.into_bytes();
        stream
            .write_all(&bytes)
            .map_err(|e| format!("write response failed: {e}"))?;
        push_log(
            logs,
            RequestLogEntry {
                id: log_id,
                time: chrono::Utc::now().to_rfc3339(),
                method,
                path,
                status: 503,
                route_id: None,
                route_name: None,
                duration_ms: start.elapsed().as_millis(),
                error: Some(OVER_LIMIT_LOG_ERROR.to_string()),
            },
        );
        return Ok(());
    }

    let preflight_method = request_header_value(&request, "Access-Control-Request-Method")
        .map(|value| value.to_ascii_uppercase());
    let mut route_id = None;
    let mut route_name = None;
    let mut error = None;
    let mut delay_ms: u64 = 0;

    let response = if method == "OPTIONS" {
        if let Some(route) = select_preflight_route(routes, &path, preflight_method.as_deref()) {
            // CORS 预检不套用路由延迟，避免浏览器场景下预检 + 实际请求双重延迟。
            route_id = Some(route.id);
            route_name = Some(route.name.clone());
            build_response(204, route.headers.clone(), build_cors_headers(&route.cors, &route.method), Vec::new(), false)
        } else if let Some(route) = select_route(routes, "OPTIONS", &path) {
            route_id = Some(route.id);
            route_name = Some(route.name.clone());
            delay_ms = route.delay_ms.max(0) as u64;
            match build_route_response(route, method == "HEAD") {
                Ok(response) => response,
                Err(err) => {
                    error = Some(err.clone());
                    build_response(500, Vec::new(), Vec::new(), err.into_bytes(), false)
                }
            }
        } else {
            build_response(404, Vec::new(), Vec::new(), b"Not Found".to_vec(), false)
        }
    } else if let Some(route) = select_route(routes, &method, &path) {
        route_id = Some(route.id);
        route_name = Some(route.name.clone());
        delay_ms = route.delay_ms.max(0) as u64;
        match build_route_response(route, method == "HEAD") {
            Ok(response) => response,
            Err(err) => {
                error = Some(err.clone());
                build_response(500, Vec::new(), Vec::new(), err.into_bytes(), false)
            }
        }
    } else {
        build_response(404, Vec::new(), Vec::new(), b"Not Found".to_vec(), false)
    };

    if delay_ms > 0 && !sleep_with_stop(delay_ms, stop) {
        // 服务停止中：直接断开，不返回旧配置的响应。
        return Ok(());
    }

    let status = response.status;
    let bytes = response.into_bytes();
    stream
        .write_all(&bytes)
        .map_err(|e| format!("write response failed: {e}"))?;
    let entry = RequestLogEntry {
        id: log_id,
        time: chrono::Utc::now().to_rfc3339(),
        method,
        path,
        status,
        route_id,
        route_name,
        duration_ms: start.elapsed().as_millis(),
        error,
    };
    push_log(logs, entry);
    Ok(())
}

fn select_preflight_route<'a>(
    routes: &'a [MockRouteSnapshot],
    path: &str,
    request_method: Option<&str>,
) -> Option<&'a MockRouteSnapshot> {
    let candidates = routes
        .iter()
        .filter(|route| route.cors.enabled && path_matches(&route.path_pattern, path))
        .filter(|route| request_method.map(|method| route.method == method).unwrap_or(true));
    candidates.max_by_key(|route| {
        (
            route_specificity(&route.path_pattern),
            std::cmp::Reverse(route.sort_order),
            std::cmp::Reverse(route.id),
        )
    })
}

struct HttpResponse {
    status: u16,
    headers: Vec<HeaderRow>,
    body: Vec<u8>,
}

impl HttpResponse {
    fn into_bytes(self) -> Vec<u8> {
        let mut out = Vec::new();
        let reason = status_reason(self.status);
        out.extend_from_slice(format!("HTTP/1.1 {} {}\r\n", self.status, reason).as_bytes());
        for header in self.headers {
            out.extend_from_slice(format!("{}: {}\r\n", header.key, header.value).as_bytes());
        }
        out.extend_from_slice(b"Connection: close\r\n\r\n");
        out.extend_from_slice(&self.body);
        out
    }
}

fn build_route_response(route: &MockRouteSnapshot, head_only: bool) -> Result<HttpResponse, String> {
    let mut headers = route.headers.clone();
    let mut body = if route.response_kind == "file" {
        let file = route.file.as_ref().ok_or("file response missing file record")?;
        let path = validate_stored_file_path(&file.stored_path)?;
        fs::read(&path).map_err(|e| format!("read api mock file failed: {e}"))?
    } else {
        route.body_text.as_bytes().to_vec()
    };
    let content_type = if !route.content_type.trim().is_empty() {
        route.content_type.clone()
    } else {
        route
            .file
            .as_ref()
            .map(|file| file.content_type.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "application/octet-stream".into())
    };
    if header_value(&headers, "Content-Type").is_none() {
        headers.push(header("Content-Type", &content_type));
    }
    headers.extend(build_cors_headers(&route.cors, &route.method));
    if head_only {
        body.clear();
    }
    Ok(build_response(route.status_code, headers, Vec::new(), body, false))
}

fn build_response(
    status: u16,
    mut headers: Vec<HeaderRow>,
    extra_headers: Vec<HeaderRow>,
    body: Vec<u8>,
    skip_content_length: bool,
) -> HttpResponse {
    headers.extend(extra_headers);
    if !skip_content_length && header_value(&headers, "Content-Length").is_none() {
        headers.push(header("Content-Length", &body.len().to_string()));
    }
    HttpResponse { status, headers, body }
}

fn status_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "OK",
    }
}

fn push_log(logs: &Arc<Mutex<VecDeque<RequestLogEntry>>>, entry: RequestLogEntry) {
    if let Ok(mut logs) = logs.lock() {
        logs.push_front(entry);
        while logs.len() > 200 {
            logs.pop_back();
        }
    }
}

const ACTIONS: &[&str] = &[
    "project_list",
    "project_create",
    "project_update",
    "project_delete",
    "project_reorder",
    "route_list",
    "route_get",
    "route_save",
    "route_toggle",
    "route_delete",
    "route_reorder",
    "file_import",
    "service_start",
    "service_stop",
    "service_status",
    "request_logs",
    "request_logs_clear",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported api_mock action: {action}"));
    }
    let conn = db_conn()?;
    match action {
        "project_list" => project_list_with_conn(&conn),
        "project_create" => project_create_with_conn(&conn, payload),
        "project_update" => project_update_with_conn(&conn, payload),
        "project_delete" => project_delete_with_conn(&conn, payload),
        "project_reorder" => project_reorder_with_conn(&conn, payload),
        "route_list" => route_list_with_conn(&conn, payload),
        "route_get" => route_get_with_conn(&conn, payload),
        "route_save" => route_save_with_conn(&conn, payload),
        "route_toggle" => route_toggle_with_conn(&conn, payload),
        "route_delete" => route_delete_with_conn(&conn, payload),
        "route_reorder" => route_reorder_with_conn(&conn, payload),
        "file_import" => file_import_with_conn(&conn, payload),
        "service_start" => service_start_with_conn(&conn, payload),
        "service_stop" => service_stop_with_conn(&conn, payload),
        "service_status" => service_status_with_conn(&conn, payload),
        "request_logs" => request_logs_with_conn(&conn, payload),
        "request_logs_clear" => request_logs_clear_with_conn(&conn, payload),
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::json;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_conn() -> Connection {
        static TEST_DB_SEQ: AtomicI64 = AtomicI64::new(1);
        let conn = Connection::open_in_memory().expect("open test db");
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .expect("foreign keys");
        conn.execute_batch(API_MOCK_SCHEMA_SQL).expect("schema");
        let seq = TEST_DB_SEQ.fetch_add(1, Ordering::SeqCst) * 1000;
        conn.execute(
            "INSERT OR REPLACE INTO sqlite_sequence(name, seq) VALUES ('api_mock_projects', ?1)",
            [seq],
        )
        .expect("project seq");
        conn
    }

    fn route(
        id: i64,
        method: &str,
        pattern: &str,
        sort_order: i64,
        status_code: u16,
    ) -> MockRouteSnapshot {
        MockRouteSnapshot {
            id,
            name: format!("route-{id}"),
            method: method.to_string(),
            path_pattern: pattern.to_string(),
            status_code,
            response_kind: "static_body".into(),
            content_type: "text/plain".into(),
            headers: vec![],
            body_text: format!("body-{id}"),
            file: None,
            cors: default_cors_config(),
            delay_ms: 0,
            sort_order,
        }
    }

    #[test]
    fn api_mock_schema_creates_core_tables() {
        let conn = test_conn();
        let tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master
                 WHERE type='table' AND name IN (
                   'api_mock_projects',
                   'api_mock_files',
                   'api_mock_routes'
                 )
                 ORDER BY name",
            )
            .expect("prepare")
            .query_map([], |row| row.get(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows");

        assert_eq!(
            tables,
            vec![
                "api_mock_files".to_string(),
                "api_mock_projects".to_string(),
                "api_mock_routes".to_string()
            ]
        );
    }

    #[test]
    fn api_mock_validates_path_patterns() {
        assert!(validate_path_pattern("/api/users").is_ok());
        assert!(validate_path_pattern("/api/users/:id").is_ok());
        assert!(validate_path_pattern("/files/*").is_ok());

        assert!(validate_path_pattern("api/users").is_err());
        assert!(validate_path_pattern("/users/:").is_err());
        assert!(validate_path_pattern("/users/user-:id").is_err());
        assert!(validate_path_pattern("/files/*/raw").is_err());
        assert!(validate_path_pattern("/bad/:1").is_err());
    }

    #[test]
    fn api_mock_matches_by_specificity_then_sort_order_then_id() {
        let routes = vec![
            route(3, "GET", "/api/users/*", 0, 203),
            route(2, "GET", "/api/users/:id", 0, 202),
            route(1, "GET", "/api/users/me", 10, 201),
            route(4, "GET", "/api/users/me", 1, 204),
        ];

        let exact = select_route(&routes, "GET", "/api/users/me").expect("exact");
        assert_eq!(exact.id, 4);

        let param = select_route(&routes, "GET", "/api/users/42").expect("param");
        assert_eq!(param.id, 2);

        let wildcard = select_route(&routes, "GET", "/api/users/42/raw").expect("wildcard");
        assert_eq!(wildcard.id, 3);
    }

    #[test]
    fn api_mock_filters_method_and_returns_none_for_missing_route() {
        let routes = vec![route(1, "POST", "/api/users", 0, 200)];
        assert!(select_route(&routes, "GET", "/api/users").is_none());
        assert!(select_route(&routes, "POST", "/api/missing").is_none());
    }

    #[test]
    fn api_mock_builds_cors_headers_and_rejects_invalid_credentials_origin() {
        let err = parse_cors_config(&json!({
            "enabled": true,
            "allowOrigin": "*",
            "allowMethods": [],
            "allowHeaders": "*",
            "exposeHeaders": "",
            "allowCredentials": true,
            "maxAgeSeconds": 600
        }))
        .expect_err("invalid cors");
        assert!(err.contains("Allow-Origin"));

        let cors = parse_cors_config(&json!({
            "enabled": true,
            "allowOrigin": "http://localhost:5173",
            "allowMethods": [],
            "allowHeaders": "X-Token",
            "exposeHeaders": "X-Trace",
            "allowCredentials": true,
            "maxAgeSeconds": 600
        }))
        .expect("cors");
        let headers = build_cors_headers(&cors, "PATCH");
        assert_eq!(
            header_value(&headers, "Access-Control-Allow-Origin").as_deref(),
            Some("http://localhost:5173")
        );
        assert_eq!(
            header_value(&headers, "Access-Control-Allow-Methods").as_deref(),
            Some("PATCH")
        );
        assert_eq!(
            header_value(&headers, "Access-Control-Allow-Credentials").as_deref(),
            Some("true")
        );
    }

    #[test]
    fn api_mock_imports_file_into_controlled_directory() {
        let source_dir = std::env::temp_dir().join(format!(
            "lazycat-api-mock-source-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&source_dir).expect("source dir");
        let source = source_dir.join("demo.txt");
        fs::write(&source, b"hello").expect("write source");

        let imported = import_file_copy(&source, "text/plain").expect("import");
        let files_dir = get_api_mock_files_dir().expect("files dir");
        let stored = std::path::PathBuf::from(&imported.stored_path);

        assert!(stored.starts_with(&files_dir));
        assert_eq!(fs::read(stored).expect("read stored"), b"hello");
        assert_eq!(imported.original_name, "demo.txt");
        assert_eq!(imported.content_type, "text/plain");
        assert_eq!(imported.size, 5);
    }

    #[test]
    fn api_mock_rejects_stored_file_path_outside_controlled_directory() {
        let outside = std::env::temp_dir().join("lazycat-api-mock-outside.txt");
        fs::write(&outside, b"outside").expect("outside");
        let err = validate_stored_file_path(&outside.to_string_lossy()).expect_err("outside");
        assert!(err.contains("outside"));
    }

    #[test]
    fn api_mock_project_crud_and_reorder_work() {
        let conn = test_conn();
        let first = project_create_with_conn(
            &conn,
            &json!({
                "name": "Admin",
                "description": "后台接口",
                "host": "127.0.0.1",
                "port": 18080
            }),
        )
        .expect("create first");
        let second = project_create_with_conn(
            &conn,
            &json!({
                "name": "Public",
                "host": "0.0.0.0",
                "port": 18081
            }),
        )
        .expect("create second");

        project_update_with_conn(
            &conn,
            &json!({
                "id": first["id"],
                "name": "Admin API",
                "description": "管理后台",
                "host": "127.0.0.1",
                "port": 18082,
                "enabledCorsDefault": false
            }),
        )
        .expect("update");
        project_reorder_with_conn(&conn, &json!({ "ids": [second["id"], first["id"]] }))
            .expect("reorder");

        let list = project_list_with_conn(&conn).expect("list");
        assert_eq!(list["projects"][0]["name"], "Public");
        assert_eq!(list["projects"][1]["name"], "Admin API");
        assert_eq!(list["projects"][1]["port"], 18082);
        assert_eq!(list["projects"][1]["enabledCorsDefault"], false);

        project_delete_with_conn(&conn, &json!({ "id": first["id"] })).expect("delete");
        let list = project_list_with_conn(&conn).expect("list after delete");
        assert_eq!(list["projects"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn api_mock_route_crud_reorder_and_validation_work() {
        let conn = test_conn();
        let project = project_create_with_conn(&conn, &json!({ "name": "Demo", "port": 18080 }))
            .expect("project");
        let project_id = project["id"].as_i64().unwrap();

        let invalid = route_save_with_conn(
            &conn,
            &json!({
                "projectId": project_id,
                "name": "Bad",
                "method": "TRACE",
                "pathPattern": "/bad",
                "statusCode": 200
            }),
        )
        .expect_err("invalid method");
        assert!(invalid.contains("unsupported method"));

        let first = route_save_with_conn(
            &conn,
            &json!({
                "projectId": project_id,
                "name": "User",
                "method": "GET",
                "pathPattern": "/api/users/:id",
                "statusCode": 200,
                "responseKind": "static_body",
                "contentType": "application/json",
                "headers": [{ "enabled": true, "key": "X-Trace", "value": "abc" }],
                "bodyText": "{\"ok\":true}",
                "cors": default_cors_config()
            }),
        )
        .expect("save first");
        let second = route_save_with_conn(
            &conn,
            &json!({
                "projectId": project_id,
                "name": "Files",
                "method": "GET",
                "pathPattern": "/files/*",
                "statusCode": 201,
                "responseKind": "static_body",
                "contentType": "text/plain",
                "headers": [],
                "bodyText": "file",
                "cors": default_cors_config()
            }),
        )
        .expect("save second");

        route_reorder_with_conn(
            &conn,
            &json!({ "projectId": project_id, "ids": [second["id"], first["id"]] }),
        )
        .expect("reorder");
        let routes = route_list_with_conn(&conn, &json!({ "projectId": project_id })).expect("list");
        assert_eq!(routes["routes"][0]["name"], "Files");
        assert_eq!(routes["routes"][1]["name"], "User");

        let detail = route_get_with_conn(&conn, &json!({ "id": first["id"] })).expect("get");
        assert_eq!(detail["route"]["headers"][0]["key"], "X-Trace");

        route_save_with_conn(
            &conn,
            &json!({
                "id": first["id"],
                "projectId": project_id,
                "name": "User detail",
                "method": "GET",
                "pathPattern": "/api/users/:id",
                "statusCode": 202,
                "responseKind": "static_body",
                "contentType": "application/json",
                "headers": [],
                "bodyText": "{}",
                "cors": default_cors_config(),
                "enabled": false
            }),
        )
        .expect("update route");
        let detail = route_get_with_conn(&conn, &json!({ "id": first["id"] })).expect("get updated");
        assert_eq!(detail["route"]["name"], "User detail");
        assert_eq!(detail["route"]["enabled"], false);

        let missing_update = route_save_with_conn(
            &conn,
            &json!({
                "id": 999_999,
                "projectId": project_id,
                "name": "Missing",
                "method": "GET",
                "pathPattern": "/missing",
                "statusCode": 200,
                "responseKind": "static_body",
                "contentType": "application/json",
                "headers": [],
                "bodyText": "{}",
                "cors": default_cors_config()
            }),
        )
        .expect_err("missing route update");
        assert!(missing_update.contains("route not found"));

        route_delete_with_conn(&conn, &json!({ "id": second["id"] })).expect("delete route");
        let routes = route_list_with_conn(&conn, &json!({ "projectId": project_id })).expect("list after delete");
        assert_eq!(routes["routes"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn api_mock_file_cleanup_keeps_shared_file_and_removes_unreferenced_file() {
        let conn = test_conn();
        let project = project_create_with_conn(&conn, &json!({ "name": "Demo", "port": 18080 }))
            .expect("project");
        let project_id = project["id"].as_i64().unwrap();
        let source_dir = std::env::temp_dir().join("lazycat-api-mock-file-cleanup");
        fs::create_dir_all(&source_dir).expect("source dir");
        let source = source_dir.join("shared.txt");
        fs::write(&source, b"shared").expect("source");
        let imported =
            file_import_with_conn(&conn, &json!({ "path": source, "contentType": "text/plain" }))
                .expect("import");
        let file_id = imported["file"]["id"].as_i64().unwrap();
        let stored_path = imported["file"]["storedPath"].as_str().unwrap().to_string();

        let route_a = route_save_with_conn(
            &conn,
            &json!({
                "projectId": project_id,
                "name": "A",
                "method": "GET",
                "pathPattern": "/a",
                "statusCode": 200,
                "responseKind": "file",
                "contentType": "text/plain",
                "headers": [],
                "fileId": file_id,
                "cors": default_cors_config()
            }),
        )
        .expect("route a");
        let route_b = route_save_with_conn(
            &conn,
            &json!({
                "projectId": project_id,
                "name": "B",
                "method": "GET",
                "pathPattern": "/b",
                "statusCode": 200,
                "responseKind": "file",
                "contentType": "text/plain",
                "headers": [],
                "fileId": file_id,
                "cors": default_cors_config()
            }),
        )
        .expect("route b");

        route_delete_with_conn(&conn, &json!({ "id": route_a["id"] })).expect("delete a");
        assert!(fs::metadata(&stored_path).is_ok());
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM api_mock_files WHERE id=?1", [file_id], |row| row.get(0))
            .expect("file count");
        assert_eq!(count, 1);

        route_delete_with_conn(&conn, &json!({ "id": route_b["id"] })).expect("delete b");
        assert!(fs::metadata(&stored_path).is_err());
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM api_mock_files WHERE id=?1", [file_id], |row| row.get(0))
            .expect("file count after cleanup");
        assert_eq!(count, 0);
    }

    #[test]
    fn api_mock_file_cleanup_keeps_duplicate_file_copy_until_all_records_unreferenced() {
        let conn = test_conn();
        let project = project_create_with_conn(&conn, &json!({ "name": "Demo", "port": 18080 }))
            .expect("project");
        let project_id = project["id"].as_i64().unwrap();
        let source_dir = std::env::temp_dir().join("lazycat-api-mock-duplicate-file-cleanup");
        fs::create_dir_all(&source_dir).expect("source dir");
        let source = source_dir.join("shared.txt");
        // 内容需与 keeps_shared_file 测试不同：物理存储按内容寻址且目录共享，
        // 两个测试若同内容会并行互删对方的存储文件
        fs::write(&source, b"shared-duplicate").expect("source");
        let source_path = source.to_string_lossy().to_string();
        let first_import =
            file_import_with_conn(&conn, &json!({ "path": source_path.clone(), "contentType": "text/plain" }))
                .expect("first import");
        let second_import =
            file_import_with_conn(&conn, &json!({ "path": source_path, "contentType": "text/plain" }))
                .expect("second import");
        let first_file_id = first_import["file"]["id"].as_i64().unwrap();
        let second_file_id = second_import["file"]["id"].as_i64().unwrap();
        let stored_path = first_import["file"]["storedPath"].as_str().unwrap().to_string();
        assert_ne!(first_file_id, second_file_id);
        assert_eq!(stored_path, second_import["file"]["storedPath"].as_str().unwrap());

        let route_a = route_save_with_conn(
            &conn,
            &json!({
                "projectId": project_id,
                "name": "A",
                "method": "GET",
                "pathPattern": "/a",
                "statusCode": 200,
                "responseKind": "file",
                "contentType": "text/plain",
                "headers": [],
                "fileId": first_file_id,
                "cors": default_cors_config()
            }),
        )
        .expect("route a");
        let route_b = route_save_with_conn(
            &conn,
            &json!({
                "projectId": project_id,
                "name": "B",
                "method": "GET",
                "pathPattern": "/b",
                "statusCode": 200,
                "responseKind": "file",
                "contentType": "text/plain",
                "headers": [],
                "fileId": second_file_id,
                "cors": default_cors_config()
            }),
        )
        .expect("route b");

        route_delete_with_conn(&conn, &json!({ "id": route_a["id"] })).expect("delete a");
        assert!(fs::metadata(&stored_path).is_ok());

        route_delete_with_conn(&conn, &json!({ "id": route_b["id"] })).expect("delete b");
        assert!(fs::metadata(&stored_path).is_err());
    }

    #[test]
    fn api_mock_execute_rejects_unknown_action() {
        let err = execute("missing", &json!({})).expect_err("unknown action");
        assert!(err.contains("unsupported api_mock action"));
    }

    fn free_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .expect("bind free port")
            .local_addr()
            .expect("local addr")
            .port()
    }

    fn http_request(port: u16, request: &str) -> String {
        let mut last_err = None;
        for _ in 0..20 {
            match TcpStream::connect(("127.0.0.1", port)) {
                Ok(mut stream) => {
                    stream.write_all(request.as_bytes()).expect("write request");
                    let mut buf = Vec::new();
                    stream.read_to_end(&mut buf).expect("read response");
                    return String::from_utf8_lossy(&buf).to_string();
                }
                Err(err) => {
                    last_err = Some(err);
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
            }
        }
        panic!("connect failed: {last_err:?}");
    }

    #[test]
    fn api_mock_service_refuses_project_without_enabled_routes() {
        let conn = test_conn();
        let port = free_port();
        let project = project_create_with_conn(&conn, &json!({ "name": "Demo", "port": port }))
            .expect("project");
        let err = service_start_with_conn(&conn, &json!({ "projectId": project["id"] }))
            .expect_err("no routes");
        assert!(err.contains("enabled route"));
    }

    #[test]
    fn api_mock_service_serves_static_route_and_logs_request() {
        let conn = test_conn();
        let port = free_port();
        let project = project_create_with_conn(&conn, &json!({ "name": "Demo", "port": port }))
            .expect("project");
        let project_id = project["id"].as_i64().unwrap();
        route_save_with_conn(
            &conn,
            &json!({
                "projectId": project_id,
                "name": "Ping",
                "method": "GET",
                "pathPattern": "/ping",
                "statusCode": 201,
                "responseKind": "static_body",
                "contentType": "application/json",
                "headers": [{ "enabled": true, "key": "X-Mock", "value": "yes" }],
                "bodyText": "{\"ok\":true}",
                "cors": default_cors_config()
            }),
        )
        .expect("route");

        service_start_with_conn(&conn, &json!({ "projectId": project_id })).expect("start");
        let response = http_request(
            port,
            "GET /ping HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
        );
        assert!(response.starts_with("HTTP/1.1 201"));
        assert!(response.contains("Content-Type: application/json"));
        assert!(response.contains("X-Mock: yes"));
        assert!(response.ends_with("{\"ok\":true}"));

        let logs = request_logs_with_conn(&conn, &json!({ "projectId": project_id })).expect("logs");
        assert_eq!(logs["logs"][0]["path"], "/ping");
        assert_eq!(logs["logs"][0]["status"], 201);
        service_stop_with_conn(&conn, &json!({ "projectId": project_id })).expect("stop");
    }

    #[test]
    fn api_mock_project_delete_stops_running_service() {
        let conn = test_conn();
        let port = free_port();
        let project = project_create_with_conn(&conn, &json!({ "name": "Demo", "port": port }))
            .expect("project");
        let project_id = project["id"].as_i64().unwrap();
        route_save_with_conn(
            &conn,
            &json!({
                "projectId": project_id,
                "name": "Ping",
                "method": "GET",
                "pathPattern": "/ping",
                "statusCode": 200,
                "responseKind": "static_body",
                "contentType": "text/plain",
                "headers": [],
                "bodyText": "pong",
                "cors": default_cors_config()
            }),
        )
        .expect("route");

        service_start_with_conn(&conn, &json!({ "projectId": project_id })).expect("start");
        project_delete_with_conn(&conn, &json!({ "id": project_id })).expect("delete");
        let status = runtime_summary_json(project_id, None);
        let running = status["running"].as_bool().unwrap();
        let _ = stop_runtime_service(project_id);

        assert!(!running);
    }

    #[test]
    fn api_mock_service_serves_file_route_and_missing_path_404() {
        let conn = test_conn();
        let port = free_port();
        let project = project_create_with_conn(&conn, &json!({ "name": "Demo", "port": port }))
            .expect("project");
        let project_id = project["id"].as_i64().unwrap();
        let source_dir = std::env::temp_dir().join("lazycat-api-mock-file-route");
        fs::create_dir_all(&source_dir).expect("source dir");
        let source = source_dir.join("hello.txt");
        fs::write(&source, b"hello file").expect("source");
        let imported =
            file_import_with_conn(&conn, &json!({ "path": source, "contentType": "text/plain" }))
                .expect("import");
        route_save_with_conn(
            &conn,
            &json!({
                "projectId": project_id,
                "name": "File",
                "method": "GET",
                "pathPattern": "/download",
                "statusCode": 200,
                "responseKind": "file",
                "contentType": "",
                "headers": [],
                "fileId": imported["file"]["id"],
                "cors": default_cors_config()
            }),
        )
        .expect("route");

        service_start_with_conn(&conn, &json!({ "projectId": project_id })).expect("start");
        let response = http_request(
            port,
            "GET /download HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
        );
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.contains("Content-Length: 10"));
        assert!(response.ends_with("hello file"));

        let missing = http_request(
            port,
            "GET /missing HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
        );
        assert!(missing.starts_with("HTTP/1.1 404"));
        service_stop_with_conn(&conn, &json!({ "projectId": project_id })).expect("stop");
    }

    #[test]
    fn api_mock_service_handles_options_preflight() {
        let conn = test_conn();
        let port = free_port();
        let project = project_create_with_conn(&conn, &json!({ "name": "Demo", "port": port }))
            .expect("project");
        let project_id = project["id"].as_i64().unwrap();
        route_save_with_conn(
            &conn,
            &json!({
                "projectId": project_id,
                "name": "User",
                "method": "PATCH",
                "pathPattern": "/users/:id",
                "statusCode": 200,
                "responseKind": "static_body",
                "contentType": "application/json",
                "headers": [],
                "bodyText": "{}",
                "cors": {
                    "enabled": true,
                    "allowOrigin": "http://localhost:5173",
                    "allowMethods": [],
                    "allowHeaders": "X-Token",
                    "exposeHeaders": "",
                    "allowCredentials": true,
                    "maxAgeSeconds": 600
                }
            }),
        )
        .expect("route");

        service_start_with_conn(&conn, &json!({ "projectId": project_id })).expect("start");
        let response = http_request(
            port,
            "OPTIONS /users/42 HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: http://localhost:5173\r\nConnection: close\r\n\r\n",
        );
        assert!(response.starts_with("HTTP/1.1 204"));
        assert!(response.contains("Access-Control-Allow-Origin: http://localhost:5173"));
        assert!(response.contains("Access-Control-Allow-Methods: PATCH"));
        service_stop_with_conn(&conn, &json!({ "projectId": project_id })).expect("stop");
    }

    #[test]
    fn api_mock_service_preflight_uses_requested_method_when_paths_overlap() {
        let conn = test_conn();
        let port = free_port();
        let project = project_create_with_conn(&conn, &json!({ "name": "Demo", "port": port }))
            .expect("project");
        let project_id = project["id"].as_i64().unwrap();
        route_save_with_conn(
            &conn,
            &json!({
                "projectId": project_id,
                "name": "Read user",
                "method": "GET",
                "pathPattern": "/users/:id",
                "statusCode": 200,
                "responseKind": "static_body",
                "contentType": "application/json",
                "headers": [],
                "bodyText": "{}",
                "cors": default_cors_config()
            }),
        )
        .expect("get route");
        route_save_with_conn(
            &conn,
            &json!({
                "projectId": project_id,
                "name": "Update user",
                "method": "POST",
                "pathPattern": "/users/:id",
                "statusCode": 200,
                "responseKind": "static_body",
                "contentType": "application/json",
                "headers": [],
                "bodyText": "{}",
                "cors": default_cors_config()
            }),
        )
        .expect("post route");

        service_start_with_conn(&conn, &json!({ "projectId": project_id })).expect("start");
        let response = http_request(
            port,
            "OPTIONS /users/42 HTTP/1.1\r\nHost: 127.0.0.1\r\nAccess-Control-Request-Method: POST\r\nConnection: close\r\n\r\n",
        );
        service_stop_with_conn(&conn, &json!({ "projectId": project_id })).expect("stop");

        assert!(response.starts_with("HTTP/1.1 204"));
        assert!(response.contains("Access-Control-Allow-Methods: POST"));
    }

    #[test]
    fn api_mock_service_supports_parallel_projects_and_reports_port_conflict() {
        let conn = test_conn();
        let first_port = free_port();
        let second_port = free_port();
        let first = project_create_with_conn(&conn, &json!({ "name": "A", "port": first_port }))
            .expect("first");
        let second = project_create_with_conn(&conn, &json!({ "name": "B", "port": second_port }))
            .expect("second");
        for project in [&first, &second] {
            route_save_with_conn(
                &conn,
                &json!({
                    "projectId": project["id"],
                    "name": "Ping",
                    "method": "GET",
                    "pathPattern": "/ping",
                    "statusCode": 200,
                    "responseKind": "static_body",
                    "contentType": "text/plain",
                    "headers": [],
                    "bodyText": project["id"].to_string(),
                    "cors": default_cors_config()
                }),
            )
            .expect("route");
        }

        service_start_with_conn(&conn, &json!({ "projectId": first["id"] })).expect("start first");
        service_start_with_conn(&conn, &json!({ "projectId": second["id"] })).expect("start second");
        assert!(http_request(first_port, "GET /ping HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
            .contains(&first["id"].to_string()));
        assert!(http_request(second_port, "GET /ping HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
            .contains(&second["id"].to_string()));

        let conflict = project_create_with_conn(&conn, &json!({ "name": "C", "port": first_port }))
            .expect("conflict project");
        route_save_with_conn(
            &conn,
            &json!({
                "projectId": conflict["id"],
                "name": "Ping",
                "method": "GET",
                "pathPattern": "/ping",
                "statusCode": 200,
                "responseKind": "static_body",
                "contentType": "text/plain",
                "headers": [],
                "bodyText": "c",
                "cors": default_cors_config()
            }),
        )
        .expect("conflict route");
        let err = service_start_with_conn(&conn, &json!({ "projectId": conflict["id"] }))
            .expect_err("port conflict");
        assert!(err.contains("bind"));

        service_stop_with_conn(&conn, &json!({ "projectId": first["id"] })).expect("stop first");
        service_stop_with_conn(&conn, &json!({ "projectId": second["id"] })).expect("stop second");
    }

    fn save_delay_route(
        conn: &Connection,
        project_id: i64,
        pattern: &str,
        delay_ms: i64,
    ) -> Value {
        route_save_with_conn(
            conn,
            &json!({
                "projectId": project_id,
                "name": format!("route {pattern}"),
                "method": "GET",
                "pathPattern": pattern,
                "statusCode": 200,
                "responseKind": "static_body",
                "contentType": "text/plain",
                "headers": [],
                "bodyText": "ok",
                "delayMs": delay_ms,
                "cors": default_cors_config()
            }),
        )
        .expect("save delay route")
    }

    #[test]
    fn api_mock_route_toggle_updates_enabled_and_rejects_missing_id() {
        let conn = test_conn();
        let project = project_create_with_conn(&conn, &json!({ "name": "Demo", "port": 18080 }))
            .expect("project");
        let project_id = project["id"].as_i64().unwrap();
        let route = save_delay_route(&conn, project_id, "/toggle", 0);

        route_toggle_with_conn(&conn, &json!({ "id": route["id"], "enabled": false }))
            .expect("disable");
        let detail = route_get_with_conn(&conn, &json!({ "id": route["id"] })).expect("get");
        assert_eq!(detail["route"]["enabled"], false);

        route_toggle_with_conn(&conn, &json!({ "id": route["id"], "enabled": true }))
            .expect("enable");
        let detail = route_get_with_conn(&conn, &json!({ "id": route["id"] })).expect("get");
        assert_eq!(detail["route"]["enabled"], true);

        let missing = route_toggle_with_conn(&conn, &json!({ "id": 999_999, "enabled": true }))
            .expect_err("missing route");
        assert!(missing.contains("route not found"));

        let no_flag = route_toggle_with_conn(&conn, &json!({ "id": route["id"] }))
            .expect_err("missing enabled");
        assert!(no_flag.contains("enabled is required"));
    }

    #[test]
    fn api_mock_route_save_validates_and_persists_delay() {
        let conn = test_conn();
        let project = project_create_with_conn(&conn, &json!({ "name": "Demo", "port": 18080 }))
            .expect("project");
        let project_id = project["id"].as_i64().unwrap();

        let route = save_delay_route(&conn, project_id, "/delayed", 1500);
        let detail = route_get_with_conn(&conn, &json!({ "id": route["id"] })).expect("get");
        assert_eq!(detail["route"]["delayMs"], 1500);

        let default_route = route_save_with_conn(
            &conn,
            &json!({
                "projectId": project_id,
                "name": "Default",
                "method": "GET",
                "pathPattern": "/default",
                "statusCode": 200,
                "responseKind": "static_body",
                "contentType": "text/plain",
                "headers": [],
                "bodyText": "ok",
                "cors": default_cors_config()
            }),
        )
        .expect("save without delay");
        let detail = route_get_with_conn(&conn, &json!({ "id": default_route["id"] })).expect("get");
        assert_eq!(detail["route"]["delayMs"], 0);

        let list = route_list_with_conn(&conn, &json!({ "projectId": project_id })).expect("list");
        assert_eq!(list["routes"][0]["delayMs"], 1500);

        for invalid in [-1_i64, MAX_MOCK_DELAY_MS + 1] {
            let err = route_save_with_conn(
                &conn,
                &json!({
                    "projectId": project_id,
                    "name": "Bad",
                    "method": "GET",
                    "pathPattern": "/bad",
                    "statusCode": 200,
                    "responseKind": "static_body",
                    "contentType": "text/plain",
                    "headers": [],
                    "bodyText": "ok",
                    "delayMs": invalid,
                    "cors": default_cors_config()
                }),
            )
            .expect_err("invalid delay");
            assert!(err.contains("delayMs"));
        }
    }

    #[test]
    fn api_mock_signature_changes_when_delay_changes() {
        let base = route(1, "GET", "/api/demo", 0, 200);
        let mut delayed = base.clone();
        delayed.delay_ms = 500;
        assert_ne!(
            build_route_signature(&[base]),
            build_route_signature(&[delayed])
        );
    }

    #[test]
    fn api_mock_request_logs_clear_removes_runtime_logs() {
        let conn = test_conn();
        let port = free_port();
        let project = project_create_with_conn(&conn, &json!({ "name": "Demo", "port": port }))
            .expect("project");
        let project_id = project["id"].as_i64().unwrap();
        save_delay_route(&conn, project_id, "/ping", 0);

        request_logs_clear_with_conn(&conn, &json!({ "projectId": project_id }))
            .expect("clear before start is ok");

        service_start_with_conn(&conn, &json!({ "projectId": project_id })).expect("start");
        http_request(port, "GET /ping HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n");
        let logs = request_logs_with_conn(&conn, &json!({ "projectId": project_id })).expect("logs");
        assert_eq!(logs["logs"].as_array().unwrap().len(), 1);

        request_logs_clear_with_conn(&conn, &json!({ "projectId": project_id })).expect("clear");
        let logs = request_logs_with_conn(&conn, &json!({ "projectId": project_id })).expect("logs");
        assert_eq!(logs["logs"].as_array().unwrap().len(), 0);
        service_stop_with_conn(&conn, &json!({ "projectId": project_id })).expect("stop");
    }

    #[test]
    fn api_mock_service_applies_route_delay() {
        let conn = test_conn();
        let port = free_port();
        let project = project_create_with_conn(&conn, &json!({ "name": "Demo", "port": port }))
            .expect("project");
        let project_id = project["id"].as_i64().unwrap();
        save_delay_route(&conn, project_id, "/slow", 400);

        service_start_with_conn(&conn, &json!({ "projectId": project_id })).expect("start");
        let started = std::time::Instant::now();
        let response = http_request(port, "GET /slow HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n");
        let elapsed = started.elapsed();
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(elapsed >= Duration::from_millis(380), "elapsed {elapsed:?}");
        service_stop_with_conn(&conn, &json!({ "projectId": project_id })).expect("stop");
    }

    #[test]
    fn api_mock_service_delay_does_not_block_parallel_requests() {
        let conn = test_conn();
        let port = free_port();
        let project = project_create_with_conn(&conn, &json!({ "name": "Demo", "port": port }))
            .expect("project");
        let project_id = project["id"].as_i64().unwrap();
        save_delay_route(&conn, project_id, "/slow", 1500);
        save_delay_route(&conn, project_id, "/fast", 0);

        service_start_with_conn(&conn, &json!({ "projectId": project_id })).expect("start");
        // 预热：确认服务已可接受连接，让后续计时不含连接重试。
        http_request(port, "GET /fast HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n");

        let slow = thread::spawn(move || {
            http_request(port, "GET /slow HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        });
        thread::sleep(Duration::from_millis(150));

        let started = std::time::Instant::now();
        let fast = http_request(port, "GET /fast HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n");
        let fast_elapsed = started.elapsed();
        assert!(fast.starts_with("HTTP/1.1 200"));
        assert!(
            fast_elapsed < Duration::from_millis(1000),
            "fast request blocked by slow route: {fast_elapsed:?}"
        );

        let slow_response = slow.join().expect("slow thread");
        assert!(slow_response.starts_with("HTTP/1.1 200"));
        service_stop_with_conn(&conn, &json!({ "projectId": project_id })).expect("stop");
    }

    #[test]
    fn api_mock_service_over_limit_returns_503() {
        let conn = test_conn();
        let port = free_port();
        let project = project_create_with_conn(&conn, &json!({ "name": "Demo", "port": port }))
            .expect("project");
        let project_id = project["id"].as_i64().unwrap();
        save_delay_route(&conn, project_id, "/slow", 3000);

        service_start_with_conn(&conn, &json!({ "projectId": project_id })).expect("start");
        let holders: Vec<_> = (0..MAX_CONCURRENT_MOCK_CONNECTIONS)
            .map(|_| {
                thread::spawn(move || {
                    http_request(port, "GET /slow HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
                })
            })
            .collect();
        thread::sleep(Duration::from_millis(500));

        let overflow = http_request(port, "GET /slow HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n");
        assert!(overflow.starts_with("HTTP/1.1 503"), "overflow response: {overflow}");

        let logs = request_logs_with_conn(&conn, &json!({ "projectId": project_id })).expect("logs");
        assert_eq!(logs["logs"][0]["status"], 503);
        assert_eq!(logs["logs"][0]["path"], "/slow");
        assert_eq!(logs["logs"][0]["error"], OVER_LIMIT_LOG_ERROR);

        service_stop_with_conn(&conn, &json!({ "projectId": project_id })).expect("stop");
        for holder in holders {
            let _ = holder.join();
        }
    }

    #[test]
    fn api_mock_service_stop_interrupts_delayed_request() {
        let conn = test_conn();
        let port = free_port();
        let project = project_create_with_conn(&conn, &json!({ "name": "Demo", "port": port }))
            .expect("project");
        let project_id = project["id"].as_i64().unwrap();
        save_delay_route(&conn, project_id, "/slow", 5000);

        service_start_with_conn(&conn, &json!({ "projectId": project_id })).expect("start");
        let pending = thread::spawn(move || {
            http_request(port, "GET /slow HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        });
        thread::sleep(Duration::from_millis(300));

        let started = std::time::Instant::now();
        service_stop_with_conn(&conn, &json!({ "projectId": project_id })).expect("stop");
        let stop_elapsed = started.elapsed();
        assert!(
            stop_elapsed < Duration::from_millis(2500),
            "stop blocked by delayed request: {stop_elapsed:?}"
        );

        let response = pending.join().expect("pending thread");
        assert!(
            !response.contains("HTTP/1.1 200"),
            "delayed request should be dropped without full response: {response}"
        );
    }
}

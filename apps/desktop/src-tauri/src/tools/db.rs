//! 数据库工作台主模块：连接管理、策略拦截（只读/二次确认）、查询编排、
//! 表数据分页与变更应用、导出、SQL 收藏与执行历史。
//!
//! 通道：`tool:db:*` -> domain `db`。异步执行复用静态 tokio Runtime + block_on
//! 模式（与 dns.rs 一致）；连接池与运行中查询在静态注册表中管理。

use std::collections::HashMap;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::{json, Value};

use super::db_drivers::{
    self, sql_text, ConnectConfig, DataFilter, DbPool, GridChange, SessionId, StatementResult,
};
use super::helpers::{db_conn, get_data_dir};
use super::vault;
use rusqlite::params;

const HISTORY_KEEP: i64 = 500;
const DEFAULT_MAX_ROWS: usize = 1000;
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 8;
/// 会话内密码占位符：编辑连接时未改动密码即不发送该字段。
const POOL_KEY_SEP: char = '\u{1}';

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static POOLS: OnceLock<Mutex<HashMap<String, DbPool>>> = OnceLock::new();
static RUNNING: OnceLock<Mutex<HashMap<String, RunningQuery>>> = OnceLock::new();
static DB_KEY: OnceLock<[u8; vault::KEY_LEN]> = OnceLock::new();
static REDIS_CONNS: OnceLock<Mutex<HashMap<String, redis::aio::MultiplexedConnection>>> =
    OnceLock::new();

struct RunningQuery {
    connection_id: String,
    database: String,
    session: SessionId,
}

fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().expect("create db runtime"))
}

fn pools() -> &'static Mutex<HashMap<String, DbPool>> {
    POOLS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn running() -> &'static Mutex<HashMap<String, RunningQuery>> {
    RUNNING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn redis_conns() -> &'static Mutex<HashMap<String, redis::aio::MultiplexedConnection>> {
    REDIS_CONNS.get_or_init(|| Mutex::new(HashMap::new()))
}

const ACTIONS: &[&str] = &[
    "connection_list",
    "connection_save",
    "connection_delete",
    "connection_test",
    "connection_open",
    "connection_close",
    "schema_databases",
    "schema_tables",
    "schema_table_detail",
    "query_execute",
    "query_cancel",
    "table_data_page",
    "table_apply_changes",
    "result_export",
    "saved_query_list",
    "saved_query_save",
    "saved_query_delete",
    "history_list",
    "history_clear",
    "redis_scan",
    "redis_key_detail",
    "redis_key_write",
    "redis_command",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported db action: {action}"));
    }
    match action {
        "connection_list" => connection_list(),
        "connection_save" => connection_save(payload),
        "connection_delete" => connection_delete(payload),
        "connection_test" => connection_test(payload),
        "connection_open" => connection_open(payload),
        "connection_close" => connection_close(payload),
        "schema_databases" => schema_databases(payload),
        "schema_tables" => schema_tables(payload),
        "schema_table_detail" => schema_table_detail(payload),
        "query_execute" => query_execute(payload),
        "query_cancel" => query_cancel(payload),
        "table_data_page" => table_data_page(payload),
        "table_apply_changes" => table_apply_changes(payload),
        "result_export" => result_export(payload),
        "saved_query_list" => saved_query_list(payload),
        "saved_query_save" => saved_query_save(payload),
        "saved_query_delete" => saved_query_delete(payload),
        "history_list" => history_list(payload),
        "history_clear" => history_clear(payload),
        "redis_scan" => redis_scan(payload),
        "redis_key_detail" => redis_key_detail(payload),
        "redis_key_write" => redis_key_write(payload),
        "redis_command" => redis_command(payload),
        _ => Err(format!("unsupported db action: {action}")),
    }
}

// ---------- 密钥与密码加密 ----------

/// 本地密钥文件 <数据目录>/db-key：32 字节随机数 hex 存储，首次使用时生成。
/// 威胁模型是防止直接翻看数据库文件读到明文密码（与设计文档一致），
/// 不承诺抵御拿到完整数据目录的攻击者。
fn db_key() -> Result<[u8; vault::KEY_LEN], String> {
    if let Some(k) = DB_KEY.get() {
        return Ok(*k);
    }
    let path = get_data_dir()?.join("db-key");
    let key: [u8; vault::KEY_LEN] = if path.is_file() {
        let text = std::fs::read_to_string(&path).map_err(|e| format!("读取 db-key 失败: {e}"))?;
        let bytes = hex::decode(text.trim()).map_err(|e| format!("db-key 内容无效: {e}"))?;
        bytes
            .try_into()
            .map_err(|_| "db-key 长度无效，应为 32 字节 hex".to_string())?
    } else {
        let mut buf = [0u8; vault::KEY_LEN];
        openssl::rand::rand_bytes(&mut buf).map_err(|e| format!("生成密钥失败: {e}"))?;
        std::fs::write(&path, hex::encode(buf)).map_err(|e| format!("写入 db-key 失败: {e}"))?;
        buf
    };
    let _ = DB_KEY.set(key);
    Ok(key)
}

/// 密文格式：base64(iv):base64(cipher)，每条独立随机 IV。
fn encrypt_password(plain: &str) -> Result<String, String> {
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;
    let key = db_key()?;
    let mut iv = [0u8; vault::IV_LEN];
    openssl::rand::rand_bytes(&mut iv).map_err(|e| format!("生成 IV 失败: {e}"))?;
    let cipher = vault::aes256_encrypt(&key, &iv, plain.as_bytes())?;
    Ok(format!("{}:{}", B64.encode(iv), B64.encode(cipher)))
}

fn decrypt_password(cipher_text: &str) -> Result<String, String> {
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;
    let key = db_key()?;
    let (iv_b64, ct_b64) = cipher_text
        .split_once(':')
        .ok_or("密文格式无效")?;
    let iv = B64.decode(iv_b64).map_err(|e| format!("密文 IV 无效: {e}"))?;
    let ct = B64.decode(ct_b64).map_err(|e| format!("密文无效: {e}"))?;
    let plain = vault::aes256_decrypt(&key, &iv, &ct)?;
    String::from_utf8(plain).map_err(|_| "密码解密结果非 UTF-8".to_string())
}

// ---------- 连接记录 ----------

#[derive(Debug, Clone)]
struct ConnRecord {
    id: String,
    engine: String,
    host: String,
    port: u16,
    username: String,
    password_cipher: Option<String>,
    default_database: Option<String>,
    env_tag: String,
    read_only: bool,
    options: Value,
}

impl ConnRecord {
    fn opt_u64(&self, key: &str, default: u64) -> u64 {
        self.options.get(key).and_then(|v| v.as_u64()).unwrap_or(default)
    }

    fn max_rows(&self) -> usize {
        self.opt_u64("maxRows", DEFAULT_MAX_ROWS as u64) as usize
    }

    fn timeout_secs(&self) -> u64 {
        self.opt_u64("timeoutSecs", DEFAULT_TIMEOUT_SECS)
    }

    fn dialect(&self) -> sql_text::SqlDialect {
        if self.engine == "mysql" {
            sql_text::SqlDialect::MySql
        } else {
            sql_text::SqlDialect::Pg
        }
    }

    fn connect_config(&self, database: Option<&str>) -> Result<ConnectConfig, String> {
        let password = match &self.password_cipher {
            Some(c) if !c.is_empty() => decrypt_password(c)?,
            _ => String::new(),
        };
        Ok(ConnectConfig {
            engine: self.engine.clone(),
            host: self.host.clone(),
            port: self.port,
            username: self.username.clone(),
            password,
            database: database
                .map(|s| s.to_string())
                .or_else(|| self.default_database.clone()),
            connect_timeout_secs: self.opt_u64("connectTimeoutSecs", DEFAULT_CONNECT_TIMEOUT_SECS),
        })
    }
}

fn load_connection(id: &str) -> Result<ConnRecord, String> {
    let conn = db_conn()?;
    conn.query_row(
        "SELECT id, engine, host, port, username, password_cipher, default_database, \
                env_tag, read_only, options_json \
         FROM db_connections WHERE id = ?1",
        params![id],
        |row| {
            Ok(ConnRecord {
                id: row.get(0)?,
                engine: row.get(1)?,
                host: row.get(2)?,
                port: row.get::<_, i64>(3)? as u16,
                username: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                password_cipher: row.get(5)?,
                default_database: row.get(6)?,
                env_tag: row.get(7)?,
                read_only: row.get::<_, i64>(8)? != 0,
                options: row
                    .get::<_, Option<String>>(9)?
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_else(|| json!({})),
            })
        },
    )
    .map_err(|_| "连接不存在或已被删除".to_string())
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

// ---------- 连接管理 actions ----------

fn connection_list() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, engine, host, port, username, password_cipher IS NOT NULL AND password_cipher != '', \
                    default_database, env_tag, read_only, group_name, sort_order, options_json, last_used_at \
             FROM db_connections ORDER BY group_name IS NULL, group_name, sort_order, name",
        )
        .map_err(|e| format!("查询连接失败: {e}"))?;
    let list: Vec<Value> = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "engine": row.get::<_, String>(2)?,
                "host": row.get::<_, String>(3)?,
                "port": row.get::<_, i64>(4)?,
                "username": row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                "hasPassword": row.get::<_, bool>(6)?,
                "defaultDatabase": row.get::<_, Option<String>>(7)?,
                "envTag": row.get::<_, String>(8)?,
                "readOnly": row.get::<_, i64>(9)? != 0,
                "groupName": row.get::<_, Option<String>>(10)?,
                "sortOrder": row.get::<_, i64>(11)?,
                "options": row.get::<_, Option<String>>(12)?
                    .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                    .unwrap_or_else(|| json!({})),
                "lastUsedAt": row.get::<_, Option<i64>>(13)?,
            }))
        })
        .map_err(|e| format!("查询连接失败: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(json!({ "connections": list }))
}

fn connection_save(payload: &Value) -> Result<Value, String> {
    let name = payload["name"].as_str().map(str::trim).filter(|s| !s.is_empty())
        .ok_or("连接名称不能为空")?;
    let engine = payload["engine"].as_str().ok_or("engine required")?;
    if !matches!(engine, "mysql" | "kingbase" | "redis") {
        return Err(format!("暂不支持的引擎: {engine}"));
    }
    let host = payload["host"].as_str().map(str::trim).filter(|s| !s.is_empty())
        .ok_or("主机不能为空")?;
    let port = payload["port"].as_u64().filter(|p| *p > 0 && *p <= 65535)
        .ok_or("端口无效")? as i64;
    let username = payload["username"].as_str().unwrap_or("").trim().to_string();
    let default_database = payload["defaultDatabase"].as_str().map(str::trim)
        .filter(|s| !s.is_empty()).map(String::from);
    if engine == "kingbase" && default_database.is_none() {
        return Err("KingbaseES 连接必须填写默认数据库".into());
    }
    let env_tag = payload["envTag"].as_str().unwrap_or("dev");
    let read_only = payload["readOnly"].as_bool().unwrap_or(false) as i64;
    let group_name = payload["groupName"].as_str().map(str::trim)
        .filter(|s| !s.is_empty()).map(String::from);
    let sort_order = payload["sortOrder"].as_i64().unwrap_or(0);
    let options_json = payload.get("options").filter(|v| v.is_object())
        .map(|v| v.to_string());
    let now = now_ms();

    let conn = db_conn()?;
    let existing_id = payload["id"].as_str().map(String::from);
    let id = match existing_id {
        Some(id) => {
            // 密码占位符语义：payload 不含 password 字段则保持原密文；
            // 显式传空串则置空；传新值则重新加密。
            if let Some(pw) = payload.get("password") {
                let cipher: Option<String> = match pw.as_str() {
                    Some(s) if !s.is_empty() => Some(encrypt_password(s)?),
                    _ => None,
                };
                conn.execute(
                    "UPDATE db_connections SET password_cipher = ?1 WHERE id = ?2",
                    params![cipher, id],
                )
                .map_err(|e| format!("更新密码失败: {e}"))?;
            }
            let updated = conn
                .execute(
                    "UPDATE db_connections SET name=?1, engine=?2, host=?3, port=?4, username=?5, \
                     default_database=?6, env_tag=?7, read_only=?8, group_name=?9, sort_order=?10, \
                     options_json=?11, updated_at=?12 WHERE id=?13",
                    params![
                        name, engine, host, port, username, default_database, env_tag, read_only,
                        group_name, sort_order, options_json, now, id
                    ],
                )
                .map_err(|e| format!("更新连接失败: {e}"))?;
            if updated == 0 {
                return Err("连接不存在或已被删除".into());
            }
            // 配置变化后旧池失效
            close_pools_of(&id);
            id
        }
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            let cipher: Option<String> = match payload.get("password").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => Some(encrypt_password(s)?),
                _ => None,
            };
            conn.execute(
                "INSERT INTO db_connections (id, name, engine, host, port, username, password_cipher, \
                 default_database, env_tag, read_only, group_name, sort_order, options_json, created_at, updated_at) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?14)",
                params![
                    id, name, engine, host, port, username, cipher, default_database, env_tag,
                    read_only, group_name, sort_order, options_json, now
                ],
            )
            .map_err(|e| format!("创建连接失败: {e}"))?;
            id
        }
    };
    Ok(json!({ "id": id }))
}

fn connection_delete(payload: &Value) -> Result<Value, String> {
    let id = payload["connectionId"].as_str().ok_or("connectionId required")?;
    close_pools_of(id);
    let conn = db_conn()?;
    // 级联：历史同删，连接级收藏转为全局收藏
    conn.execute("DELETE FROM db_query_history WHERE connection_id = ?1", params![id])
        .map_err(|e| format!("清理历史失败: {e}"))?;
    conn.execute(
        "UPDATE db_saved_queries SET connection_id = NULL WHERE connection_id = ?1",
        params![id],
    )
    .map_err(|e| format!("转移收藏失败: {e}"))?;
    let n = conn
        .execute("DELETE FROM db_connections WHERE id = ?1", params![id])
        .map_err(|e| format!("删除连接失败: {e}"))?;
    Ok(json!({ "deleted": n > 0 }))
}

/// 测试连接：优先用传入配置；编辑既有连接未重输密码时，回退已存密文。
fn connection_test(payload: &Value) -> Result<Value, String> {
    let engine = payload["engine"].as_str().ok_or("engine required")?.to_string();
    let host = payload["host"].as_str().ok_or("host required")?.to_string();
    let port = payload["port"].as_u64().ok_or("port required")? as u16;
    let username = payload["username"].as_str().unwrap_or("").to_string();
    let database = payload["defaultDatabase"].as_str().map(String::from);
    let password = match payload.get("password").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => match payload["connectionId"].as_str() {
            Some(id) => {
                let rec = load_connection(id)?;
                match &rec.password_cipher {
                    Some(c) if !c.is_empty() => decrypt_password(c)?,
                    _ => String::new(),
                }
            }
            None => String::new(),
        },
    };
    let cfg = ConnectConfig {
        engine,
        host,
        port,
        username,
        password,
        database,
        connect_timeout_secs: DEFAULT_CONNECT_TIMEOUT_SECS,
    };
    runtime().block_on(async move {
        if cfg.engine == "redis" {
            let db_index = cfg
                .database
                .as_deref()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            let mut conn = db_drivers::redis::connect(&cfg, db_index).await?;
            let version = db_drivers::redis::server_version(&mut conn).await?;
            return Ok(json!({ "ok": true, "serverVersion": version }));
        }
        let pool = db_drivers::make_pool(&cfg).await?;
        let version = pool.server_version().await?;
        pool.close().await;
        Ok(json!({ "ok": true, "serverVersion": version }))
    })
}

fn pool_key(connection_id: &str, database: &str) -> String {
    format!("{connection_id}{POOL_KEY_SEP}{database}")
}

fn close_pools_of(connection_id: &str) {
    let prefix = format!("{connection_id}{POOL_KEY_SEP}");
    let removed: Vec<DbPool> = {
        let mut guard = pools().lock().unwrap_or_else(|e| e.into_inner());
        let keys: Vec<String> = guard
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect();
        keys.iter().filter_map(|k| guard.remove(k)).collect()
    };
    if !removed.is_empty() {
        runtime().block_on(async move {
            for pool in removed {
                pool.close().await;
            }
        });
    }
    redis_conns()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|k, _| !k.starts_with(&prefix));
    running()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|_, rq| rq.connection_id != connection_id);
}

/// 取或建池。database 为空表示连接默认库（MySQL 可无库；KB 落到 default_database）。
fn get_pool(record: &ConnRecord, database: Option<&str>) -> Result<DbPool, String> {
    let db_for_key = database
        .map(String::from)
        .or_else(|| record.default_database.clone())
        .unwrap_or_default();
    let key = pool_key(&record.id, &db_for_key);
    if let Some(pool) = pools().lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return Ok(pool.clone());
    }
    let cfg = record.connect_config(database)?;
    let pool = runtime().block_on(async { db_drivers::make_pool(&cfg).await })?;
    pools()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(key, pool.clone());
    Ok(pool)
}

fn connection_open(payload: &Value) -> Result<Value, String> {
    let id = payload["connectionId"].as_str().ok_or("connectionId required")?;
    let record = load_connection(id)?;
    let (version, databases) = if record.engine == "redis" {
        let mut conn = redis_conn_for(&record, redis_db_index(&record, payload))?;
        runtime().block_on(async {
            let version = super::db_drivers::redis::server_version(&mut conn).await?;
            let count = super::db_drivers::redis::database_count(&mut conn).await;
            let databases: Vec<String> = (0..count).map(|i| i.to_string()).collect();
            Ok::<_, String>((version, databases))
        })?
    } else {
        let pool = get_pool(&record, None)?;
        runtime().block_on(async {
            let version = pool.server_version().await?;
            let databases = pool.list_databases().await?;
            Ok::<_, String>((version, databases))
        })?
    };
    let conn = db_conn()?;
    let _ = conn.execute(
        "UPDATE db_connections SET last_used_at = ?1 WHERE id = ?2",
        params![now_ms(), id],
    );
    Ok(json!({
        "serverVersion": version,
        "databases": databases,
        "defaultDatabase": record.default_database,
    }))
}

fn connection_close(payload: &Value) -> Result<Value, String> {
    let id = payload["connectionId"].as_str().ok_or("connectionId required")?;
    close_pools_of(id);
    Ok(json!({ "closed": true }))
}

// ---------- 结构浏览 actions ----------

fn schema_databases(payload: &Value) -> Result<Value, String> {
    let id = payload["connectionId"].as_str().ok_or("connectionId required")?;
    let record = load_connection(id)?;
    let pool = get_pool(&record, None)?;
    let databases = runtime().block_on(async { pool.list_databases().await })?;
    Ok(json!({ "databases": databases }))
}

fn schema_tables(payload: &Value) -> Result<Value, String> {
    let id = payload["connectionId"].as_str().ok_or("connectionId required")?;
    let database = payload["database"].as_str().ok_or("database required")?;
    let record = load_connection(id)?;
    let pool = get_pool(&record, Some(database))?;
    let tables = runtime().block_on(async { pool.list_tables(database).await })?;
    Ok(json!({ "tables": tables }))
}

fn schema_table_detail(payload: &Value) -> Result<Value, String> {
    let id = payload["connectionId"].as_str().ok_or("connectionId required")?;
    let database = payload["database"].as_str().ok_or("database required")?;
    let table = payload["table"].as_str().ok_or("table required")?;
    let record = load_connection(id)?;
    let pool = get_pool(&record, Some(database))?;
    let detail = runtime().block_on(async { pool.table_detail(database, table).await })?;
    Ok(serde_json::to_value(detail).map_err(|e| format!("序列化失败: {e}"))?)
}

// ---------- 策略层 ----------

/// 汇总需要用户确认的原因；只读拦截直接报错（无确认放行通道）。
fn policy_check(
    record: &ConnRecord,
    infos: &[(String, sql_text::StatementInfo)],
    confirmed: bool,
) -> Result<Option<Value>, String> {
    for (stmt, info) in infos {
        if record.read_only && !info.readonly {
            return Err(format!(
                "只读连接已拦截非只读语句：{}…（如需执行请在连接设置中关闭只读保护）",
                preview(stmt)
            ));
        }
    }
    if confirmed {
        return Ok(None);
    }
    let mut reasons = Vec::new();
    for (idx, (stmt, info)) in infos.iter().enumerate() {
        if record.env_tag == "prod" && (info.dml || info.ddl) {
            reasons.push(json!({
                "kind": "prodWrite",
                "statementIndex": idx,
                "verb": info.verb,
                "preview": preview(stmt),
            }));
        }
        if info.missing_where {
            reasons.push(json!({
                "kind": "missingWhere",
                "statementIndex": idx,
                "verb": info.verb,
                "preview": preview(stmt),
            }));
        }
    }
    if reasons.is_empty() {
        Ok(None)
    } else {
        Ok(Some(json!({ "needsConfirmation": true, "reasons": reasons })))
    }
}

fn preview(stmt: &str) -> String {
    let compact: String = stmt.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = compact.chars().take(80).collect();
    if compact.chars().count() > 80 {
        out.push('…');
    }
    out
}

fn record_history(connection_id: &str, sql: &str, duration_ms: u64, status: &str, rows: i64) {
    if let Ok(conn) = db_conn() {
        let _ = conn.execute(
            "INSERT INTO db_query_history (connection_id, sql, executed_at, duration_ms, status, row_count) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![connection_id, sql, now_ms(), duration_ms as i64, status, rows],
        );
        let _ = conn.execute(
            "DELETE FROM db_query_history WHERE id NOT IN \
             (SELECT id FROM db_query_history ORDER BY id DESC LIMIT ?1)",
            params![HISTORY_KEEP],
        );
    }
}

// ---------- 查询执行 ----------

fn query_execute(payload: &Value) -> Result<Value, String> {
    let id = payload["connectionId"].as_str().ok_or("connectionId required")?;
    let sql = payload["sql"].as_str().ok_or("sql required")?;
    let query_id = payload["queryId"].as_str().ok_or("queryId required")?.to_string();
    let database = payload["database"].as_str();
    let confirmed = payload["confirmed"].as_bool().unwrap_or(false);
    let record = load_connection(id)?;
    let dialect = record.dialect();

    let stmts = sql_text::split_statements(sql, dialect);
    if stmts.is_empty() {
        return Err("没有可执行的语句".into());
    }
    let infos: Vec<(String, sql_text::StatementInfo)> = stmts
        .iter()
        .map(|s| (s.clone(), sql_text::classify_statement(s, dialect)))
        .collect();
    if let Some(need) = policy_check(&record, &infos, confirmed)? {
        return Ok(need);
    }

    let max_rows = payload["maxRows"].as_u64().map(|v| v as usize).unwrap_or(record.max_rows());
    let timeout = Duration::from_secs(record.timeout_secs());
    let pool = get_pool(&record, database)?;
    let connection_id = record.id.clone();
    let db_name = database.unwrap_or_default().to_string();

    let outcome = runtime().block_on(async {
        let mut conn = pool.acquire().await?;
        let session = conn.session_id().await?;
        running().lock().unwrap_or_else(|e| e.into_inner()).insert(
            query_id.clone(),
            RunningQuery {
                connection_id: connection_id.clone(),
                database: db_name.clone(),
                session,
            },
        );

        let mut results: Vec<StatementResult> = Vec::new();
        let mut error: Option<Value> = None;
        for (idx, stmt) in stmts.iter().enumerate() {
            match tokio::time::timeout(timeout, conn.run_statement(stmt, max_rows)).await {
                Ok(Ok(res)) => {
                    record_history(
                        &connection_id,
                        stmt,
                        res.duration_ms,
                        "ok",
                        if res.rows.is_empty() { res.affected as i64 } else { res.rows.len() as i64 },
                    );
                    results.push(res);
                }
                Ok(Err(e)) => {
                    record_history(&connection_id, stmt, 0, "error", 0);
                    error = Some(json!({ "statementIndex": idx, "message": e }));
                    break;
                }
                Err(_) => {
                    record_history(&connection_id, stmt, timeout.as_millis() as u64, "error", 0);
                    let _ = pool.cancel_session(session).await;
                    conn.detach_destroy();
                    running().lock().unwrap_or_else(|e| e.into_inner()).remove(&query_id);
                    return Ok::<_, String>(json!({
                        "results": results,
                        "error": {
                            "statementIndex": idx,
                            "message": format!("执行超时（{} 秒），已尝试取消服务端查询", timeout.as_secs()),
                        },
                    }));
                }
            }
        }
        running().lock().unwrap_or_else(|e| e.into_inner()).remove(&query_id);
        Ok(json!({ "results": results, "error": error }))
    })?;

    Ok(outcome)
}

fn query_cancel(payload: &Value) -> Result<Value, String> {
    let query_id = payload["queryId"].as_str().ok_or("queryId required")?;
    let entry = {
        let guard = running().lock().unwrap_or_else(|e| e.into_inner());
        guard.get(query_id).map(|rq| {
            (rq.connection_id.clone(), rq.database.clone(), rq.session)
        })
    };
    let Some((conn_id, database, session)) = entry else {
        return Ok(json!({ "cancelled": false, "reason": "查询已结束或不存在" }));
    };
    let record = load_connection(&conn_id)?;
    let db_opt = if database.is_empty() { None } else { Some(database.as_str()) };
    let pool = get_pool(&record, db_opt)?;
    runtime().block_on(async { pool.cancel_session(session).await })?;
    Ok(json!({ "cancelled": true }))
}

// ---------- 表数据浏览与变更 ----------

fn parse_filters(payload: &Value) -> Result<Vec<DataFilter>, String> {
    let Some(arr) = payload["filters"].as_array() else {
        return Ok(Vec::new());
    };
    arr.iter()
        .map(|f| {
            Ok(DataFilter {
                column: f["column"].as_str().ok_or("筛选缺少列名")?.to_string(),
                op: f["op"].as_str().ok_or("筛选缺少操作符")?.to_string(),
                value: value_to_opt_string(&f["value"]).unwrap_or_default(),
            })
        })
        .collect()
}

fn table_data_page(payload: &Value) -> Result<Value, String> {
    let id = payload["connectionId"].as_str().ok_or("connectionId required")?;
    let database = payload["database"].as_str().ok_or("database required")?;
    let table = payload["table"].as_str().ok_or("table required")?;
    let page = payload["page"].as_u64().unwrap_or(0);
    let page_size = payload["pageSize"].as_u64().unwrap_or(200).clamp(1, 1000);
    let filters = parse_filters(payload)?;
    let order = payload["orderBy"].as_object().and_then(|o| {
        let col = o.get("column")?.as_str()?.to_string();
        let asc = o.get("ascending").and_then(|v| v.as_bool()).unwrap_or(true);
        Some((col, asc))
    });
    let record = load_connection(id)?;
    let pool = get_pool(&record, Some(database))?;
    let (result, total) = runtime().block_on(async {
        pool.table_data_page(
            database,
            table,
            &filters,
            order.as_ref().map(|(c, a)| (c.as_str(), *a)),
            page,
            page_size,
        )
        .await
    })?;
    Ok(json!({
        "result": result,
        "total": total,
        "page": page,
        "pageSize": page_size,
    }))
}

fn value_to_opt_string(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => Some(v.to_string()),
    }
}

fn parse_changes(payload: &Value) -> Result<Vec<GridChange>, String> {
    let arr = payload["changes"].as_array().ok_or("changes required")?;
    if arr.is_empty() {
        return Err("变更集为空".into());
    }
    arr.iter()
        .map(|c| {
            let change_type = c["type"].as_str().ok_or("变更缺少 type")?.to_string();
            let to_pairs = |v: &Value| -> Vec<(String, Option<String>)> {
                v.as_object()
                    .map(|m| {
                        m.iter()
                            .map(|(k, val)| (k.clone(), value_to_opt_string(val)))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            Ok(GridChange {
                change_type,
                pk: to_pairs(&c["pk"]),
                values: to_pairs(&c["values"]),
            })
        })
        .collect()
}

fn table_apply_changes(payload: &Value) -> Result<Value, String> {
    let id = payload["connectionId"].as_str().ok_or("connectionId required")?;
    let database = payload["database"].as_str().ok_or("database required")?;
    let table = payload["table"].as_str().ok_or("table required")?;
    let confirmed = payload["confirmed"].as_bool().unwrap_or(false);
    let record = load_connection(id)?;
    if record.read_only {
        return Err("只读连接不允许修改数据（如需修改请在连接设置中关闭只读保护）".into());
    }
    let changes = parse_changes(payload)?;
    if record.env_tag == "prod" && !confirmed {
        return Ok(json!({
            "needsConfirmation": true,
            "reasons": [{
                "kind": "prodWrite",
                "statementIndex": 0,
                "verb": "APPLY",
                "preview": format!("对 prod 连接的表 {table} 应用 {} 条变更", changes.len()),
            }],
        }));
    }
    let pool = get_pool(&record, Some(database))?;
    let outcome = runtime().block_on(async { pool.apply_changes(database, table, &changes).await });
    match outcome {
        Ok(applied) => {
            record_history(
                &record.id,
                &format!("-- 表数据编辑: {table}（{} 条变更）", changes.len()),
                0,
                "ok",
                applied.iter().sum::<u64>() as i64,
            );
            Ok(json!({ "ok": true, "applied": applied }))
        }
        Err((idx, msg)) => Ok(json!({
            "ok": false,
            "failedIndex": idx,
            "message": format!("第 {} 条变更失败，已整体回滚：{}", idx + 1, msg),
        })),
    }
}

// ---------- 导出 ----------

fn result_export(payload: &Value) -> Result<Value, String> {
    let id = payload["connectionId"].as_str().ok_or("connectionId required")?;
    let database = payload["database"].as_str();
    let sql = payload["sql"].as_str().ok_or("sql required")?;
    let format = payload["format"].as_str().ok_or("format required")?;
    let output_path = payload["outputPath"].as_str().ok_or("outputPath required")?;
    let query_id = payload["queryId"].as_str().unwrap_or("").to_string();
    let table_name = payload["tableName"].as_str().unwrap_or("exported_table");
    let record = load_connection(id)?;
    let dialect = record.dialect();

    let stmts = sql_text::split_statements(sql, dialect);
    if stmts.len() != 1 {
        return Err("导出仅支持单条查询语句".into());
    }
    let info = sql_text::classify_statement(&stmts[0], dialect);
    if !info.readonly {
        return Err("导出仅接受只读查询语句（防止写语句被重复执行）".into());
    }
    let stmt = stmts[0].clone();

    let pool = get_pool(&record, database)?;
    let connection_id = record.id.clone();
    let db_name = database.unwrap_or_default().to_string();
    let format = format.to_string();
    let output_path = output_path.to_string();
    let table_name = if record.engine == "mysql" {
        sql_text::quote_ident_mysql(table_name)
    } else {
        sql_text::quote_ident_pg(table_name)
    };

    let row_count = runtime().block_on(async {
        let mut conn = pool.acquire().await?;
        if !query_id.is_empty() {
            let session = conn.session_id().await?;
            running().lock().unwrap_or_else(|e| e.into_inner()).insert(
                query_id.clone(),
                RunningQuery {
                    connection_id: connection_id.clone(),
                    database: db_name,
                    session,
                },
            );
        }
        // 导出不设行数上限；上限给一个防御值避免 usize 溢出场景
        let result = conn.run_statement(&stmt, usize::MAX / 2).await;
        if !query_id.is_empty() {
            running().lock().unwrap_or_else(|e| e.into_inner()).remove(&query_id);
        }
        let result = result?;
        write_export(&result, &format, &output_path, &table_name)
            .map(|_| result.rows.len() as u64)
    })?;

    record_history(&record.id, &format!("-- 导出({format}): {}", preview(&stmt)), 0, "ok", row_count as i64);
    Ok(json!({ "rowCount": row_count, "path": output_path }))
}

fn write_export(
    result: &StatementResult,
    format: &str,
    path: &str,
    quoted_table: &str,
) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("创建导出文件失败: {e}"))?;
    let mut w = std::io::BufWriter::new(file);
    let headers: Vec<&str> = result.columns.iter().map(|c| c.name.as_str()).collect();
    match format {
        "csv" => {
            let mut writer = csv::Writer::from_writer(w);
            writer.write_record(&headers).map_err(|e| format!("写入失败: {e}"))?;
            for row in &result.rows {
                let record: Vec<String> = row
                    .iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .collect();
                writer.write_record(&record).map_err(|e| format!("写入失败: {e}"))?;
            }
            writer.flush().map_err(|e| format!("写入失败: {e}"))?;
        }
        "json" => {
            let objects: Vec<Value> = result
                .rows
                .iter()
                .map(|row| {
                    let mut obj = serde_json::Map::new();
                    for (i, col) in result.columns.iter().enumerate() {
                        obj.insert(col.name.clone(), row.get(i).cloned().unwrap_or(Value::Null));
                    }
                    Value::Object(obj)
                })
                .collect();
            serde_json::to_writer_pretty(&mut w, &objects).map_err(|e| format!("写入失败: {e}"))?;
            w.flush().map_err(|e| format!("写入失败: {e}"))?;
        }
        "insert" => {
            let cols = headers.join(", ");
            for row in &result.rows {
                let values: Vec<String> = row
                    .iter()
                    .map(|v| match v {
                        Value::Null => "NULL".to_string(),
                        Value::String(s) => format!("'{}'", s.replace('\'', "''")),
                        other => format!("'{}'", other.to_string().replace('\'', "''")),
                    })
                    .collect();
                writeln!(
                    w,
                    "INSERT INTO {quoted_table} ({cols}) VALUES ({});",
                    values.join(", ")
                )
                .map_err(|e| format!("写入失败: {e}"))?;
            }
            w.flush().map_err(|e| format!("写入失败: {e}"))?;
        }
        other => return Err(format!("不支持的导出格式: {other}")),
    }
    Ok(())
}

// ---------- 收藏与历史 ----------

fn saved_query_list(payload: &Value) -> Result<Value, String> {
    let connection_id = payload["connectionId"].as_str();
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, connection_id, title, sql, updated_at FROM db_saved_queries \
             WHERE connection_id IS NULL OR connection_id = ?1 ORDER BY updated_at DESC",
        )
        .map_err(|e| format!("查询收藏失败: {e}"))?;
    let list: Vec<Value> = stmt
        .query_map(params![connection_id], |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "connectionId": row.get::<_, Option<String>>(1)?,
                "title": row.get::<_, String>(2)?,
                "sql": row.get::<_, String>(3)?,
                "updatedAt": row.get::<_, i64>(4)?,
            }))
        })
        .map_err(|e| format!("查询收藏失败: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(json!({ "queries": list }))
}

fn saved_query_save(payload: &Value) -> Result<Value, String> {
    let title = payload["title"].as_str().map(str::trim).filter(|s| !s.is_empty())
        .ok_or("收藏标题不能为空")?;
    let sql_text_value = payload["sql"].as_str().filter(|s| !s.trim().is_empty())
        .ok_or("SQL 内容不能为空")?;
    let connection_id = payload["connectionId"].as_str();
    let now = now_ms();
    let conn = db_conn()?;
    let id = match payload["id"].as_str() {
        Some(id) => {
            conn.execute(
                "UPDATE db_saved_queries SET title=?1, sql=?2, connection_id=?3, updated_at=?4 WHERE id=?5",
                params![title, sql_text_value, connection_id, now, id],
            )
            .map_err(|e| format!("更新收藏失败: {e}"))?;
            id.to_string()
        }
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO db_saved_queries (id, connection_id, title, sql, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![id, connection_id, title, sql_text_value, now],
            )
            .map_err(|e| format!("创建收藏失败: {e}"))?;
            id
        }
    };
    Ok(json!({ "id": id }))
}

fn saved_query_delete(payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_str().ok_or("id required")?;
    let conn = db_conn()?;
    let n = conn
        .execute("DELETE FROM db_saved_queries WHERE id = ?1", params![id])
        .map_err(|e| format!("删除收藏失败: {e}"))?;
    Ok(json!({ "deleted": n > 0 }))
}

fn history_list(payload: &Value) -> Result<Value, String> {
    let connection_id = payload["connectionId"].as_str();
    let limit = payload["limit"].as_i64().unwrap_or(100).clamp(1, 500);
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, connection_id, sql, executed_at, duration_ms, status, row_count \
             FROM db_query_history WHERE ?1 IS NULL OR connection_id = ?1 \
             ORDER BY id DESC LIMIT ?2",
        )
        .map_err(|e| format!("查询历史失败: {e}"))?;
    let list: Vec<Value> = stmt
        .query_map(params![connection_id, limit], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "connectionId": row.get::<_, String>(1)?,
                "sql": row.get::<_, String>(2)?,
                "executedAt": row.get::<_, i64>(3)?,
                "durationMs": row.get::<_, Option<i64>>(4)?,
                "status": row.get::<_, String>(5)?,
                "rowCount": row.get::<_, Option<i64>>(6)?,
            }))
        })
        .map_err(|e| format!("查询历史失败: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(json!({ "history": list }))
}

fn history_clear(payload: &Value) -> Result<Value, String> {
    let connection_id = payload["connectionId"].as_str();
    let conn = db_conn()?;
    let n = match connection_id {
        Some(id) => conn
            .execute("DELETE FROM db_query_history WHERE connection_id = ?1", params![id])
            .map_err(|e| format!("清空历史失败: {e}"))?,
        None => conn
            .execute("DELETE FROM db_query_history", [])
            .map_err(|e| format!("清空历史失败: {e}"))?,
    };
    Ok(json!({ "cleared": n }))
}

// ---------- Redis（二期） ----------

use super::db_drivers::redis as redis_driver;

fn redis_db_index(record: &ConnRecord, payload: &Value) -> i64 {
    payload["db"]
        .as_i64()
        .or_else(|| payload["db"].as_str().and_then(|s| s.parse().ok()))
        .or_else(|| record.default_database.as_deref().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

/// 取或建 Redis 连接（MultiplexedConnection 可 Clone，按 connectionId+db 缓存）。
fn redis_conn_for(
    record: &ConnRecord,
    db_index: i64,
) -> Result<redis::aio::MultiplexedConnection, String> {
    if record.engine != "redis" {
        return Err("该连接不是 Redis 引擎".into());
    }
    let key = pool_key(&record.id, &db_index.to_string());
    if let Some(conn) = redis_conns().lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return Ok(conn.clone());
    }
    let cfg = record.connect_config(Some(&db_index.to_string()))?;
    let conn = runtime().block_on(async { redis_driver::connect(&cfg, db_index).await })?;
    redis_conns()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(key, conn.clone());
    Ok(conn)
}

fn redis_scan(payload: &Value) -> Result<Value, String> {
    let id = payload["connectionId"].as_str().ok_or("connectionId required")?;
    let record = load_connection(id)?;
    let cursor = payload["cursor"].as_u64().unwrap_or(0);
    let pattern = payload["pattern"].as_str().unwrap_or("").to_string();
    let count = payload["count"].as_u64().unwrap_or(200).clamp(10, 1000) as usize;
    let mut conn = redis_conn_for(&record, redis_db_index(&record, payload))?;
    let (next, keys) = runtime()
        .block_on(async { redis_driver::scan_keys(&mut conn, cursor, &pattern, count).await })?;
    let items: Vec<Value> = keys
        .into_iter()
        .map(|(key, key_type)| json!({ "key": key, "type": key_type }))
        .collect();
    Ok(json!({ "cursor": next, "done": next == 0, "keys": items }))
}

fn redis_key_detail(payload: &Value) -> Result<Value, String> {
    let id = payload["connectionId"].as_str().ok_or("connectionId required")?;
    let key = payload["key"].as_str().ok_or("key required")?;
    let record = load_connection(id)?;
    let mut conn = redis_conn_for(&record, redis_db_index(&record, payload))?;
    runtime().block_on(async { redis_driver::key_detail(&mut conn, key).await })
}

fn redis_key_write(payload: &Value) -> Result<Value, String> {
    let id = payload["connectionId"].as_str().ok_or("connectionId required")?;
    let action = payload["writeAction"].as_str().ok_or("writeAction required")?;
    let key = payload["key"].as_str().ok_or("key required")?;
    let record = load_connection(id)?;
    if record.read_only {
        return Err("只读连接不允许修改数据（如需修改请在连接设置中关闭只读保护）".into());
    }
    let mut conn = redis_conn_for(&record, redis_db_index(&record, payload))?;
    runtime().block_on(async { redis_driver::key_write(&mut conn, action, key, payload).await })
}

fn redis_command(payload: &Value) -> Result<Value, String> {
    let id = payload["connectionId"].as_str().ok_or("connectionId required")?;
    let command = payload["command"].as_str().ok_or("command required")?;
    let confirmed = payload["confirmed"].as_bool().unwrap_or(false);
    let record = load_connection(id)?;

    let args = redis_driver::parse_command_line(command)?;
    let class = redis_driver::classify_command(&args[0]);
    match class {
        redis_driver::CommandClass::Blocking => {
            return Err(format!(
                "命令 {} 属于阻塞/订阅类，控制台不支持长连接语义",
                args[0].to_ascii_uppercase()
            ));
        }
        redis_driver::CommandClass::Dangerous => {
            if record.read_only {
                return Err("只读连接已拦截该命令".into());
            }
            if !confirmed {
                return Ok(json!({
                    "needsConfirmation": true,
                    "reasons": [{
                        "kind": "dangerousCommand",
                        "statementIndex": 0,
                        "verb": args[0].to_ascii_uppercase(),
                        "preview": preview(command),
                    }],
                }));
            }
        }
        redis_driver::CommandClass::Write => {
            if record.read_only {
                return Err(format!(
                    "只读连接已拦截写命令 {}（如需执行请关闭只读保护）",
                    args[0].to_ascii_uppercase()
                ));
            }
        }
        redis_driver::CommandClass::Readonly => {}
    }

    let mut conn = redis_conn_for(&record, redis_db_index(&record, payload))?;
    let started = std::time::Instant::now();
    let result = runtime().block_on(async { redis_driver::run_command(&mut conn, &args).await });
    let duration = started.elapsed().as_millis() as u64;
    match result {
        Ok(value) => {
            record_history(&record.id, &format!("-- redis: {}", preview(command)), duration, "ok", 0);
            Ok(json!({ "result": value, "durationMs": duration }))
        }
        Err(e) => {
            record_history(&record.id, &format!("-- redis: {}", preview(command)), duration, "error", 0);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_compacts_and_truncates() {
        assert_eq!(preview("SELECT  1\n  FROM t"), "SELECT 1 FROM t");
        let long = "x".repeat(120);
        let p = preview(&long);
        assert!(p.chars().count() == 81 && p.ends_with('…'));
    }

    #[test]
    fn value_to_opt_string_variants() {
        assert_eq!(value_to_opt_string(&Value::Null), None);
        assert_eq!(value_to_opt_string(&json!("a")), Some("a".into()));
        assert_eq!(value_to_opt_string(&json!(5)), Some("5".into()));
        assert_eq!(value_to_opt_string(&json!(true)), Some("true".into()));
    }

    #[test]
    fn policy_readonly_blocks_writes() {
        let record = ConnRecord {
            id: "c1".into(),
            engine: "mysql".into(),
            host: "h".into(),
            port: 3306,
            username: "u".into(),
            password_cipher: None,
            default_database: None,
            env_tag: "dev".into(),
            read_only: true,
            options: json!({}),
        };
        let dialect = sql_text::SqlDialect::MySql;
        let infos = vec![(
            "DELETE FROM t".to_string(),
            sql_text::classify_statement("DELETE FROM t", dialect),
        )];
        assert!(policy_check(&record, &infos, false).is_err());
        // 只读语句放行
        let infos = vec![(
            "SELECT 1".to_string(),
            sql_text::classify_statement("SELECT 1", dialect),
        )];
        assert!(policy_check(&record, &infos, false).unwrap().is_none());
    }

    #[test]
    fn policy_prod_and_missing_where_need_confirmation() {
        let mut record = ConnRecord {
            id: "c1".into(),
            engine: "mysql".into(),
            host: "h".into(),
            port: 3306,
            username: "u".into(),
            password_cipher: None,
            default_database: None,
            env_tag: "prod".into(),
            read_only: false,
            options: json!({}),
        };
        let dialect = sql_text::SqlDialect::MySql;
        let infos = vec![(
            "UPDATE t SET a=1".to_string(),
            sql_text::classify_statement("UPDATE t SET a=1", dialect),
        )];
        let need = policy_check(&record, &infos, false).unwrap().unwrap();
        let reasons = need["reasons"].as_array().unwrap();
        assert_eq!(reasons.len(), 2, "prod 写 + 无 WHERE 两条原因");
        // confirmed 后放行
        assert!(policy_check(&record, &infos, true).unwrap().is_none());
        // 非 prod 只剩 missingWhere
        record.env_tag = "dev".into();
        let need = policy_check(&record, &infos, false).unwrap().unwrap();
        assert_eq!(need["reasons"].as_array().unwrap().len(), 1);
    }
}

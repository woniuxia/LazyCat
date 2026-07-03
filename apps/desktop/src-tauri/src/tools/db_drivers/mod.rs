//! 数据库工作台驱动层：统一类型定义与引擎分发。
//!
//! MySQL 走 sqlx MySqlPool，KingbaseES（人大金仓，PG 系）走 sqlx PgPool。
//! 方言差异（系统目录查询、DDL 获取、写回 CAST、取消语句）封装在各引擎模块内，
//! 上层 `db.rs` 只面对本模块导出的统一结构。

pub mod kingbase;
pub mod mysql;
pub mod redis;
pub mod sql_text;

use serde::Serialize;
use serde_json::Value;
use sqlx::pool::PoolConnection;
use sqlx::{MySql, MySqlPool, PgPool, Postgres};

/// 连接配置（已解密密码），由 db.rs 从 SQLite 行构造。
#[derive(Debug, Clone)]
pub struct ConnectConfig {
    pub engine: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: Option<String>,
    pub connect_timeout_secs: u64,
}

/// 结果列元数据。kind 用于前端渲染与可编辑性判断：
/// number / text / datetime / bool / binary / json
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMeta {
    pub name: String,
    pub type_name: String,
    pub kind: String,
}

/// 单条语句执行结果。所有单元格值为 `string | null`，精度敏感类型已字符串化。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatementResult {
    pub sql: String,
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<Value>>,
    pub affected: u64,
    pub truncated: bool,
    pub duration_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableBrief {
    pub name: String,
    pub table_type: String,
    pub comment: String,
    pub row_estimate: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnDetail {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default_value: Option<String>,
    pub comment: String,
    pub primary_key: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexDetail {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    /// PG 系直接给 indexdef 文本；MySQL 为空
    pub definition: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableDetail {
    pub columns: Vec<ColumnDetail>,
    pub indexes: Vec<IndexDetail>,
    pub ddl: String,
}

/// 表数据浏览的筛选条件。操作符白名单在构造 SQL 时校验。
#[derive(Debug, Clone)]
pub struct DataFilter {
    pub column: String,
    pub op: String,
    pub value: String,
}

/// 网格暂存变更集中的一条变更。
#[derive(Debug, Clone)]
pub struct GridChange {
    pub change_type: String, // update | insert | delete
    pub pk: Vec<(String, Option<String>)>,
    pub values: Vec<(String, Option<String>)>,
}

/// 引擎侧会话标识，供取消执行使用。
#[derive(Debug, Clone, Copy)]
pub enum SessionId {
    MySql(u64),
    Pg(i32),
}

/// 连接池的引擎无关包装（sqlx Pool 内部是 Arc，clone 廉价）。
#[derive(Clone)]
pub enum DbPool {
    MySql(MySqlPool),
    Pg(PgPool),
}

/// 从池中取出的专用连接：一次批量执行的多条语句固定走同一物理连接。
pub enum DbConn {
    MySql(PoolConnection<MySql>),
    Pg(PoolConnection<Postgres>),
}

impl DbPool {
    pub async fn acquire(&self) -> Result<DbConn, String> {
        match self {
            DbPool::MySql(p) => Ok(DbConn::MySql(
                p.acquire().await.map_err(|e| format!("获取连接失败: {e}"))?,
            )),
            DbPool::Pg(p) => Ok(DbConn::Pg(
                p.acquire().await.map_err(|e| format!("获取连接失败: {e}"))?,
            )),
        }
    }

    pub async fn close(&self) {
        match self {
            DbPool::MySql(p) => p.close().await,
            DbPool::Pg(p) => p.close().await,
        }
    }

    pub async fn server_version(&self) -> Result<String, String> {
        match self {
            DbPool::MySql(p) => mysql::server_version(p).await,
            DbPool::Pg(p) => kingbase::server_version(p).await,
        }
    }

    pub async fn list_databases(&self) -> Result<Vec<String>, String> {
        match self {
            DbPool::MySql(p) => mysql::list_databases(p).await,
            DbPool::Pg(p) => kingbase::list_databases(p).await,
        }
    }

    pub async fn list_tables(&self, database: &str) -> Result<Vec<TableBrief>, String> {
        match self {
            DbPool::MySql(p) => mysql::list_tables(p, database).await,
            DbPool::Pg(p) => kingbase::list_tables(p).await,
        }
    }

    pub async fn table_detail(&self, database: &str, table: &str) -> Result<TableDetail, String> {
        match self {
            DbPool::MySql(p) => mysql::table_detail(p, database, table).await,
            DbPool::Pg(p) => kingbase::table_detail(p, table).await,
        }
    }

    /// 取消指定会话上正在执行的语句（另取连接发送取消指令）。
    pub async fn cancel_session(&self, session: SessionId) -> Result<(), String> {
        match (self, session) {
            (DbPool::MySql(p), SessionId::MySql(id)) => mysql::cancel_query(p, id).await,
            (DbPool::Pg(p), SessionId::Pg(pid)) => kingbase::cancel_query(p, pid).await,
            _ => Err("会话标识与连接引擎不匹配".into()),
        }
    }

    pub async fn table_data_page(
        &self,
        database: &str,
        table: &str,
        filters: &[DataFilter],
        order_by: Option<(&str, bool)>,
        page: u64,
        page_size: u64,
    ) -> Result<(StatementResult, u64), String> {
        match self {
            DbPool::MySql(p) => {
                mysql::table_data_page(p, database, table, filters, order_by, page, page_size).await
            }
            DbPool::Pg(p) => {
                kingbase::table_data_page(p, table, filters, order_by, page, page_size).await
            }
        }
    }

    /// 单事务应用变更集；返回逐条影响行数。任一条失败（含影响行数为 0 的并发冲突）
    /// 整体回滚，Err 中带失败下标与原因。
    pub async fn apply_changes(
        &self,
        database: &str,
        table: &str,
        changes: &[GridChange],
    ) -> Result<Vec<u64>, (usize, String)> {
        match self {
            DbPool::MySql(p) => mysql::apply_changes(p, database, table, changes).await,
            DbPool::Pg(p) => kingbase::apply_changes(p, table, changes).await,
        }
    }
}

impl DbConn {
    /// 查询当前连接的引擎侧会话标识（执行批量语句前登记，供取消）。
    pub async fn session_id(&mut self) -> Result<SessionId, String> {
        match self {
            DbConn::MySql(c) => mysql::session_id(c).await.map(SessionId::MySql),
            DbConn::Pg(c) => kingbase::session_id(c).await.map(SessionId::Pg),
        }
    }

    /// 在本连接上执行单条语句。SELECT 类返回行（上限 max_rows，超出置 truncated），
    /// 写语句返回影响行数；PG 的 `INSERT … RETURNING` 等两者兼有。
    pub async fn run_statement(&mut self, sql: &str, max_rows: usize) -> Result<StatementResult, String> {
        match self {
            DbConn::MySql(c) => mysql::run_statement(c, sql, max_rows).await,
            DbConn::Pg(c) => kingbase::run_statement(c, sql, max_rows).await,
        }
    }

    /// 超时/取消后连接状态不可信：从池中分离并丢弃，避免脏连接回池。
    pub fn detach_destroy(self) {
        match self {
            DbConn::MySql(c) => drop(c.detach()),
            DbConn::Pg(c) => drop(c.detach()),
        }
    }
}

/// 建池。KingbaseES 走 PG 协议，database 为必填（PG 族连接必须指定库）。
pub async fn make_pool(cfg: &ConnectConfig) -> Result<DbPool, String> {
    match cfg.engine.as_str() {
        "mysql" => mysql::make_pool(cfg).await.map(DbPool::MySql),
        "kingbase" => kingbase::make_pool(cfg).await.map(DbPool::Pg),
        other => Err(format!("暂不支持的引擎: {other}")),
    }
}

/// 类型名 → 前端渲染 kind 的公共映射。
pub fn kind_of_type(type_name: &str) -> &'static str {
    let t = type_name.to_ascii_uppercase();
    let t = t.as_str();
    if t.contains("BOOL") {
        return "bool";
    }
    if t.contains("BLOB")
        || t.contains("BINARY")
        || t == "BYTEA"
        || t.starts_with("BIT")
        || t.contains("GEOMETRY")
    {
        return "binary";
    }
    if t.contains("JSON") {
        return "json";
    }
    if t.contains("DATE") || t.contains("TIME") || t == "YEAR" || t == "INTERVAL" {
        return "datetime";
    }
    if t.contains("INT")
        || t.contains("FLOAT")
        || t.contains("DOUBLE")
        || t.contains("DECIMAL")
        || t.contains("NUMERIC")
        || t.contains("REAL")
        || t == "OID"
        || t.contains("SERIAL")
        || t.contains("MONEY")
    {
        return "number";
    }
    "text"
}

/// 二进制值摘要：短的完整 hex，长的截断显示字节数。
pub fn bytes_summary(bytes: &[u8]) -> String {
    const SHOW: usize = 16;
    if bytes.len() <= SHOW {
        format!("0x{}", hex::encode(bytes))
    } else {
        format!("0x{}… ({} bytes)", hex::encode(&bytes[..SHOW]), bytes.len())
    }
}

/// 校验筛选操作符白名单，返回规范形式。
pub fn normalize_filter_op(op: &str) -> Result<&'static str, String> {
    Ok(match op.trim().to_ascii_uppercase().as_str() {
        "=" => "=",
        "!=" | "<>" => "<>",
        ">" => ">",
        "<" => "<",
        ">=" => ">=",
        "<=" => "<=",
        "LIKE" => "LIKE",
        "NOT LIKE" => "NOT LIKE",
        "IS NULL" => "IS NULL",
        "IS NOT NULL" => "IS NOT NULL",
        other => return Err(format!("不支持的筛选操作符: {other}")),
    })
}

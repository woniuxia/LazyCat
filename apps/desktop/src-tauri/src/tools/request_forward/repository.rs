use rusqlite::{params, types::Type, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::model::{ForwardProtocol, ForwardRule, RuleWriteInput, ValidatedRuleWriteInput};
use super::validation::validate_rule_input;

const RULE_COLUMNS: &str = "id, name, protocol, bind_host, listen_port, target_url, target_host, target_port, capture_http_headers, capture_http_body, auto_start, created_at, updated_at";
const MISSING_RULE_ERROR: &str = "转发规则不存在";
const LOG_RETENTION_LIMIT: i64 = 1000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct StatsDelta {
    pub(crate) event_count: u64,
    pub(crate) upload_bytes: u64,
    pub(crate) download_bytes: u64,
    pub(crate) error_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ForwardStats {
    pub(crate) rule_id: i64,
    pub(crate) event_count: u64,
    pub(crate) upload_bytes: u64,
    pub(crate) download_bytes: u64,
    pub(crate) error_count: u64,
    pub(crate) updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ForwardLogWrite {
    pub(crate) protocol: ForwardProtocol,
    pub(crate) client_addr: Option<String>,
    pub(crate) target_addr: String,
    pub(crate) method: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) status_code: Option<u16>,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) upload_bytes: u64,
    pub(crate) download_bytes: u64,
    pub(crate) request_headers: Option<Vec<(String, String)>>,
    pub(crate) response_headers: Option<Vec<(String, String)>>,
    pub(crate) request_body_preview: Option<String>,
    pub(crate) response_body_preview: Option<String>,
    pub(crate) request_body_truncated: bool,
    pub(crate) response_body_truncated: bool,
    pub(crate) error: Option<String>,
    pub(crate) created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ForwardLog {
    pub(crate) id: i64,
    pub(crate) rule_id: i64,
    pub(crate) protocol: ForwardProtocol,
    pub(crate) client_addr: Option<String>,
    pub(crate) target_addr: String,
    pub(crate) method: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) status_code: Option<u16>,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) upload_bytes: u64,
    pub(crate) download_bytes: u64,
    pub(crate) request_headers: Option<Vec<(String, String)>>,
    pub(crate) response_headers: Option<Vec<(String, String)>>,
    pub(crate) request_body_preview: Option<String>,
    pub(crate) response_body_preview: Option<String>,
    pub(crate) request_body_truncated: bool,
    pub(crate) response_body_truncated: bool,
    pub(crate) error: Option<String>,
    pub(crate) created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ForwardLogPage {
    pub(crate) items: Vec<ForwardLog>,
    pub(crate) total: u64,
    pub(crate) latest_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LogOutcome {
    Success,
    Error,
}

impl LogOutcome {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogQuery {
    pub(crate) rule_id: i64,
    pub(crate) keyword: Option<String>,
    pub(crate) outcome: Option<LogOutcome>,
    pub(crate) method: Option<String>,
    pub(crate) status_code: Option<u16>,
    pub(crate) started_at: Option<String>,
    pub(crate) ended_at: Option<String>,
    pub(crate) offset: usize,
    pub(crate) limit: usize,
}

pub(crate) fn list_with_conn(conn: &Connection) -> Result<Vec<ForwardRule>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {RULE_COLUMNS} FROM request_forward_rules ORDER BY id ASC"
        ))
        .map_err(|e| format!("查询转发规则失败: {e}"))?;
    let rows = stmt
        .query_map([], rule_from_row)
        .map_err(|e| format!("查询转发规则失败: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取转发规则失败: {e}"))
}

pub(crate) fn get_with_conn(conn: &Connection, id: i64) -> Result<ForwardRule, String> {
    conn.query_row(
        &format!("SELECT {RULE_COLUMNS} FROM request_forward_rules WHERE id = ?1"),
        [id],
        rule_from_row,
    )
    .optional()
    .map_err(|e| format!("读取转发规则失败: {e}"))?
    .ok_or_else(|| MISSING_RULE_ERROR.into())
}

pub(crate) fn create_with_conn(
    conn: &mut Connection,
    input: RuleWriteInput,
) -> Result<ForwardRule, String> {
    let input = validate_rule_input(input)?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("创建转发规则失败: {e}"))?;
    insert_rule(&tx, &input)?;
    let id = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO request_forward_stats(rule_id, updated_at) VALUES (?1, CURRENT_TIMESTAMP)",
        [id],
    )
    .map_err(|e| format!("创建转发规则统计失败: {e}"))?;
    tx.commit().map_err(|e| format!("创建转发规则失败: {e}"))?;
    get_with_conn(conn, id)
}

pub(crate) fn create_many_with_conn(
    conn: &mut Connection,
    inputs: Vec<RuleWriteInput>,
) -> Result<Vec<ForwardRule>, String> {
    let inputs = inputs
        .into_iter()
        .enumerate()
        .map(|(index, input)| {
            validate_rule_input(input)
                .map_err(|error| format!("规则包第 {} 条规则无效: {error}", index + 1))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let tx = conn
        .transaction()
        .map_err(|error| format!("导入转发规则失败: {error}"))?;
    let mut ids = Vec::with_capacity(inputs.len());
    for input in &inputs {
        insert_rule(&tx, input)?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO request_forward_stats(rule_id, updated_at) VALUES (?1, CURRENT_TIMESTAMP)",
            [id],
        )
        .map_err(|error| format!("创建转发规则统计失败: {error}"))?;
        ids.push(id);
    }
    tx.commit()
        .map_err(|error| format!("导入转发规则失败: {error}"))?;

    ids.into_iter().map(|id| get_with_conn(conn, id)).collect()
}

pub(crate) fn update_with_conn(
    conn: &Connection,
    id: i64,
    input: RuleWriteInput,
) -> Result<ForwardRule, String> {
    let existing = get_with_conn(conn, id)?;
    if existing.protocol != input.protocol {
        return Err("已保存规则不能修改协议，请新建规则".into());
    }
    let input = validate_rule_input(input)?;
    conn.execute(
        "UPDATE request_forward_rules
         SET name = ?1,
             protocol = ?2,
             bind_host = ?3,
             listen_port = ?4,
             target_url = ?5,
             target_host = ?6,
             target_port = ?7,
             capture_http_headers = ?8,
             capture_http_body = ?9,
             updated_at = CURRENT_TIMESTAMP
         WHERE id = ?10",
        params![
            input.name,
            input.protocol.as_str(),
            input.bind_host,
            input.listen_port,
            input.target_url,
            input.target_host,
            input.target_port,
            bool_to_db(input.capture_http_headers),
            bool_to_db(input.capture_http_body),
            id,
        ],
    )
    .map_err(|e| format!("更新转发规则失败: {e}"))?;
    get_with_conn(conn, id)
}

pub(crate) fn delete_with_conn(conn: &Connection, id: i64) -> Result<(), String> {
    get_with_conn(conn, id)?;
    conn.execute("DELETE FROM request_forward_rules WHERE id = ?1", [id])
        .map_err(|e| format!("删除转发规则失败: {e}"))?;
    Ok(())
}

pub(crate) fn set_auto_start_with_conn(
    conn: &Connection,
    id: i64,
    auto_start: bool,
) -> Result<(), String> {
    let changed = conn
        .execute(
            "UPDATE request_forward_rules
             SET auto_start = ?1, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?2",
            params![bool_to_db(auto_start), id],
        )
        .map_err(|e| format!("更新转发规则启动意图失败: {e}"))?;
    if changed == 0 {
        return Err(MISSING_RULE_ERROR.into());
    }
    Ok(())
}

pub(crate) fn persist_observability_with_conn(
    conn: &mut Connection,
    rule_id: i64,
    delta: StatsDelta,
    logs: &[ForwardLogWrite],
) -> Result<(), String> {
    get_with_conn(conn, rule_id)?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("持久化转发观测数据失败: {e}"))?;
    tx.execute(
        "INSERT INTO request_forward_stats
         (rule_id, event_count, upload_bytes, download_bytes, error_count, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)
         ON CONFLICT(rule_id) DO UPDATE SET
             event_count = event_count + excluded.event_count,
             upload_bytes = upload_bytes + excluded.upload_bytes,
             download_bytes = download_bytes + excluded.download_bytes,
             error_count = error_count + excluded.error_count,
             updated_at = CURRENT_TIMESTAMP",
        params![
            rule_id,
            sqlite_integer(delta.event_count, "事件计数")?,
            sqlite_integer(delta.upload_bytes, "上传字节数")?,
            sqlite_integer(delta.download_bytes, "下载字节数")?,
            sqlite_integer(delta.error_count, "错误计数")?,
        ],
    )
    .map_err(|e| format!("持久化转发统计失败: {e}"))?;

    for log in logs {
        let request_headers_json = serialize_headers(&log.request_headers)?;
        let response_headers_json = serialize_headers(&log.response_headers)?;
        tx.execute(
            "INSERT INTO request_forward_logs
             (rule_id, protocol, client_addr, target_addr, method, path, status_code,
              duration_ms, upload_bytes, download_bytes, request_headers_json,
              response_headers_json, request_body_preview, response_body_preview,
              request_body_truncated, response_body_truncated, error, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15, ?16, ?17, ?18)",
            params![
                rule_id,
                log.protocol.as_str(),
                log.client_addr,
                log.target_addr,
                log.method,
                log.path,
                log.status_code.map(i64::from),
                log.duration_ms
                    .map(|value| sqlite_integer(value, "持续时间"))
                    .transpose()?,
                sqlite_integer(log.upload_bytes, "日志上传字节数")?,
                sqlite_integer(log.download_bytes, "日志下载字节数")?,
                request_headers_json,
                response_headers_json,
                log.request_body_preview,
                log.response_body_preview,
                i64::from(log.request_body_truncated),
                i64::from(log.response_body_truncated),
                log.error,
                log.created_at,
            ],
        )
        .map_err(|e| format!("持久化转发日志失败: {e}"))?;
    }

    if !logs.is_empty() {
        tx.execute(
            "DELETE FROM request_forward_logs
             WHERE rule_id = ?1
               AND id NOT IN (
                   SELECT id FROM request_forward_logs
                   WHERE rule_id = ?1
                   ORDER BY created_at DESC, id DESC
                   LIMIT ?2
               )",
            params![rule_id, LOG_RETENTION_LIMIT],
        )
        .map_err(|e| format!("清理旧转发日志失败: {e}"))?;
    }

    tx.commit()
        .map_err(|e| format!("提交转发观测数据失败: {e}"))
}

pub(crate) fn get_stats_with_conn(conn: &Connection, rule_id: i64) -> Result<ForwardStats, String> {
    get_with_conn(conn, rule_id)?;
    conn.query_row(
        "SELECT rule_id, event_count, upload_bytes, download_bytes, error_count, updated_at
         FROM request_forward_stats WHERE rule_id = ?1",
        [rule_id],
        stats_from_row,
    )
    .optional()
    .map_err(|e| format!("查询转发统计失败: {e}"))?
    .ok_or_else(|| "转发规则统计不存在".into())
}

pub(crate) fn reset_stats_with_conn(conn: &Connection, rule_id: i64) -> Result<(), String> {
    get_with_conn(conn, rule_id)?;
    conn.execute(
        "UPDATE request_forward_stats
         SET event_count = 0, upload_bytes = 0, download_bytes = 0,
             error_count = 0, updated_at = CURRENT_TIMESTAMP
         WHERE rule_id = ?1",
        [rule_id],
    )
    .map_err(|e| format!("重置转发统计失败: {e}"))?;
    Ok(())
}

pub(crate) fn clear_logs_with_conn(conn: &Connection, rule_id: i64) -> Result<(), String> {
    get_with_conn(conn, rule_id)?;
    conn.execute(
        "DELETE FROM request_forward_logs WHERE rule_id = ?1",
        [rule_id],
    )
    .map_err(|e| format!("清空转发日志失败: {e}"))?;
    Ok(())
}

pub(crate) fn list_logs_with_conn(
    conn: &Connection,
    query: &LogQuery,
) -> Result<ForwardLogPage, String> {
    get_with_conn(conn, query.rule_id)?;
    let keyword = query
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let outcome = query.outcome.map(LogOutcome::as_str);
    let method = query
        .method
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let status_code = query.status_code.map(i64::from);
    let filter = "rule_id = ?1
        AND (?2 IS NULL OR client_addr LIKE '%' || ?2 || '%'
             OR target_addr LIKE '%' || ?2 || '%'
             OR method LIKE '%' || ?2 || '%'
             OR path LIKE '%' || ?2 || '%'
             OR CAST(status_code AS TEXT) LIKE '%' || ?2 || '%'
             OR error LIKE '%' || ?2 || '%')
        AND (?3 IS NULL
             OR (?3 = 'success' AND error IS NULL)
             OR (?3 = 'error' AND error IS NOT NULL))
        AND (?4 IS NULL OR method = ?4)
        AND (?5 IS NULL OR status_code = ?5)
        AND (?6 IS NULL OR created_at >= ?6)
        AND (?7 IS NULL OR created_at <= ?7)";
    let total = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM request_forward_logs WHERE {filter}"),
            params![
                query.rule_id,
                keyword,
                outcome,
                method,
                status_code,
                query.started_at,
                query.ended_at,
            ],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| format!("统计转发日志失败: {e}"))?;
    let latest_id = conn
        .query_row(
            &format!("SELECT MAX(id) FROM request_forward_logs WHERE {filter}"),
            params![
                query.rule_id,
                keyword,
                outcome,
                method,
                status_code,
                query.started_at,
                query.ended_at,
            ],
            |row| row.get::<_, Option<i64>>(0),
        )
        .map_err(|e| format!("查询最新转发日志失败: {e}"))?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT id, rule_id, protocol, client_addr, target_addr, method, path,
                    status_code, duration_ms, upload_bytes, download_bytes,
                    request_headers_json, response_headers_json, request_body_preview,
                    response_body_preview, request_body_truncated, response_body_truncated,
                    error, created_at
             FROM request_forward_logs
             WHERE {filter}
             ORDER BY created_at DESC, id DESC
             LIMIT ?8 OFFSET ?9"
        ))
        .map_err(|e| format!("查询转发日志失败: {e}"))?;
    let rows = stmt
        .query_map(
            params![
                query.rule_id,
                keyword,
                outcome,
                method,
                status_code,
                query.started_at,
                query.ended_at,
                sqlite_integer(query.limit as u64, "分页大小")?,
                sqlite_integer(query.offset as u64, "分页偏移")?,
            ],
            log_from_row,
        )
        .map_err(|e| format!("查询转发日志失败: {e}"))?;
    let items = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取转发日志失败: {e}"))?;
    Ok(ForwardLogPage {
        items,
        total: total as u64,
        latest_id,
    })
}

fn serialize_headers(headers: &Option<Vec<(String, String)>>) -> Result<Option<String>, String> {
    headers
        .as_ref()
        .map(|headers| {
            serde_json::to_string(headers).map_err(|e| format!("序列化转发日志请求头失败: {e}"))
        })
        .transpose()
}

fn deserialize_headers(
    value: Option<String>,
    column: usize,
) -> rusqlite::Result<Option<Vec<(String, String)>>> {
    value
        .map(|value| {
            serde_json::from_str(&value).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(column, Type::Text, Box::new(error))
            })
        })
        .transpose()
}

fn sqlite_integer(value: u64, field: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{field}超出 SQLite 整数范围"))
}

fn unsigned_from_row(row: &Row<'_>, column: usize) -> rusqlite::Result<u64> {
    let value = row.get::<_, i64>(column)?;
    u64::try_from(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(column, Type::Integer, Box::new(error))
    })
}

fn stats_from_row(row: &Row<'_>) -> rusqlite::Result<ForwardStats> {
    Ok(ForwardStats {
        rule_id: row.get(0)?,
        event_count: unsigned_from_row(row, 1)?,
        upload_bytes: unsigned_from_row(row, 2)?,
        download_bytes: unsigned_from_row(row, 3)?,
        error_count: unsigned_from_row(row, 4)?,
        updated_at: row.get(5)?,
    })
}

fn log_from_row(row: &Row<'_>) -> rusqlite::Result<ForwardLog> {
    let protocol_value: String = row.get(2)?;
    let protocol = ForwardProtocol::from_db(&protocol_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            Type::Text,
            format!("未知转发协议：{protocol_value}").into(),
        )
    })?;
    Ok(ForwardLog {
        id: row.get(0)?,
        rule_id: row.get(1)?,
        protocol,
        client_addr: row.get(3)?,
        target_addr: row.get(4)?,
        method: row.get(5)?,
        path: row.get(6)?,
        status_code: row
            .get::<_, Option<i64>>(7)?
            .map(|value| {
                u16::try_from(value).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(7, Type::Integer, Box::new(error))
                })
            })
            .transpose()?,
        duration_ms: row
            .get::<_, Option<i64>>(8)?
            .map(|value| {
                u64::try_from(value).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(8, Type::Integer, Box::new(error))
                })
            })
            .transpose()?,
        upload_bytes: unsigned_from_row(row, 9)?,
        download_bytes: unsigned_from_row(row, 10)?,
        request_headers: deserialize_headers(row.get(11)?, 11)?,
        response_headers: deserialize_headers(row.get(12)?, 12)?,
        request_body_preview: row.get(13)?,
        response_body_preview: row.get(14)?,
        request_body_truncated: row.get::<_, i64>(15)? != 0,
        response_body_truncated: row.get::<_, i64>(16)? != 0,
        error: row.get(17)?,
        created_at: row.get(18)?,
    })
}

fn insert_rule(
    tx: &rusqlite::Transaction<'_>,
    input: &ValidatedRuleWriteInput,
) -> Result<(), String> {
    tx.execute(
        "INSERT INTO request_forward_rules
         (name, protocol, bind_host, listen_port, target_url, target_host, target_port,
          capture_http_headers, capture_http_body, auto_start, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        params![
            input.name,
            input.protocol.as_str(),
            input.bind_host,
            input.listen_port,
            input.target_url,
            input.target_host,
            input.target_port,
            bool_to_db(input.capture_http_headers),
            bool_to_db(input.capture_http_body),
        ],
    )
    .map_err(|e| format!("创建转发规则失败: {e}"))?;
    Ok(())
}

fn bool_to_db(value: bool) -> i64 {
    i64::from(value)
}

fn rule_from_row(row: &Row<'_>) -> rusqlite::Result<ForwardRule> {
    let protocol_value: String = row.get(2)?;
    let protocol = ForwardProtocol::from_db(&protocol_value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            Type::Text,
            format!("未知转发协议：{protocol_value}").into(),
        )
    })?;
    Ok(ForwardRule {
        id: row.get(0)?,
        name: row.get(1)?,
        protocol,
        bind_host: row.get(3)?,
        listen_port: row.get(4)?,
        target_url: row.get(5)?,
        target_host: row.get(6)?,
        target_port: row.get(7)?,
        capture_http_headers: row.get::<_, i64>(8)? != 0,
        capture_http_body: row.get::<_, i64>(9)? != 0,
        auto_start: row.get::<_, i64>(10)? != 0,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{
        clear_logs_with_conn, create_many_with_conn, get_stats_with_conn, list_logs_with_conn,
        persist_observability_with_conn, reset_stats_with_conn, ForwardLogWrite, LogOutcome,
        LogQuery, StatsDelta,
    };
    use crate::tools::helpers::ensure_request_forward_schema_for_test;
    use crate::tools::request_forward::model::{ForwardProtocol, RuleWriteInput};

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .expect("enable foreign keys");
        ensure_request_forward_schema_for_test(&conn).expect("create request-forward schema");
        conn
    }

    fn create_rule(conn: &mut Connection, name: &str) -> i64 {
        super::create_with_conn(
            conn,
            RuleWriteInput {
                name: name.into(),
                protocol: ForwardProtocol::Http,
                bind_host: "127.0.0.1".into(),
                listen_port: 8080,
                target_url: Some("http://example.com/api".into()),
                target_host: None,
                target_port: None,
                capture_http_headers: true,
                capture_http_body: true,
            },
        )
        .expect("create rule")
        .id
    }

    #[test]
    fn bulk_create_validates_every_rule_before_committing() {
        let mut conn = test_conn();
        let valid = RuleWriteInput {
            name: "HTTP API".into(),
            protocol: ForwardProtocol::Http,
            bind_host: "127.0.0.1".into(),
            listen_port: 8080,
            target_url: Some("http://example.com/api".into()),
            target_host: None,
            target_port: None,
            capture_http_headers: true,
            capture_http_body: true,
        };
        let mut invalid = valid.clone();
        invalid.name = " ".into();

        let error = create_many_with_conn(&mut conn, vec![valid.clone(), invalid])
            .expect_err("invalid bundle must not be partially imported");
        assert!(error.contains("第 2 条"));
        assert!(super::list_with_conn(&conn).unwrap().is_empty());

        let mut tcp = valid;
        tcp.name = "TCP 数据库".into();
        tcp.protocol = ForwardProtocol::Tcp;
        tcp.listen_port = 5432;
        tcp.target_url = None;
        tcp.target_host = Some("db.internal".into());
        tcp.target_port = Some(5432);
        let imported = create_many_with_conn(&mut conn, vec![tcp]).unwrap();
        assert_eq!(imported.len(), 1);
        assert!(!imported[0].auto_start);
        assert_eq!(
            get_stats_with_conn(&conn, imported[0].id)
                .unwrap()
                .event_count,
            0
        );
    }

    fn http_log(index: usize, error: Option<&str>) -> ForwardLogWrite {
        ForwardLogWrite {
            protocol: ForwardProtocol::Http,
            client_addr: Some(format!("127.0.0.1:{}", 10_000 + index)),
            target_addr: "example.com:80".into(),
            method: Some("GET".into()),
            path: Some(format!("/items/{index}")),
            status_code: error.is_none().then_some(200),
            duration_ms: Some(index as u64),
            upload_bytes: index as u64,
            download_bytes: (index * 2) as u64,
            request_headers: Some(vec![
                ("authorization".into(), "Bearer secret".into()),
                ("x-request-id".into(), format!("request-{index}")),
            ]),
            response_headers: Some(vec![("set-cookie".into(), "session=secret".into())]),
            request_body_preview: Some(format!("request {index}")),
            response_body_preview: Some(format!("response {index}")),
            request_body_truncated: index == 64,
            response_body_truncated: index == 65,
            error: error.map(str::to_string),
            created_at: format!("2026-07-15 00:{:02}:{:02}", (index / 60) % 60, index % 60),
        }
    }

    fn log_query(rule_id: i64) -> LogQuery {
        LogQuery {
            rule_id,
            keyword: None,
            outcome: None,
            method: None,
            status_code: None,
            started_at: None,
            ended_at: None,
            offset: 0,
            limit: 100,
        }
    }

    #[test]
    fn log_insert_keeps_latest_1000_rows_per_rule() {
        let mut conn = test_conn();
        let first_rule = create_rule(&mut conn, "first");
        let second_rule = create_rule(&mut conn, "second");
        let first_logs = (0..1005)
            .map(|index| http_log(index, None))
            .collect::<Vec<_>>();
        let second_logs = vec![http_log(2000, None), http_log(2001, None)];

        persist_observability_with_conn(&mut conn, first_rule, StatsDelta::default(), &first_logs)
            .expect("persist first rule logs");
        persist_observability_with_conn(
            &mut conn,
            second_rule,
            StatsDelta::default(),
            &second_logs,
        )
        .expect("persist second rule logs");

        let first_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM request_forward_logs WHERE rule_id = ?1",
                [first_rule],
                |row| row.get(0),
            )
            .expect("count first logs");
        let oldest_path: String = conn
            .query_row(
                "SELECT path FROM request_forward_logs WHERE rule_id = ?1 ORDER BY id ASC LIMIT 1",
                [first_rule],
                |row| row.get(0),
            )
            .expect("read oldest retained log");
        let second_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM request_forward_logs WHERE rule_id = ?1",
                [second_rule],
                |row| row.get(0),
            )
            .expect("count second logs");

        assert_eq!(first_count, 1000);
        assert_eq!(oldest_path, "/items/5");
        assert_eq!(second_count, 2);
    }

    #[test]
    fn log_clear_does_not_reset_stats_and_stats_reset_does_not_clear_logs() {
        let mut conn = test_conn();
        let rule_id = create_rule(&mut conn, "independent reset");
        let delta = StatsDelta {
            event_count: 3,
            upload_bytes: 10,
            download_bytes: 20,
            error_count: 1,
        };
        persist_observability_with_conn(&mut conn, rule_id, delta, &[http_log(1, None)])
            .expect("persist stats and log");

        clear_logs_with_conn(&conn, rule_id).expect("clear logs");
        let stats = get_stats_with_conn(&conn, rule_id).unwrap();
        assert_eq!(stats.event_count, delta.event_count);
        assert_eq!(stats.upload_bytes, delta.upload_bytes);
        assert_eq!(stats.download_bytes, delta.download_bytes);
        assert_eq!(stats.error_count, delta.error_count);

        persist_observability_with_conn(
            &mut conn,
            rule_id,
            StatsDelta::default(),
            &[http_log(2, None)],
        )
        .expect("persist replacement log");
        reset_stats_with_conn(&conn, rule_id).expect("reset stats");

        let reset = get_stats_with_conn(&conn, rule_id).unwrap();
        assert_eq!(reset.event_count, 0);
        assert_eq!(reset.upload_bytes, 0);
        assert_eq!(reset.download_bytes, 0);
        assert_eq!(reset.error_count, 0);
        let log_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM request_forward_logs WHERE rule_id = ?1",
                [rule_id],
                |row| row.get(0),
            )
            .expect("count remaining logs");
        assert_eq!(log_count, 1);
    }

    #[test]
    fn log_query_filters_before_stable_pagination() {
        let mut conn = test_conn();
        let rule_id = create_rule(&mut conn, "query");
        let logs = vec![
            http_log(1, None),
            http_log(2, Some("connection reset")),
            http_log(3, None),
            http_log(4, Some("connection refused")),
            http_log(5, Some("timeout")),
        ];
        persist_observability_with_conn(&mut conn, rule_id, StatsDelta::default(), &logs)
            .expect("persist query logs");

        let page = list_logs_with_conn(
            &conn,
            &LogQuery {
                keyword: Some("connection".into()),
                outcome: Some(LogOutcome::Error),
                offset: 1,
                limit: 1,
                ..log_query(rule_id)
            },
        )
        .expect("query filtered page");

        assert_eq!(page.total, 2);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].path.as_deref(), Some("/items/2"));
        assert_eq!(page.items[0].error.as_deref(), Some("connection reset"));
        assert_eq!(page.latest_id, Some(4));
    }

    #[test]
    fn log_query_combines_http_fields_time_boundaries_and_filtered_total() {
        let mut conn = test_conn();
        let rule_id = create_rule(&mut conn, "filtered query");
        let mut first = http_log(1, None);
        first.created_at = "2026-07-15 08:00:00.000".into();
        let mut second = http_log(2, None);
        second.method = Some("POST".into());
        second.status_code = Some(201);
        second.created_at = "2026-07-15 08:00:01.000".into();
        let mut third = http_log(3, None);
        third.created_at = "2026-07-15 08:00:02.000".into();
        let mut fourth = http_log(4, None);
        fourth.method = Some("PATCH".into());
        fourth.status_code = Some(204);
        fourth.created_at = "2026-07-15 08:00:02.000".into();
        persist_observability_with_conn(
            &mut conn,
            rule_id,
            StatsDelta::default(),
            &[first, second, third, fourth],
        )
        .expect("persist filter fixtures");

        let method_page = list_logs_with_conn(
            &conn,
            &LogQuery {
                method: Some("GET".into()),
                ..log_query(rule_id)
            },
        )
        .expect("filter method");
        assert_eq!(method_page.total, 2);
        assert_eq!(
            method_page.items.iter().map(|log| log.id).collect::<Vec<_>>(),
            vec![3, 1]
        );

        let status_page = list_logs_with_conn(
            &conn,
            &LogQuery {
                status_code: Some(201),
                ..log_query(rule_id)
            },
        )
        .expect("filter status");
        assert_eq!(status_page.total, 1);
        assert_eq!(status_page.items[0].id, 2);

        let time_page = list_logs_with_conn(
            &conn,
            &LogQuery {
                started_at: Some("2026-07-15 08:00:01.000".into()),
                ended_at: Some("2026-07-15 08:00:02.000".into()),
                offset: 1,
                limit: 2,
                ..log_query(rule_id)
            },
        )
        .expect("filter inclusive time range before pagination");
        assert_eq!(time_page.total, 3);
        assert_eq!(
            time_page.items.iter().map(|log| log.id).collect::<Vec<_>>(),
            vec![3, 2],
            "equal timestamps must use id DESC before pagination"
        );
        assert_eq!(time_page.latest_id, Some(4));

        let combined_page = list_logs_with_conn(
            &conn,
            &LogQuery {
                method: Some("GET".into()),
                status_code: Some(200),
                started_at: Some("2026-07-15 08:00:02.000".into()),
                ended_at: Some("2026-07-15 08:00:02.000".into()),
                ..log_query(rule_id)
            },
        )
        .expect("combine filters");
        assert_eq!(combined_page.total, 1);
        assert_eq!(combined_page.items[0].id, 3);
        assert_eq!(combined_page.latest_id, Some(3));
    }

    #[test]
    fn tcp_and_udp_logs_do_not_match_http_only_filters() {
        let mut conn = test_conn();
        let rule_id = create_rule(&mut conn, "non-http logs");
        let logs = [ForwardProtocol::Tcp, ForwardProtocol::Udp]
            .into_iter()
            .enumerate()
            .map(|(index, protocol)| ForwardLogWrite {
                protocol,
                client_addr: Some("127.0.0.1:12345".into()),
                target_addr: "127.0.0.1:9000".into(),
                method: None,
                path: None,
                status_code: None,
                duration_ms: Some(1),
                upload_bytes: 1,
                download_bytes: 1,
                request_headers: None,
                response_headers: None,
                request_body_preview: None,
                response_body_preview: None,
                request_body_truncated: false,
                response_body_truncated: false,
                error: None,
                created_at: format!("2026-07-15 09:00:0{index}.000"),
            })
            .collect::<Vec<_>>();
        persist_observability_with_conn(&mut conn, rule_id, StatsDelta::default(), &logs)
            .expect("persist TCP and UDP logs");

        for query in [
            LogQuery {
                method: Some("GET".into()),
                ..log_query(rule_id)
            },
            LogQuery {
                status_code: Some(200),
                ..log_query(rule_id)
            },
        ] {
            let page = list_logs_with_conn(&conn, &query).expect("filter non-HTTP logs");
            assert_eq!(page.total, 0);
            assert!(page.items.is_empty());
            assert_eq!(page.latest_id, None);
        }
    }

    #[test]
    fn http_sensitive_headers_and_preview_truncation_persist() {
        let mut conn = test_conn();
        let rule_id = create_rule(&mut conn, "http capture");
        persist_observability_with_conn(
            &mut conn,
            rule_id,
            StatsDelta::default(),
            &[http_log(64, None), http_log(65, None)],
        )
        .expect("persist HTTP logs");

        let stored: (String, String, i64, i64) = conn
            .query_row(
                "SELECT request_headers_json, response_headers_json,
                        request_body_truncated, response_body_truncated
                 FROM request_forward_logs WHERE rule_id = ?1 ORDER BY id ASC LIMIT 1",
                [rule_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read persisted capture");

        assert!(stored.0.contains("Bearer secret"));
        assert!(stored.1.contains("session=secret"));
        assert_eq!(stored.2, 1);
        assert_eq!(stored.3, 0);
    }
}

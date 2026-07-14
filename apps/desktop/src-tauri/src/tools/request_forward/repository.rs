use rusqlite::{params, types::Type, Connection, OptionalExtension, Row};

use super::model::{ForwardProtocol, ForwardRule, RuleWriteInput, ValidatedRuleWriteInput};
use super::validation::validate_rule_input;

const RULE_COLUMNS: &str = "id, name, protocol, bind_host, listen_port, target_url, target_host, target_port, capture_http_headers, capture_http_body, auto_start, created_at, updated_at";
const MISSING_RULE_ERROR: &str = "转发规则不存在";

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

pub(crate) fn update_with_conn(
    conn: &Connection,
    id: i64,
    input: RuleWriteInput,
) -> Result<ForwardRule, String> {
    get_with_conn(conn, id)?;
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

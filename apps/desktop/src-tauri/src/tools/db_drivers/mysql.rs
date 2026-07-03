//! MySQL 驱动：连接、系统目录查询、语句执行与解码、表数据分页、变更应用、取消。

use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde_json::Value;
use sqlx::mysql::{MySqlColumn, MySqlConnectOptions, MySqlPool, MySqlPoolOptions, MySqlRow};
use sqlx::{Column, Either, Executor, MySqlConnection, Row, TypeInfo};

use super::sql_text::quote_ident_mysql as qi;
use super::{
    bytes_summary, kind_of_type, ColumnDetail, ColumnMeta, ConnectConfig, DataFilter, GridChange,
    IndexDetail, StatementResult, TableBrief, TableDetail,
};

pub async fn make_pool(cfg: &ConnectConfig) -> Result<MySqlPool, String> {
    let mut opts = MySqlConnectOptions::new()
        .host(&cfg.host)
        .port(cfg.port)
        .username(&cfg.username)
        .charset("utf8mb4");
    if !cfg.password.is_empty() {
        opts = opts.password(&cfg.password);
    }
    if let Some(db) = cfg.database.as_deref() {
        if !db.is_empty() {
            opts = opts.database(db);
        }
    }
    MySqlPoolOptions::new()
        .max_connections(3)
        .acquire_timeout(Duration::from_secs(cfg.connect_timeout_secs))
        .connect_with(opts)
        .await
        .map_err(|e| format!("连接失败: {e}"))
}

pub async fn server_version(pool: &MySqlPool) -> Result<String, String> {
    let row: (String,) = sqlx::query_as("SELECT VERSION()")
        .fetch_one(pool)
        .await
        .map_err(|e| format!("查询版本失败: {e}"))?;
    Ok(format!("MySQL {}", row.0))
}

pub async fn list_databases(pool: &MySqlPool) -> Result<Vec<String>, String> {
    let rows: Vec<(String,)> = sqlx::query_as("SHOW DATABASES")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("查询库列表失败: {e}"))?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

pub async fn list_tables(pool: &MySqlPool, database: &str) -> Result<Vec<TableBrief>, String> {
    let rows = sqlx::query(
        "SELECT TABLE_NAME, TABLE_TYPE, TABLE_COMMENT, COALESCE(TABLE_ROWS, 0) AS rows_est \
         FROM information_schema.TABLES WHERE TABLE_SCHEMA = ? ORDER BY TABLE_NAME",
    )
    .bind(database)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("查询表列表失败: {e}"))?;

    Ok(rows
        .iter()
        .map(|r| {
            let ttype: String = r.try_get::<String, _>(1).unwrap_or_default();
            TableBrief {
                name: r.try_get(0).unwrap_or_default(),
                table_type: if ttype.contains("VIEW") { "view" } else { "table" }.to_string(),
                comment: r.try_get(2).unwrap_or_default(),
                row_estimate: r
                    .try_get::<i64, _>(3)
                    .or_else(|_| r.try_get::<u64, _>(3).map(|v| v as i64))
                    .unwrap_or(0),
            }
        })
        .collect())
}

pub async fn table_detail(
    pool: &MySqlPool,
    database: &str,
    table: &str,
) -> Result<TableDetail, String> {
    let col_rows = sqlx::query(
        "SELECT COLUMN_NAME, COLUMN_TYPE, IS_NULLABLE, COLUMN_DEFAULT, COLUMN_COMMENT, COLUMN_KEY \
         FROM information_schema.COLUMNS \
         WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? ORDER BY ORDINAL_POSITION",
    )
    .bind(database)
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("查询字段失败: {e}"))?;

    let columns: Vec<ColumnDetail> = col_rows
        .iter()
        .map(|r| ColumnDetail {
            name: r.try_get(0).unwrap_or_default(),
            data_type: r.try_get(1).unwrap_or_default(),
            nullable: r.try_get::<String, _>(2).map(|v| v == "YES").unwrap_or(true),
            default_value: r.try_get::<Option<String>, _>(3).unwrap_or(None),
            comment: r.try_get(4).unwrap_or_default(),
            primary_key: r.try_get::<String, _>(5).map(|v| v == "PRI").unwrap_or(false),
        })
        .collect();

    let idx_rows = sqlx::query(
        "SELECT INDEX_NAME, NON_UNIQUE, COLUMN_NAME \
         FROM information_schema.STATISTICS \
         WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? ORDER BY INDEX_NAME, SEQ_IN_INDEX",
    )
    .bind(database)
    .bind(table)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("查询索引失败: {e}"))?;

    let mut indexes: Vec<IndexDetail> = Vec::new();
    for r in &idx_rows {
        let name: String = r.try_get(0).unwrap_or_default();
        let non_unique: i64 = r
            .try_get::<i64, _>(1)
            .or_else(|_| r.try_get::<i32, _>(1).map(|v| v as i64))
            .unwrap_or(1);
        let col: String = r.try_get(2).unwrap_or_default();
        match indexes.last_mut() {
            Some(last) if last.name == name => last.columns.push(col),
            _ => indexes.push(IndexDetail {
                name,
                columns: vec![col],
                unique: non_unique == 0,
                definition: String::new(),
            }),
        }
    }

    // DDL：先按表取，失败再按视图取
    let qualified = format!("{}.{}", qi(database), qi(table));
    let ddl = match sqlx::query(&format!("SHOW CREATE TABLE {qualified}"))
        .fetch_one(pool)
        .await
    {
        Ok(row) => row.try_get::<String, _>(1).unwrap_or_default(),
        Err(_) => sqlx::query(&format!("SHOW CREATE VIEW {qualified}"))
            .fetch_one(pool)
            .await
            .ok()
            .and_then(|row| row.try_get::<String, _>(1).ok())
            .unwrap_or_default(),
    };

    Ok(TableDetail {
        columns,
        indexes,
        ddl,
    })
}

pub async fn session_id(conn: &mut MySqlConnection) -> Result<u64, String> {
    let row: (u64,) = sqlx::query_as("SELECT CONNECTION_ID()")
        .fetch_one(conn)
        .await
        .map_err(|e| format!("获取会话标识失败: {e}"))?;
    Ok(row.0)
}

pub async fn cancel_query(pool: &MySqlPool, session: u64) -> Result<(), String> {
    sqlx::raw_sql(format!("KILL QUERY {session}").as_str())
        .execute(pool)
        .await
        .map_err(|e| format!("取消执行失败: {e}"))?;
    Ok(())
}

/// 在指定连接上执行单条语句。用 raw_sql（文本协议）以兼容 SHOW / USE 等
/// 无法预编译的语句；行数达到上限后继续消费流但不再收集，保证连接干净。
pub async fn run_statement(
    conn: &mut MySqlConnection,
    sql: &str,
    max_rows: usize,
) -> Result<StatementResult, String> {
    let started = Instant::now();
    let mut columns: Vec<ColumnMeta> = Vec::new();
    let mut rows: Vec<Vec<Value>> = Vec::new();
    let mut affected: u64 = 0;
    let mut truncated = false;

    {
        let mut stream = conn.fetch_many(sqlx::raw_sql(sql));
        while let Some(item) = stream.next().await {
            match item.map_err(|e| format!("{e}"))? {
                Either::Left(done) => affected += done.rows_affected(),
                Either::Right(row) => {
                    if columns.is_empty() {
                        columns = column_meta(&row);
                    }
                    if rows.len() >= max_rows {
                        truncated = true;
                        continue;
                    }
                    rows.push(decode_row(&row));
                }
            }
        }
    }

    Ok(StatementResult {
        sql: sql.to_string(),
        columns,
        rows,
        affected,
        truncated,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn column_meta(row: &MySqlRow) -> Vec<ColumnMeta> {
    row.columns()
        .iter()
        .map(|c: &MySqlColumn| {
            let tn = c.type_info().name().to_string();
            ColumnMeta {
                name: c.name().to_string(),
                kind: kind_of_type(&tn).to_string(),
                type_name: tn,
            }
        })
        .collect()
}

fn decode_row(row: &MySqlRow) -> Vec<Value> {
    (0..row.columns().len())
        .map(|i| decode_cell(row, i))
        .collect()
}

/// 单元格解码：全部字符串化（string | null），保精度、防前端 number 溢出。
fn decode_cell(row: &MySqlRow, i: usize) -> Value {
    macro_rules! try_as {
        ($t:ty, $fmt:expr) => {
            if let Ok(v) = row.try_get::<Option<$t>, _>(i) {
                return match v {
                    None => Value::Null,
                    Some(x) => Value::String($fmt(x)),
                };
            }
        };
    }
    let type_name = row.columns()[i].type_info().name().to_ascii_uppercase();

    match type_name.as_str() {
        "BOOLEAN" => try_as!(bool, |x: bool| x.to_string()),
        "JSON" => try_as!(serde_json::Value, |x: serde_json::Value| x.to_string()),
        "DATETIME" => try_as!(chrono::NaiveDateTime, |x: chrono::NaiveDateTime| x
            .format("%Y-%m-%d %H:%M:%S%.f")
            .to_string()
            .trim_end_matches(".000000")
            .trim_end_matches(".000")
            .to_string()),
        "TIMESTAMP" => try_as!(chrono::DateTime<chrono::Utc>, |x: chrono::DateTime<
            chrono::Utc,
        >| x
            .format("%Y-%m-%d %H:%M:%S%.f")
            .to_string()
            .trim_end_matches(".000000")
            .trim_end_matches(".000")
            .to_string()),
        "DATE" => try_as!(chrono::NaiveDate, |x: chrono::NaiveDate| x.to_string()),
        "TIME" => try_as!(chrono::NaiveTime, |x: chrono::NaiveTime| x.to_string()),
        _ => {}
    }

    // 通用兜底链：字符串 → 有符号 → 无符号 → 浮点 → 定点 → 时间族 → bool → 字节摘要
    try_as!(String, |x: String| x);
    try_as!(i64, |x: i64| x.to_string());
    try_as!(u64, |x: u64| x.to_string());
    try_as!(f64, |x: f64| x.to_string());
    try_as!(f32, |x: f32| x.to_string());
    try_as!(rust_decimal::Decimal, |x: rust_decimal::Decimal| x.to_string());
    try_as!(chrono::NaiveDateTime, |x: chrono::NaiveDateTime| x.to_string());
    try_as!(chrono::DateTime<chrono::Utc>, |x: chrono::DateTime<chrono::Utc>| x.to_string());
    try_as!(chrono::NaiveDate, |x: chrono::NaiveDate| x.to_string());
    try_as!(chrono::NaiveTime, |x: chrono::NaiveTime| x.to_string());
    try_as!(bool, |x: bool| x.to_string());
    try_as!(Vec<u8>, |x: Vec<u8>| bytes_summary(&x));
    Value::String(format!("(无法解码: {type_name})"))
}

/// 构造 WHERE 片段与绑定值。返回 (sql 片段, 待绑定值)。
fn build_where(filters: &[DataFilter]) -> Result<(String, Vec<String>), String> {
    if filters.is_empty() {
        return Ok((String::new(), Vec::new()));
    }
    let mut parts = Vec::new();
    let mut binds = Vec::new();
    for f in filters {
        let op = super::normalize_filter_op(&f.op)?;
        if op == "IS NULL" || op == "IS NOT NULL" {
            parts.push(format!("{} {}", qi(&f.column), op));
        } else {
            parts.push(format!("{} {} ?", qi(&f.column), op));
            binds.push(f.value.clone());
        }
    }
    Ok((format!(" WHERE {}", parts.join(" AND ")), binds))
}

pub async fn table_data_page(
    pool: &MySqlPool,
    database: &str,
    table: &str,
    filters: &[DataFilter],
    order_by: Option<(&str, bool)>,
    page: u64,
    page_size: u64,
) -> Result<(StatementResult, u64), String> {
    let qualified = format!("{}.{}", qi(database), qi(table));
    let (where_sql, binds) = build_where(filters)?;
    let order_sql = match order_by {
        Some((col, asc)) => format!(" ORDER BY {} {}", qi(col), if asc { "ASC" } else { "DESC" }),
        None => String::new(),
    };

    let count_sql = format!("SELECT COUNT(*) FROM {qualified}{where_sql}");
    let mut count_q = sqlx::query_as::<_, (i64,)>(&count_sql);
    for b in &binds {
        count_q = count_q.bind(b);
    }
    let total = count_q
        .fetch_one(pool)
        .await
        .map_err(|e| format!("统计行数失败: {e}"))?
        .0 as u64;

    let data_sql = format!(
        "SELECT * FROM {qualified}{where_sql}{order_sql} LIMIT ? OFFSET ?"
    );
    let started = Instant::now();
    let mut data_q = sqlx::query(&data_sql);
    for b in &binds {
        data_q = data_q.bind(b);
    }
    data_q = data_q.bind(page_size).bind(page.saturating_mul(page_size));
    let fetched = data_q
        .fetch_all(pool)
        .await
        .map_err(|e| format!("查询数据失败: {e}"))?;

    let columns = fetched.first().map(column_meta).unwrap_or_default();
    let rows: Vec<Vec<Value>> = fetched.iter().map(decode_row).collect();

    Ok((
        StatementResult {
            sql: data_sql,
            columns,
            rows,
            affected: 0,
            truncated: false,
            duration_ms: started.elapsed().as_millis() as u64,
        },
        total,
    ))
}

/// 单事务应用变更集。MySQL 弱类型：值一律按字符串绑定由服务端协变。
/// UPDATE / DELETE 影响行数为 0 视为并发冲突，整体回滚。
pub async fn apply_changes(
    pool: &MySqlPool,
    database: &str,
    table: &str,
    changes: &[GridChange],
) -> Result<Vec<u64>, (usize, String)> {
    let qualified = format!("{}.{}", qi(database), qi(table));
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| (0usize, format!("开启事务失败: {e}")))?;

    let mut affected_list = Vec::with_capacity(changes.len());
    for (idx, change) in changes.iter().enumerate() {
        let (sql, binds) = build_change_sql(&qualified, change).map_err(|e| (idx, e))?;
        let mut q = sqlx::query(&sql);
        for b in &binds {
            q = q.bind(b.clone());
        }
        let result = q
            .execute(&mut *tx)
            .await
            .map_err(|e| (idx, format!("{e}")))?;
        let affected = result.rows_affected();
        if change.change_type != "insert" && affected == 0 {
            return Err((idx, "影响行数为 0：目标行可能已被其他会话修改或删除".into()));
        }
        affected_list.push(affected);
    }

    tx.commit()
        .await
        .map_err(|e| (changes.len().saturating_sub(1), format!("提交事务失败: {e}")))?;
    Ok(affected_list)
}

/// 生成单条变更的参数化 SQL。绑定值为 Option<String>（None 即 SQL NULL）。
fn build_change_sql(
    qualified: &str,
    change: &GridChange,
) -> Result<(String, Vec<Option<String>>), String> {
    let mut binds: Vec<Option<String>> = Vec::new();
    match change.change_type.as_str() {
        "update" => {
            if change.values.is_empty() {
                return Err("UPDATE 变更缺少字段值".into());
            }
            if change.pk.is_empty() {
                return Err("UPDATE 变更缺少主键条件".into());
            }
            let sets: Vec<String> = change
                .values
                .iter()
                .map(|(col, val)| {
                    binds.push(val.clone());
                    format!("{} = ?", qi(col))
                })
                .collect();
            let wheres: Vec<String> = change
                .pk
                .iter()
                .map(|(col, val)| {
                    binds.push(val.clone());
                    format!("{} = ?", qi(col))
                })
                .collect();
            Ok((
                format!(
                    "UPDATE {qualified} SET {} WHERE {}",
                    sets.join(", "),
                    wheres.join(" AND ")
                ),
                binds,
            ))
        }
        "insert" => {
            if change.values.is_empty() {
                return Err("INSERT 变更缺少字段值".into());
            }
            let cols: Vec<String> = change.values.iter().map(|(c, _)| qi(c)).collect();
            let placeholders: Vec<&str> = change
                .values
                .iter()
                .map(|(_, v)| {
                    binds.push(v.clone());
                    "?"
                })
                .collect();
            Ok((
                format!(
                    "INSERT INTO {qualified} ({}) VALUES ({})",
                    cols.join(", "),
                    placeholders.join(", ")
                ),
                binds,
            ))
        }
        "delete" => {
            if change.pk.is_empty() {
                return Err("DELETE 变更缺少主键条件".into());
            }
            let wheres: Vec<String> = change
                .pk
                .iter()
                .map(|(col, val)| {
                    binds.push(val.clone());
                    format!("{} = ?", qi(col))
                })
                .collect();
            Ok((
                format!("DELETE FROM {qualified} WHERE {}", wheres.join(" AND ")),
                binds,
            ))
        }
        other => Err(format!("未知变更类型: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(t: &str, pk: &[(&str, Option<&str>)], values: &[(&str, Option<&str>)]) -> GridChange {
        GridChange {
            change_type: t.to_string(),
            pk: pk
                .iter()
                .map(|(c, v)| (c.to_string(), v.map(|s| s.to_string())))
                .collect(),
            values: values
                .iter()
                .map(|(c, v)| (c.to_string(), v.map(|s| s.to_string())))
                .collect(),
        }
    }

    #[test]
    fn build_update_sql() {
        let c = change(
            "update",
            &[("id", Some("5"))],
            &[("name", Some("x")), ("age", None)],
        );
        let (sql, binds) = build_change_sql("`db`.`t`", &c).unwrap();
        assert_eq!(sql, "UPDATE `db`.`t` SET `name` = ?, `age` = ? WHERE `id` = ?");
        assert_eq!(binds, vec![Some("x".into()), None, Some("5".into())]);
    }

    #[test]
    fn build_insert_delete_sql() {
        let c = change("insert", &[], &[("a", Some("1")), ("b", None)]);
        let (sql, binds) = build_change_sql("`d`.`t`", &c).unwrap();
        assert_eq!(sql, "INSERT INTO `d`.`t` (`a`, `b`) VALUES (?, ?)");
        assert_eq!(binds.len(), 2);

        let c = change("delete", &[("id", Some("9"))], &[]);
        let (sql, _) = build_change_sql("`d`.`t`", &c).unwrap();
        assert_eq!(sql, "DELETE FROM `d`.`t` WHERE `id` = ?");
    }

    #[test]
    fn build_change_rejects_invalid() {
        assert!(build_change_sql("`d`.`t`", &change("update", &[], &[("a", Some("1"))])).is_err());
        assert!(build_change_sql("`d`.`t`", &change("update", &[("id", Some("1"))], &[])).is_err());
        assert!(build_change_sql("`d`.`t`", &change("delete", &[], &[])).is_err());
        assert!(build_change_sql("`d`.`t`", &change("upsert", &[], &[])).is_err());
    }

    #[test]
    fn where_builder_validates_ops() {
        let filters = vec![
            DataFilter {
                column: "a".into(),
                op: "like".into(),
                value: "%x%".into(),
            },
            DataFilter {
                column: "b".into(),
                op: "IS NULL".into(),
                value: String::new(),
            },
        ];
        let (sql, binds) = build_where(&filters).unwrap();
        assert_eq!(sql, " WHERE `a` LIKE ? AND `b` IS NULL");
        assert_eq!(binds, vec!["%x%".to_string()]);

        let bad = vec![DataFilter {
            column: "a".into(),
            op: "REGEXP".into(),
            value: "x".into(),
        }];
        assert!(build_where(&bad).is_err());
    }
}

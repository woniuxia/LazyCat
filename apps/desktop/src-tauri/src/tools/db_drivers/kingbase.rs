//! KingbaseES（人大金仓）驱动：走 PG 线协议（sqlx Postgres）。
//!
//! 与 MySQL 的关键差异：
//! - PG 族连接与库绑定，database 必填；跨库需另建池（db.rs 按 connection+database 缓存池）。
//! - 表名带 schema 限定（`schema.table`），系统 schema 已在列表查询中过滤。
//! - PG 强类型：筛选与写回的字符串参数必须 `CAST($n AS 列类型)`，LIKE 则把列转 text。
//! - DDL 无 SHOW CREATE TABLE，由 pg_catalog 拼装基础版本。

use std::collections::HashMap;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde_json::Value;
use sqlx::postgres::{PgColumn, PgConnectOptions, PgPool, PgPoolOptions, PgRow};
use sqlx::{Column, Either, Executor, PgConnection, Row, TypeInfo};

use super::sql_text::quote_ident_pg as qi;
use super::{
    bytes_summary, kind_of_type, ColumnDetail, ColumnMeta, ConnectConfig, DataFilter, GridChange,
    IndexDetail, StatementResult, TableBrief, TableDetail,
};

/// KB 常见系统 schema（在 pg_% / information_schema 之外的部分）。
const SYSTEM_SCHEMAS: &str = "'pg_catalog','information_schema','sys','sysaudit','sys_catalog','sys_hm','sysmac','xlog_record_read','dbms_sql','anon','perf','sys_sqlbud','src_restrict','wmsys','olap'";

pub async fn make_pool(cfg: &ConnectConfig) -> Result<PgPool, String> {
    let database = cfg
        .database
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "KingbaseES 连接必须指定数据库".to_string())?;
    let mut opts = PgConnectOptions::new()
        .host(&cfg.host)
        .port(cfg.port)
        .username(&cfg.username)
        .database(database);
    if !cfg.password.is_empty() {
        opts = opts.password(&cfg.password);
    }
    PgPoolOptions::new()
        .max_connections(3)
        .acquire_timeout(Duration::from_secs(cfg.connect_timeout_secs))
        .connect_with(opts)
        .await
        .map_err(|e| format!("连接失败: {e}"))
}

pub async fn server_version(pool: &PgPool) -> Result<String, String> {
    let row: (String,) = sqlx::query_as("SELECT version()")
        .fetch_one(pool)
        .await
        .map_err(|e| format!("查询版本失败: {e}"))?;
    // KingbaseES 的 version() 里自带产品名，直接透传首段
    Ok(row.0.split(" on ").next().unwrap_or(&row.0).to_string())
}

pub async fn list_databases(pool: &PgPool) -> Result<Vec<String>, String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT datname FROM pg_database WHERE datallowconn AND NOT datistemplate ORDER BY datname",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("查询库列表失败: {e}"))?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// 列出当前库的所有用户表/视图，名称带 schema 限定（`schema.table`）。
pub async fn list_tables(pool: &PgPool) -> Result<Vec<TableBrief>, String> {
    let sql = format!(
        "SELECT n.nspname, c.relname, c.relkind::text, \
                COALESCE(obj_description(c.oid, 'pg_class'), '') AS comment, \
                c.reltuples::bigint AS row_estimate \
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE c.relkind IN ('r','p','v','m') \
           AND n.nspname NOT LIKE 'pg\\_%' \
           AND n.nspname NOT IN ({SYSTEM_SCHEMAS}) \
         ORDER BY n.nspname, c.relname"
    );
    let rows = sqlx::query(&sql)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("查询表列表失败: {e}"))?;

    Ok(rows
        .iter()
        .map(|r| {
            let schema: String = r.try_get(0).unwrap_or_default();
            let name: String = r.try_get(1).unwrap_or_default();
            let relkind: String = r.try_get(2).unwrap_or_default();
            TableBrief {
                name: format!("{schema}.{name}"),
                table_type: if relkind == "v" || relkind == "m" { "view" } else { "table" }
                    .to_string(),
                comment: r.try_get(3).unwrap_or_default(),
                row_estimate: r.try_get::<i64, _>(4).unwrap_or(0).max(0),
            }
        })
        .collect())
}

/// 拆出 schema 与表名（表列表返回的名称固定为 `schema.table` 形态）。
fn split_qualified(table: &str) -> (String, String) {
    match table.split_once('.') {
        Some((s, t)) => (s.to_string(), t.to_string()),
        None => ("public".to_string(), table.to_string()),
    }
}

/// 查询表的列名 → 类型映射（format_type 输出可直接用于 CAST）。
async fn column_types(pool: &PgPool, table: &str) -> Result<HashMap<String, String>, String> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT a.attname, format_type(a.atttypid, a.atttypmod) \
         FROM pg_attribute a \
         WHERE a.attrelid = $1::regclass AND a.attnum > 0 AND NOT a.attisdropped",
    )
    .bind(qi(table))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("查询列类型失败: {e}"))?;
    Ok(rows.into_iter().collect())
}

pub async fn table_detail(pool: &PgPool, table: &str) -> Result<TableDetail, String> {
    let (schema, name) = split_qualified(table);
    let regclass = qi(table);

    let col_rows = sqlx::query(
        "SELECT a.attname, format_type(a.atttypid, a.atttypmod), NOT a.attnotnull, \
                pg_get_expr(ad.adbin, ad.adrelid), \
                COALESCE(col_description(a.attrelid, a.attnum), ''), \
                COALESCE((SELECT TRUE FROM pg_index i \
                          WHERE i.indrelid = a.attrelid AND i.indisprimary \
                            AND a.attnum = ANY(i.indkey)), FALSE) \
         FROM pg_attribute a \
         LEFT JOIN pg_attrdef ad ON ad.adrelid = a.attrelid AND ad.adnum = a.attnum \
         WHERE a.attrelid = $1::regclass AND a.attnum > 0 AND NOT a.attisdropped \
         ORDER BY a.attnum",
    )
    .bind(&regclass)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("查询字段失败: {e}"))?;

    let columns: Vec<ColumnDetail> = col_rows
        .iter()
        .map(|r| ColumnDetail {
            name: r.try_get(0).unwrap_or_default(),
            data_type: r.try_get(1).unwrap_or_default(),
            nullable: r.try_get(2).unwrap_or(true),
            default_value: r.try_get::<Option<String>, _>(3).unwrap_or(None),
            comment: r.try_get(4).unwrap_or_default(),
            primary_key: r.try_get(5).unwrap_or(false),
        })
        .collect();

    let idx_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT indexname, indexdef FROM pg_indexes WHERE schemaname = $1 AND tablename = $2 ORDER BY indexname",
    )
    .bind(&schema)
    .bind(&name)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("查询索引失败: {e}"))?;

    let indexes: Vec<IndexDetail> = idx_rows
        .into_iter()
        .map(|(iname, def)| IndexDetail {
            unique: def.to_ascii_uppercase().contains("CREATE UNIQUE"),
            name: iname,
            columns: Vec::new(),
            definition: def,
        })
        .collect();

    // relkind 判定视图；视图用 pg_get_viewdef，表由目录信息拼装基础 DDL
    let relkind: (String,) = sqlx::query_as(
        "SELECT c.relkind::text FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = $1 AND c.relname = $2",
    )
    .bind(&schema)
    .bind(&name)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("查询对象类型失败: {e}"))?;

    let ddl = if relkind.0 == "v" || relkind.0 == "m" {
        let def: (String,) = sqlx::query_as("SELECT pg_get_viewdef($1::regclass, true)")
            .bind(&regclass)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("查询视图定义失败: {e}"))?;
        format!("CREATE OR REPLACE VIEW {regclass} AS\n{}", def.0)
    } else {
        let table_comment: (String,) = sqlx::query_as(
            "SELECT COALESCE(obj_description($1::regclass, 'pg_class'), '')",
        )
        .bind(&regclass)
        .fetch_one(pool)
        .await
        .unwrap_or((String::new(),));
        assemble_table_ddl(&regclass, &columns, &table_comment.0)
    };

    Ok(TableDetail {
        columns,
        indexes,
        ddl,
    })
}

/// 由列信息拼装基础建表 DDL（不含外键/检查约束，够浏览与复制用）。
fn assemble_table_ddl(regclass: &str, columns: &[ColumnDetail], table_comment: &str) -> String {
    let mut lines: Vec<String> = columns
        .iter()
        .map(|c| {
            let mut line = format!("  {} {}", qi(&c.name), c.data_type);
            if !c.nullable {
                line.push_str(" NOT NULL");
            }
            if let Some(def) = &c.default_value {
                line.push_str(&format!(" DEFAULT {def}"));
            }
            line
        })
        .collect();
    let pk: Vec<String> = columns
        .iter()
        .filter(|c| c.primary_key)
        .map(|c| qi(&c.name))
        .collect();
    if !pk.is_empty() {
        lines.push(format!("  PRIMARY KEY ({})", pk.join(", ")));
    }
    let mut ddl = format!("CREATE TABLE {regclass} (\n{}\n);", lines.join(",\n"));
    if !table_comment.is_empty() {
        ddl.push_str(&format!(
            "\nCOMMENT ON TABLE {regclass} IS '{}';",
            table_comment.replace('\'', "''")
        ));
    }
    for c in columns {
        if !c.comment.is_empty() {
            ddl.push_str(&format!(
                "\nCOMMENT ON COLUMN {regclass}.{} IS '{}';",
                qi(&c.name),
                c.comment.replace('\'', "''")
            ));
        }
    }
    ddl
}

pub async fn session_id(conn: &mut PgConnection) -> Result<i32, String> {
    let row: (i32,) = sqlx::query_as("SELECT pg_backend_pid()")
        .fetch_one(conn)
        .await
        .map_err(|e| format!("获取会话标识失败: {e}"))?;
    Ok(row.0)
}

pub async fn cancel_query(pool: &PgPool, pid: i32) -> Result<(), String> {
    sqlx::query("SELECT pg_cancel_backend($1)")
        .bind(pid)
        .execute(pool)
        .await
        .map_err(|e| format!("取消执行失败: {e}"))?;
    Ok(())
}

/// 简单协议执行单条语句（兼容 SET / SHOW 等），行数达到上限后丢弃续行。
pub async fn run_statement(
    conn: &mut PgConnection,
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

fn column_meta(row: &PgRow) -> Vec<ColumnMeta> {
    row.columns()
        .iter()
        .map(|c: &PgColumn| {
            let tn = c.type_info().name().to_string();
            ColumnMeta {
                name: c.name().to_string(),
                kind: kind_of_type(&tn).to_string(),
                type_name: tn,
            }
        })
        .collect()
}

fn decode_row(row: &PgRow) -> Vec<Value> {
    (0..row.columns().len())
        .map(|i| decode_cell(row, i))
        .collect()
}

fn decode_cell(row: &PgRow, i: usize) -> Value {
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
        "BOOL" => try_as!(bool, |x: bool| x.to_string()),
        "JSON" | "JSONB" => try_as!(serde_json::Value, |x: serde_json::Value| x.to_string()),
        "UUID" => try_as!(uuid::Uuid, |x: uuid::Uuid| x.to_string()),
        "TIMESTAMPTZ" => try_as!(chrono::DateTime<chrono::Utc>, |x: chrono::DateTime<
            chrono::Utc,
        >| x
            .format("%Y-%m-%d %H:%M:%S%.f%:z")
            .to_string()),
        "TIMESTAMP" => try_as!(chrono::NaiveDateTime, |x: chrono::NaiveDateTime| x
            .format("%Y-%m-%d %H:%M:%S%.f")
            .to_string()
            .trim_end_matches(".000000")
            .to_string()),
        "DATE" => try_as!(chrono::NaiveDate, |x: chrono::NaiveDate| x.to_string()),
        "TIME" => try_as!(chrono::NaiveTime, |x: chrono::NaiveTime| x.to_string()),
        "BYTEA" => try_as!(Vec<u8>, |x: Vec<u8>| bytes_summary(&x)),
        _ => {}
    }

    // 通用兜底链
    try_as!(String, |x: String| x);
    try_as!(i64, |x: i64| x.to_string());
    try_as!(i32, |x: i32| x.to_string());
    try_as!(i16, |x: i16| x.to_string());
    try_as!(f64, |x: f64| x.to_string());
    try_as!(f32, |x: f32| x.to_string());
    try_as!(rust_decimal::Decimal, |x: rust_decimal::Decimal| x.to_string());
    try_as!(chrono::NaiveDateTime, |x: chrono::NaiveDateTime| x.to_string());
    try_as!(chrono::DateTime<chrono::Utc>, |x: chrono::DateTime<chrono::Utc>| x.to_string());
    try_as!(chrono::NaiveDate, |x: chrono::NaiveDate| x.to_string());
    try_as!(chrono::NaiveTime, |x: chrono::NaiveTime| x.to_string());
    try_as!(bool, |x: bool| x.to_string());
    try_as!(uuid::Uuid, |x: uuid::Uuid| x.to_string());
    try_as!(serde_json::Value, |x: serde_json::Value| x.to_string());
    try_as!(Vec<u8>, |x: Vec<u8>| bytes_summary(&x));
    Value::String(format!("(无法解码: {type_name})"))
}

/// 构造 WHERE 片段（$n 占位）。比较类操作符把参数 CAST 成列类型；
/// LIKE 类把列转成 text 再匹配。
fn build_where(
    filters: &[DataFilter],
    types: &HashMap<String, String>,
    start_index: usize,
) -> Result<(String, Vec<String>), String> {
    if filters.is_empty() {
        return Ok((String::new(), Vec::new()));
    }
    let mut parts = Vec::new();
    let mut binds = Vec::new();
    let mut n = start_index;
    for f in filters {
        let op = super::normalize_filter_op(&f.op)?;
        match op {
            "IS NULL" | "IS NOT NULL" => parts.push(format!("{} {}", qi(&f.column), op)),
            "LIKE" | "NOT LIKE" => {
                parts.push(format!("{}::text {} ${}", qi(&f.column), op, n));
                binds.push(f.value.clone());
                n += 1;
            }
            _ => {
                let cast = types
                    .get(&f.column)
                    .map(|t| format!("CAST(${n} AS {t})"))
                    .unwrap_or_else(|| format!("${n}"));
                parts.push(format!("{} {} {}", qi(&f.column), op, cast));
                binds.push(f.value.clone());
                n += 1;
            }
        }
    }
    Ok((format!(" WHERE {}", parts.join(" AND ")), binds))
}

pub async fn table_data_page(
    pool: &PgPool,
    table: &str,
    filters: &[DataFilter],
    order_by: Option<(&str, bool)>,
    page: u64,
    page_size: u64,
) -> Result<(StatementResult, u64), String> {
    let types = column_types(pool, table).await?;
    let qualified = qi(table);
    let (where_sql, binds) = build_where(filters, &types, 1)?;
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
        "SELECT * FROM {qualified}{where_sql}{order_sql} LIMIT {page_size} OFFSET {}",
        page.saturating_mul(page_size)
    );
    let started = Instant::now();
    let mut data_q = sqlx::query(&data_sql);
    for b in &binds {
        data_q = data_q.bind(b);
    }
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

pub async fn apply_changes(
    pool: &PgPool,
    table: &str,
    changes: &[GridChange],
) -> Result<Vec<u64>, (usize, String)> {
    let types = column_types(pool, table)
        .await
        .map_err(|e| (0usize, e))?;
    let qualified = qi(table);
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| (0usize, format!("开启事务失败: {e}")))?;

    let mut affected_list = Vec::with_capacity(changes.len());
    for (idx, change) in changes.iter().enumerate() {
        let (sql, binds) = build_change_sql(&qualified, &types, change).map_err(|e| (idx, e))?;
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

/// 生成单条变更的参数化 SQL：值参数按列类型 CAST（PG 强类型要求）。
fn build_change_sql(
    qualified: &str,
    types: &HashMap<String, String>,
    change: &GridChange,
) -> Result<(String, Vec<Option<String>>), String> {
    let cast_expr = |col: &str, n: usize| -> String {
        match types.get(col) {
            Some(t) => format!("CAST(${n} AS {t})"),
            None => format!("${n}"),
        }
    };
    let mut binds: Vec<Option<String>> = Vec::new();
    let mut n = 0usize;
    let mut next = |binds: &mut Vec<Option<String>>, v: &Option<String>| -> usize {
        binds.push(v.clone());
        n += 1;
        n
    };
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
                    let idx = next(&mut binds, val);
                    format!("{} = {}", qi(col), cast_expr(col, idx))
                })
                .collect();
            let wheres: Vec<String> = change
                .pk
                .iter()
                .map(|(col, val)| {
                    let idx = next(&mut binds, val);
                    format!("{} = {}", qi(col), cast_expr(col, idx))
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
            let placeholders: Vec<String> = change
                .values
                .iter()
                .map(|(col, val)| {
                    let idx = next(&mut binds, val);
                    cast_expr(col, idx)
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
                    let idx = next(&mut binds, val);
                    format!("{} = {}", qi(col), cast_expr(col, idx))
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

    fn types() -> HashMap<String, String> {
        HashMap::from([
            ("id".to_string(), "integer".to_string()),
            ("name".to_string(), "character varying(50)".to_string()),
        ])
    }

    #[test]
    fn build_update_with_cast() {
        let change = GridChange {
            change_type: "update".into(),
            pk: vec![("id".into(), Some("5".into()))],
            values: vec![("name".into(), Some("x".into())), ("memo".into(), None)],
        };
        let (sql, binds) = build_change_sql("\"public\".\"t\"", &types(), &change).unwrap();
        assert_eq!(
            sql,
            "UPDATE \"public\".\"t\" SET \"name\" = CAST($1 AS character varying(50)), \"memo\" = $2 WHERE \"id\" = CAST($3 AS integer)"
        );
        assert_eq!(binds, vec![Some("x".into()), None, Some("5".into())]);
    }

    #[test]
    fn build_where_like_uses_text_cast() {
        let filters = vec![
            DataFilter {
                column: "id".into(),
                op: ">=".into(),
                value: "10".into(),
            },
            DataFilter {
                column: "id".into(),
                op: "LIKE".into(),
                value: "%1%".into(),
            },
        ];
        let (sql, binds) = build_where(&filters, &types(), 1).unwrap();
        assert_eq!(
            sql,
            " WHERE \"id\" >= CAST($1 AS integer) AND \"id\"::text LIKE $2"
        );
        assert_eq!(binds.len(), 2);
    }

    #[test]
    fn split_qualified_names() {
        assert_eq!(
            split_qualified("public.users"),
            ("public".to_string(), "users".to_string())
        );
        assert_eq!(
            split_qualified("users"),
            ("public".to_string(), "users".to_string())
        );
    }

    #[test]
    fn assemble_ddl_contains_pk_and_comments() {
        let cols = vec![
            ColumnDetail {
                name: "id".into(),
                data_type: "integer".into(),
                nullable: false,
                default_value: None,
                comment: "主键".into(),
                primary_key: true,
            },
            ColumnDetail {
                name: "name".into(),
                data_type: "text".into(),
                nullable: true,
                default_value: Some("''::text".into()),
                comment: String::new(),
                primary_key: false,
            },
        ];
        let ddl = assemble_table_ddl("\"public\".\"t\"", &cols, "用户表");
        assert!(ddl.contains("\"id\" integer NOT NULL"));
        assert!(ddl.contains("PRIMARY KEY (\"id\")"));
        assert!(ddl.contains("COMMENT ON TABLE \"public\".\"t\" IS '用户表';"));
        assert!(ddl.contains("COMMENT ON COLUMN \"public\".\"t\".\"id\" IS '主键';"));
    }
}

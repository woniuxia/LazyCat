use super::import::{
    build_record_values, build_search_text, escape_like_pattern, flatten_record,
    normalize_search_text, normalized_primary_key,
};
use super::model::*;
use super::path::get_value_by_field_path;
use super::records::{
    build_record_sort_key, is_relation_value_usable, parse_sort_direction,
    record_row_to_brief_json, record_row_to_search_item_json, sort_record_rows,
};
use rusqlite::{params, Connection, OptionalExtension, Row, ToSql};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

pub(super) type SqlParam = Box<dyn ToSql>;

pub(super) fn field_has_non_scalar_values(
    conn: &Connection,
    dictionary_id: i64,
    field_path: &str,
) -> Result<bool, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, raw_json
             FROM data_dictionary_records
             WHERE dictionary_id = ?1",
        )
        .map_err(|e| format!("prepare relation source field scan failed: {e}"))?;
    let rows = stmt
        .query_map(params![dictionary_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("query relation source field scan failed: {e}"))?;
    for row in rows {
        let (record_id, raw_json) = row.map_err(|e| e.to_string())?;
        let record = serde_json::from_str::<Value>(&raw_json)
            .map_err(|e| format!("parse data dictionary record {record_id} failed: {e}"))?;
        if matches!(
            get_value_by_field_path(&record, field_path),
            Some(Value::Array(_) | Value::Object(_))
        ) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn insert_records(
    conn: &Connection,
    dictionary_id: i64,
    records: &[IndexedRecord],
    searchable_paths: &[String],
    sort_config: Option<(&str, SortDirection)>,
) -> Result<(), String> {
    for record in records {
        let fields = flatten_record(&record.value);
        let search_text = build_search_text(&fields, searchable_paths);
        let normalized = normalize_search_text(&search_text);
        let raw_json = serde_json::to_string(&record.value)
            .map_err(|e| format!("serialize data dictionary record failed: {e}"))?;
        let sort_key = build_record_sort_key(&record.value, record.source_row_index, sort_config)?;
        conn.execute(
            "INSERT INTO data_dictionary_records
             (dictionary_id, row_index, raw_json, search_text, normalized_search_text, sort_key)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                dictionary_id,
                record.source_row_index,
                raw_json,
                search_text,
                normalized,
                sort_key,
            ],
        )
        .map_err(|e| format!("insert data dictionary record failed: {e}"))?;
        let record_id = conn.last_insert_rowid();
        insert_record_values(
            conn,
            build_record_values(record_id, dictionary_id, &raw_json)?,
            None,
        )?;
    }
    Ok(())
}

pub(super) fn insert_record_values(
    conn: &Connection,
    values: Vec<RecordValue>,
    excluded_field_path: Option<&str>,
) -> Result<usize, String> {
    let mut count = 0;
    for value in values {
        if excluded_field_path == Some(value.field_path.as_str()) {
            continue;
        }
        conn.execute(
            "INSERT INTO data_dictionary_record_values
             (record_id, dictionary_id, field_path, value_type, value_text, normalized_value)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                value.record_id,
                value.dictionary_id,
                value.field_path,
                value.value_type,
                value.value_text,
                value.normalized_value,
            ],
        )
        .map_err(|e| format!("insert data dictionary record value failed: {e}"))?;
        count += 1;
    }
    Ok(count)
}

pub(super) fn query_empty_records(
    conn: &Connection,
    scope: &SearchScope,
    limit: Option<i64>,
) -> Result<Vec<RecordRow>, String> {
    query_records(conn, scope, Vec::new(), Vec::new(), limit, true)
}

pub(super) fn query_like_records(
    conn: &Connection,
    scope: &SearchScope,
    keyword: &str,
    limit: Option<i64>,
) -> Result<Vec<RecordRow>, String> {
    let (condition, pattern) = build_keyword_search_condition(keyword);
    query_records(
        conn,
        scope,
        vec![condition],
        vec![Box::new(pattern)],
        limit,
        false,
    )
}

pub(super) fn build_keyword_search_condition(keyword: &str) -> (String, String) {
    let normalized = normalize_search_text(keyword);
    let pattern = format!("%{}%", escape_like_pattern(&normalized));
    (
        "r.normalized_search_text LIKE ? ESCAPE '\\'".to_string(),
        pattern,
    )
}

pub(super) fn build_popular_records_query(where_clause: &str) -> String {
    format!(
        "SELECT CAST(u.scope_id AS INTEGER) AS dictionary_id,
                u.resource_id AS normalized_value,
                SUM(u.use_count) AS used_count,
                MAX(u.last_used_at_ms) AS last_used_at_ms,
                d.primary_field_path
         FROM usage_daily u
         JOIN data_dictionaries d ON d.id = CAST(u.scope_id AS INTEGER)
         {where_clause}
           AND u.resource_type = 'data-dictionary-record'
           AND u.action = 'view'
         GROUP BY u.scope_id, u.resource_id, d.primary_field_path
         ORDER BY used_count DESC, last_used_at_ms DESC
         LIMIT ?"
    )
}

pub(super) fn query_records(
    conn: &Connection,
    scope: &SearchScope,
    mut conditions: Vec<String>,
    mut params_list: Vec<SqlParam>,
    limit: Option<i64>,
    _empty_keyword: bool,
) -> Result<Vec<RecordRow>, String> {
    if let SearchScope::Current(dictionary_id) = scope {
        conditions.insert(0, "r.dictionary_id = ?".to_string());
        params_list.insert(0, Box::new(*dictionary_id));
    }
    let sql = build_record_query_sql(scope, &conditions, limit);
    if let Some(limit) = limit {
        params_list.push(Box::new(limit));
    }

    let refs: Vec<&dyn ToSql> = params_list.iter().map(|param| param.as_ref()).collect();
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("prepare data dictionary records query failed: {e}"))?;
    let rows = stmt
        .query_map(refs.as_slice(), record_row)
        .map_err(|e| format!("query data dictionary records failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub(super) fn build_record_query_sql(
    scope: &SearchScope,
    conditions: &[String],
    limit: Option<i64>,
) -> String {
    let mut sql = String::from(
        "SELECT r.id, r.dictionary_id, d.name, r.row_index, r.raw_json
         , d.title_field_path
         FROM data_dictionary_records r
         JOIN data_dictionaries d ON d.id = r.dictionary_id",
    );
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    match scope {
        SearchScope::Current(_) => {
            sql.push_str(" ORDER BY r.sort_key COLLATE BINARY ASC, r.id ASC");
        }
        SearchScope::All => {
            sql.push_str(" ORDER BY d.nav_order ASC, r.sort_key COLLATE BINARY ASC, r.id ASC");
        }
    }
    if limit.is_some() {
        sql.push_str(" LIMIT ?");
    }
    sql
}

pub(super) fn load_record_row_by_id(
    conn: &Connection,
    record_id: i64,
) -> Result<RecordRow, String> {
    conn.query_row(
        "SELECT r.id, r.dictionary_id, d.name, r.row_index, r.raw_json,
                d.title_field_path
         FROM data_dictionary_records r
         JOIN data_dictionaries d ON d.id = r.dictionary_id
         WHERE r.id = ?1",
        params![record_id],
        record_row,
    )
    .optional()
    .map_err(|e| format!("load data dictionary record detail failed: {e}"))?
    .ok_or("record not found".to_string())
}

pub(super) fn load_relation_configs(
    conn: &Connection,
    dictionary_id: i64,
    reverse: bool,
) -> Result<Vec<RelationConfig>, String> {
    let (sql, param_id) = if reverse {
        (
            "SELECT r.id, r.source_dictionary_id, r.source_field_path,
                    r.target_dictionary_id, d.primary_field_path,
                    r.relation_name, r.reverse_name
             FROM data_dictionary_relations r
             JOIN data_dictionaries d ON d.id = r.target_dictionary_id
             WHERE r.target_dictionary_id = ?1
             ORDER BY r.id ASC",
            dictionary_id,
        )
    } else {
        (
            "SELECT r.id, r.source_dictionary_id, r.source_field_path,
                    r.target_dictionary_id, d.primary_field_path,
                    r.relation_name, r.reverse_name
             FROM data_dictionary_relations r
             JOIN data_dictionaries d ON d.id = r.target_dictionary_id
             WHERE r.source_dictionary_id = ?1
             ORDER BY r.id ASC",
            dictionary_id,
        )
    };
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("prepare data dictionary relation configs failed: {e}"))?;
    let rows = stmt
        .query_map(params![param_id], |row| {
            Ok(RelationConfig {
                id: row.get(0)?,
                source_dictionary_id: row.get(1)?,
                source_field_path: row.get(2)?,
                target_dictionary_id: row.get(3)?,
                target_primary_field_path: row.get(4)?,
                relation_name: row.get(5)?,
                reverse_name: row.get(6)?,
            })
        })
        .map_err(|e| format!("query data dictionary relation configs failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub(super) fn load_relation_seed_value(
    conn: &Connection,
    record_id: i64,
    dictionary_id: i64,
    field_path: &str,
) -> Result<Option<String>, String> {
    let row = conn
        .query_row(
            "SELECT value_type, normalized_value
             FROM data_dictionary_record_values
             WHERE record_id = ?1 AND dictionary_id = ?2 AND field_path = ?3",
            params![record_id, dictionary_id, field_path],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|e| format!("load relation seed value failed: {e}"))?;
    Ok(row.and_then(|(value_type, normalized_value)| {
        if is_relation_value_usable(&value_type, &normalized_value) {
            Some(normalized_value)
        } else {
            None
        }
    }))
}

pub(super) fn load_record_primary_usage_value(
    conn: &Connection,
    record_id: i64,
    dictionary_id: i64,
    primary_field_path: &str,
) -> Result<(String, String), String> {
    let row = conn
        .query_row(
            "SELECT value_type, value_text, normalized_value
             FROM data_dictionary_record_values
             WHERE record_id = ?1 AND dictionary_id = ?2 AND field_path = ?3",
            params![record_id, dictionary_id, primary_field_path],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("load record primary usage value failed: {e}"))?;

    let Some((value_type, value_text, normalized_value)) = row else {
        return Err("record primary value not found".to_string());
    };
    if !is_relation_value_usable(&value_type, &normalized_value) {
        return Err("record primary value is not usable".to_string());
    }
    Ok((value_text, normalized_value))
}

pub(super) fn load_record_row_by_primary_value(
    conn: &Connection,
    dictionary_id: i64,
    primary_field_path: &str,
    normalized_value: &str,
) -> Result<Option<RecordRow>, String> {
    conn.query_row(
        "SELECT r.id, r.dictionary_id, d.name, r.row_index, r.raw_json,
                d.title_field_path
         FROM data_dictionary_record_values v
         JOIN data_dictionary_records r ON r.id = v.record_id
         JOIN data_dictionaries d ON d.id = r.dictionary_id
         WHERE v.dictionary_id = ?1
           AND v.field_path = ?2
           AND v.normalized_value = ?3
           AND v.value_type IN ('string', 'number', 'boolean')
         ORDER BY r.sort_key COLLATE BINARY ASC, r.id ASC
         LIMIT 1",
        params![dictionary_id, primary_field_path, normalized_value],
        record_row,
    )
    .optional()
    .map_err(|e| format!("load data dictionary record by primary value failed: {e}"))
}

pub(super) fn load_related_record_briefs(
    conn: &Connection,
    dictionary_id: i64,
    field_path: &str,
    normalized_value: &str,
) -> Result<Vec<Value>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT r.id, r.dictionary_id, d.name, r.row_index, r.raw_json,
                    d.title_field_path
             FROM data_dictionary_record_values v
             JOIN data_dictionary_records r ON r.id = v.record_id
             JOIN data_dictionaries d ON d.id = r.dictionary_id
             WHERE v.dictionary_id = ?1
               AND v.field_path = ?2
               AND v.normalized_value = ?3
               AND v.value_type IN ('string', 'number', 'boolean')",
        )
        .map_err(|e| format!("prepare related data dictionary records failed: {e}"))?;
    let rows = stmt
        .query_map(
            params![dictionary_id, field_path, normalized_value],
            record_row,
        )
        .map_err(|e| format!("query related data dictionary records failed: {e}"))?;
    let mut record_rows = Vec::new();
    for row in rows {
        record_rows.push(row.map_err(|e| e.to_string())?);
    }
    let sort_config = load_dictionary_sort_config(conn, dictionary_id)?;
    if let Some((path, direction)) = sort_config.as_ref() {
        sort_record_rows(&mut record_rows, Some((path.as_str(), *direction)));
    } else {
        sort_record_rows(&mut record_rows, None);
    }
    let fields = load_field_configs(conn, dictionary_id)?;
    record_rows
        .into_iter()
        .map(|row| record_row_to_brief_json(row, &fields))
        .collect()
}

pub(super) fn rows_to_search_items(
    conn: &Connection,
    rows: Vec<RecordRow>,
    keyword: &str,
    include_raw_json: bool,
) -> Result<Vec<Value>, String> {
    let mut searchable_cache: HashMap<i64, Vec<String>> = HashMap::new();
    let mut field_cache: HashMap<i64, Vec<FieldConfig>> = HashMap::new();
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let paths = if let Some(paths) = searchable_cache.get(&row.dictionary_id) {
            paths.clone()
        } else {
            let paths = load_searchable_paths(conn, row.dictionary_id)?;
            searchable_cache.insert(row.dictionary_id, paths.clone());
            paths
        };
        let fields = if let Some(fields) = field_cache.get(&row.dictionary_id) {
            fields.clone()
        } else {
            let fields = load_field_configs(conn, row.dictionary_id)?;
            field_cache.insert(row.dictionary_id, fields.clone());
            fields
        };
        out.push(record_row_to_search_item_json(
            row,
            &paths,
            &fields,
            keyword,
            include_raw_json,
        )?);
    }
    Ok(out)
}

pub(super) fn load_searchable_paths(
    conn: &Connection,
    dictionary_id: i64,
) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT field_path
             FROM data_dictionary_fields
             WHERE dictionary_id = ?1 AND searchable = 1
             ORDER BY sort_order ASC, field_path ASC",
        )
        .map_err(|e| format!("prepare searchable fields failed: {e}"))?;
    let rows = stmt
        .query_map(params![dictionary_id], |row| row.get::<_, String>(0))
        .map_err(|e| format!("query searchable fields failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub(super) fn load_indexed_records_for_dictionary(
    conn: &Connection,
    dictionary_id: i64,
) -> Result<Vec<IndexedRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, row_index, raw_json
             FROM data_dictionary_records
             WHERE dictionary_id = ?1
             ORDER BY row_index ASC, id ASC",
        )
        .map_err(|e| format!("prepare dictionary records load failed: {e}"))?;
    let rows = stmt
        .query_map(params![dictionary_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("query dictionary records load failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (record_id, row_index, raw_json) = row.map_err(|e| e.to_string())?;
        let value = serde_json::from_str::<Value>(&raw_json)
            .map_err(|e| format!("parse data dictionary record {record_id} failed: {e}"))?;
        out.push(IndexedRecord {
            source_row_index: row_index,
            value,
        });
    }
    Ok(out)
}

pub(super) fn load_field_configs(
    conn: &Connection,
    dictionary_id: i64,
) -> Result<Vec<FieldConfig>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT field_path, display_name, meaning, searchable, visible, sort_order, type_hint, sample_value, present_count
             FROM data_dictionary_fields
             WHERE dictionary_id = ?1
             ORDER BY sort_order ASC, field_path ASC",
        )
        .map_err(|e| format!("prepare field configs failed: {e}"))?;
    let rows = stmt
        .query_map(params![dictionary_id], field_config_row)
        .map_err(|e| format!("query field configs failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub(super) fn load_relation_jsons(
    conn: &Connection,
    source_dictionary_id: i64,
) -> Result<Vec<Value>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT r.id, r.source_dictionary_id, r.source_field_path,
                    r.target_dictionary_id, d.name, d.primary_field_path,
                    r.relation_name, r.reverse_name
             FROM data_dictionary_relations r
             JOIN data_dictionaries d ON d.id = r.target_dictionary_id
             WHERE r.source_dictionary_id = ?1
             ORDER BY r.id ASC",
        )
        .map_err(|e| format!("prepare data dictionary relations failed: {e}"))?;
    let rows = stmt
        .query_map(params![source_dictionary_id], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "sourceDictionaryId": row.get::<_, i64>(1)?,
                "sourceFieldPath": row.get::<_, String>(2)?,
                "targetDictionaryId": row.get::<_, i64>(3)?,
                "targetDictionaryName": row.get::<_, String>(4)?,
                "targetPrimaryFieldPath": row.get::<_, Option<String>>(5)?,
                "relationName": row.get::<_, String>(6)?,
                "reverseName": row.get::<_, String>(7)?,
            }))
        })
        .map_err(|e| format!("query data dictionary relations failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub(super) fn load_field_config_map(
    conn: &Connection,
    dictionary_id: i64,
) -> Result<HashMap<String, FieldConfig>, String> {
    Ok(load_field_configs(conn, dictionary_id)?
        .into_iter()
        .map(|config| (config.field_path.clone(), config))
        .collect())
}

pub(super) fn load_dictionary_sort_config(
    conn: &Connection,
    dictionary_id: i64,
) -> Result<Option<(String, SortDirection)>, String> {
    let row = conn
        .query_row(
            "SELECT sort_field_path, sort_direction
             FROM data_dictionaries
             WHERE id = ?1",
            params![dictionary_id],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|e| format!("load dictionary sort config failed: {e}"))?
        .ok_or("dictionary not found")?;
    let Some(path) = row.0.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    Ok(Some((path, parse_sort_direction(Some(row.1.as_str()))?)))
}

pub(super) fn load_dictionary_primary_field_path(
    conn: &Connection,
    dictionary_id: i64,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT primary_field_path
         FROM data_dictionaries
         WHERE id = ?1",
        params![dictionary_id],
        |row| row.get::<_, Option<String>>(0),
    )
    .optional()
    .map_err(|e| format!("load dictionary primary field failed: {e}"))?
    .ok_or("dictionary not found".to_string())
}

pub(super) fn mark_field_value_index_ready(
    conn: &Connection,
    dictionary_id: i64,
) -> Result<(), String> {
    conn.execute(
        "UPDATE data_dictionaries
         SET field_value_indexed_at = CURRENT_TIMESTAMP
         WHERE id = ?1",
        params![dictionary_id],
    )
    .map_err(|e| format!("mark data dictionary value index ready failed: {e}"))?;
    Ok(())
}

pub(super) fn ensure_field_value_index_ready(
    conn: &Connection,
    dictionary_id: i64,
) -> Result<(), String> {
    let row = conn
        .query_row(
            "SELECT name, field_value_indexed_at
             FROM data_dictionaries
             WHERE id = ?1",
            params![dictionary_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map_err(|e| format!("check data dictionary value index failed: {e}"))?
        .ok_or("dictionary not found".to_string())?;
    if row.1.is_none() {
        return Err(format!("字段值索引缺失，请先对“{}”执行重建索引", row.0));
    }
    Ok(())
}

pub(super) fn ensure_scope_sort_keys_ready(
    conn: &Connection,
    scope: &SearchScope,
) -> Result<(), String> {
    let dictionary_ids = load_dictionaries_with_empty_sort_keys(conn, scope)?;
    for dictionary_id in dictionary_ids {
        let sort_config = load_dictionary_sort_config(conn, dictionary_id)?;
        let sort_config_ref = sort_config
            .as_ref()
            .map(|(path, direction)| (path.as_str(), *direction));
        refresh_dictionary_sort_keys(conn, dictionary_id, sort_config_ref)?;
    }
    Ok(())
}

pub(super) fn load_dictionaries_with_empty_sort_keys(
    conn: &Connection,
    scope: &SearchScope,
) -> Result<Vec<i64>, String> {
    let (sql, params_list): (&str, Vec<SqlParam>) = match scope {
        SearchScope::Current(dictionary_id) => (
            "SELECT DISTINCT dictionary_id
             FROM data_dictionary_records
             WHERE dictionary_id = ?1 AND sort_key = ''
             ORDER BY dictionary_id ASC",
            vec![Box::new(*dictionary_id)],
        ),
        SearchScope::All => (
            "SELECT DISTINCT dictionary_id
             FROM data_dictionary_records
             WHERE sort_key = ''
             ORDER BY dictionary_id ASC",
            Vec::new(),
        ),
    };
    let refs: Vec<&dyn ToSql> = params_list.iter().map(|param| param.as_ref()).collect();
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("prepare empty sort key scan failed: {e}"))?;
    let rows = stmt
        .query_map(refs.as_slice(), |row| row.get::<_, i64>(0))
        .map_err(|e| format!("query empty sort key scan failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub(super) fn refresh_dictionary_sort_keys(
    conn: &Connection,
    dictionary_id: i64,
    sort_config: Option<(&str, SortDirection)>,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, row_index, raw_json
             FROM data_dictionary_records
             WHERE dictionary_id = ?1
             ORDER BY row_index ASC, id ASC",
        )
        .map_err(|e| format!("prepare dictionary sort key refresh failed: {e}"))?;
    let rows = stmt
        .query_map(params![dictionary_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("query dictionary sort key refresh failed: {e}"))?;

    let mut updates = Vec::new();
    for row in rows {
        let (record_id, row_index, raw_json) = row.map_err(|e| e.to_string())?;
        let record = serde_json::from_str::<Value>(&raw_json)
            .map_err(|e| format!("parse data dictionary record {record_id} failed: {e}"))?;
        let sort_key = build_record_sort_key(&record, row_index, sort_config)?;
        updates.push((record_id, sort_key));
    }
    drop(stmt);

    for (record_id, sort_key) in updates {
        conn.execute(
            "UPDATE data_dictionary_records SET sort_key = ?1 WHERE id = ?2",
            params![sort_key, record_id],
        )
        .map_err(|e| format!("update data dictionary sort key failed: {e}"))?;
    }
    Ok(())
}

pub(super) fn rebuild_dictionary_indexes(
    conn: &Connection,
    dictionary_id: i64,
) -> Result<RebuildStats, String> {
    let primary_field_path = load_dictionary_primary_field_path(conn, dictionary_id)?;
    let searchable_paths = load_searchable_paths(conn, dictionary_id)?;
    let sort_config = load_dictionary_sort_config(conn, dictionary_id)?;
    let sort_config_ref = sort_config
        .as_ref()
        .map(|(path, direction)| (path.as_str(), *direction));
    let mut stmt = conn
        .prepare(
            "SELECT id, row_index, raw_json
             FROM data_dictionary_records
             WHERE dictionary_id = ?1
             ORDER BY row_index ASC, id ASC",
        )
        .map_err(|e| format!("prepare rebuild dictionary indexes failed: {e}"))?;
    let rows = stmt
        .query_map(params![dictionary_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("query rebuild dictionary indexes failed: {e}"))?;

    let mut records = Vec::new();
    for row in rows {
        let (record_id, row_index, raw_json) = row.map_err(|e| e.to_string())?;
        let value = serde_json::from_str::<Value>(&raw_json)
            .map_err(|e| format!("parse data dictionary record {record_id} failed: {e}"))?;
        records.push((record_id, row_index, raw_json, value));
    }

    conn.execute(
        "DELETE FROM data_dictionary_record_values WHERE dictionary_id = ?1",
        params![dictionary_id],
    )
    .map_err(|e| format!("delete old data dictionary record values failed: {e}"))?;

    let mut seen_primary_values = HashSet::new();
    let mut skipped_invalid_count = 0;
    let mut skipped_duplicate_count = 0;
    let mut value_count = 0;
    for (record_id, row_index, raw_json, value) in &records {
        let fields = flatten_record(value);
        let search_text = build_search_text(&fields, &searchable_paths);
        let normalized = normalize_search_text(&search_text);
        let sort_key = build_record_sort_key(value, *row_index, sort_config_ref)?;
        conn.execute(
            "UPDATE data_dictionary_records
             SET search_text = ?1, normalized_search_text = ?2, sort_key = ?3
             WHERE id = ?4",
            params![search_text, normalized, sort_key, record_id],
        )
        .map_err(|e| format!("update rebuilt search text failed: {e}"))?;

        let mut excluded_primary = false;
        if let Some(primary_field_path) = primary_field_path.as_deref() {
            let primary_value = get_value_by_field_path(value, primary_field_path);
            if let Some(normalized_primary) = primary_value.and_then(normalized_primary_key) {
                if !seen_primary_values.insert(normalized_primary) {
                    skipped_duplicate_count += 1;
                    excluded_primary = true;
                }
            } else {
                skipped_invalid_count += 1;
                excluded_primary = true;
            }
        }
        value_count += insert_record_values(
            conn,
            build_record_values(*record_id, dictionary_id, raw_json)?,
            if excluded_primary {
                primary_field_path.as_deref()
            } else {
                None
            },
        )?;
    }
    mark_field_value_index_ready(conn, dictionary_id)?;

    Ok(RebuildStats {
        record_count: records.len(),
        value_count,
        skipped_invalid_count,
        skipped_duplicate_count,
    })
}

pub(super) fn ensure_dictionary_exists(
    conn: &Connection,
    dictionary_id: i64,
) -> Result<(), String> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM data_dictionaries WHERE id = ?1)",
            params![dictionary_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("check data dictionary failed: {e}"))?;
    if exists {
        Ok(())
    } else {
        Err("dictionary not found".to_string())
    }
}

pub(super) fn dictionary_row_to_json(row: &Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": row.get::<_, i64>(0)?,
        "name": row.get::<_, String>(1)?,
        "description": row.get::<_, String>(2)?,
        "recordCount": row.get::<_, i64>(3)?,
        "createdAt": row.get::<_, String>(4)?,
        "updatedAt": row.get::<_, String>(5)?,
        "primaryFieldPath": row.get::<_, Option<String>>(6)?,
        "titleFieldPath": row.get::<_, Option<String>>(7)?,
        "sortFieldPath": row.get::<_, Option<String>>(8)?,
        "sortDirection": row.get::<_, String>(9)?,
        "navOrder": row.get::<_, i64>(10)?,
    }))
}

pub(super) fn field_config_row(row: &Row<'_>) -> rusqlite::Result<FieldConfig> {
    Ok(FieldConfig {
        field_path: row.get(0)?,
        display_name: row.get(1)?,
        meaning: row.get(2)?,
        searchable: row.get::<_, i64>(3)? != 0,
        visible: row.get::<_, i64>(4)? != 0,
        sort_order: row.get(5)?,
        type_hint: row.get(6)?,
        sample_value: row.get(7)?,
        present_count: row.get(8)?,
    })
}

pub(super) fn record_row(row: &Row<'_>) -> rusqlite::Result<RecordRow> {
    Ok(RecordRow {
        id: row.get(0)?,
        dictionary_id: row.get(1)?,
        dictionary_name: row.get(2)?,
        row_index: row.get(3)?,
        raw_json: row.get(4)?,
        title_field_path: row.get(5)?,
    })
}

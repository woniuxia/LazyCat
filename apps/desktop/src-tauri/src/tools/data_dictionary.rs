use super::helpers::db_conn;
use rusqlite::{params, Connection, OptionalExtension, Row, ToSql};
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

type SqlParam = Box<dyn ToSql>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct FlattenedField {
    path: String,
    value_text: String,
    type_hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldStat {
    path: String,
    type_hint: String,
    sample_value: String,
    present_count: i64,
    sort_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldConfig {
    field_path: String,
    display_name: String,
    meaning: String,
    searchable: bool,
    visible: bool,
    sort_order: i64,
    type_hint: String,
    sample_value: String,
    present_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SearchScope {
    Current(i64),
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug)]
struct RecordRow {
    id: i64,
    dictionary_id: i64,
    dictionary_name: String,
    title_field_path: Option<String>,
    row_index: i64,
    raw_json: String,
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "list" => action_list(),
        "get" => action_get(payload),
        "import_preview" => action_import_preview(payload),
        "create" => action_create(payload),
        "rename" => action_rename(payload),
        "replace_records" => action_replace_records(payload),
        "update_fields" => action_update_fields(payload),
        "reorder" => action_reorder(payload),
        "search" => action_search(payload),
        "delete" => action_delete(payload),
        _ => Err(format!("unsupported data dictionary action: {action}")),
    }
}

fn action_list() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, description, record_count, created_at, updated_at
             , title_field_path, sort_field_path, sort_direction, nav_order
             FROM data_dictionaries
             ORDER BY nav_order ASC, updated_at DESC, id DESC",
        )
        .map_err(|e| format!("prepare data dictionary list failed: {e}"))?;
    let rows = stmt
        .query_map([], dictionary_row_to_json)
        .map_err(|e| format!("query data dictionary list failed: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(json!({ "items": out }))
}

fn action_get(payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;
    let dictionary = conn
        .query_row(
            "SELECT id, name, description, record_count, created_at, updated_at
             , title_field_path, sort_field_path, sort_direction, nav_order
             FROM data_dictionaries
             WHERE id = ?1",
            params![id],
            dictionary_row_to_json,
        )
        .optional()
        .map_err(|e| format!("get data dictionary failed: {e}"))?
        .ok_or("dictionary not found")?;
    let fields = load_field_configs(&conn, id)?;
    Ok(json!({
        "dictionary": dictionary,
        "fields": fields.iter().map(field_config_to_json).collect::<Vec<_>>(),
    }))
}

fn action_import_preview(payload: &Value) -> Result<Value, String> {
    let input = payload["input"].as_str().unwrap_or_default();
    let records = parse_import_array(input)?;
    let stats = collect_field_stats(&records);
    Ok(json!({
        "recordCount": records.len(),
        "fields": stats.iter().map(field_stat_to_json).collect::<Vec<_>>(),
    }))
}

fn action_create(payload: &Value) -> Result<Value, String> {
    let name = payload["name"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("name is required")?;
    let description = payload["description"].as_str().unwrap_or("").trim();
    let input = payload["input"].as_str().unwrap_or_default();
    let records = parse_import_array(input)?;
    let stats = collect_field_stats(&records);
    let searchable_paths: Vec<String> = stats.iter().map(|stat| stat.path.clone()).collect();

    let mut conn = db_conn()?;
    let nav_order = conn
        .query_row(
            "SELECT COALESCE(MAX(nav_order), -1) + 1 FROM data_dictionaries",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| format!("load next data dictionary order failed: {e}"))?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("create dictionary transaction failed: {e}"))?;
    tx.execute(
        "INSERT INTO data_dictionaries (name, description, record_count, nav_order)
         VALUES (?1, ?2, ?3, ?4)",
        params![name, description, records.len() as i64, nav_order],
    )
    .map_err(|e| format!("insert data dictionary failed: {e}"))?;
    let dictionary_id = tx.last_insert_rowid();

    for (idx, stat) in stats.iter().enumerate() {
        tx.execute(
            "INSERT INTO data_dictionary_fields
             (dictionary_id, field_path, display_name, meaning, searchable, visible, sort_order, type_hint, sample_value, present_count)
             VALUES (?1, ?2, ?3, '', 1, ?4, ?5, ?6, ?7, ?8)",
            params![
                dictionary_id,
                stat.path,
                default_display_name(&stat.path),
                if idx < 6 { 1_i64 } else { 0_i64 },
                stat.sort_order,
                stat.type_hint,
                stat.sample_value,
                stat.present_count,
            ],
        )
        .map_err(|e| format!("insert data dictionary field failed: {e}"))?;
    }

    insert_records(&tx, dictionary_id, &records, &searchable_paths)?;
    tx.commit()
        .map_err(|e| format!("commit create dictionary failed: {e}"))?;
    rebuild_fts_for_dictionary(&conn, dictionary_id);

    Ok(json!({ "ok": true, "id": dictionary_id }))
}

fn action_rename(payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let name = payload["name"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("name is required")?;
    let conn = db_conn()?;
    if let Some(description) = payload.get("description").and_then(Value::as_str) {
        conn.execute(
            "UPDATE data_dictionaries
             SET name = ?1, description = ?2, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?3",
            params![name, description.trim(), id],
        )
    } else {
        conn.execute(
            "UPDATE data_dictionaries
             SET name = ?1, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?2",
            params![name, id],
        )
    }
    .map_err(|e| format!("rename data dictionary failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn action_replace_records(payload: &Value) -> Result<Value, String> {
    let dictionary_id = payload["dictionaryId"]
        .as_i64()
        .ok_or("dictionaryId is required")?;
    let input = payload["input"].as_str().unwrap_or_default();
    let records = parse_import_array(input)?;
    let stats = collect_field_stats(&records);

    let mut conn = db_conn()?;
    ensure_dictionary_exists(&conn, dictionary_id)?;
    let old_configs = load_field_config_map(&conn, dictionary_id)?;
    let new_paths: HashSet<String> = stats.iter().map(|stat| stat.path.clone()).collect();
    let mut searchable_paths = Vec::new();

    let tx = conn
        .transaction()
        .map_err(|e| format!("replace records transaction failed: {e}"))?;
    tx.execute(
        "DELETE FROM data_dictionary_records WHERE dictionary_id = ?1",
        params![dictionary_id],
    )
    .map_err(|e| format!("delete old dictionary records failed: {e}"))?;
    delete_fts_for_dictionary(&tx, dictionary_id);

    for stat in &stats {
        if let Some(existing) = old_configs.get(&stat.path) {
            if existing.searchable {
                searchable_paths.push(stat.path.clone());
            }
            tx.execute(
                "UPDATE data_dictionary_fields
                 SET type_hint = ?1, sample_value = ?2, present_count = ?3, updated_at = CURRENT_TIMESTAMP
                 WHERE dictionary_id = ?4 AND field_path = ?5",
                params![
                    stat.type_hint,
                    stat.sample_value,
                    stat.present_count,
                    dictionary_id,
                    stat.path,
                ],
            )
            .map_err(|e| format!("update retained field stats failed: {e}"))?;
        } else {
            searchable_paths.push(stat.path.clone());
            tx.execute(
                "INSERT INTO data_dictionary_fields
                 (dictionary_id, field_path, display_name, meaning, searchable, visible, sort_order, type_hint, sample_value, present_count)
                 VALUES (?1, ?2, ?3, '', 1, 0, ?4, ?5, ?6, ?7)",
                params![
                    dictionary_id,
                    stat.path,
                    default_display_name(&stat.path),
                    stat.sort_order,
                    stat.type_hint,
                    stat.sample_value,
                    stat.present_count,
                ],
            )
            .map_err(|e| format!("insert new replacement field failed: {e}"))?;
        }
    }

    for path in old_configs.keys().filter(|path| !new_paths.contains(*path)) {
        tx.execute(
            "UPDATE data_dictionary_fields
             SET present_count = 0, visible = 0, updated_at = CURRENT_TIMESTAMP
             WHERE dictionary_id = ?1 AND field_path = ?2",
            params![dictionary_id, path],
        )
        .map_err(|e| format!("mark missing replacement field failed: {e}"))?;
    }

    insert_records(&tx, dictionary_id, &records, &searchable_paths)?;
    tx.execute(
        "UPDATE data_dictionaries
         SET record_count = ?1, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?2",
        params![records.len() as i64, dictionary_id],
    )
    .map_err(|e| format!("update replacement count failed: {e}"))?;
    tx.commit()
        .map_err(|e| format!("commit replace records failed: {e}"))?;
    rebuild_fts_for_dictionary(&conn, dictionary_id);
    Ok(json!({ "ok": true, "recordCount": records.len() }))
}

fn action_update_fields(payload: &Value) -> Result<Value, String> {
    let dictionary_id = payload["dictionaryId"]
        .as_i64()
        .ok_or("dictionaryId is required")?;
    let fields = payload["fields"]
        .as_array()
        .ok_or("fields is required")?;
    let mut seen = HashSet::new();
    let sort_direction = parse_sort_direction(payload["sortDirection"].as_str())?;
    let mut conn = db_conn()?;
    ensure_dictionary_exists(&conn, dictionary_id)?;

    let tx = conn
        .transaction()
        .map_err(|e| format!("update fields transaction failed: {e}"))?;
    for field in fields {
        let field_path = field["fieldPath"]
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or("fieldPath is required")?;
        if !seen.insert(field_path.to_string()) {
            return Err(format!("duplicate fieldPath: {field_path}"));
        }
        let display_name = field["displayName"].as_str().unwrap_or("").trim();
        let meaning = field["meaning"].as_str().unwrap_or("").trim();
        let searchable = bool_to_i64(field["searchable"].as_bool().unwrap_or(true));
        let visible = bool_to_i64(field["visible"].as_bool().unwrap_or(true));
        let sort_order = field["sortOrder"].as_i64().unwrap_or(0);
        tx.execute(
            "UPDATE data_dictionary_fields
             SET display_name = ?1, meaning = ?2, searchable = ?3, visible = ?4, sort_order = ?5, updated_at = CURRENT_TIMESTAMP
             WHERE dictionary_id = ?6 AND field_path = ?7",
            params![
                display_name,
                meaning,
                searchable,
                visible,
                sort_order,
                dictionary_id,
                field_path,
            ],
        )
        .map_err(|e| format!("update data dictionary field failed: {e}"))?;
    }
    let title_field_path = parse_configured_field_path(payload, "titleFieldPath", &seen)?;
    let sort_field_path = parse_configured_field_path(payload, "sortFieldPath", &seen)?;
    tx.execute(
        "UPDATE data_dictionaries
         SET title_field_path = ?1, sort_field_path = ?2, sort_direction = ?3, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?4",
        params![
            title_field_path,
            sort_field_path,
            sort_direction.as_str(),
            dictionary_id,
        ],
    )
    .map_err(|e| format!("update data dictionary sort config failed: {e}"))?;
    tx.commit()
        .map_err(|e| format!("commit update fields failed: {e}"))?;
    rebuild_record_search_text(&conn, dictionary_id)?;
    rebuild_fts_for_dictionary(&conn, dictionary_id);
    Ok(json!({ "ok": true }))
}

fn action_reorder(payload: &Value) -> Result<Value, String> {
    let updates = parse_reorder_dictionary_ids(payload)?;
    let mut conn = db_conn()?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("reorder dictionaries transaction failed: {e}"))?;

    for (id, nav_order) in updates {
        let changed = tx
            .execute(
                "UPDATE data_dictionaries SET nav_order = ?1 WHERE id = ?2",
                params![nav_order, id],
            )
            .map_err(|e| format!("reorder data dictionary failed: {e}"))?;
        if changed != 1 {
            return Err("dictionary not found".to_string());
        }
    }

    tx.commit()
        .map_err(|e| format!("commit reorder dictionaries failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn action_search(payload: &Value) -> Result<Value, String> {
    let scope = parse_search_scope(payload)?;
    let keyword = payload["keyword"].as_str().unwrap_or("").trim();
    let limit = parse_limit(payload);
    let fetch_limit = limit + 1;
    let conn = db_conn()?;
    if let SearchScope::Current(id) = scope {
        ensure_dictionary_exists(&conn, id)?;
    }
    let sort_config = match scope {
        SearchScope::Current(id) => load_dictionary_sort_config(&conn, id)?,
        SearchScope::All => None,
    };
    let query_limit = if sort_config.is_some() {
        None
    } else {
        Some(fetch_limit)
    };

    let mut rows = if keyword.is_empty() {
        query_empty_records(&conn, &scope, query_limit)?
    } else {
        query_like_records(&conn, &scope, keyword, query_limit)?
    };

    if !keyword.is_empty() && rows.len() < fetch_limit as usize && data_dictionary_has_fts(&conn) {
        if let Some(fts_keyword) = build_fts_keyword(keyword) {
            if let Ok(fts_rows) = query_fts_records(&conn, &scope, &fts_keyword, query_limit) {
                let mut seen: HashSet<i64> = rows.iter().map(|row| row.id).collect();
                for row in fts_rows {
                    if seen.insert(row.id) {
                        rows.push(row);
                        if rows.len() >= fetch_limit as usize {
                            break;
                        }
                    }
                }
            }
        }
    }

    if let Some((path, direction)) = sort_config.as_ref() {
        sort_record_rows(&mut rows, Some((path.as_str(), *direction)));
    }
    let (rows, has_more) = split_limited_rows(rows, limit as usize);
    let items = rows_to_search_items(&conn, rows, keyword)?;
    Ok(json!({ "items": items, "hasMore": has_more }))
}

fn action_delete(payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;
    delete_fts_for_dictionary(&conn, id);
    conn.execute("DELETE FROM data_dictionaries WHERE id = ?1", params![id])
        .map_err(|e| format!("delete data dictionary failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn parse_import_array(input: &str) -> Result<Vec<Value>, String> {
    let value = serde_json::from_str::<Value>(input)
        .map_err(|e| format!("JSON 解析失败: {e}"))?;
    let array = value.as_array().ok_or("请输入 JSON array")?;
    let mut records = Vec::with_capacity(array.len());
    for (idx, item) in array.iter().enumerate() {
        if !item.is_object() {
            return Err(format!("第 {} 行不是 JSON object", idx + 1));
        }
        records.push(item.clone());
    }
    Ok(records)
}

fn collect_field_stats(records: &[Value]) -> Vec<FieldStat> {
    let mut map: BTreeMap<String, FieldStat> = BTreeMap::new();
    for record in records {
        for field in flatten_record(record) {
            let entry = map.entry(field.path.clone()).or_insert_with(|| FieldStat {
                path: field.path.clone(),
                type_hint: field.type_hint.clone(),
                sample_value: field.value_text.clone(),
                present_count: 0,
                sort_order: 0,
            });
            entry.present_count += 1;
            if entry.sample_value.is_empty() && !field.value_text.is_empty() {
                entry.sample_value = field.value_text.clone();
            }
            if entry.type_hint != field.type_hint {
                entry.type_hint = "mixed".to_string();
            }
        }
    }

    map.into_values()
        .enumerate()
        .map(|(idx, mut stat)| {
            stat.sort_order = idx as i64;
            stat
        })
        .collect()
}

fn flatten_record(record: &Value) -> Vec<FlattenedField> {
    let mut fields = Vec::new();
    if let Value::Object(map) = record {
        flatten_object(map, "", &mut fields);
    }
    fields.sort_by(|a, b| a.path.cmp(&b.path));
    fields
}

fn flatten_object(map: &serde_json::Map<String, Value>, prefix: &str, fields: &mut Vec<FlattenedField>) {
    for (key, value) in map {
        let segment = escape_path_segment(key);
        let path = if prefix.is_empty() {
            segment
        } else {
            format!("{prefix}.{segment}")
        };
        match value {
            Value::Object(child) if !child.is_empty() => flatten_object(child, &path, fields),
            _ => fields.push(FlattenedField {
                path,
                value_text: value_to_search_text(value),
                type_hint: value_type_hint(value).to_string(),
            }),
        }
    }
}

fn escape_path_segment(segment: &str) -> String {
    let mut out = String::new();
    for ch in segment.chars() {
        match ch {
            '\\' => out.push_str(r#"\\ "#.trim()),
            '.' => out.push_str(r#"\."#),
            _ => out.push(ch),
        }
    }
    out
}

fn unescape_path_segment(segment: &str) -> String {
    let mut out = String::new();
    let mut escaped = false;
    for ch in segment.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else {
            out.push(ch);
        }
    }
    if escaped {
        out.push('\\');
    }
    out
}

fn default_display_name(field_path: &str) -> String {
    let mut current = String::new();
    let mut last = String::new();
    let mut escaped = false;
    for ch in field_path.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '.' => {
                last = current;
                current = String::new();
            }
            _ => current.push(ch),
        }
    }
    if escaped {
        current.push('\\');
    }
    if current.is_empty() {
        unescape_path_segment(&last)
    } else {
        current
    }
}

fn value_to_search_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string(value).unwrap_or_else(|_| String::new())
        }
    }
}

fn value_type_hint(value: &Value) -> &'static str {
    match value {
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
        Value::Null => "null",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn normalize_search_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn escape_like_pattern(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            '%' | '_' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn build_search_text(fields: &[FlattenedField], searchable_paths: &[String]) -> String {
    let searchable: HashSet<&str> = searchable_paths.iter().map(String::as_str).collect();
    fields
        .iter()
        .filter(|field| searchable.contains(field.path.as_str()))
        .map(|field| field.value_text.as_str())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_search_scope(payload: &Value) -> Result<SearchScope, String> {
    match payload["scope"].as_str().unwrap_or_else(|| {
        if payload.get("dictionaryId").is_some() {
            "current"
        } else {
            "all"
        }
    }) {
        "all" => Ok(SearchScope::All),
        "current" => {
            let id = payload["dictionaryId"]
                .as_i64()
                .filter(|id| *id > 0)
                .ok_or("dictionaryId is required for current scope")?;
            Ok(SearchScope::Current(id))
        }
        other => Err(format!("unsupported search scope: {other}")),
    }
}

fn parse_reorder_dictionary_ids(payload: &Value) -> Result<Vec<(i64, i64)>, String> {
    let ids = payload["ids"].as_array().ok_or("ids is required")?;
    if ids.is_empty() {
        return Err("ids must not be empty".to_string());
    }

    let mut seen = HashSet::new();
    let mut updates = Vec::new();
    for value in ids {
        let id = value.as_i64().ok_or("ids must contain only numbers")?;
        if id <= 0 {
            return Err("ids must contain positive numbers".to_string());
        }
        if seen.insert(id) {
            updates.push((id, updates.len() as i64));
        }
    }

    if updates.is_empty() {
        return Err("ids must not be empty".to_string());
    }
    Ok(updates)
}

fn parse_configured_field_path(
    payload: &Value,
    key: &str,
    seen: &HashSet<String>,
) -> Result<Option<String>, String> {
    let path = payload[key]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if let Some(path) = path.as_deref() {
        if !seen.contains(path) {
            return Err(format!("{key} not found: {path}"));
        }
    }
    Ok(path)
}

fn parse_sort_direction(value: Option<&str>) -> Result<SortDirection, String> {
    match value.unwrap_or("asc") {
        "asc" => Ok(SortDirection::Asc),
        "desc" => Ok(SortDirection::Desc),
        other => Err(format!("unsupported sortDirection: {other}")),
    }
}

impl SortDirection {
    fn as_str(self) -> &'static str {
        match self {
            SortDirection::Asc => "asc",
            SortDirection::Desc => "desc",
        }
    }
}

fn compute_matches(record: &Value, searchable_paths: &[String], keyword: &str) -> Vec<Value> {
    let normalized_keyword = normalize_search_text(keyword);
    if normalized_keyword.is_empty() {
        return Vec::new();
    }
    let searchable: HashSet<&str> = searchable_paths.iter().map(String::as_str).collect();
    flatten_record(record)
        .into_iter()
        .filter(|field| searchable.contains(field.path.as_str()))
        .filter(|field| normalize_search_text(&field.value_text).contains(&normalized_keyword))
        .map(|field| json!({ "fieldPath": field.path, "value": field.value_text }))
        .collect()
}

fn parse_limit(payload: &Value) -> i64 {
    payload["limit"].as_i64().unwrap_or(100).clamp(1, 500)
}

fn insert_records(
    conn: &Connection,
    dictionary_id: i64,
    records: &[Value],
    searchable_paths: &[String],
) -> Result<(), String> {
    for (idx, record) in records.iter().enumerate() {
        let fields = flatten_record(record);
        let search_text = build_search_text(&fields, searchable_paths);
        let normalized = normalize_search_text(&search_text);
        let raw_json = serde_json::to_string(record)
            .map_err(|e| format!("serialize data dictionary record failed: {e}"))?;
        conn.execute(
            "INSERT INTO data_dictionary_records
             (dictionary_id, row_index, raw_json, search_text, normalized_search_text)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![dictionary_id, idx as i64, raw_json, search_text, normalized],
        )
        .map_err(|e| format!("insert data dictionary record failed: {e}"))?;
    }
    Ok(())
}

fn rebuild_record_search_text(conn: &Connection, dictionary_id: i64) -> Result<(), String> {
    let searchable_paths = load_searchable_paths(conn, dictionary_id)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, raw_json
             FROM data_dictionary_records
             WHERE dictionary_id = ?1",
        )
        .map_err(|e| format!("prepare rebuild search text failed: {e}"))?;
    let rows = stmt
        .query_map(params![dictionary_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("query rebuild search text failed: {e}"))?;
    let mut updates = Vec::new();
    for row in rows {
        let (id, raw_json) = row.map_err(|e| e.to_string())?;
        let record = serde_json::from_str::<Value>(&raw_json).unwrap_or(Value::Null);
        let fields = flatten_record(&record);
        let search_text = build_search_text(&fields, &searchable_paths);
        let normalized = normalize_search_text(&search_text);
        updates.push((id, search_text, normalized));
    }
    for (id, search_text, normalized) in updates {
        conn.execute(
            "UPDATE data_dictionary_records
             SET search_text = ?1, normalized_search_text = ?2
             WHERE id = ?3",
            params![search_text, normalized, id],
        )
        .map_err(|e| format!("update rebuilt search text failed: {e}"))?;
    }
    Ok(())
}

fn query_empty_records(
    conn: &Connection,
    scope: &SearchScope,
    limit: Option<i64>,
) -> Result<Vec<RecordRow>, String> {
    query_records(conn, scope, Vec::new(), Vec::new(), limit, true)
}

fn query_like_records(
    conn: &Connection,
    scope: &SearchScope,
    keyword: &str,
    limit: Option<i64>,
) -> Result<Vec<RecordRow>, String> {
    let normalized = normalize_search_text(keyword);
    let pattern = format!("%{}%", escape_like_pattern(&normalized));
    query_records(
        conn,
        scope,
        vec!["r.normalized_search_text LIKE ? ESCAPE '\\'".to_string()],
        vec![Box::new(pattern)],
        limit,
        false,
    )
}

fn query_fts_records(
    conn: &Connection,
    scope: &SearchScope,
    fts_keyword: &str,
    limit: Option<i64>,
) -> Result<Vec<RecordRow>, String> {
    query_records(
        conn,
        scope,
        vec![
            "r.id IN (SELECT record_id FROM data_dictionary_fts WHERE data_dictionary_fts MATCH ?)"
                .to_string(),
        ],
        vec![Box::new(fts_keyword.to_string())],
        limit,
        false,
    )
}

fn query_records(
    conn: &Connection,
    scope: &SearchScope,
    mut conditions: Vec<String>,
    mut params_list: Vec<SqlParam>,
    limit: Option<i64>,
    empty_keyword: bool,
) -> Result<Vec<RecordRow>, String> {
    if let SearchScope::Current(dictionary_id) = scope {
        conditions.insert(0, "r.dictionary_id = ?".to_string());
        params_list.insert(0, Box::new(*dictionary_id));
    }
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
    if empty_keyword {
        match scope {
            SearchScope::Current(_) => sql.push_str(" ORDER BY r.row_index ASC"),
            SearchScope::All => sql.push_str(" ORDER BY d.updated_at DESC, r.row_index ASC"),
        }
    } else {
        sql.push_str(" ORDER BY d.updated_at DESC, r.row_index ASC");
    }
    if let Some(limit) = limit {
        sql.push_str(" LIMIT ?");
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

fn sort_record_rows(rows: &mut [RecordRow], sort_config: Option<(&str, SortDirection)>) {
    let Some((field_path, direction)) = sort_config else {
        rows.sort_by(|a, b| {
            a.dictionary_id
                .cmp(&b.dictionary_id)
                .then_with(|| a.row_index.cmp(&b.row_index))
                .then_with(|| a.id.cmp(&b.id))
        });
        return;
    };

    rows.sort_by(|a, b| {
        let a_value = record_sort_value(&a.raw_json, field_path);
        let b_value = record_sort_value(&b.raw_json, field_path);
        compare_sort_values(a_value.as_ref(), b_value.as_ref(), direction)
            .then_with(|| a.row_index.cmp(&b.row_index))
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn record_sort_value(raw_json: &str, field_path: &str) -> Option<Value> {
    let record = serde_json::from_str::<Value>(raw_json).ok()?;
    get_value_by_field_path(&record, field_path).cloned()
}

fn get_value_by_field_path<'a>(source: &'a Value, field_path: &str) -> Option<&'a Value> {
    let mut current = source;
    for part in split_escaped_path(field_path) {
        current = current.as_object()?.get(&part)?;
    }
    Some(current)
}

fn split_escaped_path(field_path: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for ch in field_path.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '.' {
            parts.push(current);
            current = String::new();
            continue;
        }
        current.push(ch);
    }
    if escaped {
        current.push('\\');
    }
    parts.push(current);
    parts
}

fn compare_sort_values(
    a: Option<&Value>,
    b: Option<&Value>,
    direction: SortDirection,
) -> Ordering {
    let order = match (a, b) {
        (Some(a), Some(b)) => compare_present_sort_values(a, b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    };
    match direction {
        SortDirection::Asc => order,
        SortDirection::Desc => {
            if a.is_some() && b.is_some() {
                order.reverse()
            } else {
                order
            }
        }
    }
}

fn compare_present_sort_values(a: &Value, b: &Value) -> Ordering {
    match (a.as_f64(), b.as_f64()) {
        (Some(left), Some(right)) => {
            return left.partial_cmp(&right).unwrap_or(Ordering::Equal);
        }
        _ => {}
    }
    value_to_search_text(a).cmp(&value_to_search_text(b))
}

fn rows_to_search_items(
    conn: &Connection,
    rows: Vec<RecordRow>,
    keyword: &str,
) -> Result<Vec<Value>, String> {
    let mut searchable_cache: HashMap<i64, Vec<String>> = HashMap::new();
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let record = serde_json::from_str::<Value>(&row.raw_json).unwrap_or(Value::Null);
        let paths = if let Some(paths) = searchable_cache.get(&row.dictionary_id) {
            paths.clone()
        } else {
            let paths = load_searchable_paths(conn, row.dictionary_id)?;
            searchable_cache.insert(row.dictionary_id, paths.clone());
            paths
        };
        out.push(json!({
            "id": row.id,
            "dictionaryId": row.dictionary_id,
            "dictionaryName": row.dictionary_name,
            "titleFieldPath": row.title_field_path,
            "rowIndex": row.row_index,
            "rawJson": record,
            "matches": compute_matches(&record, &paths, keyword),
        }));
    }
    Ok(out)
}

fn split_limited_rows(mut rows: Vec<RecordRow>, limit: usize) -> (Vec<RecordRow>, bool) {
    if rows.len() > limit {
        rows.truncate(limit);
        (rows, true)
    } else {
        (rows, false)
    }
}

fn build_fts_keyword(keyword: &str) -> Option<String> {
    let parts = keyword
        .split_whitespace()
        .map(|part| part.trim_matches('"').replace('"', ""))
        .filter(|part| !part.is_empty())
        .map(|part| format!("\"{part}\""))
        .collect::<Vec<_>>();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

fn load_searchable_paths(conn: &Connection, dictionary_id: i64) -> Result<Vec<String>, String> {
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

fn load_field_configs(conn: &Connection, dictionary_id: i64) -> Result<Vec<FieldConfig>, String> {
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

fn load_field_config_map(
    conn: &Connection,
    dictionary_id: i64,
) -> Result<HashMap<String, FieldConfig>, String> {
    Ok(load_field_configs(conn, dictionary_id)?
        .into_iter()
        .map(|config| (config.field_path.clone(), config))
        .collect())
}

fn load_dictionary_sort_config(
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

fn ensure_dictionary_exists(conn: &Connection, dictionary_id: i64) -> Result<(), String> {
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

fn data_dictionary_has_fts(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT count(*) > 0 FROM sqlite_master WHERE type='table' AND name='data_dictionary_fts'",
        [],
        |row| row.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

fn rebuild_fts_for_dictionary(conn: &Connection, dictionary_id: i64) {
    if !data_dictionary_has_fts(conn) {
        return;
    }
    delete_fts_for_dictionary(conn, dictionary_id);
    let mut stmt = match conn.prepare(
        "SELECT id, search_text
         FROM data_dictionary_records
         WHERE dictionary_id = ?1",
    ) {
        Ok(stmt) => stmt,
        Err(err) => {
            eprintln!("prepare data dictionary fts rebuild failed: {err}");
            return;
        }
    };
    let rows = match stmt.query_map(params![dictionary_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(rows) => rows,
        Err(err) => {
            eprintln!("query data dictionary fts rebuild failed: {err}");
            return;
        }
    };
    for row in rows {
        let Ok((record_id, search_text)) = row else {
            continue;
        };
        if let Err(err) = conn.execute(
            "INSERT INTO data_dictionary_fts(record_id, dictionary_id, search_text)
             VALUES (?1, ?2, ?3)",
            params![record_id, dictionary_id, search_text],
        ) {
            eprintln!("insert data dictionary fts row failed: {err}");
            return;
        }
    }
}

fn delete_fts_for_dictionary(conn: &Connection, dictionary_id: i64) {
    if data_dictionary_has_fts(conn) {
        let _ = conn.execute(
            "DELETE FROM data_dictionary_fts WHERE dictionary_id = ?1",
            params![dictionary_id],
        );
    }
}

fn dictionary_row_to_json(row: &Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": row.get::<_, i64>(0)?,
        "name": row.get::<_, String>(1)?,
        "description": row.get::<_, String>(2)?,
        "recordCount": row.get::<_, i64>(3)?,
        "createdAt": row.get::<_, String>(4)?,
        "updatedAt": row.get::<_, String>(5)?,
        "titleFieldPath": row.get::<_, Option<String>>(6)?,
        "sortFieldPath": row.get::<_, Option<String>>(7)?,
        "sortDirection": row.get::<_, String>(8)?,
        "navOrder": row.get::<_, i64>(9)?,
    }))
}

fn field_config_row(row: &Row<'_>) -> rusqlite::Result<FieldConfig> {
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

fn record_row(row: &Row<'_>) -> rusqlite::Result<RecordRow> {
    Ok(RecordRow {
        id: row.get(0)?,
        dictionary_id: row.get(1)?,
        dictionary_name: row.get(2)?,
        row_index: row.get(3)?,
        raw_json: row.get(4)?,
        title_field_path: row.get(5)?,
    })
}

fn field_stat_to_json(stat: &FieldStat) -> Value {
    json!({
        "fieldPath": stat.path,
        "displayName": default_display_name(&stat.path),
        "typeHint": stat.type_hint,
        "sampleValue": stat.sample_value,
        "presentCount": stat.present_count,
        "sortOrder": stat.sort_order,
    })
}

fn field_config_to_json(config: &FieldConfig) -> Value {
    json!({
        "fieldPath": config.field_path,
        "displayName": config.display_name,
        "meaning": config.meaning,
        "searchable": config.searchable,
        "visible": config.visible,
        "sortOrder": config.sort_order,
        "typeHint": config.type_hint,
        "sampleValue": config.sample_value,
        "presentCount": config.present_count,
    })
}

fn bool_to_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_import_payload_rejects_invalid_json() {
        let err = parse_import_array("{ nope").expect_err("invalid json must fail");
        assert!(err.contains("JSON 解析失败"));
    }

    #[test]
    fn parse_import_payload_requires_array() {
        let err = parse_import_array(r#"{"id":1}"#).expect_err("object root must fail");
        assert!(err.contains("JSON array"));
    }

    #[test]
    fn parse_import_payload_requires_object_items() {
        let err = parse_import_array(r#"[{"id":1}, 2]"#).expect_err("scalar item must fail");
        assert!(err.contains("第 2 行"));
    }

    #[test]
    fn flatten_object_supports_nested_dot_path() {
        let fields = flatten_record(&json!({
            "id": 1,
            "user": { "name": "张三", "role": "admin" },
            "active": true
        }));

        let pairs: Vec<(String, String, String)> = fields
            .into_iter()
            .map(|field| (field.path, field.value_text, field.type_hint))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("active".to_string(), "true".to_string(), "boolean".to_string()),
                ("id".to_string(), "1".to_string(), "number".to_string()),
                ("user.name".to_string(), "张三".to_string(), "string".to_string()),
                ("user.role".to_string(), "admin".to_string(), "string".to_string()),
            ]
        );
    }

    #[test]
    fn flatten_object_escapes_dot_and_backslash_in_key() {
        let fields = flatten_record(&json!({
            "user.name": { "a\\b": "value" }
        }));
        assert_eq!(fields[0].path, r#"user\.name.a\\b"#);
    }

    #[test]
    fn flatten_object_treats_array_as_leaf() {
        let fields = flatten_record(&json!({ "tags": ["A", "B"] }));
        assert_eq!(fields[0].path, "tags");
        assert_eq!(fields[0].type_hint, "array");
        assert_eq!(fields[0].value_text, r#"["A","B"]"#);
    }

    #[test]
    fn build_search_text_uses_only_searchable_fields() {
        let fields = vec![
            FlattenedField {
                path: "id".to_string(),
                value_text: "1001".to_string(),
                type_hint: "number".to_string(),
            },
            FlattenedField {
                path: "secret".to_string(),
                value_text: "hidden".to_string(),
                type_hint: "string".to_string(),
            },
        ];

        assert_eq!(build_search_text(&fields, &["id".to_string()]), "1001");
    }

    #[test]
    fn normalize_search_text_lowercases_and_collapses_spaces() {
        assert_eq!(normalize_search_text("  Foo\tBAR \n Baz  "), "foo bar baz");
    }

    #[test]
    fn search_like_pattern_escapes_percent_underscore_and_backslash() {
        assert_eq!(escape_like_pattern(r#"a%b_c\d"#), r#"a\%b\_c\\d"#);
    }

    #[test]
    fn parse_search_scope_supports_current_and_all() {
        assert_eq!(
            parse_search_scope(&json!({ "scope": "current", "dictionaryId": 7 })).unwrap(),
            SearchScope::Current(7)
        );
        assert_eq!(
            parse_search_scope(&json!({ "scope": "all", "dictionaryId": 7 })).unwrap(),
            SearchScope::All
        );
        assert!(parse_search_scope(&json!({ "scope": "current" })).is_err());
    }

    #[test]
    fn compute_matches_returns_full_field_paths() {
        let matches = compute_matches(
            &json!({ "user": { "name": "张三" }, "code": "A001" }),
            &["user.name".to_string(), "code".to_string()],
            "张",
        );

        assert_eq!(matches, vec![json!({ "fieldPath": "user.name", "value": "张三" })]);
    }

    #[test]
    fn split_limited_rows_marks_truncated_results() {
        let rows = (0..3)
            .map(|idx| RecordRow {
                id: idx + 1,
                dictionary_id: 1,
                dictionary_name: "测试字典".to_string(),
                title_field_path: None,
                row_index: idx,
                raw_json: "{}".to_string(),
            })
            .collect::<Vec<_>>();

        let (limited, has_more) = split_limited_rows(rows, 2);

        assert!(has_more);
        assert_eq!(limited.len(), 2);
        assert_eq!(limited[1].row_index, 1);
    }

    #[test]
    fn sort_record_rows_orders_by_configured_number_field() {
        let mut rows = vec![
            test_record_row(1, 0, json!({ "code": "B", "rank": 20 })),
            test_record_row(2, 1, json!({ "code": "A", "rank": 3 })),
            test_record_row(3, 2, json!({ "code": "C", "rank": 11 })),
        ];

        sort_record_rows(&mut rows, Some(("rank", SortDirection::Asc)));

        assert_eq!(
            rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![2, 3, 1]
        );
    }

    #[test]
    fn sort_record_rows_places_missing_values_last_in_both_directions() {
        let mut rows = vec![
            test_record_row(1, 0, json!({ "rank": 2 })),
            test_record_row(2, 1, json!({ "name": "missing" })),
            test_record_row(3, 2, json!({ "rank": 5 })),
        ];

        sort_record_rows(&mut rows, Some(("rank", SortDirection::Desc)));

        assert_eq!(
            rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![3, 1, 2]
        );
    }

    #[test]
    fn sort_record_rows_keeps_row_index_order_without_sort_config() {
        let mut rows = vec![
            test_record_row(3, 2, json!({ "rank": 11 })),
            test_record_row(1, 0, json!({ "rank": 20 })),
            test_record_row(2, 1, json!({ "rank": 3 })),
        ];

        sort_record_rows(&mut rows, None);

        assert_eq!(
            rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn parse_reorder_dictionary_ids_deduplicates_and_assigns_gapless_order() {
        let updates = parse_reorder_dictionary_ids(&json!({ "ids": [3, 1, 3, 2] })).unwrap();

        assert_eq!(updates, vec![(3, 0), (1, 1), (2, 2)]);
    }

    #[test]
    fn parse_reorder_dictionary_ids_rejects_empty_and_invalid_ids() {
        assert!(parse_reorder_dictionary_ids(&json!({ "ids": [] })).is_err());
        assert!(parse_reorder_dictionary_ids(&json!({ "ids": [1, 0] })).is_err());
        assert!(parse_reorder_dictionary_ids(&json!({ "ids": [1, "2"] })).is_err());
    }

    #[test]
    fn parse_configured_field_path_accepts_known_field_and_blank_default() {
        let seen = HashSet::from(["name".to_string(), "code".to_string()]);

        assert_eq!(
            parse_configured_field_path(
                &json!({ "titleFieldPath": " name " }),
                "titleFieldPath",
                &seen,
            )
            .unwrap(),
            Some("name".to_string())
        );
        assert_eq!(
            parse_configured_field_path(
                &json!({ "titleFieldPath": "" }),
                "titleFieldPath",
                &seen,
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn parse_configured_field_path_rejects_unknown_field() {
        let seen = HashSet::from(["name".to_string()]);

        let err = parse_configured_field_path(
            &json!({ "titleFieldPath": "missing" }),
            "titleFieldPath",
            &seen,
        )
        .expect_err("unknown configured field must fail");

        assert_eq!(err, "titleFieldPath not found: missing");
    }

    fn test_record_row(id: i64, row_index: i64, raw_json: Value) -> RecordRow {
        RecordRow {
            id,
            dictionary_id: 1,
            dictionary_name: "测试字典".to_string(),
            title_field_path: None,
            row_index,
            raw_json: serde_json::to_string(&raw_json).unwrap(),
        }
    }
}

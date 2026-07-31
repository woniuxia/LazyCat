use super::helpers::db_conn;
use super::usage::{self, UsageKey, ACTION_VIEW, RESOURCE_DATA_DICTIONARY_RECORD};
use rusqlite::{params, Connection, OptionalExtension, Row, ToSql};
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;

mod model;
mod path;

use model::*;
use path::*;

type SqlParam = Box<dyn ToSql>;

const ACTIONS: &[&str] = &[
    "list",
    "get",
    "import_preview",
    "create",
    "rename",
    "replace_records",
    "update_fields",
    "reorder",
    "search",
    "popular_records",
    "mark_record_used",
    "record_detail",
    "rebuild_indexes",
    "delete",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported data dictionary action: {action}"));
    }
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
        "popular_records" => action_popular_records(payload),
        "mark_record_used" => action_mark_record_used(payload),
        "record_detail" => action_record_detail(payload),
        "rebuild_indexes" => action_rebuild_indexes(payload),
        "delete" => action_delete(payload),
        _ => Err(format!("unsupported data dictionary action: {action}")),
    }
}

fn action_list() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, description, record_count, created_at, updated_at
             , primary_field_path, title_field_path, sort_field_path, sort_direction, nav_order
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
             , primary_field_path, title_field_path, sort_field_path, sort_direction, nav_order
             FROM data_dictionaries
             WHERE id = ?1",
            params![id],
            dictionary_row_to_json,
        )
        .optional()
        .map_err(|e| format!("get data dictionary failed: {e}"))?
        .ok_or("dictionary not found")?;
    let fields = load_field_configs(&conn, id)?;
    let relations = load_relation_jsons(&conn, id)?;
    Ok(json!({
        "dictionary": dictionary,
        "fields": fields.iter().map(field_config_to_json).collect::<Vec<_>>(),
        "relations": relations,
    }))
}

fn action_import_preview(payload: &Value) -> Result<Value, String> {
    let input = read_import_input(payload)?;
    let records = parse_import_array(&input)?;
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
    let primary_field_path = parse_required_field_path(payload, "primaryFieldPath")?;
    let input = read_import_input(payload)?;
    let raw_records = indexed_records_from_values(parse_import_array(&input)?);
    let raw_stats = collect_field_stats_from_indexed(&raw_records);
    let field_paths: HashSet<String> = raw_stats.iter().map(|stat| stat.path.clone()).collect();
    if !field_paths.contains(&primary_field_path) {
        return Err(format!(
            "primaryFieldPath does not exist: {primary_field_path}"
        ));
    }
    let partition = partition_records_by_primary(raw_records, Some(primary_field_path.as_str()))?;
    let skipped_invalid_count = partition.skipped_invalid_count;
    let skipped_duplicate_count = partition.skipped_duplicate_count;
    let skipped_record_count = partition.skipped_record_count();
    let records = partition.accepted_records;
    let stats = collect_field_stats_from_indexed(&records);
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
        "INSERT INTO data_dictionaries (name, description, record_count, primary_field_path, nav_order)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            name,
            description,
            records.len() as i64,
            primary_field_path,
            nav_order
        ],
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

    insert_records(&tx, dictionary_id, &records, &searchable_paths, None)?;
    mark_field_value_index_ready(&tx, dictionary_id)?;
    tx.commit()
        .map_err(|e| format!("commit create dictionary failed: {e}"))?;

    Ok(json!({
        "ok": true,
        "id": dictionary_id,
        "recordCount": records.len(),
        "skippedPrimaryRecordCount": skipped_record_count,
        "skippedPrimaryInvalidCount": skipped_invalid_count,
        "skippedPrimaryDuplicateCount": skipped_duplicate_count,
    }))
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
    let input = read_import_input(payload)?;
    let mut conn = db_conn()?;
    ensure_dictionary_exists(&conn, dictionary_id)?;
    let primary_field_path = load_dictionary_primary_field_path(&conn, dictionary_id)?;
    let partition = partition_records_by_primary(
        indexed_records_from_values(parse_import_array(&input)?),
        primary_field_path.as_deref(),
    )?;
    let skipped_invalid_count = partition.skipped_invalid_count;
    let skipped_duplicate_count = partition.skipped_duplicate_count;
    let skipped_record_count = partition.skipped_record_count();
    let records = partition.accepted_records;
    let stats = collect_field_stats_from_indexed(&records);

    let old_configs = load_field_config_map(&conn, dictionary_id)?;
    let sort_config = load_dictionary_sort_config(&conn, dictionary_id)?;
    let sort_config_ref = sort_config
        .as_ref()
        .map(|(path, direction)| (path.as_str(), *direction));
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

    insert_records(
        &tx,
        dictionary_id,
        &records,
        &searchable_paths,
        sort_config_ref,
    )?;
    tx.execute(
        "UPDATE data_dictionaries
         SET record_count = ?1, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?2",
        params![records.len() as i64, dictionary_id],
    )
    .map_err(|e| format!("update replacement count failed: {e}"))?;
    mark_field_value_index_ready(&tx, dictionary_id)?;
    tx.commit()
        .map_err(|e| format!("commit replace records failed: {e}"))?;
    Ok(json!({
        "ok": true,
        "recordCount": records.len(),
        "skippedPrimaryRecordCount": skipped_record_count,
        "skippedPrimaryInvalidCount": skipped_invalid_count,
        "skippedPrimaryDuplicateCount": skipped_duplicate_count,
    }))
}

fn action_update_fields(payload: &Value) -> Result<Value, String> {
    let dictionary_id = payload["dictionaryId"]
        .as_i64()
        .ok_or("dictionaryId is required")?;
    let fields = payload["fields"]
        .as_array()
        .ok_or("fields is required")?;
    require_non_empty_field_payload(fields)?;
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
    let primary_field_path = parse_configured_field_path(payload, "primaryFieldPath", &seen)?
        .ok_or_else(|| "primaryFieldPath is required".to_string())?;
    let title_field_path = parse_configured_field_path(payload, "titleFieldPath", &seen)?;
    let sort_field_path = parse_configured_field_path(payload, "sortFieldPath", &seen)?;
    let new_sort_config = sort_field_path
        .as_deref()
        .map(|path| (path, sort_direction));
    let old_primary_field_path = load_dictionary_primary_field_path(&tx, dictionary_id)?;
    let primary_changed = old_primary_field_path.as_deref() != Some(primary_field_path.as_str());
    let confirm_primary_prune = payload["confirmPrimaryPrune"].as_bool().unwrap_or(false);
    let relations = parse_relation_drafts(
        &tx,
        dictionary_id,
        &seen,
        Some(primary_field_path.as_str()),
        payload.get("relations").and_then(Value::as_array),
    )?;
    tx.execute(
        "UPDATE data_dictionaries
         SET primary_field_path = ?1, title_field_path = ?2, sort_field_path = ?3,
             sort_direction = ?4, updated_at = CURRENT_TIMESTAMP
        WHERE id = ?5",
        params![
            primary_field_path.as_str(),
            title_field_path.as_deref(),
            sort_field_path.as_deref(),
            sort_direction.as_str(),
            dictionary_id,
        ],
    )
    .map_err(|e| format!("update data dictionary sort config failed: {e}"))?;
    tx.execute(
        "DELETE FROM data_dictionary_relations WHERE source_dictionary_id = ?1",
        params![dictionary_id],
    )
    .map_err(|e| format!("delete old data dictionary relations failed: {e}"))?;
    for relation in relations {
        tx.execute(
            "INSERT INTO data_dictionary_relations
             (source_dictionary_id, source_field_path, target_dictionary_id, relation_name, reverse_name)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                dictionary_id,
                relation.source_field_path,
                relation.target_dictionary_id,
                relation.relation_name,
                relation.reverse_name,
            ],
        )
        .map_err(|e| format!("insert data dictionary relation failed: {e}"))?;
    }
    let existing_records = load_indexed_records_for_dictionary(&tx, dictionary_id)?;
    let partition =
        partition_records_by_primary(existing_records, Some(primary_field_path.as_str()))?;
    let skipped_invalid_count = partition.skipped_invalid_count;
    let skipped_duplicate_count = partition.skipped_duplicate_count;
    let skipped_record_count = partition.skipped_record_count();
    if primary_changed && skipped_record_count > 0 && !confirm_primary_prune {
        return Err(json!({
            "code": "PRIMARY_PRUNE_CONFIRMATION_REQUIRED",
            "message": "primary key change requires confirmation",
            "skippedPrimaryRecordCount": skipped_record_count,
            "skippedPrimaryInvalidCount": skipped_invalid_count,
            "skippedPrimaryDuplicateCount": skipped_duplicate_count
        })
        .to_string());
    }
    let accepted_records = partition.accepted_records;
    let searchable_paths = load_searchable_paths(&tx, dictionary_id)?;
    tx.execute(
        "DELETE FROM data_dictionary_records WHERE dictionary_id = ?1",
        params![dictionary_id],
    )
    .map_err(|e| format!("delete invalid primary records failed: {e}"))?;
    insert_records(
        &tx,
        dictionary_id,
        &accepted_records,
        &searchable_paths,
        new_sort_config,
    )?;
    mark_field_value_index_ready(&tx, dictionary_id)?;
    let record_count = accepted_records.len();
    tx.execute(
        "UPDATE data_dictionaries
         SET record_count = ?1, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?2",
        params![record_count as i64, dictionary_id],
    )
    .map_err(|e| format!("update data dictionary record count failed: {e}"))?;
    tx.commit()
        .map_err(|e| format!("commit update fields failed: {e}"))?;
    Ok(json!({
        "ok": true,
        "recordCount": record_count,
        "skippedPrimaryRecordCount": skipped_invalid_count + skipped_duplicate_count,
        "skippedPrimaryInvalidCount": skipped_invalid_count,
        "skippedPrimaryDuplicateCount": skipped_duplicate_count,
    }))
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
    let include_raw_json = payload["includeRawJson"].as_bool().unwrap_or(true);
    let fetch_limit = limit + 1;
    let conn = db_conn()?;
    if let SearchScope::Current(id) = scope {
        ensure_dictionary_exists(&conn, id)?;
    }
    ensure_scope_sort_keys_ready(&conn, &scope)?;

    let rows = if keyword.is_empty() {
        query_empty_records(&conn, &scope, Some(fetch_limit))?
    } else {
        query_like_records(&conn, &scope, keyword, Some(fetch_limit))?
    };

    let (rows, has_more) = split_limited_rows(rows, limit as usize);
    let items = rows_to_search_items(&conn, rows, keyword, include_raw_json)?;
    Ok(json!({ "items": items, "hasMore": has_more }))
}

fn action_popular_records(payload: &Value) -> Result<Value, String> {
    let dictionary_id = payload["dictionaryId"].as_i64();
    let limit = payload["limit"].as_i64().unwrap_or(10).clamp(1, 50);
    let conn = db_conn()?;

    let mut params_list: Vec<SqlParam> = Vec::new();
    let mut where_clause = String::from(
        "WHERE d.primary_field_path IS NOT NULL AND trim(d.primary_field_path) <> ''",
    );
    if let Some(dictionary_id) = dictionary_id {
        ensure_dictionary_exists(&conn, dictionary_id)?;
        where_clause.push_str(" AND CAST(u.scope_id AS INTEGER) = ?");
        params_list.push(Box::new(dictionary_id));
    }
    params_list.push(Box::new(limit));

    let sql = build_popular_records_query(&where_clause);
    let refs: Vec<&dyn ToSql> = params_list.iter().map(|param| param.as_ref()).collect();
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("prepare popular data dictionary records failed: {e}"))?;
    let rows = stmt
        .query_map(refs.as_slice(), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| format!("query popular data dictionary records failed: {e}"))?;

    let mut candidates = Vec::new();
    for row in rows {
        candidates.push(row.map_err(|e| e.to_string())?);
    }
    drop(stmt);

    let mut items = Vec::new();
    let mut stale = Vec::new();
    for (
        dictionary_id,
        normalized_value,
        used_count,
        last_used_at_ms,
        primary_field_path,
    ) in candidates
    {
        match load_record_row_by_primary_value(
            &conn,
            dictionary_id,
            &primary_field_path,
            &normalized_value,
        )? {
            Some(record) => {
                let (business_record_id, _) = load_record_primary_usage_value(
                    &conn,
                    record.id,
                    record.dictionary_id,
                    &primary_field_path,
                )?;
                let searchable_paths = load_searchable_paths(&conn, record.dictionary_id)?;
                let fields = load_field_configs(&conn, record.dictionary_id)?;
                let mut value =
                    record_row_to_search_item_json(record, &searchable_paths, &fields, "", false)?;
                if let Some(object) = value.as_object_mut() {
                    object.insert("recordId".to_string(), json!(business_record_id));
                    object.insert("normalizedValue".to_string(), json!(normalized_value));
                    object.insert("usedCount".to_string(), json!(used_count));
                    object.insert(
                        "lastUsedAt".to_string(),
                        json!(usage::format_timestamp_ms(Some(last_used_at_ms))),
                    );
                }
                items.push(value);
            }
            None => stale.push((dictionary_id, normalized_value)),
        }
    }

    for (dictionary_id, normalized_value) in stale {
        usage::delete_resource(
            &conn,
            UsageKey {
                resource_type: RESOURCE_DATA_DICTIONARY_RECORD,
                scope_id: &dictionary_id.to_string(),
                resource_id: &normalized_value,
            },
        )?;
    }

    Ok(json!({ "items": items }))
}

fn action_mark_record_used(payload: &Value) -> Result<Value, String> {
    let row_id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;
    let record = load_record_row_by_id(&conn, row_id)?;
    let primary_field_path = load_dictionary_primary_field_path(&conn, record.dictionary_id)?
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "dictionary primaryFieldPath is required".to_string())?;
    let (_business_record_id, normalized_value) = load_record_primary_usage_value(
        &conn,
        record.id,
        record.dictionary_id,
        &primary_field_path,
    )?;

    usage::record(
        &conn,
        UsageKey {
            resource_type: RESOURCE_DATA_DICTIONARY_RECORD,
            scope_id: &record.dictionary_id.to_string(),
            resource_id: &normalized_value,
        },
        ACTION_VIEW,
    )?;

    Ok(json!({ "ok": true }))
}

fn action_rebuild_indexes(payload: &Value) -> Result<Value, String> {
    let dictionary_id = payload["dictionaryId"]
        .as_i64()
        .ok_or("dictionaryId is required")?;
    let mut conn = db_conn()?;
    ensure_dictionary_exists(&conn, dictionary_id)?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("rebuild dictionary indexes transaction failed: {e}"))?;
    let stats = rebuild_dictionary_indexes(&tx, dictionary_id)?;
    tx.commit()
        .map_err(|e| format!("commit rebuild dictionary indexes failed: {e}"))?;
    Ok(json!({
        "recordCount": stats.record_count,
        "valueCount": stats.value_count,
        "skippedPrimaryRecordCount": stats.skipped_record_count(),
        "skippedPrimaryInvalidCount": stats.skipped_invalid_count,
        "skippedPrimaryDuplicateCount": stats.skipped_duplicate_count,
    }))
}

fn action_record_detail(payload: &Value) -> Result<Value, String> {
    let record_id = payload["recordId"].as_i64().ok_or("recordId is required")?;
    let conn = db_conn()?;
    let row = load_record_row_by_id(&conn, record_id)?;
    ensure_field_value_index_ready(&conn, row.dictionary_id)?;
    let fields = load_field_configs(&conn, row.dictionary_id)?;
    let raw_json = serde_json::from_str::<Value>(&row.raw_json)
        .map_err(|e| format!("parse data dictionary record {} failed: {e}", row.id))?;
    let mut record = record_row_to_brief_json(row.clone(), &fields)?;
    if let Some(object) = record.as_object_mut() {
        object.insert("rawJson".to_string(), raw_json);
    }

    let forward_relations = load_relation_configs(&conn, row.dictionary_id, false)?;
    let reverse_relations = load_relation_configs(&conn, row.dictionary_id, true)?;
    let mut forward_groups = Vec::new();
    for relation in forward_relations {
        let target_primary = relation
            .target_primary_field_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or("target dictionary primary field is required")?;
        ensure_field_value_index_ready(&conn, relation.target_dictionary_id)?;
        let seed = load_relation_seed_value(
            &conn,
            row.id,
            row.dictionary_id,
            &relation.source_field_path,
        )?;
        let items = if let Some(seed) = seed {
            load_related_record_briefs(
                &conn,
                relation.target_dictionary_id,
                target_primary,
                &seed,
            )?
        } else {
            Vec::new()
        };
        forward_groups.push(json!({
            "relationId": relation.id,
            "name": relation.relation_name,
            "direction": "forward",
            "sourceDictionaryId": relation.source_dictionary_id,
            "targetDictionaryId": relation.target_dictionary_id,
            "itemCount": items.len(),
            "items": items,
        }));
    }

    let mut reverse_groups = Vec::new();
    for relation in reverse_relations {
        let target_primary = relation
            .target_primary_field_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or("target dictionary primary field is required")?;
        ensure_field_value_index_ready(&conn, relation.source_dictionary_id)?;
        let seed = load_relation_seed_value(&conn, row.id, row.dictionary_id, target_primary)?;
        let items = if let Some(seed) = seed {
            load_related_record_briefs(
                &conn,
                relation.source_dictionary_id,
                &relation.source_field_path,
                &seed,
            )?
        } else {
            Vec::new()
        };
        reverse_groups.push(json!({
            "relationId": relation.id,
            "name": relation.reverse_name,
            "direction": "reverse",
            "sourceDictionaryId": relation.source_dictionary_id,
            "targetDictionaryId": relation.target_dictionary_id,
            "itemCount": items.len(),
            "items": items,
        }));
    }

    Ok(json!({
        "record": record,
        "fields": fields.iter().map(field_config_to_json).collect::<Vec<_>>(),
        "forwardRelations": forward_groups,
        "reverseRelations": reverse_groups,
    }))
}

fn action_delete(payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|error| format!("begin delete data dictionary transaction failed: {error}"))?;
    tx.execute("DELETE FROM data_dictionaries WHERE id = ?1", params![id])
        .map_err(|e| format!("delete data dictionary failed: {e}"))?;
    usage::delete_scope(&tx, RESOURCE_DATA_DICTIONARY_RECORD, &id.to_string())?;
    tx.commit()
        .map_err(|error| format!("commit delete data dictionary transaction failed: {error}"))?;
    Ok(json!({ "ok": true }))
}

fn read_import_input(payload: &Value) -> Result<String, String> {
    if let Some(path) = payload["inputPath"]
        .as_str()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        return fs::read_to_string(path).map_err(|e| format!("读取导入文件失败: {e}"));
    }
    Ok(payload["input"].as_str().unwrap_or_default().to_string())
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

fn collect_field_stats_from_indexed(records: &[IndexedRecord]) -> Vec<FieldStat> {
    let values = records
        .iter()
        .map(|record| record.value.clone())
        .collect::<Vec<_>>();
    collect_field_stats(&values)
}

fn flatten_record(record: &Value) -> Vec<FlattenedField> {
    let mut fields = Vec::new();
    if let Value::Object(map) = record {
        flatten_object(map, "", &mut fields);
    }
    fields.sort_by(|a, b| a.path.cmp(&b.path));
    fields
}

fn build_record_values(
    record_id: i64,
    dictionary_id: i64,
    raw_json: &str,
) -> Result<Vec<RecordValue>, String> {
    let record = serde_json::from_str::<Value>(raw_json)
        .map_err(|e| format!("parse data dictionary record {record_id} failed: {e}"))?;
    Ok(flatten_record(&record)
        .into_iter()
        .map(|field| RecordValue {
            record_id,
            dictionary_id,
            field_path: field.path,
            value_type: field.type_hint,
            normalized_value: normalize_search_text(&field.value_text),
            value_text: field.value_text,
        })
        .collect())
}

fn indexed_records_from_values(records: Vec<Value>) -> Vec<IndexedRecord> {
    records
        .into_iter()
        .enumerate()
        .map(|(idx, value)| IndexedRecord {
            source_row_index: idx as i64,
            value,
        })
        .collect()
}

fn partition_records_by_primary(
    records: Vec<IndexedRecord>,
    primary_field_path: Option<&str>,
) -> Result<PrimaryPartition, String> {
    let Some(primary_field_path) = primary_field_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(PrimaryPartition {
            accepted_records: records,
            skipped_invalid_count: 0,
            skipped_duplicate_count: 0,
        });
    };

    let mut seen = HashSet::new();
    let mut accepted_records = Vec::new();
    let mut skipped_invalid_count = 0;
    let mut skipped_duplicate_count = 0;
    for record in records {
        let Some(primary_value) = get_value_by_field_path(&record.value, primary_field_path) else {
            skipped_invalid_count += 1;
            continue;
        };
        let Some(normalized) = normalized_primary_key(primary_value) else {
            skipped_invalid_count += 1;
            continue;
        };
        if !seen.insert(normalized) {
            skipped_duplicate_count += 1;
            continue;
        }
        accepted_records.push(record);
    }

    Ok(PrimaryPartition {
        accepted_records,
        skipped_invalid_count,
        skipped_duplicate_count,
    })
}

fn normalized_primary_key(value: &Value) -> Option<String> {
    match value {
        Value::String(_) | Value::Number(_) | Value::Bool(_) => {
            let normalized = normalize_search_text(&value_to_search_text(value));
            if normalized.is_empty() {
                None
            } else {
                Some(normalized)
            }
        }
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
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

fn parse_required_field_path(payload: &Value, key: &str) -> Result<String, String> {
    payload[key]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("{key} is required"))
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

fn parse_relation_drafts(
    conn: &Connection,
    source_dictionary_id: i64,
    field_paths: &HashSet<String>,
    current_primary_field_path: Option<&str>,
    relations: Option<&Vec<Value>>,
) -> Result<Vec<RelationDraft>, String> {
    let Some(relations) = relations else {
        return Ok(Vec::new());
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for relation in relations {
        let source_field_path = relation["sourceFieldPath"]
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or("relation sourceFieldPath is required")?;
        if !field_paths.contains(source_field_path) {
            return Err(format!("relation source field not found: {source_field_path}"));
        }
        if field_has_non_scalar_values(conn, source_dictionary_id, source_field_path)? {
            return Err(format!(
                "relation source field must be scalar: {source_field_path}"
            ));
        }
        let target_dictionary_id = relation["targetDictionaryId"]
            .as_i64()
            .filter(|id| *id > 0)
            .ok_or("relation targetDictionaryId is required")?;
        let target_primary_field_path = if target_dictionary_id == source_dictionary_id {
            current_primary_field_path.map(str::to_string)
        } else {
            load_dictionary_primary_field_path(conn, target_dictionary_id)?
        }
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or("target dictionary primary field is required")?;
        if target_dictionary_id == source_dictionary_id
            && source_field_path == target_primary_field_path
        {
            return Err("self relation source field must differ from primary field".to_string());
        }
        let relation_name = relation["relationName"]
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or("relationName is required")?;
        let reverse_name = relation["reverseName"]
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or("reverseName is required")?;
        let key = (source_field_path.to_string(), target_dictionary_id);
        if !seen.insert(key) {
            return Err(format!(
                "duplicate relation: {source_field_path} -> {target_dictionary_id}"
            ));
        }
        out.push(RelationDraft {
            source_field_path: source_field_path.to_string(),
            target_dictionary_id,
            relation_name: relation_name.to_string(),
            reverse_name: reverse_name.to_string(),
        });
    }
    Ok(out)
}

fn require_non_empty_field_payload(fields: &[Value]) -> Result<(), String> {
    if fields.is_empty() {
        return Err("fields must not be empty".to_string());
    }
    Ok(())
}

fn field_has_non_scalar_values(
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

fn insert_record_values(
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

fn build_keyword_search_condition(keyword: &str) -> (String, String) {
    let normalized = normalize_search_text(keyword);
    let pattern = format!("%{}%", escape_like_pattern(&normalized));
    (
        "r.normalized_search_text LIKE ? ESCAPE '\\'".to_string(),
        pattern,
    )
}

fn build_popular_records_query(where_clause: &str) -> String {
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

fn query_records(
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

fn build_record_query_sql(
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

fn load_record_row_by_id(conn: &Connection, record_id: i64) -> Result<RecordRow, String> {
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

fn load_relation_configs(
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

fn load_relation_seed_value(
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

fn load_record_primary_usage_value(
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

fn load_record_row_by_primary_value(
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

fn load_related_record_briefs(
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
        .query_map(params![dictionary_id, field_path, normalized_value], record_row)
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

fn build_record_sort_key(
    record: &Value,
    row_index: i64,
    sort_config: Option<(&str, SortDirection)>,
) -> Result<String, String> {
    let row_key = encode_row_index_sort_part(row_index)?;
    let Some((field_path, direction)) = sort_config else {
        return Ok(format!("1!{row_key}"));
    };
    let Some(value) = get_value_by_field_path(record, field_path) else {
        return Ok(format!("2!{row_key}"));
    };
    let value_bytes = encode_present_sort_value(value, direction)?;
    Ok(format!("0!{}!{row_key}", hex_encode_upper(&value_bytes)))
}

fn encode_present_sort_value(value: &Value, direction: SortDirection) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    match value {
        Value::Number(number) => {
            let value = number
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| "sort number is not representable as finite f64".to_string())?;
            out.push(1);
            let mut payload = ordered_f64_bytes(value).to_vec();
            if direction == SortDirection::Desc {
                invert_bytes_in_place(&mut payload);
            }
            out.extend(payload);
        }
        Value::String(text) => {
            out.push(2);
            let mut payload = text.as_bytes().to_vec();
            payload.push(0);
            if direction == SortDirection::Desc {
                invert_bytes_in_place(&mut payload);
            }
            out.extend(payload);
        }
        Value::Bool(flag) => {
            out.push(3);
            let mut payload = vec![if *flag { 1 } else { 0 }];
            if direction == SortDirection::Desc {
                invert_bytes_in_place(&mut payload);
            }
            out.extend(payload);
        }
        Value::Null => {
            out.push(4);
        }
        Value::Array(_) | Value::Object(_) => {
            out.push(5);
            let mut payload = value_to_search_text(value).into_bytes();
            payload.push(0);
            if direction == SortDirection::Desc {
                invert_bytes_in_place(&mut payload);
            }
            out.extend(payload);
        }
    }
    Ok(out)
}

fn ordered_f64_bytes(value: f64) -> [u8; 8] {
    let bits = value.to_bits();
    let ordered = if bits & (1_u64 << 63) == 0 {
        bits ^ (1_u64 << 63)
    } else {
        !bits
    };
    ordered.to_be_bytes()
}

fn encode_row_index_sort_part(row_index: i64) -> Result<String, String> {
    if row_index < 0 {
        return Err(format!("row_index must not be negative: {row_index}"));
    }
    Ok(format!("{:016X}", row_index as u64))
}

fn invert_bytes_in_place(bytes: &mut [u8]) {
    for byte in bytes {
        *byte = 255_u8 - *byte;
    }
}

fn hex_encode_upper(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0F) as usize] as char);
    }
    out
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

fn record_row_to_search_item_json(
    row: RecordRow,
    searchable_paths: &[String],
    fields: &[FieldConfig],
    keyword: &str,
    include_raw_json: bool,
) -> Result<Value, String> {
    let record = serde_json::from_str::<Value>(&row.raw_json)
        .map_err(|e| format!("parse data dictionary record {} failed: {e}", row.id))?;
    let title = build_record_title(
        &record,
        row.title_field_path.as_deref(),
        &row.dictionary_name,
        row.row_index,
    );
    let summary = build_record_summary(&record, fields, row.title_field_path.as_deref());
    let mut item = json!({
        "id": row.id,
        "dictionaryId": row.dictionary_id,
        "dictionaryName": row.dictionary_name,
        "titleFieldPath": row.title_field_path,
        "rowIndex": row.row_index,
        "matches": compute_matches(&record, searchable_paths, keyword),
        "title": title,
        "summary": summary,
    });
    if include_raw_json {
        if let Some(object) = item.as_object_mut() {
            object.insert("rawJson".to_string(), record);
        }
    }
    Ok(item)
}

fn record_row_to_brief_json(row: RecordRow, fields: &[FieldConfig]) -> Result<Value, String> {
    let record = serde_json::from_str::<Value>(&row.raw_json)
        .map_err(|e| format!("parse data dictionary record {} failed: {e}", row.id))?;
    let title = build_record_title(
        &record,
        row.title_field_path.as_deref(),
        &row.dictionary_name,
        row.row_index,
    );
    let summary = build_record_summary(&record, fields, row.title_field_path.as_deref());
    Ok(json!({
        "id": row.id,
        "dictionaryId": row.dictionary_id,
        "dictionaryName": row.dictionary_name,
        "title": title,
        "rowIndex": row.row_index,
        "summary": summary,
    }))
}

fn build_record_title(
    record: &Value,
    title_field_path: Option<&str>,
    dictionary_name: &str,
    row_index: i64,
) -> String {
    if let Some(title_field_path) = title_field_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(value) = get_value_by_field_path(record, title_field_path) {
            let title = value_to_search_text(value).trim().to_string();
            if !title.is_empty() {
                return title;
            }
        }
    }
    format!("{dictionary_name} #{}", row_index + 1)
}

fn build_record_summary(
    record: &Value,
    fields: &[FieldConfig],
    excluded_field_path: Option<&str>,
) -> Vec<Value> {
    let mut visible_fields = fields
        .iter()
        .filter(|field| field.visible)
        .filter(|field| Some(field.field_path.as_str()) != excluded_field_path)
        .collect::<Vec<_>>();
    visible_fields.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| a.field_path.cmp(&b.field_path))
    });
    visible_fields
        .into_iter()
        .filter_map(|field| {
            let value = get_value_by_field_path(record, &field.field_path)?;
            let text = value_to_search_text(value);
            if text.is_empty() {
                return None;
            }
            Some(json!({
                "fieldPath": field.field_path,
                "label": summary_field_label(field),
                "value": text,
            }))
        })
        .collect()
}

fn summary_field_label(field: &FieldConfig) -> String {
    if !field.display_name.trim().is_empty() {
        return field.display_name.trim().to_string();
    }
    if !field.meaning.trim().is_empty() {
        return field.meaning.trim().to_string();
    }
    field.field_path.clone()
}

fn is_relation_value_usable(value_type: &str, normalized_value: &str) -> bool {
    matches!(value_type, "string" | "number" | "boolean") && !normalized_value.trim().is_empty()
}

fn split_limited_rows(mut rows: Vec<RecordRow>, limit: usize) -> (Vec<RecordRow>, bool) {
    if rows.len() > limit {
        rows.truncate(limit);
        (rows, true)
    } else {
        (rows, false)
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

fn load_indexed_records_for_dictionary(
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

fn load_relation_jsons(conn: &Connection, source_dictionary_id: i64) -> Result<Vec<Value>, String> {
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

fn load_dictionary_primary_field_path(
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

fn mark_field_value_index_ready(conn: &Connection, dictionary_id: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE data_dictionaries
         SET field_value_indexed_at = CURRENT_TIMESTAMP
         WHERE id = ?1",
        params![dictionary_id],
    )
    .map_err(|e| format!("mark data dictionary value index ready failed: {e}"))?;
    Ok(())
}

fn ensure_field_value_index_ready(conn: &Connection, dictionary_id: i64) -> Result<(), String> {
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

fn ensure_scope_sort_keys_ready(conn: &Connection, scope: &SearchScope) -> Result<(), String> {
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

fn load_dictionaries_with_empty_sort_keys(
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

fn refresh_dictionary_sort_keys(
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

fn rebuild_dictionary_indexes(
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

fn dictionary_row_to_json(row: &Row<'_>) -> rusqlite::Result<Value> {
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
    fn import_preview_reads_large_json_from_file_path() {
        let large_value = "x".repeat(1024);
        let items = (0..10_000)
            .map(|idx| format!(r#"{{"id":{idx},"name":"item-{idx}","payload":"{large_value}"}}"#))
            .collect::<Vec<_>>();
        let input = format!("[{}]", items.join(","));
        assert!(input.len() > 10 * 1024 * 1024);

        let path = std::env::temp_dir().join(format!(
            "lazycat-data-dictionary-large-import-{}.json",
            std::process::id()
        ));
        std::fs::write(&path, &input).expect("write large import payload");

        let out = action_import_preview(&json!({ "inputPath": path.to_string_lossy() }))
            .expect("preview large file import");
        let _ = std::fs::remove_file(path);

        assert_eq!(out["recordCount"], json!(10_000));
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
    fn record_values_persists_value_type_for_null_vs_string_null() {
        let values =
            build_record_values(11, 7, r#"{"empty":null,"text":"null","count":3}"#).unwrap();

        let pairs = values
            .into_iter()
            .map(|value| (value.field_path, value.value_type, value.normalized_value))
            .collect::<Vec<_>>();

        assert_eq!(
            pairs,
            vec![
                ("count".to_string(), "number".to_string(), "3".to_string()),
                ("empty".to_string(), "null".to_string(), "null".to_string()),
                ("text".to_string(), "string".to_string(), "null".to_string()),
            ],
        );
    }

    #[test]
    fn partition_records_by_primary_skips_invalid_and_duplicate_values() {
        let partition = partition_records_by_primary(
            vec![
                IndexedRecord {
                    source_row_index: 0,
                    value: json!({ "employeeNo": "" }),
                },
                IndexedRecord {
                    source_row_index: 1,
                    value: json!({ "name": "missing" }),
                },
                IndexedRecord {
                    source_row_index: 2,
                    value: json!({ "employeeNo": "A001" }),
                },
                IndexedRecord {
                    source_row_index: 3,
                    value: json!({ "employeeNo": " A001 " }),
                },
                IndexedRecord {
                    source_row_index: 4,
                    value: json!({ "employeeNo": "A002" }),
                },
            ],
            Some("employeeNo"),
        )
        .unwrap();

        assert_eq!(partition.skipped_invalid_count, 2);
        assert_eq!(partition.skipped_duplicate_count, 1);
        assert_eq!(
            partition
                .accepted_records
                .iter()
                .map(|record| record.source_row_index)
                .collect::<Vec<_>>(),
            vec![2, 4],
        );
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
    fn record_brief_uses_title_field_and_visible_summary() {
        let mut row = test_record_row(
            9,
            4,
            json!({ "name": "张三", "dept": "研发", "secret": "hidden" }),
        );
        row.title_field_path = Some("name".to_string());
        let fields = vec![
            test_field_config("name", "姓名", true, 0),
            test_field_config("dept", "部门", true, 1),
            test_field_config("secret", "密钥", false, 2),
        ];

        let brief = record_row_to_brief_json(row, &fields).unwrap();

        assert_eq!(brief["title"], "张三");
        assert_eq!(brief["summary"], json!([
            { "fieldPath": "dept", "label": "部门", "value": "研发" }
        ]));
    }

    #[test]
    fn search_item_includes_title_summary_and_can_omit_raw_json() {
        let row = test_record_row(
            10,
            2,
            json!({ "id": 1001, "name": "张三", "dept": "研发", "secret": "hidden" }),
        );
        let fields = vec![
            test_field_config("id", "编号", true, 0),
            test_field_config("name", "姓名", true, 1),
            test_field_config("dept", "部门", true, 2),
            test_field_config("secret", "密钥", false, 3),
        ];

        let item = record_row_to_search_item_json(
            row,
            &["name".to_string(), "dept".to_string()],
            &fields,
            "张",
            false,
        )
        .unwrap();

        assert_eq!(item["title"], "测试字典 #3");
        assert_eq!(item["summary"], json!([
            { "fieldPath": "id", "label": "编号", "value": "1001" },
            { "fieldPath": "name", "label": "姓名", "value": "张三" },
            { "fieldPath": "dept", "label": "部门", "value": "研发" }
        ]));
        assert_eq!(item["matches"], json!([
            { "fieldPath": "name", "value": "张三" }
        ]));
        assert!(item.get("rawJson").is_none());
    }

    #[test]
    fn search_item_keeps_raw_json_by_default_shape() {
        let mut row = test_record_row(5, 0, json!({ "name": "张三" }));
        row.title_field_path = Some("name".to_string());
        let fields = vec![test_field_config("name", "姓名", true, 0)];

        let item = record_row_to_search_item_json(
            row,
            &["name".to_string()],
            &fields,
            "",
            true,
        )
        .unwrap();

        assert_eq!(item["title"], "张三");
        assert_eq!(item["rawJson"], json!({ "name": "张三" }));
        assert_eq!(item["summary"], json!([]));
    }

    #[test]
    fn search_item_returns_parse_error_for_invalid_raw_json() {
        let mut row = test_record_row(8, 0, json!({ "name": "valid" }));
        row.raw_json = "{ invalid json".to_string();
        let err = record_row_to_search_item_json(row, &[], &[], "", false).unwrap_err();

        assert!(err.contains("parse data dictionary record 8 failed"));
    }

    #[test]
    fn relation_value_filter_ignores_null_and_blank_values() {
        assert!(is_relation_value_usable("string", "a001"));
        assert!(is_relation_value_usable("number", "100"));
        assert!(!is_relation_value_usable("string", ""));
        assert!(!is_relation_value_usable("null", "null"));
        assert!(!is_relation_value_usable("array", "[\"A\"]"));
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
    fn build_record_sort_key_orders_numbers_numerically() {
        let small =
            build_record_sort_key(&json!({ "score": 2 }), 0, Some(("score", SortDirection::Asc)))
                .unwrap();
        let large = build_record_sort_key(
            &json!({ "score": 10 }),
            1,
            Some(("score", SortDirection::Asc)),
        )
        .unwrap();

        assert!(small < large);
    }

    #[test]
    fn build_record_sort_key_supports_desc_without_moving_missing_values_first() {
        let high = build_record_sort_key(
            &json!({ "score": 100 }),
            0,
            Some(("score", SortDirection::Desc)),
        )
        .unwrap();
        let low = build_record_sort_key(
            &json!({ "score": 90 }),
            1,
            Some(("score", SortDirection::Desc)),
        )
        .unwrap();
        let missing = build_record_sort_key(
            &json!({ "name": "missing" }),
            2,
            Some(("score", SortDirection::Desc)),
        )
        .unwrap();

        assert!(high < low);
        assert!(low < missing);
    }

    #[test]
    fn build_record_sort_key_uses_row_index_when_sort_field_is_not_configured() {
        let first = build_record_sort_key(&json!({ "score": 100 }), 0, None).unwrap();
        let second = build_record_sort_key(&json!({ "score": 1 }), 1, None).unwrap();

        assert!(first < second);
    }

    #[test]
    fn build_record_sort_key_uses_missing_bucket_and_row_index_when_sort_field_is_missing() {
        let present =
            build_record_sort_key(&json!({ "score": 1 }), 9, Some(("score", SortDirection::Asc)))
                .unwrap();
        let missing_first = build_record_sort_key(
            &json!({ "name": "a" }),
            0,
            Some(("score", SortDirection::Asc)),
        )
        .unwrap();
        let missing_second = build_record_sort_key(
            &json!({ "name": "b" }),
            1,
            Some(("score", SortDirection::Asc)),
        )
        .unwrap();

        assert!(present < missing_first);
        assert!(missing_first < missing_second);
    }

    #[test]
    fn build_record_sort_key_orders_strings_by_prefix_before_longer_value() {
        let short =
            build_record_sort_key(&json!({ "name": "a" }), 0, Some(("name", SortDirection::Asc)))
                .unwrap();
        let long =
            build_record_sort_key(&json!({ "name": "aa" }), 1, Some(("name", SortDirection::Asc)))
                .unwrap();

        assert!(short < long);
    }

    #[test]
    fn query_records_sql_orders_by_sort_key_not_updated_at() {
        let sql = build_record_query_sql(&SearchScope::All, &[], Some(100));

        assert!(sql.contains(
            "ORDER BY d.nav_order ASC, r.sort_key COLLATE BINARY ASC, r.id ASC"
        ));
        assert!(!sql.contains("ORDER BY d.updated_at DESC"));
        assert!(!sql.contains("ORDER BY r.row_index ASC"));
    }

    #[test]
    fn query_records_sql_current_uses_sort_key_without_nav_order() {
        let conditions = vec!["r.dictionary_id = ?".to_string()];
        let sql = build_record_query_sql(&SearchScope::Current(7), &conditions, Some(101));

        assert!(sql.contains("ORDER BY r.sort_key COLLATE BINARY ASC, r.id ASC"));
        assert!(!sql.contains("d.nav_order ASC"));
        assert!(sql.ends_with(" LIMIT ?"));
    }

    #[test]
    fn keyword_search_condition_uses_like_without_fts() {
        let (condition, pattern) = build_keyword_search_condition("  Foo\tBAR  ");

        assert_eq!(condition, "r.normalized_search_text LIKE ? ESCAPE '\\'");
        assert_eq!(pattern, "%foo bar%");
        assert!(!condition.contains("data_dictionary_fts"));
        assert!(!condition.contains("MATCH"));
    }

    #[test]
    fn popular_records_query_uses_unified_usage_identity() {
        let sql = build_popular_records_query("WHERE 1 = 1");

        assert!(sql.contains("u.resource_id AS normalized_value"));
        assert!(sql.contains("u.resource_type = 'data-dictionary-record'"));
        assert!(sql.contains("SUM(u.use_count) AS used_count"));
        assert!(!sql.contains("data_dictionary_record_usage"));
    }

    #[test]
    fn parse_limit_keeps_existing_search_limit_bounds() {
        assert_eq!(parse_limit(&json!({ "limit": 0 })), 1);
        assert_eq!(parse_limit(&json!({ "limit": 100 })), 100);
        assert_eq!(parse_limit(&json!({ "limit": 1000 })), 500);
    }

    #[test]
    fn load_dictionaries_with_empty_sort_keys_filters_scope() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE data_dictionary_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                dictionary_id INTEGER NOT NULL,
                row_index INTEGER NOT NULL,
                raw_json TEXT NOT NULL,
                search_text TEXT NOT NULL,
                normalized_search_text TEXT NOT NULL,
                sort_key TEXT NOT NULL DEFAULT ''
            );
            INSERT INTO data_dictionary_records
                (dictionary_id, row_index, raw_json, search_text, normalized_search_text, sort_key)
            VALUES
                (1, 0, '{}', '', '', ''),
                (2, 0, '{}', '', '', '1!0000000000000000'),
                (3, 0, '{}', '', '', '');
            ",
        )
        .unwrap();

        assert_eq!(
            load_dictionaries_with_empty_sort_keys(&conn, &SearchScope::All).unwrap(),
            vec![1, 3]
        );
        assert_eq!(
            load_dictionaries_with_empty_sort_keys(&conn, &SearchScope::Current(3)).unwrap(),
            vec![3]
        );
    }

    #[test]
    fn insert_records_persists_row_index_sort_key_without_sort_config() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE data_dictionary_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                dictionary_id INTEGER NOT NULL,
                row_index INTEGER NOT NULL,
                raw_json TEXT NOT NULL,
                search_text TEXT NOT NULL,
                normalized_search_text TEXT NOT NULL,
                sort_key TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE data_dictionary_record_values (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                record_id INTEGER NOT NULL,
                dictionary_id INTEGER NOT NULL,
                field_path TEXT NOT NULL,
                value_type TEXT NOT NULL,
                value_text TEXT NOT NULL,
                normalized_value TEXT NOT NULL,
                UNIQUE(record_id, field_path)
            );
            ",
        )
        .unwrap();
        let records = vec![
            IndexedRecord {
                source_row_index: 0,
                value: json!({ "score": 100 }),
            },
            IndexedRecord {
                source_row_index: 1,
                value: json!({ "score": 1 }),
            },
        ];

        insert_records(&conn, 1, &records, &["score".to_string()], None).unwrap();

        let mut stmt = conn
            .prepare("SELECT sort_key FROM data_dictionary_records ORDER BY row_index ASC")
            .unwrap();
        let keys = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().all(|key| !key.is_empty()));
        assert!(keys[0] < keys[1]);
    }

    #[test]
    fn build_record_sort_key_desc_keeps_equal_values_in_row_order() {
        let first = build_record_sort_key(
            &json!({ "score": 100 }),
            0,
            Some(("score", SortDirection::Desc)),
        )
        .unwrap();
        let second = build_record_sort_key(
            &json!({ "score": 100 }),
            1,
            Some(("score", SortDirection::Desc)),
        )
        .unwrap();

        assert!(first < second);
    }

    #[test]
    fn build_record_sort_key_handles_nested_field_paths() {
        let first = build_record_sort_key(
            &json!({ "user": { "name": "Ada" } }),
            0,
            Some(("user.name", SortDirection::Asc)),
        )
        .unwrap();
        let second = build_record_sort_key(
            &json!({ "user": { "name": "Bob" } }),
            1,
            Some(("user.name", SortDirection::Asc)),
        )
        .unwrap();

        assert!(first < second);
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
    fn require_non_empty_field_payload_rejects_empty_fields() {
        let fields: Vec<Value> = Vec::new();

        let err = require_non_empty_field_payload(&fields)
            .expect_err("empty field configuration payload must fail");

        assert_eq!(err, "fields must not be empty");
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

    #[test]
    fn parse_required_field_path_rejects_blank_values() {
        assert_eq!(
            parse_required_field_path(&json!({ "primaryFieldPath": " id " }), "primaryFieldPath")
                .unwrap(),
            "id"
        );

        let err = parse_required_field_path(
            &json!({ "primaryFieldPath": "   " }),
            "primaryFieldPath",
        )
        .expect_err("blank primary field path must fail");

        assert_eq!(err, "primaryFieldPath is required");
    }

    #[test]
    fn load_record_primary_usage_value_returns_business_and_normalized_values() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE data_dictionary_record_values (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                record_id INTEGER NOT NULL,
                dictionary_id INTEGER NOT NULL,
                field_path TEXT NOT NULL,
                value_type TEXT NOT NULL,
                value_text TEXT NOT NULL,
                normalized_value TEXT NOT NULL
            );
            INSERT INTO data_dictionary_record_values
                (record_id, dictionary_id, field_path, value_type, value_text, normalized_value)
            VALUES
                (10, 1, 'id', 'string', ' U-001 ', 'u-001'),
                (11, 1, 'id', 'null', 'null', 'null');
            ",
        )
        .unwrap();

        assert_eq!(
            load_record_primary_usage_value(&conn, 10, 1, "id").unwrap(),
            (" U-001 ".to_string(), "u-001".to_string())
        );
        assert!(load_record_primary_usage_value(&conn, 11, 1, "id").is_err());
    }

    #[test]
    fn load_record_row_by_primary_value_uses_normalized_value_not_row_id() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE data_dictionaries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                title_field_path TEXT DEFAULT NULL
            );
            CREATE TABLE data_dictionary_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                dictionary_id INTEGER NOT NULL,
                row_index INTEGER NOT NULL,
                raw_json TEXT NOT NULL,
                search_text TEXT NOT NULL,
                normalized_search_text TEXT NOT NULL,
                sort_key TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE data_dictionary_record_values (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                record_id INTEGER NOT NULL,
                dictionary_id INTEGER NOT NULL,
                field_path TEXT NOT NULL,
                value_type TEXT NOT NULL,
                value_text TEXT NOT NULL,
                normalized_value TEXT NOT NULL
            );
            INSERT INTO data_dictionaries (id, name, title_field_path)
            VALUES (1, 'Users', 'name');
            INSERT INTO data_dictionary_records
                (id, dictionary_id, row_index, raw_json, search_text, normalized_search_text, sort_key)
            VALUES
                (100, 1, 0, '{\"id\":\"u1\",\"name\":\"Old\"}', '', '', '1!0000000000000000'),
                (200, 1, 1, '{\"id\":\"u2\",\"name\":\"Current\"}', '', '', '1!0000000000000001');
            INSERT INTO data_dictionary_record_values
                (record_id, dictionary_id, field_path, value_type, value_text, normalized_value)
            VALUES
                (100, 1, 'id', 'string', 'u1', 'u1'),
                (200, 1, 'id', 'string', 'u2', 'u2');
            ",
        )
        .unwrap();

        let row = load_record_row_by_primary_value(&conn, 1, "id", "u2")
            .unwrap()
            .expect("record by normalized primary value");

        assert_eq!(row.id, 200);
        assert_eq!(row.dictionary_name, "Users");
        assert_eq!(row.title_field_path, Some("name".to_string()));
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

    fn test_field_config(
        field_path: &str,
        display_name: &str,
        visible: bool,
        sort_order: i64,
    ) -> FieldConfig {
        FieldConfig {
            field_path: field_path.to_string(),
            display_name: display_name.to_string(),
            meaning: String::new(),
            searchable: true,
            visible,
            sort_order,
            type_hint: "string".to_string(),
            sample_value: String::new(),
            present_count: 1,
        }
    }
}

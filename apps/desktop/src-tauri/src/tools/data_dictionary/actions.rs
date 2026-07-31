use super::super::helpers::db_conn;
use super::super::usage::{self, UsageKey, ACTION_VIEW, RESOURCE_DATA_DICTIONARY_RECORD};
use super::import::*;
use super::model::*;
use super::path::*;
use super::records::*;
use super::repository::*;
use rusqlite::{params, Connection, OptionalExtension, ToSql};
use serde_json::{json, Value};
use std::collections::HashSet;

pub(super) fn action_list() -> Result<Value, String> {
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

pub(super) fn action_get(payload: &Value) -> Result<Value, String> {
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

pub(super) fn action_import_preview(payload: &Value) -> Result<Value, String> {
    let input = read_import_input(payload)?;
    let records = parse_import_array(&input)?;
    let stats = collect_field_stats(&records);
    Ok(json!({
        "recordCount": records.len(),
        "fields": stats.iter().map(field_stat_to_json).collect::<Vec<_>>(),
    }))
}

pub(super) fn action_create(payload: &Value) -> Result<Value, String> {
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

pub(super) fn action_rename(payload: &Value) -> Result<Value, String> {
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

pub(super) fn action_replace_records(payload: &Value) -> Result<Value, String> {
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

pub(super) fn action_update_fields(payload: &Value) -> Result<Value, String> {
    let dictionary_id = payload["dictionaryId"]
        .as_i64()
        .ok_or("dictionaryId is required")?;
    let fields = payload["fields"].as_array().ok_or("fields is required")?;
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

pub(super) fn action_reorder(payload: &Value) -> Result<Value, String> {
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

pub(super) fn action_search(payload: &Value) -> Result<Value, String> {
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

pub(super) fn action_popular_records(payload: &Value) -> Result<Value, String> {
    let dictionary_id = payload["dictionaryId"].as_i64();
    let limit = payload["limit"].as_i64().unwrap_or(10).clamp(1, 50);
    let conn = db_conn()?;

    let mut params_list: Vec<SqlParam> = Vec::new();
    let mut where_clause =
        String::from("WHERE d.primary_field_path IS NOT NULL AND trim(d.primary_field_path) <> ''");
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
    for (dictionary_id, normalized_value, used_count, last_used_at_ms, primary_field_path) in
        candidates
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

pub(super) fn action_mark_record_used(payload: &Value) -> Result<Value, String> {
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

pub(super) fn action_rebuild_indexes(payload: &Value) -> Result<Value, String> {
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

pub(super) fn action_record_detail(payload: &Value) -> Result<Value, String> {
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
            load_related_record_briefs(&conn, relation.target_dictionary_id, target_primary, &seed)?
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

pub(super) fn action_delete(payload: &Value) -> Result<Value, String> {
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

pub(super) fn parse_search_scope(payload: &Value) -> Result<SearchScope, String> {
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

pub(super) fn parse_reorder_dictionary_ids(payload: &Value) -> Result<Vec<(i64, i64)>, String> {
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

pub(super) fn parse_required_field_path(payload: &Value, key: &str) -> Result<String, String> {
    payload[key]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("{key} is required"))
}

pub(super) fn parse_configured_field_path(
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

pub(super) fn parse_relation_drafts(
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
            return Err(format!(
                "relation source field not found: {source_field_path}"
            ));
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

pub(super) fn require_non_empty_field_payload(fields: &[Value]) -> Result<(), String> {
    if fields.is_empty() {
        return Err("fields must not be empty".to_string());
    }
    Ok(())
}

pub(super) fn parse_limit(payload: &Value) -> i64 {
    payload["limit"].as_i64().unwrap_or(100).clamp(1, 500)
}

pub(super) fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

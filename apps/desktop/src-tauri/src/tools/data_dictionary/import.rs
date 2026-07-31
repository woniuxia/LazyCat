use super::model::{FieldStat, FlattenedField, IndexedRecord, PrimaryPartition, RecordValue};
use super::path::{default_display_name, escape_path_segment, get_value_by_field_path};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::fs;

pub(super) fn read_import_input(payload: &Value) -> Result<String, String> {
    if let Some(path) = payload["inputPath"]
        .as_str()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        return fs::read_to_string(path).map_err(|e| format!("读取导入文件失败: {e}"));
    }
    Ok(payload["input"].as_str().unwrap_or_default().to_string())
}

pub(super) fn parse_import_array(input: &str) -> Result<Vec<Value>, String> {
    let value = serde_json::from_str::<Value>(input).map_err(|e| format!("JSON 解析失败: {e}"))?;
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

pub(super) fn collect_field_stats(records: &[Value]) -> Vec<FieldStat> {
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

pub(super) fn collect_field_stats_from_indexed(records: &[IndexedRecord]) -> Vec<FieldStat> {
    let values = records
        .iter()
        .map(|record| record.value.clone())
        .collect::<Vec<_>>();
    collect_field_stats(&values)
}

pub(super) fn flatten_record(record: &Value) -> Vec<FlattenedField> {
    let mut fields = Vec::new();
    if let Value::Object(map) = record {
        flatten_object(map, "", &mut fields);
    }
    fields.sort_by(|a, b| a.path.cmp(&b.path));
    fields
}

pub(super) fn build_record_values(
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

pub(super) fn indexed_records_from_values(records: Vec<Value>) -> Vec<IndexedRecord> {
    records
        .into_iter()
        .enumerate()
        .map(|(idx, value)| IndexedRecord {
            source_row_index: idx as i64,
            value,
        })
        .collect()
}

pub(super) fn partition_records_by_primary(
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

pub(super) fn normalized_primary_key(value: &Value) -> Option<String> {
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

pub(super) fn flatten_object(
    map: &serde_json::Map<String, Value>,
    prefix: &str,
    fields: &mut Vec<FlattenedField>,
) {
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

pub(super) fn value_to_search_text(value: &Value) -> String {
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

pub(super) fn value_type_hint(value: &Value) -> &'static str {
    match value {
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
        Value::Null => "null",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

pub(super) fn normalize_search_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub(super) fn escape_like_pattern(value: &str) -> String {
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

pub(super) fn build_search_text(fields: &[FlattenedField], searchable_paths: &[String]) -> String {
    let searchable: HashSet<&str> = searchable_paths.iter().map(String::as_str).collect();
    fields
        .iter()
        .filter(|field| searchable.contains(field.path.as_str()))
        .map(|field| field.value_text.as_str())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn field_stat_to_json(stat: &FieldStat) -> Value {
    json!({
        "fieldPath": stat.path,
        "displayName": default_display_name(&stat.path),
        "typeHint": stat.type_hint,
        "sampleValue": stat.sample_value,
        "presentCount": stat.present_count,
        "sortOrder": stat.sort_order,
    })
}

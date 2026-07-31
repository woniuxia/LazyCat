use super::import::{flatten_record, normalize_search_text, value_to_search_text};
use super::model::{FieldConfig, RecordRow, SortDirection};
use super::path::get_value_by_field_path;
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::HashSet;

pub(super) fn parse_sort_direction(value: Option<&str>) -> Result<SortDirection, String> {
    match value.unwrap_or("asc") {
        "asc" => Ok(SortDirection::Asc),
        "desc" => Ok(SortDirection::Desc),
        other => Err(format!("unsupported sortDirection: {other}")),
    }
}

impl SortDirection {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            SortDirection::Asc => "asc",
            SortDirection::Desc => "desc",
        }
    }
}

pub(super) fn compute_matches(
    record: &Value,
    searchable_paths: &[String],
    keyword: &str,
) -> Vec<Value> {
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

pub(super) fn build_record_sort_key(
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

pub(super) fn encode_present_sort_value(
    value: &Value,
    direction: SortDirection,
) -> Result<Vec<u8>, String> {
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

pub(super) fn ordered_f64_bytes(value: f64) -> [u8; 8] {
    let bits = value.to_bits();
    let ordered = if bits & (1_u64 << 63) == 0 {
        bits ^ (1_u64 << 63)
    } else {
        !bits
    };
    ordered.to_be_bytes()
}

pub(super) fn encode_row_index_sort_part(row_index: i64) -> Result<String, String> {
    if row_index < 0 {
        return Err(format!("row_index must not be negative: {row_index}"));
    }
    Ok(format!("{:016X}", row_index as u64))
}

pub(super) fn invert_bytes_in_place(bytes: &mut [u8]) {
    for byte in bytes {
        *byte = 255_u8 - *byte;
    }
}

pub(super) fn hex_encode_upper(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0F) as usize] as char);
    }
    out
}

pub(super) fn sort_record_rows(rows: &mut [RecordRow], sort_config: Option<(&str, SortDirection)>) {
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

pub(super) fn record_sort_value(raw_json: &str, field_path: &str) -> Option<Value> {
    let record = serde_json::from_str::<Value>(raw_json).ok()?;
    get_value_by_field_path(&record, field_path).cloned()
}

pub(super) fn compare_sort_values(
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

pub(super) fn compare_present_sort_values(a: &Value, b: &Value) -> Ordering {
    match (a.as_f64(), b.as_f64()) {
        (Some(left), Some(right)) => {
            return left.partial_cmp(&right).unwrap_or(Ordering::Equal);
        }
        _ => {}
    }
    value_to_search_text(a).cmp(&value_to_search_text(b))
}

pub(super) fn record_row_to_search_item_json(
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

pub(super) fn record_row_to_brief_json(
    row: RecordRow,
    fields: &[FieldConfig],
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
    Ok(json!({
        "id": row.id,
        "dictionaryId": row.dictionary_id,
        "dictionaryName": row.dictionary_name,
        "title": title,
        "rowIndex": row.row_index,
        "summary": summary,
    }))
}

pub(super) fn build_record_title(
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

pub(super) fn build_record_summary(
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

pub(super) fn summary_field_label(field: &FieldConfig) -> String {
    if !field.display_name.trim().is_empty() {
        return field.display_name.trim().to_string();
    }
    if !field.meaning.trim().is_empty() {
        return field.meaning.trim().to_string();
    }
    field.field_path.clone()
}

pub(super) fn is_relation_value_usable(value_type: &str, normalized_value: &str) -> bool {
    matches!(value_type, "string" | "number" | "boolean") && !normalized_value.trim().is_empty()
}

pub(super) fn split_limited_rows(mut rows: Vec<RecordRow>, limit: usize) -> (Vec<RecordRow>, bool) {
    if rows.len() > limit {
        rows.truncate(limit);
        (rows, true)
    } else {
        (rows, false)
    }
}

pub(super) fn field_config_to_json(config: &FieldConfig) -> Value {
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

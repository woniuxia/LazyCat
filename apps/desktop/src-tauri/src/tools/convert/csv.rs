use serde_json::{json, Value};
use std::fs;

pub(super) fn csv_to_json(payload: &Value) -> Result<Value, String> {
    let input = payload["input"].as_str().unwrap_or_default();
    let delimiter = payload["delimiter"].as_str().unwrap_or(",").as_bytes()[0];
    let has_header = payload["hasHeader"].as_bool().unwrap_or(true);
    let custom_headers: Option<Vec<String>> = payload["customHeaders"].as_array().map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    });
    let selected_columns: Option<Vec<usize>> = payload["selectedColumns"].as_array().map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_u64().map(|n| n as usize))
            .collect()
    });

    let mut rdr = ::csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(has_header)
        .from_reader(input.as_bytes());

    let headers: Vec<String> = if let Some(ref custom) = custom_headers {
        custom.clone()
    } else if has_header {
        rdr.headers()
            .map_err(|e| format!("csv read header failed: {e}"))?
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        // peek first record to determine column count
        let mut peek_rdr = ::csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .has_headers(false)
            .from_reader(input.as_bytes());
        let count = peek_rdr
            .records()
            .next()
            .and_then(|r| r.ok())
            .map(|r| r.len())
            .unwrap_or(0);
        (0..count).map(|i| format!("col{}", i + 1)).collect()
    };

    let mut rows = Vec::new();
    for rec in rdr.records() {
        let record = rec.map_err(|e| format!("csv record failed: {e}"))?;
        let mut obj = serde_json::Map::new();
        for (i, col) in headers.iter().enumerate() {
            if let Some(ref sel) = selected_columns {
                if !sel.contains(&i) {
                    continue;
                }
            }
            obj.insert(col.clone(), json!(record.get(i).unwrap_or("")));
        }
        rows.push(Value::Object(obj));
    }
    Ok(json!(
        serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".into())
    ))
}

pub(super) fn csv_read_file(payload: &Value) -> Result<Value, String> {
    let path = payload["path"].as_str().unwrap_or_default();
    if path.is_empty() {
        return Err("file path is empty".into());
    }
    let bytes = fs::read(path).map_err(|e| format!("read csv file failed: {e}"))?;
    // Try UTF-8 first; fall back to GBK (common on Windows for Chinese text)
    let content = match String::from_utf8(bytes.clone()) {
        Ok(s) => s,
        Err(_) => {
            let (cow, _, had_errors) = encoding_rs::GBK.decode(&bytes);
            if had_errors {
                return Err("文件编码无法识别，请使用 UTF-8 或 GBK 编码的文件".into());
            }
            cow.into_owned()
        }
    };
    Ok(json!(content))
}

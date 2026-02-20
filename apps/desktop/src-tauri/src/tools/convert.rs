use serde_json::{json, Value};
use std::fs;

fn json_to_xml(root_tag: &str, value: &Value) -> String {
    let root = sanitize_xml_tag(root_tag, "root");
    let mut out = String::new();
    append_xml_node_pretty(&mut out, &root, value, 0);
    out.trim_end_matches('\n').to_string()
}

fn append_xml_node_pretty(out: &mut String, tag: &str, value: &Value, depth: usize) {
    match value {
        Value::Array(items) => {
            if items.is_empty() {
                write_indent(out, depth);
                out.push('<');
                out.push_str(tag);
                out.push_str("/>");
                out.push('\n');
                return;
            }
            for item in items {
                append_xml_node_pretty(out, tag, item, depth);
            }
        }
        Value::Object(map) => {
            if map.is_empty() {
                write_indent(out, depth);
                out.push('<');
                out.push_str(tag);
                out.push_str("/>");
                out.push('\n');
                return;
            }

            write_indent(out, depth);
            out.push('<');
            out.push_str(tag);
            out.push('>');
            out.push('\n');
            for (key, child) in map {
                let child_tag = sanitize_xml_tag(key, "item");
                append_xml_node_pretty(out, &child_tag, child, depth + 1);
            }
            write_indent(out, depth);
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
            out.push('\n');
        }
        Value::Null => {
            write_indent(out, depth);
            out.push('<');
            out.push_str(tag);
            out.push_str("/>");
            out.push('\n');
        }
        Value::String(s) => {
            write_indent(out, depth);
            out.push('<');
            out.push_str(tag);
            out.push('>');
            out.push_str(&escape_xml_text(s));
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
            out.push('\n');
        }
        Value::Bool(b) => {
            write_indent(out, depth);
            out.push('<');
            out.push_str(tag);
            out.push('>');
            out.push_str(if *b { "true" } else { "false" });
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
            out.push('\n');
        }
        Value::Number(n) => {
            write_indent(out, depth);
            out.push('<');
            out.push_str(tag);
            out.push('>');
            out.push_str(&n.to_string());
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
            out.push('\n');
        }
    }
}

fn write_indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn sanitize_xml_tag(input: &str, fallback: &str) -> String {
    let mut out = String::new();
    for ch in input.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        return fallback.to_string();
    }
    if let Some(first) = out.chars().next() {
        if !first.is_ascii_alphabetic() && first != '_' {
            out.insert(0, '_');
        }
    }
    out
}

fn escape_xml_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "json_to_xml" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let v: Value = serde_json::from_str(input).map_err(|e| format!("invalid json: {e}"))?;
            let root_tag = payload["rootTag"]
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("root");
            Ok(json!(json_to_xml(root_tag, &v)))
        }
        "xml_to_json" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let v: Value = quick_xml::de::from_str(input).map_err(|e| format!("invalid xml: {e}"))?;
            Ok(json!(serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into())))
        }
        "json_to_yaml" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let v: Value = serde_json::from_str(input).map_err(|e| format!("invalid json: {e}"))?;
            let out = serde_yaml::to_string(&v).map_err(|e| format!("json->yaml failed: {e}"))?;
            Ok(json!(out))
        }
        "csv_to_json" => {
            let input = payload["input"].as_str().unwrap_or_default();
            let delimiter = payload["delimiter"].as_str().unwrap_or(",").as_bytes()[0];
            let has_header = payload["hasHeader"].as_bool().unwrap_or(true);
            let custom_headers: Option<Vec<String>> = payload["customHeaders"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());
            let selected_columns: Option<Vec<usize>> = payload["selectedColumns"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect());

            let mut rdr = csv::ReaderBuilder::new()
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
                let mut peek_rdr = csv::ReaderBuilder::new()
                    .delimiter(delimiter)
                    .has_headers(false)
                    .from_reader(input.as_bytes());
                let count = peek_rdr.records().next()
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
            Ok(json!(serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".into())))
        }
        "csv_read_file" => {
            let path = payload["path"].as_str().unwrap_or_default();
            if path.is_empty() {
                return Err("file path is empty".into());
            }
            let bytes = fs::read(path)
                .map_err(|e| format!("read csv file failed: {e}"))?;
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
        _ => Err(format!("unsupported convert action: {action}")),
    }
}

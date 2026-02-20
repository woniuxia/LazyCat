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

fn java_type_to_json_value(java_type: &str) -> Value {
    let t = java_type.trim().to_ascii_lowercase();
    if t.contains("list<") || t.contains("set<") || t.ends_with("[]") {
        return json!([]);
    }
    if t.contains("map<") {
        return json!({});
    }
    if [
        "string",
        "char",
        "character",
        "localdate",
        "localdatetime",
        "instant",
        "date",
    ]
    .iter()
    .any(|k| t.ends_with(k))
    {
        if t.ends_with("localdate") {
            return json!("1970-01-01");
        }
        if t.ends_with("localdatetime") || t.ends_with("instant") || t.ends_with("date") {
            return json!("1970-01-01T00:00:00Z");
        }
        return json!("");
    }
    if [
        "int",
        "integer",
        "long",
        "short",
        "byte",
        "atomicinteger",
        "atomiclong",
    ]
    .iter()
    .any(|k| t.ends_with(k))
    {
        return json!(0);
    }
    if ["double", "float", "bigdecimal", "biginteger"]
        .iter()
        .any(|k| t.ends_with(k))
    {
        return json!(0.0);
    }
    if ["boolean", "bool"].iter().any(|k| t.ends_with(k)) {
        return json!(false);
    }
    json!({})
}

fn json_to_js_object_literal(value: &Value, indent: usize, quote: char) -> String {
    let indent_str = "  ".repeat(indent);
    let next_indent = "  ".repeat(indent + 1);
    match value {
        Value::Null => "null".into(),
        Value::Bool(b) => {
            if *b {
                "true".into()
            } else {
                "false".into()
            }
        }
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            let escaped = s
                .replace('\\', "\\\\")
                .replace(quote, &format!("\\{quote}"))
                .replace('\n', "\\n");
            format!("{quote}{escaped}{quote}")
        }
        Value::Array(arr) => {
            if arr.is_empty() {
                return "[]".into();
            }
            let items = arr
                .iter()
                .map(|v| format!("{next_indent}{}", json_to_js_object_literal(v, indent + 1, quote)))
                .collect::<Vec<_>>()
                .join(",\n");
            format!("[\n{items}\n{indent_str}]")
        }
        Value::Object(map) => {
            if map.is_empty() {
                return "{}".into();
            }
            let mut lines = Vec::new();
            for (k, v) in map {
                let key = if k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$') {
                    k.clone()
                } else {
                    format!("{quote}{k}{quote}")
                };
                let vv = json_to_js_object_literal(v, indent + 1, quote);
                lines.push(format!("{next_indent}{key}: {vv}"));
            }
            format!("{{\n{}\n{indent_str}}}", lines.join(",\n"))
        }
    }
}

fn strip_java_comments(input: &str) -> String {
    let mut out = input.to_string();
    let re_block = regex::Regex::new(r"/\*[\s\S]*?\*/").expect("valid regex");
    out = re_block.replace_all(&out, "").to_string();
    let re_line = regex::Regex::new(r"//.*").expect("valid regex");
    re_line.replace_all(&out, "").to_string()
}

fn parse_java_fields(bean: &str) -> (serde_json::Map<String, Value>, Vec<Value>, Vec<String>) {
    let clean = strip_java_comments(bean);
    let mut map = serde_json::Map::new();
    let mut fields = Vec::new();
    let mut warnings = Vec::new();

    let field_re = regex::Regex::new(
        r#"(?m)^\s*(?:@\w+(?:\([^)]*\))?\s*)*(?:public|private|protected)?\s*(?:static\s+)?(?:final\s+)?(?:transient\s+)?(?:volatile\s+)?([A-Za-z_][\w<>, ?\[\].]*)\s+([A-Za-z_][\w]*)\s*(?:=[^;]+)?;"#,
    )
    .expect("valid regex");
    let ann_re =
        regex::Regex::new(r#"@JsonProperty\(\s*"([^"]+)"\s*\)"#).expect("valid regex");

    let mut pending_ann = String::new();
    for line in clean.lines() {
        let t = line.trim();
        if t.starts_with("@JsonProperty") {
            pending_ann = t.to_string();
        }
        if let Some(cap) = field_re.captures(t) {
            let java_type = cap.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            let field_name = cap.get(2).map(|m| m.as_str().trim()).unwrap_or("");
            if field_name == "serialVersionUID" {
                pending_ann.clear();
                continue;
            }
            let json_name = if !pending_ann.is_empty() {
                ann_re
                    .captures(&pending_ann)
                    .and_then(|c| c.get(1))
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(|| field_name.to_string())
            } else {
                field_name.to_string()
            };
            map.insert(json_name.clone(), java_type_to_json_value(java_type));
            fields.push(json!({
                "javaType": java_type,
                "name": field_name,
                "jsonName": json_name
            }));
            pending_ann.clear();
        } else if !t.starts_with('@') && !t.is_empty() {
            pending_ann.clear();
        }
    }
    if map.is_empty() {
        warnings.push("no fields parsed from bean source".into());
    }
    (map, fields, warnings)
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
        "java_bean_to_json" => {
            let bean = payload["bean"].as_str().unwrap_or_default();
            if bean.trim().is_empty() {
                return Err("bean is empty".into());
            }
            let (map, fields, warnings) = parse_java_fields(bean);
            Ok(json!({
                "json": serde_json::to_string_pretty(&Value::Object(map.clone())).unwrap_or_else(|_| "{}".into()),
                "fields": fields,
                "warnings": warnings
            }))
        }
        "json_to_js_object" => {
            let json_input = payload["json"].as_str().unwrap_or_default();
            if json_input.trim().is_empty() {
                return Err("json is empty".into());
            }
            let quote_style = payload["quoteStyle"].as_str().unwrap_or("single");
            let quote = if quote_style.eq_ignore_ascii_case("double") { '"' } else { '\'' };
            let value: Value =
                serde_json::from_str(json_input).map_err(|e| format!("invalid json: {e}"))?;
            let body = json_to_js_object_literal(&value, 0, quote);
            Ok(json!({
                "jsObject": format!("const payload = {body};")
            }))
        }
        "java_bean_to_js_object" => {
            let bean = payload["bean"].as_str().unwrap_or_default();
            if bean.trim().is_empty() {
                return Err("bean is empty".into());
            }
            let quote_style = payload["quoteStyle"].as_str().unwrap_or("single");
            let quote = if quote_style.eq_ignore_ascii_case("double") { '"' } else { '\'' };
            let (map, fields, warnings) = parse_java_fields(bean);
            let value = Value::Object(map.clone());
            let body = json_to_js_object_literal(&value, 0, quote);
            Ok(json!({
                "json": serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into()),
                "jsObject": format!("const payload = {body};"),
                "fields": fields,
                "warnings": warnings
            }))
        }
        _ => Err(format!("unsupported convert action: {action}")),
    }
}

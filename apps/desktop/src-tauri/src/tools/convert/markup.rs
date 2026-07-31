use serde_json::{json, Value};

fn json_to_xml_string(root_tag: &str, value: &Value) -> String {
    let root = sanitize_xml_tag(root_tag, "root");
    let mut out = String::new();
    append_xml_node_pretty(&mut out, &root, value, 0);
    out.trim_end_matches('\n').to_string()
}

pub(super) fn json_to_xml(payload: &Value) -> Result<Value, String> {
    let input = payload["input"].as_str().unwrap_or_default();
    let value: Value = serde_json::from_str(input).map_err(|e| format!("invalid json: {e}"))?;
    let root_tag = payload["rootTag"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("root");
    Ok(json!(json_to_xml_string(root_tag, &value)))
}

pub(super) fn xml_to_json(payload: &Value) -> Result<Value, String> {
    let input = payload["input"].as_str().unwrap_or_default();
    let value: Value = quick_xml::de::from_str(input).map_err(|e| format!("invalid xml: {e}"))?;
    Ok(json!(
        serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into())
    ))
}

pub(super) fn json_to_yaml(payload: &Value) -> Result<Value, String> {
    let input = payload["input"].as_str().unwrap_or_default();
    let value: Value = serde_json::from_str(input).map_err(|e| format!("invalid json: {e}"))?;
    let output = serde_norway::to_string(&value).map_err(|e| format!("json->yaml failed: {e}"))?;
    Ok(json!(output))
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

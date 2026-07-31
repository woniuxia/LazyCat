use serde_json::{json, Value};

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
                .map(|v| {
                    format!(
                        "{next_indent}{}",
                        json_to_js_object_literal(v, indent + 1, quote)
                    )
                })
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
                let key = if k
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
                {
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
    let ann_re = regex::Regex::new(r#"@JsonProperty\(\s*"([^"]+)"\s*\)"#).expect("valid regex");

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

pub(super) fn java_bean_to_json(payload: &Value) -> Result<Value, String> {
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

pub(super) fn json_to_js_object(payload: &Value) -> Result<Value, String> {
    let json_input = payload["json"].as_str().unwrap_or_default();
    if json_input.trim().is_empty() {
        return Err("json is empty".into());
    }
    let quote_style = payload["quoteStyle"].as_str().unwrap_or("single");
    let quote = if quote_style.eq_ignore_ascii_case("double") {
        '"'
    } else {
        '\''
    };
    let value: Value =
        serde_json::from_str(json_input).map_err(|e| format!("invalid json: {e}"))?;
    let body = json_to_js_object_literal(&value, 0, quote);
    Ok(json!({
        "jsObject": format!("const payload = {body};")
    }))
}

pub(super) fn java_bean_to_js_object(payload: &Value) -> Result<Value, String> {
    let bean = payload["bean"].as_str().unwrap_or_default();
    if bean.trim().is_empty() {
        return Err("bean is empty".into());
    }
    let quote_style = payload["quoteStyle"].as_str().unwrap_or("single");
    let quote = if quote_style.eq_ignore_ascii_case("double") {
        '"'
    } else {
        '\''
    };
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

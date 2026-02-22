use serde_json::{json, Value};
use std::fs;
use std::collections::BTreeMap;

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

fn parse_properties(input: &str) -> Result<Value, String> {
    let mut root = serde_json::Map::new();
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }
        if let Some(pos) = trimmed.find('=') {
            let key = trimmed[..pos].trim();
            let val = trimmed[pos + 1..].trim();
            // Support nested keys: a.b.c = v
            let parts: Vec<&str> = key.split('.').collect();
            set_nested(&mut root, &parts, Value::String(val.to_string()));
        }
    }
    Ok(Value::Object(root))
}

fn set_nested(map: &mut serde_json::Map<String, Value>, parts: &[&str], value: Value) {
    if parts.len() == 1 {
        map.insert(parts[0].to_string(), value);
        return;
    }
    let entry = map
        .entry(parts[0].to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    if let Value::Object(ref mut child) = entry {
        set_nested(child, &parts[1..], value);
    }
}

fn parse_env(input: &str) -> Result<Value, String> {
    let mut map = serde_json::Map::new();
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(pos) = trimmed.find('=') {
            let key = trimmed[..pos].trim();
            let val = trimmed[pos + 1..].trim().trim_matches('"').trim_matches('\'');
            map.insert(key.to_string(), Value::String(val.to_string()));
        }
    }
    Ok(Value::Object(map))
}

fn serialize_properties(value: &Value) -> String {
    let mut lines = Vec::new();
    flatten_value(value, "", &mut lines);
    lines.sort();
    lines.join("\n")
}

fn flatten_value(value: &Value, prefix: &str, lines: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_value(v, &key, lines);
            }
        }
        _ => {
            let s = match value {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            lines.push(format!("{prefix}={s}"));
        }
    }
}

fn serialize_env(value: &Value) -> String {
    let mut lines = Vec::new();
    if let Value::Object(map) = value {
        let sorted: BTreeMap<_, _> = map.iter().collect();
        for (k, v) in sorted {
            let s = match v {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            lines.push(format!("{k}={s}"));
        }
    } else {
        flatten_value(value, "", &mut lines);
        lines.sort();
    }
    lines.join("\n")
}

// ── SQL to Entity Class ──────────────────────────────────────────

#[derive(Debug, Clone)]
struct SqlColumn {
    name: String,
    sql_type: String,
    nullable: bool,
    default_val: Option<String>,
    comment: Option<String>,
}

#[derive(Debug, Clone)]
struct SqlTable {
    name: String,
    columns: Vec<SqlColumn>,
}

fn parse_create_tables(sql: &str) -> Vec<SqlTable> {
    let re_table = regex::Regex::new(
        r#"(?is)CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?[`"\[]?(\w+)[`"\]]?\s*\("#
    ).unwrap();

    let mut tables = Vec::new();
    for cap in re_table.captures_iter(sql) {
        let table_name = cap.get(1).unwrap().as_str().to_string();
        let start = cap.get(0).unwrap().end();
        // Find the matching closing paren, respecting nesting
        if let Some(body) = find_paren_body(sql, start) {
            let columns = parse_columns(&body);
            tables.push(SqlTable { name: table_name, columns });
        }
    }
    tables
}

fn find_paren_body(sql: &str, start: usize) -> Option<String> {
    let bytes = sql.as_bytes();
    let mut depth = 1;
    let mut i = start;
    let mut in_single_quote = false;
    while i < bytes.len() && depth > 0 {
        let ch = bytes[i] as char;
        if in_single_quote {
            if ch == '\'' {
                // Check for escaped quote ''
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    i += 2;
                    continue;
                }
                in_single_quote = false;
            }
        } else {
            match ch {
                '\'' => in_single_quote = true,
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(sql[start..i].to_string());
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

fn parse_columns(body: &str) -> Vec<SqlColumn> {
    let mut columns = Vec::new();
    // Split by top-level commas (not inside parentheses)
    let parts = split_top_level_commas(body);

    let re_col = regex::Regex::new(
        r#"(?i)^\s*[`"\[]?(\w+)[`"\]]?\s+([\w]+(?:\s*\([^)]*\))?(?:\s+(?:UNSIGNED|SIGNED|ZEROFILL))*)"#
    ).unwrap();
    let re_not_null = regex::Regex::new(r"(?i)\bNOT\s+NULL\b").unwrap();
    let re_default = regex::Regex::new(r"(?i)\bDEFAULT\s+('(?:[^']*(?:''[^']*)*)'|[^\s,]+)").unwrap();
    let re_comment = regex::Regex::new(r"(?i)\bCOMMENT\s+'((?:[^']*(?:''[^']*)*)*)'").unwrap();

    // Keywords that indicate a constraint, not a column definition
    let constraint_keywords = [
        "PRIMARY", "KEY", "UNIQUE", "INDEX", "CONSTRAINT", "CHECK", "FOREIGN",
        "FULLTEXT", "SPATIAL", "PARTITION",
    ];

    for part in &parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Skip constraint lines
        let first_word = trimmed.split_whitespace().next().unwrap_or("").to_uppercase();
        let first_word_clean = first_word.trim_start_matches('`').trim_start_matches('"').trim_start_matches('[');
        if constraint_keywords.iter().any(|k| first_word_clean == *k) {
            continue;
        }
        if let Some(cap) = re_col.captures(trimmed) {
            let col_name = cap.get(1).unwrap().as_str().to_string();
            let col_type = cap.get(2).unwrap().as_str().trim().to_string();
            let nullable = !re_not_null.is_match(trimmed);
            let default_val = re_default.captures(trimmed)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim_matches('\'').to_string());
            let comment = re_comment.captures(trimmed)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().replace("''", "'"));
            columns.push(SqlColumn {
                name: col_name,
                sql_type: col_type,
                nullable,
                default_val,
                comment,
            });
        }
    }
    columns
}

fn split_top_level_commas(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    let mut in_quote = false;
    for ch in s.chars() {
        if in_quote {
            current.push(ch);
            if ch == '\'' {
                in_quote = false;
            }
        } else {
            match ch {
                '\'' => { in_quote = true; current.push(ch); }
                '(' => { depth += 1; current.push(ch); }
                ')' => { depth -= 1; current.push(ch); }
                ',' if depth == 0 => {
                    parts.push(current.clone());
                    current.clear();
                }
                _ => current.push(ch),
            }
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

fn to_camel_case(name: &str) -> String {
    let mut result = String::new();
    let mut upper_next = false;
    for ch in name.chars() {
        if ch == '_' {
            upper_next = true;
        } else if upper_next {
            result.push(ch.to_ascii_uppercase());
            upper_next = false;
        } else {
            result.push(ch.to_ascii_lowercase());
        }
    }
    result
}

fn to_pascal_case(name: &str) -> String {
    let camel = to_camel_case(name);
    let mut chars = camel.chars();
    match chars.next() {
        Some(c) => c.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

fn convert_field_name(name: &str, naming: &str) -> String {
    match naming {
        "camelCase" => to_camel_case(name),
        "snake_case" => name.to_ascii_lowercase(),
        _ => name.to_string(), // "original"
    }
}

fn table_name_to_class(name: &str) -> String {
    // Remove common prefixes like t_, tb_, tbl_
    let stripped = regex::Regex::new(r"^(?i)(t_|tb_|tbl_)")
        .unwrap()
        .replace(name, "")
        .to_string();
    to_pascal_case(&stripped)
}

struct TypeMapping {
    java: &'static str,
    typescript: &'static str,
    go: &'static str,
    python: &'static str,
    kotlin: &'static str,
    csharp: &'static str,
}

fn map_sql_type(sql_type: &str) -> TypeMapping {
    let upper = sql_type.to_uppercase();
    let base = upper.split('(').next().unwrap_or("").trim();

    // TINYINT(1) is boolean
    if base == "TINYINT" {
        let re = regex::Regex::new(r"\(\s*1\s*\)").unwrap();
        if re.is_match(&upper) {
            return TypeMapping {
                java: "Boolean", typescript: "boolean", go: "bool",
                python: "bool", kotlin: "Boolean", csharp: "bool",
            };
        }
    }

    match base {
        "VARCHAR" | "CHAR" | "TEXT" | "TINYTEXT" | "MEDIUMTEXT" | "LONGTEXT" | "NVARCHAR" | "NCHAR" | "CLOB" => TypeMapping {
            java: "String", typescript: "string", go: "string",
            python: "str", kotlin: "String", csharp: "string",
        },
        "INT" | "INTEGER" | "SMALLINT" | "TINYINT" | "MEDIUMINT" => TypeMapping {
            java: "Integer", typescript: "number", go: "int32",
            python: "int", kotlin: "Int", csharp: "int",
        },
        "BIGINT" => TypeMapping {
            java: "Long", typescript: "number", go: "int64",
            python: "int", kotlin: "Long", csharp: "long",
        },
        "DATETIME" | "TIMESTAMP" => TypeMapping {
            java: "LocalDateTime", typescript: "Date", go: "time.Time",
            python: "datetime", kotlin: "LocalDateTime", csharp: "DateTime",
        },
        "DATE" => TypeMapping {
            java: "LocalDate", typescript: "string", go: "time.Time",
            python: "date", kotlin: "LocalDate", csharp: "DateTime",
        },
        "TIME" => TypeMapping {
            java: "LocalTime", typescript: "string", go: "string",
            python: "time", kotlin: "LocalTime", csharp: "TimeSpan",
        },
        "DECIMAL" | "NUMERIC" => TypeMapping {
            java: "BigDecimal", typescript: "number", go: "float64",
            python: "Decimal", kotlin: "BigDecimal", csharp: "decimal",
        },
        "BOOLEAN" | "BOOL" => TypeMapping {
            java: "Boolean", typescript: "boolean", go: "bool",
            python: "bool", kotlin: "Boolean", csharp: "bool",
        },
        "FLOAT" | "REAL" => TypeMapping {
            java: "Float", typescript: "number", go: "float32",
            python: "float", kotlin: "Float", csharp: "float",
        },
        "DOUBLE" | "DOUBLE PRECISION" => TypeMapping {
            java: "Double", typescript: "number", go: "float64",
            python: "float", kotlin: "Double", csharp: "double",
        },
        "BLOB" | "BINARY" | "VARBINARY" | "TINYBLOB" | "MEDIUMBLOB" | "LONGBLOB" | "BYTEA" => TypeMapping {
            java: "byte[]", typescript: "Buffer", go: "[]byte",
            python: "bytes", kotlin: "ByteArray", csharp: "byte[]",
        },
        "BIT" => TypeMapping {
            java: "Boolean", typescript: "boolean", go: "bool",
            python: "bool", kotlin: "Boolean", csharp: "bool",
        },
        _ => TypeMapping {
            java: "String", typescript: "string", go: "string",
            python: "str", kotlin: "String", csharp: "string",
        },
    }
}

fn get_type_for_lang(sql_type: &str, lang: &str) -> String {
    let m = map_sql_type(sql_type);
    match lang {
        "java" => m.java.to_string(),
        "typescript" => m.typescript.to_string(),
        "go" => m.go.to_string(),
        "python" => m.python.to_string(),
        "kotlin" => m.kotlin.to_string(),
        "csharp" => m.csharp.to_string(),
        _ => m.java.to_string(),
    }
}

fn generate_java(table: &SqlTable, naming: &str, comments: bool) -> String {
    let class_name = table_name_to_class(&table.name);
    let mut out = String::new();

    // Collect imports
    let mut imports = vec!["lombok.Data".to_string()];
    for col in &table.columns {
        let type_name = get_type_for_lang(&col.sql_type, "java");
        match type_name.as_str() {
            "LocalDateTime" => { imports.push("java.time.LocalDateTime".into()); }
            "LocalDate" => { imports.push("java.time.LocalDate".into()); }
            "LocalTime" => { imports.push("java.time.LocalTime".into()); }
            "BigDecimal" => { imports.push("java.math.BigDecimal".into()); }
            _ => {}
        }
    }
    imports.sort();
    imports.dedup();
    for imp in &imports {
        out.push_str(&format!("import {};\n", imp));
    }
    out.push('\n');

    out.push_str("@Data\n");
    out.push_str(&format!("public class {} {{\n", class_name));

    for col in &table.columns {
        if comments {
            if let Some(ref c) = col.comment {
                out.push_str(&format!("    /** {} */\n", c));
            }
        }
        let field_name = convert_field_name(&col.name, naming);
        let type_name = get_type_for_lang(&col.sql_type, "java");
        out.push_str(&format!("    private {} {};\n", type_name, field_name));
        out.push('\n');
    }

    out.push_str("}\n");
    out
}

fn generate_typescript(table: &SqlTable, naming: &str, comments: bool) -> String {
    let class_name = table_name_to_class(&table.name);
    let mut out = String::new();
    out.push_str(&format!("export interface {} {{\n", class_name));
    for col in &table.columns {
        if comments {
            if let Some(ref c) = col.comment {
                out.push_str(&format!("  /** {} */\n", c));
            }
        }
        let field_name = convert_field_name(&col.name, naming);
        let type_name = get_type_for_lang(&col.sql_type, "typescript");
        let nullable_mark = if col.nullable { "?" } else { "" };
        out.push_str(&format!("  {}{}: {};\n", field_name, nullable_mark, type_name));
    }
    out.push_str("}\n");
    out
}

fn generate_go(table: &SqlTable, naming: &str, comments: bool) -> String {
    let struct_name = table_name_to_class(&table.name);
    let mut out = String::new();
    out.push_str(&format!("type {} struct {{\n", struct_name));
    for col in &table.columns {
        let field_name = to_pascal_case(&col.name);
        let type_name = get_type_for_lang(&col.sql_type, "go");
        // Pointer type for nullable fields
        let go_type = if col.nullable && !type_name.starts_with("[]") {
            format!("*{}", type_name)
        } else {
            type_name
        };
        let db_name = convert_field_name(&col.name, naming);
        let comment_str = if comments {
            col.comment.as_ref().map(|c| format!(" // {}", c)).unwrap_or_default()
        } else {
            String::new()
        };
        out.push_str(&format!(
            "\t{} {} `json:\"{}\" db:\"{}\"`{}\n",
            field_name, go_type, db_name, col.name.to_ascii_lowercase(), comment_str
        ));
    }
    out.push_str("}\n");
    out
}

fn generate_python(table: &SqlTable, naming: &str, comments: bool) -> String {
    let class_name = table_name_to_class(&table.name);
    let mut out = String::new();
    out.push_str("from dataclasses import dataclass\n");
    out.push_str("from typing import Optional\n");
    // Check if we need date imports
    let needs_datetime = table.columns.iter().any(|c| {
        let u = c.sql_type.to_uppercase();
        let b = u.split('(').next().unwrap_or("").trim();
        matches!(b, "DATETIME" | "TIMESTAMP" | "DATE" | "TIME")
    });
    let needs_decimal = table.columns.iter().any(|c| {
        let u = c.sql_type.to_uppercase();
        let b = u.split('(').next().unwrap_or("").trim();
        matches!(b, "DECIMAL" | "NUMERIC")
    });
    if needs_datetime {
        out.push_str("from datetime import datetime, date, time\n");
    }
    if needs_decimal {
        out.push_str("from decimal import Decimal\n");
    }
    out.push('\n');
    out.push('\n');
    out.push_str("@dataclass\n");
    out.push_str(&format!("class {}:\n", class_name));
    if comments {
        // Class docstring with table name
        out.push_str(&format!("    \"\"\"Table: {}\"\"\"\n\n", table.name));
    }
    for col in &table.columns {
        let field_name = convert_field_name(&col.name, naming);
        let type_name = get_type_for_lang(&col.sql_type, "python");
        let full_type = if col.nullable {
            format!("Optional[{}]", type_name)
        } else {
            type_name
        };
        let default_str = if col.nullable { " = None" } else { "" };
        if comments {
            if let Some(ref c) = col.comment {
                out.push_str(&format!("    # {}\n", c));
            }
        }
        out.push_str(&format!("    {}: {}{}\n", field_name, full_type, default_str));
    }
    out
}

fn generate_kotlin(table: &SqlTable, naming: &str, comments: bool) -> String {
    let class_name = table_name_to_class(&table.name);
    let mut out = String::new();
    out.push_str(&format!("data class {}(\n", class_name));
    let len = table.columns.len();
    for (i, col) in table.columns.iter().enumerate() {
        if comments {
            if let Some(ref c) = col.comment {
                out.push_str(&format!("    /** {} */\n", c));
            }
        }
        let field_name = convert_field_name(&col.name, naming);
        let type_name = get_type_for_lang(&col.sql_type, "kotlin");
        let nullable_mark = if col.nullable { "?" } else { "" };
        let comma = if i < len - 1 { "," } else { "" };
        out.push_str(&format!("    val {}: {}{}{}\n", field_name, type_name, nullable_mark, comma));
    }
    out.push_str(")\n");
    out
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

fn generate_csharp(table: &SqlTable, naming: &str, comments: bool) -> String {
    let class_name = table_name_to_class(&table.name);
    let mut out = String::new();
    out.push_str(&format!("public class {}\n{{\n", class_name));
    for col in &table.columns {
        if comments {
            if let Some(ref c) = col.comment {
                out.push_str(&format!("    /// <summary>{}</summary>\n", c));
            }
        }
        let field_name = match naming {
            "camelCase" => capitalize_first(&to_camel_case(&col.name)),
            "snake_case" => capitalize_first(&col.name.to_ascii_lowercase()),
            _ => capitalize_first(&col.name),
        };
        let type_name = get_type_for_lang(&col.sql_type, "csharp");
        let nullable_mark = if col.nullable { "?" } else { "" };
        out.push_str(&format!("    public {}{} {} {{ get; set; }}\n\n", type_name, nullable_mark, field_name));
    }
    out.push_str("}\n");
    out
}

fn sql_to_entity(payload: &Value) -> Result<Value, String> {
    let sql = payload["sql"].as_str().unwrap_or_default();
    if sql.trim().is_empty() {
        return Err("SQL is empty".into());
    }
    let language = payload["language"].as_str().unwrap_or("java").to_lowercase();
    let options = &payload["options"];
    let comments = options["comments"].as_bool().unwrap_or(true);
    let naming = options["naming"].as_str().unwrap_or("camelCase");

    let tables = parse_create_tables(sql);
    if tables.is_empty() {
        return Err("No CREATE TABLE statements found".into());
    }

    let mut code_parts: Vec<String> = Vec::new();
    let mut table_infos: Vec<Value> = Vec::new();

    for table in &tables {
        let code = match language.as_str() {
            "java" => generate_java(table, naming, comments),
            "typescript" => generate_typescript(table, naming, comments),
            "go" => generate_go(table, naming, comments),
            "python" => generate_python(table, naming, comments),
            "kotlin" => generate_kotlin(table, naming, comments),
            "csharp" => generate_csharp(table, naming, comments),
            _ => return Err(format!("Unsupported language: {}", language)),
        };
        code_parts.push(code);

        let col_infos: Vec<Value> = table.columns.iter().map(|c| {
            json!({
                "name": c.name,
                "type": c.sql_type,
                "nullable": c.nullable,
                "default": c.default_val,
                "comment": c.comment,
            })
        }).collect();

        table_infos.push(json!({
            "name": table.name,
            "columns": col_infos,
        }));
    }

    let code = code_parts.join("\n");
    Ok(json!({
        "code": code,
        "tables": table_infos,
    }))
}

fn config_convert(payload: &Value) -> Result<Value, String> {
    let input = payload["input"].as_str().unwrap_or_default();
    let from = payload["from"].as_str().unwrap_or_default();
    let to = payload["to"].as_str().unwrap_or_default();

    let intermediate: Value = match from {
        "properties" => parse_properties(input)?,
        "yaml" => serde_yml::from_str(input).map_err(|e| format!("YAML 解析失败: {e}"))?,
        "toml" => toml::from_str(input).map_err(|e| format!("TOML 解析失败: {e}"))?,
        "env" => parse_env(input)?,
        _ => return Err(format!("不支持的源格式: {from}")),
    };

    let output = match to {
        "properties" => serialize_properties(&intermediate),
        "yaml" => serde_yml::to_string(&intermediate).map_err(|e| format!("YAML 序列化失败: {e}"))?,
        "toml" => toml::to_string_pretty(&intermediate).map_err(|e| format!("TOML 序列化失败: {e}"))?,
        "env" => serialize_env(&intermediate),
        _ => return Err(format!("不支持的目标格式: {to}")),
    };

    Ok(json!({ "output": output }))
}

fn yaml_validate(payload: &Value) -> Result<Value, String> {
    let input = payload["input"].as_str().unwrap_or_default();
    match serde_yml::from_str::<Value>(input) {
        Ok(_) => Ok(json!({ "valid": true, "error": null })),
        Err(e) => {
            let loc = e.location();
            Ok(json!({
                "valid": false,
                "error": {
                    "line": loc.map(|l| l.line()).unwrap_or(0),
                    "message": e.to_string(),
                }
            }))
        }
    }
}

fn yaml_format(payload: &Value) -> Result<Value, String> {
    let input = payload["input"].as_str().unwrap_or_default();
    let value: Value = serde_yml::from_str(input).map_err(|e| format!("YAML 解析失败: {e}"))?;
    let output = serde_yml::to_string(&value).map_err(|e| format!("YAML 序列化失败: {e}"))?;
    Ok(json!({ "output": output }))
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
            let out = serde_yml::to_string(&v).map_err(|e| format!("json->yaml failed: {e}"))?;
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
        "config_convert" => config_convert(payload),
        "yaml_validate" => yaml_validate(payload),
        "yaml_format" => yaml_format(payload),
        "sql_to_entity" => sql_to_entity(payload),
        _ => Err(format!("unsupported convert action: {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    #[test]
    fn json_xml_yaml_conversions_should_work() {
        let input = r#"{"name":"lazycat","age":1,"active":true}"#;
        let xml = execute("json_to_xml", &json!({ "input": input, "rootTag": "user" }))
            .expect("json_to_xml");
        let xml_text = xml.as_str().expect("xml string");
        assert!(xml_text.contains("<user>"));
        assert!(xml_text.contains("<name>lazycat</name>"));

        let xml_input = "<root><name>lazycat</name><age>1</age></root>";
        let json_out = execute("xml_to_json", &json!({ "input": xml_input })).expect("xml_to_json");
        let json_text = json_out.as_str().expect("json string");
        assert!(json_text.contains("\"name\""));

        let yaml = execute("json_to_yaml", &json!({ "input": input })).expect("json_to_yaml");
        let yaml_text = yaml.as_str().expect("yaml string");
        assert!(yaml_text.contains("name: lazycat"));
    }

    #[test]
    fn csv_to_json_should_support_header_and_selected_columns() {
        let csv = "name,age,city\nalice,18,sz\nbob,20,sh";
        let out = execute(
            "csv_to_json",
            &json!({
                "input": csv,
                "delimiter": ",",
                "hasHeader": true,
                "selectedColumns": [0, 2]
            }),
        )
        .expect("csv_to_json");
        let text = out.as_str().expect("json text");
        assert!(text.contains("\"name\""));
        assert!(text.contains("\"city\""));
        assert!(!text.contains("\"age\""));
    }

    #[test]
    fn csv_to_json_without_header_should_generate_col_names() {
        let csv = "a,1\nb,2";
        let out = execute(
            "csv_to_json",
            &json!({
                "input": csv,
                "delimiter": ",",
                "hasHeader": false
            }),
        )
        .expect("csv_to_json");
        let text = out.as_str().expect("json text");
        assert!(text.contains("\"col1\""));
        assert!(text.contains("\"col2\""));
    }

    #[test]
    fn csv_read_file_should_read_utf8_and_fail_for_missing_file() {
        let path = std::env::temp_dir().join(format!("lazycat-convert-{}.csv", std::process::id()));
        fs::write(&path, "姓名,年龄\n猫,2".as_bytes()).expect("write temp csv");
        let out = execute(
            "csv_read_file",
            &json!({ "path": path.to_string_lossy().to_string() }),
        )
        .expect("csv_read_file");
        assert!(out.as_str().unwrap_or_default().contains("姓名"));
        let _ = fs::remove_file(&path);

        let err = execute("csv_read_file", &json!({ "path": "__not_found__.csv" }))
            .expect_err("should fail");
        assert!(err.contains("read csv file failed"));
    }

    #[test]
    fn java_bean_and_json_to_js_object_should_work() {
        let bean = r#"
            public class User {
              @JsonProperty("user_name")
              private String name;
              private Integer age;
              private List<String> tags;
            }
        "#;
        let out = execute("java_bean_to_json", &json!({ "bean": bean })).expect("java_bean_to_json");
        let obj = out.as_object().expect("object");
        let json_text = obj
            .get("json")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        assert!(json_text.contains("\"user_name\""));
        assert!(json_text.contains("\"age\""));
        assert!(json_text.contains("\"tags\""));

        let js = execute(
            "json_to_js_object",
            &json!({
                "json": r#"{"a":1,"b":"x"}"#,
                "quoteStyle": "double"
            }),
        )
        .expect("json_to_js_object");
        let js_text = js["jsObject"].as_str().unwrap_or_default();
        assert!(js_text.contains("const payload ="));

        let bean_js = execute(
            "java_bean_to_js_object",
            &json!({
                "bean": bean,
                "quoteStyle": "single"
            }),
        )
        .expect("java_bean_to_js_object");
        assert!(bean_js["jsObject"].as_str().unwrap_or_default().contains("const payload ="));
    }

    #[test]
    fn convert_invalid_inputs_should_fail() {
        let err = execute("json_to_xml", &json!({ "input": "{bad json}" })).expect_err("invalid json");
        assert!(err.contains("invalid json"));

        let err = execute("xml_to_json", &json!({ "input": "<root>" })).expect_err("invalid xml");
        assert!(err.contains("invalid xml"));

        let err = execute("java_bean_to_json", &json!({ "bean": "" })).expect_err("empty bean");
        assert!(err.contains("bean is empty"));

        let err = execute("json_to_js_object", &json!({ "json": "" })).expect_err("empty json");
        assert!(err.contains("json is empty"));
    }

    #[test]
    fn config_convert_properties_to_yaml() {
        let r = execute("config_convert", &json!({
            "input": "server.port=8080\nserver.host=localhost",
            "from": "properties",
            "to": "yaml"
        })).unwrap();
        let output = r["output"].as_str().unwrap();
        assert!(output.contains("server"));
        assert!(output.contains("8080"));
    }

    #[test]
    fn config_convert_yaml_to_toml() {
        let r = execute("config_convert", &json!({
            "input": "server:\n  port: 8080",
            "from": "yaml",
            "to": "toml"
        })).unwrap();
        let output = r["output"].as_str().unwrap();
        assert!(output.contains("[server]"));
        assert!(output.contains("8080"));
    }

    #[test]
    fn config_convert_env_to_properties() {
        let r = execute("config_convert", &json!({
            "input": "DB_HOST=localhost\nDB_PORT=5432",
            "from": "env",
            "to": "properties"
        })).unwrap();
        let output = r["output"].as_str().unwrap();
        assert!(output.contains("DB_HOST=localhost"));
    }

    #[test]
    fn yaml_validate_valid() {
        let r = execute("yaml_validate", &json!({"input": "key: value\nlist:\n  - a\n  - b"})).unwrap();
        assert_eq!(r["valid"], true);
        assert!(r["error"].is_null());
    }

    #[test]
    fn yaml_validate_invalid() {
        let r = execute("yaml_validate", &json!({"input": "key: [unclosed"})).unwrap();
        assert_eq!(r["valid"], false);
        assert!(r["error"]["message"].as_str().unwrap().len() > 0);
    }

    #[test]
    fn yaml_format_indent() {
        let r = execute("yaml_format", &json!({"input": "key:   value\nlist:\n    - a", "indent": 2})).unwrap();
        let output = r["output"].as_str().unwrap();
        assert!(output.contains("key:"));
    }

    #[test]
    fn sql_to_entity_java_basic() {
        let sql = r#"
            CREATE TABLE t_user (
                id BIGINT NOT NULL AUTO_INCREMENT COMMENT 'primary key',
                user_name VARCHAR(100) NOT NULL COMMENT 'user name',
                age INT DEFAULT 0,
                created_at DATETIME NOT NULL,
                PRIMARY KEY (id)
            );
        "#;
        let r = execute("sql_to_entity", &json!({
            "sql": sql,
            "language": "java",
            "options": { "comments": true, "naming": "camelCase" }
        })).unwrap();
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("public class User"));
        assert!(code.contains("private Long id;"));
        assert!(code.contains("private String userName;"));
        assert!(code.contains("private Integer age;"));
        assert!(code.contains("private LocalDateTime createdAt;"));
        assert!(code.contains("/** primary key */"));
        assert!(code.contains("@Data"));
        assert!(code.contains("import lombok.Data;"));
        assert!(!code.contains("getId()"));
        assert!(!code.contains("setUserName("));

        let tables = r["tables"].as_array().unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0]["name"], "t_user");
        let cols = tables[0]["columns"].as_array().unwrap();
        assert_eq!(cols.len(), 4);
        assert_eq!(cols[0]["name"], "id");
        assert_eq!(cols[0]["nullable"], false);
    }

    #[test]
    fn sql_to_entity_typescript() {
        let sql = "CREATE TABLE orders (id INT NOT NULL, total DECIMAL(10,2), status VARCHAR(20));";
        let r = execute("sql_to_entity", &json!({
            "sql": sql,
            "language": "typescript",
            "options": { "comments": false, "naming": "camelCase" }
        })).unwrap();
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("export interface Orders"));
        assert!(code.contains("id: number;"));
        assert!(code.contains("total?: number;"));
        assert!(code.contains("status?: string;"));
    }

    #[test]
    fn sql_to_entity_go() {
        let sql = "CREATE TABLE `product` (id BIGINT NOT NULL, name VARCHAR(255), price FLOAT);";
        let r = execute("sql_to_entity", &json!({
            "sql": sql,
            "language": "go",
            "options": { "comments": false, "naming": "camelCase" }
        })).unwrap();
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("type Product struct"));
        assert!(code.contains("Id int64"));
        assert!(code.contains("*string"));
        assert!(code.contains("json:\"name\""));
    }

    #[test]
    fn sql_to_entity_python() {
        let sql = "CREATE TABLE user_profile (user_id INT NOT NULL, bio TEXT, created DATE NOT NULL);";
        let r = execute("sql_to_entity", &json!({
            "sql": sql,
            "language": "python",
            "options": { "comments": true, "naming": "snake_case" }
        })).unwrap();
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("@dataclass"));
        assert!(code.contains("class UserProfile:"));
        assert!(code.contains("user_id: int"));
        assert!(code.contains("bio: Optional[str]"));
        assert!(code.contains("from datetime import"));
    }

    #[test]
    fn sql_to_entity_kotlin() {
        let sql = "CREATE TABLE IF NOT EXISTS config (id INT NOT NULL, value TEXT, active BOOLEAN);";
        let r = execute("sql_to_entity", &json!({
            "sql": sql,
            "language": "kotlin",
            "options": { "comments": false, "naming": "camelCase" }
        })).unwrap();
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("data class Config("));
        assert!(code.contains("val id: Int"));
        assert!(code.contains("val value: String?"));
        assert!(code.contains("val active: Boolean?"));
    }

    #[test]
    fn sql_to_entity_csharp() {
        let sql = "CREATE TABLE [order_items] (id INT NOT NULL, order_id BIGINT NOT NULL, quantity INT DEFAULT 1);";
        let r = execute("sql_to_entity", &json!({
            "sql": sql,
            "language": "csharp",
            "options": { "comments": false, "naming": "camelCase" }
        })).unwrap();
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("public class OrderItems"));
        assert!(code.contains("public int Id { get; set; }"));
        assert!(code.contains("public long OrderId { get; set; }"));
        assert!(code.contains("public int? Quantity { get; set; }"));
    }

    #[test]
    fn sql_to_entity_multiple_tables() {
        let sql = r#"
            CREATE TABLE users (id INT NOT NULL, name VARCHAR(100));
            CREATE TABLE roles (id INT NOT NULL, role_name VARCHAR(50) NOT NULL);
        "#;
        let r = execute("sql_to_entity", &json!({
            "sql": sql,
            "language": "java",
            "options": { "comments": false, "naming": "camelCase" }
        })).unwrap();
        let tables = r["tables"].as_array().unwrap();
        assert_eq!(tables.len(), 2);
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("public class Users"));
        assert!(code.contains("public class Roles"));
    }

    #[test]
    fn sql_to_entity_empty_input() {
        let err = execute("sql_to_entity", &json!({
            "sql": "",
            "language": "java",
            "options": {}
        })).expect_err("empty sql");
        assert!(err.contains("SQL is empty"));
    }

    #[test]
    fn sql_to_entity_no_create_table() {
        let err = execute("sql_to_entity", &json!({
            "sql": "SELECT * FROM users;",
            "language": "java",
            "options": {}
        })).expect_err("no create table");
        assert!(err.contains("No CREATE TABLE"));
    }

    #[test]
    fn sql_to_entity_tinyint1_is_boolean() {
        let sql = "CREATE TABLE flags (id INT NOT NULL, active TINYINT(1) NOT NULL, count TINYINT(4));";
        let r = execute("sql_to_entity", &json!({
            "sql": sql,
            "language": "java",
            "options": { "comments": false, "naming": "camelCase" }
        })).unwrap();
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("private Boolean active;"));
        assert!(code.contains("private Integer count;"));
    }

    #[test]
    fn sql_to_entity_snake_case_naming() {
        let sql = "CREATE TABLE user_settings (user_id INT NOT NULL, config_value TEXT);";
        let r = execute("sql_to_entity", &json!({
            "sql": sql,
            "language": "java",
            "options": { "comments": false, "naming": "snake_case" }
        })).unwrap();
        let code = r["code"].as_str().unwrap();
        assert!(code.contains("private Integer user_id;"));
        assert!(code.contains("private String config_value;"));
    }
}

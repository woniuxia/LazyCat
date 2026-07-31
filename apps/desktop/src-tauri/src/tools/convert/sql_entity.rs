use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Debug, Clone)]
struct SqlColumn {
    name: String,
    sql_type: String,
    nullable: bool,
    auto_increment: bool,
    default_val: Option<String>,
    comment: Option<String>,
}

#[derive(Debug, Clone)]
struct SqlTable {
    name: String,
    columns: Vec<SqlColumn>,
    primary_keys: Vec<String>,
}

fn parse_primary_keys(body: &str) -> Vec<String> {
    let parts = split_top_level_commas(body);
    let re_table_primary_key = regex::Regex::new(r#"(?i)^\s*PRIMARY\s+KEY\s*\(([^)]+)\)"#).unwrap();
    let re_column = regex::Regex::new(r#"(?i)^\s*[`"\[]?(\w+)[`"\]]?\s+"#).unwrap();
    let re_inline_primary_key = regex::Regex::new(r"(?i)\bPRIMARY\s+KEY\b").unwrap();
    let mut primary_keys = Vec::new();

    for part in parts {
        let trimmed = part.trim();
        if let Some(cap) = re_table_primary_key.captures(trimmed) {
            if let Some(columns) = cap.get(1) {
                for column in columns.as_str().split(',') {
                    let name = column
                        .trim()
                        .trim_matches('`')
                        .trim_matches('"')
                        .trim_matches('[')
                        .trim_matches(']');
                    if !name.is_empty()
                        && !primary_keys.iter().any(|key: &String| key.as_str() == name)
                    {
                        primary_keys.push(name.to_string());
                    }
                }
            }
            continue;
        }

        if re_inline_primary_key.is_match(trimmed) {
            if let Some(cap) = re_column.captures(trimmed) {
                let name = cap.get(1).unwrap().as_str();
                if !primary_keys.iter().any(|key: &String| key.as_str() == name) {
                    primary_keys.push(name.to_string());
                }
            }
        }
    }

    primary_keys
}

fn parse_create_tables(sql: &str) -> Vec<SqlTable> {
    let re_table = regex::Regex::new(
        r#"(?is)CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?[`"\[]?(\w+)[`"\]]?\s*\("#,
    )
    .unwrap();

    let mut tables = Vec::new();
    for cap in re_table.captures_iter(sql) {
        let table_name = cap.get(1).unwrap().as_str().to_string();
        let start = cap.get(0).unwrap().end();
        // Find the matching closing paren, respecting nesting
        if let Some(body) = find_paren_body(sql, start) {
            let columns = parse_columns(&body);
            let primary_keys = parse_primary_keys(&body);
            tables.push(SqlTable {
                name: table_name,
                columns,
                primary_keys,
            });
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
    let re_auto_increment = regex::Regex::new(r"(?i)\bAUTO_INCREMENT\b").unwrap();
    let re_default =
        regex::Regex::new(r"(?i)\bDEFAULT\s+('(?:[^']*(?:''[^']*)*)'|[^\s,]+)").unwrap();
    let re_comment = regex::Regex::new(r"(?i)\bCOMMENT\s+'((?:[^']*(?:''[^']*)*)*)'").unwrap();

    // Keywords that indicate a constraint, not a column definition
    let constraint_keywords = [
        "PRIMARY",
        "KEY",
        "UNIQUE",
        "INDEX",
        "CONSTRAINT",
        "CHECK",
        "FOREIGN",
        "FULLTEXT",
        "SPATIAL",
        "PARTITION",
    ];

    for part in &parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Skip constraint lines
        let first_word = trimmed
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_uppercase();
        let first_word_clean = first_word
            .trim_start_matches('`')
            .trim_start_matches('"')
            .trim_start_matches('[');
        if constraint_keywords.iter().any(|k| first_word_clean == *k) {
            continue;
        }
        if let Some(cap) = re_col.captures(trimmed) {
            let col_name = cap.get(1).unwrap().as_str().to_string();
            let col_type = cap.get(2).unwrap().as_str().trim().to_string();
            let nullable = !re_not_null.is_match(trimmed);
            let auto_increment = re_auto_increment.is_match(trimmed);
            let default_val = re_default
                .captures(trimmed)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim_matches('\'').to_string());
            let comment = re_comment
                .captures(trimmed)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().replace("''", "'"));
            columns.push(SqlColumn {
                name: col_name,
                sql_type: col_type,
                nullable,
                auto_increment,
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
                '\'' => {
                    in_quote = true;
                    current.push(ch);
                }
                '(' => {
                    depth += 1;
                    current.push(ch);
                }
                ')' => {
                    depth -= 1;
                    current.push(ch);
                }
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

pub(super) fn needs_table_field(column_name: &str, field_name: &str) -> bool {
    field_name != column_name && field_name != to_camel_case(column_name)
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
                java: "Boolean",
                typescript: "boolean",
                go: "bool",
                python: "bool",
                kotlin: "Boolean",
                csharp: "bool",
            };
        }
    }

    match base {
        "VARCHAR" | "CHAR" | "TEXT" | "TINYTEXT" | "MEDIUMTEXT" | "LONGTEXT" | "NVARCHAR"
        | "NCHAR" | "CLOB" => TypeMapping {
            java: "String",
            typescript: "string",
            go: "string",
            python: "str",
            kotlin: "String",
            csharp: "string",
        },
        "INT" | "INTEGER" | "SMALLINT" | "TINYINT" | "MEDIUMINT" => TypeMapping {
            java: "Integer",
            typescript: "number",
            go: "int32",
            python: "int",
            kotlin: "Int",
            csharp: "int",
        },
        "BIGINT" => TypeMapping {
            java: "Long",
            typescript: "number",
            go: "int64",
            python: "int",
            kotlin: "Long",
            csharp: "long",
        },
        "DATETIME" | "TIMESTAMP" => TypeMapping {
            java: "LocalDateTime",
            typescript: "Date",
            go: "time.Time",
            python: "datetime",
            kotlin: "LocalDateTime",
            csharp: "DateTime",
        },
        "DATE" => TypeMapping {
            java: "LocalDate",
            typescript: "string",
            go: "time.Time",
            python: "date",
            kotlin: "LocalDate",
            csharp: "DateTime",
        },
        "TIME" => TypeMapping {
            java: "LocalTime",
            typescript: "string",
            go: "string",
            python: "time",
            kotlin: "LocalTime",
            csharp: "TimeSpan",
        },
        "DECIMAL" | "NUMERIC" => TypeMapping {
            java: "BigDecimal",
            typescript: "number",
            go: "float64",
            python: "Decimal",
            kotlin: "BigDecimal",
            csharp: "decimal",
        },
        "BOOLEAN" | "BOOL" => TypeMapping {
            java: "Boolean",
            typescript: "boolean",
            go: "bool",
            python: "bool",
            kotlin: "Boolean",
            csharp: "bool",
        },
        "FLOAT" | "REAL" => TypeMapping {
            java: "Float",
            typescript: "number",
            go: "float32",
            python: "float",
            kotlin: "Float",
            csharp: "float",
        },
        "DOUBLE" | "DOUBLE PRECISION" => TypeMapping {
            java: "Double",
            typescript: "number",
            go: "float64",
            python: "float",
            kotlin: "Double",
            csharp: "double",
        },
        "BLOB" | "BINARY" | "VARBINARY" | "TINYBLOB" | "MEDIUMBLOB" | "LONGBLOB" | "BYTEA" => {
            TypeMapping {
                java: "byte[]",
                typescript: "Buffer",
                go: "[]byte",
                python: "bytes",
                kotlin: "ByteArray",
                csharp: "byte[]",
            }
        }
        "BIT" => TypeMapping {
            java: "Boolean",
            typescript: "boolean",
            go: "bool",
            python: "bool",
            kotlin: "Boolean",
            csharp: "bool",
        },
        _ => TypeMapping {
            java: "String",
            typescript: "string",
            go: "string",
            python: "str",
            kotlin: "String",
            csharp: "string",
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

#[derive(Debug, Clone, Default)]
struct JavaBaseOptions {
    excluded_fields: HashSet<String>,
    parent_qualified_name: Option<String>,
}

fn parse_java_base_options(options: &Value) -> Result<JavaBaseOptions, String> {
    let items = options["baseClasses"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if items.is_empty() {
        return Ok(JavaBaseOptions::default());
    }

    let parent_id = options["parentBaseClassId"]
        .as_i64()
        .ok_or("已选择基类时必须指定实际父类")?;
    let mut result = JavaBaseOptions::default();

    for item in items {
        let id = item["id"].as_i64().ok_or("基类 ID 无效")?;
        let qualified_name = crate::tools::sql_entity::validate_java_qualified_name(
            item["qualifiedName"].as_str().ok_or("基类完整类名无效")?,
        )?;
        for field in crate::tools::sql_entity::normalize_java_fields(&item["fields"])? {
            result.excluded_fields.insert(field);
        }
        if id == parent_id {
            result.parent_qualified_name = Some(qualified_name);
        }
    }

    if result.parent_qualified_name.is_none() {
        return Err("实际父类必须属于已选基类".into());
    }
    Ok(result)
}

fn generate_java(
    table: &SqlTable,
    naming: &str,
    comments: bool,
    mybatis_plus: bool,
    base_options: &JavaBaseOptions,
) -> String {
    let class_name = table_name_to_class(&table.name);
    let mut out = String::new();
    let included_columns: Vec<(&SqlColumn, String)> = table
        .columns
        .iter()
        .map(|column| (column, convert_field_name(&column.name, naming)))
        .filter(|(_, field_name)| !base_options.excluded_fields.contains(field_name))
        .collect();

    // Collect imports
    let mut imports = vec!["lombok.Data".to_string()];
    for (col, _) in &included_columns {
        let type_name = get_type_for_lang(&col.sql_type, "java");
        match type_name.as_str() {
            "LocalDateTime" => {
                imports.push("java.time.LocalDateTime".into());
            }
            "LocalDate" => {
                imports.push("java.time.LocalDate".into());
            }
            "LocalTime" => {
                imports.push("java.time.LocalTime".into());
            }
            "BigDecimal" => {
                imports.push("java.math.BigDecimal".into());
            }
            _ => {}
        }
    }
    if let Some(parent) = &base_options.parent_qualified_name {
        if parent.contains('.') {
            imports.push(parent.clone());
        }
    }

    let single_primary_key = if table.primary_keys.len() == 1 {
        table
            .primary_keys
            .first()
            .map(String::as_str)
            .filter(|key| {
                included_columns
                    .iter()
                    .any(|(column, _)| column.name == **key)
            })
    } else {
        None
    };

    if mybatis_plus {
        imports.push("com.baomidou.mybatisplus.annotation.TableName".into());

        if single_primary_key.is_some() {
            imports.push("com.baomidou.mybatisplus.annotation.TableId".into());
        }

        if included_columns.iter().any(|(col, field_name)| {
            single_primary_key != Some(col.name.as_str())
                && needs_table_field(&col.name, field_name)
        }) {
            imports.push("com.baomidou.mybatisplus.annotation.TableField".into());
        }

        if included_columns
            .iter()
            .any(|(col, _)| single_primary_key == Some(col.name.as_str()) && col.auto_increment)
        {
            imports.push("com.baomidou.mybatisplus.annotation.IdType".into());
        }
    }

    imports.sort();
    imports.dedup();
    for imp in &imports {
        out.push_str(&format!("import {};\n", imp));
    }
    out.push('\n');

    out.push_str("@Data\n");
    if mybatis_plus {
        out.push_str(&format!("@TableName(\"{}\")\n", table.name));
    }
    let parent_name = base_options
        .parent_qualified_name
        .as_deref()
        .and_then(|name| name.rsplit('.').next());
    let extends_clause = parent_name
        .map(|name| format!(" extends {name}"))
        .unwrap_or_default();
    out.push_str(&format!(
        "public class {}{} {{\n",
        class_name, extends_clause
    ));

    for (col, field_name) in &included_columns {
        if comments {
            if let Some(ref c) = col.comment {
                out.push_str(&format!("    /** {} */\n", c));
            }
        }
        let is_single_primary_key = single_primary_key == Some(col.name.as_str());

        if mybatis_plus && is_single_primary_key {
            let renamed = field_name.as_str() != col.name;
            match (renamed, col.auto_increment) {
                (false, false) => out.push_str("    @TableId\n"),
                (true, false) => {
                    out.push_str(&format!("    @TableId(\"{}\")\n", col.name));
                }
                (false, true) => {
                    out.push_str("    @TableId(type = IdType.AUTO)\n");
                }
                (true, true) => {
                    out.push_str(&format!(
                        "    @TableId(value = \"{}\", type = IdType.AUTO)\n",
                        col.name
                    ));
                }
            }
        } else if mybatis_plus && needs_table_field(&col.name, field_name) {
            out.push_str(&format!("    @TableField(\"{}\")\n", col.name));
        }

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
        out.push_str(&format!(
            "  {}{}: {};\n",
            field_name, nullable_mark, type_name
        ));
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
            col.comment
                .as_ref()
                .map(|c| format!(" // {}", c))
                .unwrap_or_default()
        } else {
            String::new()
        };
        out.push_str(&format!(
            "\t{} {} `json:\"{}\" db:\"{}\"`{}\n",
            field_name,
            go_type,
            db_name,
            col.name.to_ascii_lowercase(),
            comment_str
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
        out.push_str(&format!(
            "    {}: {}{}\n",
            field_name, full_type, default_str
        ));
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
        out.push_str(&format!(
            "    val {}: {}{}{}\n",
            field_name, type_name, nullable_mark, comma
        ));
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
        out.push_str(&format!(
            "    public {}{} {} {{ get; set; }}\n\n",
            type_name, nullable_mark, field_name
        ));
    }
    out.push_str("}\n");
    out
}

pub(super) fn sql_to_entity(payload: &Value) -> Result<Value, String> {
    let sql = payload["sql"].as_str().unwrap_or_default();
    if sql.trim().is_empty() {
        return Err("SQL is empty".into());
    }
    let language = payload["language"]
        .as_str()
        .unwrap_or("java")
        .to_lowercase();
    let options = &payload["options"];
    let comments = options["comments"].as_bool().unwrap_or(true);
    let naming = options["naming"].as_str().unwrap_or("camelCase");
    let mybatis_plus = options["mybatisPlus"].as_bool().unwrap_or(false);
    let java_base_options = if language == "java" {
        parse_java_base_options(options)?
    } else {
        JavaBaseOptions::default()
    };

    let tables = parse_create_tables(sql);
    if tables.is_empty() {
        return Err("No CREATE TABLE statements found".into());
    }

    let mut code_parts: Vec<String> = Vec::new();
    let mut table_infos: Vec<Value> = Vec::new();

    for table in &tables {
        let code = match language.as_str() {
            "java" => generate_java(table, naming, comments, mybatis_plus, &java_base_options),
            "typescript" => generate_typescript(table, naming, comments),
            "go" => generate_go(table, naming, comments),
            "python" => generate_python(table, naming, comments),
            "kotlin" => generate_kotlin(table, naming, comments),
            "csharp" => generate_csharp(table, naming, comments),
            _ => return Err(format!("Unsupported language: {}", language)),
        };
        code_parts.push(code);

        let col_infos: Vec<Value> = table
            .columns
            .iter()
            .map(|c| {
                json!({
                    "name": c.name,
                    "type": c.sql_type,
                    "nullable": c.nullable,
                    "default": c.default_val,
                    "comment": c.comment,
                })
            })
            .collect();

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

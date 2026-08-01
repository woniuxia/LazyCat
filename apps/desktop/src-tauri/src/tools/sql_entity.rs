use rusqlite::{params, Connection, ErrorCode, OptionalExtension};
use serde_json::{json, Value};
use std::collections::HashSet;

use super::helpers::db_conn;

pub const SQL_ENTITY_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS sql_entity_base_classes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    alias TEXT NOT NULL UNIQUE,
    qualified_name TEXT NOT NULL UNIQUE,
    fields_json TEXT NOT NULL DEFAULT '[]',
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_sql_entity_base_classes_sort
    ON sql_entity_base_classes(sort_order ASC, id ASC);
"#;

const ACTIONS: &[&str] = &[
    "base_class_list",
    "base_class_create",
    "base_class_update",
    "base_class_delete",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported sql_entity action: {action}"));
    }
    let conn = db_conn()?;
    match action {
        "base_class_list" => list_with_conn(&conn),
        "base_class_create" => create_with_conn(&conn, payload),
        "base_class_update" => update_with_conn(&conn, payload),
        "base_class_delete" => delete_with_conn(&conn, payload),
        _ => Err(format!("unsupported sql_entity action: {action}")),
    }
}

fn is_java_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(c) if c == '_' || c == '$' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c == '$' || c.is_ascii_alphanumeric())
}

pub(crate) fn validate_java_qualified_name(value: &str) -> Result<String, String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err("完整类名不能为空".into());
    }
    for part in normalized.split('.') {
        if !is_java_identifier(part) {
            return Err(format!("完整类名包含非法 Java 标识符：{part}"));
        }
    }
    Ok(normalized.to_string())
}

pub(crate) fn normalize_java_fields(payload: &Value) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for value in payload.as_array().ok_or("字段列表格式错误")? {
        let field = value.as_str().ok_or("字段列表格式错误")?.trim();
        if !is_java_identifier(field) {
            return Err(format!("非法 Java 标识符：{field}"));
        }
        if seen.insert(field.to_string()) {
            result.push(field.to_string());
        }
    }
    Ok(result)
}

fn validate_alias(payload: &Value) -> Result<String, String> {
    let alias = payload["alias"].as_str().unwrap_or_default().trim();
    if alias.is_empty() {
        return Err("别名不能为空".into());
    }
    Ok(alias.to_string())
}

fn parse_id(payload: &Value) -> Result<i64, String> {
    payload["id"].as_i64().ok_or_else(|| "基类 ID 无效".into())
}

fn item_with_conn(conn: &Connection, id: i64) -> Result<Value, String> {
    conn.query_row(
        "SELECT id, alias, qualified_name, fields_json, sort_order, created_at, updated_at
         FROM sql_entity_base_classes WHERE id = ?1",
        [id],
        |row| {
            let fields_json: String = row.get(3)?;
            let fields = serde_json::from_str::<Vec<String>>(&fields_json).unwrap_or_default();
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "alias": row.get::<_, String>(1)?,
                "qualifiedName": row.get::<_, String>(2)?,
                "fields": fields,
                "sortOrder": row.get::<_, i64>(4)?,
                "createdAt": row.get::<_, String>(5)?,
                "updatedAt": row.get::<_, String>(6)?,
            }))
        },
    )
    .optional()
    .map_err(|e| format!("读取基类失败: {e}"))?
    .ok_or_else(|| "基类不存在".into())
}

fn list_with_conn(conn: &Connection) -> Result<Value, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, alias, qualified_name, fields_json, sort_order, created_at, updated_at
             FROM sql_entity_base_classes ORDER BY sort_order ASC, id ASC",
        )
        .map_err(|e| format!("查询基类失败: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            let fields_json: String = row.get(3)?;
            let fields = serde_json::from_str::<Vec<String>>(&fields_json).unwrap_or_default();
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "alias": row.get::<_, String>(1)?,
                "qualifiedName": row.get::<_, String>(2)?,
                "fields": fields,
                "sortOrder": row.get::<_, i64>(4)?,
                "createdAt": row.get::<_, String>(5)?,
                "updatedAt": row.get::<_, String>(6)?,
            }))
        })
        .map_err(|e| format!("查询基类失败: {e}"))?;
    let items = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取基类失败: {e}"))?;
    Ok(json!({ "items": items }))
}

fn conflict_message(
    conn: &Connection,
    alias: &str,
    qualified_name: &str,
    id: Option<i64>,
) -> String {
    let alias_exists = match id {
        Some(id) => conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sql_entity_base_classes WHERE alias = ?1 AND id <> ?2)",
            params![alias, id],
            |row| row.get::<_, bool>(0),
        ),
        None => conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sql_entity_base_classes WHERE alias = ?1)",
            [alias],
            |row| row.get::<_, bool>(0),
        ),
    }
    .unwrap_or(false);
    if alias_exists {
        return "别名已存在".into();
    }
    let qualified_exists = match id {
        Some(id) => conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sql_entity_base_classes WHERE qualified_name = ?1 AND id <> ?2)",
            params![qualified_name, id],
            |row| row.get::<_, bool>(0),
        ),
        None => conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sql_entity_base_classes WHERE qualified_name = ?1)",
            [qualified_name],
            |row| row.get::<_, bool>(0),
        ),
    }
    .unwrap_or(false);
    if qualified_exists {
        "完整类名已存在".into()
    } else {
        "保存基类失败：唯一约束冲突".into()
    }
}

fn create_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let alias = validate_alias(payload)?;
    let qualified_name =
        validate_java_qualified_name(payload["qualifiedName"].as_str().unwrap_or_default())?;
    let fields = normalize_java_fields(&payload["fields"])?;
    let fields_json = serde_json::to_string(&fields).map_err(|e| format!("序列化字段失败: {e}"))?;
    conn.execute(
        "INSERT INTO sql_entity_base_classes(alias, qualified_name, fields_json, sort_order, updated_at)
         VALUES(?1, ?2, ?3, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM sql_entity_base_classes), CURRENT_TIMESTAMP)",
        params![alias, qualified_name, fields_json],
    )
    .map_err(|e| {
        if e.sqlite_error_code() == Some(ErrorCode::ConstraintViolation) {
            conflict_message(conn, &alias, &qualified_name, None)
        } else {
            format!("创建基类失败: {e}")
        }
    })?;
    let item = item_with_conn(conn, conn.last_insert_rowid())?;
    Ok(json!({ "item": item }))
}

fn update_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_id(payload)?;
    let alias = validate_alias(payload)?;
    let qualified_name =
        validate_java_qualified_name(payload["qualifiedName"].as_str().unwrap_or_default())?;
    let fields = normalize_java_fields(&payload["fields"])?;
    let fields_json = serde_json::to_string(&fields).map_err(|e| format!("序列化字段失败: {e}"))?;
    let affected = conn
        .execute(
            "UPDATE sql_entity_base_classes
             SET alias = ?1, qualified_name = ?2, fields_json = ?3, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?4",
            params![alias, qualified_name, fields_json, id],
        )
        .map_err(|e| {
            if e.sqlite_error_code() == Some(ErrorCode::ConstraintViolation) {
                conflict_message(conn, &alias, &qualified_name, Some(id))
            } else {
                format!("更新基类失败: {e}")
            }
        })?;
    if affected == 0 {
        return Err("基类不存在".into());
    }
    Ok(json!({ "item": item_with_conn(conn, id)? }))
}

fn delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_id(payload)?;
    let affected = conn
        .execute("DELETE FROM sql_entity_base_classes WHERE id = ?1", [id])
        .map_err(|e| format!("删除基类失败: {e}"))?;
    if affected == 0 {
        return Err("基类不存在".into());
    }
    Ok(json!({ "ok": true }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::json;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SQL_ENTITY_SCHEMA_SQL).unwrap();
        conn
    }

    #[test]
    fn base_class_crud_normalizes_fields_and_preserves_order() {
        let conn = test_conn();
        let created = create_with_conn(
            &conn,
            &json!({
                "alias": "审计基类",
                "qualifiedName": "com.example.AuditEntity",
                "fields": ["createdAt", "updatedAt", "createdAt"]
            }),
        )
        .unwrap();
        assert_eq!(created["item"]["fields"], json!(["createdAt", "updatedAt"]));

        let listed = list_with_conn(&conn).unwrap();
        assert_eq!(listed["items"].as_array().unwrap().len(), 1);

        let id = created["item"]["id"].as_i64().unwrap();
        update_with_conn(
            &conn,
            &json!({
                "id": id,
                "alias": "基础审计",
                "qualifiedName": "com.example.AuditEntity",
                "fields": ["createdAt"]
            }),
        )
        .unwrap();
        delete_with_conn(&conn, &json!({ "id": id })).unwrap();
        assert!(list_with_conn(&conn).unwrap()["items"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn rejects_duplicate_alias_and_invalid_java_names() {
        let conn = test_conn();
        create_with_conn(
            &conn,
            &json!({
                "alias": "基础",
                "qualifiedName": "com.example.BaseEntity",
                "fields": ["id"]
            }),
        )
        .unwrap();
        let duplicate = create_with_conn(
            &conn,
            &json!({
                "alias": "基础",
                "qualifiedName": "com.example.OtherEntity",
                "fields": []
            }),
        )
        .unwrap_err();
        assert!(duplicate.contains("别名已存在"));

        let invalid = create_with_conn(
            &conn,
            &json!({
                "alias": "非法",
                "qualifiedName": "com.example.1Base",
                "fields": ["created-at"]
            }),
        )
        .unwrap_err();
        assert!(invalid.contains("非法 Java 标识符"));
    }
}

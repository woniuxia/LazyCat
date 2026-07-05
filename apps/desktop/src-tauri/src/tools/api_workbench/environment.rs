use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::HashSet;

use super::helpers::{parse_i64, parse_name, validate_variable_name};
use super::types::KeyValueRow;

pub(crate) fn parse_variable_rows(payload: &Value) -> Result<Vec<KeyValueRow>, String> {
    let rows = payload["variables"].as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();
    let mut seen_names = HashSet::new();
    for item in rows {
        let name = item["name"].as_str().unwrap_or_default().trim();
        if !validate_variable_name(name) {
            return Err(format!("变量名无效: {name}"));
        }
        if !seen_names.insert(name.to_string()) {
            return Err(format!("变量名重复: {name}"));
        }
        out.push(KeyValueRow {
            enabled: true,
            key: name.to_string(),
            value: item["value"].as_str().unwrap_or_default().to_string(),
        });
    }
    Ok(out)
}

pub(crate) fn environment_save_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let name = parse_name(payload, "name")?;
    let id = payload["id"].as_i64();
    let env_id = if let Some(id) = id {
        let affected = conn
            .execute(
                "UPDATE api_workbench_environments
                 SET name=?1, updated_at=CURRENT_TIMESTAMP
                 WHERE id=?2 AND collection_id=?3",
                params![name, id, collection_id],
            )
            .map_err(|e| format!("update environment failed: {e}"))?;
        if affected == 0 {
            return Err("环境不存在".to_string());
        }
        id
    } else {
        let next_order: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1
                 FROM api_workbench_environments WHERE collection_id=?1",
                [collection_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO api_workbench_environments(collection_id, name, sort_order)
             VALUES(?1, ?2, ?3)",
            params![collection_id, name, next_order],
        )
        .map_err(|e| format!("create environment failed: {e}"))?;
        conn.last_insert_rowid()
    };

    let mut rows = parse_variable_rows(payload)?;
    if !rows.iter().any(|row| row.key == "BASE_URL") {
        rows.insert(
            0,
            KeyValueRow {
                enabled: true,
                key: "BASE_URL".into(),
                value: "".into(),
            },
        );
    }
    conn.execute(
        "DELETE FROM api_workbench_environment_variables WHERE environment_id=?1",
        [env_id],
    )
    .map_err(|e| format!("replace environment variables failed: {e}"))?;
    for (idx, row) in rows.iter().enumerate() {
        conn.execute(
            "INSERT INTO api_workbench_environment_variables(environment_id, name, value, is_secret, sort_order)
             VALUES(?1, ?2, ?3, 0, ?4)",
            params![env_id, row.key, row.value, idx as i64],
        )
        .map_err(|e| format!("save environment variable failed: {e}"))?;
    }
    Ok(json!({ "id": env_id, "collectionId": collection_id, "name": name }))
}

pub(crate) fn environment_delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    let collection_id: i64 = conn
        .query_row(
            "SELECT collection_id FROM api_workbench_environments WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .map_err(|_| "环境不存在".to_string())?;
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM api_workbench_environments WHERE collection_id=?1",
            [collection_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("count environments failed: {e}"))?;
    if count <= 1 {
        return Err("不能删除集合内最后一个环境".to_string());
    }
    conn.execute("DELETE FROM api_workbench_environments WHERE id=?1", [id])
        .map_err(|e| format!("delete environment failed: {e}"))?;
    let next_active: i64 = conn
        .query_row(
            "SELECT id FROM api_workbench_environments
             WHERE collection_id=?1 ORDER BY sort_order ASC, id ASC LIMIT 1",
            [collection_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("pick active environment failed: {e}"))?;
    conn.execute(
        "UPDATE api_workbench_collections
         SET active_environment_id=?1, updated_at=CURRENT_TIMESTAMP
         WHERE id=?2 AND (active_environment_id IS NULL OR active_environment_id=?3)",
        params![next_active, collection_id, id],
    )
    .map_err(|e| format!("switch active environment failed: {e}"))?;
    Ok(json!({ "ok": true, "activeEnvironmentId": next_active }))
}

pub(crate) fn global_variables_save_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let rows = parse_variable_rows(payload)?;
    if rows.iter().any(|row| row.key == "BASE_URL") {
        return Err("全局变量不能使用 BASE_URL".to_string());
    }
    conn.execute("DELETE FROM api_workbench_global_variables", [])
        .map_err(|e| format!("clear global variables failed: {e}"))?;
    for (idx, row) in rows.iter().enumerate() {
        conn.execute(
            "INSERT INTO api_workbench_global_variables(name, value, is_secret, sort_order)
             VALUES(?1, ?2, 0, ?3)",
            params![row.key, row.value, idx as i64],
        )
        .map_err(|e| format!("save global variable failed: {e}"))?;
    }
    Ok(json!({ "ok": true }))
}

pub(crate) fn environment_list_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let mut stmt = conn
        .prepare(
            "SELECT id, collection_id, name, sort_order, created_at, updated_at
             FROM api_workbench_environments
             WHERE collection_id=?1 ORDER BY sort_order ASC, id ASC",
        )
        .map_err(|e| format!("prepare environment list failed: {e}"))?;
    let items = stmt
        .query_map([collection_id], |row| {
            let env_id = row.get::<_, i64>(0)?;
            let mut var_stmt = conn.prepare(
                "SELECT name, value, is_secret, sort_order
                 FROM api_workbench_environment_variables
                 WHERE environment_id=?1 ORDER BY sort_order ASC, id ASC",
            )?;
            let variables = var_stmt
                .query_map([env_id], |var_row| {
                    Ok(json!({
                        "name": var_row.get::<_, String>(0)?,
                        "value": var_row.get::<_, String>(1)?,
                        "isSecret": var_row.get::<_, i64>(2)? != 0,
                        "sortOrder": var_row.get::<_, i64>(3)?
                    }))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(json!({
                "id": env_id,
                "collectionId": row.get::<_, i64>(1)?,
                "name": row.get::<_, String>(2)?,
                "sortOrder": row.get::<_, i64>(3)?,
                "createdAt": row.get::<_, String>(4)?,
                "updatedAt": row.get::<_, String>(5)?,
                "variables": variables
            }))
        })
        .map_err(|e| format!("list environments failed: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read environments failed: {e}"))?;
    Ok(json!({ "items": items }))
}

pub(crate) fn global_variables_list_with_conn(conn: &Connection) -> Result<Value, String> {
    let mut stmt = conn
        .prepare(
            "SELECT name, value, is_secret, sort_order
             FROM api_workbench_global_variables ORDER BY sort_order ASC, name ASC",
        )
        .map_err(|e| format!("prepare global variables failed: {e}"))?;
    let items = stmt
        .query_map([], |row| {
            Ok(json!({
                "name": row.get::<_, String>(0)?,
                "value": row.get::<_, String>(1)?,
                "isSecret": row.get::<_, i64>(2)? != 0,
                "sortOrder": row.get::<_, i64>(3)?
            }))
        })
        .map_err(|e| format!("list global variables failed: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("read global variables failed: {e}"))?;
    Ok(json!({ "items": items }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::api_workbench::test_conn;
    use crate::tools::api_workbench::*;

    #[test]
    fn environment_delete_switches_active_environment_and_rejects_last_one() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let first_env_id = c["activeEnvironmentId"].as_i64().unwrap();
        let second = environment_save_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "name": "Test", "variables": [] }),
        )
        .expect("second env");
        let second_env_id = second["id"].as_i64().unwrap();

        environment_delete_with_conn(&conn, &json!({ "id": first_env_id })).expect("delete first");
        let active: i64 = conn
            .query_row(
                "SELECT active_environment_id FROM api_workbench_collections WHERE id=?1",
                [collection_id],
                |row| row.get(0),
            )
            .expect("active");
        assert_eq!(active, second_env_id);

        let err = environment_delete_with_conn(&conn, &json!({ "id": second_env_id }))
            .expect_err("reject last");
        assert!(err.contains("最后一个环境"));
    }

    #[test]
    fn environment_save_rejects_duplicate_variable_names() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let environment_id = c["activeEnvironmentId"].as_i64().unwrap();

        let err = environment_save_with_conn(
            &conn,
            &json!({
                "id": environment_id,
                "collectionId": collection_id,
                "name": "开发",
                "variables": [
                    { "name": "TOKEN", "value": "a", "isSecret": false },
                    { "name": " TOKEN ", "value": "b", "isSecret": false }
                ]
            }),
        )
        .expect_err("duplicate variable");

        assert!(err.contains("变量名重复: TOKEN"));
        assert!(!err.contains("UNIQUE constraint"));
    }

    #[test]
    fn global_variables_reject_base_url() {
        let conn = test_conn();
        let err = global_variables_save_with_conn(
            &conn,
            &json!({ "variables": [{ "name": "BASE_URL", "value": "http://x", "isSecret": false }] }),
        )
        .expect_err("reject");
        assert!(err.contains("BASE_URL"));
    }

}

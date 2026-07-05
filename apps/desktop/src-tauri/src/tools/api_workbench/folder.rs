use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::HashSet;

use super::helpers::{parse_i64, parse_name, parse_ordered_ids};

pub(crate) fn folder_create_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let name = parse_name(payload, "name")?;
    let parent_id = payload["parentId"].as_i64();
    let next_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1
             FROM api_workbench_folders
             WHERE collection_id=?1 AND parent_id IS ?2",
            params![collection_id, parent_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO api_workbench_folders(collection_id, parent_id, name, sort_order)
         VALUES(?1, ?2, ?3, ?4)",
        params![collection_id, parent_id, name, next_order],
    )
    .map_err(|e| format!("create folder failed: {e}"))?;
    Ok(json!({
        "id": conn.last_insert_rowid(),
        "collectionId": collection_id,
        "parentId": parent_id,
        "name": name
    }))
}

pub(crate) fn folder_update_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    let name = parse_name(payload, "name")?;
    let affected = conn
        .execute(
            "UPDATE api_workbench_folders SET name=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![name, id],
        )
        .map_err(|e| format!("update folder failed: {e}"))?;
    if affected == 0 {
        return Err("文件夹不存在".to_string());
    }
    Ok(json!({ "ok": true }))
}

pub(crate) fn folder_delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM api_workbench_folders WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .map_err(|e| format!("check folder failed: {e}"))?;
    if exists == 0 {
        return Err("文件夹不存在".to_string());
    }

    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("delete folder begin: {e}"))?;
    tx.execute(
        "WITH RECURSIVE descendants(id) AS (
            SELECT id FROM api_workbench_folders WHERE id=?1
            UNION ALL
            SELECT f.id FROM api_workbench_folders f
            JOIN descendants d ON f.parent_id=d.id
        )
        UPDATE api_workbench_requests
        SET folder_id=NULL, updated_at=CURRENT_TIMESTAMP
        WHERE folder_id IN (SELECT id FROM descendants)",
        [id],
    )
    .map_err(|e| format!("unassign folder requests failed: {e}"))?;
    tx.execute("DELETE FROM api_workbench_folders WHERE id=?1", [id])
        .map_err(|e| format!("delete folder failed: {e}"))?;
    tx.commit()
        .map_err(|e| format!("delete folder commit: {e}"))?;
    Ok(json!({ "ok": true }))
}

pub(crate) fn next_folder_sort_order(
    conn: &Connection,
    collection_id: i64,
    parent_id: Option<i64>,
) -> Result<i64, String> {
    conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1
         FROM api_workbench_folders
         WHERE collection_id=?1 AND parent_id IS ?2",
        params![collection_id, parent_id],
        |row| row.get(0),
    )
    .map_err(|e| format!("query next folder order failed: {e}"))
}

pub(crate) fn folder_is_descendant(
    conn: &Connection,
    folder_id: i64,
    possible_descendant_id: i64,
) -> Result<bool, String> {
    let count: i64 = conn
        .query_row(
            "WITH RECURSIVE descendants(id) AS (
                SELECT id FROM api_workbench_folders WHERE parent_id=?1
                UNION ALL
                SELECT f.id FROM api_workbench_folders f
                JOIN descendants d ON f.parent_id=d.id
            )
            SELECT COUNT(*) FROM descendants WHERE id=?2",
            params![folder_id, possible_descendant_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("check descendants failed: {e}"))?;
    Ok(count > 0)
}

pub(crate) fn folder_move_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    let target_parent_id = payload["targetParentId"].as_i64();
    if target_parent_id == Some(id) {
        return Err("不能移动到自己".to_string());
    }

    let collection_id: i64 = conn
        .query_row(
            "SELECT collection_id FROM api_workbench_folders WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .map_err(|_| "文件夹不存在".to_string())?;
    if let Some(parent_id) = target_parent_id {
        let owner: i64 = conn
            .query_row(
                "SELECT collection_id FROM api_workbench_folders WHERE id=?1",
                [parent_id],
                |row| row.get(0),
            )
            .map_err(|_| "目标文件夹不存在".to_string())?;
        if owner != collection_id {
            return Err("目标文件夹不属于当前集合".to_string());
        }
        if folder_is_descendant(conn, id, parent_id)? {
            return Err("不能移动到自己的子文件夹".to_string());
        }
    }

    let next_order = next_folder_sort_order(conn, collection_id, target_parent_id)?;
    conn.execute(
        "UPDATE api_workbench_folders
         SET parent_id=?1, sort_order=?2, updated_at=CURRENT_TIMESTAMP
         WHERE id=?3",
        params![target_parent_id, next_order, id],
    )
    .map_err(|e| format!("move folder failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

pub(crate) fn folder_reorder_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let parent_id = payload["parentId"].as_i64();
    let ordered_ids = parse_ordered_ids(payload)?;
    let existing: Vec<i64> = {
        let mut stmt = conn
            .prepare(
                "SELECT id FROM api_workbench_folders
                 WHERE collection_id=?1 AND parent_id IS ?2
                 ORDER BY sort_order ASC, id ASC",
            )
            .map_err(|e| format!("prepare folder reorder failed: {e}"))?;
        let rows = stmt
            .query_map(params![collection_id, parent_id], |row| row.get(0))
            .map_err(|e| format!("query folder reorder failed: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("collect folder reorder failed: {e}"))?;
        rows
    };
    let expected: HashSet<i64> = existing.iter().copied().collect();
    let actual: HashSet<i64> = ordered_ids.iter().copied().collect();
    if expected != actual || existing.len() != ordered_ids.len() {
        return Err("排序列表不完整".to_string());
    }

    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("folder reorder begin: {e}"))?;
    for (idx, id) in ordered_ids.iter().enumerate() {
        tx.execute(
            "UPDATE api_workbench_folders SET sort_order=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![idx as i64, id],
        )
        .map_err(|e| format!("update folder order failed: {e}"))?;
    }
    tx.commit()
        .map_err(|e| format!("folder reorder commit: {e}"))?;
    Ok(json!({ "ok": true }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::api_workbench::test_conn;
    use crate::tools::api_workbench::*;

    #[test]
    fn folder_delete_preserves_descendant_requests_as_unassigned() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let parent = folder_create_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "name": "Parent" }),
        )
        .expect("parent");
        let parent_id = parent["id"].as_i64().unwrap();
        let child = folder_create_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "parentId": parent_id, "name": "Child" }),
        )
        .expect("child");
        let child_id = child["id"].as_i64().unwrap();

        let saved = request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": child_id,
                "name": "Child request",
                "draft": {
                    "method": "GET",
                    "url": "/x",
                    "query": [],
                    "headers": [],
                    "bodyType": "none",
                    "body": "",
                    "form": [],
                    "timeoutMs": 10000
                }
            }),
        )
        .expect("request");
        let request_id = saved["id"].as_i64().unwrap();

        folder_delete_with_conn(&conn, &json!({ "id": parent_id })).expect("delete");

        let folder_id: Option<i64> = conn
            .query_row(
                "SELECT folder_id FROM api_workbench_requests WHERE id=?1",
                [request_id],
                |row| row.get(0),
            )
            .expect("request remains");
        assert_eq!(folder_id, None);
    }

    #[test]
    fn folder_delete_reports_missing_folder() {
        let conn = test_conn();
        let err = folder_delete_with_conn(&conn, &json!({ "id": 999 })).expect_err("missing");
        assert!(err.contains("文件夹不存在"));
    }

    #[test]
    fn folder_move_rejects_self_and_descendant_targets() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let parent = folder_create_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "name": "Parent" }),
        )
        .expect("parent");
        let parent_id = parent["id"].as_i64().unwrap();
        let child = folder_create_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "parentId": parent_id, "name": "Child" }),
        )
        .expect("child");
        let child_id = child["id"].as_i64().unwrap();

        let err = folder_move_with_conn(
            &conn,
            &json!({ "id": parent_id, "targetParentId": parent_id }),
        )
        .expect_err("self");
        assert!(err.contains("自己"));

        let err = folder_move_with_conn(
            &conn,
            &json!({ "id": parent_id, "targetParentId": child_id }),
        )
        .expect_err("descendant");
        assert!(err.contains("子文件夹"));
    }

    #[test]
    fn folder_reorder_requires_complete_sibling_ids() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let a = folder_create_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "name": "A" }),
        )
        .expect("a");
        let b = folder_create_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "name": "B" }),
        )
        .expect("b");
        let a_id = a["id"].as_i64().unwrap();
        let b_id = b["id"].as_i64().unwrap();

        let err = folder_reorder_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "parentId": null, "orderedIds": [b_id] }),
        )
        .expect_err("incomplete");
        assert!(err.contains("不完整"));

        folder_reorder_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "parentId": null, "orderedIds": [b_id, a_id] }),
        )
        .expect("reorder");
        let names: Vec<String> = {
            let mut stmt = conn
                .prepare(
                    "SELECT name FROM api_workbench_folders WHERE collection_id=?1 AND parent_id IS NULL ORDER BY sort_order ASC",
                )
                .unwrap();
            stmt.query_map([collection_id], |row| row.get(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(names, vec!["B", "A"]);
    }

}

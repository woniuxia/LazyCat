use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::HashSet;

use super::helpers::{
    clamp_timeout_ms, parse_i64, parse_name, parse_ordered_ids, MAX_RESPONSE_BODY_BYTES,
};
use super::types::RequestDraft;

pub(crate) fn next_request_sort_order(
    conn: &Connection,
    collection_id: i64,
    folder_id: Option<i64>,
) -> Result<i64, String> {
    conn.query_row(
        "SELECT COALESCE(MAX(sort_order), -1) + 1
         FROM api_workbench_requests
         WHERE collection_id=?1 AND folder_id IS ?2",
        params![collection_id, folder_id],
        |row| row.get(0),
    )
    .map_err(|e| format!("query next request order failed: {e}"))
}

pub(crate) fn parse_draft(payload: &Value) -> Result<RequestDraft, String> {
    serde_json::from_value(payload["draft"].clone()).map_err(|e| format!("请求草稿格式错误: {e}"))
}

pub(crate) fn request_save_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let folder_id = payload["folderId"].as_i64();
    let name = parse_name(payload, "name")?;
    let description = payload["description"].as_str().unwrap_or_default().trim();
    let draft = parse_draft(payload)?;
    let query_json = serde_json::to_string(&draft.query).map_err(|e| e.to_string())?;
    let headers_json = serde_json::to_string(&draft.headers).map_err(|e| e.to_string())?;
    let form_json = serde_json::to_string(&draft.form).map_err(|e| e.to_string())?;
    let id = payload["id"].as_i64();
    if let Some(id) = id {
        let affected = conn
            .execute(
                "UPDATE api_workbench_requests
                 SET folder_id=?1, name=?2, description=?3, method=?4, url=?5,
                     query_json=?6, headers_json=?7, body_type=?8, body_text=?9,
                     form_json=?10, timeout_ms=?11, follow_redirects=?12, updated_at=CURRENT_TIMESTAMP
                 WHERE id=?13 AND collection_id=?14",
                params![
                    folder_id,
                    name,
                    description,
                    draft.method,
                    draft.url,
                    query_json,
                    headers_json,
                    draft.body_type,
                    draft.body,
                    form_json,
                    clamp_timeout_ms(draft.timeout_ms) as i64,
                    if draft.follow_redirects { 1 } else { 0 },
                    id,
                    collection_id
                ],
            )
            .map_err(|e| format!("update request failed: {e}"))?;
        if affected == 0 {
            return Err("接口不存在".to_string());
        }
        Ok(json!({ "id": id, "ok": true }))
    } else {
        let next_order: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(sort_order), -1) + 1
                 FROM api_workbench_requests
                 WHERE collection_id=?1 AND folder_id IS ?2",
                params![collection_id, folder_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO api_workbench_requests(
                collection_id, folder_id, name, description, method, url,
                query_json, headers_json, body_type, body_text, form_json,
                timeout_ms, follow_redirects, sort_order
             )
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                collection_id,
                folder_id,
                name,
                description,
                draft.method,
                draft.url,
                query_json,
                headers_json,
                draft.body_type,
                draft.body,
                form_json,
                clamp_timeout_ms(draft.timeout_ms) as i64,
                if draft.follow_redirects { 1 } else { 0 },
                next_order
            ],
        )
        .map_err(|e| format!("create request failed: {e}"))?;
        Ok(json!({ "id": conn.last_insert_rowid(), "ok": true }))
    }
}

pub(crate) fn request_get_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    conn.query_row(
        "SELECT id, collection_id, folder_id, name, description, method, url,
                query_json, headers_json, body_type, body_text, form_json, timeout_ms,
                example_response_json, sort_order, created_at, updated_at, follow_redirects
         FROM api_workbench_requests WHERE id=?1",
        [id],
        |row| {
            let query_json: String = row.get(7)?;
            let headers_json: String = row.get(8)?;
            let form_json: String = row.get(11)?;
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "collectionId": row.get::<_, i64>(1)?,
                "folderId": row.get::<_, Option<i64>>(2)?,
                "name": row.get::<_, String>(3)?,
                "description": row.get::<_, String>(4)?,
                "draft": {
                    "method": row.get::<_, String>(5)?,
                    "url": row.get::<_, String>(6)?,
                    "query": serde_json::from_str::<Value>(&query_json).unwrap_or_else(|_| json!([])),
                    "headers": serde_json::from_str::<Value>(&headers_json).unwrap_or_else(|_| json!([])),
                    "bodyType": row.get::<_, String>(9)?,
                    "body": row.get::<_, String>(10)?,
                    "form": serde_json::from_str::<Value>(&form_json).unwrap_or_else(|_| json!([])),
                    "timeoutMs": row.get::<_, i64>(12)?,
                    "followRedirects": row.get::<_, i64>(17)? != 0
                },
                "exampleResponse": row.get::<_, Option<String>>(13)?,
                "sortOrder": row.get::<_, i64>(14)?,
                "createdAt": row.get::<_, String>(15)?,
                "updatedAt": row.get::<_, String>(16)?
            }))
        },
    )
    .map_err(|_| "接口不存在".to_string())
}

pub(crate) fn request_delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    conn.execute("DELETE FROM api_workbench_requests WHERE id=?1", [id])
        .map_err(|e| format!("delete request failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

pub(crate) fn request_move_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    let target_folder_id = payload["targetFolderId"].as_i64();
    let collection_id: i64 = conn
        .query_row(
            "SELECT collection_id FROM api_workbench_requests WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .map_err(|_| "接口不存在".to_string())?;
    if let Some(folder_id) = target_folder_id {
        let owner: i64 = conn
            .query_row(
                "SELECT collection_id FROM api_workbench_folders WHERE id=?1",
                [folder_id],
                |row| row.get(0),
            )
            .map_err(|_| "目标文件夹不存在".to_string())?;
        if owner != collection_id {
            return Err("目标文件夹不属于当前集合".to_string());
        }
    }

    let next_order = next_request_sort_order(conn, collection_id, target_folder_id)?;
    conn.execute(
        "UPDATE api_workbench_requests
         SET folder_id=?1, sort_order=?2, updated_at=CURRENT_TIMESTAMP
         WHERE id=?3",
        params![target_folder_id, next_order, id],
    )
    .map_err(|e| format!("move request failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

pub(crate) fn request_reorder_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let folder_id = payload["folderId"].as_i64();
    let ordered_ids = parse_ordered_ids(payload)?;
    let existing: Vec<i64> = {
        let mut stmt = conn
            .prepare(
                "SELECT id FROM api_workbench_requests
                 WHERE collection_id=?1 AND folder_id IS ?2
                 ORDER BY sort_order ASC, id ASC",
            )
            .map_err(|e| format!("prepare request reorder failed: {e}"))?;
        let rows = stmt
            .query_map(params![collection_id, folder_id], |row| row.get(0))
            .map_err(|e| format!("query request reorder failed: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("collect request reorder failed: {e}"))?;
        rows
    };
    let expected: HashSet<i64> = existing.iter().copied().collect();
    let actual: HashSet<i64> = ordered_ids.iter().copied().collect();
    if expected != actual || existing.len() != ordered_ids.len() {
        return Err("排序列表不完整".to_string());
    }

    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("request reorder begin: {e}"))?;
    for (idx, id) in ordered_ids.iter().enumerate() {
        tx.execute(
            "UPDATE api_workbench_requests SET sort_order=?1, updated_at=CURRENT_TIMESTAMP WHERE id=?2",
            params![idx as i64, id],
        )
        .map_err(|e| format!("update request order failed: {e}"))?;
    }
    tx.commit()
        .map_err(|e| format!("request reorder commit: {e}"))?;
    Ok(json!({ "ok": true }))
}

pub(crate) fn request_save_example_response_with_conn(
    conn: &Connection,
    payload: &Value,
) -> Result<Value, String> {
    let request_id = parse_i64(payload, "requestId")?;
    let collection_id = parse_i64(payload, "collectionId")?;
    let owner: i64 = conn
        .query_row(
            "SELECT collection_id FROM api_workbench_requests WHERE id=?1",
            [request_id],
            |row| row.get(0),
        )
        .map_err(|_| "接口不存在".to_string())?;
    if owner != collection_id {
        return Err("接口不属于当前集合".to_string());
    }
    let mut response = payload
        .get("response")
        .cloned()
        .ok_or_else(|| "response is required".to_string())?;
    sanitize_example_response(&mut response);
    let serialized =
        serde_json::to_string(&response).map_err(|e| format!("示例响应格式错误: {e}"))?;
    if serialized.len() > MAX_RESPONSE_BODY_BYTES {
        return Err("示例响应体积超过限制".to_string());
    }
    conn.execute(
        "UPDATE api_workbench_requests
         SET example_response_json=?1, updated_at=CURRENT_TIMESTAMP
         WHERE id=?2 AND collection_id=?3",
        params![serialized, request_id, collection_id],
    )
    .map_err(|e| format!("save example response failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

pub(crate) fn sanitize_example_response(response: &mut Value) {
    let Some(obj) = response.as_object_mut() else {
        return;
    };
    let storage = obj
        .get("bodyStorage")
        .and_then(|value| value.as_str())
        .unwrap_or("text")
        .to_string();
    if matches!(storage.as_str(), "file" | "truncated-binary") {
        obj.remove("bodyFilePath");
        obj.remove("bodyHash");
        let file_name = obj
            .get("bodyFileName")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let extension = obj
            .get("bodyExtension")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let body_size = obj
            .get("bodySize")
            .and_then(|value| value.as_i64())
            .unwrap_or(0);
        let summary = if storage == "truncated-binary" {
            format!("二进制响应已截断，仅保存元信息摘要（{body_size} bytes）。")
        } else if file_name.is_empty() {
            format!("二进制响应，仅保存元信息摘要（{body_size} bytes，{extension}）。")
        } else {
            format!("二进制响应，仅保存元信息摘要（{file_name}，{body_size} bytes）。")
        };
        obj.insert("bodyText".to_string(), Value::String(summary));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::api_workbench::test_conn;
    use crate::tools::api_workbench::*;

    #[test]
    fn request_save_example_response_updates_request_and_markdown() {
        let conn = test_conn();
        let collection =
            collection_create_with_conn(&conn, &json!({ "name": "Demo", "description": "" }))
                .expect("collection");
        let collection_id = collection["id"].as_i64().unwrap();
        let saved = request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": null,
                "name": "Health",
                "description": "",
                "draft": {
                    "method": "GET",
                    "url": "/health",
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

        request_save_example_response_with_conn(
            &conn,
            &json!({
                "requestId": request_id,
                "collectionId": collection_id,
                "response": {
                    "status": 200,
                    "statusText": "OK",
                    "contentType": "application/json",
                    "headers": [{ "enabled": true, "key": "Content-Type", "value": "application/json" }],
                    "bodyText": "{\"ok\":true}",
                    "bodySize": 11,
                    "bodyTruncated": false,
                    "savedAt": "2026-06-30T10:00:00+08:00"
                }
            }),
        )
        .expect("example");

        let detail = request_get_with_conn(&conn, &json!({ "id": request_id })).expect("detail");
        assert!(detail["exampleResponse"]
            .as_str()
            .unwrap()
            .contains("\"status\":200"));
        let markdown = export_markdown_with_conn(&conn, &json!({ "collectionId": collection_id }))
            .expect("markdown");
        let markdown = markdown["markdown"].as_str().unwrap();
        assert!(markdown.contains("#### 示例响应"));
        assert!(markdown.contains("`200 OK`"));
        assert!(markdown.contains("{\"ok\":true}"));
    }

    #[test]
    fn request_save_example_response_omits_binary_cache_path() {
        let conn = test_conn();
        let collection =
            collection_create_with_conn(&conn, &json!({ "name": "Demo", "description": "" }))
                .expect("collection");
        let collection_id = collection["id"].as_i64().unwrap();
        let saved = request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": null,
                "name": "Download",
                "description": "",
                "draft": {
                    "method": "GET",
                    "url": "/download",
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

        request_save_example_response_with_conn(
            &conn,
            &json!({
                "requestId": request_id,
                "collectionId": collection_id,
                "response": {
                    "status": 200,
                    "statusText": "OK",
                    "contentType": "application/pdf",
                    "headers": [],
                    "bodyText": "",
                    "bodySize": 128,
                    "bodyTruncated": false,
                    "bodyStorage": "file",
                    "bodyFilePath": "C:/should/not/be/saved.pdf",
                    "bodyFileName": "report.pdf",
                    "bodyExtension": "pdf",
                    "bodyHash": "abc",
                    "savedAt": "2026-07-01T00:00:00.000Z"
                }
            }),
        )
        .expect("example");

        let raw: String = conn
            .query_row(
                "SELECT example_response_json FROM api_workbench_requests WHERE id=?1",
                [request_id],
                |row| row.get(0),
            )
            .expect("example");
        let example: Value = serde_json::from_str(&raw).expect("example json");
        assert_eq!(example["bodyStorage"], "file");
        assert!(example.get("bodyFilePath").is_none());
        assert!(example.get("bodyHash").is_none());
        assert!(example["bodyText"].as_str().unwrap().contains("二进制响应"));
    }

    #[test]
    fn request_save_and_get_round_trips_draft_json() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let saved = request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": null,
                "name": "List users",
                "description": "Fetch users",
                "draft": {
                    "method": "GET",
                    "url": "/api/users",
                    "query": [{ "enabled": true, "key": "page", "value": "1" }],
                    "headers": [],
                    "bodyType": "none",
                    "body": "",
                    "form": [],
                    "timeoutMs": 10000
                }
            }),
        )
        .expect("save");
        let request_id = saved["id"].as_i64().unwrap();
        let detail = request_get_with_conn(&conn, &json!({ "id": request_id })).expect("get");
        assert_eq!(detail["name"], "List users");
        assert_eq!(detail["draft"]["url"], "/api/users");
        assert_eq!(detail["draft"]["query"][0]["key"], "page");
    }

    #[test]
    fn request_move_moves_between_folder_and_unassigned() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let folder = folder_create_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "name": "Users" }),
        )
        .expect("folder");
        let folder_id = folder["id"].as_i64().unwrap();
        let saved = request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": null,
                "name": "Health",
                "draft": {
                    "method": "GET",
                    "url": "/health",
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

        request_move_with_conn(
            &conn,
            &json!({ "id": request_id, "targetFolderId": folder_id }),
        )
        .expect("move to folder");
        let in_folder: Option<i64> = conn
            .query_row(
                "SELECT folder_id FROM api_workbench_requests WHERE id=?1",
                [request_id],
                |row| row.get(0),
            )
            .expect("folder id");
        assert_eq!(in_folder, Some(folder_id));

        request_move_with_conn(&conn, &json!({ "id": request_id, "targetFolderId": null }))
            .expect("move to unassigned");
        let unassigned: Option<i64> = conn
            .query_row(
                "SELECT folder_id FROM api_workbench_requests WHERE id=?1",
                [request_id],
                |row| row.get(0),
            )
            .expect("folder id");
        assert_eq!(unassigned, None);
    }

    #[test]
    fn request_reorder_rejects_duplicate_ids() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let first = request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": null,
                "name": "First",
                "draft": { "method": "GET", "url": "/1", "query": [], "headers": [], "bodyType": "none", "body": "", "form": [], "timeoutMs": 10000 }
            }),
        )
        .expect("first");
        let second = request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": null,
                "name": "Second",
                "draft": { "method": "GET", "url": "/2", "query": [], "headers": [], "bodyType": "none", "body": "", "form": [], "timeoutMs": 10000 }
            }),
        )
        .expect("second");
        let first_id = first["id"].as_i64().unwrap();
        let second_id = second["id"].as_i64().unwrap();

        let err = request_reorder_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "folderId": null, "orderedIds": [first_id, first_id] }),
        )
        .expect_err("duplicate");
        assert!(err.contains("重复"));

        request_reorder_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "folderId": null, "orderedIds": [second_id, first_id] }),
        )
        .expect("reorder");
    }

}

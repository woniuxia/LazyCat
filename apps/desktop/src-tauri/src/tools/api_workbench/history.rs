use rusqlite::{params, Connection};
use serde_json::{json, Value};

use super::executor::{
    execute_api_workbench_request, insert_history_with_conn,
    HistoryInsert,
};
use super::helpers::{
    clamp_timeout_ms, parse_i64, parse_name, MAX_HISTORY_NOTE_CHARS,
    MAX_HISTORY_ROWS,
};
use super::response::{cleanup_unreferenced_history_cache_files, collect_history_cache_refs};
use super::types::{ExecutedRequestSnapshot, RequestDraft};

pub(crate) fn history_save_request_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let history_id = parse_i64(payload, "historyId")?;
    let collection_id = parse_i64(payload, "collectionId")?;
    let folder_id = payload["folderId"].as_i64();
    let name = parse_name(payload, "name")?;

    conn.query_row(
        "SELECT id FROM api_workbench_collections WHERE id=?1",
        [collection_id],
        |row| row.get::<_, i64>(0),
    )
    .map_err(|_| "集合不存在".to_string())?;
    if let Some(folder_id) = folder_id {
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

    let history = conn
        .query_row(
            "SELECT method, url, final_url, status, duration_ms, created_at, request_snapshot_json
             FROM api_workbench_history WHERE id=?1",
            [history_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .map_err(|_| "历史记录不存在".to_string())?;
    let draft = if let Some(raw) = history.6.as_deref() {
        serde_json::from_str::<RequestDraft>(raw).map_err(|_| "历史快照已损坏".to_string())?
    } else {
        RequestDraft {
            method: history.0.clone(),
            url: history.1.clone(),
            query: Vec::new(),
            headers: Vec::new(),
            body_type: "none".to_string(),
            body: String::new(),
            form: Vec::new(),
            timeout_ms: 10000,
        }
    };
    let query_json = serde_json::to_string(&draft.query).map_err(|e| e.to_string())?;
    let headers_json = serde_json::to_string(&draft.headers).map_err(|e| e.to_string())?;
    let form_json = serde_json::to_string(&draft.form).map_err(|e| e.to_string())?;

    let description = format!(
        "来源历史记录：状态 {}，耗时 {}ms，最终 URL：{}，创建时间：{}",
        history
            .3
            .map(|status| status.to_string())
            .unwrap_or_else(|| "ERR".to_string()),
        history.4,
        history.2,
        history.5
    );
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
            timeout_ms, sort_order
         )
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
            next_order
        ],
    )
    .map_err(|e| format!("save history as request failed: {e}"))?;
    Ok(json!({ "id": conn.last_insert_rowid() }))
}

pub(crate) fn history_row_json(
    row: &rusqlite::Row<'_>,
    include_request_snapshot: bool,
) -> rusqlite::Result<Value> {
    let request_snapshot_json: Option<String> = row.get(23)?;
    let executed_snapshot_json: Option<String> = row.get(24)?;
    let mut value = json!({
        "id": row.get::<_, i64>(0)?,
        "collectionId": row.get::<_, Option<i64>>(1)?,
        "environmentId": row.get::<_, Option<i64>>(2)?,
        "requestId": row.get::<_, Option<i64>>(3)?,
        "replayedFromHistoryId": row.get::<_, Option<i64>>(25)?,
        "name": row.get::<_, String>(4)?,
        "note": row.get::<_, String>(26)?,
        "pinned": row.get::<_, i64>(27)? == 1,
        "method": row.get::<_, String>(5)?,
        "url": row.get::<_, String>(6)?,
        "finalUrl": row.get::<_, String>(7)?,
        "status": row.get::<_, Option<i64>>(8)?,
        "durationMs": row.get::<_, i64>(9)?,
        "ok": row.get::<_, i64>(10)? == 1,
        "error": row.get::<_, Option<String>>(11)?,
        "contentType": row.get::<_, String>(12)?,
        "bodySize": row.get::<_, i64>(13)?,
        "bodyPreview": row.get::<_, String>(14)?,
        "bodyTruncated": row.get::<_, i64>(15)? == 1,
        "bodyStorage": row.get::<_, String>(16)?,
        "bodyFilePath": row.get::<_, String>(17)?,
        "bodyFileName": row.get::<_, String>(18)?,
        "bodyExtension": row.get::<_, String>(19)?,
        "bodyHash": row.get::<_, String>(20)?,
        "bodyPreviewError": row.get::<_, Option<String>>(21)?,
        "createdAt": row.get::<_, String>(22)?,
        "hasRequestSnapshot": request_snapshot_json.is_some(),
        "hasExecutedRequestSnapshot": executed_snapshot_json.is_some()
    });
    if include_request_snapshot {
        value["requestSnapshot"] = match request_snapshot_json {
            Some(raw) => serde_json::from_str(&raw).unwrap_or(Value::Null),
            None => Value::Null,
        };
    }
    Ok(value)
}

pub(crate) fn history_get_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let history_id = parse_i64(payload, "historyId")?;
    let detail = conn
        .query_row(
            "SELECT id, collection_id, environment_id, request_id, name, method, url, final_url,
                    status, duration_ms, ok, error, response_content_type, response_size,
                    response_body_preview, response_body_truncated,
                    response_body_storage, response_body_file_path, response_body_file_name,
                    response_body_extension, response_body_hash, response_preview_error,
                    created_at,
                    request_snapshot_json, executed_request_snapshot_json, replayed_from_history_id,
                    note, pinned
             FROM api_workbench_history WHERE id=?1",
            [history_id],
            |row| history_row_json(row, true),
        )
        .map_err(|_| "历史记录不存在".to_string())?;
    if detail["hasRequestSnapshot"].as_bool().unwrap_or(false)
        && detail["requestSnapshot"].is_null()
    {
        return Err("历史快照已损坏".to_string());
    }
    Ok(detail)
}

pub(crate) fn history_replay_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let history_id = parse_i64(payload, "historyId")?;
    let (raw, name): (Option<String>, String) = conn
        .query_row(
            "SELECT executed_request_snapshot_json, name FROM api_workbench_history WHERE id=?1",
            [history_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| "历史记录不存在".to_string())?;
    let raw = raw.ok_or_else(|| "旧历史缺少执行快照，请载入后手动发送".to_string())?;
    let snapshot: ExecutedRequestSnapshot =
        serde_json::from_str(&raw).map_err(|_| "历史快照已损坏".to_string())?;
    let result = execute_api_workbench_request(&snapshot)?;
    let history_record_id = insert_history_with_conn(
        conn,
        &HistoryInsert {
            collection_id: None,
            environment_id: None,
            request_id: None,
            name,
            method: snapshot.method.clone(),
            url: snapshot.final_url.clone(),
            final_url: snapshot.final_url.clone(),
            status: result["status"].as_i64(),
            duration_ms: result["durationMs"].as_u64().unwrap_or(0),
            ok: result["ok"].as_bool().unwrap_or(false),
            error: result["error"].as_str().map(|s| s.to_string()),
            response_content_type: result["contentType"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            response_size: result["bodySize"].as_u64().unwrap_or(0) as usize,
            response_body_preview: result["bodyText"].as_str().unwrap_or_default().to_string(),
            response_body_truncated: result["bodyTruncated"].as_bool().unwrap_or(false),
            response_body_storage: result["bodyStorage"].as_str().unwrap_or("text").to_string(),
            response_body_file_path: result["bodyFilePath"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            response_body_file_name: result["bodyFileName"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            response_body_extension: result["bodyExtension"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            response_body_hash: result["bodyHash"].as_str().unwrap_or_default().to_string(),
            response_preview_error: result["bodyPreviewError"].as_str().map(|s| s.to_string()),
            request_snapshot_json: None,
            executed_request_snapshot_json: Some(raw),
            replayed_from_history_id: Some(history_id),
            pinned: false,
            note: String::new(),
        },
    )?;
    let mut out = result;
    out["historyId"] = json!(history_record_id);
    Ok(out)
}

pub(crate) fn history_list_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let query = payload["query"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_string();
    let pinned_only = payload["pinnedOnly"].as_bool().unwrap_or(false);
    let limit = payload["limit"]
        .as_i64()
        .unwrap_or(MAX_HISTORY_ROWS)
        .clamp(1, MAX_HISTORY_ROWS);
    let pattern = format!(
        "%{}%",
        query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let sql = r#"
SELECT id, collection_id, environment_id, request_id, name, method, url, final_url,
       status, duration_ms, ok, error, response_content_type, response_size,
       response_body_preview, response_body_truncated,
       response_body_storage, response_body_file_path, response_body_file_name,
       response_body_extension, response_body_hash, response_preview_error,
       created_at,
       request_snapshot_json, executed_request_snapshot_json, replayed_from_history_id,
       note, pinned
FROM api_workbench_history
WHERE (?1 = 0 OR pinned = 1)
  AND (
    ?2 = ''
    OR name LIKE ?3 ESCAPE '\'
    OR note LIKE ?3 ESCAPE '\'
    OR method LIKE ?3 ESCAPE '\'
    OR url LIKE ?3 ESCAPE '\'
    OR final_url LIKE ?3 ESCAPE '\'
    OR CAST(status AS TEXT) LIKE ?3 ESCAPE '\'
    OR COALESCE(error, '') LIKE ?3 ESCAPE '\'
    OR response_content_type LIKE ?3 ESCAPE '\'
  )
ORDER BY created_at DESC, id DESC
LIMIT ?4"#;
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("prepare history failed: {e}"))?;
    let rows = stmt
        .query_map(
            params![if pinned_only { 1 } else { 0 }, query, pattern, limit],
            |row| history_row_json(row, false),
        )
        .map_err(|e| format!("query history failed: {e}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| e.to_string())?);
    }
    Ok(json!({ "items": items }))
}

pub(crate) fn history_update_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    let name = payload["name"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_string();
    let note = payload["note"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_string();
    if note.chars().count() > MAX_HISTORY_NOTE_CHARS {
        return Err("历史备注超过 2000 字符".to_string());
    }
    let pinned = payload["pinned"]
        .as_bool()
        .ok_or_else(|| "pinned must be a boolean".to_string())?;
    let changed = conn
        .execute(
            "UPDATE api_workbench_history SET name=?1, note=?2, pinned=?3 WHERE id=?4",
            params![name, note, if pinned { 1 } else { 0 }, id],
        )
        .map_err(|e| format!("update history failed: {e}"))?;
    if changed == 0 {
        return Err("历史记录不存在".to_string());
    }
    Ok(json!({ "ok": true }))
}

pub(crate) fn history_clear_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let include_pinned = payload["includePinned"].as_bool().unwrap_or(false);
    let cache_refs = if include_pinned {
        collect_history_cache_refs(conn, "")?
    } else {
        collect_history_cache_refs(conn, "WHERE pinned=0")?
    };
    let sql = if include_pinned {
        "DELETE FROM api_workbench_history"
    } else {
        "DELETE FROM api_workbench_history WHERE pinned=0"
    };
    conn.execute(sql, [])
        .map_err(|e| format!("clear history failed: {e}"))?;
    cleanup_unreferenced_history_cache_files(conn, &cache_refs);
    Ok(json!({ "ok": true }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::api_workbench::test_conn;
    use crate::tools::api_workbench::*;

    #[test]
    fn history_save_request_creates_request_from_available_history_fields() {
        let conn = test_conn();
        let collection =
            collection_create_with_conn(&conn, &json!({ "name": "Demo", "description": "" }))
                .expect("collection");
        let collection_id = collection["id"].as_i64().unwrap();
        insert_history_with_conn(
            &conn,
            &HistoryInsert {
                collection_id: Some(collection_id),
                environment_id: None,
                request_id: None,
                name: "".into(),
                method: "POST".into(),
                url: "/api/users".into(),
                final_url: "http://127.0.0.1:8080/api/users".into(),
                status: Some(201),
                duration_ms: 23,
                ok: true,
                error: None,
                response_content_type: "application/json".into(),
                response_size: 11,
                response_body_preview: "{\"ok\":true}".into(),
                response_body_truncated: false,
                request_snapshot_json: None,
                executed_request_snapshot_json: None,
                replayed_from_history_id: None,
                pinned: false,
                note: String::new(),
                ..HistoryInsert::default()
            },
        )
        .expect("history");

        let saved = history_save_request_with_conn(
            &conn,
            &json!({
                "historyId": 1,
                "collectionId": collection_id,
                "folderId": null,
                "name": "POST /api/users"
            }),
        )
        .expect("save request");
        let detail = request_get_with_conn(&conn, &json!({ "id": saved["id"] })).expect("detail");

        assert_eq!(detail["name"], "POST /api/users");
        assert_eq!(detail["draft"]["method"], "POST");
        assert_eq!(detail["draft"]["url"], "/api/users");
        assert_eq!(detail["draft"]["headers"], json!([]));
        assert_eq!(detail["draft"]["bodyType"], "none");
        assert!(detail["description"].as_str().unwrap().contains("201"));
        assert!(detail["description"]
            .as_str()
            .unwrap()
            .contains("http://127.0.0.1:8080/api/users"));
    }

    #[test]
    fn history_get_returns_request_snapshot_for_loading() {
        let conn = test_conn();
        let request_snapshot = json!({
            "method": "POST",
            "url": "/login",
            "query": [{ "enabled": true, "key": "a", "value": "1" }],
            "headers": [{ "enabled": true, "key": "X-A", "value": "b" }],
            "bodyType": "json",
            "body": "{\"ok\":true}",
            "form": [],
            "timeoutMs": 15000
        });
        insert_history_with_conn(
            &conn,
            &HistoryInsert {
                collection_id: None,
                environment_id: None,
                request_id: None,
                name: "Login".into(),
                method: "POST".into(),
                url: "/login".into(),
                final_url: "http://127.0.0.1/login".into(),
                status: Some(200),
                duration_ms: 10,
                ok: true,
                error: None,
                response_content_type: "application/json".into(),
                response_size: 2,
                response_body_preview: "{}".into(),
                response_body_truncated: false,
                request_snapshot_json: Some(request_snapshot.to_string()),
                executed_request_snapshot_json: None,
                replayed_from_history_id: None,
                pinned: false,
                note: String::new(),
                ..HistoryInsert::default()
            },
        )
        .expect("history");
        let id: i64 = conn
            .query_row("SELECT id FROM api_workbench_history", [], |row| row.get(0))
            .unwrap();

        let detail = history_get_with_conn(&conn, &json!({ "historyId": id })).expect("detail");
        assert_eq!(detail["requestSnapshot"]["headers"][0]["key"], "X-A");
        assert_eq!(detail["hasRequestSnapshot"], true);
        assert_eq!(detail["hasExecutedRequestSnapshot"], false);
    }

    #[test]
    fn history_replay_uses_executed_snapshot_without_environment() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 2048];
                let size = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..size]);
                assert!(req.contains("GET /replay?token=abc HTTP/1.1"));
                assert!(req.contains("X-Token: abc"));
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nreplay");
            }
        });

        let conn = test_conn();
        let executed_snapshot = json!({
            "method": "GET",
            "finalUrl": format!("http://127.0.0.1:{port}/replay?token=abc"),
            "headers": [{ "enabled": true, "key": "X-Token", "value": "abc" }],
            "bodyType": "none",
            "body": "",
            "form": [],
            "timeoutMs": 10000
        });
        insert_history_with_conn(
            &conn,
            &HistoryInsert {
                collection_id: None,
                environment_id: None,
                request_id: None,
                name: "Replay".into(),
                method: "GET".into(),
                url: "/replay".into(),
                final_url: format!("http://127.0.0.1:{port}/replay?token=abc"),
                status: Some(200),
                duration_ms: 1,
                ok: true,
                error: None,
                response_content_type: "text/plain".into(),
                response_size: 6,
                response_body_preview: "replay".into(),
                response_body_truncated: false,
                request_snapshot_json: None,
                executed_request_snapshot_json: Some(executed_snapshot.to_string()),
                replayed_from_history_id: None,
                pinned: false,
                note: String::new(),
                ..HistoryInsert::default()
            },
        )
        .expect("history");
        let id: i64 = conn
            .query_row("SELECT id FROM api_workbench_history", [], |row| row.get(0))
            .unwrap();

        let result = history_replay_with_conn(&conn, &json!({ "historyId": id })).expect("replay");
        assert_eq!(result["status"], 200);
        assert_eq!(result["bodyText"], "replay");
        assert!(result["historyId"].as_i64().unwrap() > id);
        let parent: i64 = conn
            .query_row(
                "SELECT replayed_from_history_id FROM api_workbench_history WHERE id=?1",
                [result["historyId"].as_i64().unwrap()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(parent, id);
    }

    #[test]
    fn history_save_request_uses_request_snapshot_when_available() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let request_snapshot = json!({
            "method": "PATCH",
            "url": "/users/1",
            "query": [{ "enabled": true, "key": "expand", "value": "roles" }],
            "headers": [{ "enabled": true, "key": "X-Token", "value": "{{TOKEN}}" }],
            "bodyType": "json",
            "body": "{\"name\":\"demo\"}",
            "form": [],
            "timeoutMs": 12000
        });
        insert_history_with_conn(
            &conn,
            &HistoryInsert {
                collection_id: Some(collection_id),
                environment_id: None,
                request_id: None,
                name: "Patch user".into(),
                method: "PATCH".into(),
                url: "/users/1".into(),
                final_url: "http://127.0.0.1/users/1?expand=roles".into(),
                status: Some(200),
                duration_ms: 7,
                ok: true,
                error: None,
                response_content_type: "application/json".into(),
                response_size: 2,
                response_body_preview: "{}".into(),
                response_body_truncated: false,
                request_snapshot_json: Some(request_snapshot.to_string()),
                executed_request_snapshot_json: None,
                replayed_from_history_id: None,
                pinned: false,
                note: String::new(),
                ..HistoryInsert::default()
            },
        )
        .expect("history");
        let history_id: i64 = conn
            .query_row("SELECT id FROM api_workbench_history", [], |row| row.get(0))
            .unwrap();

        let saved = history_save_request_with_conn(
            &conn,
            &json!({ "historyId": history_id, "collectionId": collection_id, "folderId": null, "name": "Saved" }),
        )
        .expect("save");
        let detail = request_get_with_conn(&conn, &json!({ "id": saved["id"].as_i64().unwrap() }))
            .expect("detail");
        assert_eq!(detail["draft"]["method"], "PATCH");
        assert_eq!(detail["draft"]["headers"][0]["value"], "{{TOKEN}}");
        assert_eq!(detail["draft"]["timeoutMs"], 12000);
    }

    #[test]
    fn history_update_allows_empty_name_and_validates_note_length() {
        let conn = test_conn();
        insert_history_with_conn(
            &conn,
            &HistoryInsert {
                collection_id: None,
                environment_id: None,
                request_id: None,
                name: "Old".into(),
                method: "GET".into(),
                url: "/x".into(),
                final_url: "/x".into(),
                status: Some(200),
                duration_ms: 1,
                ok: true,
                error: None,
                response_content_type: String::new(),
                response_size: 0,
                response_body_preview: String::new(),
                response_body_truncated: false,
                request_snapshot_json: None,
                executed_request_snapshot_json: None,
                replayed_from_history_id: None,
                pinned: false,
                note: String::new(),
                ..HistoryInsert::default()
            },
        )
        .expect("history");
        let id: i64 = conn
            .query_row("SELECT id FROM api_workbench_history", [], |row| row.get(0))
            .unwrap();

        history_update_with_conn(
            &conn,
            &json!({ "id": id, "name": "", "note": "keep", "pinned": true }),
        )
        .expect("update");
        let (name, note, pinned): (String, String, i64) = conn
            .query_row(
                "SELECT name, note, pinned FROM api_workbench_history WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(name, "");
        assert_eq!(note, "keep");
        assert_eq!(pinned, 1);

        let long_note = "x".repeat(MAX_HISTORY_NOTE_CHARS + 1);
        let err = history_update_with_conn(
            &conn,
            &json!({ "id": id, "name": "", "note": long_note, "pinned": true }),
        )
        .expect_err("long note");
        assert!(err.contains("备注"));
    }

    #[test]
    fn history_clear_preserves_pinned_by_default() {
        let conn = test_conn();
        for (name, pinned) in [("keep", true), ("drop", false)] {
            insert_history_with_conn(
                &conn,
                &HistoryInsert {
                    collection_id: None,
                    environment_id: None,
                    request_id: None,
                    name: name.into(),
                    method: "GET".into(),
                    url: format!("/{name}"),
                    final_url: format!("/{name}"),
                    status: Some(200),
                    duration_ms: 1,
                    ok: true,
                    error: None,
                    response_content_type: String::new(),
                    response_size: 0,
                    response_body_preview: String::new(),
                    response_body_truncated: false,
                    request_snapshot_json: None,
                    executed_request_snapshot_json: None,
                    replayed_from_history_id: None,
                    pinned,
                    note: String::new(),
                    ..HistoryInsert::default()
                },
            )
            .expect("history");
        }

        history_clear_with_conn(&conn, &json!({ "includePinned": false })).expect("clear");
        let names: Vec<String> = conn
            .prepare("SELECT name FROM api_workbench_history ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(names, vec!["keep"]);

        history_clear_with_conn(&conn, &json!({ "includePinned": true })).expect("clear all");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM api_workbench_history", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn history_list_filters_search_and_pinned() {
        let conn = test_conn();
        insert_history_with_conn(
            &conn,
            &HistoryInsert {
                collection_id: None,
                environment_id: None,
                request_id: None,
                name: "Login ok".into(),
                method: "POST".into(),
                url: "/login".into(),
                final_url: "http://127.0.0.1/login".into(),
                status: Some(200),
                duration_ms: 1,
                ok: true,
                error: None,
                response_content_type: "application/json".into(),
                response_size: 2,
                response_body_preview: "{}".into(),
                response_body_truncated: false,
                request_snapshot_json: Some("{} ".trim().to_string()),
                executed_request_snapshot_json: Some("{} ".trim().to_string()),
                replayed_from_history_id: None,
                pinned: true,
                note: "admin token".into(),
                ..HistoryInsert::default()
            },
        )
        .expect("history");
        insert_history_with_conn(
            &conn,
            &HistoryInsert {
                collection_id: None,
                environment_id: None,
                request_id: None,
                name: "Health".into(),
                method: "GET".into(),
                url: "/health".into(),
                final_url: "http://127.0.0.1/health".into(),
                status: Some(500),
                duration_ms: 1,
                ok: false,
                error: Some("boom".into()),
                response_content_type: "text/plain".into(),
                response_size: 4,
                response_body_preview: "fail".into(),
                response_body_truncated: false,
                request_snapshot_json: None,
                executed_request_snapshot_json: None,
                replayed_from_history_id: None,
                pinned: false,
                note: String::new(),
                ..HistoryInsert::default()
            },
        )
        .expect("history");

        let pinned = history_list_with_conn(
            &conn,
            &json!({ "query": "token", "pinnedOnly": true, "limit": 200 }),
        )
        .expect("list");
        assert_eq!(pinned["items"].as_array().unwrap().len(), 1);
        assert_eq!(pinned["items"][0]["name"], "Login ok");
        assert_eq!(pinned["items"][0]["hasRequestSnapshot"], true);
        assert_eq!(pinned["items"][0]["hasExecutedRequestSnapshot"], true);
    }

    #[test]
    fn history_clear_removes_unreferenced_response_cache() {
        use std::fs;
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let body = vec![0x25, b'P', b'D', b'F', b'-', b'1'];
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/pdf\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&body);
            }
        });

        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let environment_id = c["activeEnvironmentId"].as_i64().unwrap();
        environment_save_with_conn(
            &conn,
            &json!({
                "id": environment_id,
                "collectionId": collection_id,
                "name": "开发",
                "variables": [{ "name": "BASE_URL", "value": format!("http://127.0.0.1:{port}"), "isSecret": false }]
            }),
        )
        .expect("env");
        let result = send_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "environmentId": environment_id,
                "draft": {
                    "method": "GET",
                    "url": "/report.pdf",
                    "query": [],
                    "headers": [],
                    "bodyType": "none",
                    "body": "",
                    "form": [],
                    "timeoutMs": 10000
                }
            }),
        )
        .expect("send");
        let file_path = result["bodyFilePath"]
            .as_str()
            .expect("cache path")
            .to_string();
        assert!(fs::metadata(&file_path).is_ok());

        history_clear_with_conn(&conn, &json!({ "includePinned": true })).expect("clear history");

        assert!(fs::metadata(&file_path).is_err());
    }

    #[test]
    fn history_clear_keeps_cache_still_referenced_by_pinned_history() {
        use std::fs;

        let conn = test_conn();
        let cache_dir = get_api_workbench_response_cache_dir().expect("cache dir");
        let (file_path, file_name, extension, hash) =
            persist_response_cache_file(&cache_dir, b"same-cache", None, Some("bin".into()))
                .expect("cache file");

        for pinned in [true, false] {
            insert_history_with_conn(
                &conn,
                &HistoryInsert {
                    name: if pinned { "keep".into() } else { "drop".into() },
                    method: "GET".into(),
                    url: "/file".into(),
                    final_url: "/file".into(),
                    status: Some(200),
                    ok: true,
                    response_body_storage: "file".into(),
                    response_body_file_path: file_path.clone(),
                    response_body_file_name: file_name.clone(),
                    response_body_extension: extension.clone(),
                    response_body_hash: hash.clone(),
                    pinned,
                    ..HistoryInsert::default()
                },
            )
            .expect("history");
        }

        history_clear_with_conn(&conn, &json!({ "includePinned": false })).expect("clear unpinned");
        assert!(fs::metadata(&file_path).is_ok());

        history_clear_with_conn(&conn, &json!({ "includePinned": true })).expect("clear all");
        assert!(fs::metadata(&file_path).is_err());
    }

    #[test]
    fn history_clear_deletes_same_hash_cache_when_path_is_unreferenced() {
        use std::fs;

        let conn = test_conn();
        let cache_dir = get_api_workbench_response_cache_dir().expect("cache dir");
        let bytes = b"same-bytes";
        let (drop_path, drop_name, extension, hash) =
            persist_response_cache_file(&cache_dir, bytes, None, Some("bin".into()))
                .expect("cache file");
        let keep_path = cache_dir.join("same-hash-different-path.bin");
        fs::write(&keep_path, bytes).expect("keep cache file");
        let keep_path = keep_path
            .canonicalize()
            .expect("canonical keep path")
            .to_string_lossy()
            .to_string();

        for (name, path, pinned) in [
            ("drop", drop_path.clone(), false),
            ("keep", keep_path.clone(), true),
        ] {
            insert_history_with_conn(
                &conn,
                &HistoryInsert {
                    name: name.into(),
                    method: "GET".into(),
                    url: "/file".into(),
                    final_url: "/file".into(),
                    status: Some(200),
                    ok: true,
                    response_body_storage: "file".into(),
                    response_body_file_path: path,
                    response_body_file_name: drop_name.clone(),
                    response_body_extension: extension.clone(),
                    response_body_hash: hash.clone(),
                    pinned,
                    ..HistoryInsert::default()
                },
            )
            .expect("history");
        }

        history_clear_with_conn(&conn, &json!({ "includePinned": false })).expect("clear unpinned");

        assert!(fs::metadata(&drop_path).is_err());
        assert!(fs::metadata(&keep_path).is_ok());
    }
}

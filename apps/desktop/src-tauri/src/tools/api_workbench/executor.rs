use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Read;
use std::time::{Duration, Instant};

use super::helpers::{
    build_final_url, clamp_timeout_ms, parse_i64, prepare_request_body, resolve_template,
    serialize_limited_json, MAX_HISTORY_BODY_PREVIEW_BYTES, MAX_HISTORY_ROWS,
    MAX_HISTORY_SNAPSHOT_BYTES, MAX_RESPONSE_BODY_BYTES,
};
use super::response::{
    build_response_body_payload, cleanup_unreferenced_history_cache_files,
    collect_history_cache_refs,
};
use super::types::{ExecutedRequestSnapshot, KeyValueRow, PreparedBody, RequestDraft};

pub(crate) fn load_variables(
    conn: &Connection,
    environment_id: i64,
) -> Result<(HashMap<String, String>, String), String> {
    let mut vars = HashMap::new();
    let mut stmt = conn
        .prepare("SELECT name, value FROM api_workbench_global_variables ORDER BY sort_order ASC")
        .map_err(|e| format!("prepare global variables failed: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("query global variables failed: {e}"))?;
    for row in rows {
        let (name, value) = row.map_err(|e| e.to_string())?;
        vars.insert(name, value);
    }

    let mut base_url = String::new();
    let mut env_stmt = conn
        .prepare(
            "SELECT name, value FROM api_workbench_environment_variables
             WHERE environment_id=?1 ORDER BY sort_order ASC",
        )
        .map_err(|e| format!("prepare environment variables failed: {e}"))?;
    let env_rows = env_stmt
        .query_map([environment_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("query environment variables failed: {e}"))?;
    for row in env_rows {
        let (name, value) = row.map_err(|e| e.to_string())?;
        if name == "BASE_URL" {
            base_url = value.clone();
        }
        vars.insert(name, value);
    }
    Ok((vars, base_url))
}

pub(crate) fn resolve_rows(
    rows: &[KeyValueRow],
    vars: &HashMap<String, String>,
) -> Result<Vec<KeyValueRow>, String> {
    let mut out = Vec::new();
    for row in rows {
        if !row.enabled {
            continue;
        }
        out.push(KeyValueRow {
            enabled: true,
            key: resolve_template(&row.key, vars)?,
            value: resolve_template(&row.value, vars)?,
        });
    }
    Ok(out)
}

pub(crate) fn prepare_api_workbench_request(
    draft: &RequestDraft,
    vars: &HashMap<String, String>,
    base_url: &str,
) -> Result<ExecutedRequestSnapshot, String> {
    let resolved_url = resolve_template(&draft.url, vars)?;
    let resolved_query = resolve_rows(&draft.query, vars)?;
    let mut resolved_headers = resolve_rows(&draft.headers, vars)?;
    let resolved_body = if matches!(draft.body_type.as_str(), "json" | "text") {
        resolve_template(&draft.body, vars)?
    } else {
        String::new()
    };
    let resolved_form = if draft.body_type == "form-urlencoded" {
        resolve_rows(&draft.form, vars)?
    } else {
        Vec::new()
    };
    let final_url = build_final_url(base_url, &resolved_url, &resolved_query)?;
    let prepared = prepare_request_body(
        &draft.body_type,
        &resolved_body,
        &resolved_form,
        &resolved_headers,
    )?;
    if let Some(content_type) = prepared.content_type {
        resolved_headers.push(KeyValueRow {
            enabled: true,
            key: "Content-Type".to_string(),
            value: content_type,
        });
    }
    Ok(ExecutedRequestSnapshot {
        method: draft.method.clone(),
        final_url,
        headers: resolved_headers,
        body_type: draft.body_type.clone(),
        body: resolved_body,
        form: resolved_form,
        timeout_ms: clamp_timeout_ms(draft.timeout_ms),
        follow_redirects: draft.follow_redirects,
    })
}

pub(crate) fn execute_api_workbench_request(snapshot: &ExecutedRequestSnapshot) -> Result<Value, String> {
    let prepared = prepare_request_body(
        &snapshot.body_type,
        &snapshot.body,
        &snapshot.form,
        &snapshot.headers,
    )?;
    let draft_for_timeout = RequestDraft {
        method: snapshot.method.clone(),
        url: snapshot.final_url.clone(),
        query: Vec::new(),
        headers: snapshot.headers.clone(),
        body_type: snapshot.body_type.clone(),
        body: snapshot.body.clone(),
        form: snapshot.form.clone(),
        timeout_ms: snapshot.timeout_ms,
        follow_redirects: snapshot.follow_redirects,
    };
    execute_http_request(
        &draft_for_timeout,
        &snapshot.final_url,
        &snapshot.headers,
        prepared,
    )
}

pub(crate) fn execute_http_request(
    draft: &RequestDraft,
    final_url: &str,
    headers: &[KeyValueRow],
    prepared: PreparedBody,
) -> Result<Value, String> {
    let started = Instant::now();
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_millis(clamp_timeout_ms(draft.timeout_ms)))
        .redirects(if draft.follow_redirects { 10 } else { 0 })
        .build();
    let method = draft.method.to_ascii_uppercase();
    let mut request = match method.as_str() {
        "GET" => agent.get(final_url),
        "POST" => agent.post(final_url),
        "PUT" => agent.put(final_url),
        "PATCH" => agent.request("PATCH", final_url),
        "DELETE" => agent.delete(final_url),
        "HEAD" => agent.head(final_url),
        "OPTIONS" => agent.request("OPTIONS", final_url),
        _ => return Err(format!("unsupported method: {method}")),
    };
    let mut request_headers = headers.to_vec();
    for row in headers {
        if row.enabled && !row.key.trim().is_empty() {
            request = request.set(row.key.trim(), row.value.as_str());
        }
    }
    if let Some(content_type) = prepared.content_type.as_deref() {
        request = request.set("Content-Type", content_type);
        request_headers.push(KeyValueRow {
            enabled: true,
            key: "Content-Type".to_string(),
            value: content_type.to_string(),
        });
    }

    let result = if let Some(body) = prepared.body {
        request.send_bytes(&body)
    } else {
        request.call()
    };
    let duration_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(resp) => response_to_json(final_url, duration_ms, resp, None, &request_headers),
        Err(ureq::Error::Status(_, resp)) => {
            response_to_json(final_url, duration_ms, resp, None, &request_headers)
        }
        Err(err) => Ok(json!({
            "finalUrl": final_url,
            "status": null,
            "statusText": "",
            "ok": false,
            "durationMs": duration_ms,
            "requestHeaders": request_headers,
            "responseHeaders": [],
            "bodyText": "",
            "bodySize": 0,
            "bodyTruncated": false,
            "contentType": "",
            "bodyStorage": "empty",
            "bodyFilePath": "",
            "bodyFileName": "",
            "bodyExtension": "",
            "bodyHash": "",
            "bodyPreviewError": null,
            "error": err.to_string()
        })),
    }
}

pub(crate) fn response_to_json(
    final_url_fallback: &str,
    duration_ms: u64,
    resp: ureq::Response,
    forced_error: Option<String>,
    request_headers: &[KeyValueRow],
) -> Result<Value, String> {
    let status = resp.status();
    let status_text = resp.status_text().to_string();
    // 跟随重定向时以实际到达的 URL 为准
    let final_url = resp.get_url().to_string();
    let final_url = if final_url.is_empty() {
        final_url_fallback.to_string()
    } else {
        final_url
    };
    let final_url = final_url.as_str();
    let content_type = resp.header("Content-Type").unwrap_or("").to_string();
    let content_disposition = resp.header("Content-Disposition").unwrap_or("").to_string();
    let response_headers: Vec<Value> = resp
        .headers_names()
        .into_iter()
        .map(|key| {
            let value = resp.header(&key).unwrap_or("").to_string();
            json!({ "enabled": true, "key": key, "value": value })
        })
        .collect();
    let mut reader = resp
        .into_reader()
        .take((MAX_RESPONSE_BODY_BYTES + 1) as u64);
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read response body failed: {e}"))?;
    let body_truncated = bytes.len() > MAX_RESPONSE_BODY_BYTES;
    if body_truncated {
        bytes.truncate(MAX_RESPONSE_BODY_BYTES);
    }
    let body = build_response_body_payload(
        final_url,
        &content_type,
        &content_disposition,
        bytes,
        body_truncated,
    );
    Ok(json!({
        "finalUrl": final_url,
        "status": status,
        "statusText": status_text,
        "ok": (200..300).contains(&status),
        "durationMs": duration_ms,
        "requestHeaders": request_headers,
        "responseHeaders": response_headers,
        "bodyText": body.body_text,
        "bodySize": body.body_size,
        "bodyTruncated": body.body_truncated,
        "contentType": content_type,
        "bodyStorage": body.body_storage,
        "bodyFilePath": body.body_file_path,
        "bodyFileName": body.body_file_name,
        "bodyExtension": body.body_extension,
        "bodyHash": body.body_hash,
        "bodyPreviewError": body.body_preview_error,
        "error": forced_error
    }))
}

pub(crate) struct HistoryInsert {
    pub(crate) collection_id: Option<i64>,
    pub(crate) environment_id: Option<i64>,
    pub(crate) request_id: Option<i64>,
    pub(crate) name: String,
    pub(crate) method: String,
    pub(crate) url: String,
    pub(crate) final_url: String,
    pub(crate) status: Option<i64>,
    pub(crate) duration_ms: u64,
    pub(crate) ok: bool,
    pub(crate) error: Option<String>,
    pub(crate) response_content_type: String,
    pub(crate) response_size: usize,
    pub(crate) response_body_preview: String,
    pub(crate) response_body_truncated: bool,
    pub(crate) response_body_storage: String,
    pub(crate) response_body_file_path: String,
    pub(crate) response_body_file_name: String,
    pub(crate) response_body_extension: String,
    pub(crate) response_body_hash: String,
    pub(crate) response_preview_error: Option<String>,
    pub(crate) request_snapshot_json: Option<String>,
    pub(crate) executed_request_snapshot_json: Option<String>,
    pub(crate) replayed_from_history_id: Option<i64>,
    pub(crate) pinned: bool,
    pub(crate) note: String,
}

impl Default for HistoryInsert {
    fn default() -> Self {
        Self {
            collection_id: None,
            environment_id: None,
            request_id: None,
            name: String::new(),
            method: "GET".to_string(),
            url: String::new(),
            final_url: String::new(),
            status: None,
            duration_ms: 0,
            ok: false,
            error: None,
            response_content_type: String::new(),
            response_size: 0,
            response_body_preview: String::new(),
            response_body_truncated: false,
            response_body_storage: "text".to_string(),
            response_body_file_path: String::new(),
            response_body_file_name: String::new(),
            response_body_extension: String::new(),
            response_body_hash: String::new(),
            response_preview_error: None,
            request_snapshot_json: None,
            executed_request_snapshot_json: None,
            replayed_from_history_id: None,
            pinned: false,
            note: String::new(),
        }
    }
}

pub(crate) fn truncate_to_max_bytes(input: &str, max: usize) -> String {
    if input.len() <= max {
        return input.to_string();
    }
    let mut end = 0;
    for (idx, _) in input.char_indices() {
        if idx > max {
            break;
        }
        end = idx;
    }
    input[..end].to_string()
}

pub(crate) fn insert_history_with_conn(conn: &Connection, item: &HistoryInsert) -> Result<i64, String> {
    let preview_too_large = item.response_body_preview.len() > MAX_HISTORY_BODY_PREVIEW_BYTES;
    let preview =
        truncate_to_max_bytes(&item.response_body_preview, MAX_HISTORY_BODY_PREVIEW_BYTES);
    conn.execute(
        "INSERT INTO api_workbench_history(
            collection_id, environment_id, request_id, name, method, url, final_url,
            status, duration_ms, ok, error, response_content_type, response_size,
            response_body_preview, response_body_truncated, request_snapshot_json,
            executed_request_snapshot_json, replayed_from_history_id, pinned, note,
            response_body_storage, response_body_file_path, response_body_file_name,
            response_body_extension, response_body_hash, response_preview_error
         )
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
        params![
            item.collection_id,
            item.environment_id,
            item.request_id,
            item.name,
            item.method,
            item.url,
            item.final_url,
            item.status,
            item.duration_ms as i64,
            if item.ok { 1 } else { 0 },
            item.error,
            item.response_content_type,
            item.response_size as i64,
            preview,
            if item.response_body_truncated || preview_too_large { 1 } else { 0 },
            item.request_snapshot_json,
            item.executed_request_snapshot_json,
            item.replayed_from_history_id,
            if item.pinned { 1 } else { 0 },
            item.note,
            item.response_body_storage,
            item.response_body_file_path,
            item.response_body_file_name,
            item.response_body_extension,
            item.response_body_hash,
            item.response_preview_error,
        ],
    )
    .map_err(|e| format!("insert history failed: {e}"))?;
    let id = conn.last_insert_rowid();
    let trimmed_cache_refs = collect_history_cache_refs(
        conn,
        &format!(
            "WHERE pinned=0
           AND id NOT IN (
            SELECT id FROM api_workbench_history
            WHERE pinned=0
            ORDER BY created_at DESC, id DESC
            LIMIT {MAX_HISTORY_ROWS}
         )"
        ),
    )?;
    conn.execute(
        "DELETE FROM api_workbench_history
         WHERE pinned=0
           AND id NOT IN (
            SELECT id FROM api_workbench_history
            WHERE pinned=0
            ORDER BY created_at DESC, id DESC
            LIMIT ?1
         )",
        [MAX_HISTORY_ROWS],
    )
    .map_err(|e| format!("trim history failed: {e}"))?;
    cleanup_unreferenced_history_cache_files(conn, &trimmed_cache_refs);
    Ok(id)
}

pub(crate) fn send_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = payload["collectionId"].as_i64();
    let environment_id = parse_i64(payload, "environmentId")?;
    let request_id = payload["requestId"].as_i64();
    let draft: RequestDraft = serde_json::from_value(payload["draft"].clone())
        .map_err(|e| format!("请求草稿格式错误: {e}"))?;

    if let Some(collection_id) = collection_id {
        let env_owner: i64 = conn
            .query_row(
                "SELECT collection_id FROM api_workbench_environments WHERE id=?1",
                [environment_id],
                |row| row.get(0),
            )
            .map_err(|_| "环境不存在".to_string())?;
        if env_owner != collection_id {
            return Err("环境不属于当前集合".to_string());
        }
        if let Some(request_id) = request_id {
            let request_owner: i64 = conn
                .query_row(
                    "SELECT collection_id FROM api_workbench_requests WHERE id=?1",
                    [request_id],
                    |row| row.get(0),
                )
                .map_err(|_| "接口不存在".to_string())?;
            if request_owner != collection_id {
                return Err("接口不属于当前集合".to_string());
            }
        }
    }

    let (vars, base_url) = load_variables(conn, environment_id)?;
    let executed_snapshot = prepare_api_workbench_request(&draft, &vars, &base_url)?;
    let result = execute_api_workbench_request(&executed_snapshot)?;
    let request_snapshot_json =
        serialize_limited_json(&draft, MAX_HISTORY_SNAPSHOT_BYTES, "请求快照体积超过限制")?;
    let executed_snapshot_json = serialize_limited_json(
        &executed_snapshot,
        MAX_HISTORY_SNAPSHOT_BYTES,
        "执行快照体积超过限制",
    )?;
    insert_history_with_conn(
        conn,
        &HistoryInsert {
            collection_id,
            environment_id: Some(environment_id),
            request_id,
            name: payload["name"].as_str().unwrap_or_default().to_string(),
            method: draft.method.clone(),
            url: draft.url.clone(),
            final_url: result["finalUrl"]
                .as_str()
                .unwrap_or(executed_snapshot.final_url.as_str())
                .to_string(),
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
            request_snapshot_json: Some(request_snapshot_json),
            executed_request_snapshot_json: Some(executed_snapshot_json),
            replayed_from_history_id: None,
            pinned: false,
            note: String::new(),
        },
    )?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::api_workbench::test_conn;
    use crate::tools::api_workbench::*;

    #[test]
    fn send_requires_environment_and_request_to_match_collection() {
        let conn = test_conn();
        let a = collection_create_with_conn(&conn, &json!({ "name": "A" })).expect("a");
        let b = collection_create_with_conn(&conn, &json!({ "name": "B" })).expect("b");
        let a_id = a["id"].as_i64().unwrap();
        let b_id = b["id"].as_i64().unwrap();
        let b_env_id = b["activeEnvironmentId"].as_i64().unwrap();
        let saved = request_save_with_conn(
            &conn,
            &json!({
                "collectionId": a_id,
                "folderId": null,
                "name": "A request",
                "draft": {
                    "method": "GET",
                    "url": "http://127.0.0.1",
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

        let err = send_with_conn(
            &conn,
            &json!({
                "collectionId": a_id,
                "environmentId": b_env_id,
                "requestId": request_id,
                "draft": {
                    "method": "GET",
                    "url": "http://127.0.0.1",
                    "query": [],
                    "headers": [],
                    "bodyType": "none",
                    "body": "",
                    "form": [],
                    "timeoutMs": 100
                }
            }),
        )
        .expect_err("environment must match");
        assert!(err.contains("环境不属于当前集合"));

        let err = send_with_conn(
            &conn,
            &json!({
                "collectionId": b_id,
                "environmentId": b_env_id,
                "requestId": request_id,
                "draft": {
                    "method": "GET",
                    "url": "http://127.0.0.1",
                    "query": [],
                    "headers": [],
                    "bodyType": "none",
                    "body": "",
                    "form": [],
                    "timeoutMs": 100
                }
            }),
        )
        .expect_err("request must match");
        assert!(err.contains("接口不属于当前集合"));
    }

    #[test]
    fn request_draft_defaults_follow_redirects_to_false() {
        let draft: RequestDraft = serde_json::from_value(json!({
            "method": "GET",
            "url": "/",
            "query": [],
            "headers": [],
            "bodyType": "none",
            "body": "",
            "form": [],
            "timeoutMs": 1000
        }))
        .expect("old draft json");
        assert!(!draft.follow_redirects);
    }

    #[test]
    fn send_follows_302_post_as_get_when_enabled() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        let handle = thread::spawn(move || {
            let mut second_request_line = String::new();
            for i in 0..2 {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut buf = [0u8; 2048];
                let n = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).to_string();
                if i == 0 {
                    let _ = stream.write_all(
                        b"HTTP/1.1 302 Found\r\nLocation: /next\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                } else {
                    second_request_line = request.lines().next().unwrap_or("").to_string();
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 4\r\nConnection: close\r\n\r\ndone",
                    );
                }
            }
            second_request_line
        });

        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let environment_id = c["activeEnvironmentId"].as_i64().unwrap();

        let result = send_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "environmentId": environment_id,
                "draft": {
                    "method": "POST",
                    "url": format!("http://127.0.0.1:{port}/redirect"),
                    "query": [],
                    "headers": [],
                    "bodyType": "json",
                    "body": "{\"a\":1}",
                    "form": [],
                    "timeoutMs": 10000,
                    "followRedirects": true
                }
            }),
        )
        .expect("send");

        assert_eq!(result["status"], 200);
        assert_eq!(result["bodyText"], "done");
        assert!(result["finalUrl"].as_str().unwrap().contains("/next"));
        let second_line = handle.join().expect("stub thread");
        assert!(
            second_line.starts_with("GET /next"),
            "302 redirect should convert POST to GET, got: {second_line}"
        );
    }

    #[test]
    fn send_returns_original_307_for_post_even_when_following() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(
                    b"HTTP/1.1 307 Temporary Redirect\r\nLocation: /next\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            }
        });

        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let environment_id = c["activeEnvironmentId"].as_i64().unwrap();

        let result = send_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "environmentId": environment_id,
                "draft": {
                    "method": "POST",
                    "url": format!("http://127.0.0.1:{port}/redirect"),
                    "query": [],
                    "headers": [],
                    "bodyType": "json",
                    "body": "{\"a\":1}",
                    "form": [],
                    "timeoutMs": 10000,
                    "followRedirects": true
                }
            }),
        )
        .expect("send");
        assert_eq!(result["status"], 307);
    }

    #[test]
    fn send_returns_http_302_without_following_redirect() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(
                    b"HTTP/1.1 302 Found\r\nLocation: /next\r\nContent-Length: 8\r\n\r\nredirect",
                );
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
                    "url": "/redirect",
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
        assert_eq!(result["status"], 302);
        assert_eq!(result["ok"], false);
        assert_eq!(result["bodyText"], "redirect");
        assert_eq!(result["responseHeaders"][0]["key"].is_string(), true);
    }

    #[test]
    fn send_ignores_inactive_body_fields_when_resolving_variables() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
            }
        });

        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let environment_id = c["activeEnvironmentId"].as_i64().unwrap();

        let result = send_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "environmentId": environment_id,
                "draft": {
                    "method": "GET",
                    "url": format!("http://127.0.0.1:{port}"),
                    "query": [],
                    "headers": [],
                    "bodyType": "none",
                    "body": "{{MISSING_BODY_VAR}}",
                    "form": [{ "enabled": true, "key": "unused", "value": "{{MISSING_FORM_VAR}}" }],
                    "timeoutMs": 10000
                }
            }),
        )
        .expect("send");
        assert_eq!(result["status"], 200);
    }

    #[test]
    fn send_writes_request_and_executed_snapshots() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 2048];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nok",
                );
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
                "variables": [
                    { "name": "BASE_URL", "value": format!("http://127.0.0.1:{port}"), "isSecret": false },
                    { "name": "TOKEN", "value": "abc", "isSecret": false }
                ]
            }),
        )
        .expect("env");

        send_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "environmentId": environment_id,
                "name": "Login",
                "draft": {
                    "method": "POST",
                    "url": "/login",
                    "query": [{ "enabled": true, "key": "token", "value": "{{TOKEN}}" }],
                    "headers": [{ "enabled": true, "key": "X-Token", "value": "{{TOKEN}}" }],
                    "bodyType": "json",
                    "body": "{\"token\":\"{{TOKEN}}\"}",
                    "form": [{ "enabled": true, "key": "unused", "value": "{{TOKEN}}" }],
                    "timeoutMs": 10000
                }
            }),
        )
        .expect("send");

        let (request_snapshot, executed_snapshot): (String, String) = conn
            .query_row(
                "SELECT request_snapshot_json, executed_request_snapshot_json FROM api_workbench_history ORDER BY id DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("history snapshots");
        let request: Value =
            serde_json::from_str(&request_snapshot).expect("request snapshot json");
        let executed: Value =
            serde_json::from_str(&executed_snapshot).expect("executed snapshot json");

        assert_eq!(request["url"], "/login");
        assert_eq!(request["headers"][0]["value"], "{{TOKEN}}");
        assert_eq!(request["form"][0]["value"], "{{TOKEN}}");
        assert!(executed["finalUrl"]
            .as_str()
            .unwrap()
            .contains("/login?token=abc"));
        assert_eq!(executed["headers"][0]["value"], "abc");
        assert_eq!(executed["body"], "{\"token\":\"abc\"}");
        assert_eq!(executed["form"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn send_caches_binary_response_without_lossy_body_text() {
        use std::fs;
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let body = vec![
            0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0xff, 0x00, 0x80,
        ];
        let expected_body = body.clone();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\n\r\n",
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
                    "url": "/image.png",
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

        assert_eq!(result["bodyStorage"], "file");
        assert_eq!(result["bodyText"], "");
        assert_eq!(result["bodyExtension"], "png");
        let file_path = result["bodyFilePath"].as_str().expect("cache path");
        assert!(file_path.contains("api-workbench"));
        assert_eq!(fs::read(file_path).expect("read cache"), expected_body);
    }

    #[test]
    fn send_does_not_cache_truncated_binary_response() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let body = vec![0x7fu8; MAX_RESPONSE_BODY_BYTES + 8];
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n",
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
                    "url": "/large.bin",
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

        assert_eq!(result["bodyStorage"], "truncated-binary");
        assert_eq!(result["bodyTruncated"], true);
        assert_eq!(result["bodyFilePath"], "");
        assert_eq!(result["bodyHash"], "");
    }

    #[test]
    fn send_writes_binary_cache_reference_to_history() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let body = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 1, 2, 3];
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\n\r\n",
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
                    "url": "/image.png",
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

        let history_id: i64 = conn
            .query_row("SELECT id FROM api_workbench_history", [], |row| row.get(0))
            .expect("history id");
        let detail = history_get_with_conn(&conn, &json!({ "historyId": history_id }))
            .expect("history detail");

        assert_eq!(detail["bodyStorage"], "file");
        assert_eq!(detail["bodyFilePath"], result["bodyFilePath"]);
        assert_eq!(detail["bodyExtension"], "png");
        assert_eq!(detail["bodyHash"], result["bodyHash"]);
    }

    #[test]
    fn insert_history_trims_unpinned_cache_files() {
        use std::fs;

        let conn = test_conn();
        let cache_dir = get_api_workbench_response_cache_dir().expect("cache dir");
        let mut first_path = String::new();

        for idx in 0..(MAX_HISTORY_ROWS + 1) {
            let bytes = format!("cache-{idx}");
            let (file_path, file_name, extension, hash) =
                persist_response_cache_file(&cache_dir, bytes.as_bytes(), None, Some("bin".into()))
                    .expect("cache file");
            if idx == 0 {
                first_path = file_path.clone();
            }
            insert_history_with_conn(
                &conn,
                &HistoryInsert {
                    name: format!("history-{idx}"),
                    method: "GET".into(),
                    url: format!("/{idx}"),
                    final_url: format!("/{idx}"),
                    status: Some(200),
                    ok: true,
                    response_body_storage: "file".into(),
                    response_body_file_path: file_path,
                    response_body_file_name: file_name,
                    response_body_extension: extension,
                    response_body_hash: hash,
                    pinned: false,
                    ..HistoryInsert::default()
                },
            )
            .expect("history");
        }

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM api_workbench_history", [], |row| {
                row.get(0)
            })
            .expect("count");
        assert_eq!(count, MAX_HISTORY_ROWS);
        assert!(fs::metadata(&first_path).is_err());
    }

    #[test]
    fn send_writes_history_and_trims_to_limit() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let environment_id = c["activeEnvironmentId"].as_i64().unwrap();
        insert_history_with_conn(
            &conn,
            &HistoryInsert {
                collection_id: Some(collection_id),
                environment_id: Some(environment_id),
                request_id: None,
                name: "x".into(),
                method: "GET".into(),
                url: "/x".into(),
                final_url: "http://127.0.0.1/x".into(),
                status: Some(200),
                duration_ms: 1,
                ok: true,
                error: None,
                response_content_type: "text/plain".into(),
                response_size: 2,
                response_body_preview: "ok".into(),
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
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM api_workbench_history", [], |row| {
                row.get(0)
            })
            .expect("count");
        assert_eq!(count, 1);
    }

}

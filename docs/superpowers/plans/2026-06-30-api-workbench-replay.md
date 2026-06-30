# Api Workbench Replay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a reproducible API Workbench history flow: every send stores a request draft snapshot and an executed request snapshot, history can be replayed, loaded into a temporary editor state, saved as a full request, searched, pinned, renamed, and cleared without deleting pinned rows by default.

**Architecture:** Keep `apps/desktop/src-tauri/src/tools/api_workbench.rs` as the backend source of truth for request preparation, execution, history persistence, replay, and history-to-request conversion. Add one focused frontend utility module for history-only pure functions, then wire the existing `ApiWorkbenchPanel.vue` to call the new actions and represent temporary history-loaded editor state without replacing saved request identity.

**Tech Stack:** Tauri 2, Rust, rusqlite, ureq, Vue 3, TypeScript, Element Plus, Vitest, pnpm.

## Global Constraints

- Completely offline at runtime; do not add CDN or network dependencies.
- Do not implement multipart file upload, Cookie Jar, redirect following, OpenAPI/Postman/HAR import, scripts, assertions, batch execution, mock server, accounts, sync, permissions, or audit.
- Keep variable scope unchanged: environment variables override global variables; `BASE_URL` only comes from the current environment.
- History replay uses `executed_request_snapshot_json`; it does not parse variables or read current environment tables.
- Loading history opens a temporary request editor state with `selectedRequestId = null` and `sourceHistoryId`; it must not overwrite a saved request identity.
- Existing response body preview limit and truncation strategy remain in place.
- PowerShell is the shell; use `;` in manual command sequences, not `&&`.

---

## File Structure

- Modify `apps/desktop/src-tauri/src/tools/api_workbench.rs`
  - Add history schema columns.
  - Add `ExecutedRequestSnapshot` and snapshot serialization.
  - Split request preparation from HTTP execution.
  - Add `history_get`, `history_replay`, `history_update`, search/filter history list, pinned-aware clear and trim.
  - Update `history_save_request` to use `request_snapshot_json`.
  - Add Rust tests near existing `api_workbench` tests.
- Modify `apps/desktop/src/bridge/tauri.ts`
  - Register `history-get`, `history-replay`, and `history-update` channels.
- Modify `apps/desktop/src/types/api-workbench.ts`
  - Add request snapshot, executed snapshot, history detail, and updated history item types.
- Modify `apps/desktop/src/types/index.ts`
  - Export the new API Workbench history types.
- Create `apps/desktop/src/utils/apiWorkbenchHistory.ts`
  - Keep history replay/load/default-name logic out of the panel.
- Create `apps/desktop/src/utils/apiWorkbenchHistory.test.ts`
  - Cover replay availability, draft construction, degraded old-history loading, and default names.
- Modify `apps/desktop/src/components/ApiWorkbenchPanel.vue`
  - Add history search/filter controls, replay/load/update/clear actions, and temporary editor state.

---

### Task 1: Backend Snapshot Schema And Send Persistence

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/api_workbench.rs`

**Interfaces:**
- Produces:
  - `ExecutedRequestSnapshot`
  - `prepare_api_workbench_request(draft: &RequestDraft, vars: &HashMap<String, String>, base_url: &str) -> Result<ExecutedRequestSnapshot, String>`
  - `execute_api_workbench_request(snapshot: &ExecutedRequestSnapshot) -> Result<Value, String>`
  - `HistoryInsert.request_snapshot_json: Option<String>`
  - `HistoryInsert.executed_request_snapshot_json: Option<String>`
  - `HistoryInsert.replayed_from_history_id: Option<i64>`
- Consumes existing:
  - `resolve_template`, `resolve_rows`, `build_final_url`, `prepare_request_body`, `response_to_json`, `load_variables`

- [ ] **Step 1: Write the failing Rust test for send snapshots**

Add this test in `apps/desktop/src-tauri/src/tools/api_workbench.rs` inside the existing `#[cfg(test)] mod tests` block:

```rust
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
    let request: Value = serde_json::from_str(&request_snapshot).expect("request snapshot json");
    let executed: Value = serde_json::from_str(&executed_snapshot).expect("executed snapshot json");

    assert_eq!(request["url"], "/login");
    assert_eq!(request["headers"][0]["value"], "{{TOKEN}}");
    assert_eq!(request["form"][0]["value"], "{{TOKEN}}");
    assert!(executed["finalUrl"].as_str().unwrap().contains("/login?token=abc"));
    assert_eq!(executed["headers"][0]["value"], "abc");
    assert_eq!(executed["body"], "{\"token\":\"abc\"}");
    assert_eq!(executed["form"].as_array().unwrap().len(), 0);
}
```

- [ ] **Step 2: Run the failing Rust test**

Run:

```powershell
cargo test api_workbench::tests::send_writes_request_and_executed_snapshots -- --nocapture
```

Expected: FAIL because `executed_request_snapshot_json` does not exist or is not written.

- [ ] **Step 3: Add schema columns and snapshot constants**

In `API_WORKBENCH_SCHEMA_SQL`, extend `api_workbench_history`:

```sql
  request_snapshot_json TEXT,
  executed_request_snapshot_json TEXT,
  replayed_from_history_id INTEGER,
  pinned INTEGER NOT NULL DEFAULT 0,
  note TEXT NOT NULL DEFAULT '',
```

Keep existing history columns unchanged. Add near history constants:

```rust
const MAX_HISTORY_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;
const MAX_HISTORY_NOTE_CHARS: usize = 2000;
```

- [ ] **Step 4: Make the test database migrate existing schemas**

In `test_conn()`, after `conn.execute_batch(API_WORKBENCH_SCHEMA_SQL)`, no special action is needed for in-memory new schemas. For production compatibility, add an idempotent helper used by app initialization paths:

```rust
fn ensure_api_workbench_history_columns(conn: &Connection) -> Result<(), String> {
    let columns = [
        ("request_snapshot_json", "TEXT"),
        ("executed_request_snapshot_json", "TEXT"),
        ("replayed_from_history_id", "INTEGER"),
        ("pinned", "INTEGER NOT NULL DEFAULT 0"),
        ("note", "TEXT NOT NULL DEFAULT ''"),
    ];
    for (name, ty) in columns {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('api_workbench_history') WHERE name=?1",
                [name],
                |row| row.get(0),
            )
            .map_err(|e| format!("inspect api history schema failed: {e}"))?;
        if exists == 0 {
            conn.execute(
                &format!("ALTER TABLE api_workbench_history ADD COLUMN {name} {ty}"),
                [],
            )
            .map_err(|e| format!("migrate api history column {name} failed: {e}"))?;
        }
    }
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_api_workbench_history_pinned_created
         ON api_workbench_history(pinned, created_at DESC, id DESC)",
        [],
    )
    .map_err(|e| format!("create api history pinned index failed: {e}"))?;
    Ok(())
}
```

Call it from `execute(action, payload)` after opening the connection and before dispatch:

```rust
ensure_api_workbench_history_columns(&conn)?;
```

- [ ] **Step 5: Add snapshot structs**

Change `RequestDraft` derive to include `Serialize`:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestDraft {
    method: String,
    url: String,
    query: Vec<KeyValueRow>,
    headers: Vec<KeyValueRow>,
    body_type: String,
    body: String,
    form: Vec<KeyValueRow>,
    timeout_ms: u64,
}
```

Add:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecutedRequestSnapshot {
    method: String,
    final_url: String,
    headers: Vec<KeyValueRow>,
    body_type: String,
    body: String,
    form: Vec<KeyValueRow>,
    timeout_ms: u64,
}
```

- [ ] **Step 6: Split preparation from execution**

Add:

```rust
fn prepare_api_workbench_request(
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
    })
}
```

Add an execution wrapper that reuses the existing `execute_http_request`:

```rust
fn execute_api_workbench_request(snapshot: &ExecutedRequestSnapshot) -> Result<Value, String> {
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
    };
    execute_http_request(
        &draft_for_timeout,
        &snapshot.final_url,
        &snapshot.headers,
        prepared,
    )
}
```

- [ ] **Step 7: Update `send_with_conn` to save both snapshots**

Replace the manual resolution block in `send_with_conn` with:

```rust
let (vars, base_url) = load_variables(conn, environment_id)?;
let executed_snapshot = prepare_api_workbench_request(&draft, &vars, &base_url)?;
let result = execute_api_workbench_request(&executed_snapshot)?;
let request_snapshot_json = serialize_limited_json(&draft, MAX_HISTORY_SNAPSHOT_BYTES, "请求快照体积超过限制")?;
let executed_snapshot_json = serialize_limited_json(
    &executed_snapshot,
    MAX_HISTORY_SNAPSHOT_BYTES,
    "执行快照体积超过限制",
)?;
```

Add helper:

```rust
fn serialize_limited_json<T: Serialize>(
    value: &T,
    max_bytes: usize,
    message: &str,
) -> Result<String, String> {
    let serialized = serde_json::to_string(value).map_err(|e| format!("serialize snapshot failed: {e}"))?;
    if serialized.len() > max_bytes {
        return Err(message.to_string());
    }
    Ok(serialized)
}
```

Pass the two JSON strings into `HistoryInsert`.

- [ ] **Step 8: Extend `HistoryInsert` and insert SQL**

Add fields:

```rust
request_snapshot_json: Option<String>,
executed_request_snapshot_json: Option<String>,
replayed_from_history_id: Option<i64>,
pinned: bool,
note: String,
```

Update `insert_history_with_conn` insert SQL to include:

```sql
request_snapshot_json, executed_request_snapshot_json, replayed_from_history_id, pinned, note
```

and values:

```rust
item.request_snapshot_json,
item.executed_request_snapshot_json,
item.replayed_from_history_id,
if item.pinned { 1 } else { 0 },
item.note,
```

- [ ] **Step 9: Make history trim pinned-aware**

Replace current trim SQL with:

```rust
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
```

- [ ] **Step 10: Update existing tests constructing `HistoryInsert`**

Every test literal must provide:

```rust
request_snapshot_json: None,
executed_request_snapshot_json: None,
replayed_from_history_id: None,
pinned: false,
note: String::new(),
```

- [ ] **Step 11: Run backend tests for Task 1**

Run:

```powershell
cargo test api_workbench::tests::send_writes_request_and_executed_snapshots -- --nocapture
cargo test api_workbench::tests::send_writes_history_and_trims_to_limit -- --nocapture
```

Expected: both PASS.

- [ ] **Step 12: Commit Task 1**

```powershell
git add apps/desktop/src-tauri/src/tools/api_workbench.rs
git commit -m "feat(api-workbench): 保存历史请求和执行快照"
```

---

### Task 2: Backend History Get, Replay, And Save-As-Request

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/api_workbench.rs`

**Interfaces:**
- Consumes:
  - `ExecutedRequestSnapshot`
  - `execute_api_workbench_request(&ExecutedRequestSnapshot) -> Result<Value, String>`
  - `request_snapshot_json`
  - `executed_request_snapshot_json`
- Produces:
  - `history_get_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String>`
  - `history_replay_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String>`
  - updated `history_save_request_with_conn`

- [ ] **Step 1: Write failing tests for history get, replay, and save-as-request**

Add:

```rust
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
        },
    )
    .expect("history");
    let id: i64 = conn.query_row("SELECT id FROM api_workbench_history", [], |row| row.get(0)).unwrap();

    let detail = history_get_with_conn(&conn, &json!({ "historyId": id })).expect("detail");
    assert_eq!(detail["requestSnapshot"]["headers"][0]["key"], "X-A");
    assert_eq!(detail["hasRequestSnapshot"], true);
    assert_eq!(detail["hasExecutedRequestSnapshot"], false);
}
```

Add replay test:

```rust
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
        },
    )
    .expect("history");
    let id: i64 = conn.query_row("SELECT id FROM api_workbench_history", [], |row| row.get(0)).unwrap();

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
```

Add save-as-request snapshot test:

```rust
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
        },
    )
    .expect("history");
    let history_id: i64 = conn.query_row("SELECT id FROM api_workbench_history", [], |row| row.get(0)).unwrap();

    let saved = history_save_request_with_conn(
        &conn,
        &json!({ "historyId": history_id, "collectionId": collection_id, "folderId": null, "name": "Saved" }),
    )
    .expect("save");
    let detail = request_get_with_conn(&conn, &json!({ "id": saved["id"].as_i64().unwrap() })).expect("detail");
    assert_eq!(detail["draft"]["method"], "PATCH");
    assert_eq!(detail["draft"]["headers"][0]["value"], "{{TOKEN}}");
    assert_eq!(detail["draft"]["timeoutMs"], 12000);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
cargo test api_workbench::tests::history_get_returns_request_snapshot_for_loading -- --nocapture
cargo test api_workbench::tests::history_replay_uses_executed_snapshot_without_environment -- --nocapture
cargo test api_workbench::tests::history_save_request_uses_request_snapshot_when_available -- --nocapture
```

Expected: FAIL because new functions/actions do not exist or do not use snapshots.

- [ ] **Step 3: Add `history_get_with_conn`**

Implement:

```rust
fn history_row_json(
    row: &rusqlite::Row<'_>,
    include_request_snapshot: bool,
) -> rusqlite::Result<Value> {
    let request_snapshot_json: Option<String> = row.get(17)?;
    let executed_snapshot_json: Option<String> = row.get(18)?;
    let mut value = json!({
        "id": row.get::<_, i64>(0)?,
        "collectionId": row.get::<_, Option<i64>>(1)?,
        "environmentId": row.get::<_, Option<i64>>(2)?,
        "requestId": row.get::<_, Option<i64>>(3)?,
        "replayedFromHistoryId": row.get::<_, Option<i64>>(19)?,
        "name": row.get::<_, String>(4)?,
        "note": row.get::<_, String>(20)?,
        "pinned": row.get::<_, i64>(21)? == 1,
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
        "createdAt": row.get::<_, String>(16)?,
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
```

Use a fixed SELECT column order in both list and get. For parse errors in `history_get`, return `"历史快照已损坏"` instead of `Value::Null`.

- [ ] **Step 4: Add `history_replay_with_conn`**

Implement:

```rust
fn history_replay_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
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
            response_content_type: result["contentType"].as_str().unwrap_or_default().to_string(),
            response_size: result["bodySize"].as_u64().unwrap_or(0) as usize,
            response_body_preview: result["bodyText"].as_str().unwrap_or_default().to_string(),
            response_body_truncated: result["bodyTruncated"].as_bool().unwrap_or(false),
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
```

Change `insert_history_with_conn` return type from `Result<(), String>` to `Result<i64, String>` and return `conn.last_insert_rowid()`.

- [ ] **Step 5: Update `history_save_request_with_conn`**

When loading the history row, select `request_snapshot_json` too:

```sql
SELECT method, url, final_url, status, duration_ms, created_at, request_snapshot_json
FROM api_workbench_history WHERE id=?1
```

If present, parse to `RequestDraft` and insert request fields from snapshot:

```rust
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
```

Insert `query_json`, `headers_json`, `body_type`, `body_text`, `form_json`, and `timeout_ms` from `draft`.

- [ ] **Step 6: Register backend actions**

In `execute(action, payload)` add:

```rust
"history_get" => history_get_with_conn(&conn, payload),
"history_replay" => history_replay_with_conn(&conn, payload),
```

- [ ] **Step 7: Run Task 2 backend tests**

Run:

```powershell
cargo test api_workbench::tests::history_get_returns_request_snapshot_for_loading -- --nocapture
cargo test api_workbench::tests::history_replay_uses_executed_snapshot_without_environment -- --nocapture
cargo test api_workbench::tests::history_save_request_uses_request_snapshot_when_available -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit Task 2**

```powershell
git add apps/desktop/src-tauri/src/tools/api_workbench.rs
git commit -m "feat(api-workbench): 支持历史详情重放和完整保存"
```

---

### Task 3: Backend History Search, Pin, Rename, Note, And Clear

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/api_workbench.rs`

**Interfaces:**
- Produces:
  - `history_update_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String>`
  - `history_list_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String>`
  - `history_clear_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String>`

- [ ] **Step 1: Write failing tests**

Add:

```rust
#[test]
fn history_update_allows_empty_name_and_validates_note_length() {
    let conn = test_conn();
    insert_history_with_conn(&conn, &HistoryInsert {
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
    }).expect("history");
    let id: i64 = conn.query_row("SELECT id FROM api_workbench_history", [], |row| row.get(0)).unwrap();

    history_update_with_conn(&conn, &json!({ "id": id, "name": "", "note": "keep", "pinned": true })).expect("update");
    let (name, note, pinned): (String, String, i64) = conn
        .query_row("SELECT name, note, pinned FROM api_workbench_history WHERE id=?1", [id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .unwrap();
    assert_eq!(name, "");
    assert_eq!(note, "keep");
    assert_eq!(pinned, 1);

    let long_note = "x".repeat(MAX_HISTORY_NOTE_CHARS + 1);
    let err = history_update_with_conn(&conn, &json!({ "id": id, "name": "", "note": long_note, "pinned": true }))
        .expect_err("long note");
    assert!(err.contains("备注"));
}
```

Add:

```rust
#[test]
fn history_clear_preserves_pinned_by_default() {
    let conn = test_conn();
    for (name, pinned) in [("keep", true), ("drop", false)] {
        insert_history_with_conn(&conn, &HistoryInsert {
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
        }).expect("history");
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
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM api_workbench_history", [], |row| row.get(0)).unwrap();
    assert_eq!(count, 0);
}
```

Add:

```rust
#[test]
fn history_list_filters_search_and_pinned() {
    let conn = test_conn();
    insert_history_with_conn(&conn, &HistoryInsert {
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
    }).expect("history");
    insert_history_with_conn(&conn, &HistoryInsert {
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
    }).expect("history");

    let pinned = history_list_with_conn(&conn, &json!({ "query": "token", "pinnedOnly": true, "limit": 200 })).expect("list");
    assert_eq!(pinned["items"].as_array().unwrap().len(), 1);
    assert_eq!(pinned["items"][0]["name"], "Login ok");
    assert_eq!(pinned["items"][0]["hasRequestSnapshot"], true);
    assert_eq!(pinned["items"][0]["hasExecutedRequestSnapshot"], true);
}
```

- [ ] **Step 2: Run failing tests**

Run:

```powershell
cargo test api_workbench::tests::history_update_allows_empty_name_and_validates_note_length -- --nocapture
cargo test api_workbench::tests::history_clear_preserves_pinned_by_default -- --nocapture
cargo test api_workbench::tests::history_list_filters_search_and_pinned -- --nocapture
```

Expected: FAIL because actions are not implemented.

- [ ] **Step 3: Update `history_list_with_conn` signature and query**

Change signature to:

```rust
fn history_list_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String>
```

Parse:

```rust
let query = payload["query"].as_str().unwrap_or_default().trim().to_string();
let pinned_only = payload["pinnedOnly"].as_bool().unwrap_or(false);
let limit = payload["limit"].as_i64().unwrap_or(MAX_HISTORY_ROWS).clamp(1, MAX_HISTORY_ROWS);
```

Use SQL with parameterized `LIKE`:

```rust
let pattern = format!("%{}%", query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
let sql = "
SELECT id, collection_id, environment_id, request_id, name, method, url, final_url,
       status, duration_ms, ok, error, response_content_type, response_size,
       response_body_preview, response_body_truncated, created_at,
       request_snapshot_json, executed_request_snapshot_json, replayed_from_history_id,
       note, pinned
FROM api_workbench_history
WHERE (?1 = 0 OR pinned = 1)
  AND (
    ?2 = ''
    OR name LIKE ?3 ESCAPE '\\'
    OR note LIKE ?3 ESCAPE '\\'
    OR method LIKE ?3 ESCAPE '\\'
    OR url LIKE ?3 ESCAPE '\\'
    OR final_url LIKE ?3 ESCAPE '\\'
    OR CAST(status AS TEXT) LIKE ?3 ESCAPE '\\'
    OR COALESCE(error, '') LIKE ?3 ESCAPE '\\'
    OR response_content_type LIKE ?3 ESCAPE '\\'
  )
ORDER BY created_at DESC, id DESC
LIMIT ?4";
```

- [ ] **Step 4: Add `history_update_with_conn`**

Implement:

```rust
fn history_update_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    let name = payload["name"].as_str().unwrap_or_default().trim().to_string();
    let note = payload["note"].as_str().unwrap_or_default().trim().to_string();
    if note.chars().count() > MAX_HISTORY_NOTE_CHARS {
        return Err("历史备注超过 2000 字符".to_string());
    }
    let pinned = payload["pinned"].as_bool().ok_or("pinned must be a boolean")?;
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
```

- [ ] **Step 5: Update `history_clear_with_conn`**

Change signature:

```rust
fn history_clear_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String>
```

Implement:

```rust
let include_pinned = payload["includePinned"].as_bool().unwrap_or(false);
let sql = if include_pinned {
    "DELETE FROM api_workbench_history"
} else {
    "DELETE FROM api_workbench_history WHERE pinned=0"
};
conn.execute(sql, [])
    .map_err(|e| format!("clear history failed: {e}"))?;
Ok(json!({ "ok": true }))
```

- [ ] **Step 6: Register dispatch changes**

Update action dispatch:

```rust
"history_list" => history_list_with_conn(&conn, payload),
"history_clear" => history_clear_with_conn(&conn, payload),
"history_update" => history_update_with_conn(&conn, payload),
```

Update `action_list_with_conn` call to:

```rust
let history = history_list_with_conn(conn, &json!({}))?["items"].clone();
```

- [ ] **Step 7: Run Task 3 tests**

Run:

```powershell
cargo test api_workbench::tests::history_update_allows_empty_name_and_validates_note_length -- --nocapture
cargo test api_workbench::tests::history_clear_preserves_pinned_by_default -- --nocapture
cargo test api_workbench::tests::history_list_filters_search_and_pinned -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit Task 3**

```powershell
git add apps/desktop/src-tauri/src/tools/api_workbench.rs
git commit -m "feat(api-workbench): 增强历史整理能力"
```

---

### Task 4: Frontend Types, Bridge Channels, And History Pure Functions

**Files:**
- Modify: `apps/desktop/src/bridge/tauri.ts`
- Modify: `apps/desktop/src/types/api-workbench.ts`
- Modify: `apps/desktop/src/types/index.ts`
- Create: `apps/desktop/src/utils/apiWorkbenchHistory.ts`
- Create: `apps/desktop/src/utils/apiWorkbenchHistory.test.ts`

**Interfaces:**
- Consumes backend response shapes:
  - `ApiWorkbenchHistoryItem.hasRequestSnapshot`
  - `ApiWorkbenchHistoryItem.hasExecutedRequestSnapshot`
  - `ApiWorkbenchHistoryDetail.requestSnapshot`
- Produces:
  - `canReplayApiWorkbenchHistory(item: ApiWorkbenchHistoryItem): boolean`
  - `buildApiWorkbenchDraftFromHistory(item: ApiWorkbenchHistoryDetail): { draft: ApiWorkbenchRequestDraft; degraded: boolean }`
  - `defaultApiWorkbenchHistoryDisplayName(item: ApiWorkbenchHistoryItem): string`

- [ ] **Step 1: Write failing frontend tests**

Create `apps/desktop/src/utils/apiWorkbenchHistory.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { ApiWorkbenchHistoryDetail, ApiWorkbenchHistoryItem } from "../types/api-workbench";
import {
  buildApiWorkbenchDraftFromHistory,
  canReplayApiWorkbenchHistory,
  defaultApiWorkbenchHistoryDisplayName,
} from "./apiWorkbenchHistory";

function history(overrides: Partial<ApiWorkbenchHistoryItem> = {}): ApiWorkbenchHistoryItem {
  return {
    id: 1,
    collectionId: null,
    environmentId: null,
    requestId: null,
    replayedFromHistoryId: null,
    name: "",
    note: "",
    pinned: false,
    method: "POST",
    url: "/api/login",
    finalUrl: "http://127.0.0.1:8080/api/login?debug=1",
    status: 200,
    durationMs: 12,
    ok: true,
    error: null,
    contentType: "application/json",
    bodySize: 2,
    bodyPreview: "{}",
    bodyTruncated: false,
    hasRequestSnapshot: false,
    hasExecutedRequestSnapshot: false,
    createdAt: "2026-06-30 10:00:00",
    ...overrides,
  };
}

describe("apiWorkbenchHistory utils", () => {
  it("allows replay only when executed snapshot exists", () => {
    expect(canReplayApiWorkbenchHistory(history({ hasExecutedRequestSnapshot: true }))).toBe(true);
    expect(canReplayApiWorkbenchHistory(history({ hasExecutedRequestSnapshot: false }))).toBe(false);
  });

  it("builds draft from request snapshot", () => {
    const detail: ApiWorkbenchHistoryDetail = {
      ...history({ hasRequestSnapshot: true }),
      requestSnapshot: {
        method: "PATCH",
        url: "/users/1",
        query: [{ enabled: true, key: "expand", value: "roles" }],
        headers: [{ enabled: true, key: "X-Token", value: "{{TOKEN}}" }],
        bodyType: "json",
        body: "{\"name\":\"demo\"}",
        form: [],
        timeoutMs: 12000,
      },
    };
    const result = buildApiWorkbenchDraftFromHistory(detail);
    expect(result.degraded).toBe(false);
    expect(result.draft.method).toBe("PATCH");
    expect(result.draft.headers[0].value).toBe("{{TOKEN}}");
    expect(result.draft.timeoutMs).toBe(12000);
  });

  it("degrades old history to method and url", () => {
    const detail: ApiWorkbenchHistoryDetail = { ...history(), requestSnapshot: null };
    const result = buildApiWorkbenchDraftFromHistory(detail);
    expect(result.degraded).toBe(true);
    expect(result.draft).toMatchObject({ method: "POST", url: "/api/login", bodyType: "none" });
    expect(result.draft.headers).toEqual([]);
    expect(result.draft.query).toEqual([]);
  });

  it("builds stable default display names", () => {
    expect(defaultApiWorkbenchHistoryDisplayName(history({ name: "  Login  " }))).toBe("Login");
    expect(defaultApiWorkbenchHistoryDisplayName(history({ name: "", url: "http://x.test/api/users?debug=1", method: "GET" }))).toBe("GET /api/users");
    expect(defaultApiWorkbenchHistoryDisplayName(history({ name: "", url: "not a url", method: "DELETE" }))).toBe("DELETE not a url");
  });
});
```

- [ ] **Step 2: Run failing frontend tests**

Run:

```powershell
pnpm test src/utils/apiWorkbenchHistory.test.ts
```

Expected: FAIL because module and types are missing.

- [ ] **Step 3: Update types**

In `apps/desktop/src/types/api-workbench.ts`, add:

```ts
export interface ApiWorkbenchHistoryRequestSnapshot extends ApiWorkbenchRequestDraft {}

export interface ApiWorkbenchExecutedRequestSnapshot {
  method: ApiWorkbenchMethod;
  finalUrl: string;
  headers: ApiWorkbenchKeyValueRow[];
  bodyType: ApiWorkbenchBodyType;
  body: string;
  form: ApiWorkbenchKeyValueRow[];
  timeoutMs: number;
}
```

Replace `ApiWorkbenchHistoryItem` with fields from the spec:

```ts
export interface ApiWorkbenchHistoryItem {
  id: number;
  collectionId: number | null;
  environmentId: number | null;
  requestId: number | null;
  replayedFromHistoryId: number | null;
  name: string;
  note: string;
  pinned: boolean;
  method: ApiWorkbenchMethod;
  url: string;
  finalUrl: string;
  status: number | null;
  durationMs: number;
  ok: boolean;
  error: string | null;
  contentType: string;
  bodySize: number;
  bodyPreview: string;
  bodyTruncated: boolean;
  hasRequestSnapshot: boolean;
  hasExecutedRequestSnapshot: boolean;
  createdAt: string;
}

export interface ApiWorkbenchHistoryDetail extends ApiWorkbenchHistoryItem {
  requestSnapshot: ApiWorkbenchHistoryRequestSnapshot | null;
}
```

In `apps/desktop/src/types/index.ts`, export the new type names.

- [ ] **Step 4: Register bridge channels**

In `apps/desktop/src/bridge/tauri.ts`, add:

```ts
"tool:api-workbench:history-get": { domain: "api_workbench", action: "history_get" },
"tool:api-workbench:history-replay": { domain: "api_workbench", action: "history_replay" },
"tool:api-workbench:history-update": { domain: "api_workbench", action: "history_update" },
```

- [ ] **Step 5: Implement pure functions**

Create `apps/desktop/src/utils/apiWorkbenchHistory.ts`:

```ts
import type {
  ApiWorkbenchHistoryDetail,
  ApiWorkbenchHistoryItem,
  ApiWorkbenchRequestDraft,
} from "../types/api-workbench";
import { normalizeApiWorkbenchDraft } from "./apiWorkbench";

export function canReplayApiWorkbenchHistory(item: ApiWorkbenchHistoryItem): boolean {
  return item.hasExecutedRequestSnapshot;
}

export function buildApiWorkbenchDraftFromHistory(
  item: ApiWorkbenchHistoryDetail,
): { draft: ApiWorkbenchRequestDraft; degraded: boolean } {
  if (item.requestSnapshot) {
    return {
      draft: normalizeApiWorkbenchDraft(item.requestSnapshot),
      degraded: false,
    };
  }
  return {
    draft: normalizeApiWorkbenchDraft({
      method: item.method,
      url: item.url,
      query: [],
      headers: [],
      bodyType: "none",
      body: "",
      form: [],
      timeoutMs: 10000,
    }),
    degraded: true,
  };
}

export function defaultApiWorkbenchHistoryDisplayName(item: ApiWorkbenchHistoryItem): string {
  const explicit = item.name.trim();
  if (explicit) return explicit;
  const raw = item.url.trim() || item.finalUrl.trim();
  try {
    const parsed = new URL(raw);
    return `${item.method} ${parsed.pathname || "/"}`;
  } catch {
    return `${item.method} ${raw || item.finalUrl || item.url}`.trim();
  }
}
```

- [ ] **Step 6: Run frontend tests**

Run:

```powershell
pnpm test src/utils/apiWorkbenchHistory.test.ts
```

Expected: PASS.

- [ ] **Step 7: Commit Task 4**

```powershell
git add apps/desktop/src/bridge/tauri.ts apps/desktop/src/types/api-workbench.ts apps/desktop/src/types/index.ts apps/desktop/src/utils/apiWorkbenchHistory.ts apps/desktop/src/utils/apiWorkbenchHistory.test.ts
git commit -m "feat(api-workbench): 添加历史前端类型和纯函数"
```

---

### Task 5: Wire ApiWorkbenchPanel History Replay And Temporary Editor

**Files:**
- Modify: `apps/desktop/src/components/ApiWorkbenchPanel.vue`

**Interfaces:**
- Consumes:
  - `history_get`, `history_replay`, `history_update`, `history_clear`, `history_list`
  - `canReplayApiWorkbenchHistory`
  - `buildApiWorkbenchDraftFromHistory`
  - `defaultApiWorkbenchHistoryDisplayName`
- Produces UI behavior:
  - History search input.
  - All/pinned segmented filter.
  - Replay button disabled for old history.
  - Load opens temporary editor state.
  - Save-as-request uses default history name.
  - Clear defaults to non-pinned only.

- [ ] **Step 1: Import new helpers and types**

In `ApiWorkbenchPanel.vue`, extend imports:

```ts
import type {
  ApiWorkbenchHistoryDetail,
  ApiWorkbenchHistoryItem,
} from "../types/api-workbench";
import {
  buildApiWorkbenchDraftFromHistory,
  canReplayApiWorkbenchHistory,
  defaultApiWorkbenchHistoryDisplayName,
} from "../utils/apiWorkbenchHistory";
```

Keep existing type imports and remove the old local `historyDefaultName` after callers are replaced.

- [ ] **Step 2: Add history state**

Near existing refs:

```ts
const sourceHistoryId = ref<number | null>(null);
const historyQuery = ref("");
const historyPinnedOnly = ref(false);
const historyLoading = ref(false);
const replayingHistoryId = ref<number | null>(null);
```

- [ ] **Step 3: Add `loadHistory` function**

Replace repeated history-list calls with:

```ts
async function loadHistory() {
  historyLoading.value = true;
  try {
    const result = (await invokeToolByChannel("tool:api-workbench:history-list", {
      query: historyQuery.value,
      pinnedOnly: historyPinnedOnly.value,
      limit: 200,
    })) as { items: ApiWorkbenchHistoryItem[] };
    history.value = result.items ?? [];
  } finally {
    historyLoading.value = false;
  }
}
```

In `loadAll`, keep assigning `history.value = result.history ?? []`. In `sendRequest`, call `await loadHistory()` after send.

- [ ] **Step 4: Wire temporary editor load**

Replace `reuseHistory` with async load:

```ts
async function loadHistoryIntoTemporaryEditor(item: ApiWorkbenchHistoryItem) {
  const detail = (await invokeToolByChannel("tool:api-workbench:history-get", {
    historyId: item.id,
  })) as ApiWorkbenchHistoryDetail;
  const { draft: nextDraft, degraded } = buildApiWorkbenchDraftFromHistory(detail);
  if (sourceHistoryId.value !== null) {
    await ElMessageBox.confirm("当前临时接口草稿会被历史记录覆盖，是否继续？", "载入历史", {
      type: "warning",
    });
  }
  selectedRequestId.value = null;
  selectedRequestFolderId.value = detail.collectionId === selectedCollectionId.value ? selectedRequestFolderId.value : null;
  sourceHistoryId.value = detail.id;
  requestName.value = defaultApiWorkbenchHistoryDisplayName(detail);
  requestDescription.value = "";
  draft.value = nextDraft;
  response.value = null;
  responseTab.value = "response";
  if (degraded) {
    ElMessage.warning("旧历史仅包含摘要，已恢复 Method 和 URL");
  }
}
```

Whenever a saved request is loaded or a new request is created, reset `sourceHistoryId.value = null`.

- [ ] **Step 5: Wire replay**

Add:

```ts
async function replayHistory(item: ApiWorkbenchHistoryItem) {
  if (!canReplayApiWorkbenchHistory(item)) {
    ElMessage.warning("旧历史缺少执行快照，请载入后手动发送");
    return;
  }
  replayingHistoryId.value = item.id;
  try {
    response.value = (await invokeToolByChannel("tool:api-workbench:history-replay", {
      historyId: item.id,
    })) as ApiWorkbenchSendResult;
    responseBodyMode.value = "pretty";
    responseTab.value = "response";
    await loadHistory();
  } finally {
    replayingHistoryId.value = null;
  }
}
```

- [ ] **Step 6: Wire pin/update**

Add:

```ts
async function toggleHistoryPinned(item: ApiWorkbenchHistoryItem) {
  await invokeToolByChannel("tool:api-workbench:history-update", {
    id: item.id,
    name: item.name,
    note: item.note,
    pinned: !item.pinned,
  });
  await loadHistory();
}

async function editHistoryMeta(item: ApiWorkbenchHistoryItem) {
  const nameResult = await ElMessageBox.prompt("历史名称可留空，留空时按 Method 和路径展示", "编辑历史名称", {
    inputValue: item.name,
    inputPlaceholder: defaultApiWorkbenchHistoryDisplayName(item),
  });
  const noteResult = await ElMessageBox.prompt("备注最多 2000 字", "编辑历史备注", {
    inputValue: item.note,
    inputType: "textarea",
  });
  await invokeToolByChannel("tool:api-workbench:history-update", {
    id: item.id,
    name: String(nameResult.value ?? ""),
    note: String(noteResult.value ?? ""),
    pinned: item.pinned,
  });
  await loadHistory();
}
```

- [ ] **Step 7: Wire clear history**

Add:

```ts
async function clearHistory() {
  const includePinned = await ElMessageBox.confirm(
    "默认只清空非标星历史。是否同时清空标星历史？",
    "清空历史",
    {
      confirmButtonText: "清空全部",
      cancelButtonText: "仅清空非标星",
      distinguishCancelAndClose: true,
      type: "warning",
    },
  )
    .then(() => true)
    .catch((action) => {
      if (action === "cancel") return false;
      throw action;
    });
  await invokeToolByChannel("tool:api-workbench:history-clear", { includePinned });
  await loadHistory();
}
```

- [ ] **Step 8: Update history tab template**

Replace the history tab inner content with:

```vue
<div class="history-toolbar">
  <el-input
    v-model="historyQuery"
    size="small"
    clearable
    placeholder="搜索历史"
    @keyup.enter="loadHistory"
    @clear="loadHistory"
  />
  <el-radio-group v-model="historyPinnedOnly" size="small" @change="loadHistory">
    <el-radio-button :label="false">全部</el-radio-button>
    <el-radio-button :label="true">标星</el-radio-button>
  </el-radio-group>
  <el-button size="small" :disabled="!history.length" @click="clearHistory">清理</el-button>
</div>
<div
  v-for="item in history"
  :key="item.id"
  class="history-item"
>
  <div class="history-main" @click="loadHistoryIntoTemporaryEditor(item)">
    <strong>{{ item.method }}</strong>
    <span>{{ defaultApiWorkbenchHistoryDisplayName(item) }}</span>
    <small>{{ item.status ?? "ERR" }} · {{ item.durationMs }}ms · {{ item.hasRequestSnapshot ? "完整快照" : "摘要历史" }}</small>
  </div>
  <el-button size="small" text @click.stop="toggleHistoryPinned(item)">
    {{ item.pinned ? "取消标星" : "标星" }}
  </el-button>
  <el-button
    size="small"
    :loading="replayingHistoryId === item.id"
    :disabled="!canReplayApiWorkbenchHistory(item)"
    @click.stop="replayHistory(item)"
  >
    重放
  </el-button>
  <el-button size="small" @click.stop="loadHistoryIntoTemporaryEditor(item)">载入</el-button>
  <el-button size="small" @click.stop="saveHistoryAsRequest(item)">保存为接口</el-button>
  <el-button size="small" @click.stop="editHistoryMeta(item)">备注</el-button>
</div>
```

- [ ] **Step 9: Update save-as-request name**

Replace:

```ts
name: historyDefaultName(item),
```

with:

```ts
name: defaultApiWorkbenchHistoryDisplayName(item),
```

- [ ] **Step 10: Run typecheck for panel wiring**

Run:

```powershell
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 11: Commit Task 5**

```powershell
git add apps/desktop/src/components/ApiWorkbenchPanel.vue
git commit -m "feat(api-workbench): 接入历史重放和临时编辑器"
```

---

### Task 6: Full Verification And Documentation Check

**Files:**
- Modify only if verification exposes small fixes:
  - `apps/desktop/src-tauri/src/tools/api_workbench.rs`
  - `apps/desktop/src/components/ApiWorkbenchPanel.vue`
  - `apps/desktop/src/utils/apiWorkbenchHistory.ts`
  - `apps/desktop/src/types/api-workbench.ts`
  - `apps/desktop/src/bridge/tauri.ts`

**Interfaces:**
- Consumes all previous tasks.
- Produces a verified implementation ready for review.

- [ ] **Step 1: Run full backend API Workbench tests**

Run:

```powershell
cargo test api_workbench -- --nocapture
```

Expected: PASS. If a failure appears in an unrelated module, capture the exact failure and run the narrower failing `api_workbench` test again before changing code.

- [ ] **Step 2: Run frontend unit tests**

Run:

```powershell
pnpm test src/utils/apiWorkbench.test.ts src/utils/apiWorkbenchTree.test.ts src/utils/apiWorkbenchHistory.test.ts
```

Expected: PASS.

- [ ] **Step 3: Run workspace typecheck**

Run:

```powershell
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 4: Run renderer build**

Run:

```powershell
pnpm --filter @lazycat/desktop build:web
```

Expected: PASS. If `spawn EPERM` occurs, retry once; if it still fails, rerun with escalation per repository build guidance.

- [ ] **Step 5: Inspect diff for unintended changes**

Run:

```powershell
git diff --stat
git diff -- apps/desktop/src-tauri/src/tools/api_workbench.rs apps/desktop/src/components/ApiWorkbenchPanel.vue apps/desktop/src/utils/apiWorkbenchHistory.ts apps/desktop/src/types/api-workbench.ts apps/desktop/src/types/index.ts apps/desktop/src/bridge/tauri.ts
```

Expected:
- Only API Workbench replay/history changes are present.
- No CDN/runtime network dependencies were added.
- No unrelated UI or data dictionary files changed.

- [ ] **Step 6: Commit verification fixes if needed**

If Step 1-5 required any fixes:

```powershell
git add apps/desktop/src-tauri/src/tools/api_workbench.rs apps/desktop/src/components/ApiWorkbenchPanel.vue apps/desktop/src/utils/apiWorkbenchHistory.ts apps/desktop/src/utils/apiWorkbenchHistory.test.ts apps/desktop/src/types/api-workbench.ts apps/desktop/src/types/index.ts apps/desktop/src/bridge/tauri.ts
git commit -m "fix(api-workbench): 完成历史复现验证修正"
```

If no fixes were needed, do not create an empty commit.

---

## Self-Review

**Spec coverage:**
- Request snapshot persistence: Task 1.
- Executed snapshot persistence and replay: Task 1 and Task 2.
- Temporary editor load: Task 4 and Task 5.
- Save history as full request: Task 2.
- Pin, rename, note, search, pinned clear: Task 3 and Task 5.
- Old history degraded behavior: Task 2, Task 4, and Task 5.
- Validation commands: Task 6.

**Placeholder scan:** No placeholder markers or unnamed validation steps remain.

**Type consistency:** The plan consistently uses `ApiWorkbenchHistoryItem`, `ApiWorkbenchHistoryDetail`, `ApiWorkbenchHistoryRequestSnapshot`, `ApiWorkbenchExecutedRequestSnapshot`, `hasRequestSnapshot`, `hasExecutedRequestSnapshot`, `history_get`, `history_replay`, and `history_update`.

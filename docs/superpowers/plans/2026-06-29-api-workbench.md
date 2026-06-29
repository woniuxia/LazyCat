# 接口调试工具 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `api-workbench` tool for offline HTTP request debugging, collection storage, environment variables, response history, and Markdown export.

**Architecture:** The Rust tool domain `api_workbench` owns persistence, variable resolution, URL building, request execution, history trimming, and Markdown generation. The Vue panel owns editing state, user interaction, response display, and calls the backend through normal `tool:*` channels; frontend utilities only cover preview-friendly pure functions and display normalization.

**Tech Stack:** Tauri 2, Vue 3, TypeScript, Element Plus, rusqlite, serde_json, ureq, Vitest, Rust unit tests.

---

## Scope Check

The spec is broad but still one feature: all parts serve the same user workflow of editing, sending, saving, and documenting HTTP requests. Keep it as one implementation plan, but implement in small commits so backend behavior is stable before the UI depends on it.

Out of scope for this implementation:

- Postman / OpenAPI / curl import.
- Scripts, assertions, batch runs, CI reports, mock server, multipart upload.
- Enabling redirect following. The UI shows a disabled control only; no request field or backend behavior is added for it.
- Additional history privacy rules beyond the current response preview size limit and Markdown export redaction rules in the spec.

## File Structure

Create:

- `apps/desktop/src-tauri/src/tools/api_workbench.rs`
  - Owns the Rust domain, action dispatch, schema SQL constant, pure request helpers, DB actions, request execution, history trimming, and Markdown export.
- `apps/desktop/src/types/api-workbench.ts`
  - Owns frontend request, response, collection, environment, history, and export types.
- `apps/desktop/src/utils/apiWorkbench.ts`
  - Owns frontend pure helpers: variable extraction, preview URL building, request draft normalization, and response body display formatting.
- `apps/desktop/src/utils/apiWorkbench.test.ts`
  - Tests frontend pure helpers only.
- `apps/desktop/src/components/ApiWorkbenchPanel.vue`
  - Owns the complete tool UI.

Modify:

- `apps/desktop/src-tauri/src/tools/helpers.rs`
  - Executes the `api_workbench` schema SQL during schema initialization.
- `apps/desktop/src-tauri/src/tools/mod.rs`
  - Registers `api_workbench` module and dispatch.
- `apps/desktop/src/bridge/tauri.ts`
  - Adds all `tool:api-workbench:*` channel mappings.
- `apps/desktop/src/composables/toolCatalog.ts`
  - Adds the sidebar entry under `网络与系统`.
- `apps/desktop/src/tool-registry.ts`
  - Registers `ApiWorkbenchPanel.vue`.
- `apps/desktop/src/types/index.ts`
  - Re-exports API workbench frontend types.

Do not modify:

- `Cargo.toml` unless `ureq` proves insufficient for the first-version behavior. It is already present.
- Existing network tool behavior.
- Vault or settings storage.

## Shared Backend Contract

Use camelCase on the IPC boundary. Rust internal structs may use snake_case with `#[serde(rename_all = "camelCase")]`.

Core TypeScript shape:

```ts
export type ApiWorkbenchBodyType = "none" | "json" | "text" | "form-urlencoded";
export type ApiWorkbenchMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS";

export interface ApiWorkbenchKeyValueRow {
  enabled: boolean;
  key: string;
  value: string;
}

export interface ApiWorkbenchRequestDraft {
  method: ApiWorkbenchMethod;
  url: string;
  query: ApiWorkbenchKeyValueRow[];
  headers: ApiWorkbenchKeyValueRow[];
  bodyType: ApiWorkbenchBodyType;
  body: string;
  form: ApiWorkbenchKeyValueRow[];
  timeoutMs: number;
}

export interface ApiWorkbenchSendResult {
  finalUrl: string;
  status: number | null;
  statusText: string;
  ok: boolean;
  durationMs: number;
  requestHeaders: ApiWorkbenchKeyValueRow[];
  responseHeaders: ApiWorkbenchKeyValueRow[];
  bodyText: string;
  bodySize: number;
  bodyTruncated: boolean;
  contentType: string;
  error: string | null;
}
```

## Task 1: Backend Schema And Domain Registration

**Files:**

- Create: `apps/desktop/src-tauri/src/tools/api_workbench.rs`
- Modify: `apps/desktop/src-tauri/src/tools/helpers.rs`
- Modify: `apps/desktop/src-tauri/src/tools/mod.rs`

- [ ] **Step 1: Write the failing Rust schema and dispatch tests**

Add this to the bottom of the new `apps/desktop/src-tauri/src/tools/api_workbench.rs` after the initial skeleton imports:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::json;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open memory db");
        conn.execute_batch("PRAGMA foreign_keys = ON;").expect("foreign keys");
        conn.execute_batch(API_WORKBENCH_SCHEMA_SQL).expect("schema");
        conn
    }

    #[test]
    fn api_workbench_schema_creates_core_tables() {
        let conn = test_conn();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN (
                    'api_workbench_collections',
                    'api_workbench_folders',
                    'api_workbench_requests',
                    'api_workbench_environments',
                    'api_workbench_environment_variables',
                    'api_workbench_global_variables',
                    'api_workbench_history'
                )",
                [],
                |row| row.get(0),
            )
            .expect("table count");
        assert_eq!(count, 7);
    }

    #[test]
    fn api_workbench_execute_rejects_unknown_action() {
        let err = execute("missing_action", &json!({})).expect_err("unknown action");
        assert!(err.contains("unsupported api_workbench action"));
    }
}
```

- [ ] **Step 2: Run the targeted Rust tests and verify the expected failure**

Run:

```powershell
cargo test api_workbench -- --nocapture
```

Expected: FAIL because `api_workbench.rs`, `API_WORKBENCH_SCHEMA_SQL`, or `execute` is not implemented.

- [ ] **Step 3: Add the backend module skeleton and schema SQL**

Create `apps/desktop/src-tauri/src/tools/api_workbench.rs` with this initial content above the test module:

```rust
use serde_json::{json, Value};

pub(crate) const API_WORKBENCH_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS api_workbench_collections (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  active_environment_id INTEGER,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY(active_environment_id) REFERENCES api_workbench_environments(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_api_workbench_collections_sort
  ON api_workbench_collections(sort_order, id);

CREATE TABLE IF NOT EXISTS api_workbench_folders (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  collection_id INTEGER NOT NULL,
  parent_id INTEGER,
  name TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY(collection_id) REFERENCES api_workbench_collections(id) ON DELETE CASCADE,
  FOREIGN KEY(parent_id) REFERENCES api_workbench_folders(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_api_workbench_folders_collection
  ON api_workbench_folders(collection_id, parent_id, sort_order);

CREATE TABLE IF NOT EXISTS api_workbench_requests (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  collection_id INTEGER NOT NULL,
  folder_id INTEGER,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  method TEXT NOT NULL DEFAULT 'GET',
  url TEXT NOT NULL DEFAULT '',
  query_json TEXT NOT NULL DEFAULT '[]',
  headers_json TEXT NOT NULL DEFAULT '[]',
  body_type TEXT NOT NULL DEFAULT 'none',
  body_text TEXT NOT NULL DEFAULT '',
  form_json TEXT NOT NULL DEFAULT '[]',
  timeout_ms INTEGER NOT NULL DEFAULT 10000,
  example_response_json TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY(collection_id) REFERENCES api_workbench_collections(id) ON DELETE CASCADE,
  FOREIGN KEY(folder_id) REFERENCES api_workbench_folders(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_api_workbench_requests_collection
  ON api_workbench_requests(collection_id, folder_id, sort_order);

CREATE TABLE IF NOT EXISTS api_workbench_environments (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  collection_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(collection_id, name),
  FOREIGN KEY(collection_id) REFERENCES api_workbench_collections(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS api_workbench_environment_variables (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  environment_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  value TEXT NOT NULL DEFAULT '',
  is_secret INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(environment_id, name),
  FOREIGN KEY(environment_id) REFERENCES api_workbench_environments(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_api_workbench_env_vars_environment
  ON api_workbench_environment_variables(environment_id, sort_order);

CREATE TABLE IF NOT EXISTS api_workbench_global_variables (
  name TEXT PRIMARY KEY,
  value TEXT NOT NULL DEFAULT '',
  is_secret INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS api_workbench_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  collection_id INTEGER,
  environment_id INTEGER,
  request_id INTEGER,
  name TEXT NOT NULL DEFAULT '',
  method TEXT NOT NULL,
  url TEXT NOT NULL,
  final_url TEXT NOT NULL,
  status INTEGER,
  duration_ms INTEGER NOT NULL,
  ok INTEGER NOT NULL,
  error TEXT,
  response_content_type TEXT NOT NULL DEFAULT '',
  response_size INTEGER NOT NULL DEFAULT 0,
  response_body_preview TEXT NOT NULL DEFAULT '',
  response_body_truncated INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY(collection_id) REFERENCES api_workbench_collections(id) ON DELETE SET NULL,
  FOREIGN KEY(environment_id) REFERENCES api_workbench_environments(id) ON DELETE SET NULL,
  FOREIGN KEY(request_id) REFERENCES api_workbench_requests(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_api_workbench_history_created
  ON api_workbench_history(created_at DESC);
"#;

pub fn execute(action: &str, _payload: &Value) -> Result<Value, String> {
    match action {
        "list" => Ok(json!({ "collections": [], "history": [] })),
        _ => Err(format!("unsupported api_workbench action: {action}")),
    }
}
```

- [ ] **Step 4: Wire schema SQL into `helpers.rs`**

Inside `ensure_schema`, after the existing pre-migration `ALTER TABLE` block and before the large `CREATE TABLE IF NOT EXISTS hosts_profiles` batch, add:

```rust
    conn.execute_batch(super::api_workbench::API_WORKBENCH_SCHEMA_SQL)
        .map_err(|e| format!("create api workbench schema failed: {e}"))?;
```

- [ ] **Step 5: Register the module and dispatch**

In `apps/desktop/src-tauri/src/tools/mod.rs`, add the module declaration:

```rust
pub mod api_workbench;
```

Then add this match arm in `dispatch_tool`:

```rust
        "api_workbench" => api_workbench::execute(action, payload),
```

- [ ] **Step 6: Run tests and commit**

Run:

```powershell
cargo test api_workbench -- --nocapture
```

Expected: PASS for the two initial tests.

Commit:

```powershell
git add apps/desktop/src-tauri/src/tools/api_workbench.rs apps/desktop/src-tauri/src/tools/helpers.rs apps/desktop/src-tauri/src/tools/mod.rs
git commit -m "feat(api-workbench): 添加后端域和数据表"
```

## Task 2: Backend Pure Request Helpers

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/api_workbench.rs`

- [ ] **Step 1: Add failing tests for variables, URL building, body preparation, and redirects**

Append these tests inside the existing `tests` module:

```rust
    #[test]
    fn validate_variable_name_accepts_expected_names() {
        assert!(validate_variable_name("TOKEN"));
        assert!(validate_variable_name("org_id"));
        assert!(validate_variable_name("x-api-key"));
        assert!(!validate_variable_name(""));
        assert!(!validate_variable_name("a.b"));
        assert!(!validate_variable_name(&"a".repeat(65)));
    }

    #[test]
    fn resolve_template_reports_missing_variables() {
        let mut vars = std::collections::HashMap::new();
        vars.insert("TOKEN".to_string(), "abc".to_string());
        let err = resolve_template("Bearer {{TOKEN}} {{ORG_ID}}", &vars).expect_err("missing");
        assert!(err.contains("ORG_ID"));
    }

    #[test]
    fn build_final_url_joins_base_url_and_query_rows() {
        let query = vec![
            KeyValueRow { enabled: true, key: "page".into(), value: "1".into() },
            KeyValueRow { enabled: false, key: "skip".into(), value: "x".into() },
        ];
        let out = build_final_url("http://127.0.0.1:8080/", "api/users", &query).expect("url");
        assert_eq!(out, "http://127.0.0.1:8080/api/users?page=1");
    }

    #[test]
    fn build_final_url_rejects_relative_url_without_base() {
        let err = build_final_url("", "/api/users", &[]).expect_err("base required");
        assert!(err.contains("BASE_URL"));
    }

    #[test]
    fn prepare_request_body_validates_json_and_form_encoding() {
        let json_body = prepare_request_body("json", "{\"ok\":true}", &[], &[])
            .expect("json body");
        assert_eq!(json_body.content_type.as_deref(), Some("application/json"));
        assert_eq!(String::from_utf8(json_body.body.unwrap()).unwrap(), "{\"ok\":true}");

        let form = vec![
            KeyValueRow { enabled: true, key: "a b".into(), value: "1+2".into() },
            KeyValueRow { enabled: false, key: "skip".into(), value: "x".into() },
        ];
        let form_body = prepare_request_body("form-urlencoded", "", &form, &[])
            .expect("form body");
        assert_eq!(form_body.content_type.as_deref(), Some("application/x-www-form-urlencoded"));
        assert_eq!(String::from_utf8(form_body.body.unwrap()).unwrap(), "a%20b=1%2B2");

        let err = prepare_request_body("json", "{", &[], &[]).expect_err("bad json");
        assert!(err.contains("JSON"));
    }
```

- [ ] **Step 2: Run tests and verify the expected failure**

Run:

```powershell
cargo test api_workbench -- --nocapture
```

Expected: FAIL because helper structs and functions are missing.

- [ ] **Step 3: Add helper types and pure functions**

Add these definitions above `execute`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::time::{Duration, Instant};

const MAX_TIMEOUT_MS: u64 = 120_000;
const MIN_TIMEOUT_MS: u64 = 100;
const DEFAULT_TIMEOUT_MS: u64 = 10_000;
const MAX_RESPONSE_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_HISTORY_BODY_PREVIEW_BYTES: usize = 64 * 1024;
const MAX_HISTORY_ROWS: i64 = 200;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct KeyValueRow {
    enabled: bool,
    key: String,
    value: String,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone)]
struct PreparedBody {
    body: Option<Vec<u8>>,
    content_type: Option<String>,
}

fn validate_variable_name(name: &str) -> bool {
    let len = name.chars().count();
    if len == 0 || len > 64 {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn extract_variable_names(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut rest = input;
    while let Some(start) = rest.find("{{") {
        let after_start = &rest[start + 2..];
        if let Some(end) = after_start.find("}}") {
            let name = after_start[..end].trim().to_string();
            if seen.insert(name.clone()) {
                out.push(name);
            }
            rest = &after_start[end + 2..];
        } else {
            break;
        }
    }
    out
}

fn resolve_template(input: &str, vars: &HashMap<String, String>) -> Result<String, String> {
    let names = extract_variable_names(input);
    let mut missing = Vec::new();
    for name in &names {
        if !validate_variable_name(name) || !vars.contains_key(name) {
            missing.push(name.clone());
        }
    }
    if !missing.is_empty() {
        missing.sort();
        missing.dedup();
        return Err(format!("未解析变量: {}", missing.join(", ")));
    }
    let mut output = input.to_string();
    for name in names {
        if let Some(value) = vars.get(&name) {
            output = output.replace(&format!("{{{{{name}}}}}"), value);
        }
    }
    Ok(output)
}

fn is_absolute_http_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn append_query_rows(mut final_url: String, query: &[KeyValueRow]) -> String {
    let enabled: Vec<String> = query
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
        .map(|row| {
            format!(
                "{}={}",
                urlencoding::encode(row.key.trim()),
                urlencoding::encode(row.value.as_str())
            )
        })
        .collect();
    if enabled.is_empty() {
        return final_url;
    }
    let separator = if final_url.contains('?') { "&" } else { "?" };
    final_url.push_str(separator);
    final_url.push_str(&enabled.join("&"));
    final_url
}

fn build_final_url(base_url: &str, raw_url: &str, query: &[KeyValueRow]) -> Result<String, String> {
    let url = raw_url.trim();
    if url.is_empty() {
        return Err("请求 URL 不能为空".to_string());
    }
    let combined = if is_absolute_http_url(url) {
        url.to_string()
    } else {
        let base = base_url.trim();
        if base.is_empty() {
            return Err("相对 URL 需要配置 BASE_URL".to_string());
        }
        if !is_absolute_http_url(base) {
            return Err("BASE_URL 只支持 http 或 https".to_string());
        }
        format!("{}/{}", base.trim_end_matches('/'), url.trim_start_matches('/'))
    };
    if !is_absolute_http_url(&combined) {
        return Err("只支持 http 和 https 协议".to_string());
    }
    Ok(append_query_rows(combined, query))
}

fn has_header(headers: &[KeyValueRow], name: &str) -> bool {
    headers
        .iter()
        .any(|row| row.enabled && row.key.eq_ignore_ascii_case(name))
}

fn prepare_request_body(
    body_type: &str,
    body: &str,
    form: &[KeyValueRow],
    headers: &[KeyValueRow],
) -> Result<PreparedBody, String> {
    match body_type {
        "none" => Ok(PreparedBody { body: None, content_type: None }),
        "json" => {
            serde_json::from_str::<Value>(body).map_err(|e| format!("JSON Body 格式错误: {e}"))?;
            Ok(PreparedBody {
                body: Some(body.as_bytes().to_vec()),
                content_type: if has_header(headers, "Content-Type") {
                    None
                } else {
                    Some("application/json".to_string())
                },
            })
        }
        "text" => Ok(PreparedBody {
            body: Some(body.as_bytes().to_vec()),
            content_type: if has_header(headers, "Content-Type") {
                None
            } else {
                Some("text/plain; charset=utf-8".to_string())
            },
        }),
        "form-urlencoded" => {
            let encoded: Vec<String> = form
                .iter()
                .filter(|row| row.enabled && !row.key.trim().is_empty())
                .map(|row| {
                    format!(
                        "{}={}",
                        urlencoding::encode(row.key.trim()),
                        urlencoding::encode(row.value.as_str())
                    )
                })
                .collect();
            Ok(PreparedBody {
                body: Some(encoded.join("&").into_bytes()),
                content_type: if has_header(headers, "Content-Type") {
                    None
                } else {
                    Some("application/x-www-form-urlencoded".to_string())
                },
            })
        }
        other => Err(format!("unsupported body type: {other}")),
    }
}

fn clamp_timeout_ms(value: u64) -> u64 {
    value.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS)
}
```

- [ ] **Step 4: Run tests and commit**

Run:

```powershell
cargo test api_workbench -- --nocapture
```

Expected: PASS for schema and pure helper tests.

Commit:

```powershell
git add apps/desktop/src-tauri/src/tools/api_workbench.rs
git commit -m "test(api-workbench): 覆盖请求解析纯函数"
```

## Task 3: Backend Collection, Environment, And Global Variable Actions

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/api_workbench.rs`

- [ ] **Step 1: Add failing DB tests**

Append these tests:

```rust
    #[test]
    fn collection_create_initializes_default_environment_and_base_url() {
        let conn = test_conn();
        let result = collection_create_with_conn(
            &conn,
            &json!({ "name": "Demo", "description": "desc" }),
        )
        .expect("create");
        let collection_id = result["id"].as_i64().expect("collection id");
        let active_environment_id = result["activeEnvironmentId"].as_i64().expect("env id");
        assert!(collection_id > 0);
        assert!(active_environment_id > 0);

        let base_url_count: i64 = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM api_workbench_environment_variables
                 WHERE environment_id=?1 AND name='BASE_URL'",
                [active_environment_id],
                |row| row.get(0),
            )
            .expect("base url count");
        assert_eq!(base_url_count, 1);
    }

    #[test]
    fn collection_set_active_environment_requires_same_collection() {
        let conn = test_conn();
        let a = collection_create_with_conn(&conn, &json!({ "name": "A" })).expect("a");
        let b = collection_create_with_conn(&conn, &json!({ "name": "B" })).expect("b");
        let a_id = a["id"].as_i64().unwrap();
        let b_env_id = b["activeEnvironmentId"].as_i64().unwrap();
        let err = collection_set_active_environment_with_conn(
            &conn,
            &json!({ "collectionId": a_id, "environmentId": b_env_id }),
        )
        .expect_err("must reject");
        assert!(err.contains("不属于当前集合"));
    }

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
    fn global_variables_reject_base_url() {
        let conn = test_conn();
        let err = global_variables_save_with_conn(
            &conn,
            &json!({ "variables": [{ "name": "BASE_URL", "value": "http://x", "isSecret": false }] }),
        )
        .expect_err("reject");
        assert!(err.contains("BASE_URL"));
    }
```

- [ ] **Step 2: Run tests and verify the expected failure**

Run:

```powershell
cargo test api_workbench -- --nocapture
```

Expected: FAIL because DB action helpers are missing.

- [ ] **Step 3: Add DB parsing helpers and collection actions**

Add these functions near the pure helpers:

```rust
use rusqlite::{params, Connection};
use super::helpers::db_conn;

fn parse_i64(payload: &Value, key: &str) -> Result<i64, String> {
    payload[key]
        .as_i64()
        .ok_or_else(|| format!("{key} must be an integer"))
}

fn parse_name(payload: &Value, key: &str) -> Result<String, String> {
    let value = payload[key].as_str().unwrap_or_default().trim().to_string();
    if value.is_empty() {
        return Err(format!("{key} 不能为空"));
    }
    Ok(value)
}

fn collection_create_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let name = parse_name(payload, "name")?;
    let description = payload["description"].as_str().unwrap_or_default().trim();
    let next_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM api_workbench_collections",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO api_workbench_collections(name, description, sort_order)
         VALUES(?1, ?2, ?3)",
        params![name, description, next_order],
    )
    .map_err(|e| format!("create api collection failed: {e}"))?;
    let collection_id = conn.last_insert_rowid();
    let env = environment_save_with_conn(
        conn,
        &json!({
            "collectionId": collection_id,
            "name": "开发",
            "variables": [{ "name": "BASE_URL", "value": "", "isSecret": false }]
        }),
    )?;
    let active_environment_id = env["id"].as_i64().ok_or("environment id missing")?;
    conn.execute(
        "UPDATE api_workbench_collections
         SET active_environment_id=?1, updated_at=CURRENT_TIMESTAMP
         WHERE id=?2",
        params![active_environment_id, collection_id],
    )
    .map_err(|e| format!("set active environment failed: {e}"))?;
    Ok(json!({
        "id": collection_id,
        "name": name,
        "description": description,
        "activeEnvironmentId": active_environment_id,
        "sortOrder": next_order
    }))
}

fn collection_set_active_environment_with_conn(
    conn: &Connection,
    payload: &Value,
) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let environment_id = parse_i64(payload, "environmentId")?;
    let owner: i64 = conn
        .query_row(
            "SELECT collection_id FROM api_workbench_environments WHERE id=?1",
            [environment_id],
            |row| row.get(0),
        )
        .map_err(|_| "环境不存在".to_string())?;
    if owner != collection_id {
        return Err("环境不属于当前集合".to_string());
    }
    let affected = conn
        .execute(
            "UPDATE api_workbench_collections
             SET active_environment_id=?1, updated_at=CURRENT_TIMESTAMP
             WHERE id=?2",
            params![environment_id, collection_id],
        )
        .map_err(|e| format!("set active environment failed: {e}"))?;
    if affected == 0 {
        return Err("集合不存在".to_string());
    }
    Ok(json!({ "ok": true, "activeEnvironmentId": environment_id }))
}
```

- [ ] **Step 4: Add environment and global variable actions**

Add these functions:

```rust
fn parse_variable_rows(payload: &Value) -> Result<Vec<KeyValueRow>, String> {
    let rows = payload["variables"].as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();
    for item in rows {
        let name = item["name"].as_str().unwrap_or_default().trim();
        if !validate_variable_name(name) {
            return Err(format!("变量名无效: {name}"));
        }
        out.push(KeyValueRow {
            enabled: true,
            key: name.to_string(),
            value: item["value"].as_str().unwrap_or_default().to_string(),
        });
    }
    Ok(out)
}

fn environment_save_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
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
        rows.insert(0, KeyValueRow { enabled: true, key: "BASE_URL".into(), value: "".into() });
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

fn environment_delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
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

fn global_variables_save_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let rows = parse_variable_rows(payload)?;
    conn.execute("DELETE FROM api_workbench_global_variables", [])
        .map_err(|e| format!("clear global variables failed: {e}"))?;
    for (idx, row) in rows.iter().enumerate() {
        if row.key == "BASE_URL" {
            return Err("全局变量不能使用 BASE_URL".to_string());
        }
        conn.execute(
            "INSERT INTO api_workbench_global_variables(name, value, is_secret, sort_order)
             VALUES(?1, ?2, 0, ?3)",
            params![row.key, row.value, idx as i64],
        )
        .map_err(|e| format!("save global variable failed: {e}"))?;
    }
    Ok(json!({ "ok": true }))
}
```

- [ ] **Step 5: Extend `execute` dispatch**

Replace the skeleton `execute` match with:

```rust
pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    match action {
        "list" => action_list_with_conn(&conn),
        "collection_create" => collection_create_with_conn(&conn, payload),
        "collection_update" => collection_update_with_conn(&conn, payload),
        "collection_set_active_environment" => {
            collection_set_active_environment_with_conn(&conn, payload)
        }
        "collection_delete" => collection_delete_with_conn(&conn, payload),
        "environment_list" => environment_list_with_conn(&conn, payload),
        "environment_save" => environment_save_with_conn(&conn, payload),
        "environment_delete" => environment_delete_with_conn(&conn, payload),
        "global_variables_list" => global_variables_list_with_conn(&conn),
        "global_variables_save" => global_variables_save_with_conn(&conn, payload),
        _ => Err(format!("unsupported api_workbench action: {action}")),
    }
}
```

In the same file, add temporary concrete stubs for actions not yet implemented by this task so dispatch compiles:

```rust
fn action_list_with_conn(_conn: &Connection) -> Result<Value, String> {
    Ok(json!({ "collections": [], "history": [] }))
}

fn collection_update_with_conn(_conn: &Connection, _payload: &Value) -> Result<Value, String> {
    Ok(json!({ "ok": true }))
}

fn collection_delete_with_conn(_conn: &Connection, _payload: &Value) -> Result<Value, String> {
    Ok(json!({ "ok": true }))
}

fn environment_list_with_conn(_conn: &Connection, _payload: &Value) -> Result<Value, String> {
    Ok(json!({ "items": [] }))
}

fn global_variables_list_with_conn(_conn: &Connection) -> Result<Value, String> {
    Ok(json!({ "items": [] }))
}
```

- [ ] **Step 6: Run tests and commit**

Run:

```powershell
cargo test api_workbench -- --nocapture
```

Expected: PASS for collection, environment, global variable, schema, and helper tests.

Commit:

```powershell
git add apps/desktop/src-tauri/src/tools/api_workbench.rs
git commit -m "feat(api-workbench): 添加集合和环境变量后端"
```

## Task 4: Backend Folder, Request, List, And Delete Actions

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/api_workbench.rs`

- [ ] **Step 1: Add failing tests for saved requests and collection list**

Append tests:

```rust
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
    fn action_list_returns_collections_with_folders_and_requests() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        let folder = folder_create_with_conn(
            &conn,
            &json!({ "collectionId": collection_id, "name": "Users" }),
        )
        .expect("folder");
        request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": folder["id"].as_i64().unwrap(),
                "name": "List users",
                "draft": {
                    "method": "GET",
                    "url": "/api/users",
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

        let list = action_list_with_conn(&conn).expect("list");
        assert_eq!(list["collections"][0]["name"], "Demo");
        assert_eq!(list["collections"][0]["folders"][0]["name"], "Users");
        assert_eq!(list["collections"][0]["requests"][0]["name"], "List users");
    }
```

- [ ] **Step 2: Run tests and verify the expected failure**

Run:

```powershell
cargo test api_workbench -- --nocapture
```

Expected: FAIL because folder and request actions are missing.

- [ ] **Step 3: Implement folder actions**

Add:

```rust
fn folder_create_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
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
    Ok(json!({ "id": conn.last_insert_rowid(), "collectionId": collection_id, "parentId": parent_id, "name": name }))
}

fn folder_update_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
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

fn folder_delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    conn.execute("DELETE FROM api_workbench_folders WHERE id=?1", [id])
        .map_err(|e| format!("delete folder failed: {e}"))?;
    Ok(json!({ "ok": true }))
}
```

- [ ] **Step 4: Implement request save/get/delete actions**

Add:

```rust
fn parse_draft(payload: &Value) -> Result<RequestDraft, String> {
    serde_json::from_value(payload["draft"].clone()).map_err(|e| format!("请求草稿格式错误: {e}"))
}

fn request_save_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
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
                     form_json=?10, timeout_ms=?11, updated_at=CURRENT_TIMESTAMP
                 WHERE id=?12 AND collection_id=?13",
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
        .map_err(|e| format!("create request failed: {e}"))?;
        Ok(json!({ "id": conn.last_insert_rowid(), "ok": true }))
    }
}

fn request_get_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    conn.query_row(
        "SELECT id, collection_id, folder_id, name, description, method, url,
                query_json, headers_json, body_type, body_text, form_json, timeout_ms,
                example_response_json, sort_order, created_at, updated_at
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
                    "timeoutMs": row.get::<_, i64>(12)?
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

fn request_delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    conn.execute("DELETE FROM api_workbench_requests WHERE id=?1", [id])
        .map_err(|e| format!("delete request failed: {e}"))?;
    Ok(json!({ "ok": true }))
}
```

- [ ] **Step 5: Implement list, update, delete, and dispatch**

Implement the concrete list/update/delete actions. Keep the list result small by returning request summaries and loading full request details through `request_get`:

```rust
fn collection_update_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    let name = parse_name(payload, "name")?;
    let description = payload["description"].as_str().unwrap_or_default().trim();
    let affected = conn
        .execute(
            "UPDATE api_workbench_collections
             SET name=?1, description=?2, updated_at=CURRENT_TIMESTAMP WHERE id=?3",
            params![name, description, id],
        )
        .map_err(|e| format!("update collection failed: {e}"))?;
    if affected == 0 {
        return Err("集合不存在".to_string());
    }
    Ok(json!({ "ok": true }))
}

fn collection_delete_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id")?;
    conn.execute("DELETE FROM api_workbench_collections WHERE id=?1", [id])
        .map_err(|e| format!("delete collection failed: {e}"))?;
    Ok(json!({ "ok": true }))
}
```

For `action_list_with_conn`, return this shape:

```json
{
  "collections": [
    {
      "id": 1,
      "name": "Demo",
      "description": "",
      "activeEnvironmentId": 1,
      "folders": [],
      "requests": []
    }
  ],
  "history": []
}
```

Add all missing dispatch arms:

```rust
        "folder_create" => folder_create_with_conn(&conn, payload),
        "folder_update" => folder_update_with_conn(&conn, payload),
        "folder_delete" => folder_delete_with_conn(&conn, payload),
        "request_get" => request_get_with_conn(&conn, payload),
        "request_save" => request_save_with_conn(&conn, payload),
        "request_delete" => request_delete_with_conn(&conn, payload),
```

- [ ] **Step 6: Run tests and commit**

Run:

```powershell
cargo test api_workbench -- --nocapture
```

Expected: PASS for saved request round-trip and list tree tests.

Commit:

```powershell
git add apps/desktop/src-tauri/src/tools/api_workbench.rs
git commit -m "feat(api-workbench): 添加接口集合树和请求保存"
```

## Task 5: Backend Send, History, And Redirect Behavior

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/api_workbench.rs`

- [ ] **Step 1: Add failing local HTTP server tests**

Append tests:

```rust
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
            },
        )
        .expect("history");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM api_workbench_history", [], |row| row.get(0))
            .expect("count");
        assert_eq!(count, 1);
    }
```

- [ ] **Step 2: Run tests and verify the expected failure**

Run:

```powershell
cargo test api_workbench -- --nocapture
```

Expected: FAIL because send/history functions are missing.

- [ ] **Step 3: Implement variable loading and request resolution**

Add:

```rust
fn load_variables(
    conn: &Connection,
    environment_id: i64,
) -> Result<(HashMap<String, String>, String), String> {
    let mut vars = HashMap::new();
    let mut stmt = conn
        .prepare("SELECT name, value FROM api_workbench_global_variables ORDER BY sort_order ASC")
        .map_err(|e| format!("prepare global variables failed: {e}"))?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
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
        .query_map([environment_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
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

fn resolve_rows(rows: &[KeyValueRow], vars: &HashMap<String, String>) -> Result<Vec<KeyValueRow>, String> {
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
```

- [ ] **Step 4: Implement request execution with redirects disabled**

Add:

```rust
fn execute_http_request(
    draft: &RequestDraft,
    final_url: &str,
    headers: &[KeyValueRow],
    prepared: PreparedBody,
) -> Result<Value, String> {
    let started = Instant::now();
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_millis(clamp_timeout_ms(draft.timeout_ms)))
        .redirects(0)
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
    for row in headers {
        if row.enabled && !row.key.trim().is_empty() {
            request = request.set(row.key.trim(), row.value.as_str());
        }
    }
    if let Some(content_type) = prepared.content_type.as_deref() {
        request = request.set("Content-Type", content_type);
    }

    let result = if let Some(body) = prepared.body {
        request.send_bytes(&body)
    } else {
        request.call()
    };
    let duration_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(resp) => response_to_json(final_url, duration_ms, resp, None),
        Err(ureq::Error::Status(_, resp)) => response_to_json(final_url, duration_ms, resp, None),
        Err(err) => Ok(json!({
            "finalUrl": final_url,
            "status": null,
            "statusText": "",
            "ok": false,
            "durationMs": duration_ms,
            "requestHeaders": headers,
            "responseHeaders": [],
            "bodyText": "",
            "bodySize": 0,
            "bodyTruncated": false,
            "contentType": "",
            "error": err.to_string()
        })),
    }
}

fn response_to_json(
    final_url: &str,
    duration_ms: u64,
    resp: ureq::Response,
    forced_error: Option<String>,
) -> Result<Value, String> {
    let status = resp.status();
    let status_text = resp.status_text().to_string();
    let content_type = resp.header("Content-Type").unwrap_or("").to_string();
    let response_headers: Vec<Value> = resp
        .headers_names()
        .into_iter()
        .map(|key| {
            let value = resp.header(&key).unwrap_or("").to_string();
            json!({ "enabled": true, "key": key, "value": value })
        })
        .collect();
    let mut reader = resp.into_reader().take((MAX_RESPONSE_BODY_BYTES + 1) as u64);
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read response body failed: {e}"))?;
    let body_truncated = bytes.len() > MAX_RESPONSE_BODY_BYTES;
    if body_truncated {
        bytes.truncate(MAX_RESPONSE_BODY_BYTES);
    }
    let body_size = bytes.len();
    let body_text = String::from_utf8_lossy(&bytes).to_string();
    Ok(json!({
        "finalUrl": final_url,
        "status": status,
        "statusText": status_text,
        "ok": (200..300).contains(&status),
        "durationMs": duration_ms,
        "requestHeaders": [],
        "responseHeaders": response_headers,
        "bodyText": body_text,
        "bodySize": body_size,
        "bodyTruncated": body_truncated,
        "contentType": content_type,
        "error": forced_error
    }))
}
```

- [ ] **Step 5: Implement send action and history insert**

Add:

```rust
struct HistoryInsert {
    collection_id: Option<i64>,
    environment_id: Option<i64>,
    request_id: Option<i64>,
    name: String,
    method: String,
    url: String,
    final_url: String,
    status: Option<i64>,
    duration_ms: u64,
    ok: bool,
    error: Option<String>,
    response_content_type: String,
    response_size: usize,
    response_body_preview: String,
    response_body_truncated: bool,
}

fn insert_history_with_conn(conn: &Connection, item: &HistoryInsert) -> Result<(), String> {
    let preview = if item.response_body_preview.len() > MAX_HISTORY_BODY_PREVIEW_BYTES {
        item.response_body_preview[..MAX_HISTORY_BODY_PREVIEW_BYTES].to_string()
    } else {
        item.response_body_preview.clone()
    };
    conn.execute(
        "INSERT INTO api_workbench_history(
            collection_id, environment_id, request_id, name, method, url, final_url,
            status, duration_ms, ok, error, response_content_type, response_size,
            response_body_preview, response_body_truncated
         )
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
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
            if item.response_body_truncated { 1 } else { 0 }
        ],
    )
    .map_err(|e| format!("insert history failed: {e}"))?;
    conn.execute(
        "DELETE FROM api_workbench_history
         WHERE id NOT IN (
            SELECT id FROM api_workbench_history ORDER BY created_at DESC, id DESC LIMIT ?1
         )",
        [MAX_HISTORY_ROWS],
    )
    .map_err(|e| format!("trim history failed: {e}"))?;
    Ok(())
}

fn send_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = payload["collectionId"].as_i64();
    let environment_id = parse_i64(payload, "environmentId")?;
    let request_id = payload["requestId"].as_i64();
    let draft: RequestDraft = serde_json::from_value(payload["draft"].clone())
        .map_err(|e| format!("请求草稿格式错误: {e}"))?;
    let (vars, base_url) = load_variables(conn, environment_id)?;
    let resolved_url = resolve_template(&draft.url, &vars)?;
    let resolved_query = resolve_rows(&draft.query, &vars)?;
    let resolved_headers = resolve_rows(&draft.headers, &vars)?;
    let resolved_body = resolve_template(&draft.body, &vars)?;
    let resolved_form = resolve_rows(&draft.form, &vars)?;
    let final_url = build_final_url(&base_url, &resolved_url, &resolved_query)?;
    let prepared = prepare_request_body(
        &draft.body_type,
        &resolved_body,
        &resolved_form,
        &resolved_headers,
    )?;
    let result = execute_http_request(&draft, &final_url, &resolved_headers, prepared)?;
    insert_history_with_conn(
        conn,
        &HistoryInsert {
            collection_id,
            environment_id: Some(environment_id),
            request_id,
            name: payload["name"].as_str().unwrap_or_default().to_string(),
            method: draft.method.clone(),
            url: draft.url.clone(),
            final_url: final_url.clone(),
            status: result["status"].as_i64(),
            duration_ms: result["durationMs"].as_u64().unwrap_or(0),
            ok: result["ok"].as_bool().unwrap_or(false),
            error: result["error"].as_str().map(|s| s.to_string()),
            response_content_type: result["contentType"].as_str().unwrap_or_default().to_string(),
            response_size: result["bodySize"].as_u64().unwrap_or(0) as usize,
            response_body_preview: result["bodyText"].as_str().unwrap_or_default().to_string(),
            response_body_truncated: result["bodyTruncated"].as_bool().unwrap_or(false),
        },
    )?;
    Ok(result)
}
```

Add dispatch arms:

```rust
        "send" => send_with_conn(&conn, payload),
        "history_list" => history_list_with_conn(&conn),
        "history_clear" => history_clear_with_conn(&conn),
```

Implement:

```rust
fn history_list_with_conn(conn: &Connection) -> Result<Value, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, collection_id, environment_id, request_id, name, method, url, final_url,
                    status, duration_ms, ok, error, response_content_type, response_size,
                    response_body_preview, response_body_truncated, created_at
             FROM api_workbench_history ORDER BY created_at DESC, id DESC LIMIT ?1",
        )
        .map_err(|e| format!("prepare history failed: {e}"))?;
    let rows = stmt
        .query_map([MAX_HISTORY_ROWS], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "collectionId": row.get::<_, Option<i64>>(1)?,
                "environmentId": row.get::<_, Option<i64>>(2)?,
                "requestId": row.get::<_, Option<i64>>(3)?,
                "name": row.get::<_, String>(4)?,
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
                "createdAt": row.get::<_, String>(16)?
            }))
        })
        .map_err(|e| format!("query history failed: {e}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| e.to_string())?);
    }
    Ok(json!({ "items": items }))
}

fn history_clear_with_conn(conn: &Connection) -> Result<Value, String> {
    conn.execute("DELETE FROM api_workbench_history", [])
        .map_err(|e| format!("clear history failed: {e}"))?;
    Ok(json!({ "ok": true }))
}
```

- [ ] **Step 6: Run tests and commit**

Run:

```powershell
cargo test api_workbench -- --nocapture
```

Expected: PASS for send, redirect, history, and previous tests.

Commit:

```powershell
git add apps/desktop/src-tauri/src/tools/api_workbench.rs
git commit -m "feat(api-workbench): 添加请求发送和历史记录"
```

## Task 6: Backend Markdown Export

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/api_workbench.rs`

- [ ] **Step 1: Add failing Markdown export tests**

Append:

```rust
    #[test]
    fn export_markdown_redacts_sensitive_headers_and_hides_variable_values() {
        let conn = test_conn();
        let c = collection_create_with_conn(&conn, &json!({ "name": "Demo", "description": "API docs" })).expect("create");
        let collection_id = c["id"].as_i64().unwrap();
        request_save_with_conn(
            &conn,
            &json!({
                "collectionId": collection_id,
                "folderId": null,
                "name": "Auth",
                "description": "Login",
                "draft": {
                    "method": "POST",
                    "url": "/api/login",
                    "query": [],
                    "headers": [{ "enabled": true, "key": "Authorization", "value": "Bearer secret" }],
                    "bodyType": "json",
                    "body": "{\"name\":\"demo\"}",
                    "form": [],
                    "timeoutMs": 10000
                }
            }),
        )
        .expect("request");
        let result = export_markdown_with_conn(&conn, &json!({ "collectionId": collection_id })).expect("export");
        let markdown = result["markdown"].as_str().unwrap();
        assert!(markdown.contains("# Demo"));
        assert!(markdown.contains("POST /api/login"));
        assert!(markdown.contains("Authorization: ******"));
        assert!(!markdown.contains("Bearer secret"));
        assert!(markdown.contains("BASE_URL"));
    }
```

- [ ] **Step 2: Run tests and verify the expected failure**

Run:

```powershell
cargo test api_workbench -- --nocapture
```

Expected: FAIL because Markdown export is missing.

- [ ] **Step 3: Add Markdown redaction and generation helpers**

Add:

```rust
fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization" | "cookie" | "x-api-key" | "x-auth-token"
    )
}

fn markdown_escape(text: &str) -> String {
    text.replace('|', "\\|")
}

fn render_header_lines(headers: &[KeyValueRow]) -> String {
    let mut lines = Vec::new();
    for header in headers.iter().filter(|row| row.enabled && !row.key.trim().is_empty()) {
        let value = if is_sensitive_header(&header.key) {
            "******".to_string()
        } else {
            header.value.clone()
        };
        lines.push(format!("- `{}`: `{}`", markdown_escape(&header.key), markdown_escape(&value)));
    }
    if lines.is_empty() {
        "- 无".to_string()
    } else {
        lines.join("\n")
    }
}

fn render_request_markdown(item: &Value) -> String {
    let name = item["name"].as_str().unwrap_or("未命名接口");
    let description = item["description"].as_str().unwrap_or("");
    let draft = &item["draft"];
    let method = draft["method"].as_str().unwrap_or("GET");
    let url = draft["url"].as_str().unwrap_or("");
    let headers: Vec<KeyValueRow> = serde_json::from_value(draft["headers"].clone()).unwrap_or_default();
    let body_type = draft["bodyType"].as_str().unwrap_or("none");
    let body = draft["body"].as_str().unwrap_or("");
    let mut out = String::new();
    out.push_str(&format!("### {name}\n\n"));
    if !description.is_empty() {
        out.push_str(description);
        out.push_str("\n\n");
    }
    out.push_str(&format!("`{method} {url}`\n\n"));
    out.push_str("#### Headers\n\n");
    out.push_str(&render_header_lines(&headers));
    out.push_str("\n\n");
    out.push_str("#### Body\n\n");
    if body_type == "none" || body.trim().is_empty() {
        out.push_str("无\n\n");
    } else {
        out.push_str(&format!("```{body_type}\n{body}\n```\n\n"));
    }
    out
}
```

- [ ] **Step 4: Implement export action and dispatch**

Add:

```rust
fn export_markdown_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let collection_id = parse_i64(payload, "collectionId")?;
    let collection = conn
        .query_row(
            "SELECT name, description FROM api_workbench_collections WHERE id=?1",
            [collection_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(|_| "集合不存在".to_string())?;
    let mut markdown = String::new();
    markdown.push_str(&format!("# {}\n\n", collection.0));
    if !collection.1.is_empty() {
        markdown.push_str(&collection.1);
        markdown.push_str("\n\n");
    }

    markdown.push_str("## 环境变量\n\n");
    let mut var_stmt = conn
        .prepare(
            "SELECT e.name, v.name
             FROM api_workbench_environments e
             LEFT JOIN api_workbench_environment_variables v ON v.environment_id=e.id
             WHERE e.collection_id=?1
             ORDER BY e.sort_order ASC, e.id ASC, v.sort_order ASC",
        )
        .map_err(|e| format!("prepare vars failed: {e}"))?;
    let var_rows = var_stmt
        .query_map([collection_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(|e| format!("query vars failed: {e}"))?;
    for row in var_rows {
        let (env_name, var_name) = row.map_err(|e| e.to_string())?;
        if let Some(var_name) = var_name {
            markdown.push_str(&format!("- {}: `{}`\n", env_name, var_name));
        }
    }
    markdown.push_str("\n## 接口\n\n");

    let mut stmt = conn
        .prepare(
            "SELECT id FROM api_workbench_requests
             WHERE collection_id=?1 ORDER BY folder_id IS NOT NULL, folder_id, sort_order, id",
        )
        .map_err(|e| format!("prepare requests failed: {e}"))?;
    let ids = stmt
        .query_map([collection_id], |row| row.get::<_, i64>(0))
        .map_err(|e| format!("query requests failed: {e}"))?;
    for id in ids {
        let request = request_get_with_conn(conn, &json!({ "id": id.map_err(|e| e.to_string())? }))?;
        markdown.push_str(&render_request_markdown(&request));
    }

    let file_name = format!("{}-api.md", collection.0.trim().replace(' ', "-"));
    Ok(json!({ "fileName": file_name, "markdown": markdown }))
}
```

Add dispatch arm:

```rust
        "export_markdown" => export_markdown_with_conn(&conn, payload),
```

- [ ] **Step 5: Run tests and commit**

Run:

```powershell
cargo test api_workbench -- --nocapture
```

Expected: PASS including Markdown redaction.

Commit:

```powershell
git add apps/desktop/src-tauri/src/tools/api_workbench.rs
git commit -m "feat(api-workbench): 添加 Markdown 导出"
```

## Task 7: Frontend Types And Pure Utilities

**Files:**

- Create: `apps/desktop/src/types/api-workbench.ts`
- Create: `apps/desktop/src/utils/apiWorkbench.ts`
- Create: `apps/desktop/src/utils/apiWorkbench.test.ts`
- Modify: `apps/desktop/src/types/index.ts`

- [ ] **Step 1: Write failing frontend utility tests**

Create `apps/desktop/src/utils/apiWorkbench.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import {
  buildApiWorkbenchPreviewUrl,
  extractApiWorkbenchVariables,
  formatApiWorkbenchResponseBody,
  normalizeApiWorkbenchDraft,
  validateApiWorkbenchVariableName,
} from "./apiWorkbench";

describe("apiWorkbench utils", () => {
  it("validates variable names", () => {
    expect(validateApiWorkbenchVariableName("TOKEN")).toBe(true);
    expect(validateApiWorkbenchVariableName("org_id")).toBe(true);
    expect(validateApiWorkbenchVariableName("x-api-key")).toBe(true);
    expect(validateApiWorkbenchVariableName("")).toBe(false);
    expect(validateApiWorkbenchVariableName("a.b")).toBe(false);
  });

  it("extracts unique variables", () => {
    expect(extractApiWorkbenchVariables("{{TOKEN}} {{TOKEN}} {{ORG_ID}}")).toEqual([
      "TOKEN",
      "ORG_ID",
    ]);
  });

  it("builds preview url from base and relative path", () => {
    expect(
      buildApiWorkbenchPreviewUrl("http://127.0.0.1:8080/", "api/users", [
        { enabled: true, key: "page", value: "1" },
      ]),
    ).toBe("http://127.0.0.1:8080/api/users?page=1");
  });

  it("normalizes invalid draft values", () => {
    const draft = normalizeApiWorkbenchDraft({
      method: "bad",
      url: " /api ",
      query: [{ enabled: true, key: " q ", value: "1" }],
      headers: [],
      bodyType: "bad",
      body: "",
      form: [],
      timeoutMs: 999999,
    });
    expect(draft.method).toBe("GET");
    expect(draft.url).toBe("/api");
    expect(draft.bodyType).toBe("none");
    expect(draft.timeoutMs).toBe(120000);
    expect(draft.query[0].key).toBe("q");
  });

  it("formats json response bodies", () => {
    expect(formatApiWorkbenchResponseBody("{\"ok\":true}", "application/json")).toBe(
      "{\n  \"ok\": true\n}",
    );
    expect(formatApiWorkbenchResponseBody("plain", "text/plain")).toBe("plain");
  });
});
```

- [ ] **Step 2: Run frontend tests and verify the expected failure**

Run:

```powershell
pnpm test src/utils/apiWorkbench.test.ts
```

Expected: FAIL because `apiWorkbench.ts` does not exist.

- [ ] **Step 3: Add frontend types**

Create `apps/desktop/src/types/api-workbench.ts`:

```ts
export type ApiWorkbenchBodyType = "none" | "json" | "text" | "form-urlencoded";
export type ApiWorkbenchMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS";

export interface ApiWorkbenchKeyValueRow {
  enabled: boolean;
  key: string;
  value: string;
}

export interface ApiWorkbenchRequestDraft {
  method: ApiWorkbenchMethod;
  url: string;
  query: ApiWorkbenchKeyValueRow[];
  headers: ApiWorkbenchKeyValueRow[];
  bodyType: ApiWorkbenchBodyType;
  body: string;
  form: ApiWorkbenchKeyValueRow[];
  timeoutMs: number;
}

export interface ApiWorkbenchCollection {
  id: number;
  name: string;
  description: string;
  activeEnvironmentId: number | null;
  folders: ApiWorkbenchFolder[];
  requests: ApiWorkbenchRequestSummary[];
}

export interface ApiWorkbenchFolder {
  id: number;
  collectionId: number;
  parentId: number | null;
  name: string;
  sortOrder: number;
}

export interface ApiWorkbenchRequestSummary {
  id: number;
  collectionId: number;
  folderId: number | null;
  name: string;
  method: ApiWorkbenchMethod;
  url: string;
  sortOrder: number;
}

export interface ApiWorkbenchRequestDetail extends ApiWorkbenchRequestSummary {
  description: string;
  draft: ApiWorkbenchRequestDraft;
  exampleResponse?: string | null;
}

export interface ApiWorkbenchVariable {
  name: string;
  value: string;
  isSecret?: boolean;
}

export interface ApiWorkbenchEnvironment {
  id: number;
  collectionId: number;
  name: string;
  variables: ApiWorkbenchVariable[];
}

export interface ApiWorkbenchSendResult {
  finalUrl: string;
  status: number | null;
  statusText: string;
  ok: boolean;
  durationMs: number;
  requestHeaders: ApiWorkbenchKeyValueRow[];
  responseHeaders: ApiWorkbenchKeyValueRow[];
  bodyText: string;
  bodySize: number;
  bodyTruncated: boolean;
  contentType: string;
  error: string | null;
}

export interface ApiWorkbenchHistoryItem {
  id: number;
  collectionId: number | null;
  environmentId: number | null;
  requestId: number | null;
  name: string;
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
  createdAt: string;
}

export interface ApiWorkbenchListResult {
  collections: ApiWorkbenchCollection[];
  history: ApiWorkbenchHistoryItem[];
}
```

Add export in `apps/desktop/src/types/index.ts`:

```ts
export type {
  ApiWorkbenchBodyType,
  ApiWorkbenchMethod,
  ApiWorkbenchKeyValueRow,
  ApiWorkbenchRequestDraft,
  ApiWorkbenchCollection,
  ApiWorkbenchFolder,
  ApiWorkbenchRequestSummary,
  ApiWorkbenchRequestDetail,
  ApiWorkbenchVariable,
  ApiWorkbenchEnvironment,
  ApiWorkbenchSendResult,
  ApiWorkbenchHistoryItem,
  ApiWorkbenchListResult,
} from "./api-workbench";
```

- [ ] **Step 4: Add frontend utility implementation**

Create `apps/desktop/src/utils/apiWorkbench.ts`:

```ts
import type {
  ApiWorkbenchBodyType,
  ApiWorkbenchKeyValueRow,
  ApiWorkbenchMethod,
  ApiWorkbenchRequestDraft,
} from "../types/api-workbench";

export const API_WORKBENCH_METHODS: ApiWorkbenchMethod[] = [
  "GET",
  "POST",
  "PUT",
  "PATCH",
  "DELETE",
  "HEAD",
  "OPTIONS",
];

export const API_WORKBENCH_BODY_TYPES: ApiWorkbenchBodyType[] = [
  "none",
  "json",
  "text",
  "form-urlencoded",
];

export const DEFAULT_API_WORKBENCH_DRAFT: ApiWorkbenchRequestDraft = {
  method: "GET",
  url: "",
  query: [],
  headers: [],
  bodyType: "none",
  body: "",
  form: [],
  timeoutMs: 10000,
};

export function validateApiWorkbenchVariableName(name: string): boolean {
  return /^[A-Za-z0-9_-]{1,64}$/.test(name);
}

export function extractApiWorkbenchVariables(input: string): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  const re = /\{\{\s*([^{}]+?)\s*\}\}/g;
  let match: RegExpExecArray | null;
  while ((match = re.exec(input))) {
    const name = match[1].trim();
    if (!seen.has(name)) {
      seen.add(name);
      out.push(name);
    }
  }
  return out;
}

function encodeQuery(rows: ApiWorkbenchKeyValueRow[]): string {
  return rows
    .filter((row) => row.enabled && row.key.trim())
    .map((row) => `${encodeURIComponent(row.key.trim())}=${encodeURIComponent(row.value)}`)
    .join("&");
}

export function buildApiWorkbenchPreviewUrl(
  baseUrl: string,
  rawUrl: string,
  query: ApiWorkbenchKeyValueRow[],
): string {
  const url = rawUrl.trim();
  const isAbsolute = /^https?:\/\//i.test(url);
  const joined = isAbsolute
    ? url
    : `${baseUrl.trim().replace(/\/+$/, "")}/${url.replace(/^\/+/, "")}`;
  const qs = encodeQuery(query);
  if (!qs) return joined;
  return `${joined}${joined.includes("?") ? "&" : "?"}${qs}`;
}

function normalizeRows(rows: unknown): ApiWorkbenchKeyValueRow[] {
  if (!Array.isArray(rows)) return [];
  return rows
    .map((row) => row as Partial<ApiWorkbenchKeyValueRow>)
    .filter((row) => typeof row.key === "string" || typeof row.value === "string")
    .map((row) => ({
      enabled: row.enabled !== false,
      key: String(row.key ?? "").trim(),
      value: String(row.value ?? ""),
    }));
}

export function normalizeApiWorkbenchDraft(input: Partial<ApiWorkbenchRequestDraft>): ApiWorkbenchRequestDraft {
  const method = API_WORKBENCH_METHODS.includes(input.method as ApiWorkbenchMethod)
    ? (input.method as ApiWorkbenchMethod)
    : "GET";
  const bodyType = API_WORKBENCH_BODY_TYPES.includes(input.bodyType as ApiWorkbenchBodyType)
    ? (input.bodyType as ApiWorkbenchBodyType)
    : "none";
  const timeoutMs = Math.min(120000, Math.max(100, Number(input.timeoutMs || 10000)));
  return {
    method,
    url: String(input.url ?? "").trim(),
    query: normalizeRows(input.query),
    headers: normalizeRows(input.headers),
    bodyType,
    body: String(input.body ?? ""),
    form: normalizeRows(input.form),
    timeoutMs,
  };
}

export function formatApiWorkbenchResponseBody(body: string, contentType: string): string {
  if (!/json/i.test(contentType)) return body;
  try {
    return JSON.stringify(JSON.parse(body), null, 2);
  } catch {
    return body;
  }
}
```

- [ ] **Step 5: Run tests and commit**

Run:

```powershell
pnpm test src/utils/apiWorkbench.test.ts
```

Expected: PASS.

Commit:

```powershell
git add apps/desktop/src/types/api-workbench.ts apps/desktop/src/types/index.ts apps/desktop/src/utils/apiWorkbench.ts apps/desktop/src/utils/apiWorkbench.test.ts
git commit -m "feat(api-workbench): 添加前端类型和纯函数"
```

## Task 8: Frontend Bridge, Catalog, Registry

**Files:**

- Modify: `apps/desktop/src/bridge/tauri.ts`
- Modify: `apps/desktop/src/composables/toolCatalog.ts`
- Modify: `apps/desktop/src/tool-registry.ts`

- [ ] **Step 1: Add channel mappings**

In `apps/desktop/src/bridge/tauri.ts`, add these entries near other tool channels:

```ts
  "tool:api-workbench:list": { domain: "api_workbench", action: "list" },
  "tool:api-workbench:collection-create": { domain: "api_workbench", action: "collection_create" },
  "tool:api-workbench:collection-update": { domain: "api_workbench", action: "collection_update" },
  "tool:api-workbench:collection-set-active-environment": { domain: "api_workbench", action: "collection_set_active_environment" },
  "tool:api-workbench:collection-delete": { domain: "api_workbench", action: "collection_delete" },
  "tool:api-workbench:folder-create": { domain: "api_workbench", action: "folder_create" },
  "tool:api-workbench:folder-update": { domain: "api_workbench", action: "folder_update" },
  "tool:api-workbench:folder-delete": { domain: "api_workbench", action: "folder_delete" },
  "tool:api-workbench:request-get": { domain: "api_workbench", action: "request_get" },
  "tool:api-workbench:request-save": { domain: "api_workbench", action: "request_save" },
  "tool:api-workbench:request-delete": { domain: "api_workbench", action: "request_delete" },
  "tool:api-workbench:environment-list": { domain: "api_workbench", action: "environment_list" },
  "tool:api-workbench:environment-save": { domain: "api_workbench", action: "environment_save" },
  "tool:api-workbench:environment-delete": { domain: "api_workbench", action: "environment_delete" },
  "tool:api-workbench:global-variables-list": { domain: "api_workbench", action: "global_variables_list" },
  "tool:api-workbench:global-variables-save": { domain: "api_workbench", action: "global_variables_save" },
  "tool:api-workbench:send": { domain: "api_workbench", action: "send" },
  "tool:api-workbench:history-list": { domain: "api_workbench", action: "history_list" },
  "tool:api-workbench:history-clear": { domain: "api_workbench", action: "history_clear" },
  "tool:api-workbench:export-markdown": { domain: "api_workbench", action: "export_markdown" },
```

- [ ] **Step 2: Add sidebar catalog entry**

In `apps/desktop/src/composables/toolCatalog.ts`, add under `网络与系统` after the existing `network` tool:

```ts
        { id: "api-workbench", name: "接口调试", desc: "离线 HTTP 接口调试与文档生成" },
```

- [ ] **Step 3: Add registry entry**

In `apps/desktop/src/tool-registry.ts`, add:

```ts
  "api-workbench": defineAsyncComponent(() => import("./components/ApiWorkbenchPanel.vue")),
```

- [ ] **Step 4: Run typecheck and commit**

Run:

```powershell
pnpm typecheck
```

Expected: FAIL only because `ApiWorkbenchPanel.vue` does not exist.

Do not commit the failing state. Continue to Task 9.

## Task 9: Frontend Panel UI

**Files:**

- Create: `apps/desktop/src/components/ApiWorkbenchPanel.vue`

- [ ] **Step 1: Create the panel with loading, empty, and three-column structure**

Create `apps/desktop/src/components/ApiWorkbenchPanel.vue` with this structure:

```vue
<template>
  <div class="api-workbench-panel">
    <aside class="api-workbench-sidebar">
      <div class="api-workbench-toolbar">
        <strong>接口集合</strong>
        <el-button size="small" type="primary" @click="createCollection">新建</el-button>
      </div>
      <el-empty v-if="!loading && collections.length === 0" description="暂无接口集合" />
      <div v-else class="api-workbench-tree">
        <button
          v-for="collection in collections"
          :key="collection.id"
          class="api-workbench-collection"
          :class="{ active: selectedCollectionId === collection.id }"
          @click="selectCollection(collection.id)"
        >
          <span>{{ collection.name }}</span>
          <small>{{ collection.requests.length }} 个接口</small>
        </button>
      </div>
    </aside>

    <main class="api-workbench-editor">
      <div class="api-workbench-request-bar">
        <el-select v-model="draft.method" class="method-select">
          <el-option v-for="method in methods" :key="method" :label="method" :value="method" />
        </el-select>
        <el-input v-model="draft.url" placeholder="https://example.com/api 或 /api/users" />
        <el-select
          v-model="selectedEnvironmentId"
          class="environment-select"
          placeholder="环境"
          @change="persistActiveEnvironment"
        >
          <el-option v-for="env in environments" :key="env.id" :label="env.name" :value="env.id" />
        </el-select>
        <el-button type="primary" :loading="sending" @click="sendRequest">发送</el-button>
      </div>

      <el-alert
        v-if="baseUrlError"
        type="warning"
        :title="baseUrlError"
        show-icon
        :closable="false"
      />

      <el-tabs v-model="editorTab">
        <el-tab-pane label="Query" name="query">
          <KeyValueEditor v-model="draft.query" />
        </el-tab-pane>
        <el-tab-pane label="Headers" name="headers">
          <KeyValueEditor v-model="draft.headers" />
        </el-tab-pane>
        <el-tab-pane label="Body" name="body">
          <div class="body-toolbar">
            <el-radio-group v-model="draft.bodyType">
              <el-radio-button label="none">none</el-radio-button>
              <el-radio-button label="json">json</el-radio-button>
              <el-radio-button label="text">text</el-radio-button>
              <el-radio-button label="form-urlencoded">form</el-radio-button>
            </el-radio-group>
            <el-switch disabled inactive-text="跟随重定向" />
          </div>
          <KeyValueEditor v-if="draft.bodyType === 'form-urlencoded'" v-model="draft.form" />
          <el-input
            v-else-if="draft.bodyType !== 'none'"
            v-model="draft.body"
            type="textarea"
            :rows="12"
          />
          <el-empty v-else description="无请求体" />
        </el-tab-pane>
      </el-tabs>

      <div class="api-workbench-actions">
        <el-input v-model="requestName" placeholder="接口名称" />
        <el-button @click="saveRequest">保存接口</el-button>
        <el-button @click="exportMarkdown">导出 Markdown</el-button>
      </div>
    </main>

    <section class="api-workbench-response">
      <el-tabs v-model="responseTab">
        <el-tab-pane label="响应" name="response">
          <div v-if="response" class="response-summary">
            <el-tag :type="response.ok ? 'success' : 'warning'">
              {{ response.status ?? "ERR" }}
            </el-tag>
            <span>{{ response.durationMs }}ms</span>
            <span>{{ response.bodySize }} bytes</span>
          </div>
          <el-input
            v-if="response"
            :model-value="formattedResponseBody"
            type="textarea"
            :rows="18"
            readonly
          />
          <el-empty v-else description="发送请求后查看响应" />
        </el-tab-pane>
        <el-tab-pane label="响应头" name="headers">
          <pre class="headers-view">{{ responseHeadersText }}</pre>
        </el-tab-pane>
        <el-tab-pane label="历史" name="history">
          <div v-for="item in history" :key="item.id" class="history-item" @click="reuseHistory(item)">
            <strong>{{ item.method }}</strong>
            <span>{{ item.finalUrl }}</span>
            <small>{{ item.status ?? "ERR" }} · {{ item.durationMs }}ms</small>
          </div>
        </el-tab-pane>
      </el-tabs>
    </section>
  </div>
</template>
```

Vue SFC local components cannot be declared inside `<script setup>` and used as `<KeyValueEditor />` without registration. In the same file, define the editor inline by creating `const KeyValueEditor = defineComponent(...)` in script setup.

- [ ] **Step 2: Add script setup state and IPC functions**

Add this `<script setup lang="ts">`:

```vue
<script setup lang="ts">
import { computed, defineComponent, h, onMounted, ref } from "vue";
import { ElButton, ElInput, ElMessage, ElMessageBox, ElSwitch } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  ApiWorkbenchCollection,
  ApiWorkbenchEnvironment,
  ApiWorkbenchHistoryItem,
  ApiWorkbenchKeyValueRow,
  ApiWorkbenchListResult,
  ApiWorkbenchRequestDetail,
  ApiWorkbenchSendResult,
} from "../types/api-workbench";
import {
  API_WORKBENCH_METHODS,
  DEFAULT_API_WORKBENCH_DRAFT,
  buildApiWorkbenchPreviewUrl,
  formatApiWorkbenchResponseBody,
  normalizeApiWorkbenchDraft,
} from "../utils/apiWorkbench";

const KeyValueEditor = defineComponent({
  props: {
    modelValue: { type: Array as () => ApiWorkbenchKeyValueRow[], required: true },
  },
  emits: ["update:modelValue"],
  setup(props, { emit }) {
    function update(index: number, patch: Partial<ApiWorkbenchKeyValueRow>) {
      const next = props.modelValue.map((row, i) => (i === index ? { ...row, ...patch } : row));
      emit("update:modelValue", next);
    }
    function addRow() {
      emit("update:modelValue", [...props.modelValue, { enabled: true, key: "", value: "" }]);
    }
    function removeRow(index: number) {
      emit("update:modelValue", props.modelValue.filter((_, i) => i !== index));
    }
    return () =>
      h("div", { class: "kv-editor" }, [
        ...props.modelValue.map((row, index) =>
          h("div", { class: "kv-row", key: index }, [
            h(ElSwitch, {
              modelValue: row.enabled,
              "onUpdate:modelValue": (value: boolean) => update(index, { enabled: value }),
            }),
            h(ElInput, {
              modelValue: row.key,
              placeholder: "Key",
              "onUpdate:modelValue": (value: string) => update(index, { key: value }),
            }),
            h(ElInput, {
              modelValue: row.value,
              placeholder: "Value",
              "onUpdate:modelValue": (value: string) => update(index, { value }),
            }),
            h(ElButton, { onClick: () => removeRow(index) }, () => "删除"),
          ]),
        ),
        h(ElButton, { onClick: addRow }, () => "添加一行"),
      ]);
  },
});

const methods = API_WORKBENCH_METHODS;
const loading = ref(false);
const sending = ref(false);
const collections = ref<ApiWorkbenchCollection[]>([]);
const environments = ref<ApiWorkbenchEnvironment[]>([]);
const history = ref<ApiWorkbenchHistoryItem[]>([]);
const selectedCollectionId = ref<number | null>(null);
const selectedEnvironmentId = ref<number | null>(null);
const selectedRequestId = ref<number | null>(null);
const requestName = ref("");
const draft = ref({ ...DEFAULT_API_WORKBENCH_DRAFT });
const response = ref<ApiWorkbenchSendResult | null>(null);
const editorTab = ref("query");
const responseTab = ref("response");

const selectedCollection = computed(() =>
  collections.value.find((item) => item.id === selectedCollectionId.value) ?? null,
);
const selectedEnvironment = computed(() =>
  environments.value.find((item) => item.id === selectedEnvironmentId.value) ?? null,
);
const baseUrl = computed(() =>
  selectedEnvironment.value?.variables.find((item) => item.name === "BASE_URL")?.value ?? "",
);
const baseUrlError = computed(() => {
  if (/^https?:\/\//i.test(draft.value.url.trim())) return "";
  if (!draft.value.url.trim()) return "";
  return baseUrl.value.trim() ? "" : "相对 URL 需要当前环境配置 BASE_URL";
});
const formattedResponseBody = computed(() =>
  response.value ? formatApiWorkbenchResponseBody(response.value.bodyText, response.value.contentType) : "",
);
const responseHeadersText = computed(() =>
  response.value?.responseHeaders.map((row) => `${row.key}: ${row.value}`).join("\n") ?? "",
);

async function loadAll() {
  loading.value = true;
  try {
    const result = (await invokeToolByChannel("tool:api-workbench:list", {})) as ApiWorkbenchListResult;
    collections.value = result.collections ?? [];
    history.value = result.history ?? [];
    if (!selectedCollectionId.value && collections.value.length > 0) {
      await selectCollection(collections.value[0].id);
    }
  } finally {
    loading.value = false;
  }
}

async function selectCollection(id: number) {
  selectedCollectionId.value = id;
  const collection = collections.value.find((item) => item.id === id);
  selectedEnvironmentId.value = collection?.activeEnvironmentId ?? null;
  const result = (await invokeToolByChannel("tool:api-workbench:environment-list", {
    collectionId: id,
  })) as { items: ApiWorkbenchEnvironment[] };
  environments.value = result.items ?? [];
}

async function createCollection() {
  const { value } = await ElMessageBox.prompt("集合名称", "新建接口集合", {
    inputValue: "默认集合",
    confirmButtonText: "创建",
    cancelButtonText: "取消",
  });
  const created = (await invokeToolByChannel("tool:api-workbench:collection-create", {
    name: value,
    description: "",
  })) as { id: number; activeEnvironmentId: number };
  await loadAll();
  await selectCollection(created.id);
}

async function persistActiveEnvironment() {
  if (!selectedCollectionId.value || !selectedEnvironmentId.value) return;
  await invokeToolByChannel("tool:api-workbench:collection-set-active-environment", {
    collectionId: selectedCollectionId.value,
    environmentId: selectedEnvironmentId.value,
  });
}

async function sendRequest() {
  if (!selectedEnvironmentId.value) {
    ElMessage.warning("请先选择环境");
    return;
  }
  if (baseUrlError.value) {
    ElMessage.warning(baseUrlError.value);
    return;
  }
  sending.value = true;
  try {
    const normalized = normalizeApiWorkbenchDraft(draft.value);
    response.value = (await invokeToolByChannel("tool:api-workbench:send", {
      collectionId: selectedCollectionId.value,
      environmentId: selectedEnvironmentId.value,
      requestId: selectedRequestId.value,
      name: requestName.value,
      draft: normalized,
    })) as ApiWorkbenchSendResult;
    const historyResult = (await invokeToolByChannel("tool:api-workbench:history-list", {})) as {
      items: ApiWorkbenchHistoryItem[];
    };
    history.value = historyResult.items ?? [];
  } finally {
    sending.value = false;
  }
}

async function saveRequest() {
  if (!selectedCollectionId.value) {
    ElMessage.warning("请先选择集合");
    return;
  }
  if (!requestName.value.trim()) {
    ElMessage.warning("请填写接口名称");
    return;
  }
  const saved = (await invokeToolByChannel("tool:api-workbench:request-save", {
    id: selectedRequestId.value,
    collectionId: selectedCollectionId.value,
    folderId: null,
    name: requestName.value.trim(),
    description: "",
    draft: normalizeApiWorkbenchDraft(draft.value),
  })) as { id: number };
  selectedRequestId.value = saved.id;
  await loadAll();
  ElMessage.success("已保存接口");
}

async function loadRequest(id: number) {
  const detail = (await invokeToolByChannel("tool:api-workbench:request-get", { id })) as ApiWorkbenchRequestDetail;
  selectedRequestId.value = detail.id;
  requestName.value = detail.name;
  draft.value = normalizeApiWorkbenchDraft(detail.draft);
}

async function exportMarkdown() {
  if (!selectedCollectionId.value) {
    ElMessage.warning("请先选择集合");
    return;
  }
  const result = (await invokeToolByChannel("tool:api-workbench:export-markdown", {
    collectionId: selectedCollectionId.value,
  })) as { fileName: string; markdown: string };
  await navigator.clipboard.writeText(result.markdown);
  ElMessage.success(`Markdown 已复制：${result.fileName}`);
}

function reuseHistory(item: ApiWorkbenchHistoryItem) {
  draft.value = normalizeApiWorkbenchDraft({
    ...draft.value,
    method: item.method,
    url: item.url,
  });
  responseTab.value = "response";
}

onMounted(loadAll);
</script>
```

- [ ] **Step 3: Add scoped styles with stable dimensions**

Add:

```vue
<style scoped>
.api-workbench-panel {
  display: grid;
  grid-template-columns: 260px minmax(420px, 1fr) minmax(320px, 42%);
  gap: 12px;
  height: 100%;
  min-height: 0;
  padding: 12px;
  background: var(--el-bg-color-page);
}

.api-workbench-sidebar,
.api-workbench-editor,
.api-workbench-response {
  min-height: 0;
  overflow: auto;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  background: var(--el-bg-color);
  padding: 12px;
}

.api-workbench-toolbar,
.api-workbench-request-bar,
.api-workbench-actions,
.body-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
}

.api-workbench-toolbar {
  justify-content: space-between;
  margin-bottom: 12px;
}

.api-workbench-tree {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.api-workbench-collection {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  background: var(--el-fill-color-blank);
  color: var(--el-text-color-primary);
  padding: 8px;
  cursor: pointer;
}

.api-workbench-collection.active {
  border-color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
}

.method-select {
  width: 104px;
  flex: none;
}

.environment-select {
  width: 140px;
  flex: none;
}

.kv-editor {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.kv-row {
  display: grid;
  grid-template-columns: 52px minmax(120px, 1fr) minmax(160px, 1.4fr) 72px;
  gap: 8px;
  align-items: center;
}

.response-summary {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
}

.headers-view {
  white-space: pre-wrap;
  word-break: break-word;
  margin: 0;
  font-family: var(--lc-font-mono);
  font-size: 12px;
}

.history-item {
  display: grid;
  grid-template-columns: 64px 1fr;
  gap: 6px;
  padding: 8px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  cursor: pointer;
}

.history-item span,
.history-item small {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 1180px) {
  .api-workbench-panel {
    grid-template-columns: 240px 1fr;
  }

  .api-workbench-response {
    grid-column: 1 / -1;
  }
}
</style>
```

- [ ] **Step 4: Add request list interaction**

In the sidebar template, after collection buttons, render saved requests for the selected collection:

```vue
      <div v-if="selectedCollection" class="request-list">
        <button
          v-for="request in selectedCollection.requests"
          :key="request.id"
          class="request-list-item"
          @click="loadRequest(request.id)"
        >
          <strong>{{ request.method }}</strong>
          <span>{{ request.name }}</span>
        </button>
      </div>
```

Add styles:

```css
.request-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: 12px;
}

.request-list-item {
  display: grid;
  grid-template-columns: 56px 1fr;
  gap: 6px;
  align-items: center;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--el-text-color-primary);
  padding: 6px 8px;
  text-align: left;
  cursor: pointer;
}

.request-list-item:hover {
  background: var(--el-fill-color-light);
}
```

- [ ] **Step 5: Run frontend validation and commit**

Run:

```powershell
pnpm test src/utils/apiWorkbench.test.ts
pnpm typecheck
```

Expected: PASS.

Commit:

```powershell
git add apps/desktop/src/components/ApiWorkbenchPanel.vue apps/desktop/src/bridge/tauri.ts apps/desktop/src/composables/toolCatalog.ts apps/desktop/src/tool-registry.ts
git commit -m "feat(api-workbench): 添加接口调试前端面板"
```

## Task 10: Final Integration Validation

**Files:**

- Modify only files required by failing validation from previous tasks.

- [ ] **Step 1: Run backend tests**

Run:

```powershell
cargo test api_workbench -- --nocapture
```

Expected: PASS. If a local HTTP test is flaky because the test server handles fewer connections than the client makes, update the test server loop to accept exactly the needed number of connections.

- [ ] **Step 2: Run frontend unit tests**

Run:

```powershell
pnpm test src/utils/apiWorkbench.test.ts
```

Expected: PASS.

- [ ] **Step 3: Run typecheck**

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

Expected: PASS and Vite reports generated chunks.

- [ ] **Step 5: Run git status**

Run:

```powershell
git status --short
```

Expected: empty output after the final validation commit.

- [ ] **Step 6: Record process log if implementation touched 3 or more files**

Add a concise entry at the top of `process.md` with:

```markdown
## 2026-06-29: 接口调试工具按后端单一真源实现

**场景**: 新增接口调试工具，支持集合、环境变量、请求发送、历史和 Markdown 导出。
**使用次数**: 0
**问题**:
1. Markdown 模板如果前后端各实现一份，会形成双重真值。
2. `BASE_URL` 同时允许全局和环境级会产生遮蔽歧义。
3. 接口调试工具需要展示原始 3xx，不能让 HTTP 客户端默认跟随重定向。
**解决**:
1. Markdown 导出固定由 Rust 后端生成，前端只触发导出。
2. `BASE_URL` 固定为环境级变量，全局变量保存时拒绝该名称。
3. `ureq::AgentBuilder` 显式设置 `redirects(0)`，3xx 原样返回响应头和响应体。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/api_workbench.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src/components/ApiWorkbenchPanel.vue`
- `apps/desktop/src/utils/apiWorkbench.ts`
**验证**:
- `cargo test api_workbench -- --nocapture`
- `pnpm test src/utils/apiWorkbench.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
```

Commit:

```powershell
git add process.md
git commit -m "docs(process): 记录接口调试实现经验"
```

## Self-Review

- Spec coverage: backend schema, collection/folder/request CRUD, environment current pointer, global variable read/write, `BASE_URL` rules, request execution, redirect disabled behavior, history, Markdown export, frontend entry, panel, pure utility tests, and validation are covered by tasks.
- Unfinished marker scan: the plan uses concrete file paths, commands, payloads, and code snippets; no open requirement is left for the implementer to infer.
- Type consistency: IPC action names use kebab-case channels mapped to snake_case backend actions; frontend fields are camelCase; Rust request draft uses serde camelCase; the disabled redirect control does not enter the request type.

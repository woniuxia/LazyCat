# Request Forwarding Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build an offline LazyCat tool that persists and concurrently runs one-to-one HTTP, TCP, and UDP forwarding rules with explicit runtime state, bounded logs, statistics, and startup restoration.

**Architecture:** Add a `request_forward` Rust domain with repository, validation, runtime manager, and isolated HTTP/TCP/UDP handlers. The manager owns one dedicated Tokio multi-thread runtime and is the sole source of actual runtime state; SQLite stores rule configuration, `auto_start`, cumulative statistics, and bounded logs. A standalone Vue panel manages stopped rules, starts/stops services through the existing tool channel, and displays protocol-specific status and logs.

**Tech Stack:** Tauri 2, Rust 2021, Tokio, Hyper 1, Hyper-Util, Hyper-Rustls, Tokio-Util, Rusqlite, Vue 3, TypeScript, Element Plus, Vitest.

---

## Preconditions

- Design source: `docs/superpowers/specs/2026-07-13-request-forwarding-design.md`
- Work directly in the current repository unless the user explicitly requests isolation; project rules prefer no worktree by default.
- Use TDD for every behavior-bearing unit. Run the narrowest test first, then the relevant Rust/frontend suites.
- Do not start `pnpm dev` unless the user explicitly requests UI runtime testing.
- This task touches more than three files. After implementation and validation, add a concise reusable entry to `process.md`.
- Before each commit, run `git status --short` and stage only files belonging to that task.

## Planned File Map

Backend:

- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Modify: `apps/desktop/src-tauri/src/main.rs`
- Modify: `apps/desktop/src-tauri/src/tools/helpers.rs`
- Modify: `apps/desktop/src-tauri/src/tools/mod.rs`
- Modify: `apps/desktop/src-tauri/src/tools/contract_tests.rs`
- Create: `apps/desktop/src-tauri/src/tools/request_forward/mod.rs`
- Create: `apps/desktop/src-tauri/src/tools/request_forward/model.rs`
- Create: `apps/desktop/src-tauri/src/tools/request_forward/repository.rs`
- Create: `apps/desktop/src-tauri/src/tools/request_forward/validation.rs`
- Create: `apps/desktop/src-tauri/src/tools/request_forward/runtime.rs`
- Create: `apps/desktop/src-tauri/src/tools/request_forward/http.rs`
- Create: `apps/desktop/src-tauri/src/tools/request_forward/tcp.rs`
- Create: `apps/desktop/src-tauri/src/tools/request_forward/udp.rs`
- Create: `apps/desktop/src-tauri/src/tools/request_forward/observability.rs`

Frontend:

- Modify: `apps/desktop/src/composables/toolCatalog.ts`
- Modify: `apps/desktop/src/tool-registry.ts`
- Modify: `apps/desktop/src/bridge/tauri.ts`
- Modify: `apps/desktop/src/types/index.ts`
- Create: `apps/desktop/src/types/request-forward.ts`
- Create: `apps/desktop/src/utils/requestForward.ts`
- Create: `apps/desktop/src/utils/requestForward.test.ts`
- Create: `apps/desktop/src/components/RequestForwardPanel.vue`
- Create: `apps/desktop/src/components/RequestForwardPanel.test.ts`
- Create: `apps/desktop/src/components/request-forward/RequestForwardRuleList.vue`
- Create: `apps/desktop/src/components/request-forward/RequestForwardRuleForm.vue`
- Create: `apps/desktop/src/components/request-forward/RequestForwardLogList.vue`

Documentation:

- Modify: `process.md`

### Task 1: Add the request-forward domain contract and async networking dependencies

**Files:**

- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Modify: `apps/desktop/src-tauri/src/tools/mod.rs`
- Modify: `apps/desktop/src-tauri/src/tools/contract_tests.rs`
- Create: `apps/desktop/src-tauri/src/tools/request_forward/mod.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`

**Step 1: Write the failing action-contract expectation**

Add `request_forward` to `DOMAINS` in `contract_tests.rs`, then add the fourteen channel rows to `CHANNEL_MAP`:

```ts
"tool:request-forward:list": { domain: "request_forward", action: "list" },
"tool:request-forward:get": { domain: "request_forward", action: "get" },
"tool:request-forward:create": { domain: "request_forward", action: "create" },
"tool:request-forward:update": { domain: "request_forward", action: "update" },
"tool:request-forward:delete": { domain: "request_forward", action: "delete" },
"tool:request-forward:start": { domain: "request_forward", action: "start" },
"tool:request-forward:stop": { domain: "request_forward", action: "stop" },
"tool:request-forward:start-all": { domain: "request_forward", action: "start_all" },
"tool:request-forward:stop-all": { domain: "request_forward", action: "stop_all" },
"tool:request-forward:status": { domain: "request_forward", action: "status" },
"tool:request-forward:log-list": { domain: "request_forward", action: "log_list" },
"tool:request-forward:log-clear": { domain: "request_forward", action: "log_clear" },
"tool:request-forward:stats-get": { domain: "request_forward", action: "stats_get" },
"tool:request-forward:stats-reset": { domain: "request_forward", action: "stats_reset" },
```

**Step 2: Run the contract test to verify it fails**

Run:

```powershell
cargo test contract_tests::channel_map_actions_are_supported_by_backend -- --nocapture
```

Working directory: `apps/desktop/src-tauri`

Expected: FAIL because `request_forward` is not registered by `supported_actions`.

**Step 3: Add the minimal domain skeleton**

Create `request_forward/mod.rs`:

```rust
use serde_json::Value;

pub const ACTIONS: &[&str] = &[
    "list", "get", "create", "update", "delete",
    "start", "stop", "start_all", "stop_all", "status",
    "log_list", "log_clear", "stats_get", "stats_reset",
];

pub fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, _payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported request_forward action: {action}"));
    }
    Err(format!("request_forward action not implemented: {action}"))
}
```

Register `pub mod request_forward`, dispatch `"request_forward"`, and expose it from `supported_actions` in `tools/mod.rs`.

Update dependencies:

```toml
bytes = "1"
http-body-util = "0.1"
hyper = { version = "1", features = ["http1", "server"] }
hyper-util = { version = "0.1", features = ["client", "client-legacy", "http1", "server", "tokio"] }
hyper-rustls = { version = "0.27", features = ["http1", "native-tokio", "tls12"] }
tokio = { version = "1", features = ["rt", "rt-multi-thread", "macros", "net", "io-util", "sync", "time"] }
tokio-util = { version = "0.7", features = ["rt"] }
```

Use the existing workspace lockfile resolution; do not pin duplicate transitive TLS crates manually.

**Step 4: Run contract and compile checks**

Run:

```powershell
cargo test contract_tests::channel_map_actions_are_supported_by_backend -- --nocapture
cargo check
```

Expected: contract test PASS and `cargo check` PASS.

**Step 5: Commit**

```powershell
git add apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/Cargo.lock apps/desktop/src-tauri/src/tools/mod.rs apps/desktop/src-tauri/src/tools/contract_tests.rs apps/desktop/src-tauri/src/tools/request_forward/mod.rs apps/desktop/src/bridge/tauri.ts
git commit -m "feat(request-forward): 建立工具域与通道契约"
```

### Task 2: Add SQLite schema, models, validation, and rule CRUD

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/helpers.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/mod.rs`
- Create: `apps/desktop/src-tauri/src/tools/request_forward/model.rs`
- Create: `apps/desktop/src-tauri/src/tools/request_forward/repository.rs`
- Create: `apps/desktop/src-tauri/src/tools/request_forward/validation.rs`

**Step 1: Write failing schema and validation tests**

Under `request_forward` tests, create an in-memory connection helper that calls a new `ensure_request_forward_schema_for_test(&Connection)` and assert:

```rust
#[test]
fn schema_creates_rules_stats_logs_and_cascade() {
    let conn = test_conn();
    let rule = repository::create_rule(&conn, valid_http_input()).unwrap();
    repository::insert_test_log(&conn, rule.id).unwrap();
    repository::delete_rule(&conn, rule.id).unwrap();
    assert_eq!(repository::count_logs(&conn, rule.id).unwrap(), 0);
}

#[test]
fn validation_rejects_protocol_field_mismatch_and_self_forward() {
    assert!(validation::validate_rule_input(&invalid_tcp_with_url()).is_err());
    assert!(validation::validate_rule_input(&http_self_forward()).is_err());
}
```

Also cover:

- bind host must be an IP literal;
- ports must be `1..=65535`;
- HTTP target allows only `http/https`, optional path, no query/fragment;
- HTTP Base URL normalization removes trailing slash;
- TCP/UDP require target host and port;
- `create/update` DTO has no `auto_start` field.

**Step 2: Run tests to verify failure**

Run:

```powershell
cargo test request_forward::tests::schema_ -- --nocapture
cargo test request_forward::tests::validation_ -- --nocapture
```

Expected: FAIL because schema/repository/validation do not exist.

**Step 3: Implement schema and domain models**

Add the three tables and indexes from the approved design to `helpers.rs`. Extract an internal helper so production initialization and in-memory tests use the same SQL:

```rust
fn ensure_request_forward_schema(conn: &Connection) -> Result<(), String> { ... }

#[cfg(test)]
pub(crate) fn ensure_request_forward_schema_for_test(conn: &Connection) -> Result<(), String> {
    ensure_request_forward_schema(conn)
}
```

Define explicit models:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForwardProtocol { Http, Tcp, Udp }

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleWriteInput {
    pub name: String,
    pub protocol: ForwardProtocol,
    pub bind_host: String,
    pub listen_port: u16,
    pub target_url: Option<String>,
    pub target_host: Option<String>,
    pub target_port: Option<u16>,
    pub capture_http_headers: bool,
    pub capture_http_body: bool,
}
```

Keep `auto_start` only on stored/read models.

**Step 4: Implement CRUD actions minimally**

Implement `list/get/create/update/delete` with these invariants:

- create starts stopped and `auto_start = false`;
- update/delete call a temporary runtime-state hook that currently reports stopped; Task 4 replaces it with the real manager;
- failed/stopped may update/delete; running/starting/stopping may not;
- all SQL uses parameters;
- create inserts the stats row in the same transaction;
- delete relies on foreign-key cascade.

**Step 5: Run narrow tests**

Run:

```powershell
cargo test request_forward -- --nocapture
```

Expected: schema, validation, normalization, CRUD, cascade tests PASS.

**Step 6: Commit**

```powershell
git add apps/desktop/src-tauri/src/tools/helpers.rs apps/desktop/src-tauri/src/tools/request_forward
git commit -m "feat(request-forward): 添加规则存储与校验"
```

### Task 3: Implement HTTP helper semantics and bounded log capture

**Files:**

- Create: `apps/desktop/src-tauri/src/tools/request_forward/http.rs`
- Create: `apps/desktop/src-tauri/src/tools/request_forward/observability.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/mod.rs`

**Step 1: Write failing pure helper tests**

Add tests before networking code:

```rust
#[test]
fn joins_base_path_and_inbound_uri_without_reinterpreting_query() {
    let target = build_target_uri("https://example.com/api", "/users?x=1").unwrap();
    assert_eq!(target.to_string(), "https://example.com/api/users?x=1");
}

#[test]
fn rebuilds_forwarded_headers_without_trusting_client_chain() {
    let headers = rebuild_forward_headers(client_headers(), "127.0.0.1", "demo.local").unwrap();
    assert_eq!(headers["x-forwarded-for"], "127.0.0.1");
    assert!(!headers["x-forwarded-for"].to_str().unwrap().contains("spoofed"));
}

#[test]
fn capture_masks_secrets_and_truncates_text_only() {
    let capture = capture_http_preview(text_headers_with_authorization(), vec![b'a'; 70 * 1024]);
    assert_eq!(capture.headers["authorization"], "[REDACTED]");
    assert_eq!(capture.body.unwrap().len(), 64 * 1024);
    assert!(capture.truncated);
}
```

Cover all approved semantics:

- Base URL query/fragment rejection;
- hop-by-hop request/response header removal, including names declared by `Connection`;
- Host replacement;
- `Forwarded` and `X-Forwarded-*` deletion and rebuild;
- secret masking for Authorization, Proxy-Authorization, Cookie, Set-Cookie;
- binary and non-identity Content-Encoding skip preview;
- 64 KiB per-side truncation without limiting forwarded bytes.

**Step 2: Run tests to verify failure**

Run:

```powershell
cargo test request_forward::http::tests -- --nocapture
cargo test request_forward::observability::tests -- --nocapture
```

Expected: FAIL because helpers are missing.

**Step 3: Implement pure helpers**

Implement small functions with explicit inputs:

```rust
pub fn build_target_uri(base: &Url, inbound: &Uri) -> Result<Uri, String>;
pub fn strip_hop_by_hop(headers: &mut HeaderMap);
pub fn rebuild_forward_headers(
    headers: &mut HeaderMap,
    client_ip: IpAddr,
    original_host: Option<&HeaderValue>,
) -> Result<(), String>;
pub fn should_capture_body(headers: &HeaderMap) -> bool;
pub fn redact_headers(headers: &HeaderMap) -> Vec<(String, String)>;
```

Implement a `PreviewTap` that observes chunks up to 64 KiB while passing every chunk through unchanged. Do not introduce full-body collection in forwarding paths.

**Step 4: Run helper tests**

Run:

```powershell
cargo test request_forward::http::tests -- --nocapture
cargo test request_forward::observability::tests -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```powershell
git add apps/desktop/src-tauri/src/tools/request_forward/http.rs apps/desktop/src-tauri/src/tools/request_forward/observability.rs apps/desktop/src-tauri/src/tools/request_forward/mod.rs
git commit -m "feat(request-forward): 定义 HTTP 与日志边界语义"
```

### Task 4: Build the runtime manager and exact state/compensation semantics

**Files:**

- Create: `apps/desktop/src-tauri/src/tools/request_forward/runtime.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/mod.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/repository.rs`

**Step 1: Write failing state-machine tests**

Use a fake protocol runner and injectable repository failure points. Cover:

```rust
#[test]
fn failed_state_has_no_live_task_and_allows_update_delete() { ... }

#[test]
fn start_persist_failure_stops_new_runtime_before_returning_error() { ... }

#[test]
fn stop_persist_failure_restarts_old_config_before_returning_error() { ... }

#[test]
fn double_compensation_failure_reports_runtime_truth() { ... }

#[test]
fn same_rule_operations_are_serialized_and_start_is_idempotent() { ... }
```

The test response must expose both the primary and compensation error when both fail.

**Step 2: Run tests to verify failure**

Run:

```powershell
cargo test request_forward::runtime::tests -- --nocapture
```

Expected: FAIL because manager/state transitions are missing.

**Step 3: Implement the manager**

Use one process-global manager initialized through `OnceLock`:

```rust
pub struct ForwardManager {
    runtime: tokio::runtime::Runtime,
    instances: Mutex<HashMap<i64, RuntimeEntry>>,
    rule_locks: Mutex<HashMap<i64, Arc<tokio::sync::Mutex<()>>>>,
}
```

Do not hold the global `instances` mutex across bind, database IO, task join, or cancellation wait. Each runtime entry contains the immutable rule snapshot, state, cancellation token, join handle, counters, and last errors.

Provide synchronous action wrappers because `tool_execute` is synchronous:

```rust
pub fn start_rule(rule_id: i64, mode: StartMode) -> Result<RuntimeSummary, String>;
pub fn stop_rule(rule_id: i64, persist_auto_start: bool) -> Result<RuntimeSummary, String>;
pub fn status(rule_id: Option<i64>) -> Result<Value, String>;
```

Use `runtime.block_on` only outside Tokio worker threads. The request-forward manager owns its dedicated runtime, so calls from Tauri command workers remain explicit and isolated.

**Step 4: Connect CRUD guards and batch actions**

Replace the Task 2 temporary state hook with real manager status. Implement:

- `start/stop` with the approved compensation ordering;
- `start_all/stop_all` returning `{ results: [{ ruleId, ok, error, state }] }`;
- `status` for one/all rules;
- failed as a cleaned terminal state;
- no automatic retry loop.

Use a fake runner until protocol tasks are added; the manager tests must already pass.

**Step 5: Run state tests**

Run:

```powershell
cargo test request_forward::runtime::tests -- --nocapture
cargo test request_forward -- --nocapture
```

Expected: PASS.

**Step 6: Commit**

```powershell
git add apps/desktop/src-tauri/src/tools/request_forward
git commit -m "feat(request-forward): 添加运行状态管理器"
```

### Task 5: Implement TCP forwarding with half-close, overload, and cancellation

**Files:**

- Create: `apps/desktop/src-tauri/src/tools/request_forward/tcp.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/runtime.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/observability.rs`

**Step 1: Write failing local-socket integration tests**

Bind all tests to `127.0.0.1:0`. Add:

```rust
#[test]
fn tcp_forwards_both_directions_and_preserves_half_close() { ... }

#[test]
fn tcp_downstream_failure_only_closes_current_client() { ... }

#[test]
fn tcp_overload_closes_new_connection_and_keeps_existing_connections() { ... }

#[test]
fn stopping_tcp_rule_closes_listener_and_existing_connections() { ... }
```

Assert upload/download byte counters and one accepted connection event.

**Step 2: Run tests to verify failure**

Run:

```powershell
cargo test request_forward::tcp::tests -- --nocapture
```

Expected: FAIL because TCP runner is missing.

**Step 3: Implement TCP runner**

Use `TcpListener`, `TcpStream`, a rule-level `Semaphore`, `CancellationToken`, and `tokio::io::copy_bidirectional` or an equivalent explicit split-copy implementation that preserves half-close semantics.

Required behavior:

- stop accepting on cancellation;
- register every child task so stop waits for cleanup;
- acquire permits without unbounded queueing;
- close and log overload immediately;
- resolve/connect downstream per client connection;
- never fail the listener because one downstream connection fails.

**Step 4: Run TCP tests**

Run:

```powershell
cargo test request_forward::tcp::tests -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```powershell
git add apps/desktop/src-tauri/src/tools/request_forward/tcp.rs apps/desktop/src-tauri/src/tools/request_forward/runtime.rs apps/desktop/src-tauri/src/tools/request_forward/observability.rs
git commit -m "feat(request-forward): 实现 TCP 双向转发"
```

### Task 6: Implement UDP per-client sessions and bounded cleanup

**Files:**

- Create: `apps/desktop/src-tauri/src/tools/request_forward/udp.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/runtime.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/observability.rs`

**Step 1: Write failing UDP integration tests**

Add local UDP echo services and cover:

```rust
#[test]
fn udp_keeps_responses_isolated_between_two_clients() { ... }

#[test]
fn udp_event_count_means_client_datagrams_received() { ... }

#[test]
fn udp_reclaims_idle_sessions() { ... }

#[test]
fn udp_drops_new_client_when_session_limit_is_reached() { ... }

#[test]
fn stopping_udp_rule_closes_listener_and_sessions() { ... }
```

Use test-only short idle timeouts and small session limits through a `UdpLimits` struct; production uses named constants.

**Step 2: Run tests to verify failure**

Run:

```powershell
cargo test request_forward::udp::tests -- --nocapture
```

Expected: FAIL because UDP runner is missing.

**Step 3: Implement UDP runner**

Maintain `HashMap<SocketAddr, Session>` where each session owns a connected downstream UDP socket and response task. Update `last_active` on client and downstream traffic. A periodic cleanup task removes idle sessions.

Required behavior:

- one downstream socket per client address;
- no cross-client response delivery;
- existing sessions continue at the cap;
- first datagram from a new client is dropped at the cap and logged;
- `event_count` increments for every client datagram received, including a dropped-overload datagram;
- stop cancels listener, cleanup task, and every session.

**Step 4: Run UDP tests**

Run:

```powershell
cargo test request_forward::udp::tests -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```powershell
git add apps/desktop/src-tauri/src/tools/request_forward/udp.rs apps/desktop/src-tauri/src/tools/request_forward/runtime.rs apps/desktop/src-tauri/src/tools/request_forward/observability.rs
git commit -m "feat(request-forward): 实现 UDP 会话转发"
```

### Task 7: Implement streaming HTTP/HTTPS forwarding and SSE

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/request_forward/http.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/runtime.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/observability.rs`

**Step 1: Write failing local HTTP integration tests**

Create local Hyper test servers and cover:

```rust
#[test]
fn http_forwards_method_path_query_headers_and_streaming_body() { ... }

#[test]
fn http_filters_hop_headers_and_rebuilds_forward_chain() { ... }

#[test]
fn http_returns_502_for_connect_or_tls_failure() { ... }

#[test]
fn http_returns_504_for_timeout_before_response_starts() { ... }

#[test]
fn http_returns_503_without_queueing_when_concurrency_is_full() { ... }

#[test]
fn http_streams_sse_without_waiting_for_completion() { ... }

#[test]
fn http_rejects_websocket_upgrade_explicitly() { ... }

#[test]
fn https_downstream_works_with_test_trust_roots() { ... }
```

For HTTPS, inject a test client connector/root store. Production uses native roots; tests must not depend on the public internet.

**Step 2: Run tests to verify failure**

Run:

```powershell
cargo test request_forward::http::tests -- --nocapture
```

Expected: helper tests pass, forwarding integration tests FAIL.

**Step 3: Implement the HTTP runner**

Use Hyper HTTP/1 server connections and Hyper-Util legacy client with Hyper-Rustls. Keep bodies streaming:

```rust
type ProxyBody = http_body_util::combinators::BoxBody<Bytes, BoxError>;
```

Wrap request and response bodies with the bounded preview tap only when capture is enabled. Do not call `collect()` on arbitrary forwarded bodies.

Required behavior:

- request permit uses non-queued acquisition;
- 503 on overload;
- 502 on DNS/connect/TLS failure;
- 504 on configured pre-response timeout;
- after response headers start, stream errors terminate the body and log the error;
- SSE has no total-response timeout;
- Upgrade gets an explicit non-success response and no downstream connection;
- cancellation stops listener and active requests.

**Step 4: Run HTTP tests**

Run:

```powershell
cargo test request_forward::http::tests -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```powershell
git add apps/desktop/src-tauri/src/tools/request_forward/http.rs apps/desktop/src-tauri/src/tools/request_forward/runtime.rs apps/desktop/src-tauri/src/tools/request_forward/observability.rs
git commit -m "feat(request-forward): 实现 HTTP 流式转发"
```

### Task 8: Persist bounded logs and protocol-specific statistics

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/request_forward/observability.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/repository.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/mod.rs`

**Step 1: Write failing repository and degradation tests**

Cover:

```rust
#[test]
fn log_insert_keeps_latest_1000_rows_per_rule() { ... }

#[test]
fn log_clear_does_not_reset_stats_and_stats_reset_does_not_clear_logs() { ... }

#[test]
fn log_query_filters_before_stable_pagination() { ... }

#[test]
fn observability_write_failure_keeps_forwarding_and_exposes_last_error() { ... }
```

Also assert protocol-specific event labels/values and that failed DB writes do not create an unbounded in-memory retry queue.

**Step 2: Run tests to verify failure**

Run:

```powershell
cargo test request_forward::observability::tests -- --nocapture
cargo test request_forward::repository::tests -- --nocapture
```

Expected: FAIL on persistence/query actions.

**Step 3: Implement actions and bounded persistence**

Implement:

- `log_list` with rule ID, keyword, success/error filter, cursor/offset and limit;
- SQL filtering before `ORDER BY created_at DESC, id DESC` and pagination;
- `log_clear` per rule;
- `stats_get` merging persisted totals with unflushed in-memory deltas without double counting;
- `stats_reset` synchronizing with the rule counter before resetting DB and memory;
- periodic/terminal stat flush;
- `last_observability_error` on runtime summary;
- bounded error summary only, no unbounded pending-write queue.

**Step 4: Run request-forward tests**

Run:

```powershell
cargo test request_forward -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```powershell
git add apps/desktop/src-tauri/src/tools/request_forward
git commit -m "feat(request-forward): 持久化转发日志与统计"
```

### Task 9: Wire startup restoration and application shutdown

**Files:**

- Modify: `apps/desktop/src-tauri/src/main.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/mod.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/runtime.rs`

**Step 1: Write failing lifecycle tests**

Keep lifecycle decisions testable outside Tauri setup:

```rust
#[test]
fn restore_attempts_every_auto_start_rule_and_isolates_failures() { ... }

#[test]
fn restore_failure_keeps_auto_start_true_and_failed_runtime_state() { ... }

#[test]
fn app_shutdown_stops_runtime_without_changing_auto_start() { ... }
```

Use an injectable repository and fake runner.

**Step 2: Run tests to verify failure**

Run:

```powershell
cargo test request_forward::runtime::tests::restore_ -- --nocapture
cargo test request_forward::runtime::tests::app_shutdown_ -- --nocapture
```

Expected: FAIL because lifecycle functions are absent.

**Step 3: Implement lifecycle entry points**

Expose:

```rust
pub fn initialize_manager() -> Result<(), String>;
pub fn restore_auto_start_rules() -> Result<Vec<RestoreResult>, String>;
pub fn on_app_exit();
```

In `main.rs`:

- initialize the manager during `.setup` after database access is available;
- launch restoration without blocking initial window display;
- log each failed restore explicitly;
- in `RunEvent::ExitRequested`, call `request_forward::on_app_exit()` before/alongside widget cleanup;
- do not change `auto_start` during process shutdown.

Document in code why the manager is process-global rather than Tauri-managed state: existing tool dispatch is synchronous and global; one dedicated runtime avoids coupling every action to `AppHandle`.

**Step 4: Run lifecycle tests and compile**

Run:

```powershell
cargo test request_forward -- --nocapture
cargo check
```

Expected: PASS.

**Step 5: Commit**

```powershell
git add apps/desktop/src-tauri/src/main.rs apps/desktop/src-tauri/src/tools/request_forward
git commit -m "feat(request-forward): 接入自动恢复与退出清理"
```

### Task 10: Add frontend types, pure utilities, catalog entry, and registry

**Files:**

- Create: `apps/desktop/src/types/request-forward.ts`
- Modify: `apps/desktop/src/types/index.ts`
- Create: `apps/desktop/src/utils/requestForward.ts`
- Create: `apps/desktop/src/utils/requestForward.test.ts`
- Modify: `apps/desktop/src/composables/toolCatalog.ts`
- Modify: `apps/desktop/src/tool-registry.ts`

**Step 1: Write failing utility tests**

Add tests for:

```ts
it("formats protocol-specific event labels", () => {
  expect(getForwardEventLabel("http")).toBe("请求数");
  expect(getForwardEventLabel("tcp")).toBe("连接数");
  expect(getForwardEventLabel("udp")).toBe("数据报数");
});

it("detects exposed bind addresses including IPv6 wildcard", () => {
  expect(isExposedForwardBindHost("127.0.0.1")).toBe(false);
  expect(isExposedForwardBindHost("::1")).toBe(false);
  expect(isExposedForwardBindHost("0.0.0.0")).toBe(true);
  expect(isExposedForwardBindHost("::")).toBe(true);
});

it("builds write payload without autoStart", () => { ... });
it("keeps running forms readonly and failed forms editable", () => { ... });
it("formats IPv6 endpoints with brackets", () => { ... });
```

Also test default forms, protocol-specific required fields, endpoint summaries, batch result messages, and log status tone.

**Step 2: Run tests to verify failure**

Run:

```powershell
pnpm test src/utils/requestForward.test.ts
```

Expected: FAIL because files/functions are missing.

**Step 3: Implement types and pure utilities**

Define frontend interfaces matching backend camelCase JSON. Keep write DTO separate:

```ts
export interface RequestForwardRuleWriteInput {
  name: string;
  protocol: RequestForwardProtocol;
  bindHost: string;
  listenPort: number;
  targetUrl: string | null;
  targetHost: string | null;
  targetPort: number | null;
  captureHttpHeaders: boolean;
  captureHttpBody: boolean;
}
```

Do not include `autoStart` in the write DTO.

Register the catalog item under “网络与系统”:

```ts
{ id: "request-forward", name: "请求转发", desc: "HTTP、TCP 与 UDP 本地端口转发" },
```

Register the async component in `tool-registry.ts`.

**Step 4: Run utility tests and typecheck**

Run:

```powershell
pnpm test src/utils/requestForward.test.ts
pnpm typecheck
```

Expected: PASS.

**Step 5: Commit**

```powershell
git add apps/desktop/src/types/request-forward.ts apps/desktop/src/types/index.ts apps/desktop/src/utils/requestForward.ts apps/desktop/src/utils/requestForward.test.ts apps/desktop/src/composables/toolCatalog.ts apps/desktop/src/tool-registry.ts
git commit -m "feat(request-forward): 添加前端模型与工具入口"
```

### Task 11: Build the rule list and stopped-state editor

**Files:**

- Create: `apps/desktop/src/components/RequestForwardPanel.vue`
- Create: `apps/desktop/src/components/RequestForwardPanel.test.ts`
- Create: `apps/desktop/src/components/request-forward/RequestForwardRuleList.vue`
- Create: `apps/desktop/src/components/request-forward/RequestForwardRuleForm.vue`

**Step 1: Write the failing component structure test**

Follow the repository's source-structure Vitest pattern:

```ts
const source = readFileSync(new URL("./RequestForwardPanel.vue", import.meta.url), "utf8");
const formSource = readFileSync(
  new URL("./request-forward/RequestForwardRuleForm.vue", import.meta.url),
  "utf8",
);

it("keeps running rules readonly and exposes stop-and-edit", () => {
  expect(source).toContain("停止并编辑");
  expect(formSource).toContain(":disabled=\"readonly\"");
});

it("separates save from save-and-start", () => {
  expect(source).toContain("仅保存");
  expect(source).toContain("保存并启动");
});
```

Also assert single/all start-stop controls and exposed-listener warning.

**Step 2: Run test to verify failure**

Run:

```powershell
pnpm test src/components/RequestForwardPanel.test.ts
```

Expected: FAIL because components do not exist.

**Step 3: Implement minimal panel orchestration**

Use `useToolInvoke` for user-triggered operations. Implement:

- initial `list` load;
- selection preserved after refresh when possible;
- new stopped draft;
- create/update via “仅保存”;
- create/update followed by `start` via “保存并启动”;
- start/stop and start-all/stop-all;
- stop-and-edit;
- update/delete blocked while actual state is starting/running/stopping;
- delete confirmation and no silent delete after stop failure;
- exposed bind warning for non-loopback addresses;
- protocol-specific form fields;
- protocol immutable after persistence.

Do not poll aggressively. Use a modest status refresh interval only while at least one rule is starting/running/stopping, and clear it on unmount.

**Step 4: Run component test and typecheck**

Run:

```powershell
pnpm test src/components/RequestForwardPanel.test.ts src/utils/requestForward.test.ts
pnpm typecheck
```

Expected: PASS.

**Step 5: Commit**

```powershell
git add apps/desktop/src/components/RequestForwardPanel.vue apps/desktop/src/components/RequestForwardPanel.test.ts apps/desktop/src/components/request-forward/RequestForwardRuleList.vue apps/desktop/src/components/request-forward/RequestForwardRuleForm.vue
git commit -m "feat(request-forward): 添加规则管理界面"
```

### Task 12: Add statistics and log browsing UI

**Files:**

- Modify: `apps/desktop/src/components/RequestForwardPanel.vue`
- Create: `apps/desktop/src/components/request-forward/RequestForwardLogList.vue`
- Modify: `apps/desktop/src/components/RequestForwardPanel.test.ts`

**Step 1: Extend the failing component test**

Assert:

- protocol-specific “请求数 / 连接数 / 数据报数” label;
- upload/download/error cards;
- HTTP expandable headers/body preview;
- TCP/UDP payload is never rendered;
- keyword and success/error filters;
- clear-log confirmation;
- observability warning text.

**Step 2: Run test to verify failure**

Run:

```powershell
pnpm test src/components/RequestForwardPanel.test.ts
```

Expected: FAIL on missing log/stats UI.

**Step 3: Implement log and stats flow**

Implement:

- `stats_get` on selection and status refresh;
- `log_list` with debounced keyword and backend filters;
- stable pagination/load-more;
- HTTP detail expansion with masked headers and truncated markers;
- TCP/UDP summary-only rows;
- `log_clear` and `stats_reset` as separate confirmed operations;
- visible `lastObservabilityError` warning without marking the forwarding service stopped.

Use existing clean light styling and scoped component CSS. Teleported dialogs/messages must not rely on panel-local CSS variables.

**Step 4: Run frontend validation**

Run:

```powershell
pnpm test src/components/RequestForwardPanel.test.ts src/utils/requestForward.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

Expected: all PASS.

**Step 5: Commit**

```powershell
git add apps/desktop/src/components/RequestForwardPanel.vue apps/desktop/src/components/RequestForwardPanel.test.ts apps/desktop/src/components/request-forward/RequestForwardLogList.vue
git commit -m "feat(request-forward): 展示转发日志与统计"
```

### Task 13: Run cross-layer verification and record reusable engineering knowledge

**Files:**

- Modify: `process.md`
- Potential generated update: `apps/desktop/src/components.d.ts` only if the build legitimately changes it

**Step 1: Run targeted backend tests**

Run from `apps/desktop/src-tauri`:

```powershell
cargo test request_forward -- --nocapture
cargo test contract_tests -- --nocapture
```

Expected: all request-forward and contract tests PASS.

**Step 2: Run the full Rust suite**

Run:

```powershell
cargo test
```

Expected: PASS. If a real-socket test flakes, fix its readiness synchronization; do not add blind sleep or ignore the failure.

**Step 3: Run frontend tests**

Run from repository root:

```powershell
pnpm test src/utils/requestForward.test.ts src/components/RequestForwardPanel.test.ts
pnpm test
```

Expected: targeted and full frontend tests PASS.

**Step 4: Run type and build validation**

Run:

```powershell
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

Expected: PASS.

**Step 5: Perform a non-UI local smoke test through Rust integration coverage**

Confirm the integration tests prove this minimum chain without starting the desktop UI:

```text
HTTP client -> local forward -> HTTP(S) test upstream -> streamed response
TCP client  -> local forward -> TCP echo upstream    -> half-close response
UDP clients -> local forward -> UDP echo upstream    -> isolated responses
```

Expected: all three chains pass in automated tests.

**Step 6: Record the stable implementation pattern**

Add one concise entry to `process.md` covering only reusable facts discovered during implementation, for example:

- dedicated Tokio runtime ownership behind synchronous Tauri tool dispatch;
- runtime truth vs persisted auto-start expectation and compensation;
- streaming preview taps that never buffer the full body;
- UDP per-client session isolation.

Do not copy the whole design or implementation log.

**Step 7: Inspect final diff**

Run:

```powershell
git status --short
git diff --check
git diff --stat
```

Expected: only request-forward implementation, required generated declarations, and `process.md` are changed; no `.superpowers/brainstorm` files are tracked.

**Step 8: Commit final verification/documentation changes**

```powershell
git add process.md
git add apps/desktop/src/components.d.ts
git commit -m "docs: 记录请求转发实现经验"
```

If `components.d.ts` did not change, omit it. If `process.md` contains no genuinely reusable new information, do not manufacture an entry; instead make the final commit only for legitimate generated changes, or skip this commit.

## Completion Criteria

The implementation is complete only when all of the following are true:

1. Multiple HTTP/TCP/UDP rules can bind different local ports concurrently.
2. HTTP streams to HTTP/HTTPS downstream, preserves the approved URI/header semantics, supports SSE, and rejects WebSocket.
3. TCP supports bidirectional transfer, half-close, overload rejection, and stop-time connection cancellation.
4. UDP isolates responses per client, reclaims sessions, and enforces the session cap.
5. Actual runtime state never comes from SQLite alone.
6. Start/stop persistence failures execute and test the specified compensation behavior.
7. Failed rules own no live listener/tasks and can be edited/deleted.
8. Logs are masked, bounded to 1000 rows per rule, and optional body previews are text-only and capped at 64 KiB per side.
9. Statistics use HTTP request, TCP connection, and UDP client-datagram semantics.
10. Startup restoration isolates per-rule failures and application exit preserves `auto_start`.
11. The frontend keeps running rules readonly and stopped/failed rules editable.
12. Targeted tests, full relevant suites, typecheck, and web build pass.


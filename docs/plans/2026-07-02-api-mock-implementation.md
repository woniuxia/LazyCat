# API Mock Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a standalone API Mock tool with persisted projects/routes/files, manually controlled local HTTP services, CORS, file responses, and recent runtime logs.

**Architecture:** Add a normal LazyCat tool wired through `toolCatalog`, `tool-registry`, `bridge/tauri.ts`, and Rust `tools/mod.rs`. Persist configuration in SQLite via `helpers.rs`, while running services and request logs stay in a process-local registry keyed by project ID. Keep route validation/matching and file path safety enforced in Rust, with TypeScript utilities only for UI feedback and derived state.

**Tech Stack:** Tauri 2, Vue 3, TypeScript, Element Plus, Vitest, Rust std `TcpListener`, rusqlite, serde_json.

---

### Task 1: Frontend Types And Pure Functions

**Files:**

- Create: `apps/desktop/src/types/api-mock.ts`
- Modify: `apps/desktop/src/types/index.ts`
- Create: `apps/desktop/src/utils/apiMock.ts`
- Create: `apps/desktop/src/utils/apiMock.test.ts`

**Step 1: Write failing tests**

Cover:

- `validateMockPathPattern` accepts exact, parameter, wildcard patterns and rejects invalid ones.
- `validateMockCorsConfig` rejects `allowCredentials=true` with `allowOrigin="*"`.
- `normalizeMockHeaderRows` trims keys, drops blank disabled rows, preserves enabled values.
- `deriveMockProjectRuntimeState` returns `stopped`, `running`, `error`, `restart-required`.
- `isMockProjectRestartRequired` compares persisted config snapshot and running snapshot.
- `getMockRouteSpecificityLabel` returns exact/parameter/wildcard labels.

Run: `pnpm test src/utils/apiMock.test.ts`
Expected: FAIL because `apiMock.ts` does not exist.

**Step 2: Implement minimal utilities and types**

Add API Mock project, route, file, CORS, header row, runtime status and log types. Implement pure functions with no Vue dependencies.

**Step 3: Verify**

Run: `pnpm test src/utils/apiMock.test.ts`
Expected: PASS.

### Task 2: Rust Schema, Validation, Matching, And File Safety Tests

**Files:**

- Create: `apps/desktop/src-tauri/src/tools/api_mock.rs`
- Modify: `apps/desktop/src-tauri/src/tools/helpers.rs`
- Modify: `apps/desktop/src-tauri/src/tools/mod.rs`

**Step 1: Write failing Rust tests**

Cover:

- schema creates `api_mock_projects`, `api_mock_files`, `api_mock_routes`.
- path pattern validation for exact/param/wildcard and invalid patterns.
- matching priority exact > param > wildcard, same level by `sort_order`, then `id`.
- method filtering and 404.
- CORS config validation and header generation.
- file import copies into `<dataDir>/api-mock/files/`.
- file path validation rejects paths outside API Mock files directory.

Run: `cargo test api_mock -- --nocapture`
Expected: FAIL because module/schema are not implemented.

**Step 2: Implement schema and pure backend logic**

Add `API_MOCK_SCHEMA_SQL`, validation structs, route pattern parser/matcher, CORS parser/header builder, file directory helper and controlled file import helper. Register module in `mod.rs` and schema in `helpers.rs`.

**Step 3: Verify**

Run: `cargo test api_mock -- --nocapture`
Expected: PASS for the new backend pure/storage tests.

### Task 3: Rust CRUD IPC Actions

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/api_mock.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`

**Step 1: Write failing Rust action tests**

Cover:

- `project_create/list/update/reorder/delete`.
- `route_save/list/get/reorder/delete`.
- route save validates method, status code, CORS and path pattern.
- route/project delete cleans unreferenced file records and leaves shared files intact.
- unsupported action returns explicit `unsupported api_mock action`.

Run: `cargo test api_mock -- --nocapture`
Expected: FAIL on missing actions.

**Step 2: Implement actions**

Implement supported action dispatch, SQLite reads/writes, JSON serialization, warning collection for file cleanup failures, and bridge channel mappings for all `tool:api-mock:*` channels.

**Step 3: Verify**

Run: `cargo test api_mock -- --nocapture`
Expected: PASS.

### Task 4: Runtime HTTP Services And Smoke Tests

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/api_mock.rs`

**Step 1: Write failing runtime tests**

Cover:

- service refuses to start with no enabled routes.
- service start/stop updates runtime status.
- static route returns status, content type, custom header and body.
- file route returns bytes and content length.
- missing file copy returns `500` and records log.
- OPTIONS preflight returns `204` with CORS headers.
- multiple projects can run on different ports.
- occupied port start fails explicitly.

Run: `cargo test api_mock -- --nocapture`
Expected: FAIL on missing runtime.

**Step 2: Implement runtime registry and HTTP server**

Use a process-local `OnceLock<Mutex<HashMap<i64, RunningMockService>>>`. Each service owns a stop flag, a thread handle, immutable route snapshot, and bounded recent logs. Implement simple HTTP request parsing sufficient for method/path/header line, static/file response writing, CORS headers, stop polling, and runtime status/log actions. Do not persist runtime state.

**Step 3: Verify**

Run: `cargo test api_mock -- --nocapture`
Expected: PASS, including local HTTP smoke coverage.

### Task 5: Frontend Tool Registration And Panel

**Files:**

- Modify: `apps/desktop/src/composables/toolCatalog.ts`
- Modify: `apps/desktop/src/tool-registry.ts`
- Create: `apps/desktop/src/components/ApiMockPanel.vue`

**Step 1: Build panel against existing IPC contract**

Implement a three-column, light, utilitarian panel:

- left project list with host/port, runtime badge, start/stop buttons;
- middle route list with method, pattern, status, response kind and enabled state;
- right project/route editor plus recent logs.

Support create/update/delete/reorder at a basic usable level, file selection via Tauri dialog followed by `file-import`, route-level CORS fields, and explicit “需重启生效” when running snapshot differs from current config.

**Step 2: Keep UI logic thin**

Use `apiMock.ts` functions for path/CORS/header validation and runtime labels. Component only coordinates state, IPC calls and Element Plus UI.

**Step 3: Verify frontend tests**

Run: `pnpm test src/utils/apiMock.test.ts`
Expected: PASS.

### Task 6: Final Validation And Process Note

**Files:**

- Modify: `process.md` if the implementation yields reusable project experience.

**Step 1: Run required validation**

Run:

- `cargo test api_mock -- --nocapture`
- `pnpm test src/utils/apiMock.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

Expected: all PASS.

**Step 2: Review scope**

Check `git diff --stat` and confirm changes are limited to API Mock and required shared registries/schema. Do not modify browser-profiles files.

**Step 3: Record process note**

If the implementation touches 3+ files and yields reusable guidance, add a concise `process.md` entry describing API Mock runtime-state versus persisted-state separation and local HTTP smoke-test strategy.

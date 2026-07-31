# Remove API and Database Workbenches Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 完整移除无人使用的接口调试和数据库工作台，保留 API Mock、共享工具与已有磁盘数据，并清理专属依赖和历史文档。

**Architecture:** 先从前端目录和 IPC 契约移除两个工具，使产品入口与类型依赖收敛；再删除 Rust 域、schema 初始化和数据目录联动，随后移除只由数据库工作台使用的 Rust 依赖。最后删除专属历史文档并精确修订共享文档，使用全仓搜索和全量测试证明没有活跃引用或共享能力回归。

**Tech Stack:** Vue 3、TypeScript、Vitest、Tauri 2、Rust、rusqlite、pnpm、Cargo。

## Global Constraints

- 保留 API Mock 的前后端、测试、文档和 `process.md` 记录。
- 保留 HTTP 状态码、SQL 转实体、SQL 格式化、Vault 数据库凭据、Monaco 和 JSON 树等独立或共享能力。
- 不执行 `DROP TABLE`，不删除 `db-key`、接口响应缓存目录或 `user_settings` 遗留值。
- 新版本不再创建或读写 `api_workbench_*`、`db_connections`、`db_saved_queries`、`db_query_history`。
- 不增加替代功能、外部工具启动器或导出迁移流程。
- 不启动 `pnpm dev`，不自动打开产品 UI。
- 所有删除使用 `apply_patch`，不使用递归删除命令。
- 执行前检查工作区；任何不属于本计划的用户修改均不得改写、暂存或提交。
- 本计划基于设计文档 `docs/superpowers/specs/2026-07-13-remove-api-db-workbenches-design.md`。

---

## File Map

### 保留并修改

- `apps/desktop/src/composables/toolCatalog.ts`：移除数据库分组和接口调试入口，保留网络组中的 API Mock。
- `apps/desktop/src/tool-registry.ts`：移除两个异步组件注册。
- `apps/desktop/src/bridge/tauri.ts`：移除 `tool:api-workbench:*` 与 `tool:db:*` 通道。
- `apps/desktop/src/types/index.ts`：移除接口调试类型再导出。
- `apps/desktop/src-tauri/src/tools/mod.rs`：移除两个 Rust 域注册、分发与测试契约入口。
- `apps/desktop/src-tauri/src/tools/helpers.rs`：停止初始化两个功能的数据表；保留 API Mock schema 初始化。
- `apps/desktop/src-tauri/src/tools/settings.rs`：数据目录迁移不再复制 `db-key`。
- `apps/desktop/src-tauri/src/tools/vault.rs`：将只因数据库工作台而公开到 crate 的 AES 常量和函数恢复为模块私有。
- `apps/desktop/src-tauri/Cargo.toml`、`apps/desktop/src-tauri/Cargo.lock`：移除数据库驱动专属依赖。
- `process.md`：删除两个功能的专属历史章节，并清除共享章节中的过期引用。
- 共享设计/计划文档：只移除接口调试和数据库工作台相关段落，保留其他主题。

### 新增

- `apps/desktop/src/composables/toolCatalog.test.ts`：防止已下线工具重新进入目录，并保护 API Mock 仍存在。

### 删除

- 接口调试前端、测试和 Rust 目录。
- 数据库工作台前端、测试、Rust 模块和驱动目录。
- 仅描述两个已下线功能的旧设计文档和实施计划。

---

### Task 1: 移除前端入口、IPC 契约和专属前端代码

**Files:**

- Create: `apps/desktop/src/composables/toolCatalog.test.ts`
- Modify: `apps/desktop/src/composables/toolCatalog.ts`
- Modify: `apps/desktop/src/tool-registry.ts`
- Modify: `apps/desktop/src/bridge/tauri.ts`
- Modify: `apps/desktop/src/types/index.ts`
- Delete: `apps/desktop/src/components/ApiWorkbenchPanel.vue`
- Delete: `apps/desktop/src/components/ApiWorkbenchContextMenu.vue`
- Delete: `apps/desktop/src/components/ApiWorkbenchCurlImportDialog.vue`
- Delete: `apps/desktop/src/components/ApiWorkbenchKeyValueEditor.vue`
- Delete: `apps/desktop/src/components/ApiWorkbenchResponseViewer.vue`
- Delete: `apps/desktop/src/components/ApiWorkbenchSidebar.vue`
- Delete: `apps/desktop/src/components/ApiWorkbenchTabsBar.vue`
- Delete: `apps/desktop/src/components/ApiWorkbenchVariablePopover.vue`
- Delete: `apps/desktop/src/composables/useApiWorkbenchTabs.ts`
- Delete: `apps/desktop/src/types/api-workbench.ts`
- Delete: `apps/desktop/src/utils/apiWorkbench.ts`
- Delete: `apps/desktop/src/utils/apiWorkbench.test.ts`
- Delete: `apps/desktop/src/utils/apiWorkbenchCurl.ts`
- Delete: `apps/desktop/src/utils/apiWorkbenchCurl.test.ts`
- Delete: `apps/desktop/src/utils/apiWorkbenchHeaders.ts`
- Delete: `apps/desktop/src/utils/apiWorkbenchHistory.ts`
- Delete: `apps/desktop/src/utils/apiWorkbenchHistory.test.ts`
- Delete: `apps/desktop/src/utils/apiWorkbenchKvPaste.ts`
- Delete: `apps/desktop/src/utils/apiWorkbenchKvPaste.test.ts`
- Delete: `apps/desktop/src/utils/apiWorkbenchResponsePreview.ts`
- Delete: `apps/desktop/src/utils/apiWorkbenchResponsePreview.test.ts`
- Delete: `apps/desktop/src/utils/apiWorkbenchSearch.ts`
- Delete: `apps/desktop/src/utils/apiWorkbenchSearch.test.ts`
- Delete: `apps/desktop/src/utils/apiWorkbenchTabs.ts`
- Delete: `apps/desktop/src/utils/apiWorkbenchTabs.test.ts`
- Delete: `apps/desktop/src/utils/apiWorkbenchTree.ts`
- Delete: `apps/desktop/src/utils/apiWorkbenchTree.test.ts`
- Delete: `apps/desktop/src/utils/apiWorkbenchVariables.ts`
- Delete: `apps/desktop/src/utils/apiWorkbenchVariables.test.ts`
- Delete: `apps/desktop/src/components/DbWorkbenchPanel.vue`
- Delete: `apps/desktop/src/components/db/DbConnectionDialog.vue`
- Delete: `apps/desktop/src/components/db/DbRedisBrowser.vue`
- Delete: `apps/desktop/src/components/db/DbResultGrid.vue`
- Delete: `apps/desktop/src/components/db/DbSqlEditor.vue`
- Delete: `apps/desktop/src/components/db/DbSqlWorkspace.vue`
- Delete: `apps/desktop/src/components/db/DbTableStructure.vue`
- Delete: `apps/desktop/src/composables/useDbConnections.ts`
- Delete: `apps/desktop/src/types/db.ts`
- Delete: `apps/desktop/src/utils/dbGridChanges.ts`
- Delete: `apps/desktop/src/utils/dbGridChanges.test.ts`
- Delete: `apps/desktop/src/utils/dbRedisKeyTree.ts`
- Delete: `apps/desktop/src/utils/dbRedisKeyTree.test.ts`
- Delete: `apps/desktop/src/utils/dbSqlClassify.ts`
- Delete: `apps/desktop/src/utils/dbSqlClassify.test.ts`

**Interfaces:**

- Consumes: `getSidebarItems()`、`getAllTools()`、`isRealToolId()` 的现有公开接口。
- Produces: 不包含 `api-workbench`、`db-workbench` 和空数据库分组的工具目录；仍包含 `api-mock`。

- [ ] **Step 1: 写目录退场回归测试**

创建 `apps/desktop/src/composables/toolCatalog.test.ts`：

```ts
import { describe, expect, it } from "vitest";
import { getAllTools, getSidebarItems, isRealToolId } from "./toolCatalog";

describe("toolCatalog retired workbenches", () => {
  it("removes retired workbenches while keeping API Mock", () => {
    const toolIds = getAllTools().map((tool) => tool.id);
    const groupIds = getSidebarItems()
      .filter((item) => item.kind === "group")
      .map((item) => item.group.id);

    expect(toolIds).not.toContain("api-workbench");
    expect(toolIds).not.toContain("db-workbench");
    expect(groupIds).not.toContain("database");
    expect(toolIds).toContain("api-mock");
    expect(isRealToolId("api-workbench")).toBe(false);
    expect(isRealToolId("db-workbench")).toBe(false);
    expect(isRealToolId("api-mock")).toBe(true);
  });
});
```

- [ ] **Step 2: 运行测试并确认当前目录仍暴露两个工具**

Run:

```powershell
pnpm test src/composables/toolCatalog.test.ts
```

Workdir: `apps/desktop`

Expected: FAIL，至少出现 `api-workbench`、`db-workbench` 或 `database` 仍存在的断言失败。

- [ ] **Step 3: 从工具目录和组件注册中移除两个入口**

在 `toolCatalog.ts` 中删除整个 `database` 分组，并从网络组中只删除以下接口调试条目：

```ts
{ id: "api-workbench", name: "接口调试", desc: "离线 HTTP 接口调试与文档生成" },
```

网络组中的 `network`、`api-mock`、`dns` 及其余既有工具保持原顺序不变。

在 `tool-registry.ts` 中删除以下两项，其他注册不动：

```ts
"db-workbench": defineAsyncComponent(() => import("./components/DbWorkbenchPanel.vue")),
"api-workbench": defineAsyncComponent(() => import("./components/ApiWorkbenchPanel.vue")),
```

- [ ] **Step 4: 移除 bridge 通道和接口调试类型再导出**

从 `apps/desktop/src/bridge/tauri.ts` 删除所有 key 前缀为以下值的映射：

```ts
"tool:api-workbench:";
"tool:db:";
```

保留紧邻的 API Mock 映射和其他域映射。删除完成后，相邻结构应直接从 API Mock 进入 DNS 等其他域，不保留空注释或占位。

从 `apps/desktop/src/types/index.ts` 删除完整的接口调试导出块：

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
  ApiWorkbenchHistoryRequestSnapshot,
  ApiWorkbenchExecutedRequestSnapshot,
  ApiWorkbenchHistoryItem,
  ApiWorkbenchHistoryDetail,
  ApiWorkbenchListResult,
} from "./api-workbench";
```

- [ ] **Step 5: 使用 apply_patch 删除全部专属前端文件**

按本任务 `Files` 中的 Delete 清单逐个删除。不要删除以下共享文件：

```text
apps/desktop/src/components/ApiMockPanel.vue
apps/desktop/src/components/api-mock/**
apps/desktop/src/components/JsonTreeViewer.vue
apps/desktop/src/components/MonacoEditor.vue
apps/desktop/src/utils/jsonProcessTree.ts
apps/desktop/src/utils/apiMock.ts
apps/desktop/src/types/api-mock.ts
```

- [ ] **Step 6: 运行前端针对性验证**

Run:

```powershell
pnpm test src/composables/toolCatalog.test.ts
pnpm typecheck
```

第一条命令 Workdir 为 `apps/desktop`；第二条命令 Workdir 为仓库根目录。

Expected: 新目录测试 PASS；类型检查成功且没有已删除组件、类型或工具函数的 import 错误。

- [ ] **Step 7: 检查前端残留并提交**

Run:

```powershell
rg -n "api-workbench|api_workbench|ApiWorkbench|db-workbench|DbWorkbench|tool:db:" apps/desktop/src
```

Expected: 无输出。`api-mock`、Vault 数据库类型和通用 `db` 文案不属于残留。

Commit:

```powershell
git add apps/desktop/src/composables/toolCatalog.test.ts apps/desktop/src/composables/toolCatalog.ts apps/desktop/src/tool-registry.ts apps/desktop/src/bridge/tauri.ts apps/desktop/src/types/index.ts
git add -u -- apps/desktop/src/components/ApiWorkbenchPanel.vue apps/desktop/src/components/ApiWorkbenchContextMenu.vue apps/desktop/src/components/ApiWorkbenchCurlImportDialog.vue apps/desktop/src/components/ApiWorkbenchKeyValueEditor.vue apps/desktop/src/components/ApiWorkbenchResponseViewer.vue apps/desktop/src/components/ApiWorkbenchSidebar.vue apps/desktop/src/components/ApiWorkbenchTabsBar.vue apps/desktop/src/components/ApiWorkbenchVariablePopover.vue apps/desktop/src/composables/useApiWorkbenchTabs.ts apps/desktop/src/types/api-workbench.ts apps/desktop/src/utils/apiWorkbench.ts apps/desktop/src/utils/apiWorkbench.test.ts apps/desktop/src/utils/apiWorkbenchCurl.ts apps/desktop/src/utils/apiWorkbenchCurl.test.ts apps/desktop/src/utils/apiWorkbenchHeaders.ts apps/desktop/src/utils/apiWorkbenchHistory.ts apps/desktop/src/utils/apiWorkbenchHistory.test.ts apps/desktop/src/utils/apiWorkbenchKvPaste.ts apps/desktop/src/utils/apiWorkbenchKvPaste.test.ts apps/desktop/src/utils/apiWorkbenchResponsePreview.ts apps/desktop/src/utils/apiWorkbenchResponsePreview.test.ts apps/desktop/src/utils/apiWorkbenchSearch.ts apps/desktop/src/utils/apiWorkbenchSearch.test.ts apps/desktop/src/utils/apiWorkbenchTabs.ts apps/desktop/src/utils/apiWorkbenchTabs.test.ts apps/desktop/src/utils/apiWorkbenchTree.ts apps/desktop/src/utils/apiWorkbenchTree.test.ts apps/desktop/src/utils/apiWorkbenchVariables.ts apps/desktop/src/utils/apiWorkbenchVariables.test.ts apps/desktop/src/components/DbWorkbenchPanel.vue apps/desktop/src/components/db apps/desktop/src/composables/useDbConnections.ts apps/desktop/src/types/db.ts apps/desktop/src/utils/dbGridChanges.ts apps/desktop/src/utils/dbGridChanges.test.ts apps/desktop/src/utils/dbRedisKeyTree.ts apps/desktop/src/utils/dbRedisKeyTree.test.ts apps/desktop/src/utils/dbSqlClassify.ts apps/desktop/src/utils/dbSqlClassify.test.ts
git commit -m "refactor(ui): 移除接口调试和数据库工作台"
```

提交前运行 `git diff --cached --name-only`，确认不包含 `apps/desktop/src-tauri/src/tools/convert.rs`。

---

### Task 2: 移除 Rust 域、schema 初始化和数据目录联动

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/mod.rs`
- Modify: `apps/desktop/src-tauri/src/tools/helpers.rs`
- Modify: `apps/desktop/src-tauri/src/tools/settings.rs`
- Modify: `apps/desktop/src-tauri/src/tools/vault.rs`
- Delete: `apps/desktop/src-tauri/src/tools/api_workbench/mod.rs`
- Delete: `apps/desktop/src-tauri/src/tools/api_workbench/collection.rs`
- Delete: `apps/desktop/src-tauri/src/tools/api_workbench/environment.rs`
- Delete: `apps/desktop/src-tauri/src/tools/api_workbench/executor.rs`
- Delete: `apps/desktop/src-tauri/src/tools/api_workbench/export.rs`
- Delete: `apps/desktop/src-tauri/src/tools/api_workbench/folder.rs`
- Delete: `apps/desktop/src-tauri/src/tools/api_workbench/helpers.rs`
- Delete: `apps/desktop/src-tauri/src/tools/api_workbench/history.rs`
- Delete: `apps/desktop/src-tauri/src/tools/api_workbench/request.rs`
- Delete: `apps/desktop/src-tauri/src/tools/api_workbench/response.rs`
- Delete: `apps/desktop/src-tauri/src/tools/api_workbench/types.rs`
- Delete: `apps/desktop/src-tauri/src/tools/db.rs`
- Delete: `apps/desktop/src-tauri/src/tools/db_drivers/mod.rs`
- Delete: `apps/desktop/src-tauri/src/tools/db_drivers/sql_text.rs`
- Delete: `apps/desktop/src-tauri/src/tools/db_drivers/mysql.rs`
- Delete: `apps/desktop/src-tauri/src/tools/db_drivers/kingbase.rs`
- Delete: `apps/desktop/src-tauri/src/tools/db_drivers/redis.rs`

**Interfaces:**

- Consumes: `execute_tool()` 和测试态 `supported_actions()` 的现有域分发结构。
- Produces: 不再识别 `api_workbench` 和 `db` 域；API Mock、Vault、settings 和其他域行为不变。

- [ ] **Step 1: 从 Rust 模块注册和域分发中删除两个域**

从 `tools/mod.rs` 删除：

```rust
pub mod api_workbench;
pub mod db;
pub mod db_drivers;
```

从 `dispatch_tool` 的 `match domain` 删除：

```rust
"api_workbench" => api_workbench::execute(action, payload),
"db" => db::execute(action, payload),
```

从测试态 `supported_actions` 删除：

```rust
"api_workbench" => Some(api_workbench::supported_actions()),
"db" => Some(db::supported_actions()),
```

保留 `api_mock` 三处注册/分发/契约入口。

- [ ] **Step 2: 停止创建接口调试和数据库工作台表**

从 `helpers.rs` 删除接口调试 schema 调用：

```rust
conn
    .execute_batch(super::api_workbench::API_WORKBENCH_SCHEMA_SQL)
    .map_err(|e| format!("create api workbench schema failed: {e}"))?;
```

删除从 `CREATE TABLE IF NOT EXISTS db_connections` 开始、以 `initialize db workbench tables failed` 错误映射结束的整个 `conn.execute_batch` 调用块。

删除后必须保留并相邻衔接：

```rust
conn
    .execute_batch(super::api_mock::API_MOCK_SCHEMA_SQL)
    .map_err(|e| format!("create api mock schema failed: {e}"))?;
```

以及其后的 snippet FTS、attachments 等现有初始化逻辑。不得新增任何 `DROP TABLE` 或旧表清理 SQL。

- [ ] **Step 3: 停止迁移 db-key，但不删除磁盘文件**

从 `settings.rs` 删除以下复制逻辑：

```rust
// Copy db-key（数据库工作台连接密码的本地加密密钥，缺失会导致已存密码无法解密）
let current_db_key = current_dir.join("db-key");
if current_db_key.is_file() {
    fs::copy(&current_db_key, target_path.join("db-key"))
        .map_err(|e| format!("copy db-key failed: {e}"))?;
}
```

不得加入 `remove_file`，也不得修改 SQLite、附件或 Hosts 备份的既有迁移流程。

- [ ] **Step 4: 收回 Vault 中仅为数据库工作台开放的可见性**

在 `vault.rs` 中只修改以下四个声明的可见性；参数、返回类型和函数体保持原样：

```rust
const KEY_LEN: usize = 32;
const IV_LEN: usize = 16;
fn aes256_encrypt(key: &[u8], iv: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String>
fn aes256_decrypt(key: &[u8], iv: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, String>
```

`supported_actions()` 仍需保持 `pub(crate)`，因为 `tools/mod.rs` 测试契约仍使用它。

- [ ] **Step 5: 使用 apply_patch 删除两个 Rust 域**

按本任务 Delete 清单删除 `api_workbench/` 下全部 11 个文件、`db.rs` 和 `db_drivers/` 下全部 5 个文件。不要删除：

```text
apps/desktop/src-tauri/src/tools/api_mock.rs
apps/desktop/src-tauri/src/tools/network.rs
apps/desktop/src-tauri/src/tools/vault.rs
apps/desktop/src-tauri/src/tools/helpers.rs
```

- [ ] **Step 6: 运行 Rust 结构验证**

Run:

```powershell
cargo check
cargo test api_mock -- --nocapture
cargo test vault -- --nocapture
cargo test settings -- --nocapture
```

Workdir: `apps/desktop/src-tauri`

Expected: 全部成功；不存在已删除模块 import、schema 常量或 `db-key` 复制引用。

- [ ] **Step 7: 检查后端残留并提交**

Run:

```powershell
rg -n "api_workbench|api-workbench|db_workbench|db-workbench|db_connections|db_saved_queries|db_query_history|db-key" apps/desktop/src-tauri/src
```

Expected: 无输出。此时 `Cargo.toml` 中依赖尚未删除，留给 Task 3。

Commit:

```powershell
git add apps/desktop/src-tauri/src/tools/mod.rs apps/desktop/src-tauri/src/tools/helpers.rs apps/desktop/src-tauri/src/tools/settings.rs apps/desktop/src-tauri/src/tools/vault.rs apps/desktop/src-tauri/src/tools/api_workbench apps/desktop/src-tauri/src/tools/db.rs apps/desktop/src-tauri/src/tools/db_drivers
git commit -m "refactor(tauri): 移除接口调试和数据库工作台后端"
```

---

### Task 3: 移除数据库驱动专属依赖并更新 Cargo.lock

**Files:**

- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Modify: `apps/desktop/src-tauri/Cargo.lock`

**Interfaces:**

- Consumes: Task 2 已删除的 `db` 和 `db_drivers` 模块。
- Produces: 不再直接依赖 `sqlx`、`rust_decimal`、`redis`、`futures-util` 的 Tauri 后端。

- [ ] **Step 1: 确认四个依赖没有剩余源码消费方**

Run:

```powershell
rg -n "sqlx::|rust_decimal|redis::|futures_util" apps/desktop/src-tauri/src
```

Expected: 无输出。若有输出，先判断是否为非工作台的真实消费方；存在真实消费方时停止，不得删除对应依赖。

- [ ] **Step 2: 从 Cargo.toml 删除专属直接依赖**

删除以下四行：

```toml
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "mysql", "postgres", "chrono", "rust_decimal", "uuid", "json"] }
rust_decimal = "1"
redis = { version = "0.27", default-features = false, features = ["tokio-comp", "acl"] }
futures-util = { version = "0.3", default-features = false }
```

保留 `uuid`、`chrono`、`tokio`、`ureq` 和其他仍被项目使用的依赖。

- [ ] **Step 3: 更新锁文件并验证依赖解析**

Run:

```powershell
cargo check
```

Workdir: `apps/desktop/src-tauri`

Expected: 成功，并由 Cargo 自动更新 `Cargo.lock`。

- [ ] **Step 4: 验证直接依赖已消失**

Run:

```powershell
Select-String -Path Cargo.toml -Pattern '^sqlx\s*=|^rust_decimal\s*=|^redis\s*=|^futures-util\s*='
cargo test
```

Workdir: `apps/desktop/src-tauri`

Expected: `Select-String` 无输出；`cargo test` 全量成功。不要要求 `futures-util` 必须从 `Cargo.lock` 完全消失，因为其他传递依赖可能继续使用它。

- [ ] **Step 5: 提交依赖清理**

```powershell
git add apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/Cargo.lock
git commit -m "chore(tauri): 移除数据库工作台驱动依赖"
```

---

### Task 4: 删除专属历史文档并精确清理共享记录

**Files:**

- Delete: `docs/plans/2026-06-30-api-workbench-navigation.md`
- Delete: `docs/plans/2026-07-11-api-workbench-request-bar-layout.md`
- Delete: `docs/plans/2026-07-12-api-workbench-meta-actions-nowrap.md`
- Delete: `docs/superpowers/plans/2026-06-29-api-workbench.md`
- Delete: `docs/superpowers/plans/2026-06-30-api-workbench-replay.md`
- Delete: `docs/superpowers/plans/2026-07-03-db-workbench.md`
- Delete: `docs/superpowers/specs/2026-06-29-api-workbench-design.md`
- Delete: `docs/superpowers/specs/2026-06-30-api-workbench-navigation-design.md`
- Delete: `docs/superpowers/specs/2026-06-30-api-workbench-personal-debugging-design.md`
- Delete: `docs/superpowers/specs/2026-06-30-api-workbench-replay-design.md`
- Delete: `docs/superpowers/specs/2026-07-01-api-workbench-response-preview-design.md`
- Delete: `docs/superpowers/specs/2026-07-02-db-workbench-design.md`
- Delete: `docs/superpowers/specs/2026-07-04-api-workbench-ux-design.md`
- Delete: `docs/superpowers/specs/2026-07-04-api-workbench-ux-plan.md`
- Delete: `docs/superpowers/specs/2026-07-11-api-workbench-request-bar-layout-design.md`
- Delete: `docs/superpowers/specs/2026-07-12-api-workbench-meta-actions-nowrap-design.md`
- Modify: `process.md`
- Modify: `docs/superpowers/specs/2026-07-04-json-tree-viewer-extensions-design.md`
- Modify: `docs/superpowers/specs/2026-07-04-json-tree-viewer-extensions-plan.md`
- Modify: `docs/superpowers/specs/2026-07-04-structure-refactor-roadmap-design.md`
- Rename: `docs/superpowers/plans/2026-07-04-structure-refactor-batch0-2-plan.md` -> `docs/superpowers/plans/2026-07-04-structure-refactor-batch0-1-plan.md`
- Modify: `docs/superpowers/plans/2026-07-04-structure-refactor-batch0-1-plan.md`
- Modify: `docs/superpowers/specs/2026-07-05-ipc-contract-cross-cutting-design.md`
- Modify: `docs/superpowers/plans/2026-07-05-ipc-contract-cross-cutting-plan.md`
- Keep: `docs/superpowers/specs/2026-07-13-remove-api-db-workbenches-design.md`
- Keep: `docs/superpowers/plans/2026-07-13-remove-api-db-workbenches.md`

**Interfaces:**

- Consumes: 用户确认的“删除旧设计文档和 process.md 历史记录”决策。
- Produces: 只有本次移除 spec/plan 仍说明两个工具；共享文档不再把它们当作现行或待实施能力。

- [ ] **Step 1: 使用 apply_patch 删除 16 份专属旧文档**

严格按 Delete 清单删除。不要删除任何文件名含 `api-mock` 的文档，也不要删除本次移除 spec/plan。

- [ ] **Step 2: 删除 process.md 的专属章节**

按完整二级标题边界删除以下章节，从标题开始删除到下一个 `## ` 标题之前：

```text
## 2026-07-01: API Workbench 环境管理弹窗用两栏结构承载多环境编辑
## 2026-07-01: 接口调试响应预览要分离文本与二进制存储
## 2026-06-30: 接口调试环境变量重复名要在提交前和后端同时校验
## 2026-06-30: 接口调试环境编辑入口收敛到管理弹窗
## 2026-06-30: 接口调试历史复现以执行快照为重放真源
## 2026-06-30: 接口调试个人闭环继续保持发送路径单一真源
## 2026-06-30: 接口调试导航树管理要以后端排序为真源
## 2026-06-30: 接口调试状态切换和变量解析要按实际执行路径验证
## 2026-06-29: 接口调试工具按后端单一真源实现
## 2026-07-03: 数据库工作台（MySQL/KingbaseES/Redis）两期落地
## 2026-07-11: 接口调试 UX 优化 Phase 2-4 实施（响应区/工作流/多标签/拖拽）
```

在文件开头的通用经验中，将以 `api_workbench` 为唯一例子的路径说明改成不依赖已删除模块的通用描述；不要删除仍适用于 Rust 子模块路径变化的规则。

对于 `## 2026-07-07: 结构治理批次 0-2 行为保持拆分` 共享章节：

- 标题改为 `## 2026-07-07: 结构治理批次 0-1 行为保持拆分`。
- 删除接口调试 Rust 巨型模块、目录化、测试对账和相关文件/验证描述。
- 保留 App e2e 守卫、PM 组件迁移和 PmPanel 拆分经验。

- [ ] **Step 3: 清理 JSON 树共享文档中的过期消费者**

在两份 JSON 树文档中：

- 删除 `api-workbench-ux-plan`、API 响应预览、Redis String、DB 结果单元格的专属引用。
- 将 1MB 体积阈值说明改为 JSON 处理自身约束，例如：

```markdown
进入树形的闸门：内容 `JSON.parse` 成功且 `text.length <= 1_000_000`；不满足时说明原因并停留文本模式。
```

- 保留数据字典、JSON 处理、JWT、CSV 等仍存在的消费方和全部通用 JSON 树设计。

- [ ] **Step 4: 将结构治理共享计划收敛到批次 0-1**

使用 `apply_patch` 完成文件重命名：

```text
docs/superpowers/plans/2026-07-04-structure-refactor-batch0-2-plan.md
-> docs/superpowers/plans/2026-07-04-structure-refactor-batch0-1-plan.md
```

在 roadmap 和重命名后的 plan 中：

- 删除 `api_workbench.rs` 候选、批次 2 详设、33 action、目录化步骤、测试对账和软目标。
- 将完成定义改为只覆盖仍存在的批次 0-1；不要留下“批次 2”空章节。
- 保留 App e2e 与 PM 前端治理内容。
- 若后续 Todo 批次编号依赖原批次 2，统一改称“后续批次”，避免历史编号出现断层。

- [ ] **Step 5: 清理 IPC 契约共享文档**

在 IPC contract design/plan 中：

- 工具域清单删除 `api_workbench` 和 `db`。
- 模块数量从 `40` 改为 `38`。
- 特例说明只保留 `api_mock`，删除 `is_supported_api_workbench_action`、路线图批次 2 和拆后路径协调内容。
- 保留通用 `supported_actions()` 契约、安全网和其他工具域计划。

调整后的特例文字应类似：

```markdown
- **api_mock**：沿用既有 supported-actions 先例并统一为通用 `ACTIONS` const + `supported_actions()` 形态；内嵌测试语义不变。
```

- [ ] **Step 6: 搜索文档残留**

Run:

```powershell
rg -n -i "api-workbench|api_workbench|db-workbench|数据库工作台" docs process.md
```

Expected: 只允许命中以下两份文件：

```text
docs/superpowers/specs/2026-07-13-remove-api-db-workbenches-design.md
docs/superpowers/plans/2026-07-13-remove-api-db-workbenches.md
```

API Mock 文档与 `process.md` 章节必须仍然存在：

```powershell
rg -n "API Mock" process.md docs/superpowers/specs docs/superpowers/plans
```

Expected: 有多处现有 API Mock 记录。

- [ ] **Step 7: 检查 Markdown 结构并提交**

Run:

```powershell
rg -n "^## " process.md docs/superpowers/specs/2026-07-04-json-tree-viewer-extensions-design.md docs/superpowers/specs/2026-07-04-structure-refactor-roadmap-design.md docs/superpowers/specs/2026-07-05-ipc-contract-cross-cutting-design.md
git diff --check
```

Expected: 标题层级连续，无空章节、尾随空格或冲突标记。

Commit:

```powershell
$paths = @(
  "docs/plans/2026-06-30-api-workbench-navigation.md",
  "docs/plans/2026-07-11-api-workbench-request-bar-layout.md",
  "docs/plans/2026-07-12-api-workbench-meta-actions-nowrap.md",
  "docs/superpowers/plans/2026-06-29-api-workbench.md",
  "docs/superpowers/plans/2026-06-30-api-workbench-replay.md",
  "docs/superpowers/plans/2026-07-03-db-workbench.md",
  "docs/superpowers/specs/2026-06-29-api-workbench-design.md",
  "docs/superpowers/specs/2026-06-30-api-workbench-navigation-design.md",
  "docs/superpowers/specs/2026-06-30-api-workbench-personal-debugging-design.md",
  "docs/superpowers/specs/2026-06-30-api-workbench-replay-design.md",
  "docs/superpowers/specs/2026-07-01-api-workbench-response-preview-design.md",
  "docs/superpowers/specs/2026-07-02-db-workbench-design.md",
  "docs/superpowers/specs/2026-07-04-api-workbench-ux-design.md",
  "docs/superpowers/specs/2026-07-04-api-workbench-ux-plan.md",
  "docs/superpowers/specs/2026-07-11-api-workbench-request-bar-layout-design.md",
  "docs/superpowers/specs/2026-07-12-api-workbench-meta-actions-nowrap-design.md",
  "docs/superpowers/specs/2026-07-04-json-tree-viewer-extensions-design.md",
  "docs/superpowers/specs/2026-07-04-json-tree-viewer-extensions-plan.md",
  "docs/superpowers/specs/2026-07-04-structure-refactor-roadmap-design.md",
  "docs/superpowers/plans/2026-07-04-structure-refactor-batch0-2-plan.md",
  "docs/superpowers/plans/2026-07-04-structure-refactor-batch0-1-plan.md",
  "docs/superpowers/specs/2026-07-05-ipc-contract-cross-cutting-design.md",
  "docs/superpowers/plans/2026-07-05-ipc-contract-cross-cutting-plan.md",
  "process.md"
)
git add -A -- $paths
git commit -m "docs: 清理已移除工作台历史文档"
```

提交前确认本次移除 spec 和 plan 仍存在，API Mock 文档没有进入删除列表。

---

### Task 5: 全仓残留扫描与完整验证

**Files:**

- Verify only; no planned source changes.

**Interfaces:**

- Consumes: Tasks 1-4 的全部提交。
- Produces: 可交付的删除结果和验证证据。

- [ ] **Step 1: 检查工作区边界**

Run:

```powershell
git status --short
git diff
```

Expected: 记录执行前已有的无关改动；后续提交不得包含这些文件。

- [ ] **Step 2: 扫描活跃源码残留**

Run:

```powershell
rg -n "api-workbench|api_workbench|ApiWorkbench|db-workbench|DbWorkbench|tool:db:|db_connections|db_saved_queries|db_query_history|db-key" apps/desktop/src apps/desktop/src-tauri/src
```

Expected: 无输出。

Run:

```powershell
rg -n "api-mock|api_mock|ApiMock" apps/desktop/src apps/desktop/src-tauri/src
```

Expected: 有多处输出，证明 API Mock 仍在目录、bridge、前端和后端中。

- [ ] **Step 3: 运行前端完整验证**

Run:

```powershell
pnpm test
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

Expected: 三条命令全部成功。测试总数会因删除专属测试下降，不要求与删除前数量相同。

- [ ] **Step 4: 运行 Rust 完整验证**

Run:

```powershell
cargo test
cargo check
```

Workdir: `apps/desktop/src-tauri`

Expected: 两条命令全部成功；不存在 dead code、unused import 或已删除依赖相关错误。

- [ ] **Step 5: 最终 diff 审计**

Run:

```powershell
git diff --check HEAD~4..HEAD
git status --short
git log -5 --oneline
```

Expected:

- 最近提交包含前端移除、后端移除、依赖清理、文档清理四个独立提交。
- `git diff --check` 无输出。
- 工作区只保留执行前已有或执行期间用户新增的明确无关改动。
- 不创建空的“验证提交”；若验证发现问题，将修复归入对应任务并重新运行该任务验证。

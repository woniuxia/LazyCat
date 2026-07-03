# DB Workbench Implementation Plan

> **For Claude:** 按任务逐项实施，每个任务完成后执行对应验证再进入下一任务。

**Goal:** 新增 LazyCat「数据库工作台」工具（`db-workbench`）。一期交付 MySQL + KingbaseES 的 SQL 工作台（连接管理、库表结构浏览、SQL 查询、表数据网格编辑、导出、收藏/历史、安全阀）；二期交付 Redis key 浏览器。

**Architecture:** 后端新增 `db` 常规 tool domain，`db_drivers/` 内以 trait 隔离引擎方言（MySQL/KingbaseES 走 sqlx，Redis 走 redis crate）。异步执行复用 dns.rs 的静态 `OnceLock<Runtime>` + `block_on` 模式。连接密码用 vault 的 AES-256-CBC 函数加密，密钥为数据目录下自动生成的 `db-key` 文件。前端单入口面板按连接引擎分流视图，SQL 编辑器基于已本地打包的 monaco-editor。

**Tech Stack:** Tauri 2, Rust, sqlx 0.8 (mysql + postgres, 纯 Rust 零外部 DLL), redis crate（二期）, rusqlite, Vue 3, TypeScript, monaco-editor, Element Plus, Vitest.

**设计文档:** `docs/superpowers/specs/2026-07-02-db-workbench-design.md`

---

## 关键实现决策

1. **连接池注册表**：静态 `Mutex<HashMap<String, DbPool>>`，key 为 `connectionId\u{1}database`（PG 族物理上每库一连接，MySQL 统一同构处理）。`DbPool` 为 enum { MySql(MySqlPool), Pg(PgPool) }，max_connections=3。`connection_close` 按 connectionId 前缀清理；删除连接时同步清理。
2. **运行查询登记**：静态 `Mutex<HashMap<String, RunningQuery>>`，queryId（前端 uuid）→ { connectionId, database, engine 会话标识 }。执行前在同一连接上先查 `SELECT CONNECTION_ID()`（MySQL）/ `SELECT pg_backend_pid()`（KB）登记，结束注销。`query_cancel` 另取连接执行 `KILL QUERY <id>` / `SELECT pg_cancel_backend(<pid>)`。
3. **超时**：`tokio::time::timeout`（默认 30s，连接 options 可调）包裹执行；超时后尝试按登记取消，返回超时错误。
4. **行数上限与截断**：SELECT 类语句用 `fetch` 流式读取 maxRows+1 行（不改写 SQL），超出置 `truncated: true`。
5. **值编码**：所有单元格值序列化为 `string | null`，列元数据带 `typeName` 与 `kind`（number/text/datetime/bool/binary/json）。MySQL/PG 各自按 `type_info().name()` 匹配解码（chrono/rust_decimal/uuid/json 特性），二进制显示 `0x…(N bytes)` 摘要且标记不可编辑。
6. **写回绑定**：MySQL 直接绑字符串参数（弱类型可协变）；KB/PG 生成 `CAST($n AS <data_type>)`（data_type 取自 information_schema.columns），NULL 绑 `Option<String>::None`。标识符引用：MySQL 反引号、KB 双引号，转义函数配单测。
7. **确认握手**：后端始终自行分类语句；只读连接上的非只读语句无条件拒绝（错误返回）；需确认类（prod 的 DML/DDL、无 WHERE 的 UPDATE/DELETE）在未带 `confirmed: true` 时返回 `{ needsConfirmation: true, reasons: [...] }` 且不执行，前端弹窗确认后原样重发。
8. **只读口径**（TS 与 Rust 同规则同测试向量）：SELECT、WITH…SELECT、VALUES、SHOW、EXPLAIN、DESC/DESCRIBE、括号包裹 SELECT。
9. **语句拆分**：按分号拆分，正确跳过单双引号字符串、反引号标识符、`--`/`#` 行注释与 `/* */` 块注释；TS/Rust 双端实现共享同一组测试用例。
10. **表数据浏览**：`SELECT * FROM <t> [WHERE …] [ORDER BY …] LIMIT ? OFFSET ?` + 同条件 `COUNT(*)`；filters 操作符白名单（= != > < >= <= LIKE NOT-LIKE IS-NULL IS-NOT-NULL）；主键列来自 schema 查询，无主键 → `editable: false` + 原因。
11. **apply_changes**：单事务逐条执行；UPDATE/DELETE 影响行数为 0 视为并发冲突 → 整体回滚并返回冲突行索引。
12. **KB 表定位**：KingbaseES 表名带 schema 限定（`schema.table`），系统 schema（pg_catalog、information_schema、sys*）过滤；DDL 由 pg_catalog 拼装基础版。
13. **历史**：逐条语句一条记录，全局环形 500，`executed_at` 建索引。
14. **导出**：前端 `@tauri-apps/plugin-dialog` 选保存路径，后端流式写 CSV/JSON/INSERT；仅接受只读语句；INSERT 导出弹窗确认目标表名。
15. **密钥文件**：`<数据目录>/db-key`（32 字节随机，hex 存储）；`password_cipher` 存 `base64(iv):base64(cipher)`；settings.rs 数据目录迁移同步复制 db-key。
16. **KB 连通性风险**：开发环境无 KB 实例，实现按 PG 协议交付，真实连通性由用户在内网验收时确认；若认证握手失败，退路见设计文档。

## File Structure

新增（一期）：

- `apps/desktop/src-tauri/src/tools/db.rs`
- `apps/desktop/src-tauri/src/tools/db_drivers/mod.rs`
- `apps/desktop/src-tauri/src/tools/db_drivers/sql_text.rs`（拆分/分类/标识符转义纯函数 + 单测）
- `apps/desktop/src-tauri/src/tools/db_drivers/mysql.rs`
- `apps/desktop/src-tauri/src/tools/db_drivers/kingbase.rs`
- `apps/desktop/src/components/DbWorkbenchPanel.vue`
- `apps/desktop/src/components/db/DbConnectionDialog.vue`
- `apps/desktop/src/components/db/DbSqlWorkspace.vue`
- `apps/desktop/src/components/db/DbSqlEditor.vue`
- `apps/desktop/src/components/db/DbResultGrid.vue`
- `apps/desktop/src/components/db/DbTableStructure.vue`
- `apps/desktop/src/composables/useDbConnections.ts`
- `apps/desktop/src/utils/dbSqlClassify.ts` + `.test.ts`
- `apps/desktop/src/utils/dbGridChanges.ts` + `.test.ts`
- `apps/desktop/src/types/db.ts`

新增（二期）：

- `apps/desktop/src-tauri/src/tools/db_drivers/redis.rs`
- `apps/desktop/src/components/db/DbRedisBrowser.vue`
- `apps/desktop/src/utils/dbRedisKeyTree.ts` + `.test.ts`

修改：

- `apps/desktop/src-tauri/Cargo.toml`（sqlx；二期 redis）
- `apps/desktop/src-tauri/src/tools/mod.rs`（注册 db 域）
- `apps/desktop/src-tauri/src/tools/helpers.rs`（三张表 + 索引）
- `apps/desktop/src-tauri/src/tools/vault.rs`（aes256_encrypt/decrypt/random_bytes 提为 pub(crate)）
- `apps/desktop/src-tauri/src/tools/settings.rs`（迁移复制 db-key）
- `apps/desktop/src/bridge/tauri.ts`（tool:db:* 通道）
- `apps/desktop/src/tool-registry.ts`、`apps/desktop/src/composables/toolCatalog.ts`（新「数据库」分组）

## Tasks（一期）

- T1 依赖与存储地基：Cargo 加 sqlx；helpers.rs 建 `db_connections` / `db_saved_queries` / `db_query_history`（含 executed_at 索引）；vault.rs 函数提权；db-key 密钥管理；settings.rs 迁移复制。验证：`cargo check`。
- T2 sql_text.rs：语句拆分、只读/危险分类、无 WHERE 检测、标识符转义纯函数 + 单测。验证：`cargo test sql_text`。
- T3 db_drivers/mod.rs + mysql.rs：trait、公共类型、MySQL 连接/池、schema 三查询（databases/tables/table_detail 含 DDL）、执行与解码、会话登记、取消。
- T4 kingbase.rs：PG 协议同套能力，schema 限定表名、CAST 写回、pg_cancel_backend。
- T5 db.rs 编排：action 分发、连接 CRUD（加密、占位符语义、级联删除）、connection_test/open/close、query_execute（只读拦截 → 确认握手 → 超时 → 逐条历史）、table_data_page、table_apply_changes（事务 + 冲突回滚）、result_export（只读校验 + 流式写文件 + 可取消）、收藏/历史 CRUD；mod.rs 注册。验证：`cargo test db`、`cargo check`。
- T6 前端地基：types/db.ts、bridge 通道、tool-registry、toolCatalog 新分组。验证：`pnpm typecheck`。
- T7 前端纯函数：dbSqlClassify.ts（拆分/分类/光标语句提取，与 Rust 共享测试向量）、dbGridChanges.ts（变更集归一化、预览 SQL 渲染）+ 单测。验证：`pnpm test src/utils/dbSqlClassify.test.ts src/utils/dbGridChanges.test.ts`。
- T8 连接管理 UI：useDbConnections.ts、DbConnectionDialog.vue（引擎切换默认端口、测试连接、密码占位符）、DbWorkbenchPanel.vue（连接树、分组、环境色点、只读徽标、空态）。
- T9 SQL 工作台 UI：DbSqlWorkspace.vue（库表树 + 页签）、DbSqlEditor.vue（monaco sql + 结构补全 + Ctrl+Enter 执行选中/光标语句）、DbTableStructure.vue（字段/索引/DDL）。
- T10 结果网格与编辑：DbResultGrid.vue（分页、截断提示、状态栏、导出三格式、表数据模式的暂存变更集/NULL 标记/新增删除行/预览应用）、确认握手弹窗、query_cancel 按钮。
- T11 一期收尾验证：`pnpm typecheck`、`pnpm test`、`pnpm --filter @lazycat/desktop build:web`、`cargo test`，提交。

## Tasks（二期）

- T12 redis.rs：连接（auth/db index）、SCAN 分批、key 详情（类型/TTL/编码/值分页截断）、写操作（set/del/expire/rename/字段级）、命令执行（黑名单 + 阻塞类拒绝 + 10s 超时）、只读拦截。验证：`cargo test redis`。
- T13 db.rs 挂 redis_* actions + bridge 通道 + 连接对话框 redis 字段。
- T14 dbRedisKeyTree.ts（`:` 聚合树）+ 单测；DbRedisBrowser.vue（key 树 + pattern 过滤 + 类型化值查看编辑 + TTL + 命令控制台）。
- T15 二期收尾验证：同 T11，提交。

## 不做

- 不做 ER 图、可视化 ALTER、智能语义补全、数据同步。
- 不做达梦、Oracle、SQL Server（三期另立项）。
- 不做 Redis cluster/哨兵拓扑发现。
- 不做 Spotlight 连接搜索（三期）。
- 单元测试不依赖真实数据库实例；真实连通性由用户内网验收。

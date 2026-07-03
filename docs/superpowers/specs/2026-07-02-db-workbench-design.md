# 数据库工作台设计

## 概述

新增深度工具「数据库工作台」，内部工具 ID 为 `db-workbench`，侧边栏新增「数据库」分组。目标是让内网离线全栈工程师在 LazyCat 内完成日常数据库工作：连接管理、库表结构浏览、SQL 查询执行、表数据网格编辑、结果导出，以及 Redis key 浏览与编辑。

本设计一次覆盖两期实施范围：

- 一期：MySQL 与 KingbaseES（人大金仓）的 SQL 工作台，含表数据网格编辑。
- 二期：Redis key 浏览器。

达梦 DM 支持、表结构文档导出、Spotlight 连接搜索为三期占位，不在本设计展开。

## 目标

1. 统一管理多引擎数据库连接：分组、环境标签（dev/test/prod）、只读保护标记、测试连接。
2. 连接密码本地加密存储，复用 vault 的 AES-256-CBC 加密实现。
3. 浏览库表结构：表/视图列表、字段、索引、DDL，一键复制。
4. 多页签 SQL 查询：Monaco 编辑器、语法高亮、基于结构树的静态补全、多语句顺序执行。
5. 结果集分页展示与导出（CSV / JSON / INSERT 语句），大结果集后端流式导出。
6. 表数据浏览页签支持网格编辑：暂存变更集、SQL 预览、单事务应用。
7. SQL 收藏与执行历史。
8. 后端强制的安全阀：只读连接、prod 二次确认、无 WHERE 警告、超时与行数上限、取消执行。
9. 二期提供 Redis key 浏览、类型感知值查看与编辑、受控命令控制台。
10. 完全离线运行：sqlx 与 redis 均为纯 Rust 驱动，零外部 DLL，便携包免驱动。

## 非目标

1. 不做任意 SQL 结果集的网格编辑（仅表数据浏览页签可编辑）。
2. 不做 SQL 语义分析级的智能补全，仅基于已加载结构树的静态补全。
3. 不做 ER 图、模型设计器、数据同步/迁移。
4. 一期不支持达梦 DM、Oracle、SQL Server。
5. 不做数据库用户/权限管理。
6. 不把连接凭据绑定 vault 主密码解锁流程。
7. Redis 首版（二期）仅支持单机直连单节点，不支持 cluster 与哨兵拓扑发现。
8. 不做表结构变更（ALTER 可视化设计器），DDL 通过 SQL 页签手写执行。

## 与既有能力的关系

- 加密：复用 `vault.rs` 的 AES-256-CBC 加密函数（实施时提为共享 helper），密钥独立（见「密码加密」节）。
- 异步运行时：复用 `dns.rs` 确立的静态 `OnceLock<tokio::runtime::Runtime>` + `block_on` 模式。
- 面板内多视图：复用 PM 面板 `pmViewRegistry` 的视图注册与 `<component :is>` 分流模式。
- 多页签：复用 `useTabs` 的页签管理模式。
- 请求时序：面板内异步请求带请求序号，旧响应不覆盖新状态，沿用数据字典面板约定。
- Monaco：项目已本地打包 monaco-editor 0.52.2，SQL 编辑器直接复用。

## 架构

### 工具形态

- 侧边栏新增「数据库」分组，一期唯一入口「数据库工作台」。
- 面板布局：左侧连接树（分组折叠、引擎图标、环境标签色点），右侧主工作区。
- 打开 MySQL/KingbaseES 连接进入 SQL 工作台视图；打开 Redis 连接（二期）进入 key 浏览器视图。

### 前端结构

```text
components/DbWorkbenchPanel.vue          主面板：连接树 + 主区视图分流
components/db/DbConnectionDialog.vue     连接新建/编辑对话框（含测试连接）
components/db/DbSqlWorkspace.vue         SQL 工作台视图：库表树 + 多页签
components/db/DbSqlEditor.vue            Monaco SQL 编辑器封装
components/db/DbResultGrid.vue           结果网格：分页、只读/编辑态
components/db/DbTableStructure.vue       表结构页签：字段/索引/DDL
components/db/DbRedisBrowser.vue         二期：key 浏览器视图
composables/useDbConnections.ts          连接列表与打开状态管理
utils/dbGridChanges.ts                   变更集归一化与校验（纯函数 + 单测）
utils/dbSqlClassify.ts                   SQL 语句分类与危险检测（纯函数 + 单测）
utils/dbRedisKeyTree.ts                  二期：key 按分隔符聚合成树（纯函数 + 单测）
types/db.ts                              集中类型定义
```

### Rust 端结构

```text
src-tauri/src/tools/db.rs                action 分发 + 连接管理 + 池管理
src-tauri/src/tools/db_drivers/
  mod.rs                                 DbDriver trait 与公共类型
  mysql.rs                               sqlx MySqlPool 实现
  kingbase.rs                            sqlx PgPool 实现（PG 协议直连）
  redis.rs                               二期：redis crate，独立 KV trait
```

- 通道走常规分发：`tool:db:*` -> `db` 域，在 `bridge/tauri.ts` 的 `CHANNEL_MAP` 与 `tools/mod.rs` 注册。
- 静态 `OnceLock<Runtime>` 提供异步执行环境；活动连接池存于静态 `HashMap<connectionId, PoolHandle>`（Mutex 保护），池大小 2-4，`connection_close` 与应用退出时回收。
- 新增依赖：`sqlx`（features: mysql, postgres, runtime-tokio, tls-rustls）；二期 `redis`（features: tokio-comp）。
- KingbaseES 与 MySQL 的方言差异（系统目录查询、DDL 获取、取消语句）封装在各自 driver 内，前端拿到统一结构。

## 数据模型

SQLite 新增三张表（`helpers.rs` 建表与迁移）：

```sql
CREATE TABLE db_connections (
  id TEXT PRIMARY KEY,              -- uuid
  name TEXT NOT NULL,
  engine TEXT NOT NULL,             -- 'mysql' | 'kingbase' | 'redis'
  host TEXT NOT NULL,
  port INTEGER NOT NULL,            -- 默认值按引擎：3306 / 54321 / 6379
  username TEXT,                    -- redis 可空
  password_cipher TEXT,             -- AES-256-CBC，见密码加密节
  default_database TEXT,            -- mysql: schema；kingbase: database；redis: db index
  env_tag TEXT NOT NULL DEFAULT 'dev',   -- 'dev' | 'test' | 'prod' | 'other'
  read_only INTEGER NOT NULL DEFAULT 0,  -- 只读保护，后端强制
  group_name TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  options_json TEXT,                -- 引擎特定扩展：charset、超时、行数上限等
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  last_used_at INTEGER
);

CREATE TABLE db_saved_queries (
  id TEXT PRIMARY KEY,
  connection_id TEXT,               -- 可空表示全局收藏
  title TEXT NOT NULL,
  sql TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE db_query_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  connection_id TEXT NOT NULL,
  sql TEXT NOT NULL,
  executed_at INTEGER NOT NULL,
  duration_ms INTEGER,
  status TEXT NOT NULL,             -- 'ok' | 'error'
  row_count INTEGER
);
```

- 执行历史全局环形保留最近 500 条，写入时裁剪；`executed_at` 建索引支撑倒序分页。
- 密码只存密文；`connection_list` 返回时不带密文，编辑对话框回显用占位符，仅在用户重新输入时更新。

## 密码加密

- 复用 vault 的 AES-256-CBC 加密/解密函数，每条密文使用独立随机 IV。
- 密钥不绑定 vault 主密码：首次使用时在数据目录自动生成 32 字节随机密钥文件（如 `<数据目录>/db-key`），权限仅当前用户。
- 取舍说明：绑定 vault 意味着每次连库都要先解锁 vault，日常摩擦过大。本地密钥的威胁模型是防止随手翻看数据库文件读到明文密码，与 Navicat 等桌面工具同级；不承诺抵御拿到完整数据目录的攻击者。
- 数据目录迁移时，`db-key` 文件必须随 `lazycat.sqlite` 一起复制，迁移逻辑同步更新。

## IPC actions（tool:db:*）

连接管理：

- `connection_list` / `connection_save` / `connection_delete`
- `connection_test`：用传入配置试连，不落库；编辑既有连接且密码占位符未改动时，携带 `connectionId` 由后端回退已存密文
- `connection_open`：建池，返回引擎版本与库列表；`connection_close`：关池
- `connection_delete`：级联清理——该连接的执行历史同删，连接级 SQL 收藏转为全局收藏（connection_id 置空）
- 密码提交语义：编辑对话框密码框回显固定占位符；占位符未改动则保持原密文，用户显式清空则置空

结构浏览：

- `schema_databases`：库列表
- `schema_tables`：某库的表/视图列表（含注释、行数估计）
- `schema_table_detail`：字段/索引/DDL

查询与编辑：

- `query_execute`：{ connectionId, database, sql, queryId, maxRows?, confirmed? } -> 多语句逐条结果（列元数据、行数据、影响行数、耗时）
  - `queryId` 由前端生成（uuid）随请求传入；后端开始执行时将该 queryId 与引擎侧会话标识（MySQL processlist id / KB backend pid）登记到运行中查询表，结束后注销
  - `confirmed` 用于二次确认握手，见「安全阀」节
- `query_cancel`：{ connectionId, queryId }，按登记信息另开连接执行 MySQL `KILL QUERY` / KingbaseES `pg_cancel_backend`
- `table_data_page`：{ connectionId, database, table, page, pageSize, orderBy?, filters? }，返回数据页 + 主键列信息
- `table_apply_changes`：{ connectionId, database, table, changes[], confirmed? }，单事务执行，返回逐条影响行数；UPDATE/DELETE 影响行数为 0 视为并发冲突（行已被他人修改或删除），按失败处理：整体回滚并标出冲突行
- `result_export`：{ connectionId, database, sql, format, outputPath, queryId }，后端流式查询直写文件；仅接受只读语句（防止 DML 被二次执行）；同样走 queryId 登记支持取消；导出会重新执行查询，结果可能与屏幕上已加载的快照存在差异，UI 需说明

事务语义：

- 单次 `query_execute` 内的多语句保证在同一物理连接上顺序执行，事务块（BEGIN…COMMIT）写在同一次执行内有效。
- 跨多次调用的交互式事务不支持（池化下会落在不同物理连接上）；前端检测到孤立 BEGIN/COMMIT 时提示用户将完整事务块放入一次执行。

收藏与历史：

- `saved_query_list` / `saved_query_save` / `saved_query_delete`
- `history_list` / `history_clear`

Redis（二期）：

- `redis_scan`：游标分批 + pattern 过滤
- `redis_key_detail`：类型、TTL、编码、值（大值截断分页）
- `redis_key_write`：set/del/expire/rename 与字段级增删改
- `redis_command`：单条命令执行（黑名单拦截）

## SQL 工作台交互

### 结构浏览

- 连接树展开：库 -> 表/视图；点击表打开「表详情」页签，含字段列表（名称/类型/可空/默认值/注释/主键标记）、索引列表、DDL，均可一键复制。

### 查询页签

- 多页签，每页签一个 Monaco 编辑器 + 结果区。
- 执行规则：有选中执行选中文本，无选中执行光标所在语句；多语句按分号顺序执行、逐条返回。
- 结果区默认 500 行/页，后端强制行数上限（默认 1000，连接选项可调）；命中上限时结果元数据带 `truncated` 标记，状态栏提示"已截断至 N 行"；状态栏显示耗时与影响行数。
- 导出当前结果为 CSV / JSON / INSERT；导出 INSERT 时弹窗确认目标表名（默认从 SQL 中提取，可改）。「导出全部」走 `result_export` 后端流式写文件，同样携带 queryId 登记，可取消。
- 执行历史按逐条语句记录（多语句一次执行产生多条历史）。SQL 收藏与历史在侧抽屉，双击回填编辑器。

### 网格编辑

可编辑入口只有一个：从表树进入的「表数据浏览」页签（自带分页、按列排序、列 + 操作符 + 值的简单筛选，后端拼参数化 WHERE）。自写 SQL 的结果一律只读，这是刻意的 MVP 边界，避免解析任意 SQL 推断来源表与主键。

暂存变更集流程：

1. 双击单元格编辑，改动行标黄；「新增行」标绿，「删除行」标红划线；支持显式设 NULL（区分空字符串）。
2. 「应用更改」弹窗预览参数化 UPDATE/INSERT/DELETE 列表。
3. 确认后单事务执行，任一失败整体回滚并标出失败行。
4. 成功后刷新当前页。

约束：无主键的表禁止编辑并提示原因；二进制/超长字段只读显示摘要。

### 安全阀（后端强制）

只读判定口径两端统一：`utils/dbSqlClassify.ts`（前端提示用）与后端拦截（强制用）共用同一份分类规则——只读形态包括 SELECT、WITH…SELECT、VALUES、SHOW、EXPLAIN、DESC/DESCRIBE 及括号包裹的 SELECT；后端始终自行分类，不信任前端结论。

- **只读连接**：非只读形态语句无条件拒绝，不提供确认放行通道。
- **需确认类操作**（两段式握手）：prod 环境标签连接上的 DML/DDL、无 WHERE 的 UPDATE/DELETE。`query_execute` / `table_apply_changes` 未携带 `confirmed: true` 时，后端不执行，返回结构化 `needsConfirmation` 响应（含触发原因列表）；前端据此弹窗确认，用户确认后携带 `confirmed: true` 原样重发。
- 查询超时默认 30 秒；行数上限默认 1000（表数据浏览分页除外）；均可按连接调整。
- 取消执行：见 IPC actions 节的 queryId 登记机制。

## Redis 浏览器（二期）

- 连接对话框按引擎切换字段：host/port/密码/db 编号（0-15），支持只读标记。
- key 浏览：SCAN 游标分批加载（禁止 KEYS *），pattern 过滤，key 按 `:` 聚合成树，节点显示子 key 数量。
- 值查看：类型感知（string 尝试 JSON 美化、hash 字段表格、list/set 成员列表、zset 成员+score），显示 TTL、编码、内存估计。
- 编辑：set 值、删除、改 TTL、重命名、集合类型字段级增删改；只读连接下后端全部拒绝。
- 命令控制台：单条命令执行，默认 10 秒超时。两类拦截：FLUSHALL/FLUSHDB/CONFIG/SHUTDOWN 等破坏性命令需在弹窗中手动输入命令名确认才放行；SUBSCRIBE/PSUBSCRIBE/MONITOR/BLPOP/BRPOP/WAIT 等阻塞与订阅类命令直接拒绝（控制台不支持长连接语义）。

## 错误处理

- 连接失败分类提示：网络不通 / 认证失败 / 目标库不存在，措辞对齐 network 工具。
- 查询错误原样透传数据库错误消息，引擎提供行号/位置时一并显示。
- 断线自愈：执行时发现池内连接失效，自动重建一次后重试，再失败才报错。
- 派生状态一致性：连接树、页签、结果区的异步请求全部带请求序号，旧响应丢弃。

## 已知风险与验证点

1. KingbaseES 认证兼容：KB 定制认证算法（sm3/sha256）可能导致 sqlx PG 握手失败。实施计划第一步做连通性 spike；失败退路为调整 KB 服务端认证配置（md5/scram），或一期 KB 降级为结构浏览 + 查询、编辑视验证结果决定。
2. sqlx 列类型到 JSON 的映射：DECIMAL/DATETIME/二进制等类型的序列化边界需在 driver 层统一约定（字符串化 + 类型标记），避免前端精度丢失。
3. 大结果集内存：后端强制行数上限 + 流式导出兜底；表数据浏览始终分页。
4. 连接池泄漏：面板关闭、应用退出、连接删除三条路径都必须回收池。

## 测试策略

- 前端纯函数单测（vitest）：变更集归一化与校验、SQL 语句分类与危险检测、筛选条件构造、key 树聚合。
- Rust 单测：标识符转义、参数化语句构造、系统目录查询拼装、只读拦截、历史环形裁剪。
- 真实数据库集成测试：feature-gated（`cargo test --features db-integration`），本机有库则跑、无库跳过。
- E2E：面板打开、连接对话框展示（mock IPC）。
- 常规基线：`pnpm typecheck`、`pnpm --filter @lazycat/desktop build:web`、`pnpm test`。

## 分期交付

| 期 | 内容 | 前置验证 |
|----|------|----------|
| 一期 | 连接管理 + MySQL/KB SQL 工作台全量（结构浏览、查询、表数据网格编辑、导出、收藏/历史、安全阀） | KingbaseES 连通性 spike 置于最前 |
| 二期 | Redis 浏览器视图 + redis 连接类型 | 无 |
| 三期（占位） | 达梦 ODBC 调研、表结构文档导出（联动 data-dictionary / sql-entity）、Spotlight 连接搜索 | 达梦驱动可行性调研 |

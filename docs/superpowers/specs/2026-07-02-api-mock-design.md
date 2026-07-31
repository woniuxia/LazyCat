# API Mock 工具设计

## 概述

新增独立工具「API Mock」，内部工具 ID 为 `api-mock`，后端 domain 为 `api_mock`。该工具面向本机和局域网内的前端联调、第三方回调调试、离线演示和接口占位场景，允许用户创建多个 Mock 项目，每个项目绑定独立监听地址和端口，并维护多条 Mock 路由。

首版采用持久化配置 + 手动启动/停止的模式。数据库保存项目、路由、响应和文件副本元信息；运行中的 HTTP 服务、停止信号和最近请求日志只保存在后端进程内。应用重启后不会自动恢复服务，用户需要显式启动项目。

该工具与现有「接口调试」并列，不嵌入 API Workbench。设计中保留从 API Workbench 请求快照生成 Mock 路由的字段，后续可增加导入入口。

## 已确认决策

1. API Mock 做成独立工具，后续可从 API Workbench 快速生成 Mock。
2. 支持多个 Mock 项目，每个项目绑定一个监听地址和端口。
3. 多个项目可以同时运行，只要端口不冲突。
4. 服务默认监听 `127.0.0.1`，项目级可显式选择 `0.0.0.0`。
5. Mock 配置持久化，服务由用户手动启动/停止。
6. 路由匹配支持 `Method + 精确路径 / 路径参数 / 通配符`。
7. 同一项目内允许表面冲突路由，匹配优先级固定为 `精确 > 路径参数 > 通配符`。
8. 首版响应配置支持状态码、Content-Type、响应头、静态文本/JSON/HTML Body 和文件返回。
9. 首版不实现随机变量和动态响应模板，只预留后续 `template_body` 扩展。
10. 文件返回使用受控副本：导入文件复制到 LazyCat 数据目录，路由引用副本。
11. 首版做路由级 CORS 配置，并支持 OPTIONS 预检。
12. 运行期记录最近请求日志：时间、方法、路径、状态、命中路由、耗时和错误摘要；不保存完整请求头和请求体。

## 目标 / 非目标

### 目标

1. 新增独立工具入口「API Mock」。
2. 支持项目创建、更新、删除、排序。
3. 支持每个项目配置 `host`、`port`、名称和描述。
4. 支持项目手动启动、停止、状态查询。
5. 支持多项目并行运行。
6. 支持路由创建、更新、删除、排序和启用/禁用。
7. 支持 Method、路径模式、状态码、Content-Type、响应头和响应体配置。
8. 支持静态响应和文件响应。
9. 支持路由级 CORS 和 OPTIONS 预检。
10. 支持每项目最近请求日志查询。
11. 所有运行时资源离线可用，不新增公网依赖。

### 非目标

1. 首版不实现随机变量、动态模板、条件响应或请求体解析响应。
2. 首版不支持按 Query、Header 或 Body 条件匹配。
3. 首版不支持代理转发、录制回放、压测或自动化断言。
4. 首版不支持 HTTPS、本地证书或 HTTP/2。
5. 首版不支持自动恢复上次运行的 Mock 服务。
6. 首版不保存完整请求头和请求体日志。
7. 首版不提供文件缓存管理页面；只做导入、引用和删除项目/路由时的安全清理。
8. 首版不把 API Workbench 集合直接复用为 Mock 项目。

## 用户流程

### 创建并启动 Mock 项目

1. 用户进入「API Mock」工具。
2. 新建项目，填写名称，选择监听地址，填写端口。
3. 新增路由，例如 `GET /api/users/:id`。
4. 配置状态码、Content-Type、响应头和响应 Body。
5. 点击启动项目。
6. 后端绑定 `host:port`，启动 HTTP 服务。
7. 用户通过浏览器、前端应用或 API Workbench 请求该地址。
8. 面板展示运行状态和最近请求日志。

### 返回文件

1. 用户创建或编辑路由。
2. 将响应类型切换为「文件」。
3. 选择本地文件。
4. 后端将文件复制到 `<dataDir>/api-mock/files/`，保存文件元信息。
5. 路由保存 `file_id`，请求命中后从数据目录副本读取并返回。

### 修改运行中项目

1. 项目运行时，用户修改项目配置或路由配置。
2. 前端保存配置到数据库。
3. 如果该项目正在运行，前端标记「需重启生效」。
4. 首版不做热更新；用户显式停止后重新启动。

## 架构

采用独立工具、独立后端域和每项目一个本地 HTTP 服务的结构。

### 前端

新增：

- `apps/desktop/src/components/ApiMockPanel.vue`
- `apps/desktop/src/types/api-mock.ts`
- `apps/desktop/src/utils/apiMock.ts`
- `apps/desktop/src/utils/apiMock.test.ts`

修改：

- `apps/desktop/src/composables/toolCatalog.ts`：在网络与系统分组加入 `api-mock`。
- `apps/desktop/src/tool-registry.ts`：注册 `ApiMockPanel.vue`。
- `apps/desktop/src/bridge/tauri.ts`：新增 `tool:api-mock:*` 通道。

前端只负责状态编排和 UI 绑定。路由匹配、端口绑定、文件复制、运行状态和日志都由后端负责。

### 后端

新增：

- `apps/desktop/src-tauri/src/tools/api_mock.rs`

修改：

- `apps/desktop/src-tauri/src/tools/mod.rs`：注册 `api_mock` domain。
- `apps/desktop/src-tauri/src/tools/helpers.rs`：初始化 API Mock 表结构。

后端职责：

1. SQLite 配置读写。
2. 路由和 CORS 配置校验。
3. 文件导入和受控副本管理。
4. Mock 服务启动、停止和状态查询。
5. HTTP 请求路由匹配和响应构造。
6. 运行期最近请求日志维护。

### 运行态注册表

后端维护全局运行态注册表：

```rust
project_id -> RunningMockService
```

运行态包含：

- `project_id`
- `host`
- `port`
- `started_at`
- `stop_signal`
- `thread_handle`
- `route_snapshot`
- `recent_logs`
- `last_error`

运行态只在进程内存在，不写入 SQLite。数据库只保存配置。

## 数据模型

### `api_mock_projects`

```sql
CREATE TABLE IF NOT EXISTS api_mock_projects (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  host TEXT NOT NULL DEFAULT '127.0.0.1',
  port INTEGER NOT NULL,
  enabled_cors_default INTEGER NOT NULL DEFAULT 1,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_api_mock_projects_sort
  ON api_mock_projects(sort_order, id);
```

规则：

1. `host` 首版只允许 `127.0.0.1` 或 `0.0.0.0`。
2. `port` 范围为 `1..=65535`。
3. `enabled_cors_default` 只作为新建路由的初始默认值，不覆盖已有路由。

### `api_mock_routes`

```sql
CREATE TABLE IF NOT EXISTS api_mock_routes (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  method TEXT NOT NULL,
  path_pattern TEXT NOT NULL,
  status_code INTEGER NOT NULL DEFAULT 200,
  response_kind TEXT NOT NULL DEFAULT 'static_body',
  content_type TEXT NOT NULL DEFAULT 'application/json; charset=utf-8',
  headers_json TEXT NOT NULL DEFAULT '[]',
  body_text TEXT NOT NULL DEFAULT '',
  file_id INTEGER,
  cors_json TEXT NOT NULL DEFAULT '{}',
  enabled INTEGER NOT NULL DEFAULT 1,
  sort_order INTEGER NOT NULL DEFAULT 0,
  source_request_id INTEGER,
  source_snapshot_json TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY(project_id) REFERENCES api_mock_projects(id) ON DELETE CASCADE,
  FOREIGN KEY(file_id) REFERENCES api_mock_files(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_api_mock_routes_project
  ON api_mock_routes(project_id, enabled, sort_order, id);
```

规则：

1. `method` 支持 `GET`、`POST`、`PUT`、`PATCH`、`DELETE`、`HEAD`、`OPTIONS`。
2. `response_kind` 首版支持 `static_body` 和 `file`，预留 `template_body`。
3. `source_request_id` 和 `source_snapshot_json` 只为后续 API Workbench 转 Mock 预留，首版不使用。

### `api_mock_files`

```sql
CREATE TABLE IF NOT EXISTS api_mock_files (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  original_name TEXT NOT NULL,
  stored_path TEXT NOT NULL,
  content_type TEXT NOT NULL DEFAULT '',
  size INTEGER NOT NULL DEFAULT 0,
  hash TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_api_mock_files_hash
  ON api_mock_files(hash);
```

文件副本目录：

```text
<dataDir>/api-mock/files/
```

规则：

1. 文件由后端复制到数据目录，前端不直接保存可执行读取路径。
2. 路由通过 `file_id` 引用文件。
3. 删除项目、删除路由或替换路由文件引用后，后端按引用计数清理不再使用的文件副本。
4. 文件副本丢失时，请求返回 `500`，并记录日志。

实际迁移建表时先创建 `api_mock_files`，再创建 `api_mock_routes`，确保外键引用顺序清晰。

### 文件引用清理

所有会移除文件引用的 action 都必须走同一套引用计数清理函数：

1. `route_delete`：删除路由后检查旧 `file_id` 是否仍被其他路由引用。
2. `route_save`：如果路由从旧 `file_id` 切换到新 `file_id`，保存成功后检查旧文件是否仍被引用。
3. `project_delete`：删除项目前先收集项目下所有路由引用的 `file_id`，项目删除成功后逐个检查引用计数。

清理规则：

1. 仍被任一路由引用的文件不能删除。
2. 未被引用的文件先删除磁盘副本，再删除 `api_mock_files` 记录。
3. 磁盘文件不存在时，仍删除无引用的文件记录。
4. 文件清理失败不应回滚已完成的配置删除，但 action 结果需要返回 warning，前端可提示“配置已删除，部分文件副本清理失败”。

## 路由匹配

### 支持语法

1. 精确路径：`/api/users`
2. 路径参数：`/api/users/:id`
3. 通配符：`/files/*`

路径规则：

1. 必须以 `/` 开头。
2. 参数段必须完整占用一个 path segment，例如 `/:id` 合法，`/user-:id` 不作为首版语法。
3. 参数名只允许字母、数字、下划线和短横线，首字符必须是字母或下划线。
4. 通配符 `*` 只能作为最后一个完整 segment，例如 `/files/*`。
5. 首版不支持 query 条件匹配。

### 匹配优先级

请求进入后：

1. 对非 OPTIONS 请求，先按 Method 过滤启用路由。
2. 按路径匹配等级排序：
   - 精确路径
   - 参数路径
   - 通配符
3. 同等级多条命中时，按 `sort_order ASC, id ASC` 取第一条。
4. 没有命中时返回 `404`。

同一项目内允许表面冲突路由。保存时不因 `/users/:id` 和 `/users/*` 同时存在而阻断，匹配结果由固定优先级决定。

### OPTIONS 预检

OPTIONS 请求优先进入 CORS 预检逻辑：

1. 按 path 查找任一启用 CORS 的路由。
2. 找到后返回 `204` 和对应 CORS headers。
3. 没有启用 CORS 的匹配路由时，按普通 OPTIONS 路由匹配。
4. 仍无匹配时返回 `404`。

## 响应模型

### 静态响应

路由字段：

- `status_code`
- `content_type`
- `headers_json`
- `body_text`
- `response_kind = static_body`

后端行为：

1. 不解析 JSON。
2. 不自动美化、修正或压缩 body。
3. 按用户配置原样返回响应体。
4. 自动补 `Content-Type`，除非 headers 中已有同名 header。
5. 自动补 `Content-Length`。

JSON 合法性只做前端提示，不阻断保存非 JSON 文本。这样用户可以模拟错误响应或不规范响应。

### 文件响应

路由字段：

- `response_kind = file`
- `file_id`
- `content_type`
- `headers_json`

后端行为：

1. 通过 `file_id` 读取 `api_mock_files`。
2. 校验 `stored_path` canonicalize 后仍位于 `<dataDir>/api-mock/files/`。
3. 读取文件副本并返回。
4. `Content-Type` 优先使用路由配置，其次文件记录，最后 `application/octet-stream`。
5. 自动补 `Content-Length`。
6. `Content-Disposition` 由用户在 headers 中配置，首版不单独做下载开关。

如果文件记录存在但副本丢失，返回 `500`，错误摘要写入请求日志。

## CORS

路由级 CORS 配置保存在 `cors_json`：

```ts
interface ApiMockCorsConfig {
  enabled: boolean;
  allowOrigin: string;
  allowMethods: string[];
  allowHeaders: string;
  exposeHeaders: string;
  allowCredentials: boolean;
  maxAgeSeconds: number | null;
}
```

默认值：

```json
{
  "enabled": true,
  "allowOrigin": "*",
  "allowMethods": [],
  "allowHeaders": "*",
  "exposeHeaders": "",
  "allowCredentials": false,
  "maxAgeSeconds": 600
}
```

规则：

1. `allowMethods` 为空时默认使用当前路由 Method。
2. 非 OPTIONS 请求命中启用 CORS 的路由时，附加 CORS headers。
3. OPTIONS 预检命中启用 CORS 的路由时，返回 `204`。
4. `allowCredentials=true` 时，`allowOrigin` 不能为 `*`，保存时前后端都阻断。
5. `allowHeaders` 首版用字符串保存，允许 `*` 或逗号分隔值。
6. `exposeHeaders` 首版用字符串保存，允许为空或逗号分隔值。

## IPC Action

通道统一走 `tool:api-mock:*`。

| Channel                         | Domain     | Action            | 说明                   |
| ------------------------------- | ---------- | ----------------- | ---------------------- |
| `tool:api-mock:project-list`    | `api_mock` | `project_list`    | 获取项目列表和运行摘要 |
| `tool:api-mock:project-create`  | `api_mock` | `project_create`  | 创建项目               |
| `tool:api-mock:project-update`  | `api_mock` | `project_update`  | 更新项目               |
| `tool:api-mock:project-delete`  | `api_mock` | `project_delete`  | 删除项目               |
| `tool:api-mock:project-reorder` | `api_mock` | `project_reorder` | 保存项目排序           |
| `tool:api-mock:route-list`      | `api_mock` | `route_list`      | 获取项目路由列表       |
| `tool:api-mock:route-get`       | `api_mock` | `route_get`       | 获取路由详情           |
| `tool:api-mock:route-save`      | `api_mock` | `route_save`      | 新建或更新路由         |
| `tool:api-mock:route-delete`    | `api_mock` | `route_delete`    | 删除路由               |
| `tool:api-mock:route-reorder`   | `api_mock` | `route_reorder`   | 保存路由排序           |
| `tool:api-mock:file-import`     | `api_mock` | `file_import`     | 导入文件副本           |
| `tool:api-mock:service-start`   | `api_mock` | `service_start`   | 启动项目服务           |
| `tool:api-mock:service-stop`    | `api_mock` | `service_stop`    | 停止项目服务           |
| `tool:api-mock:service-status`  | `api_mock` | `service_status`  | 查询运行状态           |
| `tool:api-mock:request-logs`    | `api_mock` | `request_logs`    | 获取最近请求日志       |

## 前端交互

页面建议采用三块布局：

1. 左侧项目列表：项目名、监听地址、端口、运行状态、启动/停止按钮。
2. 中间路由列表：Method、路径、启用状态、响应类型、状态码。
3. 右侧详情区：项目编辑、路由编辑和最近请求日志。

项目编辑字段：

- 名称
- 描述
- 监听地址：`127.0.0.1` / `0.0.0.0`
- 端口：`1..=65535`

路由编辑字段：

- 名称
- Method
- Path Pattern
- 启用状态
- 状态码
- Content-Type
- 响应头表格
- 响应类型：文本 / 文件
- 文本 Body
- 文件选择和文件元信息
- CORS 配置折叠区

运行态规则：

1. 运行中项目可以继续编辑配置。
2. 保存后如配置快照和运行快照不一致，前端显示「需重启生效」。
3. 首版不自动重启项目。
4. 删除运行中项目时，后端先停止服务，再删除配置。

## 纯函数

前端 `utils/apiMock.ts` 放置：

1. `validateMockPathPattern`
2. `validateMockCorsConfig`
3. `normalizeMockHeaderRows`
4. `buildMockRouteSummary`
5. `deriveMockProjectRuntimeState`
6. `isMockProjectRestartRequired`
7. `getMockRouteSpecificityLabel`

后端 `api_mock.rs` 中也保留对应核心校验，前端校验只用于即时反馈，后端校验是写入和启动前的最终防线。

## 错误处理

1. 项目不存在：返回明确错误。
2. 端口非法：保存或启动时阻断。
3. 监听地址非法：保存或启动时阻断。
4. 启动时无启用路由：拒绝启动。
5. 启动时端口被占用：返回绑定失败和地址。
6. 路由语法非法：保存时阻断。
7. Method 不支持：保存时阻断。
8. 状态码不在 `100..=599`：保存时阻断。
9. CORS 配置非法：保存时阻断。
10. 文件导入失败：返回真实文件错误。
11. 文件副本丢失：请求返回 `500`，日志记录错误摘要。
12. 删除运行中项目：先停止服务；停止失败则阻断删除。
13. 配置修改但服务运行中：保存成功，前端标记需重启。

错误必须显式暴露，不做伪成功，不用空响应掩盖文件缺失或绑定失败。

## 安全边界

1. 默认监听 `127.0.0.1`。
2. `0.0.0.0` 必须由用户在项目设置中显式选择。
3. 文件返回只能读取 `<dataDir>/api-mock/files/` 下的受控副本。
4. 前端不能传任意路径让 Mock 服务读取。
5. 所有文件路径操作必须 canonicalize 后检查目录前缀。
6. 首版不执行脚本、不执行模板表达式、不解析请求体生成响应。
7. 请求日志不保存完整 header/body，避免误存敏感数据。
8. CORS 允许开放配置，但 `allowCredentials=true` 与 `allowOrigin=*` 必须拒绝。
9. 删除项目和文件副本清理只处理 API Mock 数据目录，不触碰用户原始文件。

## 后续扩展

### 从 API Workbench 生成 Mock

后续可在 API Workbench 中增加「保存为 Mock」入口，将请求 Method、URL path、响应状态、响应头和响应体快照写入 API Mock 路由。首版已预留：

- `source_request_id`
- `source_snapshot_json`

### 动态模板响应

后续可新增 `response_kind = template_body`，支持随机变量和请求上下文变量，例如：

- `{{$uuid}}`
- `{{$timestamp}}`
- `{{$randomInt(1,100)}}`
- `{{path.id}}`
- `{{query.page}}`

该能力需要单独设计模板语法、JSON 转义规则、预览、错误提示和测试，不进入首版。

## 测试计划

### Rust 单测

建议命令：

```bash
cargo test api_mock -- --nocapture
```

覆盖：

1. route pattern 校验：精确、参数、通配符。
2. route pattern 非法输入：无 `/` 前缀、非法参数名、通配符不在末尾。
3. 匹配优先级：精确 > 参数 > 通配符。
4. 同级命中按 `sort_order ASC, id ASC`。
5. Method 过滤和 404。
6. CORS headers 生成。
7. `allowCredentials=true` 与 `allowOrigin=*` 拒绝。
8. OPTIONS 预检命中启用 CORS 路由。
9. 文件导入复制到 API Mock 数据目录。
10. 文件路径校验拒绝目录外路径。
11. 启动和停止本地 HTTP 服务。
12. 静态响应返回状态码、Content-Type、响应头和 Body。
13. 文件响应返回 Content-Length 和文件内容。
14. 文件副本丢失返回 500。
15. 多项目不同端口并行运行。
16. 端口占用启动失败。
17. 删除运行中项目先停止服务。

### 前端单测

建议命令：

```bash
pnpm test src/utils/apiMock.test.ts
```

覆盖：

1. path pattern 校验提示。
2. CORS 配置校验。
3. Header 行归一化。
4. 运行态派生。
5. 需重启状态比较。
6. 路由优先级展示文案。

### 集成验证

按影响面执行：

```bash
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

实现涉及 Rust 服务运行时，至少补充一次本地 HTTP 冒烟：启动项目后请求静态路由、文件路由、OPTIONS 预检和不存在路径。

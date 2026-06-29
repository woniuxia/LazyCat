# 接口调试工具设计

## 概述

新增独立工具「接口调试」，内部工具 ID 为 `api-workbench`。该工具面向个人和小团队在完全离线的内网环境中临时调试 HTTP 接口、保存接口集合，并从集合生成离线 Markdown 文档。

第一版集成在 LazyCat 内，不做独立应用。请求执行放在 Rust 后端，避免浏览器 CORS 限制，并复用现有桌面应用的数据目录、SQLite、设置、打包和离线交付能力。前端负责请求编辑、集合管理、环境切换、响应展示和触发文档导出；Markdown 内容由后端统一生成。

工具定位是「接口调试」，不是 Postman / Apifox 替代品。第一版只覆盖高频个人调试工作流，不引入导入、脚本、版本管理、团队协作或批量测试平台能力。

## 目标 / 非目标

### 目标

1. 新增独立工具入口「接口调试」，ID 为 `api-workbench`。
2. 支持创建多个接口集合，每个集合下可维护文件夹和接口。
3. 支持每个集合配置多个环境，环境名用户自定义，同一集合内不重复。
4. 每个环境自动包含默认变量 `BASE_URL`，作为相对请求 URL 的默认 HTTP 前缀。
5. 支持请求编辑：Method、URL、Query、Headers、Body、Timeout。
6. 支持 Body 类型：`none`、`json`、`text`、`form-urlencoded`。
7. 支持变量语法 `{{name}}`，在 URL、Query、Headers 和 Body 中解析。
8. 支持响应展示：状态码、耗时、响应头、响应体、JSON 格式化。
9. 支持最近请求历史，保存请求和响应摘要，限制响应全文大小。
10. 支持从集合导出 Markdown 接口文档。
11. 离线运行，不新增运行时公网依赖。

### 非目标

1. 第一版不支持导入 Postman Collection、OpenAPI、curl 或其他外部格式。
2. 第一版不支持前置脚本、后置脚本、断言脚本或脚本沙箱。
3. 第一版不做接口版本管理、差异对比或变更审批。
4. 第一版不做团队协作、权限、审计或远程同步。
5. 第一版不做批量执行、自动化测试报告、CI 集成或压测。
6. 第一版不做 Mock Server。
7. 第一版不做 multipart 文件上传；后续按实际需求扩展。
8. 第一版不做复杂变量作用域，只支持全局变量和当前集合环境变量。
9. 第一版不实现跟随重定向执行能力；界面只预留禁用态开关占位。

## 用户流程

### 临时调试

1. 用户进入「接口调试」工具。
2. 选择或新建一个集合。
3. 在顶部选择当前环境，例如「开发」「测试」「预发」。
4. 在请求编辑区填写 Method、URL、Query、Headers、Body。
5. 如果 URL 是 `/api/users` 这类相对路径，发送前自动拼接当前环境的 `BASE_URL`。
6. 点击发送，后端执行请求。
7. 右侧或下方面板展示状态码、耗时、响应头和响应体。
8. 请求完成后写入最近历史。

### 保存接口

1. 用户调试完一个请求后点击保存。
2. 选择集合、文件夹，填写接口名称和说明。
3. 保存请求配置，不保存本次响应全文到接口定义。
4. 后续点击接口列表项可恢复请求编辑区。

### 管理环境

1. 用户在集合设置中新增环境。
2. 环境名可自定义，例如「本地」「开发」「测试」。
3. 每个环境默认包含 `BASE_URL` 变量，用户填写对应服务地址。
4. 用户可新增普通变量，例如 `TOKEN`、`ORG_ID`。
5. 切换环境后，同一接口使用当前环境变量重新解析。
6. 用户显式切换当前环境时，后端保存该集合的 `active_environment_id`。

### 生成文档

1. 用户在集合菜单点击「导出 Markdown」。
2. 前端请求后端读取集合、文件夹、接口定义。
3. 生成 Markdown 文档，包含集合名、环境变量名、接口分组、请求示例和响应示例摘要。
4. 默认不导出环境变量真实值，不导出敏感 Header 值。

## 产品规则

### 命名

- 工具显示名：`接口调试`
- 工具 ID：`api-workbench`
- 后端 domain：`api_workbench`
- 前端组件：`ApiWorkbenchPanel.vue`

### 集合与环境

1. 集合是接口、文件夹和环境的归属边界。
2. 每个集合可有多个环境。
3. 同一集合内环境名唯一。
4. 每个集合必须有一个当前环境；创建集合时自动创建一个「开发」环境，并写入 `active_environment_id`。
5. 用户切换当前环境必须走后端 action，后端校验环境属于当前集合后再保存。
6. 每个环境自动具备 `BASE_URL` 变量。
7. `BASE_URL` 可以为空，但当请求 URL 是相对路径时必须有值。
8. 环境变量名大小写敏感，`BASE_URL` 为环境级保留名，不能删除。
9. 全局变量不允许使用 `BASE_URL` 作为变量名；`BASE_URL` 只存在于当前集合环境，避免全局值和环境值产生遮蔽歧义。
10. 删除环境时，后端拒绝删除集合内最后一个环境；如果删除的是当前环境，删除成功后自动切换到同集合内 `sort_order ASC, id ASC` 的第一个剩余环境。
11. 环境变量值第一版按明文存储；文档导出时默认隐藏值。后续可接 Vault 或加密存储。

### URL 与 `BASE_URL`

请求最终 URL 由以下规则得到：

1. 先解析 URL 中的 `{{name}}` 变量。
2. 解析后的 URL 以 `http://` 或 `https://` 开头时，直接作为完整 URL 使用。
3. 解析后的 URL 为相对路径时，自动拼接当前环境 `BASE_URL`。
4. URL 允许显式使用 `{{BASE_URL}}/path`；变量解析后它会成为完整 URL，不再额外追加 `BASE_URL`。
5. 拼接时统一处理斜杠，避免 `//` 或缺失 `/`。
6. 如果 URL 是相对路径且 `BASE_URL` 为空，发送前返回明确错误。
7. 只支持 `http` 和 `https` 协议；其他协议直接拒绝。
8. Query 表格中的启用项在最终 URL 之后追加；如果 URL 本身已有 query string，则合并追加。

示例：

```text
BASE_URL = http://127.0.0.1:8080
URL      = /api/users
final    = http://127.0.0.1:8080/api/users
```

```text
BASE_URL = http://127.0.0.1:8080/
URL      = api/users
final    = http://127.0.0.1:8080/api/users
```

### 变量解析

变量语法固定为 `{{name}}`。

解析范围：

- URL
- Query key / value
- Header key / value
- Body 文本

变量来源：

1. 全局变量。
2. 当前集合当前环境变量。

优先级：

```text
当前环境变量 > 全局变量
```

规则：

1. 未解析变量必须显式报错，不能静默替换为空字符串。
2. 变量名只允许字母、数字、下划线和短横线，长度 1 到 64。
3. 变量解析只做字符串替换，不执行表达式。
4. 请求发送前后端重新解析并校验，前端预览仅用于提示。
5. 文档导出默认保留 `{{name}}`，不替换为真实变量值。
6. 保存全局变量时，如果变量名为 `BASE_URL`，后端直接拒绝。

### 请求编辑

第一版支持 Method：

- `GET`
- `POST`
- `PUT`
- `PATCH`
- `DELETE`
- `HEAD`
- `OPTIONS`

请求字段：

```ts
interface ApiWorkbenchRequestDraft {
  method: string;
  url: string;
  query: Array<{ enabled: boolean; key: string; value: string }>;
  headers: Array<{ enabled: boolean; key: string; value: string }>;
  bodyType: "none" | "json" | "text" | "form-urlencoded";
  body: string;
  form: Array<{ enabled: boolean; key: string; value: string }>;
  timeoutMs: number;
}
```

行为：

1. Query 与 Headers 支持启用 / 禁用。
2. Body 类型为 `json` 时，发送前校验 JSON 格式；格式错误时不发送。
3. Body 类型为 `form-urlencoded` 时，以启用的表单行编码为 `application/x-www-form-urlencoded`。
4. 若用户未手动设置 `Content-Type`，根据 Body 类型自动补默认值。
5. Timeout 第一版范围为 100ms 到 120000ms，默认 10000ms。
6. 请求编辑区预留「跟随重定向」开关占位，第一版禁用态展示，不写入请求草稿、不发送后端、不影响请求执行。

### 响应展示

响应结果包含：

```ts
interface ApiWorkbenchSendResult {
  finalUrl: string;
  status: number | null;
  statusText: string;
  ok: boolean;
  durationMs: number;
  requestHeaders: Array<{ key: string; value: string }>;
  responseHeaders: Array<{ key: string; value: string }>;
  bodyText: string;
  bodySize: number;
  bodyTruncated: boolean;
  contentType: string;
  error: string | null;
}
```

展示规则：

1. 2xx / 3xx / 4xx / 5xx 都视为有 HTTP 响应，展示状态码和响应体。
2. DNS、连接失败、TLS 错误、超时等视为请求错误，`status = null`。
3. JSON 响应自动格式化；格式化失败时展示原文。
4. 响应体保存和展示设置最大字节数，第一版建议 2MB；超出后截断并提示。
5. 二进制响应第一版不做专门预览，仅展示大小、类型和截断说明。
6. 第一版明确不自动跟随重定向；3xx 响应按原始响应展示 `Location` 等响应头。界面中的「跟随重定向」开关只是禁用态占位。

## 前端接入

### 工具入口

修改：

- `apps/desktop/src/App.vue`：在侧边栏合适分组加入 `api-workbench`。
- `apps/desktop/src/composables/toolCatalog.ts`：加入 `{ id: "api-workbench", name: "接口调试", desc: "离线 HTTP 接口调试与文档生成" }`。
- `apps/desktop/src/tool-registry.ts`：注册 `ApiWorkbenchPanel.vue`。
- `apps/desktop/src/bridge/tauri.ts`：新增 `tool:api-workbench:*` channel 映射。

新增：

- `apps/desktop/src/components/ApiWorkbenchPanel.vue`
- `apps/desktop/src/types/api-workbench.ts`
- `apps/desktop/src/utils/apiWorkbench.ts`
- `apps/desktop/src/utils/apiWorkbench.test.ts`

### 页面结构

第一版采用三栏布局：

1. 左栏：集合、文件夹、接口树，提供新建、重命名、删除、导出文档入口。
2. 中栏：请求编辑区，包含 Method、URL、环境选择、Query、Headers、Body。
3. 右栏或下栏：响应区和历史记录，展示状态、耗时、响应体、响应头。

关键交互：

1. 环境选择放在请求 URL 附近，避免用户忘记当前环境。
2. `BASE_URL` 缺失时，在发送按钮附近给出明确错误。
3. 未保存的请求修改需要显示脏状态。
4. 删除集合、文件夹、接口、环境前做二次确认。
5. 文档导出前提示不会导出敏感变量真实值。

### 纯函数

以下逻辑优先放到 `utils/apiWorkbench.ts` 并配套单测：

1. 变量名校验。
2. `{{name}}` 变量提取与解析。
3. `BASE_URL` 与相对 URL 拼接。
4. 请求草稿归一化。
5. 响应内容类型与 JSON 格式化展示判断。

## 后端接入

新增：

- `apps/desktop/src-tauri/src/tools/api_workbench.rs`

修改：

- `apps/desktop/src-tauri/src/tools/mod.rs`：注册 `api_workbench` domain。
- `apps/desktop/src-tauri/src/tools/helpers.rs`：新增 SQLite 表和索引迁移。
- `apps/desktop/src-tauri/Cargo.toml`：第一版优先复用已有 `ureq`；如果实现时证书、代理或更复杂请求能力不足，再评估引入 `reqwest`。

### IPC action

通道映射：

| Channel | Action | 说明 |
|---|---|---|
| `tool:api-workbench:list` | `list` | 获取集合树、环境摘要和最近历史 |
| `tool:api-workbench:collection-create` | `collection_create` | 创建集合并初始化默认环境 |
| `tool:api-workbench:collection-update` | `collection_update` | 更新集合名称和描述 |
| `tool:api-workbench:collection-set-active-environment` | `collection_set_active_environment` | 设置集合当前环境 |
| `tool:api-workbench:collection-delete` | `collection_delete` | 删除集合 |
| `tool:api-workbench:folder-create` | `folder_create` | 创建文件夹 |
| `tool:api-workbench:folder-update` | `folder_update` | 更新文件夹 |
| `tool:api-workbench:folder-delete` | `folder_delete` | 删除文件夹 |
| `tool:api-workbench:request-get` | `request_get` | 获取接口详情 |
| `tool:api-workbench:request-save` | `request_save` | 新建或更新接口 |
| `tool:api-workbench:request-delete` | `request_delete` | 删除接口 |
| `tool:api-workbench:environment-list` | `environment_list` | 获取集合环境和变量 |
| `tool:api-workbench:environment-save` | `environment_save` | 新建或更新环境 |
| `tool:api-workbench:environment-delete` | `environment_delete` | 删除环境 |
| `tool:api-workbench:global-variables-list` | `global_variables_list` | 获取全局变量 |
| `tool:api-workbench:global-variables-save` | `global_variables_save` | 保存全局变量 |
| `tool:api-workbench:send` | `send` | 解析变量并执行请求 |
| `tool:api-workbench:history-list` | `history_list` | 获取请求历史 |
| `tool:api-workbench:history-clear` | `history_clear` | 清空历史 |
| `tool:api-workbench:export-markdown` | `export_markdown` | 导出集合 Markdown 文档 |

### 请求执行策略

1. 后端接收请求草稿、集合 ID 和环境 ID。
2. 后端读取当前环境变量和全局变量。
3. 后端解析变量、拼接最终 URL、校验协议。
4. 后端按 Body 类型生成请求体和 Content-Type。
5. 后端用 HTTP 客户端执行请求。
6. 后端限制响应体读取大小，返回截断标记。
7. 后端写入历史摘要。
8. 第一版后端显式关闭自动跟随重定向，确保 3xx 作为原始 HTTP 响应返回。后续启用「跟随重定向」时再扩展请求草稿、存储字段和执行策略。

第一版优先使用现有 `ureq`：

- 当前项目已有 `ureq = { version = "2", features = ["tls"] }`。
- 现有 `network.rs` 已使用 `ureq` 做 HTTP 探测。
- 同步请求模型足够覆盖第一版调试场景，能避免新增依赖和构建风险。

若后续需要代理、自签证书细粒度控制、客户端证书、流式下载或 multipart 文件上传，再单独评估迁移到 `reqwest`。

## 数据模型

### `api_workbench_collections`

```sql
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
```

### `api_workbench_folders`

```sql
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
```

第一版 UI 可以只暴露一层文件夹；表结构允许后续扩展多层。

### `api_workbench_requests`

```sql
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
```

`example_response_json` 只保存用户显式选择的示例响应摘要，不自动保存每次响应全文。

### `api_workbench_environments`

```sql
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
```

### `api_workbench_environment_variables`

```sql
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
```

创建环境时必须写入 `BASE_URL` 变量。删除变量时后端拒绝删除 `BASE_URL`。

删除环境时必须在事务中处理当前环境指针：拒绝删除集合内最后一个环境；删除当前环境后，将集合的 `active_environment_id` 切换到同集合内 `sort_order ASC, id ASC` 的第一个剩余环境。

### `api_workbench_global_variables`

```sql
CREATE TABLE IF NOT EXISTS api_workbench_global_variables (
  name TEXT PRIMARY KEY,
  value TEXT NOT NULL DEFAULT '',
  is_secret INTEGER NOT NULL DEFAULT 0,
  sort_order INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

### `api_workbench_history`

```sql
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
```

历史保留策略：

1. 默认最多保留 200 条。
2. 单条响应体预览最大 64KB。
3. 写入新历史后清理超出数量的旧历史。

## Markdown 文档生成

Markdown 文档由纯函数生成，输入为集合、文件夹、接口定义和环境变量名。

生成职责固定在 Rust 后端：`export_markdown` 读取集合数据后调用后端纯函数返回 Markdown 字符串和建议文件名。前端不重复实现 Markdown 模板，只负责触发导出、展示确认提示和保存 / 复制结果。

默认包含：

1. 集合名称和描述。
2. 环境列表，只展示环境名和变量名，不展示变量值。
3. 按文件夹分组的接口列表。
4. 每个接口展示 Method、URL、说明、Query、Headers、Body 示例。
5. Header 中 `Authorization`、`Cookie`、`X-Api-Key`、`X-Auth-Token` 等敏感项默认脱敏。
6. 如果接口保存了示例响应摘要，则展示状态码、Content-Type 和截断后的响应示例。

不包含：

1. 当前环境变量真实值。
2. 历史响应全文。
3. 用户未显式保存的临时请求。

## 错误处理

错误必须显式暴露，不做伪成功：

1. 集合、环境或请求不存在：返回明确错误。
2. 相对 URL 缺少 `BASE_URL`：发送前阻断。
3. 未解析变量：列出变量名并阻断。
4. 非 HTTP/HTTPS 协议：阻断。
5. JSON Body 格式错误：阻断。
6. 请求超时：返回超时错误和耗时。
7. 响应体超限：返回截断标记，不视为请求失败。
8. 设置当前环境时，如果环境不存在或不属于当前集合，返回明确错误。
9. 保存全局变量时，如果包含 `BASE_URL`，返回明确错误。

## 安全与隐私

1. 请求从本机发出，只访问用户填写的地址，不依赖公网 CDN 或外部服务。
2. 文档导出默认不导出变量值和敏感 Header 值。
3. 历史记录只保存响应体预览，避免无限保存敏感响应。
4. 变量第一版按明文 SQLite 存储；如果后续需要存储长期密钥，应接入 Vault 或增加加密存储设计。
5. 不引入脚本运行能力，避免离线工具内出现不必要的执行风险。

## 测试计划

### Rust 单测

覆盖：

1. `BASE_URL` 与相对 URL 拼接。
2. 完整 URL 不追加 `BASE_URL`。
3. 非 HTTP/HTTPS 协议拒绝。
4. 变量解析、未解析变量报错、环境变量优先级。
5. 全局变量拒绝 `BASE_URL`。
6. 当前环境设置校验、删除当前环境后的自动切换、删除最后环境拒绝。
7. JSON Body 校验。
8. form-urlencoded 编码。
9. 历史记录数量和响应预览截断。
10. Markdown 导出脱敏规则。
11. 本地 HTTP server 请求执行成功、HTTP 3xx/4xx/5xx 仍返回原始响应。

建议命令：

```bash
cargo test api_workbench -- --nocapture
```

### 前端单测

覆盖：

1. 变量提取和解析。
2. URL 拼接。
3. 请求草稿归一化。
4. 响应内容类型与 JSON 格式化展示判断。

建议命令：

```bash
pnpm test src/utils/apiWorkbench.test.ts
```

### 集成验证

按影响面执行：

```bash
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

## 后续扩展

后续按真实使用需求评估：

1. curl / Postman / OpenAPI 导入。
2. 前置 / 后置脚本和断言。
3. 批量运行与测试报告。
4. multipart 文件上传。
5. 代理、自签证书、客户端证书。
6. 接入 Vault 管理敏感变量。
7. 接口版本管理和变更对比。
8. 启用「跟随重定向」开关，并补齐请求草稿、存储字段和后端执行策略。

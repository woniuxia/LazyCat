# 接口调试个人高频闭环设计

## 概述

本次迭代继续完善「接口调试」工具，目标是优化个人离线开发时最高频的调试链路：从外部拿到一个接口命令，快速导入调通，复用历史结果沉淀为接口，并把可复现信息复制给终端或文档。

现有版本已经支持集合、文件夹、接口树、环境变量、请求发送、历史记录和 Markdown 导出。本版不把工具扩展成 Postman / Apifox 替代品，也不引入团队协作、脚本系统或 Mock Server。重点是补齐日常个人开发中最常发生的四类动作：

1. 粘贴 cURL 快速生成请求。
2. 把当前请求复制为 cURL，便于终端复现。
3. 把历史调试结果保存为接口。
4. 对响应和接口树做更快的复制、搜索和快捷键操作。

## 目标

1. 支持从 cURL 命令导入请求草稿。
2. 支持把当前请求导出为 cURL 命令。
3. 支持把历史记录保存为接口。
4. 支持响应体、响应头、最终 URL 和请求 cURL 的快速复制。
5. 支持响应体 JSON 美化 / 原文展示切换。
6. 支持把当前响应保存为接口的示例响应。
7. 支持集合内接口搜索，按接口名称、Method、URL 和文件夹名称过滤左侧接口树。
8. 支持常用快捷键：`Ctrl+Enter` 发送、`Ctrl+S` 保存接口、`Esc` 关闭浮层。
9. 补齐环境管理入口：新增、重命名、复制、删除环境。
10. 展示当前请求引用的变量、缺失变量和变量来源，减少发送前的不确定性。
11. 保持完全离线运行，不新增运行时公网依赖。

## 非目标

1. 不支持完整 Postman Collection 导入。
2. 不支持 OpenAPI 导入。
3. 不支持前置脚本、后置脚本、断言脚本或脚本沙箱。
4. 不支持批量执行、自动化测试报告或 CI 集成。
5. 不实现 Mock Server。
6. 不实现 multipart 文件上传。
7. 不改变当前“不自动跟随重定向”的执行策略。
8. 不引入账号、同步、分享、权限或团队协作能力。
9. 不做全局跨集合搜索，只做当前集合内接口搜索。

## 用户流程

### 从 cURL 导入调试

1. 用户点击「导入 cURL」。
2. 弹窗中粘贴 `curl` 命令。
3. 前端解析并展示预览：Method、URL、Headers、Body 类型和 Body 内容。
4. 用户确认后覆盖当前请求草稿。
5. 如果当前没有选中集合，导入只填充草稿，不自动创建集合。
6. 用户发送请求，按现有环境变量和后端校验规则执行。

### 复制当前请求为 cURL

1. 用户在请求编辑区点击「复制 cURL」。
2. 后端根据当前集合、环境、请求草稿解析变量并生成 cURL。
3. 生成结果包含最终 URL、启用的 Header、Query 和实际会发送的 Body。
4. 敏感 Header 不主动脱敏，因为该命令用于本机终端复现；用户复制前可手动检查。
5. 变量缺失时返回明确错误，不生成不可执行命令。

### 历史保存为接口

1. 用户在历史记录右侧点击「保存为接口」。
2. 弹窗选择目标文件夹，默认放到当前选中集合的未分组。
3. 接口名称默认使用历史记录名称；如果为空，使用 `METHOD path`。
4. 新接口草稿从历史记录还原 Method、URL 和可保存的基础信息。
5. 当前历史表没有完整请求头和请求体，第一版只从历史记录保存可用字段，不伪造缺失的 Headers / Body。
6. 保存成功后刷新集合树并打开新接口。

### 响应沉淀为示例

1. 用户发送请求后点击「保存为示例响应」。
2. 只有当前请求已保存为接口时可直接保存。
3. 当前请求未保存时，先提示用户保存接口。
4. 示例响应写入 `api_workbench_requests.example_response_json`。
5. 示例响应保存状态码、Content-Type、响应头、响应体预览、是否截断、保存时间。
6. Markdown 导出继续使用后端单一真源读取示例响应。

### 集合内搜索

1. 左侧接口树上方增加搜索框。
2. 输入关键词后，只过滤当前集合内接口。
3. 搜索匹配字段：接口名称、Method、URL、文件夹名称。
4. 文件夹在自身命中或后代接口命中时显示；文件夹名称命中时显示该文件夹下的完整直接结构。
5. 搜索不改变后端排序，不写入数据库。
6. 清空搜索后恢复原树和原展开态。

## 产品规则

### cURL 导入范围

支持常见形式：

```text
curl http://127.0.0.1:8080/api/users
curl -X POST http://127.0.0.1:8080/api/users -H "Content-Type: application/json" -d '{"name":"Tom"}'
curl 'http://127.0.0.1:8080/api/users?page=1' -H 'Authorization: Bearer token'
```

第一版支持参数：

- `-X` / `--request`
- `-H` / `--header`
- `-d` / `--data` / `--data-raw` / `--data-binary`
- `--url`
- `-G`

解析规则：

1. 不带 `-X` 且存在 data 时，Method 默认为 `POST`。
2. 不带 `-X` 且不存在 data 时，Method 默认为 `GET`。
3. `-H "A: B"` 解析为 Header 行；没有冒号的 Header 返回解析错误。
4. `Content-Type` 包含 `application/json` 时，Body 类型为 `json`。
5. `Content-Type` 包含 `application/x-www-form-urlencoded` 时，优先解析为 `form-urlencoded`；解析失败则保留为 `text`。
6. 其他 data 默认作为 `text`。
7. URL 中已有 query string 时，解析为 `draft.query`，URL 本体保留不带 query 的地址。
8. `-G` 表示 data 作为 query 参数追加，不作为 Body。
9. 不执行 shell 命令，不展开环境变量，不读取本地文件。
10. `--data` / `--data-binary` 的值以 `@` 开头时按文件引用处理并返回解析错误，提示第一版不读取本地文件内容；`--data-raw` 中的 `@` 保持字面量。
11. 遇到不支持的复杂参数时返回明确提示，用户可手动调整后再导入。

### cURL 导出范围

导出由后端生成，避免前后端各实现一套变量解析。

导出规则：

1. 使用当前后端发送路径相同的变量解析规则。
2. 只导出启用的 Query、Headers 和实际会发送的 Body。
3. `bodyType = none` 时不输出 data 参数。
4. `bodyType = json` 或 `text` 时输出 `--data-raw`。
5. `bodyType = form-urlencoded` 时输出编码后的 `--data-raw` 并补 `Content-Type`。
6. Payload 显式传入目标 Shell：`targetShell = "powershell" | "bash"`，第一版默认 `powershell`。
7. 后端按目标 Shell 生成对应转义；不承诺同一条命令同时兼容 PowerShell 和 Bash。
8. PowerShell 使用单引号并把 `'` 转义为 `''`；Bash 使用单引号并用 `'\''` 处理内部单引号。
9. Header 或 Body 含换行时返回明确错误，提示用户改用手动粘贴 Body，避免生成跨 Shell 不稳定命令。
10. 3xx 跟随策略保持现状，不输出 `-L`。

### 历史转接口

当前历史表只保存请求摘要和响应预览，不保存完整请求头、请求体和 query 行。因此第一版历史转接口遵循真实可用数据，不补假数据。

保存规则：

1. 使用 `history.method`、`history.url` 作为请求草稿来源。
2. `history.url` 是发送前用户输入 URL，`history.finalUrl` 只用于展示和响应说明。
3. 新接口 `description` 写入来源历史的状态码、耗时、最终 URL 和创建时间。
4. 如果历史记录有关联 `collection_id`，默认保存到该集合。
5. 如果历史记录没有集合，保存到当前选中集合；没有当前集合时要求用户先选择集合。
6. 保存后不删除历史。

### 示例响应

示例响应字段以 JSON 字符串保存，结构为：

```ts
interface ApiWorkbenchExampleResponse {
  status: number | null;
  statusText: string;
  contentType: string;
  headers: Array<{ enabled: true; key: string; value: string }>;
  bodyText: string;
  bodySize: number;
  bodyTruncated: boolean;
  savedAt: string;
}
```

规则：

1. `bodyText` 使用当前响应展示范围内的文本，不重新发请求。
2. 如果响应体已被截断，示例响应明确记录 `bodyTruncated = true`。
3. 保存示例响应不改变请求草稿。
4. Markdown 导出遇到示例响应时展示状态码、响应头和响应体预览。

### 环境管理

在当前「环境」页签补齐管理入口：

1. 新增环境：默认从当前环境复制变量，名称为「新环境」并允许用户修改。
2. 重命名环境：复用 `environment_save`，后端校验同集合唯一。
3. 复制环境：以当前环境变量创建新环境，名称默认 `原名称 副本`。
4. 删除环境：复用现有 `environment_delete`，后端拒绝删除最后一个环境。
5. 删除当前环境后，前端使用后端返回或重新拉取结果同步当前环境。

### 变量引用提示

请求编辑区增加变量摘要，不阻断编辑。

展示内容：

1. 当前请求引用的变量名。
2. 每个变量来源：当前环境、全局变量、缺失。
3. `BASE_URL` 对相对 URL 的影响。
4. 缺失变量用警告样式展示。

规则：

1. 前端摘要仅用于提示。
2. 发送、cURL 导出仍以后端校验为准。
3. 只检查当前 `bodyType` 实际会发送的 Body / Form，避免隐藏字段污染结果。

## 前端架构

### `ApiWorkbenchPanel.vue`

继续作为总编排组件，负责：

1. 请求编辑、发送、保存和响应展示。
2. 当前集合、环境、请求状态。
3. 调用 cURL 导入、cURL 导出、保存示例响应、历史保存为接口和环境保存 action。
4. 管理快捷键绑定和浮层关闭。

本次不把整个面板重构拆分，避免扩大改动面。只把新增的复杂纯逻辑放入 `utils`，把新增弹窗控制留在面板内。

### `ApiWorkbenchSidebar.vue`

新增搜索输入和过滤展示：

1. 接收 `searchQuery` 或内部维护搜索文本。
2. 使用纯函数过滤当前集合树。
3. 搜索时自动展开命中的文件夹路径。
4. 不改变 `collections` 原始数据。

### `utils/apiWorkbenchCurl.ts`

新增纯函数模块：

```ts
interface ApiWorkbenchCurlParseResult {
  draft: ApiWorkbenchRequestDraft;
  warnings: string[];
}

function parseApiWorkbenchCurl(input: string): ApiWorkbenchCurlParseResult;
```

职责：

1. 将 cURL 字符串 token 化，支持单双引号和反斜杠转义。
2. 解析支持范围内的参数。
3. 生成请求草稿和非阻断警告。
4. 对不完整或明显错误的命令返回可读错误。

### `utils/apiWorkbenchSearch.ts`

新增纯函数模块：

```ts
function filterApiWorkbenchCollection(
  collection: ApiWorkbenchCollection,
  query: string,
): ApiWorkbenchCollection;
```

职责：

1. 按接口名称、Method、URL 过滤 requests。
2. 保留命中接口的祖先文件夹。
3. 保留文件夹自身名称命中时的全部直接结构。
4. 保持原有 `sortOrder` 和 id 顺序。

### `utils/apiWorkbenchVariables.ts`

新增纯函数模块：

```ts
interface ApiWorkbenchVariableUsage {
  name: string;
  source: "environment" | "global" | "missing";
}

function summarizeApiWorkbenchVariables(input: {
  draft: ApiWorkbenchRequestDraft;
  environmentVariables: ApiWorkbenchVariable[];
  globalVariables: ApiWorkbenchVariable[];
}): ApiWorkbenchVariableUsage[];
```

职责：

1. 只从实际发送路径提取变量。
2. 合并 URL、Query、Headers、当前 Body / Form。
3. 标记变量来源。
4. 保持与后端变量名规则一致。

## 后端接口设计

继续使用 `api_workbench` domain。

新增 action：

| Channel | Action | 说明 |
|---|---|---|
| `tool:api-workbench:export-curl` | `export_curl` | 根据当前草稿和环境生成 cURL |
| `tool:api-workbench:history-save-request` | `history_save_request` | 将历史记录保存为接口 |
| `tool:api-workbench:request-save-example-response` | `request_save_example_response` | 保存当前响应为接口示例响应 |

环境新增、重命名和复制复用既有 `tool:api-workbench:environment-save` / `environment_save`，避免新增重复入口。

### `export_curl`

Payload：

```json
{
  "collectionId": 1,
  "environmentId": 2,
  "targetShell": "powershell",
  "draft": {
    "method": "POST",
    "url": "/api/users",
    "query": [],
    "headers": [],
    "bodyType": "json",
    "body": "{\"name\":\"Tom\"}",
    "form": [],
    "timeoutMs": 10000
  }
}
```

Response：

```json
{
  "shell": "powershell",
  "command": "curl -X POST 'http://127.0.0.1:8080/api/users' -H 'Content-Type: application/json' --data-raw '{\"name\":\"Tom\"}'"
}
```

后端必须复用发送路径中的变量解析、URL 构造、Body 准备和归属校验。

### `history_save_request`

Payload：

```json
{
  "historyId": 10,
  "collectionId": 1,
  "folderId": null,
  "name": "POST /api/users"
}
```

Response：

```json
{
  "id": 42
}
```

规则：

1. 后端校验历史存在。
2. 后端校验目标集合和文件夹归属一致。
3. 保存时使用同级下一个 `sort_order`。
4. 新接口的草稿字段只能来自历史记录已有字段。

### `request_save_example_response`

Payload：

```json
{
  "requestId": 42,
  "collectionId": 1,
  "response": {
    "status": 200,
    "statusText": "OK",
    "contentType": "application/json",
    "headers": [{ "enabled": true, "key": "Content-Type", "value": "application/json" }],
    "bodyText": "{\"ok\":true}",
    "bodySize": 11,
    "bodyTruncated": false,
    "savedAt": "2026-06-30T10:00:00+08:00"
  }
}
```

Response：

```json
{
  "ok": true
}
```

规则：

1. 后端校验 request 属于 collection。
2. 后端限制示例响应 JSON 最大保存体积，沿用当前响应体限制。
3. 保存后更新请求 `updated_at`。

### `environment_save`

环境新增、重命名和复制继续复用既有 `environment_save`。

新增 / 复制 Payload：

```json
{
  "collectionId": 1,
  "name": "本地 副本",
  "variables": [
    { "name": "BASE_URL", "value": "http://127.0.0.1:8080", "isSecret": false }
  ]
}
```

重命名 / 更新 Payload：

```json
{
  "id": 5,
  "collectionId": 1,
  "name": "本地",
  "variables": [
    { "name": "BASE_URL", "value": "http://127.0.0.1:8080", "isSecret": false }
  ]
}
```

Response：

```json
{
  "id": 5,
  "collectionId": 1,
  "name": "本地"
}
```

规则：

1. 后端校验集合存在。
2. 后端校验环境名同集合唯一。
3. 后端校验变量名合法。
4. 如果变量中没有 `BASE_URL`，自动补空值。

## 数据模型

本版不新增表。

复用字段：

1. `api_workbench_requests.example_response_json` 保存示例响应。
2. `api_workbench_history` 继续保存历史摘要。
3. `api_workbench_environments` 和 `api_workbench_environment_variables` 继续保存环境。

可选迁移：

1. 如果历史表后续需要完整请求快照，再新增请求快照字段；本版不做。
2. 如果环境复制需要标识来源环境，第一版不落库，只按变量值复制。

## 错误处理

1. cURL 导入解析失败时，弹窗内展示错误，不修改当前草稿。
2. cURL 导入只有警告时允许确认导入。
3. cURL 导出变量缺失时，不写剪贴板，展示缺失变量名。
4. 历史保存为接口时，如果目标集合或文件夹不存在，提示用户刷新后重试。
5. 保存示例响应时，如果当前接口已删除，提示用户重新保存接口。
6. 环境删除最后一个环境时，展示后端错误。
7. 搜索无结果时，左侧展示「当前集合无匹配接口」。

## 验证计划

Rust：

```powershell
cargo test api_workbench -- --nocapture
```

前端单测：

```powershell
pnpm test src/utils/apiWorkbench.test.ts src/utils/apiWorkbenchTree.test.ts src/utils/apiWorkbenchCurl.test.ts src/utils/apiWorkbenchSearch.test.ts src/utils/apiWorkbenchVariables.test.ts
```

类型检查：

```powershell
pnpm typecheck
```

必要时执行渲染层构建：

```powershell
pnpm --filter @lazycat/desktop build:web
```

## 分阶段交付

### 阶段 1：cURL 和响应复制闭环

1. cURL 导入。
2. cURL 导出。
3. 响应体、响应头、最终 URL 复制。
4. JSON 美化 / 原文切换。

### 阶段 2：历史和示例沉淀

1. 历史保存为接口。
2. 当前响应保存为示例响应。
3. Markdown 导出展示示例响应。

### 阶段 3：搜索、变量提示和环境管理

1. 当前集合接口搜索。
2. 变量引用摘要。
3. 环境新增、重命名、复制、删除。
4. 快捷键。

每个阶段都应保持独立可验证，不要求一次性大改完成。

## 风险与取舍

1. cURL 解析不追求覆盖所有 shell 语法。第一版覆盖常见调试命令，复杂脚本由用户手动调整。
2. 历史转接口不保存历史中没有的请求头和请求体，避免制造不真实的接口定义。
3. cURL 导出放在后端，减少变量解析和 Body 准备的双重真值。
4. 搜索只作用于当前集合，避免在接口量不大时过早引入跨集合索引。
5. 示例响应复用已有字段，不新增表，降低迁移和回滚成本。

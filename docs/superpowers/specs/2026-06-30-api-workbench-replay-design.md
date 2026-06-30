# 接口调试复现闭环设计

## 概述

本次迭代继续完善「接口调试」工具，目标是让个人离线开发中“刚刚调通的一次请求”可以被完整复现、重新发送、沉淀为接口，并能在历史中快速找回。

现有版本已经具备集合、文件夹、接口树、环境变量、请求发送、cURL 导入导出、历史记录、示例响应和 Markdown 导出。但当前历史记录只保存请求摘要和响应预览，缺少完整请求头、请求体、表单、查询参数和环境变量快照。因此历史复用和历史保存为接口只能恢复 `method/url`，会丢失关键上下文。

本版以“历史请求快照”为核心补齐复现链路。请求发送后，后端保存当次发送使用的完整请求草稿和环境变量快照；历史项可以一键重放、载入编辑器、保存为完整接口，并支持标星、命名、备注和搜索。

## 目标

1. 每次发送请求后，历史记录保存完整请求快照。
2. 历史记录保存发送时的环境快照，便于解释当时变量解析来源。
3. 支持从历史记录一键重放请求。
4. 支持把历史记录载入当前编辑器，不立即发送。
5. 支持把带快照的历史记录保存为完整接口定义。
6. 支持历史记录标星、重命名和备注。
7. 支持历史列表按关键词搜索和按标星筛选。
8. 旧历史记录没有快照时明确降级，只恢复 `method/url`，不伪造缺失字段。
9. 保持完全离线运行，不新增运行时公网依赖。

## 非目标

1. 不做 multipart 文件上传。
2. 不做 Cookie Jar 自动管理。
3. 不启用跟随重定向。
4. 不做 OpenAPI、Postman Collection 或 HAR 导入。
5. 不做脚本、断言、批量执行、自动化报告或 CI 集成。
6. 不做 Mock Server。
7. 不接入团队协作、账号、同步、权限或审计。
8. 不新增复杂变量作用域；仍沿用当前“环境变量优先于全局变量”的规则。
9. 不把历史响应全文无限保存；仍保留当前响应体大小限制和截断策略。

## 用户流程

### 发送后自动形成可复现历史

1. 用户在接口调试面板填写或载入请求。
2. 用户点击发送。
3. 后端按现有发送路径解析变量、构造最终 URL、准备 Headers 和 Body。
4. 后端执行请求并返回响应。
5. 后端写入历史记录，同时保存：
   - 发送前的规范化请求草稿。
   - 发送时选中的集合、环境和接口引用。
   - 发送时的环境变量快照。
   - 最终 URL、响应摘要、响应体预览和错误信息。
6. 前端刷新历史列表，最新记录可立即重放或保存为接口。

### 一键重放历史

1. 用户在历史列表点击「重放」。
2. 前端调用后端 `history_replay`。
3. 后端读取历史中的请求快照和环境快照。
4. 后端使用快照执行请求，不依赖当前编辑器草稿。
5. 重放结果写入一条新的历史记录，来源指向原历史。
6. 前端展示新的响应，并把响应页签切到「响应」。

### 载入历史到编辑器

1. 用户在历史列表点击「载入」。
2. 如果历史存在请求快照，前端用快照覆盖当前请求草稿。
3. 如果历史没有请求快照，前端只恢复 `method/url`，并提示“旧历史仅包含摘要”。
4. 载入不会立即发送，也不会自动保存接口。
5. 当前集合和环境不自动切换；如果历史关联的集合或环境仍存在，前端可以提示用户切换。

### 历史保存为接口

1. 用户在历史列表点击「保存为接口」。
2. 用户选择目标集合和目标文件夹。
3. 如果历史存在请求快照，后端用快照创建完整接口定义。
4. 如果历史没有请求快照，后端沿用旧降级逻辑，只写 `method/url`，并在接口说明中标明“来源历史无完整请求快照”。
5. 保存成功后前端刷新集合树并打开新接口。

### 历史整理

1. 用户可对历史记录标星，标星记录不会被普通历史数量清理策略删除。
2. 用户可给历史记录设置显示名称和备注。
3. 用户可在历史页搜索关键词。
4. 用户可切换“全部 / 标星”视图。
5. 清空历史时，前端二次确认；默认清空非标星历史，并提供“同时清空标星历史”的明确选项。

## 产品规则

### 请求快照

请求快照保存发送时的规范化草稿：

```ts
interface ApiWorkbenchHistoryRequestSnapshot {
  method: ApiWorkbenchMethod;
  url: string;
  query: ApiWorkbenchKeyValueRow[];
  headers: ApiWorkbenchKeyValueRow[];
  bodyType: ApiWorkbenchBodyType;
  body: string;
  form: ApiWorkbenchKeyValueRow[];
  timeoutMs: number;
}
```

规则：

1. 快照保存的是发送前用户输入的请求草稿，不保存变量解析后的替换结果。
2. `query`、`headers` 和 `form` 保留 enabled 状态。
3. 发送时未参与的 Body 分支仍按草稿保存，便于载入后继续编辑。
4. 重放时只使用当前 `bodyType` 对应的 Body 或 Form，保持与现有发送路径一致。
5. `request_snapshot_json` 序列化后最大 2MB，超过时阻断发送并返回明确错误，不写入不完整历史。

### 环境快照

环境快照用于复现发送时的变量值：

```ts
interface ApiWorkbenchHistoryEnvironmentSnapshot {
  environmentId: number | null;
  environmentName: string;
  variables: Array<{
    name: string;
    value: string;
    isSecret: boolean;
    source: "environment" | "global";
  }>;
}
```

规则：

1. 快照只保存发送路径可见的变量集合，包括当前环境变量和全局变量。
2. 当前环境变量优先于全局变量；被遮蔽的全局同名变量不进入有效快照。
3. `BASE_URL` 只来自当前环境，保持既有规则。
4. 本版沿用现有明文变量存储策略，不新增加密层。
5. Markdown 导出仍不输出变量真实值。
6. 历史列表默认不直接展示敏感变量值，只在用户明确查看快照详情时显示，并沿用现有敏感标记。
7. `environment_snapshot_json` 序列化后最大 256KB，超过时阻断发送并提示用户减少环境变量体积。

### 重放语义

重放的目标是复现历史当时的请求，而不是使用当前环境最新变量。

规则：

1. `history_replay` 必须使用历史中的请求快照和环境快照。
2. 如果历史没有请求快照，后端拒绝重放并提示“旧历史缺少请求快照，请载入后手动发送”。
3. 如果环境快照缺失，后端拒绝重放，不回退到当前环境。
4. 重放产生的新历史记录保存自己的新响应，同时记录 `replayed_from_history_id`。
5. 重放不修改当前接口定义，不修改当前环境变量。
6. 重放不要求原集合、原环境或原接口仍然存在。

### 载入语义

载入的目标是让用户继续编辑。

规则：

1. 载入快照只更新前端草稿、请求名称建议和当前响应状态。
2. 载入不会自动发送请求。
3. 载入不会写数据库。
4. 载入旧历史时只恢复 `method/url`，Headers、Query、Body 保持空值。
5. 如果当前草稿有未保存修改，前端必须二次确认。

### 历史保存为接口

规则：

1. 有请求快照时，创建接口时完整写入 `method/url/query/headers/body_type/body_text/form/timeout_ms`。
2. 无请求快照时，保留现有降级行为，只写入 `method/url`。
3. 接口说明写入来源历史的状态码、耗时、最终 URL、创建时间和是否来自完整快照。
4. 目标文件夹必须属于目标集合。
5. 保存后不删除原历史。

### 历史整理

规则：

1. 标星字段为历史记录自身状态，不影响原请求或集合。
2. 标星历史不参与 `MAX_HISTORY_ROWS` 自动清理。
3. 非标星历史仍保留当前数量限制，默认最多 200 条。
4. 历史名称为空时使用 `METHOD path` 展示。
5. 搜索匹配字段：名称、备注、Method、原始 URL、最终 URL、状态码、错误信息、Content-Type。
6. 搜索只在本地 SQLite 查询，不引入全文索引；历史量小，普通 `LIKE` 足够。

## 前端设计

### `ApiWorkbenchPanel.vue`

继续作为总编排组件，负责：

1. 请求编辑、发送、保存和响应展示。
2. 历史列表状态、搜索条件和标星筛选。
3. 调用历史重放、历史更新、历史保存为接口等 action。
4. 处理“载入历史覆盖未保存草稿”的确认。

本次不整体拆分面板，避免扩大改动面。历史列表内部复杂逻辑优先抽到纯函数。

### 历史列表交互

历史页签顶部增加：

1. 搜索输入框。
2. “全部 / 标星”分段控件。
3. 清理按钮。

每条历史显示：

1. Method、名称、URL。
2. 状态码、耗时、创建时间。
3. 快照标识：完整快照 / 摘要历史。
4. 标星按钮。
5. 操作：重放、载入、保存为接口、重命名/备注。

### `utils/apiWorkbenchHistory.ts`

新增纯函数模块：

```ts
function canReplayApiWorkbenchHistory(item: ApiWorkbenchHistoryItem): boolean;

function buildApiWorkbenchDraftFromHistory(
  item: ApiWorkbenchHistoryItem,
): {
  draft: ApiWorkbenchRequestDraft;
  degraded: boolean;
};

function defaultApiWorkbenchHistoryDisplayName(item: ApiWorkbenchHistoryItem): string;
```

职责：

1. 判断历史是否可重放。
2. 从历史快照构造请求草稿。
3. 对旧历史执行 `method/url` 降级恢复。
4. 生成稳定展示名称。

### 类型更新

在 `types/api-workbench.ts` 中扩展：

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
  requestSnapshot: ApiWorkbenchHistoryRequestSnapshot | null;
  environmentSnapshot: ApiWorkbenchHistoryEnvironmentSnapshot | null;
  createdAt: string;
}
```

## 后端设计

继续使用 `api_workbench` domain。

### 新增 action

| Channel | Action | 说明 |
|---|---|---|
| `tool:api-workbench:history-replay` | `history_replay` | 使用历史快照重放请求 |
| `tool:api-workbench:history-update` | `history_update` | 更新历史名称、备注、标星 |

### 调整 action

| Channel | Action | 调整 |
|---|---|---|
| `tool:api-workbench:send` | `send` | 写历史时保存请求快照和环境快照 |
| `tool:api-workbench:history-list` | `history_list` | 返回快照存在状态、标星、备注和快照 JSON |
| `tool:api-workbench:history-clear` | `history_clear` | 默认只清空非标星历史，可显式清空全部 |
| `tool:api-workbench:history-save-request` | `history_save_request` | 优先用请求快照创建完整接口 |

### `history_list`

Payload：

```json
{
  "query": "login",
  "pinnedOnly": false,
  "limit": 200
}
```

规则：

1. `query` 为空时返回最近历史。
2. `pinnedOnly = true` 时只返回标星历史。
3. `limit` 最大 200，缺省为 200。
4. 搜索在 SQLite 中用普通 `LIKE` 完成。
5. 搜索匹配字段固定为：名称、备注、Method、原始 URL、最终 URL、状态码、错误信息、Content-Type。
6. 结果始终按 `created_at DESC, id DESC` 返回，不因为搜索改变排序。

### `history_replay`

Payload：

```json
{
  "historyId": 10
}
```

Response：

```json
{
  "finalUrl": "http://127.0.0.1:8080/api/users",
  "status": 200,
  "statusText": "OK",
  "ok": true,
  "durationMs": 42,
  "requestHeaders": [],
  "responseHeaders": [],
  "bodyText": "{\"ok\":true}",
  "bodySize": 11,
  "bodyTruncated": false,
  "contentType": "application/json",
  "error": null,
  "historyId": 11
}
```

规则：

1. 历史必须存在。
2. 历史必须有 `request_snapshot_json`。
3. 历史必须有 `environment_snapshot_json`。
4. 后端用快照构造变量映射，不读取当前环境表。
5. 后端复用发送路径的 URL 构造、Body 准备和 HTTP 执行逻辑。
6. 重放后插入新历史，`replayed_from_history_id` 指向原历史。

### `history_update`

Payload：

```json
{
  "id": 10,
  "name": "登录成功",
  "note": "本地 8080，使用管理员 token",
  "pinned": true
}
```

Response：

```json
{
  "ok": true
}
```

规则：

1. `name` 允许为空，前端展示时回退默认名称。
2. `note` 最大 2000 字符。
3. `pinned` 为布尔值。
4. 更新不存在的历史返回明确错误。

### `history_clear`

Payload：

```json
{
  "includePinned": false
}
```

规则：

1. `includePinned = false` 时只删除非标星历史。
2. `includePinned = true` 时删除全部历史。
3. 删除前由前端二次确认。

## 数据模型

### `api_workbench_history` 增量字段

新增字段：

```sql
ALTER TABLE api_workbench_history
  ADD COLUMN request_snapshot_json TEXT;

ALTER TABLE api_workbench_history
  ADD COLUMN environment_snapshot_json TEXT;

ALTER TABLE api_workbench_history
  ADD COLUMN replayed_from_history_id INTEGER;

ALTER TABLE api_workbench_history
  ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;

ALTER TABLE api_workbench_history
  ADD COLUMN note TEXT NOT NULL DEFAULT '';
```

新增索引：

```sql
CREATE INDEX IF NOT EXISTS idx_api_workbench_history_pinned_created
  ON api_workbench_history(pinned, created_at DESC, id DESC);
```

迁移规则：

1. 使用 `ALTER TABLE ... ADD COLUMN` 兼容历史数据库。
2. 旧历史的快照字段保持 `NULL`。
3. 旧历史的 `pinned` 默认为 `0`。
4. 旧历史的 `note` 默认为空字符串。
5. 不回填伪快照。

### 自动清理策略

写入新历史后执行清理：

1. 标星历史全部保留。
2. 非标星历史最多保留 `MAX_HISTORY_ROWS = 200` 条。
3. 清理只删除超出限制的非标星历史。
4. 重放产生的新历史按普通历史处理，除非用户手动标星。

## 发送路径复用

为避免双重真值，后端需要把现有发送逻辑拆出内部 helper：

```rust
fn execute_prepared_api_workbench_request(
    draft: &RequestDraft,
    vars: &HashMap<String, String>,
    base_url: &str,
) -> Result<Value, String>;
```

职责：

1. 解析 URL、Query、Headers、Body 和 Form。
2. 构造最终 URL。
3. 准备请求体和 Content-Type。
4. 调用 HTTP 客户端执行请求。

`send` 和 `history_replay` 都调用该 helper。差异只在变量来源：

1. `send` 从当前环境和全局变量加载，并保存快照。
2. `history_replay` 从历史环境快照加载，不读取当前环境表。

## 错误处理

1. 历史不存在：提示“历史记录不存在”。
2. 旧历史没有请求快照：重放按钮禁用；后端仍防御性返回“旧历史缺少请求快照”。
3. 历史没有环境快照：重放按钮禁用；后端返回“历史缺少环境快照”。
4. 快照 JSON 解析失败：返回“历史快照已损坏”，不尝试猜测恢复。
5. 历史保存为接口时目标集合不存在：提示刷新后重试。
6. 历史保存为接口时目标文件夹不属于集合：返回明确错误。
7. 更新历史备注超过长度限制：返回明确错误。
8. 清空历史前端必须二次确认，且明确是否包含标星历史。

## 验证计划

Rust 单测：

```powershell
cargo test api_workbench -- --nocapture
```

重点覆盖：

1. `send` 写入请求快照和环境快照。
2. `history_replay` 使用快照重放，不读取当前环境变量。
3. 旧历史没有快照时拒绝重放。
4. 带快照历史保存为接口时保留 Headers、Query、Body 和 Timeout。
5. 无快照历史保存为接口时保持降级行为。
6. 标星历史不被自动清理。
7. `history_clear` 默认保留标星历史。
8. `history_update` 校验备注长度和历史存在性。

前端单测：

```powershell
pnpm test src/utils/apiWorkbench.test.ts src/utils/apiWorkbenchTree.test.ts src/utils/apiWorkbenchHistory.test.ts
```

重点覆盖：

1. 有快照历史可重放。
2. 无快照历史不可重放。
3. 从快照构造完整请求草稿。
4. 从旧历史降级构造 `method/url` 草稿。
5. 默认历史展示名称稳定。

类型检查：

```powershell
pnpm typecheck
```

必要时执行渲染层构建：

```powershell
pnpm --filter @lazycat/desktop build:web
```

## 分阶段交付

### 阶段 1：历史快照落库

1. 数据库迁移新增历史快照和标星字段。
2. `send` 写入请求快照和环境快照。
3. `history_list` 返回快照字段。
4. 保持现有 UI 行为不变。

### 阶段 2：重放和载入

1. 新增 `history_replay`。
2. 前端历史项增加「重放」和「载入」。
3. 旧历史禁用重放并展示降级提示。
4. 重放结果写入新历史并展示响应。

### 阶段 3：历史沉淀增强

1. `history_save_request` 优先使用快照创建完整接口。
2. 新增历史标星、命名和备注。
3. 新增历史搜索和标星筛选。
4. 调整清空历史策略，默认保留标星历史。

每个阶段都应保持独立可验证，不要求一次性完成全部 UI 优化。

## 风险与取舍

1. 保存环境变量快照会增加历史中敏感信息留存。本版沿用现有明文策略，但 UI 默认不直接展示敏感值，后续可接 Vault 或加密存储。
2. 不对旧历史回填伪快照，避免制造看似完整但实际缺字段的接口定义。
3. 重放使用历史环境快照，而不是当前环境，能最大化复现能力；代价是无法自动使用最新 token。用户需要最新变量时，应载入到编辑器后手动发送。
4. 标星历史不自动清理，便于保留关键调试记录；代价是长期使用后可能占用更多本地 SQLite 空间。
5. 本版先用普通 `LIKE` 做历史搜索，不引入 FTS，避免为最多数百条历史增加额外索引和维护成本。

# Spotlight 接入数据字典设计

## 背景

数据字典已经提供全局/当前字典搜索、记录详情、显示字段摘要和完整 JSON 查看能力。Spotlight 目前通过 provider 体系接入工具、凭据、Hosts、Todo、PM、Launcher 等数据源，结果支持默认动作和 `Tab` 动作菜单。

本次目标是让 Spotlight 能搜索数据字典记录，展示记录的显示字段，并支持从动作菜单复制字段值和完整 JSON。

## 目标

1. 数据字典作为默认启用的 Spotlight provider，在用户输入有效关键词后参与全局搜索。
2. 支持 provider alias 限定搜索，默认 alias 为 `dd` 和 `dict`。
3. Spotlight 结果行展示记录标题、字典来源和显示字段摘要。
4. `Enter` 默认打开数据字典工具并定位到该记录。
5. `Tab` 动作菜单支持逐个复制显示字段值，并支持复制完整 `rawJson`。

## 非目标

1. 不为每个数据字典新增独立 Spotlight scope 或自定义别名。
2. 不新增数据库表或数据迁移。
3. 不改变数据字典主面板现有搜索排序语义。
4. 不在 Spotlight 中展示多行详情卡片或完整 JSON 预览。
5. 不在 Spotlight 空输入状态预取或展示数据字典记录。

## 方案概述

采用兼容扩展数据字典 `search` 返回结构，并为 Spotlight 增加 query-time provider 的方案。后端在每条搜索结果中直接返回 `title` 和 `summary`，Spotlight provider 在用户输入关键词后按需请求少量结果，再映射为 `SpotlightItem`。

这样可以复用数据字典后端已有的标题字段和显示字段规则，避免 Spotlight 前端重复实现字段路径解析、字段排序和摘要生成逻辑。同时避免在 Spotlight 空输入或窗口预创建时把大量数据字典记录、尤其是完整 `rawJson` 塞进通用 IPC 和 Spotlight 窗口内存。

## 后端 API

扩展 `DataDictionarySearchItem`：

```ts
interface DataDictionarySearchItem {
  id: number;
  dictionaryId: number;
  dictionaryName: string;
  titleFieldPath: string | null;
  rowIndex: number;
  rawJson?: unknown;
  matches: DataDictionaryMatch[];
  title: string;
  summary: DataDictionaryRecordSummaryPart[];
}
```

扩展 `search` 请求参数：

```ts
interface DataDictionarySearchRequest {
  scope: "current" | "all";
  dictionaryId?: number;
  keyword?: string;
  limit?: number;
  includeRawJson?: boolean;
}
```

`includeRawJson` 默认保持 `true`，兼容数据字典主面板现有调用；Spotlight 调用时传 `includeRawJson: false`，搜索结果不携带完整原始 JSON。复制完整 JSON 时再通过 `tool:data-dictionary:record-detail` 按记录懒加载。

`title` 使用现有 `build_record_title` 生成。`summary` 使用现有 `build_record_summary` 生成，规则保持为：

1. 只包含 `visible = true` 的字段。
2. 按 `sort_order ASC, field_path ASC` 排列。
3. 排除标题字段。
4. 字段标签优先使用 `display_name`，其次 `meaning`，最后 `field_path`。
5. 字段值使用现有紧凑文本格式。

`rows_to_search_items` 内按字典缓存字段配置，避免跨字典搜索时重复加载同一字典字段。记录 JSON 解析失败时返回显式错误，不生成 `Value::Null` 伪结果。

## Spotlight Query-Time Provider

当前 Spotlight 主要使用 `prefetch` + 本地 fuzzy 排序模型。数据字典记录数量和单条 JSON 体积都不可控，不适合空输入预取，因此为 provider 增加可选 query-time 接口：

```ts
interface ProviderDescriptor {
  prefetch: () => Promise<SpotlightItem[]>;
  search?: (query: string, ctx: SpotlightSearchContext) => Promise<SpotlightItem[]>;
}

interface SpotlightSearchContext {
  scope: SpotlightProviderId | null;
}
```

Spotlight 搜索流程：

1. 空输入时只使用 `prefetch` 数据，不调用 query-time provider。
2. 普通全局搜索中，关键词长度 `>= 2` 时调用启用的 query-time provider。
3. alias 限定搜索中，关键词长度 `>= 1` 时调用对应 provider 的 query-time 搜索。
4. query-time 请求使用请求序号绑定当前 query 和 scope，旧响应不能覆盖新结果。
5. query-time 结果与本地预取结果合并后继续走现有排序和结果上限；同一 `providerId:itemId` 去重。
6. provider 请求失败时只返回该 provider 空结果或保留已有预取结果，不阻断其他 provider。

数据字典 provider 的 `prefetch` 返回空数组，所有记录检索都走 `search`。

## Spotlight Provider

新增 `apps/desktop/src/spotlight/providers/data-dictionary.ts`。

Provider 配置：

1. `id`: `data-dictionary`
2. `name`: `数据字典`
3. `badgeShort`: `典`
4. `defaultAliases`: `["dd", "dict"]`
5. `defaultEnabled`: `true`

`search(query, ctx)` 调用 `tool:data-dictionary:search`：

1. `scope: "all"`。
2. `keyword` 使用 Spotlight 解析 alias 后的真实关键词。
3. `limit: 50`。
4. `includeRawJson: false`。

空关键词不请求数据字典。普通全局搜索少于 2 个字符时不请求；`dd` / `dict` alias 限定搜索少于 1 个字符时不请求。

每个 `SpotlightItem`：

1. `title` 使用后端返回的 `title`。
2. `subtitle` 由字典名和前几项显示字段摘要拼接，例如 `用户字典 · 编号：1001 · 姓名：张三`，过长时裁剪。
3. `searchFields` 包含标题、字典名、显示字段标签和值、匹配字段值。
4. `payload` 保存 `recordId`、`dictionaryId` 和 `summary`，不保存完整 `rawJson`。
5. `status` 可显示可复制显示字段数量，例如 `3 字段`；没有显示字段时不显示。

默认动作调用 `spotlight_pick`，目标为 `data-dictionary`，`itemId` 为记录 ID。

## 动作菜单

`buildActions(item)` 返回：

1. 每个显示字段一个复制动作，动作 ID 使用稳定前缀加字段路径或索引，例如 `copy_field:<index>`。
2. 最后一项固定为 `copy_raw_json`，标签为 `复制完整 JSON`。

`executeAction` 行为：

1. `copy_field:*` 复制对应 `summary` 项的 `value`。
2. `copy_raw_json` 调用 `tool:data-dictionary:record-detail` 拉取记录详情，复制格式化后的完整 `rawJson`，使用两空格缩进。
3. 复制成功后关闭 Spotlight 并显示成功提示。
4. 复制失败时返回 `errorMessage`，不关闭窗口。

字段不存在或摘要为空时不生成对应字段复制动作。空字符串字段沿用现有摘要规则，不作为本次 Spotlight 接入的单独行为变更。

## 打开并定位记录

新增 `useDataDictionaryNavigation`，模式对齐 `useTodoNavigation` 和 `usePmNavigation`：

```ts
interface DataDictionaryFocusRequest {
  recordId: number;
}
```

`App.vue` 在 `hotkey-navigate` 中处理 `target === "data-dictionary"`：

1. 解析 `itemId` 为数字。
2. 调用 `useDataDictionaryNavigation().requestFocus(recordId)`。
3. `onSelect("data-dictionary")` 打开数据字典面板。

`DataDictionaryPanel.vue` 消费 focus request 后定位记录：

1. 切到全局搜索上下文。
2. 优先尝试在当前搜索结果中选中目标记录。
3. 如果当前结果不存在目标记录，则调用 `tool:data-dictionary:record-detail` 拉取详情，并设置为当前选中详情。
4. 定位失败时显示明确错误，不伪装成功。

该设计不修改用户当前关键词，也不为定位执行大范围重搜。

## 前端注册

需要同步更新：

1. `SpotlightProviderId` 增加 `data-dictionary`。
2. `SpotlightPanel.vue` 导入数据字典 provider。
3. `SpotlightSettings.vue` 导入数据字典 provider，保证设置页能展示和配置该 provider。
4. `App.vue` 增加数据字典定位分支。

## 错误处理

1. 数据字典为空或搜索失败时，provider 返回空数组，不阻断 Spotlight 其他 provider。
2. 后端搜索返回单条记录解析失败时，保持现有搜索错误策略，不静默生成伪结果。
3. 复制字段值失败时显示 Spotlight 错误条。
4. 复制完整 JSON 的详情加载或序列化失败时显示 `复制 JSON 失败`。
5. 打开定位失败时由数据字典面板显示错误。

## 测试

Rust：

1. 覆盖 `search` 返回 `title` 和 `summary`。
2. 覆盖显示字段摘要排除标题字段。
3. 覆盖显示字段顺序和标签优先级。

TypeScript：

1. 新增数据字典 provider 单测，覆盖 `SpotlightItem` 构造。
2. 覆盖 `searchFields` 包含标题、字典名和显示字段值。
3. 覆盖逐字段复制动作和 `copy_raw_json`。
4. 覆盖无显示字段时只保留 `复制完整 JSON`。
5. 覆盖空输入不请求数据字典、全局搜索少于 2 个字符不请求、alias 搜索 1 个字符可请求。
6. 覆盖 `copy_raw_json` 通过 `record-detail` 懒加载完整 JSON，搜索候选 payload 不携带 `rawJson`。

组件/集成：

1. 更新 Spotlight provider 注册相关测试。
2. 覆盖 `hotkey-navigate` 到数据字典 focus request 的分支。
3. 覆盖 query-time provider 请求序号，旧响应不能覆盖当前 query 结果。

验证命令：

```powershell
cargo test data_dictionary -- --nocapture
pnpm test src/spotlight/providers/data-dictionary.test.ts
pnpm typecheck
```

如实现触及数据字典面板定位逻辑，再补充对应组件测试。

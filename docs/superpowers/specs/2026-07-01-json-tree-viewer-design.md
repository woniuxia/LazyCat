# 通用 JSON 树视图与数据字典接入设计

## 概述

数据字典详情区当前使用 `<pre class="dd-json-view">{{ selectedJson }}</pre>` 直接展示格式化 JSON。它能完整呈现原始记录，但面对嵌套对象或数组时无法折叠，用户查看大 JSON 时需要反复滚动。

本设计新增一个通用只读 JSON 树组件，首个接入点是数据字典 `dd-json-view`。组件默认保持完整展开，支持对有子节点的对象和数组进行折叠，同时提供复制、展开全部、折叠全部和折到 2 层工具。组件不绑定数据字典，后续可复用于 API 响应预览、JSON 处理、JSON Schema 等页面。

首版只增强数据字典详情区的只读 JSON 展示层，不改变详情请求、关系查询、字段摘要、后端 IPC、数据库结构或 `rawJson` 作为原始记录唯一事实源的模型。

## 目标

1. 数据字典详情 JSON 区支持对象和数组节点折叠。
2. 默认全部展开，保持当前“完整展示”的行为。
3. 折叠入口采用行头摘要样式，折叠后显示 `object · N` 或 `array · N`。
4. 提供全局工具栏：复制、展开全部、折叠全部、折到 2 层。
5. 新增通用 `JsonTreeViewer` 组件，不依赖数据字典业务类型。
6. 不新增第三方 JSON viewer 依赖。
7. 树构建和展开状态计算放入纯函数，配套单元测试。
8. 保持数据字典详情区现有阅读顺序：摘要 tag、JSON 原文区、关系分组。

## 非目标

1. 不在本轮迁移 API Workbench、JSON 处理或其他页面的 JSON 展示。
2. 不实现编辑 JSON、搜索 JSON、路径复制、字段高亮或 JSONPath 定位。
3. 不持久化用户的展开/折叠偏好。
4. 不改数据字典后端、IPC、数据库或原始 JSON 存储模型。
5. 不引入 Monaco 作为此处的 JSON viewer；Monaco 折叠入口和摘要行为不符合本设计。
6. 不使用 Element Plus `el-tree` 重建展示，因为它会丢失原始 JSON 阅读感。
7. 不在首版实现超大 JSON 虚拟滚动、分块渲染或后台解析。
8. 不在首版修改 API Workbench 当前 JSON 响应预览；后续复用另开设计或计划。

## 可行性结论

当前实现入口集中在 `DataDictionaryPanel.vue` 的详情 JSON 区，`recordDetail.record.rawJson` 已经是前端可直接消费的结构化 JSON，`selectedJson` 已经能提供复制用格式化文本。因此首版可通过前端组件替换完成，不需要触碰 Rust 后端、IPC 通道、数据库迁移或数据字典记录模型。

主要风险可控：

1. `apps/desktop/src/components/common` 当前不存在，需要首版新建作为通用组件目录。
2. 递归渲染需要避免把业务组件和节点组件耦合在一起；外部只暴露 `JsonTreeViewer`。
3. 循环引用、异常对象和最大深度保护必须落在纯函数层，组件只消费构建后的树。
4. 替换旧 `<pre>` 后，需要同步清理旧 hover 复制按钮和对应源码结构测试断言。

## 组件边界

新增通用组件：

```text
apps/desktop/src/components/common/JsonTreeViewer.vue
```

`components/common` 是本次新增目录，用于放置不绑定具体工具域的可复用 Vue 组件。首版只需要导出 `JsonTreeViewer.vue`；如果实现时递归节点模板过重，可以在同目录新增内部组件，例如 `JsonTreeNode.vue`，但外部调用方不直接依赖内部节点组件。

组件职责：

1. 渲染只读 JSON 树。
2. 维护当前实例内的展开节点集合。
3. 响应工具栏操作。
4. 暴露稳定、业务无关的 props。
5. 将复制成功 / 失败反馈封装在组件内部，避免调用方重复实现复制按钮。

首版 props：

```ts
interface JsonTreeViewerProps {
  value: unknown;
  defaultExpandDepth?: number | "all";
  showToolbar?: boolean;
  copyText?: string;
  ariaLabel?: string;
}
```

默认值：

1. `defaultExpandDepth` 默认 `"all"`。
2. `showToolbar` 默认 `true`。
3. `copyText` 未传时，由组件内部使用格式化 JSON 生成复制内容。
4. `ariaLabel` 默认 `"JSON 内容"`。

数据字典只负责传入：

```vue
<JsonTreeViewer
  class="dd-json-view"
  :value="recordDetail.record.rawJson"
  :copy-text="selectedJson"
  default-expand-depth="all"
/>
```

`DataDictionaryPanel.vue` 不再直接渲染 `<pre>`，也不承担树节点生成逻辑。

内部状态：

1. `tree` 由 `value` 计算生成。
2. `expandedKeys` 为组件实例内 `Set<string>`，只保存当前树的可展开节点 key。
3. `value` 或 `defaultExpandDepth` 变化时，按 `defaultExpandDepth` 重建 `expandedKeys`，不继承上一条记录的手动展开状态。
4. 工具栏按钮只修改 `expandedKeys`，不修改输入 `value`。
5. 标量根节点没有可展开节点时，复制按钮仍可用，展开类按钮隐藏或禁用，二者择一即可，但实现内保持一致。

## 纯函数设计

新增：

```text
apps/desktop/src/utils/jsonTreeView.ts
apps/desktop/src/utils/jsonTreeView.test.ts
```

核心类型：

```ts
type JsonTreeValueType = "object" | "array" | "string" | "number" | "boolean" | "null" | "unknown";

interface JsonTreeNode {
  key: string;
  path: Array<string | number>;
  depth: number;
  label: string;
  value: unknown;
  valueType: JsonTreeValueType;
  childCount: number;
  summary: string;
  children: JsonTreeNode[];
}
```

核心函数：

1. `buildJsonTree(value: unknown): JsonTreeNode`
2. `formatJsonForCopy(value: unknown): string`
3. `summarizeJsonNode(node: JsonTreeNode): string`
4. `collectExpandableKeys(root: JsonTreeNode): Set<string>`
5. `collectExpandedKeysByDepth(root: JsonTreeNode, depth: number | "all"): Set<string>`
6. 如实现需要，可新增小型 helper，例如 `isJsonTreeExpandable(node)`、`formatJsonPrimitive(value)`、`encodeJsonTreePath(path)`，但保持在同一工具文件内。

设计规则：

1. 只有非空对象和非空数组是可展开节点。
2. 空对象和空数组显示 `{}` / `[]` 或 `object · 0` / `array · 0`，但不显示可点击折叠入口。
3. 对象字段按 `Object.entries` 原始顺序渲染。
4. 数组按自然索引顺序渲染，label 使用 `[0]`、`[1]`。
5. 节点 key 使用稳定路径编码，不使用简单 `path.join(".")`，避免字段名中包含点号或数组样式字符时冲突。
6. `value` 变化时，组件按 `defaultExpandDepth` 重建展开集合。
7. 根节点 `depth = 0`；`collectExpandedKeysByDepth(root, 2)` 表示展开根节点和第一层可展开子节点，使用户看到两层内容。
8. 路径编码必须区分对象字段 `"0"` 和数组索引 `0`，也要能区分字段名中的点号、反斜杠和方括号。
9. 树构建和复制格式化都使用安全遍历策略处理循环引用，避免一个函数安全、另一个函数仍可能递归崩溃。

## 展示规则

展开状态：

```text
- "user": { object · 3
    "name": "张三",
  + "roles": array · 3,
    "active": true
  }
```

折叠状态：

```text
+ "roles": array · 3,
+ "profile": object · 5,
```

根节点：

1. 根节点为对象或数组时，仍显示外层括号。
2. 根节点为标量时，直接显示标量值。
3. 工具栏对标量值仍显示复制按钮；展开类按钮可禁用或隐藏。
4. “折叠全部”不隐藏根节点本身；根对象 / 数组至少保留外层结构和摘要。

值渲染：

1. 字符串使用 JSON 字符串转义结果，包含引号。
2. 数字、布尔、`null` 使用 JSON 原始字面量。
3. 非 JSON 标准值（如 `undefined`、`NaN`、函数、Symbol）显示为 `String(value)` 或 `unknown`，并不作为数据字典常见路径优化对象。数据字典后端返回的是标准 JSON，此分支主要服务组件通用性。
4. 对象字段 label 使用 JSON 字符串形式，例如 `"user.name"`；数组元素 label 使用 `[0]`。
5. 展开对象 / 数组时，开括号行显示 label、类型摘要和 `{` / `[`；子节点逐行缩进；闭括号单独成行。
6. 折叠对象 / 数组时，单行显示 label、类型摘要和 `{...}` / `[...]` 语义，不渲染子节点。

工具栏：

1. 复制：复制完整格式化 JSON，不受折叠状态影响。
2. 展开全部：展开全部可展开节点。
3. 折叠全部：折叠除根之外全部节点；根节点保持可见。
4. 折到 2 层：展开深度小于 2 的可展开节点。
5. 按钮文案使用中文；图标优先使用现有 `@element-plus/icons-vue`。
6. 工具栏应位于 JSON 区顶部，不再使用旧 hover-only 圆形复制按钮，避免复制入口隐藏。

## 数据字典接入

改动范围：

1. `DataDictionaryPanel.vue` 引入 `JsonTreeViewer`。
2. 用通用组件替换现有 `dd-json-shell` 内的 `<pre>` 和独立复制按钮。
3. 保留 `selectedJson` 计算属性，作为复制文本传给组件。
4. 保留 `dd-json-view` 的视觉基调：浅色背景、细边框、等宽字体、可滚动区域。
5. 移除 `copySelectedJson` 函数和 `.dd-json-copy-btn` 相关样式，复制反馈由 `JsonTreeViewer` 接管。
6. 如 `CopyDocument` 只被旧 JSON 复制按钮使用，接入后同步移除该 import；如果其他区域仍使用则保留。
7. 更新 `DataDictionaryPanel.context-menu.test.ts` 中关于 raw JSON 复制入口的源码结构断言，改为断言使用 `JsonTreeViewer`、传入 `copy-text`，并确认关系分组仍位于 JSON 区之后。

不改：

1. `recordDetail` 请求。
2. `rawJson` 类型。
3. 关系区、摘要 tag、搜索结果。
4. 后端 `record_detail` action。
5. `formatJsonDocument` 可以继续服务 `selectedJson`；是否后续合并到 `jsonTreeView.ts` 不作为首版要求，避免扩大改动。

## 错误与边界

1. `formatJsonForCopy` 使用安全序列化：标准 JSON 正常格式化；循环引用替换为字符串 `"[Circular]"`；函数、Symbol、`undefined` 等非 JSON 值替换为 `String(value)`。
2. 复制动作由 `JsonTreeViewer` 内部处理。复制成功显示轻量成功提示；`navigator.clipboard.writeText` 失败时显示 `复制失败`，不静默吞错。
3. 循环引用理论上不来自数据字典，但通用组件需要避免递归爆栈：树构建时记录访问对象，遇到循环显示 `[Circular]` 并停止展开。
4. 树构建设置内部最大深度 `100`。超过后显示 `[Max depth reached]` 节点并停止递归，避免异常输入拖垮界面。
5. 超大 JSON 的虚拟滚动不在本轮范围；后续如 API 响应预览接入大响应，再单独设计性能策略。
6. 最大深度保护不是用户错误提示，只是渲染保护；数据字典正常记录不应触发。
7. 复制失败必须显式提示；树构建遇到异常输入时应显示可读占位值，不能让组件渲染崩溃。

## 验证

单元测试：

```text
pnpm test src/utils/jsonTreeView.test.ts
pnpm test src/components/DataDictionaryPanel.context-menu.test.ts
```

覆盖：

1. 对象、数组、标量节点生成。
2. 对象字段含点号、反斜杠、数组样式字符时 key 仍稳定不冲突。
3. 摘要输出 `object · N`、`array · N`。
4. 默认 `"all"` 展开集合包含所有可展开节点。
5. 深度展开只包含目标层级内节点。
6. 空对象和空数组不可展开。
7. 循环引用不会递归崩溃。
8. 超过最大深度时生成 `[Max depth reached]` 保护节点。
9. 安全复制文本能处理循环引用和非 JSON 标准值。
10. `collectExpandedKeysByDepth(root, 2)` 的深度语义稳定，能锁住“折到 2 层”行为。

数据字典相关回归：

```text
pnpm test src/utils/jsonTreeView.test.ts src/components/DataDictionaryPanel.context-menu.test.ts
pnpm test src/utils/dataDictionary.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

`DataDictionaryPanel.context-menu.test.ts` 当前以源码结构断言为主，本轮需要同步更新旧 JSON 复制按钮断言，避免测试继续要求已删除的 `copySelectedJson` 和 `.dd-json-copy-btn`。

如实现中组件测试能力足够，再补轻量组件测试；否则用纯函数测试、源码结构断言、typecheck 和 build:web 验证模板、props 和样式接入。

## 后续复用路径

后续页面复用时，优先只传 `value`、`copyText` 和 `defaultExpandDepth`。如果某个页面需要搜索、定位、路径复制、超大 JSON 虚拟滚动或响应体二进制兜底，应另开设计，不把这些能力提前塞进首版组件。

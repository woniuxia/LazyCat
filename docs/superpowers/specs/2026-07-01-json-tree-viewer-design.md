# 通用 JSON 树视图与数据字典接入设计

## 概述

数据字典详情区当前使用 `<pre class="dd-json-view">{{ selectedJson }}</pre>` 直接展示格式化 JSON。它能完整呈现原始记录，但面对嵌套对象或数组时无法折叠，用户查看大 JSON 时需要反复滚动。

本设计新增一个通用只读 JSON 树组件，首个接入点是数据字典 `dd-json-view`。组件默认保持完整展开，支持对有子节点的对象和数组进行折叠，同时提供复制、展开全部、折叠全部和折到 2 层工具。组件不绑定数据字典，后续可复用于 API 响应预览、JSON 处理、JSON Schema 等页面。

## 目标

1. 数据字典详情 JSON 区支持对象和数组节点折叠。
2. 默认全部展开，保持当前“完整展示”的行为。
3. 折叠入口采用行头摘要样式，折叠后显示 `object · N` 或 `array · N`。
4. 提供全局工具栏：复制、展开全部、折叠全部、折到 2 层。
5. 新增通用 `JsonTreeViewer` 组件，不依赖数据字典业务类型。
6. 不新增第三方 JSON viewer 依赖。
7. 树构建和展开状态计算放入纯函数，配套单元测试。

## 非目标

1. 不在本轮迁移 API Workbench、JSON 处理或其他页面的 JSON 展示。
2. 不实现编辑 JSON、搜索 JSON、路径复制、字段高亮或 JSONPath 定位。
3. 不持久化用户的展开/折叠偏好。
4. 不改数据字典后端、IPC、数据库或原始 JSON 存储模型。
5. 不引入 Monaco 作为此处的 JSON viewer；Monaco 折叠入口和摘要行为不符合本设计。
6. 不使用 Element Plus `el-tree` 重建展示，因为它会丢失原始 JSON 阅读感。

## 组件边界

新增通用组件：

```text
apps/desktop/src/components/common/JsonTreeViewer.vue
```

组件职责：

1. 渲染只读 JSON 树。
2. 维护当前实例内的展开节点集合。
3. 响应工具栏操作。
4. 暴露稳定、业务无关的 props。

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

设计规则：

1. 只有非空对象和非空数组是可展开节点。
2. 空对象和空数组显示 `{}` / `[]` 或 `object · 0` / `array · 0`，但不显示可点击折叠入口。
3. 对象字段按 `Object.entries` 原始顺序渲染。
4. 数组按自然索引顺序渲染，label 使用 `[0]`、`[1]`。
5. 节点 key 使用稳定路径编码，不使用简单 `path.join(".")`，避免字段名中包含点号或数组样式字符时冲突。
6. `value` 变化时，组件按 `defaultExpandDepth` 重建展开集合。

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

值渲染：

1. 字符串使用 JSON 字符串转义结果，包含引号。
2. 数字、布尔、`null` 使用 JSON 原始字面量。
3. 非 JSON 标准值（如 `undefined`、`NaN`、函数、Symbol）显示为 `String(value)` 或 `unknown`，并不作为数据字典常见路径优化对象。数据字典后端返回的是标准 JSON，此分支主要服务组件通用性。

工具栏：

1. 复制：复制完整格式化 JSON，不受折叠状态影响。
2. 展开全部：展开全部可展开节点。
3. 折叠全部：折叠除根之外全部节点；根节点保持可见。
4. 折到 2 层：展开深度小于 2 的可展开节点。

## 数据字典接入

改动范围：

1. `DataDictionaryPanel.vue` 引入 `JsonTreeViewer`。
2. 用通用组件替换现有 `dd-json-shell` 内的 `<pre>` 和独立复制按钮。
3. 保留 `selectedJson` 计算属性，作为复制文本传给组件。
4. 保留 `dd-json-view` 的视觉基调：浅色背景、细边框、等宽字体、可滚动区域。

不改：

1. `recordDetail` 请求。
2. `rawJson` 类型。
3. 关系区、摘要 tag、搜索结果。
4. 后端 `record_detail` action。

## 错误与边界

1. `formatJsonForCopy` 如果无法序列化，返回空字符串或可读错误文本；复制失败时组件通过 Element Plus message 或父层可感知失败提示处理。
2. 循环引用理论上不来自 JSON，但通用组件需要避免递归爆栈：树构建时记录访问对象，遇到循环显示 `[Circular]` 并停止展开。
3. 过深 JSON 需要递归保护；首版可设置内部最大深度保护并显示 `[Max depth reached]`，避免异常输入拖垮界面。
4. 超大 JSON 的虚拟滚动不在本轮范围；后续如 API 响应预览接入大响应，再单独设计性能策略。

## 验证

单元测试：

```text
pnpm test src/utils/jsonTreeView.test.ts
```

覆盖：

1. 对象、数组、标量节点生成。
2. 对象字段含点号、反斜杠、数组样式字符时 key 仍稳定不冲突。
3. 摘要输出 `object · N`、`array · N`。
4. 默认 `"all"` 展开集合包含所有可展开节点。
5. 深度展开只包含目标层级内节点。
6. 空对象和空数组不可展开。
7. 循环引用不会递归崩溃。

数据字典相关回归：

```text
pnpm test src/utils/dataDictionary.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

如实现中组件测试能力足够，再补轻量组件测试；否则用 typecheck 和 build:web 验证模板、props 和样式接入。

## 后续复用路径

后续页面复用时，优先只传 `value`、`copyText` 和 `defaultExpandDepth`。如果某个页面需要搜索、定位、路径复制、超大 JSON 虚拟滚动或响应体二进制兜底，应另开设计，不把这些能力提前塞进首版组件。

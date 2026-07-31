# JSON 树查看编辑扩展设计（JsonTreeViewer Extensions）

## 概述

`JsonTreeViewer`（2026-07-01 设计）目前是只读 JSON 树组件，仅数据字典详情区一个消费方。本设计把它升级为通用 JSON 树"查看 + 编辑"组件：

1. 补齐查看能力：树内搜索定位、节点路径复制、子树值复制。
2. 新增 opt-in 树内编辑：改标量值、重命名 key、增删节点、上移/下移、类型切换、撤销/重做。
3. 接入消费方：JSON 处理面板新增文本/树双模式（编辑主战场）；JWT 解码、CSV→JSON 输出接入只读树；数据字典零改动自动获得查看增强。

三个约束贯穿设计：

1. 对现有 props 纯增量、向后兼容，数据字典等既有消费方不因本设计产生改动。
2. 本轮不做性能改造（无虚拟滚动、无分块渲染）。大文档靠默认折叠深度与消费方体积闸门兜底。
3. 不新增第三方 JSON viewer/editor 依赖，继续自研树 + Element Plus。

## 目标

1. 树内搜索采用定位模式：key 与标量值大小写不敏感子串匹配，显示 `第 n/N 处`，支持上一处/下一处导航；跳转自动展开祖先链并滚动到可见位置；不隐藏未命中节点。
2. 节点级复制：右键菜单（行 hover 的 `⋯` 按钮为同一菜单的第二入口）提供"复制路径（JSONPath）"与"复制值（子树格式化 JSON）"，只读态即可用。
3. opt-in 编辑：`editable` 默认 `false`；开启后支持双击改标量值、双击重命名 key、添加子字段、数组前后插入、删除节点、上移/下移、类型切换。
4. 撤销/重做：编辑态提供 undo/redo（工具栏按钮 + Ctrl+Z / Ctrl+Y），上限 100 步，撤销同时恢复展开状态。
5. 编辑数据流受控：消费方用 `v-model:value`；编辑回流保持展开状态，外部换文档才按 `defaultExpandDepth` 重置。
6. 编辑与搜索逻辑全部落纯函数层（`jsonTreeEdit.ts`、`jsonTreeSearch.ts`），配套单元测试。
7. 分三阶段实施，每阶段独立可交付、可提交。

## 非目标

1. 不做虚拟滚动、分块渲染等性能改造；超大文档"展开全部"会卡是本轮已知边界，写入组件文档注释。
2. 不做过滤式搜索（只显示命中分支）。
3. 不做拖拽移动节点；排序诉求由菜单"上移/下移"承接。
4. 不接入收纳箱详情（内容不保证是 JSON，按需另议）。
5. 不引入第三方 JSON viewer/editor 依赖。
6. 不持久化展开状态、搜索词或 undo 栈。
7. 不做子串级 `<mark>` 高亮；命中高亮整段 label 或值文本。

## 分期

| 阶段               | 内容                                       | 交付判定                                 |
| ------------------ | ------------------------------------------ | ---------------------------------------- |
| Phase 1 查看增强   | 搜索定位；只读右键菜单（复制路径/复制值）  | 数据字典不改代码即可搜索、复制路径       |
| Phase 2 编辑内核   | patch 引擎；`editable` 交互层；undo/redo   | 组件编辑能力可用，默认只读行为与现状一致 |
| Phase 3 消费方接入 | JSON 处理面板双模式；JWT、CSV 输出只读接入 | 三个面板改造完成并通过验证               |

## 组件接口

```ts
interface JsonTreeViewerProps {
  value: unknown; // 既有
  defaultExpandDepth?: number | "all"; // 既有，默认 "all"
  showToolbar?: boolean; // 既有，默认 true
  copyText?: string; // 既有，仅影响工具栏"复制"按钮
  ariaLabel?: string; // 既有，默认 "JSON 内容"
  editable?: boolean; // 新增，默认 false
  showSearch?: boolean; // 新增，默认 true；控制工具栏内搜索区显隐
}
```

新增事件：

```ts
emits: { "update:value": [value: unknown] }
```

仅在 `editable` 且编辑成功时发出，消费方以 `v-model:value` 绑定。

状态与行为规则：

1. 组件是受控组件：每次编辑成功，基于 patch 引擎产出新根对象并 emit；父组件回写 `value` 后树重算。
2. 编辑回流判定：组件记录自己最近一次 emit 的原始根引用，`value` 变化时用 `toRaw(props.value)` 与之比较——相等判定为编辑回流，保持展开状态（迁移已在 op 应用时完成，回流不再迁移），保留 undo 栈与搜索状态；不等判定为外部换文档，按 `defaultExpandDepth` 重置展开状态，清空 undo 栈与搜索。必须经 `toRaw` 比较：消费方以 `ref` 持有文档时回传的 `value` 是 reactive Proxy，直接 `===` 会把每次编辑误判为换文档，导致展开状态与 undo 栈每次编辑后被清空。
3. 组件内部（树构建、patch 引擎、快照）一律基于 `toRaw(props.value)` 的原始对象操作，避免经 Proxy 读子节点产生 raw/proxy 混合子树、破坏结构共享；建议消费方用 `shallowRef` 持有文档，规避大文档深度代理开销。
4. `editable=false` 时不渲染任何编辑入口，行为与现状一致；右键菜单仍提供复制路径/复制值两项。
5. `copyText` 只作用于工具栏"复制"；节点级"复制值"始终使用子树数据经 `formatJsonForCopy` 生成。
6. 根节点为标量且 `editable` 时，双击根值可编辑，等价 `set-value path=[]` 的整根替换。
7. 编辑模式约定输入为标准 JSON 数据（`JSON.parse` 产物或等价结构）。含循环引用、函数等非标准值的活对象仅保证只读展示保护（`[Circular]` 等占位），patch 引擎对占位节点路径的操作返回失败。

组件内部结构（控制 SFC 膨胀）：

1. `JsonTreeViewer.vue` 保持编排职责；搜索与编辑状态各自抽成 composable：`src/composables/useJsonTreeSearch.ts`、`src/composables/useJsonTreeEditing.ts`。undo/redo、编辑回流判定、搜索导航等行为状态机全部落在 composable 内——Vue reactivity 不依赖 DOM，可在现有 node 环境 vitest 下直接测试；组件模板只做绑定。
2. 新增 `components/common/JsonTreeNodeMenu.vue`：Teleport 到 body 的右键菜单，样式使用全局变量或硬编码设计色，不依赖父容器局部 CSS 变量。
3. `JsonTreeNode.vue` 增加高亮与编辑相关 props（命中集合、当前命中 key、编辑中路径等）和意图 emits（打开菜单、请求编辑、提交、取消），保持递归渲染结构不变；行内编辑输入实现为其内部模板分支或小型子组件。

## 纯函数设计

新增：

```text
apps/desktop/src/utils/jsonTreeEdit.ts
apps/desktop/src/utils/jsonTreeEdit.test.ts
apps/desktop/src/utils/jsonTreeSearch.ts
apps/desktop/src/utils/jsonTreeSearch.test.ts
```

### patch 引擎（jsonTreeEdit.ts）

```ts
type JsonTreePath = Array<string | number>;

type JsonTreeEditOp =
  | { type: "set-value"; path: JsonTreePath; value: unknown }
  | { type: "rename-key"; path: JsonTreePath; newKey: string }
  | { type: "insert"; parentPath: JsonTreePath; key?: string; index?: number; value: unknown }
  | { type: "remove"; path: JsonTreePath }
  | { type: "move"; path: JsonTreePath; offset: -1 | 1 };

function applyJsonTreeEdit(
  root: unknown,
  op: JsonTreeEditOp,
): { ok: true; value: unknown } | { ok: false; reason: string };
```

规则：

1. 不可变更新：只沿目标路径浅克隆祖先容器，其余结构共享；单次编辑开销与路径深度成正比，与文档大小无关。
2. 失败显式返回 `reason`：重命名撞已有 key、insert 对象撞已有 key（含目标对象已存在空字符串 key 时"添加子字段"直接失败）、路径不存在或类型不匹配、insert 缺 key/index 或越界、对根执行 remove/move 等。失败时不修改文档。
3. `insert` 对对象取 `key` 追加到末尾（或语义等价位置），对数组取 `index` 插入。
4. `move` 对数组交换相邻索引；对对象调整键序（重建对象保持插入序，JSON 序列化尊重键序，因此对象移动有意义）。
5. `set-value` 是类型切换的落点：切到容器为 `{}`/`[]`，切到标量的缺省值为 string `""`、number `0`、boolean `false`、null `null`；原子树丢弃（可撤销）。

### 展开状态迁移

```ts
function migrateExpandedKeys(keys: Set<string>, op: JsonTreeEditOp): Set<string>;
```

1. `set-value`：不变（目标子树的展开 key 自然失效，渲染忽略即可）。
2. `rename-key`：以旧路径为前缀的 key 全部替换为新路径前缀。
3. `insert`/`remove`（数组）：受影响索引之后的兄弟子树 key 整体平移；`remove` 同时丢弃被删子树的 key。
4. `move`：交换两个相邻索引子树的 key 前缀；对象 move 不改路径，无需迁移。
5. 迁移只在每次编辑 op 成功应用时执行一次（emit 前）；undo/redo 直接恢复快照中的展开集合，不再做迁移计算。

### 撤销/重做

1. 每次 apply 产生新根、旧根结构共享，因此历史栈直接存快照 `{ value, expandedKeys }`，不需要逆操作模型。
2. `past` / `future` 双栈；成功编辑时 `past.push(当前快照)` 并清空 `future`；上限 100 步，超限丢最旧。
3. undo/redo 同时恢复值与展开状态，随后 emit `update:value`（同样记录 emit 引用，走编辑回流路径）。
4. 外部换文档时清空双栈。

### 搜索（jsonTreeSearch.ts）

```ts
interface JsonTreeSearchMatch {
  key: string; // 节点 key（encodeJsonTreePath 产物）
  path: JsonTreePath;
  field: "key" | "value";
}

function collectJsonTreeSearchMatches(root: JsonTreeNode, query: string): JsonTreeSearchMatch[];
function collectJsonTreeAncestorKeys(path: JsonTreePath): string[];
```

1. 大小写不敏感子串匹配；空 query 返回空集。
2. 对象/数组节点只匹配 key（label）；标量节点匹配 key 与格式化后的值文本（`formatJsonPrimitive` 产物）。同一节点 key 与 value 都命中时记两处，field 区分。
3. 结果按 DFS 文档序，供 `第 n/N 处` 导航。
4. 组件层输入防抖约 200ms；`value` 变化后重算，当前命中按 key 尽力保持，找不到则回到第 1 处。

### JSONPath 复制格式

1. 根为 `$`。
2. 对象字段名匹配 `/^[A-Za-z_$][A-Za-z0-9_$]*$/` 时用 `.name`；否则用方括号形式 `["..."]`，内部按 JSON 字符串规则转义引号与反斜杠。
3. 数组索引用 `[0]`。
4. 生成函数为 `jsonTreeView.ts` 新增的 `toJsonPath(path)`（与既有 `encodeJsonTreePath` 同域），配单测，输入为 `node.path`。

## 交互规则

### 工具栏

现有"复制 / 展开全部 / 折叠全部 / 折到 2 层"之后追加：

1. 搜索区（`showSearch=true` 时）：输入框 + `第 n/N 处` 计数 + 上一处/下一处按钮；输入框内 Enter 等价下一处、Shift+Enter 等价上一处；无命中显示"无匹配"。
2. `editable` 时追加撤销/重做按钮；快捷键 Ctrl+Z / Ctrl+Y（含 Ctrl+Shift+Z 别名）在组件容器获得焦点时生效，不做全局监听；行内编辑输入框、搜索输入框聚焦时不触发文档级 undo/redo，让位于输入框原生撤销行为。

### 搜索呈现

1. 命中节点的 label 或值整段高亮，当前命中使用更强样式。
2. 跳转时把祖先链 key 并入 `expandedKeys`（不折叠其他节点），并对目标行 `scrollIntoView`（行元素带 `data-key` 定位）。

### 节点菜单

右键与行 hover `⋯` 按钮打开同一菜单（Teleport 到 body）：

| 菜单项                  | 只读态 | 编辑态 | 说明                                                                       |
| ----------------------- | :----: | :----: | -------------------------------------------------------------------------- |
| 复制路径                |   有   |   有   | JSONPath 格式                                                              |
| 复制值                  |   有   |   有   | 子树格式化 JSON                                                            |
| 编辑值 / 重命名 key     |   -    |   有   | 与双击等价；容器节点无"编辑值"；重命名仅对象字段（数组元素与根节点不适用） |
| 添加子字段              |   -    |   有   | 对象/数组容器，追加到末尾                                                  |
| 在此前插入 / 在此后插入 |   -    |   有   | 仅数组元素；插入 `null` 并立即进入值编辑态（与添加子字段对齐）             |
| 类型切换                |   -    |   有   | string/number/boolean/null/object/array 子菜单                             |
| 上移 / 下移             |   -    |   有   | 数组挪索引；对象挪键序；边界项对应方向禁用                                 |
| 删除                    |   -    |   有   | 根节点禁用                                                                 |

删除与类型切换不弹确认，靠 undo 兜底。

### 行内编辑

1. 双击标量值进入值编辑；双击对象字段的 key 进入重命名。Enter 提交、Esc 取消；失焦视为取消（避免误提交）。
2. 值输入宽松解析：先严格 `JSON.parse`——成功则用解析结果（数字、布尔、null、带引号字符串、对象、数组均可，容器结果整体替换该子树）；失败则整段按字符串处理。
3. 重命名提交时校验同级重复 key，冲突报错不提交。
4. 添加子字段落地行为：对象插入 `"": null` 并立即进入 key 重命名态；数组插入 `null` 并立即进入值编辑态。取消时若 key 仍为空字符串，回滚该次插入——实现为弹出该条 undo 历史（`past` 栈顶），不追加反向操作。
5. 同一时刻至多一个节点处于编辑态；切换编辑目标前先结算当前编辑。

## 消费方接入

### JSON 处理面板（编辑主战场）

1. 工具栏加"文本 | 树形"切换（segmented），默认文本；文本 `input` 始终是事实源。面板重挂载（工具切换）后模式重置为文本——模块级持久化仅覆盖 input/output 文本，属预期行为。
2. 进入树形的闸门：内容 `JSON.parse` 成功且 `text.length <= 1_000_000`。不满足时 `ElMessage` 说明原因（解析失败给出错误信息；超限说明体积），停留文本模式。
3. 输入侧树形使用 `editable` 且 `default-expand-depth="2"`，避免大文档过闸门后因组件默认 `"all"` 全展开触发已知卡顿路径。
4. 树内编辑经 `v-model:value` 维护对象；每次 `update:value` 立即序列化（2 空格缩进）回写 `input`，使文本在任何时刻都是最新事实源——面板经 `<component :is>` 切换工具时直接卸载（无 keep-alive），即时回写保证卸载不丢编辑。
5. 树模式下点击任何文本类操作（格式化、压缩、各转换按钮）：`input` 已因即时回写保持最新，直接切回文本模式执行。
6. 面板区分"自身回写"与"外部写入"（对比最近一次回写的文本值）：树模式下 `input` 被外部覆写（如剪贴板建议注入 `watchPendingInput`）按换文档处理——重新过闸门，成功则刷新树，失败则退回文本模式并提示。
7. 已知预期行为：树形→文本→树形往返会重新 parse 产生新根引用，组件按换文档处理、undo 栈清空，不作为缺陷。
8. 输出区：内容为合法 JSON 且 `text.length <= 1_000_000` 时提供"文本 | 树形"只读切换（`default-expand-depth="2"`），默认文本；非 JSON 输出（XML/YAML）或超限不显示切换。
9. 数字精度为已知边界：文本经 parse、编辑、序列化后全部数字按 JS number 语义重写，超过 `Number.MAX_SAFE_INTEGER` 的大整数丢失精度。与面板现有"格式化"行为一致，但编辑场景更隐蔽（改 A 字段导致 B 大数字段悄悄变值），在面板与组件文档注释中写明；本轮不做 BigInt 保真。

### JWT 解码

1. header/payload 两个 `<pre>` 替换为只读 `JsonTreeViewer`；`showSearch=false`，工具栏保留（顺带补上此前缺失的复制入口）；内容量小，展开深度保持默认 `"all"`。
2. 解码逻辑需同步保留原始对象（当前仅存格式化字符串），组件 `value` 传对象、`copyText` 传格式化文本。
3. signature 仍为文本展示；过期标签等既有元素位置不变。

### CSV→JSON 输出

输出区加"文本 | 树形"只读切换，默认文本；树形在输出为合法 JSON 且 `text.length <= 1_000_000` 时可用（CSV 可经文件读入，输出可能远超阈值），使用 `default-expand-depth="2"`。

### 数据字典

零改动：自动获得搜索与右键复制路径/复制值。回归确认既有测试不破坏即可。

## 错误与边界

1. 编辑 op 失败：`ElMessage.error(reason)`，文档、展开状态、undo 栈均不变。
2. 外部换文档（`value` 引用不等于最近 emit 的根）：重置展开状态、清空 undo 栈、清空搜索词与命中。
3. 非 JSON 标准值（`undefined`、`NaN`、函数等，消费方直接传活对象时可能出现）：展示沿用现有 `formatJsonPrimitive` / `formatJsonForCopy` 规则；编辑产物永远是标准 JSON 值。
4. 复制（路径、值）失败显式 toast，不静默。
5. 性能边界写入组件文档注释：无虚拟滚动；大文档依赖默认折叠与消费方闸门；"展开全部"在超大文档下可能长时间阻塞渲染。
6. 搜索高亮与编辑态叠加：编辑中的行不显示命中高亮，结算后重算。
7. 外部换文档时取消一切进行中交互：关闭菜单、退出行内值编辑/重命名/空 key 插入编辑态，未提交内容丢弃；此时 undo 栈已随换文档清空，不执行空 key 插入的回滚（该插入已随旧文档整体被替换）。

## 验证

单元测试：

```text
pnpm test src/utils/jsonTreeEdit.test.ts src/utils/jsonTreeSearch.test.ts
pnpm test src/composables/useJsonTreeEditing.test.ts src/composables/useJsonTreeSearch.test.ts
pnpm test src/components/common/JsonTreeViewer.test.ts
```

测试策略：不新增 jsdom / @vue/test-utils 依赖——desktop 工作区 vitest 为 node 环境，可测行为全部下沉到纯函数与 composable（Vue reactivity API 无 DOM 可用），组件层沿用现有源码结构断言形态，与首版 spec 的降级策略一致。

覆盖：

1. 每种 op 的正常路径与失败路径（重复 key、路径失效、越界、根节点非法操作）。
2. 结构共享断言：未触路径的子树与原根引用相等。
3. `migrateExpandedKeys` 在 insert/remove/move/rename 下的位移正确性。
4. 搜索匹配语义（key/值、大小写、文档序）、祖先链收集、`toJsonPath` 转义。
5. composable 行为（无 DOM 直接测）：`update:value` 时机；undo/redo 恢复值与展开状态；编辑回流保持展开状态、外部换文档重置并取消进行中编辑态；搜索导航与祖先展开集合计算。
6. 组件层源码结构断言：`editable` 默认关闭且无编辑入口、工具栏搜索区与菜单结构存在性。`scrollIntoView`、双击、快捷键等 DOM 交互列入手动检查清单。

消费方与回归：

```text
pnpm test src/components/DataDictionaryPanel.context-menu.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

1. JSON 处理面板：闸门（解析失败/超限）、模式切换序列化回写、文本类操作自动回切。
2. JSON 处理 / JWT / CSV 三个面板当前均无测试文件，相关源码结构断言均为新增。
3. 手动检查：浅色主题下工具栏、菜单、高亮、编辑输入的视觉与空态；`element-overrides.css` 与 `theme-light.css` 联动检查。

## 后续路径

1. 虚拟滚动/大文档性能另开设计；触发条件是 API 响应预览或数据字典出现真实卡顿反馈。
2. 过滤式搜索、拖拽排序、子串级高亮按需求另议。
3. 收纳箱详情等新消费方接入各自另开小型设计。

# JSON 树查看编辑扩展实施计划

> **For Claude:** REQUIRED SUB-SKILL: 使用 superpowers:executing-plans 按任务逐个执行本计划。
> 依据设计文档：`docs/superpowers/specs/2026-07-04-json-tree-viewer-extensions-design.md`（已通过三轮评审，事实声称均经代码核验）

**Goal:** 把 `JsonTreeViewer` 从只读 JSON 树升级为通用查看+编辑组件（搜索定位、路径/子树复制、opt-in 树内编辑、撤销重做），并接入 JSON 处理面板（文本/树双模式）、JWT 解码、CSV 输出三个消费方。

**Architecture:** 三个 Phase 对应设计文档三个阶段。所有行为状态机落在纯函数（`utils/`）与 composable（`composables/`，Vue reactivity 无 DOM 可测）中，组件模板只做绑定；每个任务收尾即提交。对现有 props 纯增量，数据字典等既有消费方用法零改动。

**Tech Stack:** Vue 3 + TypeScript + Element Plus；测试为 node 环境 vitest（无 jsdom / @vue/test-utils，不新增）。

---

## 总览

| Phase              | 目标                                                                    | 关键依赖                |
| ------------------ | ----------------------------------------------------------------------- | ----------------------- |
| Phase 1 查看增强   | 搜索纯函数与 composable、toJsonPath、工具栏搜索区、节点复制菜单（只读） | 无                      |
| Phase 2 编辑内核   | patch 引擎、展开迁移、undo/回流 composable、editable 交互               | Phase 1（复用菜单组件） |
| Phase 3 消费方接入 | JSON 处理面板双模式、JWT、CSV 输出                                      | Phase 1、2              |

每个任务收尾即提交（约定式中文提交信息）；每个 Phase 结束跑验证门。

**通用命令**（在仓库根执行）：

- 单测：`pnpm test src/utils/xxx.test.ts`（可多个路径）
- 类型：`pnpm typecheck`
- 构建：`pnpm --filter @lazycat/desktop build:web`

---

## Phase 0：准备

1. 通读设计文档与本计划；确认工作区干净（`git status`）。
2. 关键现状锚点（行号为 2026-07-04 快照，执行时以搜索为准）：
   - `components/common/JsonTreeViewer.vue`（125 行）：props 接口 `:35-41`、工具栏 `:3-10`、`watch([tree, defaultExpandDepth])` 重置展开 `:53-59`、`copyJson :82`。
   - `components/common/JsonTreeNode.vue`（173 行）：行结构 `:3-29`（label `:19`、值 `:26`）、递归子节点 `:31-45`。
   - `utils/jsonTreeView.ts`（233 行）：`encodeJsonTreePath :39`、`buildJsonTree :168`、`collectExpandableKeys :176`、`collectExpandedKeysByDepth :186`、`formatJsonForCopy :211`。
   - `components/JsonProcessPanel.vue`：工具栏 `:3-15`、输入 textarea `:17`、输出 `:19`、`watchPendingInput("json-process") :117` 附近、模块级状态 `jsonProcessState`（只存 input/output 文本）。
   - `components/JwtPanel.vue`：header `<pre> :14`、payload `:18`、signature `:30`、`decoded` 仅存格式化字符串 `:49-53` 与 `:70-77`。
   - `components/CsvJsonPanel.vue`：输入 `:8`、JSON 输出 `:9`、`pickFile → csv-read-file`（文件读入，输出可远超 1MB）。
   - 消费方现状：`DataDictionaryPanel.vue :201-207` 是唯一 JsonTreeViewer 引用；`App.vue :50-53` 经 `<component :is>` 渲染面板、无 keep-alive。
   - 测试先例：`composables/useFavorites.test.ts` 在 node 环境测 composable 并 `vi.mock("element-plus")`；`JsonTreeViewer.test.ts` 为源码结构断言。

---

## Phase 1：查看增强

### Task 1.1 搜索匹配与 JSONPath 纯函数

**文件：** 新增 `utils/jsonTreeSearch.ts` + `jsonTreeSearch.test.ts`；修改 `utils/jsonTreeView.ts` + `jsonTreeView.test.ts`

1. 先写失败测试（`jsonTreeSearch.test.ts`）：
   - `collectJsonTreeSearchMatches(root, query)`：大小写不敏感子串；对象/数组节点只匹配 label，标量节点匹配 label 与 `formatJsonPrimitive` 值文本；同节点 key/value 双命中记两条（`field` 区分）；结果 DFS 文档序；空 query 返回空数组。
   - `collectJsonTreeAncestorKeys(path)`：返回根到父级的全部 `encodeJsonTreePath` key。
2. `jsonTreeView.test.ts` 增补 `toJsonPath(path)` 用例：`[] → "$"`；`["a",0,"b"] → "$.a[0].b"`；字段名不匹配 `/^[A-Za-z_$][A-Za-z0-9_$]*$/` 时用 `["..."]` 且按 JSON 规则转义引号/反斜杠（如 `a.b`、`he"llo`）。
3. 跑测确认失败，再实现两个文件。
4. 提交：`feat(json-tree): 搜索匹配与 JSONPath 纯函数`

### Task 1.2 搜索状态 composable

**文件：** 新增 `composables/useJsonTreeSearch.ts` + `useJsonTreeSearch.test.ts`

1. 接口：输入 `tree`（`Ref<JsonTreeNode>`）；暴露 `query`、`matches`、`activeIndex`、`activeKey`、`goNext()`、`goPrev()`、`revealKeys`（当前命中需展开的祖先 key 集合，含防抖后的重算）。
2. 行为测试（node 环境，`vi.useFakeTimers` 处理 200ms 防抖）：
   - 输入 query 防抖后产出 matches 与 `第 1/N 处`；`goNext/goPrev` 循环步进。
   - `tree` 变化后重算：当前命中 key 仍存在则保持 activeIndex 指向它，否则回到第 1 处；query 清空时 matches 清空。
3. 提交：`feat(json-tree): 树内搜索状态 composable`

### Task 1.3 工具栏搜索区与命中高亮

**文件：** 修改 `components/common/JsonTreeViewer.vue`、`JsonTreeNode.vue`、`JsonTreeViewer.test.ts`

1. `JsonTreeViewer` 新增 `showSearch` prop（默认 `true`）；工具栏追加搜索输入框、`第 n/N 处` 计数、上一处/下一处按钮；输入框内 Enter=下一处、Shift+Enter=上一处；无命中显示"无匹配"。
2. 跳转：把 `revealKeys` 并入 `expandedKeys`（不折叠其他节点），对目标行 `scrollIntoView`（`JsonTreeNode` 行元素加 `data-key`）。
3. `JsonTreeNode` 新增 props：`matchedKeys: Set<string>`、`activeMatchKey: string | null`；命中整段 label/值加高亮 class，当前命中更强样式（样式写在组件 scoped 内，浅色基调对齐现有配色）。
4. 更新源码结构断言：搜索区存在、`data-key` 存在、`showSearch` 默认值。
5. 验证：`pnpm test src/components/common/JsonTreeViewer.test.ts src/composables/useJsonTreeSearch.test.ts` + `pnpm typecheck`。
6. 提交：`feat(json-tree): 工具栏搜索定位与命中高亮`

### Task 1.4 节点复制菜单（只读集）

**文件：** 新增 `components/common/JsonTreeNodeMenu.vue`；修改 `JsonTreeNode.vue`、`JsonTreeViewer.vue`、`JsonTreeViewer.test.ts`

1. 菜单组件 Teleport 到 body；样式只用全局变量或硬编码设计色（Teleport 约定）；右键与行 hover `⋯` 按钮打开同一菜单。
2. 只读态两项：复制路径（`toJsonPath(node.path)`）、复制值（子树经 `formatJsonForCopy`）；复制失败显式 toast（`ElMessage`）。
3. 菜单打开期间文档变化则关闭菜单丢弃交互；模板函数 ref 只写非响应式缓存（参考数据字典右键菜单经验）。
4. 更新源码结构断言；回归数据字典测试。
5. 验证门（Phase 1 收尾）：`pnpm test src/utils/jsonTreeSearch.test.ts src/utils/jsonTreeView.test.ts src/composables/useJsonTreeSearch.test.ts src/components/common/JsonTreeViewer.test.ts src/components/DataDictionaryPanel.context-menu.test.ts` + `pnpm typecheck` + `pnpm --filter @lazycat/desktop build:web`。
6. 提交：`feat(json-tree): 节点复制路径与复制值菜单`

---

## Phase 2：编辑内核

### Task 2.1 不可变编辑 patch 引擎

**文件：** 新增 `utils/jsonTreeEdit.ts` + `jsonTreeEdit.test.ts`

1. 先写失败测试，覆盖 5 种 op（`set-value` / `rename-key` / `insert` / `remove` / `move`）：
   - 正常路径：对象/数组/根标量各形态；`move` 数组交换相邻索引、对象重建键序。
   - 失败路径（`{ ok: false, reason }` 且不改文档）：rename 撞已有 key、insert 撞已有 key（含目标已存在空字符串 key）、路径不存在/类型不匹配、index 越界、对根 remove/move、`[Circular]` 占位路径。
   - 结构共享断言：未触路径子树与原根 `toBe` 引用相等；触及路径祖先均为新引用。
   - 类型切换缺省值：容器 `{}`/`[]`，标量 `""`/`0`/`false`/`null`。
2. `migrateExpandedKeys(keys, op)` 用例：rename 前缀替换；数组 insert/remove 兄弟索引平移（remove 同时丢弃被删子树 key）；数组 move 交换前缀；对象 move 与 set-value 不变。
3. 实现（输入先 `toRaw` 归一，沿路径浅克隆）。
4. 提交：`feat(json-tree): 不可变编辑 patch 引擎`

### Task 2.2 编辑撤销重做 composable

**文件：** 新增 `composables/useJsonTreeEditing.ts` + `useJsonTreeEditing.test.ts`

1. 职责：持有 `past/future` 快照栈（`{ value, expandedKeys }`，上限 100，超限丢最旧）；`applyOp(op)` 成功后推栈、清 `future`、经回调 emit 新根并记录 `lastEmittedRaw`；`undo()/redo()` 恢复快照并 emit；`onValueChange(newValue)` 用 `toRaw` 比较 `lastEmittedRaw` 区分编辑回流与外部换文档（换文档时清双栈、清编辑态）；管理进行中编辑态（`editingKey` + 模式：值编辑/重命名/空 key 插入）。
2. 关键行为测试（`vi.mock("element-plus")` 先例）：
   - apply 成功 emit 新根；失败 toast 且栈与文档不动。
   - undo/redo 同时恢复值与展开集合；redo 在新编辑后被清空。
   - 编辑回流不清栈不重置展开；外部换文档三清空并取消进行中编辑态。
   - 空 key 插入取消 = 弹出 `past` 栈顶恢复，且**不产生 redo 项**（被弃状态不入 `future`）；数组前/后插入取消**不回滚**（`null` 保留，undo 兜底）。
3. 提交：`feat(json-tree): 编辑撤销重做状态 composable`

### Task 2.3 组件编辑交互

**文件：** 修改 `components/common/JsonTreeViewer.vue`、`JsonTreeNode.vue`、`JsonTreeNodeMenu.vue`、`JsonTreeViewer.test.ts`

1. `JsonTreeViewer`：新增 `editable` prop（默认 `false`）与 `update:value` emit；接入 `useJsonTreeEditing`；工具栏追加撤销/重做按钮；Ctrl+Z / Ctrl+Y（含 Ctrl+Shift+Z）监听在组件容器 keydown，行内编辑输入框与搜索输入框聚焦时不拦截（让位原生撤销）；`watch` 重置逻辑改走 `onValueChange` 回流判定。
2. 组件文档注释写明受控契约：消费方须原样回写 emit 的根对象（不得 clone/转换）、建议 `shallowRef` 持有文档、无虚拟滚动的性能边界、编辑产物按 JS number 语义（数字精度边界）。
3. `JsonTreeNode`：双击标量值进入值编辑、双击对象字段 key 进入重命名（仅对象字段）；行内输入 Enter 提交、Esc 取消、失焦取消；值输入宽松解析（严格 `JSON.parse` 成功用结果，失败整段按字符串）；编辑中的行不显示命中高亮。
4. `JsonTreeNodeMenu` 编辑态菜单项：编辑值/重命名、添加子字段（对象插 `"": null` 进重命名态；数组插 `null` 进值编辑态）、在此前/后插入（仅数组，插 `null` 进值编辑态）、类型切换子菜单、上移/下移（边界禁用）、删除（根禁用）；`editable=false` 时仅保留复制两项。
5. 更新源码结构断言：`editable` 默认 `false` 且默认无编辑入口、`update:value` 声明存在。
6. 验证门（Phase 2 收尾）：`pnpm test src/utils/jsonTreeEdit.test.ts src/composables/useJsonTreeEditing.test.ts src/components/common/JsonTreeViewer.test.ts src/components/DataDictionaryPanel.context-menu.test.ts` + `pnpm typecheck` + `pnpm --filter @lazycat/desktop build:web`。
7. 提交：`feat(json-tree): 树内编辑交互与撤销重做`

---

## Phase 3：消费方接入

### Task 3.1 JSON 处理面板文本/树双模式

**文件：** 修改 `components/JsonProcessPanel.vue`；新增 `components/JsonProcessPanel.test.ts`；闸门逻辑如可抽则新增 `utils/jsonProcessTree.ts` + 单测

1. 闸门纯函数（建议抽 `canEnterJsonTree(text): { ok: true; value: unknown } | { ok: false; reason: string }`）：`JSON.parse` 成功且 `text.length <= 1_000_000`；失败 reason 区分解析错误（带 message）与超限。
2. 面板：工具栏加"文本 | 树形"segmented（默认文本；重挂载重置为文本——模块级状态只存 input/output 文本，属预期）；输入侧树形 `editable` + `default-expand-depth="2"`。
3. 数据流：每次 `update:value` 立即 `JSON.stringify(v, null, 2)` 回写 `input`（卸载不丢编辑）；记录最近回写文本，`input` 与之不符的外部写入（如剪贴板注入）按换文档处理——重过闸门，成功刷新树、失败退回文本模式并提示；树模式点击文本类操作直接切回文本执行。
4. 输出区：合法 JSON 且 ≤ 1MB 时提供"文本 | 树形"只读切换（`default-expand-depth="2"`），XML/YAML 或超限不显示。
5. 面板注释写明数字精度边界（大整数经 parse/序列化丢精度，与既有格式化一致）。
6. 测试：闸门纯函数单测 + 面板源码结构断言（segmented、闸门调用、即时回写、外部写入判定均存在）。
7. 提交：`feat(json-process): 文本/树形双模式编辑`

### Task 3.2 JWT 解码接入

**文件：** 修改 `components/JwtPanel.vue`；新增 `components/JwtPanel.test.ts`

1. 解码逻辑同步保留原始对象（`headerValue`/`payloadValue`），格式化字符串继续作 `copyText`。
2. header/payload `<pre>` 替换为只读 `JsonTreeViewer`（`showSearch=false`，展开深度默认 `"all"`）；signature 与过期标签结构不动。
3. 新增源码结构断言（两处树组件、对象透传）。
4. 提交：`feat(jwt): 解码结果树形展示`

### Task 3.3 CSV 输出接入

**文件：** 修改 `components/CsvJsonPanel.vue`；新增 `components/CsvJsonPanel.test.ts`

1. 输出区加"文本 | 树形"只读切换（默认文本）：合法 JSON 且 `<= 1_000_000` 字符时可用（文件读入的输出可能远超阈值），`default-expand-depth="2"`。
2. 新增源码结构断言。
3. 提交：`feat(csv-json): 转换结果树形查看`

### Phase 3 验证门与收尾

1. 全量：`pnpm typecheck`、`pnpm --filter @lazycat/desktop build:web`、`pnpm test`。
2. 手动检查清单（浅色主题下逐项过）：
   - 工具栏搜索/撤销按钮、右键菜单、命中高亮、行内编辑输入的视觉与空态；`element-overrides.css` 与 `theme-light.css` 联动确认。
   - 数据字典：搜索大记录、复制路径/值，展开折叠回归。
   - JSON 处理：粘 1MB 边界文档进树、树内增删改后切文本核对序列化、树模式下触发剪贴板注入、双击 Esc/失焦取消。
   - JWT：粘超长 payload 观察默认全展开表现；CSV：文件读入超限输出不显示树切换。
   - `scrollIntoView`、Ctrl+Z/Y、Enter/Shift+Enter 等 DOM 交互逐项过。
3. 评估记录 `process.md`（3+ 文件复杂任务，含 toRaw 回流判定与快照式 undo 两条可沉淀经验）。

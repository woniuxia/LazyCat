# 任务清单快速添加栏实施计划

- 日期：2026-07-04
- 依据 spec：`docs/superpowers/specs/2026-07-04-todo-quick-add-design.md`（已三轮评审通过）
- 影响范围：仅 `apps/desktop` 前端，后端零改动
- 执行约定：每阶段先写失败测试再实现（TDD）；阶段验证通过即提交；严格按 spec，不顺手扩散

## 总览

| 阶段 | 产出 | 新增/修改文件 |
|------|------|---------------|
| 1 | payload 合成纯函数 + 单测 | 新增 `utils/todoQuickAdd.ts`、`utils/todoQuickAdd.test.ts`；微调 `utils/todoSchedule.ts` |
| 2 | 快速添加栏组件 + 组件测试 | 新增 `components/TodoQuickAddBar.vue`、`components/TodoQuickAddBar.test.ts` |
| 3 | 面板接线（上下文/挂载/反馈）+ 面板测试 | 修改 `components/TodoPanel.vue`；新增 `components/TodoPanel.quick-add.test.ts` |
| 4 | 全量验证与经验沉淀 | `process.md` |

以下路径均相对 `apps/desktop/src/`。

## 阶段 1：纯函数 `todoQuickAdd.ts`（TDD）

### 1.1 `todoSchedule.ts` 最小导出调整

- 将内部常量 `DEFAULT_TIME`（第 50 行，值 `"09:00"`）改为 `export const`。仅此一处，不动其他逻辑。

### 1.2 先写失败单测 `utils/todoQuickAdd.test.ts`

用例清单（对照 spec 2.3 / 2.5 / 5）：

1. 空标题、全空白标题 → 返回 `null`。
2. “今天”：`now=14:03` → eventAt 为当日 `14:05`（下一个 5 分钟刻度，取整口径与 `getCreateDraftDefaultDateTime` 一致：`floor(min/5)*5+5`）；`now=14:00` → `14:05`。
3. “今天”午夜边界：`now=23:58` → **次日** `00:00`（日期进位，不得只做分钟回绕）。
4. “明天” → 明日 `09:00`；指定日期 `2026-07-20` → 该日 `09:00`。
5. 未选日期 → payload 不含 `eventAt`。
6. 优先级：`priorityOverride` 优先；为 null 时用 `context.priorityDefault`。
7. 继承：`context.typeId`/`projectId` 为 null 时字段不出现在 payload（含“未分类”筛选解析不到 id 的降级场景）；非 null 时原样带上。
8. payload 恒含 `reminderPresets: ["none"]`，不含 `kind`（后端默认 one_off，与 QuickCapture 现状一致）；eventAt 满足 RFC3339 且 5 分钟对齐（可用 `isFiveMinuteDateTime` 断言）。

### 1.3 实现 `utils/todoQuickAdd.ts`

```ts
import type { TodoPriority } from "../types";
import { combineLocalDateTime, DEFAULT_TIME } from "./todoSchedule";

export type QuickAddDateChoice =
  | { kind: "today" }
  | { kind: "tomorrow" }
  | { kind: "date"; date: string }   // YYYY-MM-DD
  | null;

export interface QuickAddInput {
  title: string;
  dateChoice: QuickAddDateChoice;
  priorityOverride: TodoPriority | null;
}

export interface QuickAddContext {
  typeId: number | null;
  projectId: number | null;
  priorityDefault: TodoPriority;
}

export function buildQuickAddPayload(
  input: QuickAddInput,
  context: QuickAddContext,
  now = new Date(),
): Record<string, unknown> | null
```

- “今天”的实现要点：基于 `now` 用 `new Date(y, m, d, h, floor(min/5)*5 + 5)` 构造——Date 构造器分钟溢出自动进位跨日，天然覆盖 23:58 → 次日 00:00（勿复用 `getCreateDraftDefaultDateTime` 的 `% 1440` 分钟回绕，它不带日期进位）。
- “明天”/指定日期：`combineLocalDateTime(date, DEFAULT_TIME)`；本地日期字符串拼接禁止 `new Date('YYYY-MM-DD')`（项目 05.5 时间语义约定）。
- 字段裁剪：null 值不放入 payload。

**验证**：`pnpm --filter @lazycat/desktop test src/utils/todoQuickAdd.test.ts`
**提交**：`feat(todo): 快速添加 payload 合成纯函数与单测`

## 阶段 2：`TodoQuickAddBar.vue` 组件（TDD）

### 2.1 先写组件测试 `components/TodoQuickAddBar.test.ts`

- 桩模式参照 `TodoPanel.edit-focus.test.ts`：mock `../bridge/tauri` 的 `invokeToolByChannel`。
- 用例：
  1. 输入标题回车 → 调用 `tool:todo:item-create` 且 payload 形状正确 → emit `created(id)`（后端返回 `{ ok, id, rootId }`，取 `id`）→ 标题清空、焦点仍在输入框、日期/优先级选值保留。
  2. 空标题回车 → 不调用。
  3. `KeyboardEvent.isComposing === true` 的回车 → 不调用（IME 守卫，参照 `TodoPanel.vue` 第 1001 行 `onTitleEnter` 模式）。
  4. in-flight：首次回车的 Promise 未 resolve 时再次回车 → 只调用一次。
  5. Esc → 标题清空、`dateChoice`/`priorityOverride` 重置为 null。
  6. 创建失败（mock reject）→ 标题与控件值原样保留。

### 2.2 实现组件

- Props / Emits 按 spec 3.1：`context: { typeId, projectId, priorityDefault }`；`created(id: number)`。
- 内部状态仅：`title`、`dateChoice`、`priorityOverride`；in-flight 复用 `useToolInvoke()` 的 `loading`，不另设标志。
- 创建路径：`buildQuickAddPayload` 为 null 直接返回；`loading.value` 为 true 直接返回；用 `invokeWithLoading`（失败已内置 `ElMessage.error` 并返回 `undefined`，组件不重复 toast，undefined 时不清空输入）。
- 成功反馈：输入框绿色边框一闪（本地 class + 约 600ms 定时移除）；`emit("created", id)`；清空标题并 `focus()`。
- 模板结构：
  - `el-input`（placeholder `添加任务，回车创建…`，`@keydown.enter` 带 isComposing 守卫，`@keydown.esc` 重置）。
  - 日期 `el-dropdown`：今天 / 明天 / 选日期…（触发隐藏 `el-date-picker`，`value-format="YYYY-MM-DD"`）/ 清除日期；未选灰色占位“日期”，选中显示相对文案（今天 / 明天 / MM-DD）+ 单独清除（×）。
  - 优先级 `el-dropdown`：P0-P3；未手动选择时灰色显示 `context.priorityDefault`，手动选择后着色（着色 = 显式指定）+ 单独清除（×）。
- 手动值与自动默认值是独立模型：`priorityOverride` 一旦非 null 不随 `context.priorityDefault` 变化，直至 Esc 或手动清除（spec 2.5）。
- 样式：`scoped`、浅色干净风格；el-dropdown 弹层 Teleport 到 body，样式勿依赖面板局部 CSS 变量（项目 05.5）；如需覆盖 Element 变量，`element-overrides.css` 与 `theme-light.css` 双文件同步检查（项目 05.1）。

**验证**：`pnpm --filter @lazycat/desktop test src/components/TodoQuickAddBar.test.ts`
**提交**：`feat(todo): 新增任务清单快速添加栏组件`

## 阶段 3：`TodoPanel.vue` 接线

### 3.1 上下文与挂载

- 挂载点：模板中 `.toolbar` 结束标签之后、`.todo-list-scroll`（`v-if="viewMode === 'list'"`，第 50 行附近）之前，独立一行：
  `<TodoQuickAddBar v-if="viewMode === 'list'" :context="quickAddContext" @created="onQuickAddCreated" />`
  （置于滚动容器之外保证“常驻”，随列表滚动不消失。）
- `quickAddContext` computed：
  - `typeId`：`filterType === null ? null : (types.value.find(t => t.name === filterType)?.id ?? null)`（“未分类”自然解析为 null）。
  - `projectId`：`typeof filterProjectId.value === "number" ? filterProjectId.value : null`（`"none"`/null 不继承）。
  - `priorityDefault`：`filterPriority.value ?? "P2"`。

### 3.2 创建反馈

- `onQuickAddCreated(id)`：`await loadItems()` → 可见性判定 `displayActiveItems.value.some(r => r.id === id)`（最终渲染口径，spec 3.1；建议抽为 `isItemVisibleInList(id)` 小函数）：
  - 可见：`quickAddHighlightId = id`，约 1.5 秒后清除；连续录入时后一次覆盖前一次定时器（先 clearTimeout 再设新值）。
  - 不可见：`ElMessage.info("已添加，当前筛选/搜索条件下不可见")`。
- 卡片高亮：`.todo-card` 的 `:class` 数组增加 `'is-quick-add-highlight': quickAddHighlightId === row.id`；新增渐隐动画样式（背景高亮 → 透明，约 1.5s）。

### 3.3 面板侧测试 `components/TodoPanel.quick-add.test.ts`

1. 无筛选：created 后新任务在 `displayActiveItems` → 设置高亮、无 info 提示。
2. 关键词隐藏：`itemKeyword` 不匹配新标题 → info 提示、不高亮。
3. 优先级隐藏：`filterPriority = "P0"` 且创建 payload 优先级为手动覆盖的 `P3` → info 提示、不高亮。

**验证**：`pnpm --filter @lazycat/desktop test src/components/TodoPanel.quick-add.test.ts`
**提交**：`feat(todo): 面板接入快速添加栏与创建反馈`

## 阶段 4：全量验证与收尾

1. `pnpm --filter @lazycat/desktop test src/utils/todoQuickAdd.test.ts src/components/TodoQuickAddBar.test.ts src/components/TodoPanel.quick-add.test.ts`
2. `pnpm typecheck`
3. `pnpm --filter @lazycat/desktop build:web`
4. 按项目 07.3 约定在 `process.md` 记录本次实施经验（涉及 6+ 文件）。
5. 如有 process.md 记录：`docs(process): 记录快速添加栏实施经验`。

## 风险与注意

- `TodoPanel.vue` 超 2000 行：改动严格限于挂载点、`quickAddContext`、`onQuickAddCreated`、卡片 class、高亮样式块，不做无关整理。
- 组件测试涉及 `el-dropdown`/`el-date-picker` 弹层时，优先测状态与回调而非弹层 DOM（Teleport 到 body，查询成本高）。
- 高亮定时器在面板卸载时清理，避免悬挂 setTimeout。
- 若 dev 联调需要跑应用，先与用户确认（项目 07.1：不自动启动 dev server）。

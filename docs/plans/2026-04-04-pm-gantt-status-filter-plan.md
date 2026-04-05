# 项目管理甘特图状态筛选实现计划

> 说明：本计划对应“状态筛选仅作用于甘特图”的早期方案，已被
> `docs/plans/2026-04-05-pm-shared-status-filter-plan.md` 替代。
> 当前实现请以共享工具栏方案的新计划为准。

**Goal:** 在 `项目管理 > 甘特图` 中新增状态多选筛选，直接显示 4 个状态按钮以及 `全选 / 清空`，且只影响甘特图，不影响看板。

**Architecture:** `PmPanel.vue` 持有状态筛选和 `baseFilteredItems / ganttFilteredItems` 两层过滤；`PmGanttView.vue` 只负责渲染按钮、统计和事件；状态筛选逻辑优先抽成纯函数，避免把回归风险压到组件内部。

**Tech Stack:** Vue 3、TypeScript、Element Plus、Vitest

---

### Task 0: 代码现实校验

**Files:**
- Read: `apps/desktop/src/types/pm.ts`
- Read: `apps/desktop/src/utils/pmDate.ts`
- Read: `apps/desktop/src/utils/pmGantt.ts`
- Read: `apps/desktop/src/components/PmPanel.vue`
- Read: `apps/desktop/src/components/PmGanttView.vue`

**目标：**

1. 确认 `PmItemStatus` 与 `PM_STATUS_COLUMNS` 的真实枚举值仍是：
   - `todo`
   - `in_progress`
   - `testing`
   - `done`
2. 确认 `hasPmDateSchedule(startAt, endAt)` 仍是“任意一侧有合法日期即算已排期”。
3. 确认 `PmGanttView.vue` 当前工具栏仍与空态绑定在同一个 `v-if` 分支里，后续需要拆开。

**产出：**

若发现以上约束与 spec 不一致，先修正文档或计划，再进入实现。

---

### Task 1: 提取甘特状态筛选纯函数

**Files:**
- Add: `apps/desktop/src/utils/pmGanttFilter.ts`
- Add: `apps/desktop/src/utils/pmGanttFilter.test.ts`

**目标：**

把“状态选择、未知状态兜底、稳定顺序、过滤结果”从组件里抽出来，保证：

1. `selectedStatuses` 对外永远是稳定数组。
2. 不允许重复值。
3. 顺序永远跟随 `PM_STATUS_COLUMNS`。
4. 未知状态在筛选归类时按 `todo` 兜底。
5. 清空后保持空数组，不自动回填全选。

**建议提供的纯函数：**

1. `getPmGanttDefaultStatuses(): PmItemStatus[]`
2. `normalizePmGanttSelectedStatuses(input): PmItemStatus[]`
3. `togglePmGanttStatus(selected, status): PmItemStatus[]`
4. `selectAllPmGanttStatuses(): PmItemStatus[]`
5. `clearPmGanttStatuses(): PmItemStatus[]`
6. `coercePmItemStatusForGanttFilter(status): PmItemStatus`
7. `filterPmItemsByGanttStatuses(items, selectedStatuses): PmItem[]`

**测试覆盖：**

1. 默认状态顺序正确。
2. 重复值会被去重。
3. 未知状态按 `todo` 归类。
4. 仅选 `todo` 时未知状态可见。
5. 取消 `todo` 后未知状态随之不可见。
6. 清空后保持空数组，不自动恢复全选。

---

### Task 2: 改造 `PmPanel.vue` 的过滤链路

**Files:**
- Modify: `apps/desktop/src/components/PmPanel.vue`

**步骤：**

**Step 1: 将现有 `filteredItems` 改名为 `baseFilteredItems`**

保持它只处理：

1. 搜索
2. 类型
3. 优先级

看板相关逻辑全部继续消费这一层。

**Step 2: 新增甘特专用状态筛选状态**

在 `PmPanel.vue` 中新增：

1. `ganttSelectedStatuses`
2. `ganttFilteredItems`
3. 状态切换、全选、清空的事件处理函数

要求：

1. 默认值来自 `getPmGanttDefaultStatuses()`
2. 状态选择为 PM 面板会话级全局状态，不按项目分别记忆
3. 切换看板 / 甘特、切换项目 / 总览时不重置
4. `PmPanel` 实例销毁后重新进入 PM 再恢复默认

**Step 3: 调整甘特数据来源**

把传给 `PmGanttView` 的 `:items` 从当前 `filteredItems` 改成 `ganttFilteredItems`。

看板相关位置继续使用 `baseFilteredItems`，避免状态筛选误伤看板列数据。

**Step 4: 保持详情面板行为不变**

即使当前选中工作项被状态筛掉：

1. 不主动关闭右侧详情
2. 详情仍基于 `items.value` 查找
3. 仅甘特图中不再显示该条目

---

### Task 3: 改造 `PmGanttView.vue` 的工具栏与接口

**Files:**
- Modify: `apps/desktop/src/components/PmGanttView.vue`

**Step 1: 扩展 props / emits**

新增：

1. `selectedStatuses: PmItemStatus[]`
2. `toggle-status`
   - payload: `{ status: PmItemStatus }`
3. `select-all-statuses`
4. `clear-statuses`

`PmGanttView.vue` 内部直接复用 `PM_STATUS_COLUMNS` 生成 4 个状态按钮，不从父层再传一套状态定义。

**Step 2: 工具栏从空态分支中拆出来**

当前结构是：

1. `ganttTasks.length === 0` 时只显示 `el-empty`
2. 有甘特条时才显示工具栏

需要改成：

1. 进入甘特视图后始终显示工具栏
2. 工具栏下面再根据 `ganttTasks` 和 `props.items` 判断显示甘特图或空态

这样 `清空` 后仍保留恢复入口。

**Step 3: 渲染 6 个按钮**

左侧工具栏顺序固定为：

1. `日 / 周 / 月`
2. `待办`
3. `进行中`
4. `测试中`
5. `已完成`
6. `全选`
7. `清空`

要求：

1. 状态按钮有明确的选中态 / 未选中态
2. `全选 / 清空` 允许弱化，但不禁用
3. 窄宽度下允许换行

**Step 4: 统计口径改为基于 `props.items`**

继续沿用：

1. `已排期 X 项`
2. `另有 Y 项未设置日期`

但口径必须变成 `ganttFilteredItems` 对应的 `props.items`：

1. `X = buildPmGanttTasks(props.items).length`
2. `Y = countPmGanttUnscheduledItems(props.items)`

**Step 5: 空态判断调整**

1. `props.items.length === 0`
   - 显示“当前筛选结果没有可显示的工作项”
2. `props.items.length > 0 && ganttTasks.length === 0`
   - 显示“当前筛选结果中有 X 项未设置日期，无法显示甘特图”

注意不要把“筛选后为空”和“未排期为空”混成同一个分支。

---

### Task 4: 完成测试与回归验证

**Files:**
- Modify: `apps/desktop/src/utils/pmGantt.test.ts`
- Add: `apps/desktop/src/utils/pmGanttFilter.test.ts`

**测试重点：**

1. `pmGanttFilter.test.ts`
   - 默认全选
   - 单个切换
   - 全选
   - 清空
   - 去重与稳定顺序
   - 未知状态按 `todo` 归类
2. `pmGantt.test.ts`
   - `countPmGanttUnscheduledItems()` 对单边日期、双边空值、非法值的统计不回归
   - `buildPmGanttTasks()` 仍按现有日期语义输出

**说明：**

仓库当前没有现成的 `PmPanel` / `PmGanttView` 组件测试基础，本轮优先把高风险逻辑压到纯函数测试。

---

### Task 5: 手工验证与命令验证

**命令：**

```powershell
pnpm test src/utils/pmGanttFilter.test.ts src/utils/pmGantt.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

**手工验证清单：**

1. 进入任意项目甘特图，默认 4 个状态全选。
2. 点击单个状态按钮，甘特条按状态收缩。
3. 点击 `清空` 后，工具栏仍在，甘特显示空结果态。
4. `清空` 后切到看板再切回甘特，仍保持清空。
5. 切换项目 / 总览后，状态选择仍保留。
6. 搜索、类型、优先级仍同时影响看板和甘特。
7. 看板列数量不受甘特状态筛选影响。
8. 当结果里只有未排期工作项时，显示“未设置日期”空态而不是“无结果”空态。

---

### Task 6: 提交

```powershell
git add apps/desktop/src/components/PmPanel.vue apps/desktop/src/components/PmGanttView.vue apps/desktop/src/utils/pmGantt.ts apps/desktop/src/utils/pmGanttFilter.ts apps/desktop/src/utils/pmGantt.test.ts apps/desktop/src/utils/pmGanttFilter.test.ts
git commit -m "feat(pm): 为甘特图添加状态多选筛选"
```

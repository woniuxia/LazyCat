# 项目管理共享状态筛选实现计划

日期：2026-04-05

## 目标

把项目管理中的状态筛选从“甘特图内部专用能力”提升为“PM 顶部工具栏共享筛选能力”，并满足以下已确认约束：

1. 看板和甘特图共用同一套状态筛选。
2. 入口放在顶部工具栏第二行，位于 `思源设置` 前。
3. 入口形态为 `el-select multiple`。
4. 闭合态固定显示 `状态筛选`，不回显已选状态文案，不提供额外 `clearable` 图标。
5. 看板未选中的状态列直接隐藏。
6. 清空后允许两种视图都显示空结果，不自动回填全选。

## 当前代码现实

已存在的实现基础如下：

1. `apps/desktop/src/utils/pmGanttFilter.ts`
   - 已经承载默认状态、切换、全选、清空、未知状态按 `todo` 兜底等逻辑。
2. `apps/desktop/src/components/PmPanel.vue`
   - 已有 `baseFilteredItems`
   - 已有 `ganttSelectedStatuses`
   - 已有 `ganttFilteredItems`
3. `apps/desktop/src/components/PmGanttView.vue`
   - 仍在内部渲染状态 chip 和 `全选 / 清空`
   - 仍通过 `selectedStatuses` / `toggle-status` / `select-all-statuses` / `clear-statuses` 与父层交互
4. `apps/desktop/src/utils/pmGanttFilter.test.ts`
   - 已覆盖既有甘特状态筛选规则

因此本轮不是从零实现，而是把现有“甘特专用方案”迁移为“PM 共享方案”，同时清理旧接口与文档歧义。

## 总体实现顺序

1. 先迁移共享状态 helper，稳定纯函数边界。
2. 再改 `PmPanel.vue` 的工具栏和共享筛选链路。
3. 再收口 `PmGanttView.vue` 接口，去掉内部状态筛选 UI。
4. 最后补测试和验证。

这样可以把风险聚焦在“状态源从哪里来”和“哪个组件负责展示”两个最关键边界上。

---

## Task 1：迁移甘特专用 helper 为共享状态 helper

**Files**

- Add: `apps/desktop/src/utils/pmStatusFilter.ts`
- Add: `apps/desktop/src/utils/pmStatusFilter.test.ts`
- Delete: `apps/desktop/src/utils/pmGanttFilter.ts`
- Delete: `apps/desktop/src/utils/pmGanttFilter.test.ts`

**目标**

把“状态选择、稳定顺序、未知状态兜底、按状态过滤、看板列归类”从甘特命名迁移为 PM 共享命名。

**建议提供的函数**

1. `getPmDefaultSelectedStatuses()`
2. `normalizePmSelectedStatuses(input)`
3. `togglePmSelectedStatus(selected, status)`
4. `selectAllPmStatuses()`
5. `clearPmStatuses()`
6. `coercePmItemStatusForFilter(status)`
7. `filterPmItemsBySelectedStatuses(items, selectedStatuses)`
8. `getVisiblePmStatusColumns(selectedStatuses)`
9. `groupPmItemsByStatus(items)`

**关键约束**

1. 默认仍按当前产品口径返回 `todo / in_progress / testing`，不默认包含 `done`。
2. 对外输出始终是稳定顺序数组，顺序跟随 `PM_STATUS_COLUMNS`。
3. 未知状态在筛选命中判断和看板列分组时都按 `todo` 处理。
4. 只处理筛选参与性与归类，不修改工作项原始 `status`。

**说明**

旧 `pmGanttFilter.ts` 的逻辑不是重写，而是迁移并扩充；目的是让“共享状态筛选”不再挂在甘特语义下。

---

## Task 2：改造 `PmPanel.vue` 顶部工具栏与共享状态源

**Files**

- Modify: `apps/desktop/src/components/PmPanel.vue`

**步骤**

### Step 2.1：把状态筛选状态源提升为共享命名

1. 把 `ganttSelectedStatuses` 改成 `selectedStatuses`
2. 默认值改为来自 `getPmDefaultSelectedStatuses()`
3. 保持该状态为 `PmPanel` 实例级共享状态

### Step 2.2：把第二行工具栏调整为共享筛选区

顺序固定为：

1. 搜索
2. 类型
3. 优先级
4. `状态筛选`
5. 思源设置

### Step 2.3：实现 `el-select multiple` 的固定触发器文案

要求：

1. 触发器闭合态固定显示 `状态筛选`
2. 不显示选中标签
3. 不显示“2/4”之类汇总文本
4. 不额外开启 `clearable`
5. 通过面板内逐项取消实现清空

### Step 2.4：调整筛选分层

保留并明确以下三层：

1. `baseFilteredItems`
   - 搜索 / 类型 / 优先级
2. `statusFilteredItems`
   - 在 `baseFilteredItems` 上叠加共享状态筛选
3. 视图派生层
   - 看板列数据、甘特任务数据都从 `statusFilteredItems` 派生

### Step 2.5：保持详情面板查找链路不变

即使工作项因状态变更或筛选变化从当前视图消失：

1. `selectedItem` 仍从完整 `items` 列表里查找
2. 详情面板不主动关闭

---

## Task 3：改造看板视图为“共享筛选 + 隐藏列”

**Files**

- Modify: `apps/desktop/src/components/PmPanel.vue`

**目标**

让看板直接消费共享状态筛选后的结果，而不是继续使用只含搜索/类型/优先级的旧结果。

**具体变更**

1. 新增 `visibleStatusColumns`
   - 来源复用 `PM_STATUS_COLUMNS`
   - 只保留当前选中的状态列
2. 将看板 `v-for` 数据源从 `PM_STATUS_COLUMNS` 改为 `visibleStatusColumns`
3. 将 `columnItemsMap` 的数据源改为 `statusFilteredItems`
4. 未知状态工作项在看板中归入 `todo` 列

**空态规则**

1. 当 `selectedStatuses` 为空时，不渲染任何列骨架，显示统一空态
2. 当 `statusFilteredItems` 为空时，也显示统一空态
3. 不保留空壳列，不展示“空列墙”

**拖拽相关检查点**

1. 列被隐藏后，不应该初始化对应列的 Sortable 实例
2. 只在可见列上保留拖拽目标
3. 用户把工作项推进到未选中状态后，该卡片应立即从当前看板结果集中消失

---

## Task 4：收口 `PmGanttView.vue` 接口并移除内部状态筛选 UI

**Files**

- Modify: `apps/desktop/src/components/PmGanttView.vue`
- Modify: `apps/desktop/src/components/PmPanel.vue`

**目标**

让甘特图只负责甘特图本身，不再持有状态筛选的展示与交互。

**接口收口**

保留：

1. `items`
2. `selectedItemId`
3. `showProjectMeta`
4. `view-change`
5. 既有 `select / edit / item-context / date-change / viewport-scroll`

删除：

1. `selectedStatuses`
2. `toggle-status`
3. `select-all-statuses`
4. `clear-statuses`

**UI 收口**

1. 删除甘特工具栏中的状态 chip 与 `全选 / 清空`
2. 保留 `日 / 周 / 月`
3. 保留 `已排期 X 项`
4. 保留 `另有 Y 项未设置日期`

**空态判定**

继续复用现有甘特排期语义：

1. `startAt` 或 `endAt` 任意一侧可归一为合法日期，即视为可排期
2. 只有单边日期时，仍显示甘特条
3. 双边都空或非法时，视为未排期

显示规则：

1. `props.items.length === 0`
   - 显示“当前筛选结果没有可显示的工作项”
2. `props.items.length > 0 && ganttTasks.length === 0 && unscheduledCount > 0`
   - 显示“当前筛选结果中有 X 项未设置日期，无法显示甘特图”
3. 只要 `ganttTasks.length > 0`
   - 必须正常渲染甘特图，不进入空态

---

## Task 5：补齐测试

**Files**

- Add: `apps/desktop/src/utils/pmStatusFilter.test.ts`
- Modify: `apps/desktop/src/utils/pmGantt.test.ts`

**测试重点**

### 5.1 `pmStatusFilter.test.ts`

1. 默认选中状态顺序正确
2. 重复值会被去重
3. 状态切换保持稳定顺序
4. 全选与清空行为正确
5. 未知状态按 `todo` 命中
6. 未知状态工作项进入 `todo` 列
7. 空选择时看板可见列为空

### 5.2 `pmGantt.test.ts`

1. 单边日期仍算可排期
2. 双边非法值算未排期
3. `countPmGanttUnscheduledItems()` 不回归
4. “只要存在任一可排期任务就不进入未排期空态”的统计前提保持成立

**补充说明**

当前仓库没有成熟的 `PmPanel` / `PmGanttView` 组件测试基础，因此本轮仍以纯函数和现有甘特工具函数测试为主；UI 行为通过手工验证兜底。

---

## Task 6：回归验证

**命令**

```powershell
pnpm test src/utils/pmStatusFilter.test.ts src/utils/pmGantt.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

**手工验证清单**

1. 进入 PM 面板后，第二行控件顺序为：搜索 / 类型 / 优先级 / 状态筛选 / 思源设置。
2. `状态筛选` 闭合态固定显示该文案，不回显选中状态。
3. 看板和甘特图切换后，共用同一套状态筛选。
4. 取消某个状态后，看板对应列直接隐藏。
5. 清空全部状态后，看板和甘特图都显示空结果态。
6. 清空后切换项目 / 总览，再切回，状态筛选仍保持当前值。
7. 通过右键菜单或快捷推进把工作项改到未选中状态后，该项立即从当前视图消失。
8. 上述工作项若原本处于详情打开状态，详情面板保持打开。
9. 甘特图内部不再显示状态 chip 与 `全选 / 清空`。
10. 当结果中只有未排期工作项时，甘特图显示“未设置日期”空态，而不是“无结果”空态。

---

## Task 7：文档与提交

**Files**

- Modify: `process.md`

**要求**

1. 实现完成后，把这次“甘特专用状态筛选迁移为共享工具栏筛选”的经验补到 `process.md`
2. 重点记录：
   - 现有 `pmGanttFilter` 如何安全迁移为共享 helper
   - 看板隐藏列对 Sortable 初始化的影响
   - 甘特空态与排期统计的判定边界

**提交建议**

```powershell
git add docs/plans/2026-04-05-pm-shared-status-filter-plan.md
git commit -m "docs(pm): 添加共享状态筛选实现计划"
```

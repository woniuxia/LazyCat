# PM 视图扩展实施计划

> 依据设计文档：`docs/superpowers/specs/2026-04-19-pm-view-extensions-design.md`
> 目标：基础设施升级 + 4 个新视图（今日/列表/日历/四象限）+ 侧栏今日入口

---

## 总览

| Phase | 目标 | 预估 | 关键依赖 |
|-------|------|------|---------|
| Phase 1 | 基础设施（视图注册、切换器、看板抽出、记忆） | 1-2 天 | 无 |
| Phase 2 | 今日视图 + 列表视图 | 3-4 天 | Phase 1 |
| Phase 3 | 日历视图 + 四象限视图 | 3-4 天 | Phase 1 |
| Phase 4 | 响应式 / 性能 / 文档 | 1-2 天 | Phase 2+3 |

**Phase 2 和 Phase 3 中的前端视图互不依赖，可并行实现。四个视图共享切换器基础设施。**

---

## Phase 0：准备（开工前）

### 0.1 类型定义与基线

**目标**：确定共用类型，避免后续各视图自己造轮子。

### 任务

1. **确认 `PmItem` 类型完整字段**
   - 文件：`apps/desktop/src/composables/usePmItems.ts` 或 `types/` 下
   - 核对字段：`id, project_id, title, description, link_url, item_type, priority, status, start_at, end_at, started_at, testing_at, completed_at, updated_at, pinned, tags`
   - 如缺少字段类型，补全导出

2. **确认当前 `selectedProjectId` 类型**
   - `PmPanel.vue:428` `ref<number | "overview" | null>(null)`
   - 后续本设计不改变此类型（「今日」不做上下文，沿用 overview/number/null）

### 验证

- `pnpm typecheck` 通过

---

## Phase 1：基础设施

### 1.1 视图注册表

**新增文件**：`apps/desktop/src/composables/pmViewRegistry.ts`

```typescript
import type { AsyncComponent } from 'vue';
import { defineAsyncComponent } from 'vue';

export type ViewId = 'kanban' | 'gantt' | 'today' | 'list' | 'calendar' | 'matrix';

export interface ViewDefinition {
  id: ViewId;
  label: string;
  icon: string;
  component: AsyncComponent;
}

export const PM_VIEWS: ViewDefinition[] = [
  { id: 'kanban',   label: '看板',   icon: '▦', component: defineAsyncComponent(() => import('@/components/PmKanbanView.vue')) },
  { id: 'gantt',    label: '甘特',   icon: '▤', component: defineAsyncComponent(() => import('@/components/PmGanttViewAdapter.vue')) },
  { id: 'list',     label: '列表',   icon: '≡', component: defineAsyncComponent(() => import('@/components/PmListView.vue')) },
  { id: 'calendar', label: '日历',   icon: '▥', component: defineAsyncComponent(() => import('@/components/PmCalendarView.vue')) },
  { id: 'matrix',   label: '四象限', icon: '⊞', component: defineAsyncComponent(() => import('@/components/PmMatrixView.vue')) },
  { id: 'today',    label: '今日',   icon: '◷', component: defineAsyncComponent(() => import('@/components/PmTodayView.vue')) },
];

export function getViewById(id: ViewId): ViewDefinition {
  return PM_VIEWS.find(v => v.id === id) ?? PM_VIEWS[0];
}
```

**注**：Phase 1 仅 kanban/gantt 的组件先存在；其他 4 个视图 Phase 2/3 逐步实现。异步组件在未实现时导入会失败 — Phase 1 阶段只把 kanban/gantt 注册进去，其他留空数组，后续 Phase 追加注册即可。

#### 1.1 阶段性注册（Phase 1 版）

```typescript
export const PM_VIEWS: ViewDefinition[] = [
  { id: 'kanban', ... },
  { id: 'gantt',  ... },
];
```

### 1.2 视图选择记忆

**新增文件**：`apps/desktop/src/composables/usePmViewMemory.ts`

```typescript
import { ref, watch } from 'vue';
import { invokeToolByChannel } from '@/bridge/tauri';
import type { ViewId } from './pmViewRegistry';

type ContextId = number | 'overview';

function settingsKey(ctx: ContextId): string {
  return `pm:view:${ctx === 'overview' ? 'overview' : `project-${ctx}`}`;
}

function defaultView(ctx: ContextId): ViewId {
  return ctx === 'overview' ? 'list' : 'kanban';
}

export function usePmViewMemory(contextRef: Ref<ContextId | null>) {
  const currentView = ref<ViewId>('kanban');

  watch(contextRef, async (ctx) => {
    if (ctx === null) return;
    const saved = await readSetting<ViewId>(settingsKey(ctx));
    currentView.value = saved ?? defaultView(ctx);
  }, { immediate: true });

  async function setView(viewId: ViewId) {
    const ctx = contextRef.value;
    if (ctx === null) return;
    currentView.value = viewId;
    await writeSetting(settingsKey(ctx), viewId);
  }

  return { currentView, setView };
}
```

**依赖**：如项目已有 `useSettings` composable，复用之；否则增加 `readSetting/writeSetting` 工具函数（走 `user_settings` 表 channel）。

### 1.3 PmViewSwitcher.vue 切换器组件

**新增文件**：`apps/desktop/src/components/PmViewSwitcher.vue`

**Props/Emits**：

```typescript
defineProps<{ modelValue: ViewId }>();
defineEmits<{ 'update:modelValue': [ViewId] }>();
```

**模板结构**：

```html
<div class="pm-view-switcher">
  <button
    v-for="view in PM_VIEWS"
    :key="view.id"
    class="switcher-item"
    :class="{ on: modelValue === view.id }"
    :title="isCompact ? view.label : undefined"
    @click="$emit('update:modelValue', view.id)"
  >
    <span class="i">{{ view.icon }}</span>
    <span v-if="!isCompact" class="label">{{ view.label }}</span>
  </button>
</div>
```

**响应式降级**：

```typescript
const isCompact = ref(false);
useResizeObserver(containerRef, (entries) => {
  isCompact.value = entries[0].contentRect.width < 440;
});
```

**样式**：参考原型页 `switcher.html` 形态 A（Tab 标签），复用 CSS 变量 `--pm-accent`、`--pm-edge-soft` 等。样式放在 `<style scoped>`，激活态遵守 `theme-light.css` 覆盖规则（CLAUDE.md 05.1）。

### 1.4 抽出 PmKanbanView.vue

**新增文件**：`apps/desktop/src/components/PmKanbanView.vue`

**模板来源**：`PmPanel.vue:157-261` 的 `div.kanban-board` 和相关空态

**Props/Emits**：

```typescript
defineProps<{
  items: PmItem[];
  projects: PmProject[];
  selectedProjectId: number | 'overview' | null;
  // 现有看板需要的 props 全部列出，参考当前 PmPanel 中看板区域使用的变量
}>();
defineEmits<{
  'item-click': [PmItem];
  'item-status-change': [PmItem, string];
  'create-item-in-column': [columnKey: string];
  // ... 等
}>();
```

**迁入内容**：

- 模板 `div.kanban-board` + `div.kanban-column × N` + `div.kanban-card × N`
- 计算属性 `statusFilteredItems`, `columnItemsMap`, `visibleStatusColumns`（PmPanel.vue:647-655 附近）
- 拖拽处理函数 `onColumnDragOver`, `onColumnDrop`, `onCardDragStart` 等
- 相关样式块（搜索 `.kanban-column`, `.kanban-card`, `.kanban-board`）

**PmPanel.vue 改动**：
- 模板对应区域替换为 `<PmKanbanView v-if="viewId === 'kanban'" ... />`
- 相关状态通过 props 传递

### 1.5 甘特图适配器

**新增文件**：`apps/desktop/src/components/PmGanttViewAdapter.vue`

现有 `PmGanttView.vue` 已是独立组件，但 props 接口可能和新切换器期望不一致。创建极薄 wrapper，统一视图 props 协议：

```html
<template>
  <PmGanttView v-bind="$attrs" />
</template>
```

**或者**：如果现有 `PmGanttView.vue` props 已经完备，直接注册它，无需 wrapper。**开工前先 Read 它的 props 签名**决定。

### 1.6 PmPanel.vue 迁移 viewMode → viewId

**关键修改点**：

| 位置 | 改动 |
|------|------|
| `PmPanel.vue:433` | `const viewMode = ref<"kanban" \| "gantt">("kanban")` → 删除，改用 `usePmViewMemory` |
| `PmPanel.vue:95` 附近 | `<el-switch v-model="viewMode" ...>` → `<PmViewSwitcher v-model="viewId" />` |
| `PmPanel.vue:157, 257, 262` | `viewMode === 'kanban'/'gantt'` → `viewId === 'kanban'/'gantt'` |
| `PmPanel.vue:1077, 1152, 1160` | watch 依赖 `viewMode.value` → `viewId.value` |
| 主内容区 | `v-if/v-else-if` 改为 `<component :is="currentView.component" v-bind="viewProps" @... />` |

**向后兼容**：usePmViewMemory 首次读取时如果 key 不存在，fallback 查旧 key `pm:viewMode`（如曾用过），再 fallback 到默认。

### 1.7 侧栏「今日」入口骨架

**位置**：`PmPanel.vue:17` 的 `.sidebar-overview-card` 之前，增加「今日」入口。

**模板新增**：

```html
<div
  class="sidebar-today-card"
  :class="{ 'is-active': viewId === 'today' }"
  @click="setView('today')"
>
  <div class="sidebar-today-head">
    <span class="icon">◷</span>
    <span class="today-name">今日</span>
    <span class="today-badge">{{ todayBadgeCount }}</span>
  </div>
</div>
```

**Phase 1 阶段**：`todayBadgeCount` 先用 `0` 占位，Phase 2 完成后端后联动。

**视觉**：参考原型 `today.html` 侧栏 mock，样式写在 PmPanel.vue 的 `<style>` 中。

### 验证 Phase 1

- [ ] `pnpm typecheck` 通过
- [ ] `pnpm --filter @lazycat/desktop build:web` 通过
- [ ] 切换器切「看板」↔「甘特图」功能等价旧版
- [ ] 新建/编辑/删除/拖拽/置顶/状态流转均正常
- [ ] 切换项目后视图选择能记住（选中 LazyCat 用甘特、选中个人成长用看板，切来切去保持）
- [ ] 侧栏「今日」入口出现，点击能切到 viewId='today'（此时主区为空或渲染未实现占位）
- [ ] 甘特图、看板的响应式降级在窄窗口下不破损

---

## Phase 2：今日视图 + 列表视图

### 2.1 后端：今日聚合接口

**文件**：`apps/desktop/src-tauri/src/tools/pm.rs`（或按现有分模块规则，拆到 `pm/today.rs`）

**新增 action**：

```rust
"item_today_list" => today::item_today_list(payload),
"item_today_counts" => today::item_today_counts(payload),
```

**接口签名**：

```rust
fn item_today_list(payload: &Value) -> Result<Value, String>;
// 参数：{ project_id?: i64, today_date: String ("YYYY-MM-DD") }
// 返回：{ overdue: [PmItem], due_today: [PmItem], in_progress: [PmItem], completed_today: [PmItem] }

fn item_today_counts(payload: &Value) -> Result<Value, String>;
// 参数：{ project_id?: i64 }
// 返回：{ overdue: u32, due_today: u32, in_progress: u32, completed_today: u32, total_active: u32 }
```

**SQL 要点**：

- 逾期：`end_at IS NOT NULL AND end_at < today AND status != 'completed'`
- 今日到期：`DATE(end_at) = today AND status != 'completed' AND id NOT IN (overdue ids)`
- 进行中：`started_at IS NOT NULL AND status != 'completed' AND id NOT IN (overdue/today ids)`
- 今日完成：`DATE(completed_at) = today`

**去重**：同一任务只出现在最先匹配的分区（优先级：逾期 > 今日 > 进行中 > 完成）。

**索引**：`CREATE INDEX IF NOT EXISTS idx_pm_items_end_at ON pm_items(end_at);` 等（见 2.2）。

### 2.2 后端：索引优化

**文件**：pm.rs 的初始化 schema 块或 migration。

```sql
CREATE INDEX IF NOT EXISTS idx_pm_items_end_at ON pm_items(end_at);
CREATE INDEX IF NOT EXISTS idx_pm_items_status ON pm_items(status);
CREATE INDEX IF NOT EXISTS idx_pm_items_updated_at ON pm_items(updated_at);
CREATE INDEX IF NOT EXISTS idx_pm_items_completed_at ON pm_items(completed_at);
CREATE INDEX IF NOT EXISTS idx_pm_items_project_id ON pm_items(project_id);
```

### 2.3 Channel 注册

**文件**：`apps/desktop/src/bridge/tauri.ts`

```typescript
CHANNEL_MAP: {
  'tool:pm:item_today_list': { domain: 'pm', action: 'item_today_list' },
  'tool:pm:item_today_counts': { domain: 'pm', action: 'item_today_counts' },
  // ... 列表的 item_batch_update 也在此加
}
```

### 2.4 PmTodayView.vue

**新增文件**：`apps/desktop/src/components/PmTodayView.vue`

**Props**：

```typescript
defineProps<{
  selectedProjectId: number | 'overview' | null;
}>();
```

**核心逻辑**：

```typescript
const data = ref<TodayListResponse | null>(null);

async function load() {
  const projectId = props.selectedProjectId === 'overview' ? null : props.selectedProjectId;
  data.value = await invokeToolByChannel('tool:pm:item_today_list', {
    project_id: projectId,
    today_date: formatLocalDate(new Date()),
  });
}

watchEffect(load);
```

**模板结构**（参考原型页 `today.html`）：

- 顶部统计条 4 卡片
- 4 个分区（逾期/今日到期/进行中/今日已完成折叠）
- 任务卡片：复用与列表视图相同的卡片样式（建议抽到 `PmTaskMiniCard.vue` 共享，但可以 Phase 2 末尾再抽）

**快捷操作**：
- 「开始做」：调 `item_change_status`，status → 'doing' or 'testing'
- 「推到明天」：调 `item_update`，end_at += 1 day
- 「标记完成」：调 `item_change_status`，status → 'completed'

点击卡片 → 触发 `PmDetailPanel`（通过 emit 或 provide/inject 拿到全局 detail 控制）。

### 2.5 侧栏今日 badge 联动

**文件**：PmPanel.vue

在 `watch` selectedProjectId 时同步拉取 `item_today_counts`：

```typescript
const todayBadgeCount = ref(0);

watchEffect(async () => {
  const projectId = selectedProjectId.value === 'overview' ? null : selectedProjectId.value;
  if (projectId === null && selectedProjectId.value !== 'overview') return;
  const counts = await invokeToolByChannel('tool:pm:item_today_counts', { project_id: projectId });
  todayBadgeCount.value = counts.overdue + counts.due_today + counts.in_progress;
});
```

任务状态变化后要刷新这个值（监听 item-updated 事件或在变更函数末尾调用）。

### 2.6 后端：批量更新接口

**action**：`item_batch_update`

```rust
fn item_batch_update(payload: &Value) -> Result<Value, String>;
// 参数：{ ids: Vec<i64>, fields: { status?, priority?, project_id?, tags?, pinned? } }
// 返回：{ updated: u32 }
```

事务中批量更新，返回受影响行数。

### 2.7 PmListView.vue

**新增文件**：`apps/desktop/src/components/PmListView.vue`

**规模**：预计 700-900 行，包含表格、行内编辑、批量操作、分组、筛选、排序。建议进一步拆分为：

- `PmListView.vue`（容器 + 数据与状态）
- `PmListTable.vue`（表格本体）
- `PmListBatchBar.vue`（底部批量操作浮条）
- `PmListFilterBar.vue`（顶部筛选条）

**Props**：

```typescript
defineProps<{
  items: PmItem[];
  projects: PmProject[];
  tags: PmTag[];
  selectedProjectId: number | 'overview' | null;
}>();
```

**核心状态**：

```typescript
const selectedIds = ref<Set<number>>(new Set());
const groupBy = ref<'none' | 'project' | 'status' | 'priority' | 'tag'>('none');
const sortBy = ref<{ col: string; dir: 'asc' | 'desc' | null }>({ col: 'default', dir: null });
const filters = ref<Record<string, unknown>>({});
const visibleCols = ref<ColId[]>(['title', 'project', 'status', 'priority', 'end_at', 'tags', 'updated_at']);
const expandedRows = ref<Set<number>>(new Set());
```

**记忆**：`groupBy / visibleCols` 持久化到 `pm:view:list:*:<contextId>`。

**行内编辑**：
- 单元格点击 → 切换为编辑态（el-select / el-date-picker / el-input / el-cascader）
- Enter 提交，Esc 取消
- 提交 → 调 `item_update`，乐观更新

**批量操作**：
- 选中 ≥ 1 行 → 底部浮条出现
- 「改状态」等按钮 → 弹出下拉，调 `item_batch_update`

**分组**：当 `groupBy !== 'none'` 时，按指定字段 group by，每组渲染一个可折叠块。

**行展开**：

- 行首 ▶ 图标点击 → 展开下方快速预览行（描述 + 元数据 + 居中排布的「打开完整详情面板」「标记完成」「复制链接」按钮）
- 「打开完整详情面板」→ 触发 `PmDetailPanel`（同今日视图）

### 2.8 统一 detail 触发机制

**新增文件**（可选，可选择内联在 PmPanel）：`apps/desktop/src/composables/pmDetailKey.ts`

```typescript
import type { InjectionKey, Ref } from 'vue';
import type { PmItem } from '@/types';

export interface PmDetailController {
  show: (item: PmItem) => void;
  hide: () => void;
  current: Ref<PmItem | null>;
}

export const PM_DETAIL_KEY: InjectionKey<PmDetailController> = Symbol('pm-detail');
```

PmPanel.vue 提供：

```typescript
provide(PM_DETAIL_KEY, {
  show: (item) => { selectedItemId.value = item.id; },
  hide: () => { selectedItemId.value = null; },
  current: computed(() => selectedItem.value),
});
```

各视图通过 `inject(PM_DETAIL_KEY)` 触发详情面板。复用现有 `PmDetailPanel.vue`，避免视图多次重建面板。

### 验证 Phase 2

- [ ] 今日视图 4 分区数据正确，跨项目/单项目下切换展示对应数据
- [ ] 侧栏「今日」badge 数字实时更新（创建、完成、改日期后）
- [ ] 列表视图默认 7 列渲染；单项目时「项目」列隐藏
- [ ] 行内编辑状态/优先级/截止/标签能持久化
- [ ] 多选 → 批量改状态 → 数据库全部更新
- [ ] 分组按项目 / 状态 / 优先级正常切换
- [ ] 筛选、排序组合使用无冲突
- [ ] 行展开点「打开完整详情面板」能调起 `PmDetailPanel`
- [ ] `pnpm typecheck` + `pnpm --filter @lazycat/desktop build:web` + `pnpm test` 全部通过

---

## Phase 3：日历视图 + 四象限视图

### 3.1 后端：日历范围查询

**action**：`item_calendar_range`

```rust
fn item_calendar_range(payload: &Value) -> Result<Value, String>;
// 参数：{ project_id?: i64, start_date: String, end_date: String }
// 返回：{ items: [PmItem] }（含 id, title, project_id, status, priority, start_at, end_at, completed_at 等）
```

SQL：
```sql
SELECT * FROM pm_items
WHERE (project_id = ?1 OR ?1 IS NULL)
  AND ((end_at >= ?2 AND end_at <= ?3)
       OR (start_at >= ?2 AND start_at <= ?3)
       OR (start_at < ?2 AND end_at > ?3))  -- 跨区间的长任务
```

### 3.2 PmCalendarView.vue

**新增文件**：`apps/desktop/src/components/PmCalendarView.vue`

**子视图**：`subview: 'month' | 'week'`，持久化 `pm:view:calendar:subview:<contextId>`。

**结构**（按原型 `calendar.html`）：

- 顶部工具栏：上一月/下一月/今天、月/周子视图切换、色标切换
- 月视图：5-6 行 × 7 列（周日起始），格子内容 = 日期 + 任务条
- 周视图：7 列任务列表，无任务列加 `&nbsp;` 占位

**关键实现**：

```typescript
function buildCalendarGrid(year: number, month: number): CalendarCell[] {
  // 周日起始
  const firstDay = new Date(year, month - 1, 1);
  const startOffset = firstDay.getDay(); // 0 = 周日
  const start = new Date(firstDay);
  start.setDate(1 - startOffset);
  // 生成 5-6 周 cells
}

function classifyTaskColor(item: PmItem, colorBy: 'project' | 'priority' | 'status'): string {
  // 返回 CSS class
}

function computeOverflow(tasks: PmItem[]): { visible: PmItem[]; overflow: string } {
  if (tasks.length <= 4) return { visible: tasks, overflow: '' };
  const visible = tasks.slice(0, 3);
  const hiddenProjects = [...new Set(tasks.slice(3).map(t => projectName(t.project_id)))];
  const label = hiddenProjects.slice(0, 2).join('、');
  return { visible, overflow: `${label} 等项目还有 ${tasks.length - 3} 条` };
}
```

**交互**：

- 点击日期格空白 → `PmItemDialog.showCreate({ end_at: cellDate })`
- 点击任务条 → `ElPopover` 快速预览 → 「详情」按钮 → 触发 `PmDetailPanel`
- 拖拽任务条（月视图内跨格子） → 确认 dialog → 调 `item_update` 改 `end_at`

### 3.3 后端：四象限分桶

**action**：`item_matrix_bucket`

```rust
fn item_matrix_bucket(payload: &Value) -> Result<Value, String>;
// 参数：{ project_id?: i64, urgent_threshold_days: u32, hide_completed: bool, today_date: String }
// 返回：{ q1: [PmItem], q2: [PmItem], q3: [PmItem], q4: [PmItem] }
```

分类：
```rust
fn classify(item: &Item, urgent_threshold: u32, today: NaiveDate) -> Quadrant {
  let important = matches!(item.priority.as_str(), "P0" | "P1");
  let urgent = match &item.end_at {
    None => false,
    Some(dt) => {
      let end = parse_date(dt);
      let days_left = (end - today).num_days();
      days_left < 0 || days_left <= urgent_threshold as i64
    }
  };
  match (important, urgent) {
    (true, true) => Quadrant::Q1,
    (true, false) => Quadrant::Q2,
    (false, true) => Quadrant::Q3,
    (false, false) => Quadrant::Q4,
  }
}
```

**可选替代**：分桶完全可在前端做（后端只返回 items，前端按规则分）。选择后端做的好处：规则集中在一处，前端各视图不用重复判断。

### 3.4 PmMatrixView.vue

**新增文件**：`apps/desktop/src/components/PmMatrixView.vue`

**结构**（按原型 `matrix.html`）：

- 顶部工具栏：紧急阈值切换（3/7/14）、隐藏已完成开关
- 外围坐标轴：纵轴用 `writing-mode: vertical-rl + text-orientation: upright`；横轴水平
- 2×2 象限网格
- 象限内任务卡片紧凑版

**不支持拖拽**：仅 `cursor: pointer`，点击卡片 → 快速预览 Popover → 「详情」。

**顶栏设置持久化**：
- `pm:view:matrix:urgentThreshold` (数字)
- `pm:view:matrix:hideCompleted` (bool)

### 3.5 在 pmViewRegistry 注册所有新视图

Phase 3 结束时 `PM_VIEWS` 全部 6 项完备。

### 验证 Phase 3

- [ ] 月视图当月日期分布正确，周日起始；当日高亮正确（测试月初/月末边界）
- [ ] 溢出处理：4 条全显；5 条显示前 3 + 提示文案含项目名
- [ ] 点击日期格空白能调起新建 dialog，end_at 预填
- [ ] 拖拽任务条跨格子 → 确认 dialog → 数据更新
- [ ] 周视图 7 列等宽，无任务列高度和有任务列一致
- [ ] 四象限分布符合预期（P0 逾期在 Q1；P3 无截止在 Q4 等）
- [ ] 紧急阈值 3 → 7 天切换实时重渲染
- [ ] 隐藏已完成开关生效
- [ ] 纵坐标 ▲重要/不重要▼ 竖排正确，各自半区居中
- [ ] `pnpm typecheck` + `pnpm --filter @lazycat/desktop build:web` 通过

---

## Phase 4：打磨

### 4.1 切换器响应式降级

- `PmViewSwitcher.vue` 在窗口宽度 < 1100px 时降为纯图标
- Tooltip 显示 label
- 使用 `ResizeObserver`，不依赖媒体查询（容器可能嵌入不同父布局）

### 4.2 列表视图虚拟滚动

- 数据量 > 500 行时启用
- 候选库：`vue-virtual-scroller` 或自实现
- 分组模式下虚拟滚动的高度计算复杂，首版可只支持无分组时虚拟滚动

### 4.3 性能索引验证

- 创建 1000 条 mock 任务数据
- 测试 `item_today_list` / `item_calendar_range` / `item_matrix_bucket` 响应时间
- 目标 < 50ms（1000 条数据量下）

### 4.4 文档更新

- `CLAUDE.md` / `AGENTS.md` 04 章节「常规工具调用链路」补充新增的 6 个 action
- `04.3 前端组织` 增加视图注册表机制说明
- `process.md` 记录本次"视图扩展"经验（如果有沉淀价值）

### 验证 Phase 4

- [ ] 窄窗口（<1100px）切换器显示纯图标 + Tooltip
- [ ] 1000 条任务时列表视图虚拟滚动流畅
- [ ] 查询响应时间符合目标
- [ ] 文档同步更新

---

## 任务依赖图

```
Phase 1 (基础设施)
  ├─ 1.1 视图注册表     ──┐
  ├─ 1.2 视图记忆       ──┤
  ├─ 1.3 切换器组件     ──┼── 1.6 PmPanel 迁移
  ├─ 1.4 看板抽出       ──┤
  ├─ 1.5 甘特适配       ──┘
  └─ 1.7 侧栏今日入口

Phase 2 (今日 + 列表)
  ├─ 2.1-2.3 后端今日 + 索引 + channel
  │    └─ 2.4 PmTodayView
  │         └─ 2.5 侧栏 badge 联动
  ├─ 2.6 后端批量更新
  │    └─ 2.7 PmListView
  └─ 2.8 统一 detail 触发（2.4 和 2.7 都依赖）

Phase 3 (日历 + 四象限)  [可和 Phase 2 部分并行]
  ├─ 3.1 后端日历范围
  │    └─ 3.2 PmCalendarView
  ├─ 3.3 后端四象限分桶
  │    └─ 3.4 PmMatrixView
  └─ 3.5 注册表补齐

Phase 4 (打磨)
  ├─ 4.1 响应式
  ├─ 4.2 虚拟滚动
  ├─ 4.3 性能验证
  └─ 4.4 文档
```

---

## 风险与缓解（实施层面）

| 风险 | 缓解 |
|------|------|
| 看板从 PmPanel 抽出后 props 爆炸 | 先列出所有用到的变量（~20 个），能通过 inject 拿到的（如 pmTodoLinking）不走 props |
| 视图组件里各自重新实现 detail 触发 | Phase 2 早期就定义 `PM_DETAIL_KEY`，所有视图统一用 inject |
| 列表视图行内编辑与详情面板同步 | 所有改动走同一条 `item_update` 路径，事件总线广播 `item-updated` 刷新各视图 |
| 日历拖拽误操作 | 必须有确认 dialog，不静默改日期 |
| 索引未建导致跨项目查询卡顿 | Phase 2 开始前先建好所有索引，避免后期返工 |
| 向后兼容旧 viewMode 设置 | usePmViewMemory 首次读取时 fallback 旧 key，一次性迁移到新 key |

---

## 开工前需用户再次确认的点

1. **是否接受整体工期（预估 8-12 天）**？如不接受，可削减范围（如先做 Phase 1 + 今日视图 + 列表视图，Phase 3 下期再做）
2. **甘特图是否需要改动**？当前设计不动它，仅做视图 registry 注册。如要顺带升级（缩放/依赖连线），需单独立项
3. **列表视图是否上来就做虚拟滚动**？如数据量目前不大（< 500），可 Phase 4 再做
4. **Rust 后端分桶 vs 前端分桶**（四象限）：如倾向前端简单，Phase 3.3 可跳过，前端直接基于 `item_list` 结果分类

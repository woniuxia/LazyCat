# PM 视图扩展设计（今日 / 列表 / 日历 / 四象限 + 切换器基础设施）

- **状态**：草案
- **日期**：2026-04-19
- **目标版本**：v0.4.x
- **关联**：PmPanel.vue、pm.rs、usePmSiyuan.ts、PmDetailPanel.vue、PmItemDialog.vue
- **原型页**：`.superpowers/brainstorm/<session>/{switcher,today,list,calendar,matrix}.html`

## 1. 背景

当前 PM 面板只有 **看板** 和 **甘特图** 两个视图，通过二态 `el-switch` 切换。缺少以下关键场景的能力：

- 跨项目聚合的「今天要做什么」视图
- 高信息密度的批量操作（表格形态）
- 按日期排布的规划视图（日历）
- 按重要/紧急决策的辅助视图（四象限）

本设计新增 4 个视图，同时升级视图切换器为可扩展基础设施。

## 2. 目标与非目标

### 目标

- 基础设施：切换器升级为 Tab 形态，支持 6+ 视图
- 视图注册机制（`viewId + 组件`），新增视图只需注册一条
- 每上下文独立记忆视图选择
- 新增 4 个视图：今日 / 列表 / 日历 / 四象限
- 侧栏新增「今日」快捷入口

### 非目标

- 不改 `pm_items` / `pm_projects` 表结构
- 不做视图自定义（用户新建视图）
- 不做任务拖拽改属性（四象限视图明确不支持）
- 不做时段排布的精细周视图（任务通常无精确小时）
- AI 智能分类/排序属于另一方向，本设计不涵盖

## 3. 核心模型：上下文 × 视图正交

### 上下文（Context）

- `overview`：总览，跨项目聚合数据
- `project-<id>`：具体项目，仅该项目数据

### 视图（View）

| viewId | 标签 | 图标 | 备注 |
|--------|------|------|------|
| `kanban` | 看板 | ▦ | 现有 |
| `gantt` | 甘特 | ▤ | 现有 |
| `today` | 今日 | ◷ | 新增，分区 Dashboard |
| `list` | 列表 | ≡ | 新增，Notion 风格表格 |
| `calendar` | 日历 | ▥ | 新增，月 + 简化周 |
| `matrix` | 四象限 | ⊞ | 新增，决策矩阵 |

### 正交原则

所有视图都能在所有上下文下工作。上下文决定数据集（按项目过滤），视图决定展示方式。新增的 4 个视图跟随上下文变化，不是特殊视图。

- `kanban + overview`：跨项目看板（卡片带项目色点）
- `kanban + project-X`：单项目看板（当前行为）
- `today + overview`：全部项目的今日任务（最常用）
- `today + project-X`：仅该项目的今日任务
- 以此类推

## 4. 侧栏结构调整

### 结构

```
[快捷入口]
└── ◷ 今日         (onClick = switchView('today'))

[数据范围]
├── 总览           (selectedProjectId = 'overview')
├── LazyCat        (selectedProjectId = X)
├── 个人成长
└── 副业
```

### 「今日」入口行为

- `onClick`：切换当前 `viewId` 为 `'today'`，**不改 `selectedProjectId`**
- `badge` 数字 = 当前上下文下的今日任务数（通过 `item_today_counts` 查询）
- 和视图切换器里的「今日」tab 是等价入口（一个是侧栏快捷，一个在顶栏）

## 5. 基础设施（Phase 1）

### 5.1 视图注册表

新建文件 `apps/desktop/src/composables/pmViewRegistry.ts`：

```typescript
export interface ViewDefinition {
  id: ViewId;
  label: string;
  icon: string;              // 或 Element Plus 图标组件
  component: AsyncComponent;  // defineAsyncComponent
}

export type ViewId = 'kanban' | 'gantt' | 'today' | 'list' | 'calendar' | 'matrix';

export const PM_VIEWS: ViewDefinition[] = [
  { id: 'kanban',   label: '看板',   icon: '▦', component: () => import('@/components/PmKanbanView.vue') },
  { id: 'gantt',    label: '甘特',   icon: '▤', component: () => import('@/components/PmGanttView.vue') },
  { id: 'list',     label: '列表',   icon: '≡', component: () => import('@/components/PmListView.vue') },
  { id: 'calendar', label: '日历',   icon: '▥', component: () => import('@/components/PmCalendarView.vue') },
  { id: 'matrix',   label: '四象限', icon: '⊞', component: () => import('@/components/PmMatrixView.vue') },
  { id: 'today',    label: '今日',   icon: '◷', component: () => import('@/components/PmTodayView.vue') },
];
```

### 5.2 切换器组件

新建 `apps/desktop/src/components/PmViewSwitcher.vue`：

- 横向 Tab 容器（6 个 tab）
- 激活态：主色背景 + 白色文字
- 悬停态：主色软背景
- 响应式：窗口宽度 < 1100px 时降级为纯图标（`label` 用 Tooltip 显示）

替换 PmPanel.vue 里的 `el-switch` 部分（line 95 附近）。

### 5.3 `viewMode` 迁移到 `viewId`

```typescript
// 旧
const viewMode = ref<"kanban" | "gantt">("kanban");

// 新
const viewId = ref<ViewId>('kanban');
```

所有 `viewMode === 'kanban'` 的判断同步替换为 `viewId.value === 'kanban'`。

### 5.4 视图选择持久化

通过 `user_settings` 表（已有）存储，key 规则：

- `pm:view:overview` = viewId
- `pm:view:project-<id>` = viewId

切换上下文时读取对应 key 恢复视图。首次进入时使用默认：
- `overview` 默认 `list`
- `project-<id>` 默认 `kanban`（保持现有习惯）

通过新增 composable `usePmViewMemory.ts` 封装读写逻辑。

## 6. 视图 A1：今日视图（Phase 2）

### 6.1 定位

跨项目聚合「今天需要关注的任务」的分区 Dashboard，跟随上下文过滤。

### 6.2 顶部统计条（4 张卡片）

| 卡片 | 主指标 | 副指标 |
|------|--------|--------|
| 逾期未完成 | 数量 | 最长逾期 X 天 |
| 今日到期 | 数量 | P0 × N · P1 × M |
| 进行中 | 数量 | 跨 X 个项目（overview 上下文） |
| 今日已完成 | 数量 | 鼓励文案 |

### 6.3 分区布局

四个分区 + 一个折叠分区：

1. **逾期未完成**（红色警示）：`end_at < today AND status != 'completed'`，按逾期天数降序
2. **今日到期**（黄）：`date(end_at) = today AND status != 'completed'`
3. **进行中**（蓝）：`started_at IS NOT NULL AND status != 'completed'`（可能和上两类重叠，去重显示在最先匹配的分区）
4. **今日已完成**（绿，默认折叠）：`date(completed_at) = today`

### 6.4 任务卡片

```
[checkbox]  [标题]                                [截止时间] [快捷操作]
            [项目色点 项目名] [优先级chip] [状态chip]
```

快捷操作按状态变化：
- `todo` 状态：「开始做」
- 逾期状态：「推到明天」
- 进行中：「标记完成」
- 默认始终有：「详情」

### 6.5 跨项目聚合查询

后端新增 `item_today_list` 接口，参数：
- `project_id: Option<i64>`（None 表示全项目）
- `today_date: String`（客户端传本地日期，避免时区混淆）

返回 4 个分区的任务列表。

## 7. 视图 A2：列表视图（Notion 风格）（Phase 2）

### 7.1 默认列

- 标题（带 pin 图标 + 展开按钮）
- 项目（色点 + 名称）
- 状态
- 优先级
- 截止时间
- 标签
- 更新于

### 7.2 动态列行为

- 单项目上下文：自动隐藏「项目」列
- 可选列（用户在顶栏「列 ▾」勾选）：开始时间、实际开始、完成时间、描述摘要、链接、Todo 数

### 7.3 行内编辑

双击进入编辑态，支持字段：
- **状态**：下拉（todo / doing / testing / completed）
- **优先级**：下拉（P0 / P1 / P2 / P3）
- **项目**：下拉
- **截止时间**：日期选择器
- **标签**：标签选择器
- **标题**：文本输入

不支持行内编辑（必须展开或打开详情）：描述、链接、起止时间、思源/Todo 关联。

### 7.4 批量操作

选中多行 → 底部深色浮条出现（非模态），按钮：
- 标记完成 | 改状态 | 改优先级 | 移动项目 | 批量打标签 | 删除

Esc 或点「取消 ×」清空选择。

### 7.5 分组

顶部「分组 ▾」下拉，选项：`无 / 项目 / 状态 / 优先级 / 标签`。

分组时每组一个可折叠块，组头显示：
- 折叠/展开 caret
- 分组值（带色点）
- 数量
- 关键指标（如「逾期 1 · 进行中 2」）

分组选择**按上下文记忆**（`user_settings` key: `pm:view:list:groupBy:<contextId>`）。

### 7.6 排序

单列排序（不支持多列）：
- 点击列头切换升序 → 降序 → 默认
- 默认排序：`pinned DESC, priority ASC, end_at ASC NULLS LAST, updated_at DESC`

### 7.7 筛选

列头右键或 ▾ 图标 → 该列筛选器（status / priority / project / tags 的多选菜单）。

激活的筛选汇总到表格顶部的「筛选条」，可快速单独移除或整体清除。

### 7.8 行展开与详情

- 点击行首「▶」小图标 → 行内展开快速预览（描述 + 时间元数据 + 快捷操作按钮居中排布）
- 展开内的「打开完整详情面板」按钮 → 触发 `PmDetailPanel.vue`（复用现有组件）
- 点击单元格空白处 → 行内编辑该字段
- 点击标题文字 → 进入标题编辑

### 7.9 性能

数据量 > 500 行时启用虚拟滚动（`vue-virtual-scroller` 或类似）。

## 8. 视图 A3：日历视图（Phase 3）

### 8.1 子视图

- **月视图**（默认）
- **周视图**（简化版：7 列任务列表）
- **不做日视图**（语义和今日视图重复）

子视图切换状态持久化：`pm:view:calendar:subview:<contextId>`。

### 8.2 月视图

#### 布局

- 周起始：**周日**
- 5 或 6 行 × 7 列（按月份起止自动计算）
- 格子 `min-height: 130px`
- 当日蓝边框 + 「今天」徽章
- 周末格子浅灰底
- 跨月日期淡化为「other」样式

#### 任务条显示

单条格式：`[◉ 小圆点] 标题（省略）`

颜色策略（`pm:view:calendar:colorBy:<contextId>`）：
- **项目色**（默认）：任务条底色用项目色
- **优先级色**：P0 红、P1 橙、P2 灰、P3 浅灰
- **状态色**：todo 灰、doing 绿、testing 黄、completed 淡蓝

特殊状态：
- 已完成：淡色 + 删除线
- 逾期：红色背景 + 红色边框

#### 溢出处理（阶梯式）

- **≤ 4 条**：全部显示
- **≥ 5 条**：显示前 3 条 + 底部提示 `<项目A>、<项目B> 等项目还有 N 条`（项目名按该天任务项目去重，取前 2 个）
- 点击提示 → 弹出 Popover 列出当日全部任务

### 8.3 周视图（简化版）

7 列等宽布局，每列结构：

```
[列头]
 周X / 日期数字 / N 项（或 &nbsp; 占位保持高度一致）
[列体]
 任务卡片 × N
 或 "无安排"
```

任务卡片信息：标题 + 项目色点 + 项目名 + 优先级 chip + 状态/逾期/完成 tag。

### 8.4 交互

- **点击日期格空白**：调起 `PmItemDialog`（新建），`end_at` 预填为该日期
- **点击任务条**：弹出 Popover（项目 + 状态 + 快捷操作 + 「详情」）
- **Popover 中的「详情」**：打开 `PmDetailPanel`
- **拖拽任务条**：改 `end_at`（确认对话框避免误操作）
- **Shift+拖拽**：同时改 `start_at`（保持跨度）

### 8.5 字段映射

- 主字段：`end_at`
- 跨日任务（`start_at` 和 `end_at` 都有且跨日）：在每一天格子里都显示一次（不做真正的跨列条，简化实现）
- 逾期任务：**在原 `end_at` 日期显示**，不迁移到今天（让用户直观看到「哪天失约」）

### 8.6 后端接口

新增 `item_calendar_range`：参数 `project_id, start_date, end_date`；返回该范围内的任务列表。

## 9. 视图 A4：四象限视图（Phase 3）

### 9.1 判定规则

| 维度 | 重要 | 不重要 |
|------|------|--------|
| 判定 | P0 / P1 | P2 / P3 |

| 维度 | 紧急 | 不紧急 |
|------|------|--------|
| 判定 | `end_at ≤ today + 紧急阈值` 或已逾期 | `end_at > today + 紧急阈值` 或无 `end_at` |

**紧急阈值**：默认 3 天，可切换 3 / 7 / 14，持久化到 `pm:view:matrix:urgentThreshold`。

### 9.2 象限布局

2×2 网格 + 外围坐标轴标签：

| | 紧急（左） | 不紧急（右） |
|--|-----------|-------------|
| **重要（上）** | I. 立即做（红） | II. 计划做（蓝） |
| **不重要（下）** | III. 快速处理（黄） | IV. 少做/推迟（灰） |

### 9.3 坐标轴

- **纵轴**（左侧）：上半显示「▲重要」，下半显示「不重要▼」。每个标签在各自半区垂直居中。
  - 实现：`writing-mode: vertical-rl; text-orientation: upright;` 让中文和箭头竖排立字
- **横轴**（上方）：「◀ 紧急」左端，「不紧急 ▶」右端

### 9.4 任务卡片

紧凑版：
- 标题
- 左侧项目色条（3px border-left）
- 项目色点 + 项目名
- 优先级 chip
- 截止 tag（今天、明天、逾期 X 天、具体日期）

### 9.5 不支持拖拽

**只读分布视图**。象限位置由 `priority + end_at` 自动计算，拖拽语义模糊（到底改哪个字段？），改属性走行内编辑或打开详情面板。

### 9.6 已完成任务

默认隐藏（顶栏「隐藏已完成 ✓」开关）。持久化：`pm:view:matrix:hideCompleted`。

### 9.7 无截止日期归类

归入「不紧急」：
- P0/P1 + 无截止 → 象限 II（计划做），鼓励定计划
- P2/P3 + 无截止 → 象限 IV（少做），提示可能该清理

### 9.8 和今日视图的差异边界

- **今日视图**：按时间维度组织（逾期/今日/进行中/完成），回答「今天做什么」
- **四象限**：按决策维度组织（重要×紧急），回答「长期优先级对不对」

两者互补不重叠。

## 10. 后端 (Rust) 变更

### 10.1 无 DDL 变更

沿用现有表：`pm_projects`、`pm_items`、`pm_item_siyuan_links`。

### 10.2 新增接口（`pm.rs` + `CHANNEL_MAP`）

| action | 用途 | 参数 |
|--------|------|------|
| `item_today_list` | 今日视图分区数据 | `project_id?, today_date` |
| `item_today_counts` | 侧栏「今日」badge 计数 | `project_id?` |
| `item_calendar_range` | 日历视图区间查询 | `project_id?, start_date, end_date` |
| `item_matrix_bucket` | 四象限分桶数据 | `project_id?, urgent_threshold_days, hide_completed` |
| `item_batch_update` | 列表视图批量操作 | `ids: i64[], fields: {...}` |

### 10.3 现有接口扩展

`item_list`：
- 新参数 `group_by?: string`（列表视图分组支持）
- 参数 `project_id`：已支持为空表示跨项目

### 10.4 性能考量

- 跨项目查询（`project_id IS NULL`）需全表扫描。加索引：
  - `CREATE INDEX IF NOT EXISTS idx_pm_items_end_at ON pm_items(end_at)` (已有待确认)
  - `CREATE INDEX IF NOT EXISTS idx_pm_items_status ON pm_items(status)`
  - `CREATE INDEX IF NOT EXISTS idx_pm_items_updated_at ON pm_items(updated_at)`
- 数据量基准：< 5000 条任务时保持 < 50ms 响应

## 11. 前端组件清单

### 新增

- `apps/desktop/src/components/PmViewSwitcher.vue` - 切换器 Tab
- `apps/desktop/src/components/PmTodayView.vue` - 今日视图
- `apps/desktop/src/components/PmListView.vue` - 列表视图
- `apps/desktop/src/components/PmCalendarView.vue` - 日历视图
- `apps/desktop/src/components/PmMatrixView.vue` - 四象限视图
- `apps/desktop/src/components/PmKanbanView.vue` - 看板视图（从 PmPanel.vue 抽出）
- `apps/desktop/src/components/PmSidebarTodayEntry.vue` - 侧栏今日快捷入口（可选，或内联到 PmPanel.vue）
- `apps/desktop/src/composables/pmViewRegistry.ts` - 视图注册表
- `apps/desktop/src/composables/usePmViewMemory.ts` - 视图选择记忆

### 修改

- `apps/desktop/src/components/PmPanel.vue`
  - 移除 `el-switch` 视图切换，换成 `PmViewSwitcher`
  - 移除内联看板 JSX，改为 `<component :is="currentView.component" />`
  - 侧栏顶部新增「今日」快捷入口
  - `viewMode` 重命名为 `viewId`，类型扩展
- `apps/desktop/src/bridge/tauri.ts` - 注册新 channel
- `apps/desktop/src-tauri/src/tools/pm.rs` - 新增 action 分发
- `apps/desktop/src-tauri/src/tools/pm/item.rs` 或类似子模块 - 实现新接口

### 复用

- `PmDetailPanel.vue` - 所有视图的详情面板
- `PmItemDialog.vue` - 所有视图的新建/编辑 Dialog
- `PmContextMenu.vue` - 右键菜单
- `PmProjectDialog.vue` - 项目管理

## 12. 实施阶段划分

### Phase 1：基础设施（1-2 天）

1. `pmViewRegistry.ts` + `usePmViewMemory.ts`
2. `PmViewSwitcher.vue`
3. 将现有看板从 PmPanel 抽出到 `PmKanbanView.vue`
4. `viewMode` → `viewId` 重命名，类型扩展
5. 侧栏「今日」入口骨架（先只做 UI，点击暂切到已有视图）

**验证**：切换器可以切看板/甘特图，功能和旧版等价。

### Phase 2：高频视图（3-4 天）

1. 今日视图 + 跨项目聚合后端接口（`item_today_list`, `item_today_counts`）
2. 侧栏「今日」入口点击联动（switchView + badge）
3. 列表视图主体（表格、行内编辑、基础排序、筛选）
4. 列表视图批量操作 + 分组
5. 列表视图复用 `PmDetailPanel`

**验证**：
- 今日视图各分区数据正确，侧栏 badge 实时
- 列表视图行内改状态/优先级/截止后持久化，刷新后一致
- 批量改状态能一次更新多条

### Phase 3：规划视图（3-4 天）

1. 日历视图月视图 + `item_calendar_range`
2. 日历视图周视图简化版
3. 日历视图点击新建、Popover、拖拽改日期
4. 四象限视图 + `item_matrix_bucket`
5. 紧急阈值切换 + 隐藏已完成开关

**验证**：
- 月视图下拖拽任务改日期能持久化
- 四象限分布正确，阈值切换实时生效

### Phase 4：打磨（1-2 天）

1. 切换器响应式降级（窄窗口纯图标）
2. 列表视图虚拟滚动（>500 条）
3. 性能优化（索引、查询）
4. 文档更新（CLAUDE.md / AGENTS.md 的 04 章节新增工具链路说明）

## 13. 风险与权衡

### 13.1 跨项目查询性能

全项目查询要全表扫描，> 10000 条时可能变慢。缓解：
- 索引（见 10.4）
- 今日视图限定时间范围 `end_at BETWEEN today-30d AND today+30d`
- 列表视图强制分页或虚拟滚动

### 13.2 上下文 × 视图的双向记忆复杂度

`viewId` 和 `selectedProjectId` 双向记忆可能造成「切上下文时意外切视图」的感知。缓解：
- 首次进入某上下文才用默认视图，之后完全按用户最后选择
- 切换器本身作为主要交互路径（用户主动切，不意外切）

### 13.3 向后兼容

已有用户的 `viewMode` 设置可能存在旧 user_settings 里。读取时做映射：
```typescript
// 读取 pm:view:<context> 失败时 fallback
const legacy = getSetting('pm:viewMode'); // 旧 key
if (legacy === 'kanban' || legacy === 'gantt') return legacy;
return defaultView(context);
```

### 13.4 Windows 字符编码

四象限坐标轴用了 `▲▼` 和中文。CSS `writing-mode` + `text-orientation: upright` 在 WebView2 下的兼容性已在 Chromium 70+ 稳定支持，无需 fallback。

### 13.5 UI 一致性

- 所有视图的卡片/标签/chip 色系统一引用 `theme-light.css` 定义的 `--pm-*` 变量
- 按 CLAUDE.md 05.1 规范同步检查 `element-overrides.css` 和 `theme-light.css`
- 详情面板和 Dialog 全部复用现有组件，避免视觉分裂

## 14. 决策追溯

以下决策在原型页的决策卡里完整展开，设计文档不再重复：

| 视图 | 关键决策 |
|------|----------|
| 切换器 | 形态 A Tab（图标+文字）；响应式降级；`viewId` + 注册对象 |
| 今日 | 4+1 分区；Q4 项目筛选复用侧栏（对称模型） |
| 列表 | 6 默认列 + 动态列；行内编辑 6 字段；批量操作 6 种；分组 5 维度 |
| 日历 | 月视图默认；周起始周日；溢出阶梯规则；拖拽改日期 |
| 四象限 | 紧急阈值默认 3 天可切；不支持拖拽；纵坐标竖排立字 |

## 15. 未来迭代（不在本期）

- 视图内的「保存的筛选器」收藏
- 任务依赖（blocks / blocked by）→ 甘特图连线 + 四象限连线
- 子任务层级 → 树形列表
- AI 智能排序 / 自动归类
- 拖拽在日历/看板里的联动刷新

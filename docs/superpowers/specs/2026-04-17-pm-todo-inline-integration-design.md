# PM-Todo 交互优化设计：内联化 + 双向打通

> 日期: 2026-04-17
> 状态: 已确认

## 背景

当前 PM 工作项与 Todo 任务清单的交互存在以下问题:

1. **6 个弹窗实例** -- PmTodoCreateDialog 和 PmTodoLinkDialog 在编辑模式、创建模式、详情面板三处各一份，代码冗余
2. **创建模式 bug** -- PmPanel.vue 中 `pendingTodoCreates` 和 `pendingTodoLinkItems` 积累的数据在 `submitItem()` 中从未提交到后端
3. **状态不同步** -- 详情面板和编辑对话框各自持有独立的 `usePmTodoLinking` 实例
4. **操作路径深** -- Todo 侧关联工作项藏在「更多设置」下拉框中

## 设计目标

- 消除所有弹窗，改为内联交互
- 双向打通: PM 侧管理 Todo，Todo 侧管理 PM 关联
- 修复创建模式待关联任务不提交的 bug
- 减少代码量

## 方案选择

**选定: 方案 A - 内联自动完成**

- 「绑定已有」触发内联下拉搜索列表，边搜边选
- 零弹窗，搜索+选择一步完成
- 实现简单，复用现有搜索 API

备选方案:
- B. 侧滑抽屉: 空间充裕但占屏幕宽度，需额外动画逻辑
- C. 气泡 Popover: 轻量但空间有限，不适合大量候选项

## 详细设计

### 一、PM 侧 - InlineTodoList 组件

**新组件**: `InlineTodoList.vue`（约 200 行）

替换 PmDetailPanel 和 PmItemDialog 中的任务区域，以及 PmPanel 中的 pending 状态管理。

**布局（从上到下）**:

1. **进度摘要**（保持现有）-- `3/5 已完成` + 进度条
2. **已关联任务列表** -- 每行紧凑显示:
   - 复选框（点击切换 完成/待办）
   - 逾期红点（如有）
   - 任务标题（完成状态显示删除线）
   - 优先级标签 P0-P3（右侧）
   - hover 时右侧出现「解绑」按钮
3. **内联输入框** -- placeholder「输入任务标题，回车创建...」
   - 左侧文本输入，右侧 P0-P3 优先级选择器（默认 P2）
   - 回车即创建并关联，无需额外确认
4. **绑定已有任务入口** -- 文字链接「+ 绑定已有任务」
   - 点击展开内联搜索区: 搜索框 + 候选列表（带复选框，支持多选）
   - 点选即绑定，Esc 或点外部收起

**Props**:
- `pmItemId`: `() => number | undefined`（getter）
- `items`: `PmTodoLinkItem[]`
- `summary`: `PmTodoSummary | null`
- `loading`: `boolean`
- `mode`: `'detail' | 'edit'`
- `candidates`: `PmTodoCandidateItem[]`
- `candidatesLoading`: `boolean`

**Emits**:
- `create(title: string, priority: string)` -- 快速创建任务
- `toggle(id: number)` -- 切换完成状态
- `unlink(id: number)` -- 解绑任务
- `link(ids: number[])` -- 批量绑定
- `search-candidates(keyword: string)` -- 搜索候选任务

**模式差异**:
- `detail` 模式: 隐藏创建/绑定入口，只展示只读列表
- `edit` 模式: 完整交互能力

### 二、Todo 侧 - InlinePmSelector 组件

**新组件**: `InlinePmSelector.vue`（约 180 行）

替换 TodoDetailEdit 中现有的 project selector + pm-link-selector 区域。项目+工作项合为一个卡片，始终可见，不再藏于「更多设置」。

**三种显示状态**:

1. **空态**（无项目/无关联）-- 虚线框引导「+ 关联项目或工作项」
2. **已关联** -- 卡片展示:
   - 项目色标 + 项目名称（顶部）
   - 工作项状态标签 + 标题（中部）
   - 一键解除按钮（右侧 x）
   - 「切换关联」文字链接（底部）
3. **搜索中** -- 内联下拉:
   - 搜索框 + 候选工作项列表（带状态标签）
   - 底部「新建工作项并关联」入口

**Props**:
- `projectId`: `number | null`
- `projectName`: `string | null`
- `projectColor`: `string | null`
- `pmItemId`: `number | null`
- `pmItemTitle`: `string | null`
- `pmItemStatus`: `string | null`
- `candidates`: `PmCandidateItem[]`
- `candidatesLoading`: `boolean`
- `projectList`: `PmProject[]`（用于新建工作项时选项目）

**Emits**:
- `link(pmItemId: number)` -- 关联工作项
- `unlink()` -- 解除关联
- `create-pm(title: string, projectId: number)` -- 新建工作项并关联
- `search(keyword: string)` -- 搜索候选工作项
- `change-project(projectId: number)` -- 切换项目

### 三、Todo 列表卡片

在 TodoPanel 的任务卡片底部，当任务有关联工作项时显示:

```
[状态标签] #12 前端界面重构
```

- 状态标签使用与 PM 面板一致的颜色方案
- 一行小字，不影响卡片主体布局
- 点击可跳转到 PM 面板（保留现有 navigateToPm 行为）

### 四、Composable 重构

**`usePmTodoLinking.ts` 简化**（243 行 → 约 120 行）

移除:
- `createDialogVisible`、`createForm`（弹窗状态）
- `linkDialogVisible`、`linkSelectedIds`（弹窗状态）
- `loadCandidates()` 的弹窗关联逻辑
- `submitCreate()`、`submitLink()`（弹窗提交逻辑）

保留:
- `loadItems(pmItemId)` -- 加载关联任务列表
- `toggleComplete(todo)` -- 切换完成状态
- `unlink(todoItemId)` -- 解绑任务
- `items`、`summary`、`loading` 响应式状态

新增:
- `quickCreate(title, priority)` -- 内联创建任务并自动关联当前 PM 工作项
- `searchCandidates(keyword)` -- 搜索候选任务，返回候选列表供 InlineTodoList 消费
- `linkBatch(ids: number[])` -- 批量绑定任务

### 五、创建模式 Bug 修复

**问题**: PmPanel.vue 中创建工作项时，`pendingTodoCreates` 和 `pendingTodoLinkItems` 积累的数据在 `submitItem()` 中从未提交到后端。

**修复方案**: 创建模式下 InlineTodoList 维护本地待关联列表（无需 composable 实例）。`submitItem()` 拿到新建工作项 ID 后，依次调用 `quickCreate` 和 `linkBatch` 完成批量关联。

移除 PmPanel.vue 中手写的 `pendingTodo*` 系列状态（约 65 行）:
- `pendingTodoCreates`
- `pendingTodoLinkItems`
- `pendingTodoCreateDialogVisible`
- `pendingTodoLinkDialogVisible`
- `pendingTodoLinkKeyword`
- `pendingTodoLinkSelectedIds`
- `pendingTodoLinkLoading`
- `pendingTodoLinkCandidates`
- `pendingTodoLinkReason`

统一由 InlineTodoList 组件内部管理本地待关联状态，通过 emit 将数据传回父组件在提交时批量处理。

### 六、删除的文件

- `PmTodoCreateDialog.vue`（78 行）
- `PmTodoLinkDialog.vue`（87 行）

## 影响范围

| 文件 | 操作 | 预计改动量 |
|------|------|-----------|
| `InlineTodoList.vue` | 新建 | ~200 行 |
| `InlinePmSelector.vue` | 新建 | ~180 行 |
| `PmTodoCreateDialog.vue` | 删除 | -78 行 |
| `PmTodoLinkDialog.vue` | 删除 | -87 行 |
| `usePmTodoLinking.ts` | 重构 | 243→120 行 |
| `PmPanel.vue` | 改造 | -200 行 |
| `PmItemDialog.vue` | 改造 | -80 行 |
| `PmDetailPanel.vue` | 改造 | -60 行 |
| `TodoDetailEdit.vue` | 改造 | -80 行 |
| `TodoDetailView.vue` | 改造 | -20 行 |
| `TodoPanel.vue` | 改造 | -40 行 |

**净效果**: +380 行新组件，-545 行旧代码，总计减少约 165 行。消除 6 个弹窗实例和 1 个 bug。

## 不在范围内

- `usePmSiyuan.ts` 不涉及
- 数据库结构不变（仍使用 `TodoItem.pmItemId` 单字段关联）
- Rust 后端不变（现有 IPC 通道足够支撑）
- TodoPanel 的其他功能（筛选、分组等）不动

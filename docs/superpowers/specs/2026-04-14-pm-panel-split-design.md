# PmPanel.vue 保守拆分设计

> 日期：2026-04-14
> 状态：已确认

## 背景

PmPanel.vue 当前 4809 行（模板 998 + 脚本 1193 + scoped 样式 1617 + 全局样式 998），难以导航和维护。已完成的拆分包括 PmItemDialog.vue、PmTodoCreateDialog.vue、PmTodoLinkDialog.vue、usePmSiyuan.ts、usePmTodoLinking.ts。

本次目标：保守拆分，只提取三个最大且边界清晰的独立区块，将 PmPanel 降至可维护的范围。

## 目标

- 提取 3 个子组件：PmDetailPanel、PmSiyuanDrawer、PmProjectDialog
- 样式随组件走，PmPanel 只保留自身布局和编排相关的样式
- 通过 provide/inject 共享 usePmSiyuan composable 实例
- usePmSiyuan 别名保持现状（在 PmPanel 中），不做重构；仅服务于此抽屉的别名可移除
- 不改变任何用户可见行为

## 整体架构

```
PmPanel.vue (编排层)
├── provide('pmSiyuan', siyuanComposable)
├── 侧边栏 (保留，含项目右键菜单)
├── 工具栏 + 看板/甘特图 (保留)
├── <PmDetailPanel />        ← 新组件（含详情面板的 todo 弹窗）
├── <PmSiyuanDrawer />       ← 新组件
├── <PmProjectDialog />      ← 新组件（含思源位置选择器依赖）
├── <PmItemDialog />         (已存在，含其 todo 弹窗)
└── 右键菜单 Teleport        (保留)
```

## 组件 1：PmDetailPanel.vue

**提取范围：** PmPanel.vue 行 279-474（模板，含 `<Transition name="pm-detail-slide">` 包裹层）+ 详情面板相关样式

**职责：** 显示选中工作项的完整详情 — 英雄区、时间线、描述、关联任务、资源链接、操作按钮。

### Props

| Prop | 类型 | 说明 |
|------|------|------|
| project | `PmProject \| null` | 所属项目（显示项目芯片） |
| item | `PmItem \| null` | 当前工作项 |

### Emits

| Emit | Payload | 说明 |
|------|---------|------|
| close | - | 关闭详情面板 |
| toggle-pin | item | 置顶/取消置顶 |
| advance-status | item | 推进状态 |
| delete | item | 删除工作项 |

### Inject

- `pmSiyuan` — 仅用于 `openSiyuanPage(url)` 方法，打开思源页面链接。思源页面数据（`siyuanPrimaryPage`、`siyuanExtraPages`）直接从 `item` prop 读取，不需要通过 inject。

### 内部调用

- `usePmTodoLinking(() => props.item?.id)` — 详情面板自己的任务关联实例，管理已关联 todo 的加载、新建、绑定、解绑。watch `item` prop 变化触发 `loadItems`。详情面板内的 PmTodoCreateDialog 和 PmTodoLinkDialog 实例由本组件管理。

### 常量与辅助函数

以下需在子组件中自行导入或从 PmPanel 迁入：

**从外部模块导入：**
- `PM_STATUS_COLUMNS`、`PM_ITEM_TYPE_MAP`、`PM_PRIORITY_MAP` — 状态/类型/优先级常量
- `getPmLightTagStyle` — 标签样式计算
- `formatPmDateRangeForDisplay` — 日期范围格式化

**从 PmPanel 迁入：**
- `isOverdue(item: PmItem)` — 判断工作项是否逾期（注意：接收 PmItem 对象，不是 date）
- `nextStatusLabel(status)` — 获取下一状态标签
- `formatDateTime(date)` — 格式化时间
- `normalizeItemLinkUrl(url)` / `openItemLink(url)` — 外部链接处理

### 模板结构

```
<Transition name="pm-detail-slide">  ← 由本组件接管
<aside class="pm-detail">
  头部（标题 + 关闭按钮）
  英雄区（项目芯片 + 工作项标题 + 类型/优先级/状态/标签）
  时间线区块（时间安排、创建、执行、测试、完成）
  描述区块
  关联任务区块
    已关联 todo 列表 + 摘要 + 操作按钮
    PmTodoCreateDialog / PmTodoLinkDialog（详情面板实例）
  资源链接区块
    外部链接（打开/移除）
    思源主页面（从 item prop 读取数据，inject pmSiyuan 调用 openSiyuanPage）
    思源附加页面（同上）
  操作按钮（置顶、推进状态、删除）
</aside>
</Transition>
```

### 样式迁移

从 PmPanel 的 `<style scoped>` 和 `<style>` 中迁移：
- `.pm-detail` 及内部所有样式（~200 行）
- 详情面板相关的新版视觉覆盖样式

### PmPanel 变化

- 模板行 279-474（含 Transition）替换为 `<PmDetailPanel v-if="selectedItemId" :project="selectedProject" :item="selectedItem" @close="selectedItemId = null" @toggle-pin="togglePin" @advance-status="advanceStatus" @delete="deleteItem" />`
- 详情面板相关的 todo 弹窗实例（行 478-495）移入 PmDetailPanel
- 移除详情面板相关样式
- 移除详情面板专属的辅助函数（isOverdue、nextStatusLabel、formatDateTime、normalizeItemLinkUrl、openItemLink）
- 详情面板专属的 pmTodo linking 实例和 state（行 1108-1177 中 detail 面板部分）移入 PmDetailPanel
- 移除 `watch(selectedItemId, ...)` 触发 pmTodo.loadItems 的 watcher（改由 PmDetailPanel 内部 watch `item` prop）
- 注意：`onDetailClickAway` 中引用了 `.pm-detail` 选择器，提取后需确认点击检测仍有效（PmDetailPanel 使用 scoped style 时类名不变，但需要实际验证）

## 组件 2：PmSiyuanDrawer.vue

**提取范围：** PmPanel.vue 行 713-995（模板）+ ~399 行全局思源样式

**职责：** 思源配置抽屉、位置选择器、页面选择器的 UI 展示。

### Props / Emits

无。所有数据和操作通过 inject 的 composable 实例获取。

### Inject

- `pmSiyuan` — 思源 composable 实例，直接访问：
  - 状态：drawerVisible、config、locationPickerVisible、pagePickerVisible、tree、搜索结果等
  - 方法：saveConfig、testConnection、loadTree、选择位置/页面、确认/取消等

### 模板结构

```
<el-drawer>  思源配置抽屉
  服务地址输入
  Token 输入
  默认位置选择
  保存 / 测试连接 / 加载目录 按钮
  连接状态提示
  目录树浏览

<el-dialog>  位置选择器
  搜索框
  树形选择器
  当前选择预览
  确认/取消按钮

<el-dialog>  页面选择器
  搜索输入
  搜索范围切换
  全库搜索开关
  新建页面入口
  结果列表
  确认/取消按钮
```

### 样式迁移

从 PmPanel 的 `<style>` 全局样式中迁移：
- `.siyuan-drawer` 及内部所有样式
- `.siyuan-config-card`、`.siyuan-page-list`、`.siyuan-picker` 等
- `.siyuan-tree`、`.siyuan-error-alert` 等
- 部分样式需保留为非 scoped（el-drawer/el-dialog 子元素穿透）

### PmPanel 变化

- 模板行 713-995 替换为 `<PmSiyuanDrawer />`
- 移除思源相关全局样式（~399 行）
- 仅服务于此抽屉的 usePmSiyuan 别名可移除（如 drawerVisible、config 等，改由子组件 inject 直接访问）
- 仍被 PmPanel 其他区域使用的别名保留（如 openDrawer 触发按钮在工具栏）

## 组件 3：PmProjectDialog.vue

**提取范围：** PmPanel.vue 行 497-532（模板）+ ~118 行项目 CRUD 函数 + 相关样式

**职责：** 项目新建/编辑对话框 + 项目 CRUD 操作（含归档、恢复、删除）。

**重要依赖：** 项目对话框内嵌思源位置选择器（`useSiyuanOverride`、`openSiyuanLocationPicker('project')`、`globalSiyuanLocation`、`formatPmSiyuanLocationLabel`、`clearProjectSiyuanOverride`），需要通过 inject pmSiyuan 获取这些能力。

### Props / Emits

无 props。对话框的打开/关闭由内部状态管理。

| Emit | Payload | 说明 |
|------|---------|------|
| projects-changed | `{ newProjectId?: string }` | 项目列表变更，通知父组件刷新。`newProjectId` 用于新建后自动选中新项目 |

### Inject

- `pmSiyuan` — 访问思源位置选择器相关功能：
  - `globalSiyuanLocation` — 显示全局默认位置
  - `formatPmSiyuanLocationLabel` — 格式化位置标签
  - `openSiyuanLocationPicker('project')` — 打开位置选择器
  - `clearProjectSiyuanOverride` — 清空项目覆盖位置

### 内部状态

- `projectDialogVisible` — 对话框可见性
- `editingProject` — 当前编辑的项目（null 为新建模式）
- `projectForm` — 表单数据（名称、颜色、描述、useSiyuanOverride、siyuanLocationOverride）
- `presetColors` — 颜色预设列表

### defineExpose

| 方法 | 参数 | 说明 |
|------|------|------|
| showCreate() | - | 打开新建项目对话框 |
| showEdit(project) | PmProject | 打开编辑项目对话框 |
| handleContext(action, project) | `'archive' \| 'restore' \| 'delete'`, PmProject | 处理右键菜单动作 |

### 内部函数

- `showCreateProject()` — 重置表单并打开对话框
- `showEditProject(project)` — 填充表单并打开对话框
- `submitProject()` — 提交表单（新建或更新）。新建成功后 emit `projects-changed` 并携带新项目 ID，父组件据此自动选中
- `archiveProject(project)` — 归档项目，成功后 emit `projects-changed`
- `restoreProject(project)` — 恢复项目，成功后 emit `projects-changed`
- `deleteProject(project)` — 删除项目，成功后 emit `projects-changed`
- `onProjectContext(action, project)` — 右键菜单分发

### 模板结构

```
<el-dialog>
  项目名称输入
  颜色选择（预设色块）
  思源位置设置（inject pmSiyuan 获取位置选择器功能）
  描述输入
  确定/取消按钮
</el-dialog>
```

### 样式迁移

从 PmPanel 中迁移项目对话框相关的少量样式。

### PmPanel 变化

- 模板行 497-532 替换为 `<PmProjectDialog ref="projectDialogRef" @projects-changed="onProjectsChanged" />`
- 侧边栏"新建项目"按钮改为调用 `projectDialogRef.value.showCreate()`
- 项目右键菜单动作改为调用 `projectDialogRef.value.handleContext(action, project)`
- 新增 `onProjectsChanged({ newProjectId? })` 处理函数：刷新项目列表，若有 newProjectId 则自动选中
- 移除相关 state（projectDialogVisible、editingProject、projectForm、presetColors）
- 移除相关 CRUD 函数（~118 行）
- 移除相关样式

## provide/inject 约定

### Key

```typescript
const PM_SIYUAN_KEY: InjectionKey<ReturnType<typeof usePmSiyuan>> = Symbol('pmSiyuan')
```

### Provider（PmPanel.vue）

```typescript
const siyuan = usePmSiyuan({ ... })
provide(PM_SIYUAN_KEY, siyuan)
```

### Consumer（PmDetailPanel.vue、PmSiyuanDrawer.vue、PmProjectDialog.vue）

```typescript
const siyuan = inject(PM_SIYUAN_KEY)!
```

导出 key 的位置：`apps/desktop/src/composables/pmSiyuanKey.ts`（新建小文件，仅导出 InjectionKey）。

## 样式迁移策略

### 原则

- 子组件使用 `<style scoped>` 包裹自身样式
- 需要穿透 Element Plus 子元素（el-drawer、el-dialog）的样式使用 `<style>` 非 scoped 块（每个组件最多一个非 scoped 块）
- 全局 `<style>` 中不属于任何子组件的共享样式留在 PmPanel

### 样式归属

| PmPanel 全局样式区域 | 归属 | 说明 |
|---------------------|------|------|
| `.pm-siyuan-drawer .el-drawer__body` 等 | PmSiyuanDrawer | 思源抽屉专属 |
| `.siyuan-config-card`、`.siyuan-page-list`、`.siyuan-picker`、`.siyuan-tree`、`.siyuan-error-alert` | PmSiyuanDrawer | 思源配置 UI |
| `.pm-form-item-top` | 共享 | 项目对话框和 PmItemDialog 都使用 |
| `.pm-item-dialog-form`、`.pm-item-section` | 留在 PmPanel | 属于 PmItemDialog slots |
| PM-Todo Linking 样式 | 留在 PmPanel | 详情面板和 item dialog slots 共用 |

## 行数预估

| 文件 | 变化前 | 变化后 |
|------|--------|--------|
| PmPanel.vue | 4809 | ~3100（含剩余样式） |
| PmDetailPanel.vue | 新建 | ~600 |
| PmSiyuanDrawer.vue | 新建 | ~650 |
| PmProjectDialog.vue | 新建 | ~380 |
| pmSiyuanKey.ts | 新建 | ~5 |

## 验证计划

1. `pnpm typecheck` — 类型检查通过
2. `pnpm --filter @lazycat/desktop build:web` — 构建通过
3. `pnpm test` — 单元测试通过
4. 手动验证：
   - 详情面板：打开/关闭动画（pm-detail-slide Transition）、工作项显示、任务关联、思源页面链接、操作按钮
   - 思源配置抽屉：打开、位置选择器、页面选择器
   - 项目对话框：新建（含自动选中）、编辑、归档、恢复、删除、思源位置选择器
   - 看板拖拽、右键菜单功能不受影响
   - onDetailClickAway 点击检测仍正常工作

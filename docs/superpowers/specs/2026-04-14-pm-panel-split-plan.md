# PmPanel.vue 拆分实施计划

> 依据设计文档：`docs/superpowers/specs/2026-04-14-pm-panel-split-design.md`
> 目标：从 4809 行的 PmPanel.vue 中提取 3 个子组件

---

## Phase 0: 基础设施 — provide/inject 与 InjectionKey

**目标：** 创建共享 key 文件，在 PmPanel 中添加 provide，为后续组件提取铺路。

### 任务

1. **创建 `apps/desktop/src/composables/pmSiyuanKey.ts`**
   - 导出 `InjectionKey<ReturnType<typeof usePmSiyuan>>`
   - 内容约 5 行

2. **在 PmPanel.vue 中添加 provide**
   - 位置：script 区 `usePmSiyuan(...)` 调用之后（约 line 1207）
   - 导入 `provide`（已在 line 1001 的 vue 导入中）和 `PM_SIYUAN_KEY`
   - 添加 `provide(PM_SIYUAN_KEY, siyuan)`
   - 此时 `siyuan` 变量即 `usePmSiyuan(...)` 的返回值

### 文件引用

- 新文件：`apps/desktop/src/composables/pmSiyuanKey.ts`
- 修改：`apps/desktop/src/components/PmPanel.vue`（script imports + provide 调用）
- 参考：`apps/desktop/src/composables/usePmSiyuan.ts`（返回类型，line 764-854）

### 验证

- `pnpm typecheck` 通过
- 现有功能无变化（provide 只是注册，尚无消费者）

---

## Phase 1: 提取 PmDetailPanel.vue

**目标：** 提取详情面板为独立组件。这是最自包含的区块，适合先提取。

### 1.1 创建 PmDetailPanel.vue 骨架

**模板来源：** PmPanel.vue lines 279-474（Transition + aside.pm-detail）+ lines 478-495（detail 面板的 PmTodoCreateDialog + PmTodoLinkDialog）

**Props/Emits 定义（从设计文档）：**

```typescript
// Props
project: PmProject | null   // 显示项目芯片
item: PmItem | null         // 当前工作项

// Emits
close                       // 关闭详情面板
toggle-pin: [item]          // 置顶/取消置顶
advance-status: [item]      // 推进状态
delete: [item]              // 删除工作项
```

**模板迁移清单（精确行号）：**

| 内容 | PmPanel 行号 | 说明 |
|------|-------------|------|
| `<Transition name="pm-detail-slide">` + `<aside class="pm-detail">` | 279-474 | 整体搬入 |
| `selectedItem` → `item` prop | 280, 多处 | 变量替换 |
| `selectedItemProject` → `project` prop | 294-296 | 变量替换 |
| `selectedItemId = null` → `emit('close')` | 286 | 行为替换 |
| `togglePin` → `emit('toggle-pin', item)` | 466 | 行为替换 |
| `advanceStatus` → `emit('advance-status', item)` | 467 | 行为替换 |
| `deleteItem` → `emit('delete', item)` | 470 | 行为替换 |
| PmTodoCreateDialog (detail) | 479-482 | 搬入组件内部 |
| PmTodoLinkDialog (detail) | 485-495 | 搬入组件内部 |

### 1.2 内部状态与 composable

- `usePmTodoLinking(() => props.item?.id)` — 详情面板自己的 pmTodo 实例，包装 `reactive()`
- `selectedItemDescriptionText` — 改为从 `props.item` 派生的 computed
- `isOverdue(item)` — 从 PmPanel 迁入，内部调用 `isPmItemOverdue`
- `nextStatusLabel(item)` — 从 PmPanel 迁入
- `formatDateTime(date)` — 从 PmPanel 迁入（line 2150-2154）
- `normalizeItemLinkUrl(url)` — 从 PmPanel 迁入（line 1427-1439）
- `openItemLink(url)` — 从 PmPanel 迁入（line 1441-1449）
- `getPmLightTagStyle` — 从 utils 导入
- `formatPmDateRangeForDisplay` — 从 utils 导入

### 1.3 inject

- `pmSiyuan` — 仅用于 `openSiyuanPage(url)` 打开思源页面链接
- 思源页面数据直接从 `item` prop 读取（`item.siyuanPrimaryPage`, `item.siyuanExtraPages`）

### 1.4 样式迁移

**从 scoped style（`<style scoped>`）迁移：**

| 行号 | 内容 |
|------|------|
| 2659-2769 | 原始详情面板样式（.pm-detail, .detail-*, .pm-detail-slide-*） |
| 3516-3715 | 重新设计详情面板样式（覆盖层） |
| 3717-3725 中 .pm-detail 相关 | 响应式断点中详情面板相关 |

**从全局 style（`<style>`）迁移：**

| 行号 | 内容 |
|------|------|
| 4371-4395 | .detail-siyuan-page-* 样式 |

### 1.5 PmPanel.vue 变更

- 模板 lines 279-474 替换为：
  ```html
  <PmDetailPanel v-if="selectedItem" :project="selectedItemProject" :item="selectedItem"
    @close="selectedItemId = null" @toggle-pin="togglePin" @advance-status="advanceStatus"
    @delete="deleteItem" />
  ```
- 删除模板 lines 478-495（detail 面板的 todo 弹窗，已迁入子组件）
- 移除 `pmTodo` reactive composable 实例（line 1109）
- 移除 `watch(selectedItemId, ...)` 中的 pmTodo.loadItems/reset 逻辑（line 1482-1488）— 改由子组件内部 watch `item` prop
- 移除详情面板辅助函数：isOverdue、nextStatusLabel、formatDateTime、normalizeItemLinkUrl、openItemLink
- 移除 `selectedItemDescriptionText` computed（line 1353）— 迁入子组件
- 移除详情面板相关样式
- 添加 PmDetailPanel 组件导入

### 验证

- `pnpm typecheck` 通过
- `pnpm --filter @lazycat/desktop build:web` 通过
- 详情面板打开/关闭动画正常
- 工作项信息完整显示
- Todo 关联功能正常（创建/绑定/解绑/完成）
- 思源页面链接可点击打开
- 置顶/推进状态/删除按钮正常工作
- **onDetailClickAway 点击检测仍有效**（需验证 `.pm-detail` 类名在 scoped style 下仍可被 document.querySelector 匹配）

---

## Phase 2: 提取 PmSiyuanDrawer.vue

**目标：** 提取思源配置抽屉 + 位置选择器 + 页面选择器。这是最大的提取区块。

### 2.1 创建 PmSiyuanDrawer.vue 骨架

**模板来源：** PmPanel.vue lines 713-995

| 模板块 | 行号 | 行数 |
|--------|------|------|
| el-drawer 思源配置 | 713-813 | 101 |
| el-dialog 位置选择器 | 815-920 | 106 |
| el-dialog 页面选择器 | 922-995 | 74 |
| **合计** | | **281** |

### 2.2 数据获取方式

**全部通过 inject `pmSiyuan` 获取，无需任何 props/emits。**

子组件内部创建本地别名映射（从 inject 的 composable 实例）：

| 模板中使用的别名 | inject 映射 |
|-----------------|-------------|
| `siyuanDrawerVisible` | `siyuan.drawerVisible` |
| `siyuanForm` | `siyuan.form` |
| `siyuanShowToken` | `siyuan.showToken` |
| `siyuanTesting` | `siyuan.testing` |
| `siyuanTestingVersion` | `siyuan.testingVersion` |
| `siyuanLoadingDirectory` | `siyuan.loadingDirectory` |
| `siyuanDirectory` | `siyuan.directory` |
| `siyuanDirectoryFetchedAt` | `siyuan.directoryFetchedAt` |
| `siyuanError` | `siyuan.error` |
| `siyuanErrorTitle` | `siyuan.errorTitle` |
| `siyuanTreeProps` | `siyuan.treeProps` |
| `siyuanLocationDialogVisible` | `siyuan.locationDialogVisible` |
| `siyuanLocationPickerTitle` | `siyuan.locationPickerTitle` |
| `siyuanLocationPickerSearch` | `siyuan.locationPickerSearch` |
| ... (约 40+ 个状态别名) | ... |
| `saveSiyuanConfig` | `siyuan.saveConfig` |
| `handleTestConnection` | `siyuan.testConnection` |
| `handleLoadDirectory` | `siyuan.loadDirectory` |
| ... (约 20+ 个函数别名) | ... |

**注意：** 别名命名在模板中需要保持不变（如 `siyuanDrawerVisible`），但来源从 PmPanel 的顶层变量改为 `inject` 返回值的属性。

### 2.3 额外导入

- `formatPmSiyuanLocationLabel` — 从 `utils/pmSiyuan` 导入
- `isPmSiyuanNotebookDirectory` — 从 `utils/pmSiyuan` 导入
- 类型：`PmSiyuanNotebookDirectory` 等

### 2.4 样式迁移

**从全局 style（`<style>`）迁移：**

| 行号 | 内容 |
|------|------|
| 3866-3868 | `.pm-siyuan-drawer .el-drawer__body` |
| 4201-4238 | `.pm-siyuan-config-card`, `.pm-siyuan-link-card`, 通知框 |
| 4240-4370 | `.pm-siyuan-link-*`, `.pm-siyuan-page-*`, `.pm-siyuan-empty-*` |
| 4396-4618 | `.siyuan-drawer-*`, `.siyuan-tree-*`, `.siyuan-node-*`, `.pm-siyuan-picker-*`, `.siyuan-error-alert` |
| 4620-4683 中 siyuan 相关 | 响应式断点中 siyuan 相关选择器 |

**样式块组织：**
- `<style scoped>` — 组件内部布局样式
- `<style>` 非 scoped — 需要穿透 el-drawer/el-dialog 的样式

### 2.5 PmPanel.vue 变更

- 模板 lines 713-995 替换为 `<PmSiyuanDrawer />`
- 移除仅服务于此抽屉的 usePmSiyuan 别名（约 50+ 个状态别名 + 20+ 个函数别名）
- **保留**仍被 PmPanel 其他区域使用的别名：
  - `openSiyuanDrawer` — 工具栏按钮触发
  - `openSiyuanPageDialog` — item dialog 中使用
  - `openSiyuanLinkPicker` — item dialog 中使用
  - `openReplacePrimarySiyuanDialog` — item dialog 中使用
  - `handleItemSiyuanPageCommand` — item dialog 中使用
  - `applyItemPrimaryPage` / `addItemExtraPage` / `hasItemLinkedPage` / `removeItemLinkedPage` — item dialog 中使用
  - `itemEffectiveLocation` — item dialog 中使用
  - `globalSiyuanLocation` — project dialog 中使用
  - `openSiyuanLocationPicker` — project dialog 中使用（但此功能将在 Phase 3 后移至 PmProjectDialog）
  - `clearProjectSiyuanOverride` — project dialog 中使用（同上）
  - `formatPmSiyuanLocationLabel` — project dialog 中使用（同上）
- 移除思源相关全局样式（~420 行）
- 添加 PmSiyuanDrawer 组件导入

### 验证

- `pnpm typecheck` 通过
- `pnpm --filter @lazycat/desktop build:web` 通过
- 思源配置抽屉可打开/关闭
- 保存配置、测试连接正常
- 位置选择器可打开、选择、确认/取消
- 页面选择器可打开、搜索、选择、新建
- 工具栏"思源设置"按钮仍能打开抽屉

---

## Phase 3: 提取 PmProjectDialog.vue

**目标：** 提取项目新建/编辑对话框及项目 CRUD 操作。

### 3.1 创建 PmProjectDialog.vue 骨架

**模板来源：** PmPanel.vue lines 497-532（36 行）

### 3.2 defineExpose

```typescript
defineExpose({
  showCreate,      // 打开新建项目对话框
  showEdit,        // 打开编辑项目对话框
  handleContext,    // 处理右键菜单动作（archive/restore/delete）
});
```

### 3.3 内部状态

从 PmPanel 迁入：

| 变量 | PmPanel 行号 | 类型 |
|------|-------------|------|
| `projectDialogVisible` | 1076 | `Ref<boolean>` |
| `editingProject` | 1077 | `Ref<PmProject \| null>` |
| `projectForm` | 1078-1084 | `Ref<{name, description, color, useSiyuanOverride, siyuanLocationOverride}>` |
| `presetColors` | 1085 | 常量数组 |

### 3.4 内部函数

从 PmPanel 迁入（约 118 行）：

| 函数 | PmPanel 行号 |
|------|-------------|
| `showCreateProject` → `showCreate()` | 1537-1548 |
| `showEditProject(p)` → `showEdit(p)` | 1550-1560 |
| `resetProjectForm` | 1562-1564 |
| `submitProject` | 1566-1600 |
| `archiveProject` | 1602-1609 |
| `restoreProject` | 1611-1618 |
| `deleteProject` | 1620-1635 |
| `onProjectContext` → `handleContext` | 1637-1652 |

### 3.5 inject

```typescript
const siyuan = inject(PM_SIYUAN_KEY)!;
// 使用：
// siyuan.globalSiyuanLocation — 显示全局默认位置
// siyuan.formatPmSiyuanLocationLabel — 格式化位置标签（需确认是否在 composable 返回中）
// siyuan.openSiyuanLocationPicker('project') — 打开位置选择器
// siyuan.clearProjectSiyuanOverride — 清空项目覆盖位置
```

**注意：** `formatPmSiyuanLocationLabel` 来自 `utils/pmSiyuan` 工具函数，不在 composable 返回中，需直接导入。

### 3.6 Emits

```typescript
const emit = defineEmits<{
  'projects-changed': [{ newProjectId?: string }];
}>();
```

### 3.7 样式迁移

项目对话框样式很少，主要使用全局样式中的共享类（`.pm-item-dialog-*`、`.pm-item-section-*`）。这些共享样式留在 PmPanel 中。

### 3.8 PmPanel.vue 变更

- 模板 lines 497-532 替换为 `<PmProjectDialog ref="projectDialogRef" @projects-changed="onProjectsChanged" />`
- 添加 `projectDialogRef` ref
- 侧边栏"新建项目"按钮改为调用 `projectDialogRef.value.showCreate()`
- 项目右键菜单改为调用 `projectDialogRef.value.handleContext(action, project)`
- 新增 `onProjectsChanged` 处理函数：刷新项目列表 + 自动选中新项目
- 移除相关 state（projectDialogVisible, editingProject, projectForm, presetColors）
- 移除 CRUD 函数（~118 行）
- 移除 `onProjectContext` 函数
- 添加 PmProjectDialog 组件导入

**`onProjectsChanged` 实现：**
```typescript
async function onProjectsChanged({ newProjectId }: { newProjectId?: string }) {
  await loadProjects();
  await loadItemCounts();
  if (newProjectId) {
    selectedProjectId.value = Number(newProjectId);
  }
}
```

### 3.9 额外考虑

- `onProjectContext`（line 1637-1652）构建右键菜单，需确认菜单定位逻辑（`openCtxMenuAt`）留在 PmPanel 还是也迁入。**决策：** 留在 PmPanel，`handleContext` 只处理业务动作。
- `submitProject` 中成功后需要刷新项目列表 — 改为 emit `projects-changed` 由父组件处理。

### 验证

- `pnpm typecheck` 通过
- `pnpm --filter @lazycat/desktop build:web` 通过
- 新建项目（含思源位置选择）正常，新项目自动选中
- 编辑项目正常
- 归档/恢复/删除项目正常
- 侧边栏右键菜单动作正常

---

## Phase 4: 清理与最终验证

**目标：** 清理 PmPanel 中不再需要的代码，全面验证。

### 4.1 PmPanel.vue 清理

- 移除已迁入子组件的所有别名变量
- 移除不再需要的导入
- 确认 `onDetailClickAway` 中 `.pm-detail` 选择器仍然有效（scoped style 下 Vue 会添加 data-v 属性，但 document.querySelector 可能匹配不到）

  **处理方案：** 在 PmDetailPanel 的 aside 元素上添加一个固定类名（如 `pm-detail-panel`），并在 `onDetailClickAway` 中使用该类名。或改用 ref 判断。

  **推荐方案：** 保留 `.pm-detail` 类名，在 PmDetailPanel 中不使用 scoped style 作用于根元素（使用普通类名），或使用 `:deep()` / unscoped 块。实际上，`onDetailClickAway` 使用 `document.querySelector`，不受 Vue scoped 影响 — 只要 DOM 中存在 `.pm-detail` 类名即可。**需实际测试验证。**

### 4.2 全量验证清单

| 验证项 | 方法 |
|--------|------|
| 类型检查 | `pnpm typecheck` |
| 构建 | `pnpm --filter @lazycat/desktop build:web` |
| 单元测试 | `pnpm test` |
| 详情面板动画 | 手动：pm-detail-slide transition |
| 详情面板数据 | 手动：选中工作项后所有字段正确显示 |
| 详情面板 Todo | 手动：创建/绑定/解绑/完成 todo |
| 详情面板思源 | 手动：点击思源页面链接 |
| 详情面板操作 | 手动：置顶/推进/删除 |
| 点击外部关闭 | 手动：onDetailClickAway |
| 思源抽屉 | 手动：打开/配置/保存/测试连接 |
| 思源选择器 | 手动：位置选择器 + 页面选择器 |
| 项目 CRUD | 手动：新建/编辑/归档/恢复/删除 |
| 看板拖拽 | 手动：拖拽排序 |
| 右键菜单 | 手动：工作项/项目右键 |
| 甘特图 | 手动：切换视图并操作 |

### 4.3 行数预估

| 文件 | 变化后行数 |
|------|-----------|
| PmPanel.vue | ~3100（含剩余共享样式） |
| PmDetailPanel.vue | ~600 |
| PmSiyuanDrawer.vue | ~650 |
| PmProjectDialog.vue | ~380 |
| pmSiyuanKey.ts | ~5 |
| **合计** | ~4735（与原始 4809 基本持平，但分布合理） |

---

## 执行风险与缓解

| 风险 | 缓解措施 |
|------|---------|
| onDetailClickAway 匹配失败 | Phase 4 专门验证；备选方案：用 ref 代替 querySelector |
| scoped style 类名冲突 | 子组件使用 scoped + 必要时 unscoped 块 |
| 思源别名遗漏 | 逐个核对模板引用与 inject 映射 |
| 响应式断点遗漏 | 全量迁移断点中相关选择器 |
| 双层样式（原始+重设计）迁移不完整 | 原始层作为基础一并迁入子组件 |

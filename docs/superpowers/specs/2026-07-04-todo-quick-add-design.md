# 任务清单快速添加栏设计

- 日期：2026-07-04
- 状态：交互设计与技术设计均已与用户确认
- 影响范围：`apps/desktop` 前端；后端零改动

## 1. 背景与目标

任务清单（Todo）面板内新建任务目前必须点击工具栏“新建”按钮并填写右侧完整表单，对“随手记一条”的高频场景摩擦过大。本次用户定位的两个痛点中，仅处理“添加任务不够快”；“浏览与查找不便”用户明确暂缓。

目标：在任务清单列表顶部提供常驻的快速添加栏——输入标题回车即建、焦点保留可连续录入；输入行右侧带日期与优先级两个轻量速选控件（用户在两个候选形态中选定了带内联速设的方案 B）。

### 非目标（明确排除）

- 浏览/查找类优化（时间分区、分组、智能清单等）
- 自然语言日期解析（如“明天下午3点 交周报”）
- 全局快速捕获小窗（Ctrl+Shift+N）的增强
- 新建表单默认值记忆
- `src-tauri` 后端任何改动

## 2. 交互设计

### 2.1 位置与显隐

- 常驻于工具栏下方、待办列表上方。
- 仅列表视图显示；日历视图不显示（日历已有“点日期创建”入口）。

### 2.2 构成

- 标题输入框，placeholder：`添加任务，回车创建…`。
- 日期速选控件：菜单项为 今天 / 明天 / 选日期…（弹日历）/ 清除日期；未选择时呈灰色占位“日期”，选中后显示具体值并可单独清除（×）。
- 优先级速选控件：P0-P3，默认 P2；选中非默认值时着色显示。

### 2.3 时间规则

- 后端 `eventAt` 要求完整 RFC3339 时间且 5 分钟对齐，数据模型无“仅日期”概念。
- 选“今天”：取当前时刻向后取整到下一个 5 分钟刻度，避免一创建就被判定“已逾期”；深夜边界（如 23:58）自然跨到次日 00:00，语义按“5 分钟后”接受。
- 选“明天”或自选日期：该日 09:00（与现有编辑表单默认时间惯例一致）。
- 不设提醒：`reminderPresets` 固定为 `["none"]`。

### 2.4 回车创建与连续录入

- 空标题回车忽略。
- 创建成功后：标题清空、焦点保留；日期/优先级选值保留（便于连续录入同类任务）。
- Esc：清空标题并将两个控件重置为默认。

### 2.5 继承筛选上下文

- 当前筛选了分类（`filterType`，单选）→ 新任务自动带该分类。
- 当前筛选了具体项目（`filterProjectId` 为 number）→ 新任务归入该项目；为 `"none"` 或 null 时不设项目。
- 优先级控件的初始默认值跟随优先级筛选（无筛选时为 P2），用户可手动覆盖。
- 设计目的：新任务不会“建完就被当前筛选条件藏起来”。

### 2.6 创建反馈

- 成功：输入框边框绿色一闪；新任务按现有排序插入列表并高亮约 1.5 秒渐隐；不自动滚动（避免连续录入时视口跳动）。
- 失败：红色提示展示后端错误信息；标题与控件值原样保留，不丢用户输入。

## 3. 技术设计

### 3.1 组件

- 新增 `apps/desktop/src/components/TodoQuickAddBar.vue`（延续 TodoPanel 已有的子组件拆分模式）：
  - 内部状态仅三个：标题、日期选值、优先级选值。
  - Props：`context: { typeId: number | null; projectId: number | null; priorityDefault: TodoPriority }`。
  - Emits：`created(id: number)`。
- `TodoPanel.vue` 变更：
  - 由筛选状态计算 `context`（`filterType` 存的是分类名，用面板已加载的分类列表解析为 typeId；`filterProjectId` 为具体 id 时才继承）。
  - 监听 `created` → `loadItems()` → 将该 id 设为高亮目标，约 1.5 秒后清除。
  - 仅在列表视图模板中挂载快速添加栏。

### 3.2 纯函数 util

- 新增 `apps/desktop/src/utils/todoQuickAdd.ts`：`buildQuickAddPayload(input, context, now)` 合成 item-create payload（时间取整、继承规则、字段裁剪）。
- 复用 `todoSchedule.ts` 已有的 5 分钟刻度处理与 `combineLocalDateTime`；如需使用其内部 `DEFAULT_TIME`，做最小导出调整。
- 配套 `todoQuickAdd.test.ts` 单测。

### 3.3 调用链

- `TodoQuickAddBar` → `useToolInvoke` → 既有通道 `tool:todo:item-create`。
- payload：`{ title, typeId?, priority, eventAt?, projectId?, reminderPresets: ["none"] }`。
- 后端 `item_create` 已支持上述全部字段并返回新任务 id，无需任何改动。

### 3.4 UI 选型与样式

- 日期与优先级速选用 `el-dropdown`；“选日期…”触发 `el-date-picker`。
- 如需覆盖 Element Plus 样式，遵守 `element-overrides.css` 与 `theme-light.css` 双文件同步约束。
- 视觉保持面板现有干净浅色风格。

## 4. 错误处理

- 创建失败：`ElMessage.error` 展示后端错误信息；输入不清空。
- `reminderPresets=["none"]` 保证不会触发后端“设置提醒前需要先提供事件时间”校验。
- 5 分钟对齐由前端合成逻辑保证，`todoQuickAdd.ts` 单测覆盖。

## 5. 测试与验证

- `todoQuickAdd.test.ts`：时间取整（含午夜回绕边界）、筛选继承规则、空标题、payload 形状。
- `TodoQuickAddBar` 组件测试（参照既有 `TodoPanel.*.test.ts` 模式）：回车创建、空标题忽略、Esc 重置、创建成功后清空且焦点保留。
- 基线验证：相关单测 + `pnpm typecheck` + `pnpm --filter @lazycat/desktop build:web`。

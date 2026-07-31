# 桌面挂件快捷操作增强设计

**日期**：2026-05-10
**状态**：待实现

## 一、背景

当前桌面挂件（360×800）展示内容仅包含概览统计和待办列表，底部扩展位空白。用户希望在挂件上补充快捷操作入口，减少打开主窗口的频率。

## 二、设计目标

1. 概览块底部增加 **动态工具推荐**（3 个），按近 30 天使用频率排序
2. 待办列表表头增加 **新建待办** 按钮
3. 点击快捷入口通过 IPC 通知主窗口执行对应操作

## 三、布局变更

### 概览块（WidgetOverviewBlock）

现有布局不变（左侧进度环 + 右侧警戒栏），下方增加分隔线和工具推荐行：

```
┌─────────────────────────────────┐
│ [进度环]  ⚠ P0×2  ⏰ 3h       │
│ ─────────────────────────────── │
│ 📋 PM   🔍 搜索   📝 Inbox     │  ← 新增：动态推荐
└─────────────────────────────────┘
```

- 高度从 200px 维持不变（内容自行撑开）
- 分隔线使用 `--wc-divider` 色
- 每个工具按钮：半透明背景 `--wc-block-bg`，显示工具名
- 无使用记录时不显示此行（隐藏分隔线）
- 敏感模式下工具推荐正常显示（工具入口不含敏感信息）

### 待办列表表头（WidgetTodoList）

列表顶部新增表头栏：

```
┌─────────────────────────────────┐
│ 待办事项                + 新建  │  ← 新增：表头
│ ─────────────────────────────── │
│ ☐ 修复登录页样式     逾期2天   │
│ ☐ 重构 API 层          明天    │
└─────────────────────────────────┘
```

- 表头高度 ~36px，左标题右按钮
- "+ 新建" 按钮：半透明背景，与动态推荐按钮同一风格
- 空态（无待办）时表头仍显示

## 四、类型变更

### WidgetDashboardData 扩展（`types/widget.ts`）

```typescript
export interface WidgetHotTool {
  /** 工具 ID，对应 ToolDef.id */
  id: string;
  /** 近 30 天点击次数 */
  count: number;
}

export interface WidgetDashboardData {
  overview: WidgetOverview;
  todoList: WidgetTodoItem[];
  echo: string | null;
  generatedAt: string;
  privacyMask?: boolean;
  hotTools: WidgetHotTool[]; // 新增：仅含 id + count，name 由前端查 toolCatalog
}
```

`WidgetOverview` 不变。`hotTools` 放在 `WidgetDashboardData` 顶层，由 `WidgetCanvas` 向下传递给 `WidgetOverviewBlock`。不放入 `WidgetOverview`，因其语义不属于概览指标。

## 五、后端变更

### data.rs — dashboard_data()

新增 `user_settings` 读取路径（`data.rs` 此前仅读取 `pm_items` / `todo_items` 表，这是首次跨到 `user_settings` 读数据）：

1. 通过 `config::read_string("tool_clicks")` 读取 `user_settings` 中的点击历史
2. 解析 JSON `Record<string, number[]>`，统计每个工具近 30 天（`now - 30 * 86400000`）的点击数
3. 排除 `todo` 和 `widget`（挂件内不应推荐挂件本身或已有快捷入口的待办）
4. 取 Top 3，返回 `[{ id, count }]` — **不含 name**，由前端 widget 通过 `toolCatalog.ts` 查找中文名
5. 返回 `Vec<WidgetHotTool>`

**错误处理**：

- `tool_clicks` key 不存在 / 值为 null → 返回空数组，不报错
- JSON 解析失败 → `log::warn` + 返回空数组，不阻断 dashboard 主流程

### 前端名称查找（替代 Rust TOOL_NAME_MAP）

`WidgetCanvas.vue` 收到 `hotTools` 后，导入 `getAllToolMap()` 从 `toolCatalog.ts` 查找展示名称。该 Map 已覆盖全部 50+ 工具（与 sidebar 实际注册一致），无需在 Rust 端维护重复映射。

```typescript
import { getAllToolMap } from "../composables/toolCatalog";

function resolveHotToolNames(hotTools: WidgetHotTool[]): Array<WidgetHotTool & { name: string }> {
  const map = getAllToolMap();
  return hotTools
    .map((t) => ({ ...t, name: map.get(t.id)?.name }))
    .filter((t) => t.name !== undefined);
}
```

缺失 ID 的工具被静默丢弃（理论上不会发生，因为 `tool_clicks` 的 key 均来自 `isRealToolId` 验证过的工具 ID）。

### apply.rs — compute_input_hash

`hotTools` 须纳入 `compute_input_hash` 的输入。将序列化后的 hot tools 数组拼接进 hash 字符串，确保工具推荐变化时能触发重新推送。

若 `hotTools` 变化但 overview + todoList 不变（例如：用户点击了工具，虽 immediate refresh 时 dashboard 内容未变但频率计数变了），Hash 能检测到变化并推送新数据。实际上用户点击行为通常会伴随 5s 内的 `dashboard_data_invalidated` 触发（`events.rs` 已监听 `widget://canvas-action`），所以刷新周期原本就会走到。

## 六、前端变更

### 事件两层转发（重要）

子组件（`WidgetOverviewBlock`、`WidgetTodoList`）与 `WidgetCanvas` 之间使用 **Vue emit** 通信；`WidgetCanvas` 收拢后通过 **Tauri `emit()`** 发到 Rust 后端。两层用相同的 payload 结构但不同的传输机制：

```
子组件 Vue emit("action", payload)
  → WidgetCanvas 模板 @action="onAction"
  → WidgetCanvas 调用 Tauri emit("widget://canvas-action", payload)
  → Rust events.rs 接收
```

现有代码（`WidgetCanvas.vue:90`）已使用此模式，本次变更仅扩展 action 类型。

### WidgetCanvas.vue

1. 接收 `hotTools` 从 `widget://dashboard-data` 事件
2. 向下传递：`WidgetOverviewBlock` — props `hotTools`
3. 新增 `onCanvasAction` 处理：子组件 emit 的 action 统一通过 `emit("widget://canvas-action", payload)` 转发后端

### WidgetOverviewBlock.vue

1. 新增 props: `hotTools: WidgetHotTool[]`
2. 新增 template：分隔线 + 工具推荐按钮行
3. 点击工具按钮：`emit("action", { kind: "open-tool", toolId })`
4. 无推荐（空数组）时隐藏该行

### WidgetTodoList.vue

1. 新增表头栏：左侧 "待办事项"，右侧 "+ 新建" 按钮
2. 点击新建：`emit("action", { kind: "open-todo-create" })`
3. 空态时仍显示表头

### canvas-action 类型定义

| kind               | payload                    | 行为                                   |
| ------------------ | -------------------------- | -------------------------------------- |
| `open-tool`        | `{ kind, toolId: string }` | 主窗口聚焦到指定工具面板               |
| `open-todo-create` | `{ kind }`                 | 主窗口切换到 Todo 面板并打开创建对话框 |

### 主窗口响应

后端 `events.rs` 收到 `widget://canvas-action` 后，通过 Tauri event `widget://navigate` 推送给主窗口：

- `open-tool { toolId }`：主窗口 `App.vue` 调用 `onSelect(toolId)` 切换到目标工具
- `open-todo-create`：主窗口切换 `activeTool = "todo"`，同时设置 `TodoPanel` 内部状态打开创建对话框。具体实现：`widget://navigate` 携带 `{ action: "open-todo-create" }`，`App.vue` 切换工具后通过 provide/inject 或 event 通知 `TodoPanel` 弹出创建表单。

主窗口（`App.vue`）需新增 `widget://navigate` 事件监听，当前尚未存在（此为新增功能点）。

## 七、数据流

```
user_settings.tool_clicks
  → Rust dashboard_data() 读取 + 统计 Top 3
  → WidgetDashboardData.hotTools（仅 id + count，纳入 compute_input_hash）
  → Tauri event widget://dashboard-data
  → WidgetCanvas 接收，通过 getAllToolMap() 查找工具名
  → resolve 后的 hotTools 通过 props 传给 WidgetOverviewBlock
  → 用户点击按钮
  → 子组件 emit("action", ...) → WidgetCanvas 收拢
  → WidgetCanvas Tauri emit("widget://canvas-action", ...)
  → Rust events.rs 接收，emit("widget://navigate", ...) 到主窗口
  → App.vue 监听 widget://navigate，切换工具 / 触发创建对话框
```

## 八、不变项

- 扩展位（WidgetExtensionSlot）保持现状不动
- 概览块进度环和警戒栏逻辑不变
- 待办列表行样式和交互不变
- 敏感模式仅对 todo 标题打码；概览块、表头和工具推荐正常显示
- 挂件尺寸 360×800 不变

## 九、风险点

1. **名称查找**：工具名由前端 `toolCatalog.ts` 的 `getAllToolMap()` 提供，与 sidebar 注册同源，无额外维护负担。新增工具时只需在 `toolCatalog.ts` 中注册（已有流程），映射自动生效。
2. **跨 WebView IPC**：`widget://navigate` 是新增事件，主窗口当前无此监听。需在 `App.vue` 新增 `listen("widget://navigate", ...)`，且需处理主窗口最小化到托盘时的唤醒逻辑。
3. **首次使用 / 清零**：无点击记录时 `hotTools` 为空数组，概览块不显示推荐行。用户首次使用需一段时间积累点击数据后才出现推荐。
4. **tool_clicks 跨层依赖**：这是 `data.rs` 首次读取 `user_settings`，与以往仅依赖业务表不同。`tool_clicks` 缺失或格式错误时不阻断主流程，降级为空推荐。

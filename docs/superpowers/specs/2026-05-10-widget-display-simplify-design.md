# 桌面挂件展示效果重构设计

> 日期：2026-05-10
> 状态：待实现

## 1. 目标

简化桌面挂件视觉层级，从三区块（概览 + 待办 + 快捷操作）缩减为两区块（待办 + 快捷操作），并将热门工具与固定快捷按钮合并为一个统一的快捷区。

## 2. 当前架构

```
┌──────────────────────────┐
│  Drag Handle (16px)       │
├──────────────────────────┤
│  WidgetOverviewBlock      │  ← 移除
│  - SVG 环形图             │
│  - P0 徽章 / 截止日期徽章  │
│  - 热门工具按钮            │
├──────────────────────────┤
│  WidgetTodoList           │
│  - 最多 10 条，溢出 +N 件  │
├──────────────────────────┤
│  WidgetExtensionSlot      │
│  - [PM] [待办] [Inbox]   │
└──────────────────────────┘
```

## 3. 目标架构

```
┌──────────────────────────┐
│  Drag Handle (16px)       │
├──────────────────────────┤
│  WidgetTodoList           │
│  - 可滚动，无条数上限       │
│  - 自定义滚动条样式         │
│  - 隐私遮罩保留             │
├──────────────────────────┤
│  WidgetExtensionSlot      │  ← 合并热门工具
│  [PM][待办][Inbox] [热1][热2][热3]
└──────────────────────────┘
```

### 关键决策

| 决策项     | 结论                                                                                  |
| ---------- | ------------------------------------------------------------------------------------- |
| 移除内容   | OverviewBlock 整体移除（环形图、P0 徽章、截止日期徽章、热门工具行）                   |
| 待办列表   | 可滚动，最多 100 条（后端截断），移除 MAX_LINES 和 "+N 件"                            |
| 快捷区合并 | 固定按钮在前，热门工具在后，统一横向排列，无视觉分隔                                  |
| 滚动与收起 | 不处理冲突，保持现有 Full→Peek 800ms 收起逻辑（滚动中不触发收起，由现有交互逻辑保证） |
| 隐私遮罩   | 保留                                                                                  |
| 窗口尺寸   | 保持 360x800                                                                          |

## 4. 前端组件变更

### 4.1 移除 WidgetOverviewBlock.vue

- 整个文件删除
- WidgetCanvas.vue 移除对该组件的引用和渲染

### 4.2 WidgetTodoList.vue 改造

- 外层容器改为 `overflow-y: auto`，添加自定义滚动条样式（6px 宽，thumb 使用 `--wc-bg-tertiary`，track 透明，hover 态 thumb 加深；深色/浅色主题通过 `--wc-*` 变量自动适配）
- 移除 `MAX_LINES` 常量
- 移除 "+N 件" 溢出提示逻辑
- 后端返回多少条就渲染多少条（后端 `TODO_LIMIT` 放宽至 100，前端可滚动浏览全部条目）

### 4.3 WidgetExtensionSlot.vue 扩展

- 新增 `hotTools` prop（类型：`{ id: string; label: string }[]`，label 由 `WidgetCanvas` 解析后传入）
- 渲染顺序：固定按钮（PM / 待办 / Inbox）→ 热门工具按钮
- 固定按钮与热门工具按钮统一风格，无视觉分隔
- **去重规则**：热门工具在渲染前过滤掉与固定按钮重复的 `id`（如 PM 同时出现在固定区和热门区，热门区不再重复显示）
- 热门工具按钮点击触发 `{ kind: 'open-tool', toolId }` 事件，与固定按钮走同一事件通道（通过 Tauri IPC 聚焦主窗口并切换 `activeTool`，主应用未运行时由系统托盘唤起）
- **样式修正**：移除内联 `rgba(255,255,255,...)` 硬编码颜色，改用 `--wc-*` CSS 变量，确保浅色/深色主题均可适配

### 4.4 WidgetCanvas.vue 调整

- 移除 OverviewBlock 引用和渲染
- 将 `dashboard.hotTools` 传递给 ExtensionSlot
- 布局简化为：拖拽手柄 → 待办列表 → 快捷操作区
- 移除 overview 相关响应式变量和 watch

## 5. 后端数据变更

> 文件实际路径：`src-tauri/src/tools/widget/`（widget 子模块），非 `src-tauri/src/tools/`。

### 5.1 data.rs

- 后端无 `DashboardData` 结构体，仪表盘 payload 通过 `serde_json::Value` + `json!({})` 动态构建
- 移除 `overview` 对象的构建代码：`completedToday`、`totalToday`、`p0Pending`、`nearestDeadlineHours` 的 SQL 查询与 JSON 组装
- 保留 `todoList` 和 `hotTools` 构建逻辑不变
- `TODO_LIMIT = 20` 同步放宽至 `100`（配合前端"无上限"承诺），后续视性能可再调整
- 保留 `HOT_TOOLS_LIMIT = 3` 不变

### 5.2 dashboard_logic.rs

- 移除 `compute_nearest_deadline_hours()` 函数（overview 专用，通过 `cargo check` + 全局 grep 确认无其他调用方）
- `format_deadline_label()` 已标记 `#[allow(dead_code)]`，一并移除（同样经 grep 确认无引用）
- `compute_dashboard_hash()` 移除 `overview` 输入，仅对 `todoList + hotTools + privacyMask` 计算哈希
- 保留 `merge_and_dedup_items()`、`sort_dashboard_items()` 等 todoList/hotTools 相关函数

### 5.3 apply.rs

- 哈希计算已随 `compute_dashboard_hash()` 修改同步调整，此处无需额外改动

### 5.4 无变更模块

`config.rs`、`session.rs`、`pulse.rs`、`widget.rs`、`guards.rs`、`diagnostics.rs`、`conflicts.rs`、`mod.rs`（widget 子模块入口）均不受影响。

## 6. 类型变更

### types/widget.ts

- 移除 `WidgetOverview` 类型
- `WidgetDashboardData` 中移除 `overview` 字段
- `WidgetDashboardData` 中移除 `echo` 字段（后端始终返回 `null`，经 grep 确认前端无消费方，为未启用的占位符，同步清理）
- 保留 `todoList`、`hotTools`、`privacyMask` 类型

## 7. 风险与缓解

| 风险                                             | 缓解措施                                                                                                           |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------ |
| 移除 overview 后遗漏后端引用                     | `cargo check` + 全局 grep 确认无引用，编译 + typecheck 覆盖                                                        |
| 待办列表大数据量渲染性能                         | 后端截断放宽至 100，滚动容器天然按需渲染，Vue 虚拟列表可后续引入                                                   |
| 快捷区按钮过多导致换行                           | 限制热门工具最多 3 个，固定 3 个，去重后总计最多 6 个，360px 足够单行                                              |
| 移除 OverviewBlock 后待办列表获得更多高度空间    | 利用多出空间展示更多待办项，无需调整窗口尺寸                                                                       |
| 哈希计算移除 overview 输入后首次启动触发全量推送 | 升级后首次哈希必然不匹配，触发一次全量推送；该推送仅更新仪表盘数据，不影响隐私遮罩等独立状态，属可接受的一次性开销 |
| 概览信息无替代入口                               | 如需恢复需回退本变更或新增轻量概览行，当前用户可通过主应用 PM 面板查看详情                                         |

## 8. 验证策略

- `pnpm typecheck`：类型变更覆盖检查
- `pnpm --filter @lazycat/desktop build:web`：前端构建验证
- `cargo check`（在 `src-tauri/` 下）：Rust 编译验证
- 手动验证：启动挂件，在 Full/Peek 两种状态下确认待办列表可滚动、快捷区布局正常、热门工具去重生效

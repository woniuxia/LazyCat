# API Workbench 元信息操作组窄宽度对齐修复设计

## 背景与根因

请求配置和保存按钮移动到 `.api-workbench-primary-actions` 后，现有 CSS 仍包含两项互相冲突的约束：

- `.api-workbench-primary-actions` 使用 `flex-wrap: wrap`。
- `.meta-environment-select` 在默认样式下固定 `width: 180px`，并继承 `.environment-select { flex: none; }`，无法收缩。

上一轮修复只在 `max-width: 1180px` 内覆盖环境选择器的 Flex 行为。但截图中的问题发生在 `1180px–1380px` 三栏布局：编辑区可收缩至约 `420px`，元信息行右侧操作区宽度不足以同时容纳固定 180px 的环境选择器、配置按钮、保存按钮和间距，于是保存按钮被 Flex 换到下一行。

## 目标

- 接口名称、环境、配置、保存处于同一视觉行时，右侧操作组内部始终保持单行。
- 空间不足时优先收缩环境选择器，不改变配置和保存按钮尺寸。
- `1180px` 以下元信息行按现有规则上下分层时，环境、配置、保存仍保持同一操作行。
- 不修改模板、事件绑定、Tab 顺序或业务逻辑。

## 方案

只修改 `apps/desktop/src/components/ApiWorkbenchPanel.vue` 的局部样式：

1. 为 `.api-workbench-primary-actions` 设置 `min-width: 0` 和 `flex-wrap: nowrap`，允许该 Grid Item 收缩并禁止内部按钮换行。
2. 将 `.meta-environment-select` 的默认布局改为可收缩 Flex Item：保留 `180px` 基准宽度，设置 `width: auto`、`min-width: 0`、`flex: 1 1 180px`。
3. 删除 `max-width: 1180px` 中重复的 `.meta-environment-select` 覆盖，因为相同行为应在所有窗口宽度生效。
4. `.curl-actions` 继续允许换行，不与 `.api-workbench-primary-actions` 共用新的 nowrap 规则。

## 响应式行为

- `>1380px`：环境选择器通常保持 180px，名称输入框占据剩余空间。
- `1181px–1380px`：操作区变窄时环境选择器优先收缩，配置和保存按钮保持同一行。
- `≤1180px`：元信息行继续变为上下两行；第二行的环境、配置、保存保持单行，环境选择器填充剩余宽度。
- `≤820px` 和最小验证宽度 `375px`：操作组保持单行且无横向溢出。

## 改动边界

- 只修改 `ApiWorkbenchPanel.vue` 的 CSS。
- 不改请求栏、环境选择逻辑、配置 Popover、保存行为和 Element Plus 全局主题。
- 不引入容器查询、新断点或“更多”菜单。

## 验证

1. 执行 `git diff --check`。
2. 执行 `pnpm typecheck`。
3. 执行 `pnpm --filter @lazycat/desktop build:web`。
4. 检查截图对应的约 420px 编辑区宽度，以及窗口宽度 `1380px`、`1181px`、`1180px`、`820px`、`375px`：
   - 保存按钮不换行。
   - 环境选择器可收缩，内容截断不撑开操作组。
   - 配置和保存按钮尺寸不变。
   - 元信息区与请求栏无横向滚动。

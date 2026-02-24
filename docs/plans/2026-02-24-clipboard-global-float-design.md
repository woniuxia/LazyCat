# 剪贴板提示条全局浮动改造

## 背景

代码片段、密码管理、快捷启动三个面板使用独立的 workspace viewMode 渲染，
与 `main` 视图互斥（`v-if / v-else-if`）。`ClipboardSuggestionBar` 仅存在于
`main` 分支的 `<main class="content">` 内，workspace 模式下不在 DOM 中，
导致剪贴板智能检测在这三个页面无法显示提示。

## 方案

将 `ClipboardSuggestionBar` 提升为全局浮动通知，脱离 viewMode 分支。

### 模板结构

- 从 `<main class="content">` 中移除 `<ClipboardSuggestionBar>`
- 在 App.vue 模板根层（`v-if/v-else-if` 链之前）放置单实例
- Vue 3 支持多根节点，无需额外包裹

### 样式改造

- `position: fixed`，水平居中，`top: 12px`
- `max-width: 600px`，`width: auto` 自适应内容
- `z-index: 9000`（高于内容区，低于对话框）
- 增强 `box-shadow` 使浮动条在任何背景上可辨识
- 移除原有的 `margin-bottom: 16px`（不再参与文档流）
- 进出动画方向调整为从顶部滑入（`translateY(-12px)` → `translateY(0)`）

### 交互行为

- workspace 中点击操作按钮 → `onClipboardToolOpen` 切回 main 视图并打开目标工具（已有逻辑，无需改动）
- 6 秒自动消失、Esc 关闭、悬停暂停倒计时 — 均不变

### 不改动的部分

- `useClipboardSuggestion` composable（状态已是全局单例）
- `detectClipboard()` 调用逻辑
- `consumePendingInput` 机制
- 各目标面板的集成代码

## 涉及文件

| 文件 | 改动 |
|------|------|
| `apps/desktop/src/App.vue` | 模板：移动 `ClipboardSuggestionBar` 到根层 |
| `apps/desktop/src/components/ClipboardSuggestionBar.vue` | 样式：`position: fixed` + 居中 + z-index + 阴影增强 + 动画方向调整 |

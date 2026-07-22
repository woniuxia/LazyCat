# UI 与样式经验

适用范围：Element Plus 覆盖、Teleport、右键菜单、布局状态和视觉验证。

关键词：`Element Plus`、`Teleport`、`theme-light.css`、`Dropdown`、`Dialog`

## Element Plus 主题覆盖

- 修改 Element Plus 变量或组件覆盖时，同时检查 `src/styles/element-overrides.css` 与 `src/styles/theme-light.css`。
- `theme-light.css` 的 `html[data-theme="light"]` 特异度更高，单改前者可能在浅色主题失效。
- `ElMessageBox` 宽度通过 `customClass` 覆盖 `--el-messagebox-width`，不要直接硬改 `width`。

**使用次数**：0

## Teleport 内容不依赖父容器变量

`Dialog`、`Drawer` 等 Teleport 到 `body` 的内容拿不到父容器局部 CSS 变量。弹层使用全局 token 或弹层自身变量；验证不能只看 `typecheck/build`，还要检查弹窗态、空态和浅色主题。

**使用次数**：0

## Dropdown 打开本地弹层的时序

从右键 Dropdown 打开组件内 Dialog/Drawer 时，先让 Dropdown 完成关闭流程再打开弹层。模板函数 ref 只写普通 `Map` 等非响应式缓存，避免渲染期写响应式状态触发递归更新。

**使用次数**：0

## UI 中间态先核对模板与样式

接手未完成 UI 改动时先看 `git diff`、模板新类名与样式定义是否对应。编译通过不能证明视觉完成；scoped 页面样式与 Teleport 全局样式要分别核对。

**使用次数**：0

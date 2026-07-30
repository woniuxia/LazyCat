# UI 与样式经验

适用范围：Element Plus 覆盖、Teleport、右键菜单、scoped CSS、响应式布局状态和视觉验证。

关键词：`Element Plus`、`Teleport`、`theme-light.css`、`scoped CSS`、`Dropdown`、`Dialog`、`overflow`

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

## 程序化子组件明确 scoped CSS 边界

同文件中通过 `defineComponent()` 与 `h()` 渲染的子组件内部 DOM，不能假设会命中父 SFC 的 scoped 选择器。优先拆成自持模板和样式的独立 SFC；确需全局选择器时使用唯一业务前缀限制作用域，并检查最终 DOM 上的 scope 属性和构建后的 CSS，而不是只看类名存在。

**对话证据**：2026-07 的 JSON 树与 KV 编辑器连续两次因该边界出现视觉返工。

**使用次数**：0

## 响应式布局按真实容器和状态矩阵验证

多栏桌面界面的有效宽度由窗口、侧栏、分隔条、滚动条和条件区域共同决定，不能只按 viewport 断点推算。布局改动至少检查用户报告宽度、关键断点前后、常用与最窄支持窗口，并切换会改变结构的条件字段；覆盖长路径、长日志、空态、弹层、最大化、内容溢出和滚动链。

滚动所有权保持单一且显式：纵向 flex/grid 链路中的可滚动子项通常需要祖先 `min-height: 0`，横向区域需要明确 `min-width: 0` 与溢出策略。固定工具栏、侧栏和条件表单应保持稳定轨道，避免内容或控件显隐造成整体跳变、重叠或操作入口不可达。

需要启动产品 UI 或开发服务器才能完成视觉验证时，按项目规则先向用户申请；未运行时只报告静态和构建结果，不写“视觉已验证”。

**对话证据**：2026-07 至少 10 个独立会话出现窄窗重叠、页面无法滚动、字号不可读、最大化留白、动态网格跳变或宽度计算错误。

**使用次数**：0

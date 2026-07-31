# API Mock 面板 tab 内容区滚动修复设计

- 日期：2026-07-09
- 状态：已确认（用户已选定方案 A：内容区滚动）
- 影响文件：`apps/desktop/src/components/ApiMockPanel.vue`（仅 `<style scoped>` 块）

## 1. 问题

API Mock 页面右侧详情区使用 `el-tabs`（「路由」「请求日志」两个 tab）。当窗口高度不足以容纳 tab 内容时：

- 路由表单下半部分被直接裁剪，看不到「响应头」区块与 CORS 折叠区；
- 任何一层都不出现纵向滚动条，内容不可达；
- 保存栏（`position: sticky; bottom: 0`）仍贴底可见，但其上方内容缺失。

## 2. 根因

Element Plus 2.13.2（实际安装版本）的 tabs 默认样式：

```css
.el-tabs {
  display: flex;
}
.el-tabs--top {
  flex-direction: column;
}
.el-tabs__content {
  flex-grow: 1;
  overflow: hidden;
  position: relative;
}
```

即 `.el-tabs` 根节点本身是 flex 列容器，`.el-tabs__content` 是其中一个自带 `overflow: hidden` 的 flex 子项。按 CSS 规范，flex 子项的 `overflow` 非 `visible` 时，其自动最小尺寸（`min-height: auto`）解析为 0 而非内容高度。因此窗口不够高时：

1. `.el-tabs__content` 直接收缩到剩余空间，靠 `overflow: hidden` 裁剪表单；
2. 外层 `.api-mock-tabs` 的子元素永远恰好填满自身，永不溢出，现有的 `overflow: auto`（写在 tabs 根上）永远不会触发滚动条。

现状代码（`ApiMockPanel.vue` 中 `.api-mock-tabs` 规则）里那条「flex-basis 必须为 auto」的注释是上一次修复尝试的结论，层级判断有误：无论 basis 取值如何，收缩都发生在 `.el-tabs__content` 上，根上的 overflow 修复无效。

佐证：保存栏 sticky 的参照滚动容器正是 `overflow: hidden` 的 `.el-tabs__content`（非 visible 的 overflow 即构成 sticky 参照），因此保存栏贴底可见而中间内容被裁——与症状完全吻合。

## 3. 目标 / 非目标

目标：

- 窗口高度不足时，tab 内容区出现纵向滚动条，表单全部内容（含响应头、CORS 折叠区）可滚动到达；
- tab 头（「路由 / 请求日志」）保持固定不滚走；
- 保存栏继续吸底（滚动时始终可见），不引入额外处理；
- 「请求日志」tab 在日志较多时同样可滚动。

非目标：

- 不固定日志 tab 的工具栏（状态 + 清空按钮跟随内容滚动，与现状语义一致）;
- 不做全局 `element-overrides.css` 层面的 el-tabs 治理（各面板已各自处理，全局改动会造成双滚动条回归）;
- 不调整表单内部布局、Monaco 编辑器高度策略与面板三栏 grid 结构。

## 4. 方案

复用仓库既有模式（与 `ApiWorkbenchPanel.vue` 完全一致；`DbSqlWorkspace.vue`、`DbTableStructure.vue` 为同族变体，滚动下沉更深一层）：把滚动从 tabs 根下沉到 `.el-tabs__content`。

`ApiMockPanel.vue` 的 `<style scoped>` 中，将现有 `.api-mock-tabs` 规则替换为：

```css
.api-mock-tabs {
  /* EP 2.13 起 .el-tabs 自身是 flex 列容器；滚动收敛到 .el-tabs__content（见下） */
  flex: 1;
  min-height: 0;
  padding: 0 16px 16px;
}

.api-mock-tabs :deep(.el-tabs__content) {
  /* EP 默认 overflow:hidden 使内容区作为 flex 子项的自动最小高度为 0，
     窗口不够高时会收缩并裁剪长表单；改为在此层滚动，tab 头固定、保存栏吸底 */
  flex: 1;
  min-height: 0;
  overflow: auto;
}
```

要点：

- 根上删除失效的 `overflow: auto`，`flex: auto` 改为 `flex: 1`（与仓库其他 tabs 面板一致），避免双滚动条；
- 原「flex-basis 必须为 auto」注释一并删除，替换为指向真实根因的注释；
- 不动 `.el-tab-pane`（两个 tab 内容都是自然高度，在 content 层滚动即可）；
- 保存栏 sticky 的参照滚动容器仍是 `.el-tabs__content`，其从 hidden 变为 auto 后，吸底行为恰好从「贴住被裁剪盒子的底边」变为「贴住滚动视口底边」，即设计意图本身，无需改动表单组件。

### 备选方案（已否决）

- 滚动下沉到 `.el-tab-pane`（`content` 仅放开收缩，pane 设 `height: 100%; overflow: auto`）：效果相同但多一层规则，仅当各 tab 需要独立滚动位置记忆时有价值，本页不需要；
- 全局修改 `element-overrides.css`：影响全部使用 tabs 的面板（13 处），已各自处理的面板会出现双滚动条回归。

## 5. 验收标准

1. 压矮窗口后，「路由」tab 内容区出现纵向滚动条，可滚动到「响应头」区块、CORS 折叠区（含展开态）与表单底部；
2. 滚动过程中 tab 头固定不动，保存栏始终吸底可见；
3. Monaco 编辑器经 `resize: vertical` 拖大后，内容区仍可滚动到底部；
4. 「请求日志」tab 日志超出可视高度时同样出现滚动条；
5. 全程只有一条纵向滚动条（tabs 根上不再出现滚动条；Monaco 编辑器内部滚动条不计入）；
6. 窄屏（`max-width: 860px`）单列布局行为不变（该分支下 `.api-mock-panel` 行高随内容，content 不收缩，原本无此问题）。

## 6. 风险与权衡

- 滚动条出现在内容区右缘，位于面板 16px 内边距内侧（padding 保留在 tabs 根上），与整卡贴边滚动条相比略靠内，视觉可接受；如后续觉得突兀再单独调整 padding 归属，不在本次范围内。
- 纯 CSS 改动、作用域为 scoped + `:deep()` 局部选择器，不影响其他面板。

## 7. 验证

- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- 手动验证：按第 5 节验收标准逐项检查（压矮窗口、展开 CORS、拖大 Monaco、切日志 tab）。

纯样式改动，无单元测试要求。

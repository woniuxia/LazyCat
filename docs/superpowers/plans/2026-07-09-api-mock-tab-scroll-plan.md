# API Mock 面板 tab 内容区滚动修复实施计划

- 日期：2026-07-09
- 依据 spec：`docs/superpowers/specs/2026-07-09-api-mock-tab-scroll-design.md`（评审一轮通过，用户已确认）
- 影响范围：仅 `apps/desktop/src/components/ApiMockPanel.vue` 的 `<style scoped>` 块，纯 CSS，无逻辑改动
- 执行约定：严格按 spec 第 4 节的目标样式实施，不顺手调整其他样式规则

## 阶段 1：替换 `.api-mock-tabs` 样式块

定位 `ApiMockPanel.vue` 中现有规则（当前约 832-839 行）：

```css
.api-mock-tabs {
  /* flex-basis 必须为 auto：EP 的 .el-tabs__content 自带 overflow:hidden，
     basis 0 会让内容被 content 裁掉且根上永不出现滚动条 */
  flex: auto;
  min-height: 0;
  padding: 0 16px 16px;
  overflow: auto;
}
```

整块替换为：

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

不动模板、脚本、`.el-tab-pane` 及其他任何样式规则。

## 阶段 2：验证

1. `pnpm typecheck`
2. `pnpm --filter @lazycat/desktop build:web`
3. 手动验收（需运行应用，按 spec 第 5 节 6 条逐项检查）：
   - 压矮窗口，「路由」tab 出现纵向滚动条，可滚到「响应头」与 CORS 折叠区（含展开态）及表单底部；
   - tab 头固定不滚走，保存栏始终吸底；
   - Monaco 编辑器拖大（`resize: vertical`）后仍可滚到底；
   - 「请求日志」tab 日志超高时可滚动；
   - 全程仅一条纵向滚动条（Monaco 内部滚动条不计入）；
   - 窄屏（`max-width: 860px`）单列布局行为不变。

## 提交与收尾

- **提交策略**：当前 worktree 中 `ApiMockPanel.vue` 已包含进行中的 api-mock UX 改动（未提交），本修复无法作为干净的独立提交拆分；修改保留在工作区，随本轮 api-mock 工作一并提交，或按用户指示单独处理。
- 单文件改动，未达 `process.md` 记录阈值（3+ 文件），不记录；根因结论已沉淀在 spec 文档中。
- 不自动启动 dev server（项目 07.1）；手动验收由用户执行，或经用户同意后代跑。

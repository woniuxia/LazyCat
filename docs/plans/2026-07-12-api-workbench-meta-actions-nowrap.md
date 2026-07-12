# API Workbench Meta Actions No-Wrap Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 修复 API Workbench 在中窄窗口下环境、配置、保存操作错位换行的问题。

**Architecture:** 只调整 `ApiWorkbenchPanel.vue` 的局部 Flex CSS。操作组禁止换行，环境选择器覆盖通用 `flex: none` 并承担收缩；删除仅在 `1180px` 以下生效的重复覆盖，使修复覆盖三栏布局中的 `1181px–1380px` 区间。

**Tech Stack:** Vue 3、Element Plus、Scoped CSS

---

### Task 1: 修正元信息操作组的 Flex 收缩行为

**Files:**
- Modify: `apps/desktop/src/components/ApiWorkbenchPanel.vue:2076-2085`
- Modify: `apps/desktop/src/components/ApiWorkbenchPanel.vue:2358-2366`
- Modify: `apps/desktop/src/components/ApiWorkbenchPanel.vue:2668-2682`

**Step 1: 确认修改前根因**

Run:

```powershell
rg -n "api-workbench-primary-actions|environment-select|meta-environment-select" apps/desktop/src/components/ApiWorkbenchPanel.vue
```

Expected: 操作组继承 `flex-wrap: wrap`；环境选择器默认 `flex: none` 和 `width: 180px`；可收缩覆盖仅存在于 `max-width: 1180px`。

**Step 2: 实施最小 CSS 修复**

将操作组从共享换行规则中拆出：

```css
.api-workbench-primary-actions {
  display: flex;
  min-width: 0;
  flex-wrap: nowrap;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}
```

保留 `.curl-actions` 既有可换行行为。将位于 `.environment-select` 规则之后的 `.meta-environment-select` 改为：

```css
.meta-environment-select {
  width: auto;
  min-width: 0;
  flex: 1 1 180px;
}
```

删除 `max-width: 1180px` 中重复的 `.meta-environment-select` 块；保留该断点下操作组 `justify-content: flex-start`。

**Step 3: 检查差异**

Run:

```powershell
git diff --check
git diff -- apps/desktop/src/components/ApiWorkbenchPanel.vue
```

Expected: 只包含上述局部 CSS 调整，无模板、业务逻辑或其他文件变化。

**Step 4: 执行类型检查**

Run: `pnpm typecheck`

Expected: PASS。

**Step 5: 执行渲染层构建**

Run: `pnpm --filter @lazycat/desktop build:web`

Expected: PASS；允许既有 chunk-size warning。

**Step 6: 布局验证**

检查编辑区约 `420px` 宽度，以及窗口宽度 `1380px`、`1181px`、`1180px`、`820px`、`375px`：

- 环境、配置、保存始终同一行。
- 环境选择器优先收缩，内部文本不撑开 Flex Item。
- 配置和保存按钮尺寸不变。
- 元信息区无横向滚动。

若无法启动产品 UI，报告必须明确区分静态 CSS 检查与运行时视觉验证。

**Step 7: 提交**

```powershell
git add apps/desktop/src/components/ApiWorkbenchPanel.vue
git commit -m "fix(api-workbench): 修正窄屏元信息操作错位"
```

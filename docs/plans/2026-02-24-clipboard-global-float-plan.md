# 剪贴板提示条全局浮动改造 - 实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 ClipboardSuggestionBar 从 main 视图内联位置提升为全局 fixed 浮动通知，使其在所有 viewMode（含 workspace）下均可显示。

**Architecture:** 模板层将组件移到 v-if 链之前（Vue 3 多根节点），样式层改为 `position: fixed` 顶部居中。composable 和交互逻辑不变。

**Tech Stack:** Vue 3, TypeScript, CSS

---

### Task 1: 移动模板位置 (App.vue)

**Files:**

- Modify: `apps/desktop/src/App.vue:1-67`（template 区域）

**Step 1: 从 `<main class="content">` 中删除 ClipboardSuggestionBar**

在 `apps/desktop/src/App.vue` 第 43 行，删除：

```html
<ClipboardSuggestionBar @open-tool="onClipboardToolOpen" />
```

**Step 2: 在模板根层最前面插入 ClipboardSuggestionBar**

在 `<template>` 的第一个子元素位置（第 2 行之前）插入：

```html
<!-- 全局浮动：剪贴板智能提示（脱离 viewMode 分支，所有视图可见） -->
<ClipboardSuggestionBar @open-tool="onClipboardToolOpen" />
```

改动后模板开头结构：

```html
<template>
  <!-- 全局浮动：剪贴板智能提示（脱离 viewMode 分支，所有视图可见） -->
  <ClipboardSuggestionBar @open-tool="onClipboardToolOpen" />

  <div v-if="viewMode === 'main'" class="shell" ...>...</div>
  <div v-else-if="viewMode === 'snippet-workspace'" ...>...</div></template
>
```

---

### Task 2: 样式改为 fixed 浮动 (ClipboardSuggestionBar.vue)

**Files:**

- Modify: `apps/desktop/src/components/ClipboardSuggestionBar.vue:126-374`（style 区域）

**Step 1: 修改 `.cb-strip` 容器样式**

将 `.cb-strip` 从内联流式改为 fixed 浮动居中：

```css
.cb-strip {
  position: fixed;
  top: 12px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 9000;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 14px 0 0;
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-2);
  border: 1px solid var(--lc-border-hover);
  overflow: hidden;
  box-shadow:
    0 4px 24px rgba(0, 0, 0, 0.25),
    0 0 24px rgba(56, 189, 248, 0.08);
  flex-shrink: 0;
  min-height: 44px;
  max-width: 600px;
  width: max-content;
}
```

关键变化：

- 删除 `position: relative` → `position: fixed`
- 新增 `top: 12px; left: 50%; transform: translateX(-50%);`
- 新增 `z-index: 9000`
- 新增 `max-width: 600px; width: max-content;`
- 删除 `margin-bottom: 16px`（原本就没有在组件内，是外部隐含的，确认无残留）
- 增强 `box-shadow`

**Step 2: 调整过渡动画方向**

将 enter/leave 动画从 `translateY(-8px)` 改为兼容 fixed + `translateX(-50%)` 的写法：

```css
.cb-pill-enter-from {
  opacity: 0;
  transform: translateX(-50%) translateY(-12px);
  max-height: 0;
}

.cb-pill-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(-8px);
  max-height: 0;
}
```

注意：因为正常状态的 `transform` 是 `translateX(-50%)`，动画状态必须保留这个基础偏移，否则进出时会水平跳动。

**Step 3: 调整亮色主题覆盖**

亮色主题的 `.cb-strip` 阴影也需要增强：

```css
[data-theme="light"] .cb-strip {
  background: #f0f7ff;
  border-color: rgba(56, 189, 248, 0.2);
  box-shadow:
    0 4px 24px rgba(0, 0, 0, 0.12),
    0 0 16px rgba(56, 189, 248, 0.06);
}
```

---

### Task 3: 验证

**Step 1: 类型检查**

```bash
pnpm --filter @lazycat/desktop typecheck
```

Expected: 无错误

**Step 2: 构建验证**

```bash
pnpm --filter @lazycat/desktop build:web
```

Expected: 构建成功

---

### Task 4: 提交

```bash
git add apps/desktop/src/App.vue apps/desktop/src/components/ClipboardSuggestionBar.vue
git commit -m "fix(clipboard): 剪贴板提示条改为全局浮动，兼容 workspace 视图"
```

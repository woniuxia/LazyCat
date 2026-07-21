# PM Detail Copy Title Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让项目管理工作项详情中的编号与标题保持一致字形，并支持直接点击整行复制“编号 + 标题”。

**Architecture:** 保持改动局限在 `PmDetailPanel.vue`，将现有标题容器改为语义化无边框按钮，并在组件内完成复制文本组装、剪贴板写入和消息反馈。测试沿用仓库现有的 Vue 源码结构断言模式，先验证交互契约失败，再写最小实现。

**Tech Stack:** Vue 3、TypeScript、Element Plus、Vitest、CSS scoped styles

---

## File Structure

- Create: `apps/desktop/src/components/pm/PmDetailPanel.copy.test.ts` - 固化标题复制控件、复制文本格式、反馈文案和编号继承样式的契约。
- Modify: `apps/desktop/src/components/pm/PmDetailPanel.vue` - 提供标题点击复制行为并统一编号与标题样式。

### Task 1: 标题复制交互

**Files:**
- Create: `apps/desktop/src/components/pm/PmDetailPanel.copy.test.ts`
- Modify: `apps/desktop/src/components/pm/PmDetailPanel.vue`

- [ ] **Step 1: 写入失败测试**

创建 `apps/desktop/src/components/pm/PmDetailPanel.copy.test.ts`：

```ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./PmDetailPanel.vue", import.meta.url), "utf-8");

describe("PmDetailPanel title copy interaction", () => {
  it("uses the whole title as an accessible copy control", () => {
    expect(source).toContain('class="detail-item-title"');
    expect(source).toContain('type="button"');
    expect(source).toContain('title="点击复制编号和标题"');
    expect(source).toContain('aria-label="复制编号和标题"');
    expect(source).toContain('@click="copyItemTitle"');
  });

  it("copies the trimmed reference code and title with explicit feedback", () => {
    expect(source).toContain(
      'const copyText = [item.refCode?.trim(), item.title.trim()].filter(Boolean).join(" ");',
    );
    expect(source).toContain("await navigator.clipboard.writeText(copyText);");
    expect(source).toContain('ElMessage.success("已复制编号和标题");');
    expect(source).toContain('ElMessage.error("复制失败");');
  });

  it("lets the reference code inherit the title typography", () => {
    const refCodeStyle = source.match(/\.detail-ref-code\s*\{([^}]*)\}/s)?.[1] ?? "";

    expect(refCodeStyle).not.toMatch(/font-size|font-weight|font-family|color/);
    expect(refCodeStyle).toContain("margin-right: 6px;");
  });
});
```

- [ ] **Step 2: 运行测试并确认按预期失败**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/components/pm/PmDetailPanel.copy.test.ts
```

Expected: FAIL，至少报告缺少 `type="button"`、`@click="copyItemTitle"` 或复制逻辑；失败原因必须是功能尚未实现，而不是测试语法错误。

- [ ] **Step 3: 写入最小模板和复制逻辑**

将 `PmDetailPanel.vue` 中现有标题容器替换为：

```vue
<button
  type="button"
  class="detail-item-title"
  title="点击复制编号和标题"
  aria-label="复制编号和标题"
  @click="copyItemTitle"
>
  <span v-if="item.refCode" class="detail-ref-code">{{ item.refCode }}</span>
  {{ item.title }}
</button>
```

在 `openItemLink` 前增加：

```ts
async function copyItemTitle(): Promise<void> {
  const item = props.item;
  if (!item) return;

  const copyText = [item.refCode?.trim(), item.title.trim()].filter(Boolean).join(" ");
  try {
    await navigator.clipboard.writeText(copyText);
    ElMessage.success("已复制编号和标题");
  } catch {
    ElMessage.error("复制失败");
  }
}
```

- [ ] **Step 4: 统一标题控件和编号样式**

将后置的 `.detail-item-title` 样式补全为无边框文本按钮，并移除 `.detail-ref-code` 中独立的字号、字重、颜色和字体设置：

```css
.detail-item-title {
  width: 100%;
  margin: 0;
  padding: 0;
  border: 0;
  appearance: none;
  background: transparent;
  text-align: left;
  font-family: inherit;
  font-size: 18px;
  font-weight: 700;
  line-height: 1.4;
  color: var(--pm-text-main);
  cursor: pointer;
  transition: color 0.15s ease, opacity 0.15s ease;
}

.detail-item-title:hover {
  color: var(--el-color-primary);
}

.detail-item-title:focus-visible {
  outline: 2px solid var(--el-color-primary-light-5);
  outline-offset: 3px;
  border-radius: 4px;
}

.detail-item-title:active {
  opacity: 0.72;
}

.detail-ref-code {
  margin-right: 6px;
}
```

- [ ] **Step 5: 运行针对性测试并确认通过**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/components/pm/PmDetailPanel.copy.test.ts
```

Expected: PASS，3 个测试全部通过且无错误输出。

- [ ] **Step 6: 运行类型、完整单测和渲染层构建验证**

Run:

```powershell
pnpm typecheck
pnpm test
pnpm --filter @lazycat/desktop build:web
```

Expected: 三条命令退出码均为 0；类型检查无错误，测试全部通过，Vite 构建完成。

- [ ] **Step 7: 提交实现**

```powershell
git add "apps/desktop/src/components/pm/PmDetailPanel.vue" "apps/desktop/src/components/pm/PmDetailPanel.copy.test.ts" "docs/superpowers/plans/2026-07-21-pm-detail-copy-title.md"
git commit -m "feat(pm): 支持点击复制工作项编号和标题"
```

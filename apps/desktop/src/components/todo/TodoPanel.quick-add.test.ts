import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./TodoPanel.vue", import.meta.url), "utf-8");

describe("TodoPanel quick add wiring", () => {
  it("mounts the quick add bar only in list view, outside the scroll container", () => {
    const barIndex = source.indexOf("<TodoQuickAddBar");
    const scrollIndex = source.indexOf('class="todo-list-scroll"');
    expect(barIndex).toBeGreaterThan(-1);
    expect(barIndex).toBeLessThan(scrollIndex);
    expect(source).toMatch(
      /<TodoQuickAddBar\s+v-if="viewMode === 'list'"[\s\S]*?:context="quickAddContext"[\s\S]*?@created="onQuickAddCreated"/,
    );
  });

  it("derives the quick add context from current filters", () => {
    // 分类名解析不到 id（如“未分类”）时降级为 null
    expect(source).toMatch(
      /typeId:\s*filterType\.value === null\s*\?\s*null\s*:\s*\(?types\.value\.find\(\(t\) => t\.name === filterType\.value\)\?\.id\s*\?\?\s*null\)?/,
    );
    // 仅具体项目 id 才继承，"none"/null 不继承
    expect(source).toContain(
      'projectId: typeof filterProjectId.value === "number" ? filterProjectId.value : null',
    );
    expect(source).toContain('priorityDefault: filterPriority.value ?? "P2"');
  });

  it("judges visibility against the final rendered list after reload", () => {
    expect(source).toMatch(
      /async function onQuickAddCreated\(id: number\)\s*\{\s*await loadItems\(\);/,
    );
    expect(source).toMatch(
      /function isItemVisibleInList\(id: number\)\s*\{\s*return displayActiveItems\.value\.some\(\(row\) => row\.id === id\);/,
    );
  });

  it("shows an info toast instead of highlight when the new item is filtered out", () => {
    expect(source).toMatch(
      /if\s*\(!isItemVisibleInList\(id\)\)\s*\{\s*ElMessage\.info\("已添加，当前筛选\/搜索条件下不可见"\);\s*return;/,
    );
  });

  it("highlights the visible new item for 1.5s and lets rapid entries replace the timer", () => {
    expect(source).toContain("quickAddHighlightId.value = id;");
    expect(source).toMatch(
      /if\s*\(quickAddHighlightTimer\)\s*clearTimeout\(quickAddHighlightTimer\);/,
    );
    expect(source).toMatch(/quickAddHighlightTimer = setTimeout\([\s\S]*?, 1500\);/);
    expect(source).toContain("'is-quick-add-highlight': quickAddHighlightId === row.id,");
    expect(source).toContain(".todo-card.is-quick-add-highlight");
  });

  it("cleans up the highlight timer on unmount", () => {
    expect(source).toMatch(
      /onBeforeUnmount\(\(\) => \{[\s\S]*?if \(quickAddHighlightTimer\) \{\s*clearTimeout\(quickAddHighlightTimer\);\s*quickAddHighlightTimer = null;\s*\}[\s\S]*?\}\);/,
    );
  });
});

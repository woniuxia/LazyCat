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

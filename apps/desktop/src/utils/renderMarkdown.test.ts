import { describe, expect, it } from "vitest";
import { renderMarkdown } from "./renderMarkdown";

describe("renderMarkdown", () => {
  it("renders GFM tables and task lists", () => {
    const html = renderMarkdown("| A | B |\n| - | - |\n| 1 | 2 |\n\n- [x] 完成");
    expect(html).toContain("<table>");
    expect(html).toContain('type="checkbox"');
    expect(html).toContain("完成");
  });

  it("highlights fenced code with a language class", () => {
    const html = renderMarkdown("```js\nconst answer = 42;\n```");
    expect(html).toContain('class="language-js"');
    expect(html).toContain("hljs-keyword");
  });

  it("does not execute raw html or unsafe links", () => {
    const html = renderMarkdown('<img src=x onerror="alert(1)">\n\n[x](javascript:alert(1))');
    expect(html).not.toContain("<img");
    expect(html).not.toContain('href="javascript:');
    expect(html).toContain("&lt;img");
  });
});

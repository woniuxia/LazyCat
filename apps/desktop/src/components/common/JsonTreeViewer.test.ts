import { existsSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./JsonTreeViewer.vue", import.meta.url), "utf8");
const nodePath = new URL("./JsonTreeNode.vue", import.meta.url);
const nodeSource = readFileSync(nodePath, "utf8");

describe("JsonTreeViewer source structure", () => {
  it("keeps JSON traversal in shared pure utilities", () => {
    expect(source).toContain("buildJsonTree");
    expect(source).toContain("collectExpandableKeys");
    expect(source).toContain("collectExpandedKeysByDepth");
    expect(source).toContain("formatJsonForCopy");
  });

  it("renders the expected toolbar actions and copy feedback", () => {
    expect(source).toContain("复制");
    expect(source).toContain("展开全部");
    expect(source).toContain("折叠全部");
    expect(source).toContain("折到 2 层");
    expect(source).toContain('ElMessage.success("已复制")');
    expect(source).toContain('ElMessage.error("复制失败")');
  });

  it("rebuilds expansion state from value and default depth changes", () => {
    expect(source).toContain("watch(");
    expect(source).toContain("defaultExpandDepth");
    expect(source).toContain("expandedKeys.value =");
  });

  it("renders tree rows through an SFC so scoped styles apply to node markup", () => {
    expect(existsSync(nodePath)).toBe(true);
    expect(source).toContain('import JsonTreeNode from "./JsonTreeNode.vue"');
    expect(source).toContain("<JsonTreeNode");
    expect(source).not.toContain("defineComponent");
    expect(source).not.toContain('h("div"');
  });

  it("keeps object and array metadata after the bracket token", () => {
    const bracketIndex = nodeSource.indexOf('class="json-tree-bracket"');
    const summaryIndex = nodeSource.indexOf('class="json-tree-summary"');

    expect(bracketIndex).toBeGreaterThan(-1);
    expect(summaryIndex).toBeGreaterThan(-1);
    expect(bracketIndex).toBeLessThan(summaryIndex);
  });
});

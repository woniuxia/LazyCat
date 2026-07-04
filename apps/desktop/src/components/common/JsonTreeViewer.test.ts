import { existsSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./JsonTreeViewer.vue", import.meta.url), "utf8");
const nodePath = new URL("./JsonTreeNode.vue", import.meta.url);
const nodeSource = readFileSync(nodePath, "utf8");
const menuPath = new URL("./JsonTreeNodeMenu.vue", import.meta.url);
const menuSource = readFileSync(menuPath, "utf8");

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

  it("wires the toolbar search region behind the showSearch prop", () => {
    expect(source).toContain("showSearch?: boolean");
    expect(source).toContain("showSearch: true");
    expect(source).toContain('class="json-tree-search"');
    expect(source).toContain("useJsonTreeSearch");
    expect(source).toContain("无匹配");
    expect(source).toContain("@keydown.enter.exact.prevent");
    expect(source).toContain("@keydown.shift.enter.prevent");
    expect(source).toContain("上一处");
    expect(source).toContain("下一处");
  });

  it("reveals ancestors and scrolls to the active match row", () => {
    expect(source).toContain("revealKeys");
    expect(source).toContain("scrollIntoView");
    expect(source).toContain(':matched-keys="searchMatchedIds"');
    expect(source).toContain(':active-match-key="searchActiveMatchId"');
    expect(nodeSource).toContain(':data-key="node.key"');
  });

  it("highlights matched labels and values with dedicated classes", () => {
    expect(nodeSource).toContain("matchedKeys");
    expect(nodeSource).toContain("activeMatchKey");
    expect(nodeSource).toContain("json-tree-match");
    expect(nodeSource).toContain("json-tree-match-active");
    expect(nodeSource).toContain('jsonTreeSearchMatchId({ field: "key"');
    expect(nodeSource).toContain('jsonTreeSearchMatchId({ field: "value"');
  });

  it("opens one shared node menu from both context click and the row more button", () => {
    expect(existsSync(menuPath)).toBe(true);
    expect(nodeSource).toContain("@contextmenu.prevent.stop");
    expect(nodeSource).toContain("json-tree-more");
    expect(nodeSource).toContain("open-menu");
    expect(source).toContain('import JsonTreeNodeMenu from "./JsonTreeNodeMenu.vue"');
    expect(source).toContain("@open-menu=");
  });

  it("copies JSONPath and formatted subtree values from the node menu", () => {
    expect(menuSource).toContain("复制路径");
    expect(menuSource).toContain("复制值");
    expect(source).toContain("toJsonPath(node.path)");
    expect(source).toContain("formatJsonForCopy(node.value)");
  });

  it("teleports the menu to body with global styles and closes it on document change", () => {
    expect(menuSource).toContain('<Teleport to="body">');
    expect(menuSource).toContain("clampContextMenuPosition");
    expect(menuSource).not.toContain("<style scoped>");
    expect(source).toContain("closeNodeMenu");
    expect(source).toMatch(/watch\(tree, \(\) => \{\s*if \(menuVisible\.value\) closeNodeMenu\(\);/);
  });
});

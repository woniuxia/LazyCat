import type { JsonTreeNode, JsonTreePath } from "./jsonTreeView";
import { encodeJsonTreePath, formatJsonPrimitive } from "./jsonTreeView";

export interface JsonTreeSearchMatch {
  key: string;
  path: JsonTreePath;
  field: "key" | "value";
}

/** 命中标识:区分同一节点上 key 命中与 value 命中,供高亮与导航定位使用。 */
export function jsonTreeSearchMatchId(match: Pick<JsonTreeSearchMatch, "field" | "key">): string {
  return `${match.field}:${match.key}`;
}

export function collectJsonTreeSearchMatches(
  root: JsonTreeNode,
  query: string,
): JsonTreeSearchMatch[] {
  if (!query) return [];

  const needle = query.toLowerCase();
  const matches: JsonTreeSearchMatch[] = [];
  const visit = (node: JsonTreeNode) => {
    const isContainer = node.valueType === "object" || node.valueType === "array";
    if (node.label.toLowerCase().includes(needle)) {
      matches.push({ key: node.key, path: node.path, field: "key" });
    }
    if (!isContainer) {
      const valueText = formatJsonPrimitive(node.value, node.valueType);
      if (valueText.toLowerCase().includes(needle)) {
        matches.push({ key: node.key, path: node.path, field: "value" });
      }
    }
    for (const child of node.children) visit(child);
  };
  visit(root);
  return matches;
}

export function collectJsonTreeAncestorKeys(path: JsonTreePath): string[] {
  const keys: string[] = [];
  for (let length = 0; length < path.length; length += 1) {
    keys.push(encodeJsonTreePath(path.slice(0, length)));
  }
  return keys;
}

import type { JsonTreeNode } from "../utils/jsonTreeView";

/** 节点菜单动作(Phase 1 只读集;编辑动作在 Phase 2 扩展)。 */
export type JsonTreeNodeMenuAction = { kind: "copy-path" } | { kind: "copy-value" };

/** JsonTreeNode 行请求打开菜单时携带的目标与锚点。 */
export interface JsonTreeNodeMenuTarget {
  node: JsonTreeNode;
  x: number;
  y: number;
}

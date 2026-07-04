import type { JsonTreeNode } from "../utils/jsonTreeView";
import type { JsonTreeSwitchableType } from "../utils/jsonTreeEdit";
import type { JsonTreeEditingMode } from "../composables/useJsonTreeEditing";

/** 节点菜单动作:只读集(复制)+ 编辑集(editable 时)。 */
export type JsonTreeNodeMenuAction =
  | { kind: "copy-path" }
  | { kind: "copy-value" }
  | { kind: "edit-value" }
  | { kind: "rename-key" }
  | { kind: "add-child" }
  | { kind: "insert-before" }
  | { kind: "insert-after" }
  | { kind: "switch-type"; valueType: JsonTreeSwitchableType }
  | { kind: "move-up" }
  | { kind: "move-down" }
  | { kind: "remove" };

/** JsonTreeNode 行请求打开菜单时携带的目标与锚点。 */
export interface JsonTreeNodeMenuTarget {
  node: JsonTreeNode;
  x: number;
  y: number;
}

/** 双击等入口发起的行内编辑请求。 */
export interface JsonTreeNodeEditRequest {
  node: JsonTreeNode;
  mode: "value" | "rename";
}

/** 行内编辑提交:text 由组件层做宽松解析/重命名校验。 */
export interface JsonTreeNodeEditSubmit {
  node: JsonTreeNode;
  mode: JsonTreeEditingMode;
  text: string;
}

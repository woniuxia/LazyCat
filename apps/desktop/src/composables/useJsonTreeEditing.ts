import { computed, ref, shallowRef, toRaw } from "vue";
import type { Ref } from "vue";
import { ElMessage } from "element-plus";
import { applyJsonTreeEdit, migrateExpandedKeys } from "../utils/jsonTreeEdit";
import type { JsonTreeEditOp } from "../utils/jsonTreeEdit";

export const JSON_TREE_EDIT_HISTORY_LIMIT = 100;

export type JsonTreeEditingMode = "value" | "rename" | "insert-key";

export interface JsonTreeEditingState {
  key: string;
  mode: JsonTreeEditingMode;
}

interface JsonTreeEditSnapshot {
  value: unknown;
  expandedKeys: Set<string>;
}

export interface UseJsonTreeEditingOptions {
  /** 当前文档(props.value),内部一律经 toRaw 归一。 */
  getValue: () => unknown;
  /** 组件持有的展开集合;op 成功时迁移,undo/redo 恢复快照。 */
  expandedKeys: Ref<Set<string>>;
  /** 编辑成功后上抛新根(update:value)。 */
  emitValue: (value: unknown) => void;
  /** 外部换文档时通知组件重置展开、清空搜索、取消进行中交互。 */
  onExternalDocument?: () => void;
}

/**
 * JsonTreeViewer 编辑状态机:快照式 undo/redo(值 + 展开集合)、
 * toRaw 回流判定、进行中行内编辑态管理。
 */
export function useJsonTreeEditing(options: UseJsonTreeEditingOptions) {
  const past = shallowRef<JsonTreeEditSnapshot[]>([]);
  const future = shallowRef<JsonTreeEditSnapshot[]>([]);
  const editing = ref<JsonTreeEditingState | null>(null);

  // 最近一次 emit 的原始根引用;null 表示尚未 emit 过(首个 value 视为外部文档)
  let lastEmittedRaw: unknown = null;
  let hasEmitted = false;

  const canUndo = computed(() => past.value.length > 0);
  const canRedo = computed(() => future.value.length > 0);

  function normalizeRaw(value: unknown): unknown {
    return typeof value === "object" && value !== null ? toRaw(value) : value;
  }

  function takeSnapshot(): JsonTreeEditSnapshot {
    return {
      value: normalizeRaw(options.getValue()),
      expandedKeys: new Set(options.expandedKeys.value),
    };
  }

  function restoreSnapshot(snapshot: JsonTreeEditSnapshot) {
    options.expandedKeys.value = new Set(snapshot.expandedKeys);
    lastEmittedRaw = snapshot.value;
    hasEmitted = true;
    options.emitValue(snapshot.value);
  }

  function pushPast(snapshot: JsonTreeEditSnapshot) {
    const stack = [...past.value, snapshot];
    if (stack.length > JSON_TREE_EDIT_HISTORY_LIMIT) stack.shift();
    past.value = stack;
  }

  function applyOp(op: JsonTreeEditOp): boolean {
    const current = normalizeRaw(options.getValue());
    const result = applyJsonTreeEdit(current, op);
    if (!result.ok) {
      ElMessage.error(result.reason);
      return false;
    }
    pushPast({ value: current, expandedKeys: new Set(options.expandedKeys.value) });
    future.value = [];
    options.expandedKeys.value = migrateExpandedKeys(options.expandedKeys.value, op);
    lastEmittedRaw = result.value;
    hasEmitted = true;
    options.emitValue(result.value);
    return true;
  }

  function undo() {
    const stack = past.value;
    if (!stack.length) return;
    const snapshot = stack[stack.length - 1];
    past.value = stack.slice(0, -1);
    future.value = [...future.value, takeSnapshot()];
    restoreSnapshot(snapshot);
  }

  function redo() {
    const stack = future.value;
    if (!stack.length) return;
    const snapshot = stack[stack.length - 1];
    future.value = stack.slice(0, -1);
    pushPast(takeSnapshot());
    restoreSnapshot(snapshot);
  }

  function onValueChange(newValue: unknown) {
    const raw = normalizeRaw(newValue);
    if (hasEmitted && raw === lastEmittedRaw) return;
    // 外部换文档:清双栈、取消进行中编辑态(不回滚,旧文档已整体被替换)
    past.value = [];
    future.value = [];
    editing.value = null;
    lastEmittedRaw = raw;
    hasEmitted = true;
    options.onExternalDocument?.();
  }

  function beginEdit(key: string, mode: JsonTreeEditingMode) {
    editing.value = { key, mode };
  }

  function endEdit() {
    editing.value = null;
  }

  /**
   * 取消进行中编辑:空 key 插入弹出 past 栈顶恢复(被弃状态不入 future);
   * 其余模式仅退出编辑态。
   */
  function cancelEditing() {
    const state = editing.value;
    editing.value = null;
    if (!state || state.mode !== "insert-key") return;
    const stack = past.value;
    if (!stack.length) return;
    const snapshot = stack[stack.length - 1];
    past.value = stack.slice(0, -1);
    restoreSnapshot(snapshot);
  }

  return {
    editing,
    canUndo,
    canRedo,
    applyOp,
    undo,
    redo,
    beginEdit,
    endEdit,
    cancelEditing,
    onValueChange,
  };
}

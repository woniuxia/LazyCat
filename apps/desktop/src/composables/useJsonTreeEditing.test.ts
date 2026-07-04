import { describe, expect, it, vi } from "vitest";
import { ref, shallowRef } from "vue";
import type { Ref } from "vue";

vi.mock("element-plus", () => ({
  ElMessage: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

import { ElMessage } from "element-plus";
import { encodeJsonTreePath } from "../utils/jsonTreeView";
import {
  JSON_TREE_EDIT_HISTORY_LIMIT,
  useJsonTreeEditing,
} from "./useJsonTreeEditing";

const enc = encodeJsonTreePath;

function createHarness(initial: unknown, options: { deepReactive?: boolean } = {}) {
  const doc = options.deepReactive ? ref(initial) : shallowRef(initial);
  const expandedKeys: Ref<Set<string>> = ref(new Set<string>());
  const onExternalDocument = vi.fn();
  const editing = useJsonTreeEditing({
    getValue: () => doc.value,
    expandedKeys,
    emitValue: (value) => {
      doc.value = value;
    },
    onExternalDocument,
  });
  /** 模拟受控回路:父组件回写后 watch 触发 onValueChange。 */
  function reflow() {
    editing.onValueChange(doc.value);
  }
  return { doc, expandedKeys, editing, onExternalDocument, reflow };
}

describe("useJsonTreeEditing applyOp", () => {
  it("emits the new root and records history on success", () => {
    const { doc, editing } = createHarness({ a: 1 });

    const ok = editing.applyOp({ type: "set-value", path: ["a"], value: 2 });

    expect(ok).toBe(true);
    expect(doc.value).toEqual({ a: 2 });
    expect(editing.canUndo.value).toBe(true);
    expect(editing.canRedo.value).toBe(false);
  });

  it("toasts and keeps stacks and document untouched on failure", () => {
    const { doc, editing } = createHarness({ a: 1 });

    const ok = editing.applyOp({ type: "remove", path: [] });

    expect(ok).toBe(false);
    expect(ElMessage.error).toHaveBeenCalled();
    expect(doc.value).toEqual({ a: 1 });
    expect(editing.canUndo.value).toBe(false);
  });

  it("migrates expanded keys when an op shifts paths", () => {
    const { expandedKeys, editing } = createHarness({ list: [[1], [2]] });
    expandedKeys.value = new Set([enc([]), enc(["list"]), enc(["list", 1])]);

    editing.applyOp({ type: "insert", parentPath: ["list"], index: 0, value: null });

    expect(expandedKeys.value).toEqual(new Set([enc([]), enc(["list"]), enc(["list", 2])]));
  });
});

describe("useJsonTreeEditing undo/redo", () => {
  it("restores both value and expanded keys", () => {
    const { doc, expandedKeys, editing, reflow } = createHarness({ list: [1] });
    expandedKeys.value = new Set([enc([]), enc(["list"])]);

    editing.applyOp({ type: "remove", path: ["list"] });
    reflow();
    expect(doc.value).toEqual({});
    expect(expandedKeys.value).toEqual(new Set([enc([])]));

    editing.undo();
    reflow();
    expect(doc.value).toEqual({ list: [1] });
    expect(expandedKeys.value).toEqual(new Set([enc([]), enc(["list"])]));
    expect(editing.canRedo.value).toBe(true);

    editing.redo();
    reflow();
    expect(doc.value).toEqual({});
    expect(expandedKeys.value).toEqual(new Set([enc([])]));
  });

  it("clears redo history after a new edit", () => {
    const { editing, reflow } = createHarness({ a: 1 });

    editing.applyOp({ type: "set-value", path: ["a"], value: 2 });
    reflow();
    editing.undo();
    reflow();
    expect(editing.canRedo.value).toBe(true);

    editing.applyOp({ type: "set-value", path: ["a"], value: 3 });
    reflow();
    expect(editing.canRedo.value).toBe(false);
  });

  it("drops the oldest snapshot beyond the history limit", () => {
    const { doc, editing, reflow } = createHarness({ n: 0 });

    for (let i = 1; i <= JSON_TREE_EDIT_HISTORY_LIMIT + 5; i += 1) {
      editing.applyOp({ type: "set-value", path: ["n"], value: i });
      reflow();
    }

    let undoCount = 0;
    while (editing.canUndo.value) {
      editing.undo();
      reflow();
      undoCount += 1;
    }
    expect(undoCount).toBe(JSON_TREE_EDIT_HISTORY_LIMIT);
    expect(doc.value).toEqual({ n: 5 });
  });
});

describe("useJsonTreeEditing onValueChange", () => {
  it("treats a deep-reactive write-back as edit reflow via toRaw comparison", () => {
    const { editing, reflow, onExternalDocument } = createHarness({ a: 1 }, { deepReactive: true });

    editing.applyOp({ type: "set-value", path: ["a"], value: 2 });
    reflow();

    expect(editing.canUndo.value).toBe(true);
    expect(onExternalDocument).not.toHaveBeenCalled();
  });

  it("clears stacks, cancels editing, and notifies on an external document", () => {
    const { doc, editing, onExternalDocument } = createHarness({ a: 1 });

    editing.applyOp({ type: "set-value", path: ["a"], value: 2 });
    editing.onValueChange(doc.value);
    editing.beginEdit(enc(["a"]), "value");

    doc.value = { brand: "new" };
    editing.onValueChange(doc.value);

    expect(editing.canUndo.value).toBe(false);
    expect(editing.canRedo.value).toBe(false);
    expect(editing.editing.value).toBeNull();
    expect(onExternalDocument).toHaveBeenCalledTimes(1);
  });

  it("does not roll back an insert cancelled by an external document change", () => {
    const { doc, editing } = createHarness({});

    editing.applyOp({ type: "insert", parentPath: [], key: "", value: null });
    editing.onValueChange(doc.value);
    editing.beginEdit(enc([""]), "insert-key");

    doc.value = { brand: "new" };
    editing.onValueChange(doc.value);
    editing.cancelEditing();

    expect(doc.value).toEqual({ brand: "new" });
    expect(editing.canUndo.value).toBe(false);
  });
});

describe("useJsonTreeEditing editing state", () => {
  it("rolls back an empty-key insert on cancel without creating a redo entry", () => {
    const { doc, expandedKeys, editing, reflow } = createHarness({ wrap: {} });
    expandedKeys.value = new Set([enc([]), enc(["wrap"])]);

    editing.applyOp({ type: "insert", parentPath: ["wrap"], key: "", value: null });
    reflow();
    editing.beginEdit(enc(["wrap", ""]), "insert-key");
    editing.cancelEditing();
    reflow();

    expect(doc.value).toEqual({ wrap: {} });
    expect(expandedKeys.value).toEqual(new Set([enc([]), enc(["wrap"])]));
    expect(editing.editing.value).toBeNull();
    expect(editing.canUndo.value).toBe(false);
    expect(editing.canRedo.value).toBe(false);
  });

  it("keeps array inserts on cancel and relies on undo instead", () => {
    const { doc, editing, reflow } = createHarness({ list: [1] });

    editing.applyOp({ type: "insert", parentPath: ["list"], index: 0, value: null });
    reflow();
    editing.beginEdit(enc(["list", 0]), "value");
    editing.cancelEditing();
    reflow();

    expect(doc.value).toEqual({ list: [null, 1] });
    expect(editing.editing.value).toBeNull();
    expect(editing.canUndo.value).toBe(true);
  });

  it("clears plain value or rename editing on cancel without touching history", () => {
    const { doc, editing } = createHarness({ a: 1 });

    editing.beginEdit(enc(["a"]), "rename");
    expect(editing.editing.value).toEqual({ key: enc(["a"]), mode: "rename" });

    editing.cancelEditing();
    expect(editing.editing.value).toBeNull();
    expect(doc.value).toEqual({ a: 1 });
    expect(editing.canUndo.value).toBe(false);
  });
});

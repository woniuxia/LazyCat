import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

describe("ActionCenterPanel contracts", () => {
  it("keeps run guarded by saved state and exposes step results", () => {
    const panelSource = readFileSync(new URL("./ActionCenterPanel.vue", import.meta.url), "utf8");
    const editorSource = readFileSync(
      new URL("./action-center/ActionCombinationEditor.vue", import.meta.url),
      "utf8",
    );

    expect(panelSource).toContain("ActionRunHistory");
    expect(panelSource).toContain('@run="runCombination"');
    expect(editorSource).toContain(':disabled="dirty || !canRun || runActive"');
  });

  it("uses SortableJS with a dedicated accessible drag handle", () => {
    const source = readFileSync(
      new URL("./action-center/ActionCombinationEditor.vue", import.meta.url),
      "utf8",
    );

    expect(source).toContain('import Sortable from "sortablejs"');
    expect(source).toContain('handle: ".action-step-drag"');
    expect(source).toContain("sortable?.destroy()");
    expect(source).toContain('title="拖动排序"');
  });

  it("caps the visible run history at twenty records", () => {
    const source = readFileSync(
      new URL("./action-center/ActionRunHistory.vue", import.meta.url),
      "utf8",
    );

    expect(source).toContain(".slice(0, 20)");
  });

  it("shows an active run only for the currently saved combination", () => {
    const source = readFileSync(new URL("./ActionCenterPanel.vue", import.meta.url), "utf8");

    expect(source).toContain("const displayedRun = computed(() =>");
    expect(source).toContain("draft.value?.id === activeRun.value?.combinationId");
    expect(source).toContain(':active-run="displayedRun"');
  });

  it("clears saved target snapshots when the action or target changes", () => {
    const source = readFileSync(
      new URL("./action-center/ActionCombinationEditor.vue", import.meta.url),
      "utf8",
    );

    expect(source.match(/targetLabel: undefined/g)).toHaveLength(2);
    expect(source.match(/unavailableReason: undefined/g)).toHaveLength(2);
  });

  it("locks interactions while operations are pending and confirms dirty create", () => {
    const source = readFileSync(new URL("./ActionCenterPanel.vue", import.meta.url), "utf8");
    expect(source).toContain("const interactionLocked = computed(");
    expect(source).toContain("async function confirmDiscardChanges");
    expect(source).toContain('await confirmDiscardChanges("新建组合")');
    expect(source.match(/:run-active="interactionLocked"/g)).toHaveLength(2);
    expect(source).toContain("const selecting = ref(false)");
    expect(source).toContain("selecting.value = true");
    expect(source).toContain("selecting.value = false");
    expect(source).toContain("const initializing = ref(true)");
    expect(source).toContain(
      "runActive.value || operationPending.value || initializing.value || selecting.value",
    );
    expect(source).toContain("initializing.value = false");
  });

  it("stops initialization continuations after the panel is unmounted", () => {
    const source = readFileSync(new URL("./ActionCenterPanel.vue", import.meta.url), "utf8");
    expect(source).toContain("let panelMounted = true");
    expect(source).toContain("await start();\n    if (!panelMounted) return;");
    expect(source).toContain(
      "await selectStoredCombination(id);\n  if (!panelMounted) return;\n  await loadDraftTargets();",
    );
    expect(source).toContain("panelMounted = false;\n  stop();");
  });

  it("tracks terminal notifications by run id and reloads targets after copy", () => {
    const source = readFileSync(new URL("./ActionCenterPanel.vue", import.meta.url), "utf8");
    expect(source).toContain("[activeRun.value?.id, activeRun.value?.status] as const");
    expect(source).toContain("async function copyCombinationDraft");
    expect(source).toContain('@copy="copyCombinationDraft"');
    expect(source).toContain("copyCombination();\n  await loadDraftTargets();");
    expect(source).toContain("activeRun.value?.combinationId ?? combinations.value[0].id");
  });

  it("supports keyboard step reordering with accessible drag labels", () => {
    const source = readFileSync(
      new URL("./action-center/ActionCombinationEditor.vue", import.meta.url), "utf8",
    );
    expect(source).toContain("function reorderStepWithKeyboard(index: number, event: KeyboardEvent)");
    expect(source).toContain('event.key === "ArrowUp"');
    expect(source).toContain('event.key === "ArrowDown"');
    expect(source).toContain('@keydown="reorderStepWithKeyboard(index, $event)"');
    expect(source).toContain(':aria-label="`拖动排序，第 ${index + 1} 步`"');
  });
});

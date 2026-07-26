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

  it("separates save refresh warnings from write failures", () => {
    const source = readFileSync(new URL("./ActionCenterPanel.vue", import.meta.url), "utf8");
    expect(source).toContain("const result = await persistCombination();");
    expect(source).toContain("result.refreshError");
    expect(source).toContain("组合动作已保存，但刷新失败");
  });

  it("surfaces run-level errors in notifications and run history", () => {
    const panelSource = readFileSync(new URL("./ActionCenterPanel.vue", import.meta.url), "utf8");
    const historySource = readFileSync(
      new URL("./action-center/ActionRunHistory.vue", import.meta.url), "utf8",
    );
    expect(panelSource).toContain("run.error?.trim()");
    expect(panelSource).toContain('status === "succeeded" ? "组合动作运行完成"');
    expect(panelSource).toContain('"组合动作运行失败"');
    expect(historySource).toContain("activeRun.error");
    expect(historySource).toContain("run.error");
    expect(historySource.match(/class="run-history__error"/g)).toHaveLength(2);
  });

  it("offers the target tool when a loaded target list is empty", () => {
    const source = readFileSync(
      new URL("./action-center/ActionCombinationEditor.vue", import.meta.url), "utf8",
    );
    expect(source).toContain(
      `targets.has(step.localId)
              && (
                targetState(step.localId).selected?.available === false
                || targetState(step.localId).options.length === 0
              )`,
    );
    expect(source).toContain("暂无可用目标，请先在对应工具中完成配置");
    expect(source).toContain("definitionFor(step.actionType)?.targetToolId");
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

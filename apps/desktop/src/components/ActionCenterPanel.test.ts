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
});

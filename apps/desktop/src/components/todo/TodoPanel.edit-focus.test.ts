import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./TodoPanel.vue", import.meta.url), "utf-8");
const detailEditSource = readFileSync(new URL("./TodoDetailEdit.vue", import.meta.url), "utf-8");

describe("TodoPanel edit focus behavior", () => {
  it("focuses the title input after entering edit mode from task items", () => {
    expect(source).toContain('function enterEditMode(item?: TodoItem | null, options: { focusTitle?: boolean } = {})');
    expect(source).toMatch(/if\s*\(options\.focusTitle\s*!==\s*false\)\s*\{\s*await focusTitleInputWhenActive\("edit", "edit_item"\);\s*\}/);
  });

  it("uses an exposed child method instead of reaching through the child ref internals", () => {
    expect(detailEditSource).toContain("function focusTitleInput()");
    expect(detailEditSource).toContain("function focusScheduleInput()");
    expect(detailEditSource).toMatch(/defineExpose\(\{\s*focusTitleInput,/);
    expect(source).toContain("todoDetailEditRef.value?.focusTitleInput();");
    expect(source).toContain("todoDetailEditRef.value?.focusScheduleInput();");
    expect(source).not.toContain("todoDetailEditRef.value?.titleInputRef.value?.focus();");
    expect(source).not.toContain("todoDetailEditRef.value?.scheduleRef.value");
  });

  it("does not steal focus from the edit-time shortcut", () => {
    expect(source).toContain("await enterEditMode(target, { focusTitle: false });");
  });
});

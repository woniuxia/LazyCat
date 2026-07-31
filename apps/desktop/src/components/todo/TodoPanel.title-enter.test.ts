import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./TodoPanel.vue", import.meta.url), "utf-8");

describe("TodoPanel title enter behavior", () => {
  it("saves both create and edit forms when Enter is pressed in the title input", () => {
    expect(source).toContain('@title-enter="onTitleEnter"');
    expect(source).toContain("function onTitleEnter(event: KeyboardEvent)");
    expect(source).toMatch(
      /detailMode\.value\s*===\s*"create"[\s\S]*itemDialogMode\.value\s*===\s*"create"/,
    );
    expect(source).toMatch(
      /detailMode\.value\s*===\s*"edit"[\s\S]*itemDialogMode\.value\s*===\s*"edit_item"/,
    );
    expect(source).toMatch(/void saveItem\(\)/);
  });
});

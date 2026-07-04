import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./JsonProcessPanel.vue", import.meta.url), "utf8");

describe("JsonProcessPanel source structure", () => {
  it("offers a text/tree segmented control defaulting to text", () => {
    expect(source).toContain("el-segmented");
    expect(source).toContain('{ label: "文本", value: "text" }');
    expect(source).toContain('{ label: "树形", value: "tree" }');
    expect(source).toContain('ref<InputMode>("text")');
  });

  it("guards tree mode behind the shared gate helper", () => {
    expect(source).toContain("canEnterJsonTree");
    expect(source).toContain("ElMessage.error(gate.reason)");
  });

  it("writes tree edits back to the input text immediately", () => {
    expect(source).toContain("JSON.stringify(value, null, 2)");
    expect(source).toContain("lastWrittenBack = text");
    expect(source).toContain("input.value = text");
  });

  it("treats unexpected input writes as a document change", () => {
    expect(source).toContain("if (text === lastWrittenBack) return");
    expect(source).toContain("已切回文本模式");
  });

  it("switches back to text mode before running text operations", () => {
    expect(source).toContain("function ensureTextMode()");
    const occurrences = source.split("ensureTextMode();").length - 1;
    expect(occurrences).toBeGreaterThanOrEqual(5);
  });

  it("mounts the editable tree on the input side with depth 2", () => {
    expect(source).toContain("JsonTreeViewer");
    expect(source).toContain("editable");
    expect(source).toContain(':default-expand-depth="2"');
  });

  it("shows a read-only output tree toggle only for eligible JSON", () => {
    expect(source).toContain("outputTreeAvailable");
    expect(source).toMatch(/outputMode\.value = "text";/);
    expect(source).toContain(':copy-text="output"');
  });

  it("documents the number precision boundary", () => {
    expect(source).toContain("Number.MAX_SAFE_INTEGER");
  });
});

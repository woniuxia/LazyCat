import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./CsvJsonPanel.vue", import.meta.url), "utf8");

describe("CsvJsonPanel source structure", () => {
  it("offers a read-only text/tree toggle on the output defaulting to text", () => {
    expect(source).toContain("el-segmented");
    expect(source).toContain('{ label: "文本", value: "text" }');
    expect(source).toContain('{ label: "树形", value: "tree" }');
    expect(source).toContain('ref<OutputMode>("text")');
    expect(source).not.toContain("editable");
  });

  it("gates the tree behind the shared 1MB JSON gate", () => {
    expect(source).toContain("canEnterJsonTree");
    expect(source).toContain("outputTreeAvailable");
  });

  it("mounts the tree with depth 2 and formatted copy text", () => {
    expect(source).toContain('import JsonTreeViewer from "./common/JsonTreeViewer.vue"');
    expect(source).toContain(':default-expand-depth="2"');
    expect(source).toContain(':copy-text="jsonOutput"');
  });

  it("resets to text mode whenever the output changes", () => {
    expect(source).toMatch(/watch\(jsonOutput, \(\) => \{\s*outputMode\.value = "text";/);
  });
});

import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const panelSource = readFileSync(new URL("./FileLockPanel.vue", import.meta.url), "utf8");

describe("FileLockPanel source structure", () => {
  it("uses the single-file inspect channel and file picker", () => {
    expect(panelSource).toContain('"tool:file-lock:inspect"');
    expect(panelSource).toContain("await open({");
    expect(panelSource).toContain("directory: false");
    expect(panelSource).toContain("multiple: false");
    expect(panelSource).toContain("path.value = selectedPath;");
    expect(panelSource).toContain("await inspect();");
  });

  it("keeps the diagnostic read-only and surfaces partial results", () => {
    expect(panelSource).toContain("只读诊断");
    expect(panelSource).toContain("result.warnings.length > 0");
    expect(panelSource).toContain("Windows 未报告可关联进程");
    expect(panelSource).not.toContain("tool:port:kill");
    expect(panelSource).not.toContain("commandLine");
  });

  it("provides copy and explorer actions for available process paths", () => {
    expect(panelSource).toContain("navigator.clipboard.writeText(value)");
    expect(panelSource).toContain('"tool:system:reveal-in-folder"');
    expect(panelSource).toContain("row.executablePath");
  });
});

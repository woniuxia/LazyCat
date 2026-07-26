import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const root = new URL("../", import.meta.url);
const read = (path: string) => readFileSync(new URL(path, root), "utf-8");

describe("ReferenceCard window wiring", () => {
  const component = read("components/ReferenceCard.vue");
  const main = read("main.ts");
  const bridge = read("bridge/tauri.ts");
  const monaco = read("components/MonacoPane.vue");

  it("mounts a dedicated card view", () => {
    expect(main).toContain('currentView === "reference-card"');
    expect(main).toContain('import("./ReferenceCardApp")');
  });

  it("subscribes before announcing ready", () => {
    expect(component.indexOf("listen<ReferenceCardInitPayload>")).toBeGreaterThan(-1);
    expect(component.indexOf("listen<ReferenceCardInitPayload>")).toBeLessThan(
      component.indexOf("referenceCardReady()"),
    );
    expect(bridge).toContain('invoke("reference_card_ready")');
  });

  it("uses Monaco and keeps transient content out of persistence", () => {
    expect(component).toContain("<MonacoPane");
    expect(component).toContain("data-tauri-drag-region");
    expect(component).toContain("suppressClipboardCapture(content.value)");
    expect(component).not.toContain("localStorage");
    expect(component).not.toContain("setSetting(");
  });

  it("adds explicit word-wrap and focus APIs without changing defaults", () => {
    expect(monaco).toContain("wordWrap?: boolean");
    expect(monaco).toContain("wordWrap: false");
    expect(monaco).toContain("function focusEditor()");
    expect(monaco).toContain("defineExpose({ formatDocument, focusLine, focusText, focusEditor })");
  });

  it("reports Monaco initialization and language failures with context", () => {
    expect(monaco).toContain('(event: "error", message: string): void');
    expect(monaco).toContain("Monaco 初始化失败");
    expect(monaco).toContain("切换 Monaco 语言失败");
    expect(component).toContain('@error="handleEditorError"');
  });
});

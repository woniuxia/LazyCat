import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const root = new URL("../", import.meta.url);
const read = (path: string) => readFileSync(new URL(path, root), "utf-8");
const referenceCardBackend = read("../src-tauri/src/reference_card/mod.rs");

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

  it("closes the focused card on Escape before Monaco handles it", () => {
    expect(component).toContain('window.addEventListener("keydown", onWindowKeydown, true)');
    expect(component).toContain('window.removeEventListener("keydown", onWindowKeydown, true)');
    expect(component).toContain('if (event.key !== "Escape") return;');
    expect(component).toContain("event.preventDefault();");
    expect(component).toContain("event.stopPropagation();");
    expect(component).toContain("void closeCard();");
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

  it("auto-sizes only during hidden creation and preserves manual resizing", () => {
    expect(referenceCardBackend).toContain(".visible(false)");
    expect(referenceCardBackend).toContain("configure_initial_geometry(&window, &text, ordinal)");
    expect(referenceCardBackend).toContain(".resizable(true)");
    expect(referenceCardBackend.match(/\.set_size\(/g)).toHaveLength(1);
    expect(referenceCardBackend).not.toContain(".max_inner_size(");
    expect(component).not.toContain("setSize(");
    expect(component).not.toContain("onResized(");
  });
});

describe("ReferenceCard shortcut settings", () => {
  const app = read("App.vue");
  const settings = read("components/SettingsPanel.vue");

  it("loads and registers the default shortcut", () => {
    expect(app).toContain('getSetting("hotkey_reference_card") ?? "Ctrl+Alt+Space"');
    expect(app).toContain('registerNamedHotkey("reference-card", savedReferenceCardHotkey)');
  });

  it("includes the shortcut in conflict, save and clear flows", () => {
    expect(settings).toContain('{ key: "referenceCardHotkeyInput" as const, label: "置顶参考卡" }');
    expect(settings).toContain('registerNamedHotkey("reference-card", referenceCard)');
    expect(settings).toContain('setSetting("hotkey_reference_card", referenceCard)');
    expect(settings).toContain('unregisterNamedHotkey("reference-card")');
    expect(settings).toContain('emit("update:referenceCardHotkeyInput", "")');
  });
});

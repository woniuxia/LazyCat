import { describe, expect, it } from "vitest";
import {
  shouldHideNamedHotkeyWindow,
  type HotkeyNavigatePayload,
} from "./hotkeyNavigate";

function createPayload(overrides: Partial<HotkeyNavigatePayload> = {}): HotkeyNavigatePayload {
  return {
    target: "snippets",
    didMoveToCursorMonitor: false,
    wasWindowVisible: true,
    wasWindowFocused: true,
    ...overrides,
  };
}

describe("shouldHideNamedHotkeyWindow", () => {
  it("hides the main window when the same tool was already visible and focused", () => {
    expect(shouldHideNamedHotkeyWindow(createPayload(), {
      activeTool: "snippets",
    })).toBe(true);
  });

  it("does not hide the main window after a cross-screen move", () => {
    expect(shouldHideNamedHotkeyWindow(createPayload({ didMoveToCursorMonitor: true }), {
      activeTool: "snippets",
    })).toBe(false);
  });

  it("does not hide the main window when the window was not visible before trigger", () => {
    expect(shouldHideNamedHotkeyWindow(createPayload({ wasWindowVisible: false }), {
      activeTool: "snippets",
    })).toBe(false);
  });

  it("does not hide the main window when the window was not focused before trigger", () => {
    expect(shouldHideNamedHotkeyWindow(createPayload({ wasWindowFocused: false }), {
      activeTool: "snippets",
    })).toBe(false);
  });

  it("does not hide the main window when the target tool is different", () => {
    expect(shouldHideNamedHotkeyWindow(createPayload({ target: "vault" }), {
      activeTool: "snippets",
    })).toBe(false);
  });
});

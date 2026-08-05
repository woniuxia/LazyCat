import { describe, expect, it } from "vitest";
import type { HotkeyNavigatePayload } from "./hotkeyNavigate";
import {
  normalizeActionDispatchRequest,
  normalizeWidgetNavigation,
  resolveHotkeyNavigation,
} from "./navigation-handoff";

const isRealToolId = (toolId: string) =>
  new Set(["action-center", "formatter", "pm", "todo", "data-dictionary"]).has(toolId);

function hotkey(overrides: Partial<HotkeyNavigatePayload> = {}): HotkeyNavigatePayload {
  return {
    target: "formatter",
    didMoveToCursorMonitor: false,
    wasWindowVisible: false,
    wasWindowFocused: false,
    ...overrides,
  };
}

describe("navigation handoff", () => {
  it("validates action-center requests before they enter pending state", () => {
    expect(
      normalizeActionDispatchRequest(
        {
          dispatchId: "dispatch-1",
          actionType: "release_package.run",
          targetToolId: "formatter",
          targetId: "42",
        },
        isRealToolId,
      ),
    ).toEqual({
      dispatchId: "dispatch-1",
      actionType: "release_package.run",
      targetToolId: "formatter",
      targetId: "42",
    });
    expect(
      normalizeActionDispatchRequest(
        { dispatchId: "dispatch-2", targetToolId: "unknown", targetId: "1" },
        isRealToolId,
      ),
    ).toBeNull();
  });

  it("keeps widget navigation discriminated and rejects unknown tools", () => {
    expect(normalizeWidgetNavigation({ kind: "open-todo-create" }, isRealToolId)).toEqual({
      kind: "open-todo-create",
    });
    expect(
      normalizeWidgetNavigation({ kind: "open-tool", toolId: "formatter" }, isRealToolId),
    ).toEqual({ kind: "open-tool", toolId: "formatter" });
    expect(
      normalizeWidgetNavigation({ kind: "open-tool", toolId: "unknown" }, isRealToolId),
    ).toBeNull();
  });

  it("resolves focus and prefill from one hotkey intent", () => {
    expect(
      resolveHotkeyNavigation(
        hotkey({
          target: "pm",
          itemId: "12",
          projectId: "7",
          view: "kanban",
          text: "来自 Spotlight",
          source: "clipboard-suggestion",
        }),
        isRealToolId,
      ),
    ).toMatchObject({
      targetToolId: "pm",
      focus: { kind: "pm", itemId: 12, projectId: 7, view: "kanban" },
      pendingInput: { toolId: "pm", text: "来自 Spotlight", source: "clipboard-suggestion" },
    });
  });

  it("does not create focus for invalid ids or silently turn unknown sources into inbox", () => {
    expect(
      resolveHotkeyNavigation(
        hotkey({ target: "todo", itemId: "0", text: "x", source: "keyword" }),
        isRealToolId,
      ),
    ).toMatchObject({ targetToolId: "todo" });
    expect(
      resolveHotkeyNavigation(
        hotkey({ target: "todo", itemId: "0", text: "x", source: "keyword" }),
        isRealToolId,
      ),
    ).not.toHaveProperty("pendingInput");
  });
});

import { beforeEach, describe, expect, it, vi } from "vitest";

const { listenMock, listeners, disposers } = vi.hoisted(() => ({
  listenMock: vi.fn(),
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  disposers: [] as Array<() => void>,
}));

vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

import { APP_EVENTS } from "../bridge/events";
import {
  startNavigationHandoffListeners,
  stopNavigationHandoffListeners,
  useNavigationHandoff,
} from "./useNavigationHandoff";

function emit(name: string, payload: unknown): void {
  listeners.get(name)?.({ payload });
}

const handlers = () => ({
  isRealToolId: (toolId: string) => toolId === "formatter" || toolId === "todo",
  onActionCenterDispatch: vi.fn(),
  onInvalidActionCenterDispatch: vi.fn(),
  onWidgetNavigate: vi.fn(),
  onHotkeyNavigate: vi.fn(),
});

describe("useNavigationHandoff", () => {
  beforeEach(() => {
    stopNavigationHandoffListeners();
    useNavigationHandoff().reset();
    listenMock
      .mockReset()
      .mockImplementation(async (name: string, handler: (event: { payload: unknown }) => void) => {
        listeners.set(name, handler);
        const dispose = vi.fn();
        disposers.push(dispose);
        return dispose;
      });
    listeners.clear();
    disposers.length = 0;
  });

  it("routes normalized events and releases every listener", async () => {
    const current = handlers();
    await startNavigationHandoffListeners(current);

    expect(listenMock).toHaveBeenCalledTimes(3);
    emit(APP_EVENTS.ACTION_CENTER_DISPATCH_REQUEST, {
      dispatchId: "dispatch-1",
      actionType: "release_package.run",
      targetToolId: "formatter",
      targetId: "42",
    });
    emit(APP_EVENTS.WIDGET_NAVIGATE, { kind: "open-todo-create" });
    emit(APP_EVENTS.HOTKEY_NAVIGATE, {
      target: "todo",
      didMoveToCursorMonitor: false,
      wasWindowVisible: false,
      wasWindowFocused: false,
      itemId: "7",
    });

    expect(current.onActionCenterDispatch).toHaveBeenCalledWith(
      expect.objectContaining({ dispatchId: "dispatch-1" }),
    );
    expect(current.onWidgetNavigate).toHaveBeenCalledWith({ kind: "open-todo-create" });
    expect(current.onHotkeyNavigate).toHaveBeenCalledWith(
      expect.objectContaining({ targetToolId: "todo" }),
    );

    stopNavigationHandoffListeners();
    expect(disposers).toHaveLength(3);
    for (const dispose of disposers) expect(dispose).toHaveBeenCalledTimes(1);
  });

  it("reports invalid action requests and does not duplicate listeners", async () => {
    const current = handlers();
    await startNavigationHandoffListeners(current);
    await startNavigationHandoffListeners(current);

    emit(APP_EVENTS.ACTION_CENTER_DISPATCH_REQUEST, {
      dispatchId: "dispatch-2",
      actionType: "release_package.run",
      targetToolId: "unknown",
      targetId: "42",
    });

    expect(listenMock).toHaveBeenCalledTimes(3);
    expect(current.onInvalidActionCenterDispatch).toHaveBeenCalledTimes(1);
    expect(current.onActionCenterDispatch).not.toHaveBeenCalled();
  });
});

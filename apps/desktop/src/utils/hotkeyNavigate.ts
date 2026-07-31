export interface HotkeyNavigatePayload {
  target: string;
  didMoveToCursorMonitor: boolean;
  wasWindowVisible: boolean;
  wasWindowFocused: boolean;
  text?: string;
  source?: string;
  itemId?: string;
  projectId?: string;
  view?: string;
}

interface NamedHotkeyWindowState {
  activeTool: string;
}

export function shouldHideNamedHotkeyWindow(
  payload: HotkeyNavigatePayload,
  state: NamedHotkeyWindowState,
): boolean {
  return (
    payload.target === state.activeTool &&
    payload.wasWindowVisible &&
    payload.wasWindowFocused &&
    !payload.didMoveToCursorMonitor
  );
}

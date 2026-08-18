import type { ActionDispatchRequest } from "../types/action-center";
import type {
  ActionCenterNavigationTarget,
  PendingToolInput,
  PendingInputSource,
  WidgetNavigationIntent,
  WidgetNavigatePayload,
} from "../types/navigation-handoff";
import type { HotkeyNavigatePayload } from "./hotkeyNavigate";

export type ToolIdValidator = (toolId: string) => boolean;

export type HotkeyFocusTarget =
  | { kind: "action-center"; target: ActionCenterNavigationTarget }
  | { kind: "pm"; itemId: number; projectId: number | null; view?: string }
  | { kind: "todo"; itemId: number }
  | { kind: "follow-up"; itemId: number | null; dueOnly: boolean }
  | { kind: "data-dictionary"; itemId: number };

export interface HotkeyNavigationIntent {
  payload: HotkeyNavigatePayload;
  targetToolId: string;
  focus?: HotkeyFocusTarget;
  pendingInput?: PendingToolInput;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" ? (value as Record<string, unknown>) : null;
}

function readRequiredString(record: Record<string, unknown>, key: string): string | null {
  const value = record[key];
  return typeof value === "string" && value.trim() ? value : null;
}

function readOptionalString(
  record: Record<string, unknown>,
  key: string,
): string | undefined | null {
  const value = record[key];
  if (value === undefined) return undefined;
  return typeof value === "string" ? value : null;
}

export function normalizeActionDispatchRequest(
  payload: unknown,
  isRealToolId: ToolIdValidator,
): ActionDispatchRequest | null {
  const record = asRecord(payload);
  if (!record) return null;

  const dispatchId = readRequiredString(record, "dispatchId");
  const actionType = readRequiredString(record, "actionType");
  const targetToolId = readRequiredString(record, "targetToolId");
  const targetId = readRequiredString(record, "targetId");
  if (!dispatchId || !actionType || !targetToolId || !targetId) return null;
  if (!isRealToolId(targetToolId)) return null;

  return { dispatchId, actionType, targetToolId, targetId };
}

export function normalizeWidgetNavigation(
  payload: unknown,
  isRealToolId: ToolIdValidator,
): WidgetNavigationIntent | null {
  const record = asRecord(payload) as Partial<WidgetNavigatePayload> | null;
  if (!record || typeof record.kind !== "string") return null;

  if (record.kind === "open-todo-create") {
    return { kind: "open-todo-create" };
  }
  if (record.kind !== "open-tool" || typeof record.toolId !== "string") return null;
  if (!isRealToolId(record.toolId)) return null;
  return { kind: "open-tool", toolId: record.toolId };
}

export function normalizeHotkeyNavigatePayload(
  payload: unknown,
  isRealToolId: ToolIdValidator,
): HotkeyNavigatePayload | null {
  const record = asRecord(payload);
  if (!record) return null;

  const target = readRequiredString(record, "target");
  const didMoveToCursorMonitor = record.didMoveToCursorMonitor;
  const wasWindowVisible = record.wasWindowVisible;
  const wasWindowFocused = record.wasWindowFocused;
  if (!target || !isRealToolId(target)) return null;
  if (
    typeof didMoveToCursorMonitor !== "boolean" ||
    typeof wasWindowVisible !== "boolean" ||
    typeof wasWindowFocused !== "boolean"
  ) {
    return null;
  }

  const text = readOptionalString(record, "text");
  const source = readOptionalString(record, "source");
  const itemId = readOptionalString(record, "itemId");
  const projectId = readOptionalString(record, "projectId");
  const view = readOptionalString(record, "view");
  if (text === null || source === null || itemId === null || projectId === null || view === null) {
    return null;
  }

  return {
    target,
    didMoveToCursorMonitor,
    wasWindowVisible,
    wasWindowFocused,
    ...(text === undefined ? {} : { text }),
    ...(source === undefined ? {} : { source }),
    ...(itemId === undefined ? {} : { itemId }),
    ...(projectId === undefined ? {} : { projectId }),
    ...(view === undefined ? {} : { view }),
  };
}

function parsePositiveId(value: string | undefined): number | null {
  if (!value) return null;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : null;
}

function resolveFocusTarget(payload: HotkeyNavigatePayload): HotkeyFocusTarget | undefined {
  if (payload.target === "todo" && payload.view?.startsWith("follow-up")) {
    return {
      kind: "follow-up",
      itemId: parsePositiveId(payload.itemId),
      dueOnly: payload.view === "follow-up-due",
    };
  }
  if (!payload.itemId) return undefined;

  if (payload.target === "action-center") {
    if (payload.view === "run" && payload.itemId.trim()) {
      return { kind: "action-center", target: { kind: "run", runId: payload.itemId.trim() } };
    }
    const combinationId = parsePositiveId(payload.itemId);
    return combinationId
      ? { kind: "action-center", target: { kind: "combination", combinationId } }
      : undefined;
  }

  const itemId = parsePositiveId(payload.itemId);
  if (!itemId) return undefined;

  if (payload.target === "pm") {
    const projectId = parsePositiveId(payload.projectId);
    return { kind: "pm", itemId, projectId, view: payload.view };
  }
  if (payload.target === "todo") return { kind: "todo", itemId };
  if (payload.target === "data-dictionary") {
    return { kind: "data-dictionary", itemId };
  }
  return undefined;
}

function resolvePendingInput(payload: HotkeyNavigatePayload): PendingToolInput | undefined {
  if (!payload.text || !payload.source) return undefined;
  const source: PendingInputSource | undefined =
    payload.source === "clipboard-suggestion" || payload.source === "inbox"
      ? payload.source
      : undefined;
  if (!source) return undefined;
  return { toolId: payload.target, text: payload.text, source };
}

export function resolveHotkeyNavigation(
  payload: unknown,
  isRealToolId: ToolIdValidator,
): HotkeyNavigationIntent | null {
  const normalized = normalizeHotkeyNavigatePayload(payload, isRealToolId);
  if (!normalized) return null;
  const focus = resolveFocusTarget(normalized);
  const pendingInput = resolvePendingInput(normalized);
  return {
    payload: normalized,
    targetToolId: normalized.target,
    ...(focus ? { focus } : {}),
    ...(pendingInput ? { pendingInput } : {}),
  };
}

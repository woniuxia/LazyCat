import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { ref } from "vue";
import { APP_EVENTS } from "../bridge/events";
import type { ActionDispatchRequest } from "../types/action-center";
import type {
  ActionCenterNavigationTarget,
  PendingToolInput,
  WidgetNavigationIntent,
} from "../types/navigation-handoff";
import {
  normalizeActionDispatchRequest,
  normalizeWidgetNavigation,
  resolveHotkeyNavigation,
  type HotkeyNavigationIntent,
  type ToolIdValidator,
} from "../utils/navigation-handoff";

const pendingToolInput = ref<PendingToolInput | null>(null);
const pendingIntent = ref<ActionDispatchRequest | null>(null);
const pendingActionCenterTarget = ref<ActionCenterNavigationTarget | null>(null);
const pendingTodoCreate = ref(false);

export interface NavigationHandoffHandlers {
  isRealToolId: ToolIdValidator;
  onActionCenterDispatch: (request: ActionDispatchRequest) => void;
  onInvalidActionCenterDispatch?: () => void;
  onWidgetNavigate: (intent: WidgetNavigationIntent) => void;
  onHotkeyNavigate: (intent: HotkeyNavigationIntent) => void | Promise<void>;
}

let listenerGeneration = 0;
let listenerPromise: Promise<void> | null = null;
let activeUnlisteners: UnlistenFn[] = [];

function disposeListeners(unlisteners: UnlistenFn[]): void {
  for (const unlisten of unlisteners) unlisten();
}

function registerNavigationListeners(handlers: NavigationHandoffHandlers): Promise<UnlistenFn[]> {
  const registrations = [
    Promise.resolve().then(() =>
      listen<unknown>(APP_EVENTS.ACTION_CENTER_DISPATCH_REQUEST, ({ payload }) => {
        const request = normalizeActionDispatchRequest(payload, handlers.isRealToolId);
        if (request) handlers.onActionCenterDispatch(request);
        else handlers.onInvalidActionCenterDispatch?.();
      }),
    ),
    Promise.resolve().then(() =>
      listen<unknown>(APP_EVENTS.WIDGET_NAVIGATE, ({ payload }) => {
        const intent = normalizeWidgetNavigation(payload, handlers.isRealToolId);
        if (intent) handlers.onWidgetNavigate(intent);
      }),
    ),
    Promise.resolve().then(() =>
      listen<unknown>(APP_EVENTS.HOTKEY_NAVIGATE, ({ payload }) => {
        const intent = resolveHotkeyNavigation(payload, handlers.isRealToolId);
        if (intent) void handlers.onHotkeyNavigate(intent);
      }),
    ),
  ];

  return Promise.allSettled(registrations).then((results) =>
    results.flatMap((result) => (result.status === "fulfilled" ? [result.value] : [])),
  );
}

export function startNavigationHandoffListeners(
  handlers: NavigationHandoffHandlers,
): Promise<void> {
  if (activeUnlisteners.length > 0) return Promise.resolve();
  if (listenerPromise) return listenerPromise;

  const generation = ++listenerGeneration;
  const registration = registerNavigationListeners(handlers).then((unlisteners) => {
    if (generation !== listenerGeneration) {
      disposeListeners(unlisteners);
      return;
    }
    activeUnlisteners = unlisteners;
  });
  listenerPromise = registration;
  void registration.finally(() => {
    if (listenerPromise === registration) listenerPromise = null;
  });
  return registration;
}

export function stopNavigationHandoffListeners(): void {
  listenerGeneration += 1;
  listenerPromise = null;
  const unlisteners = activeUnlisteners;
  activeUnlisteners = [];
  disposeListeners(unlisteners);
}

export function useNavigationHandoff() {
  function setPendingToolInput(input: PendingToolInput): void {
    pendingToolInput.value = input;
  }

  function consumePendingToolInput(toolId: string): PendingToolInput | null {
    if (pendingToolInput.value?.toolId !== toolId) return null;
    const current = pendingToolInput.value;
    pendingToolInput.value = null;
    return current;
  }

  function setPendingIntent(intent: ActionDispatchRequest): void {
    pendingIntent.value = intent;
  }

  function consumePendingIntent(toolId: string): ActionDispatchRequest | null {
    if (pendingIntent.value?.targetToolId !== toolId) return null;
    const current = pendingIntent.value;
    pendingIntent.value = null;
    return current;
  }

  function requestCombination(combinationId: number): void {
    if (!Number.isSafeInteger(combinationId) || combinationId <= 0) return;
    pendingActionCenterTarget.value = { kind: "combination", combinationId };
  }

  function requestRun(runId: string): void {
    const normalized = runId.trim();
    if (!normalized) return;
    pendingActionCenterTarget.value = { kind: "run", runId: normalized };
  }

  function consumeActionCenterTarget(target: ActionCenterNavigationTarget): void {
    if (pendingActionCenterTarget.value === target) pendingActionCenterTarget.value = null;
  }

  function requestTodoCreate(): void {
    pendingTodoCreate.value = true;
  }

  function reset(): void {
    pendingToolInput.value = null;
    pendingIntent.value = null;
    pendingActionCenterTarget.value = null;
    pendingTodoCreate.value = false;
  }

  return {
    pendingToolInput,
    pendingIntent,
    pendingActionCenterTarget,
    pendingTodoCreate,
    setPendingToolInput,
    consumePendingToolInput,
    setPendingIntent,
    consumePendingIntent,
    requestCombination,
    requestRun,
    consumeActionCenterTarget,
    requestTodoCreate,
    reset,
  };
}

export type {
  ActionCenterNavigationTarget,
  PendingToolInput,
  WidgetNavigationIntent,
} from "../types/navigation-handoff";
export type { HotkeyNavigationIntent } from "../utils/navigation-handoff";

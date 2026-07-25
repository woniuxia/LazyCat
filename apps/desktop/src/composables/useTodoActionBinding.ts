import { ref } from "vue";

import { invokeToolByChannel } from "../bridge/tauri";
import type {
  ActionDefinition,
  ActionDispatchSummary,
  ActionTargetOption,
} from "../types";

interface TodoActionDraft {
  actionType: string | null;
  actionTargetId: string | null;
}

interface TodoActionTrigger {
  id: number;
}

interface DispatchOptions {
  triggerEventId?: string;
}

export function useTodoActionBinding(itemDraft: TodoActionDraft) {
  const actionDefinitions = ref<ActionDefinition[]>([]);
  const actionTargets = ref<ActionTargetOption[]>([]);
  const latestDispatch = ref<ActionDispatchSummary | null>(null);
  const loadedActionType = ref<string | null>(null);
  let targetRequestVersion = 0;
  let latestDispatchRequestVersion = 0;

  async function loadDefinitions() {
    const response = (await invokeToolByChannel("tool:action-center:definition-list", {})) as {
      definitions?: ActionDefinition[];
    };
    actionDefinitions.value = response.definitions || [];
  }

  async function loadTargets(actionType = itemDraft.actionType) {
    const requestVersion = ++targetRequestVersion;
    if (!actionType) {
      actionTargets.value = [];
      loadedActionType.value = null;
      return;
    }
    const response = (await invokeToolByChannel("tool:action-center:target-list", {
      actionType,
    })) as { targets?: ActionTargetOption[] };
    if (requestVersion !== targetRequestVersion || itemDraft.actionType !== actionType) return;
    actionTargets.value = response.targets || [];
    loadedActionType.value = actionType;
  }

  async function onActionTypeChange(actionType: string | null) {
    itemDraft.actionType = actionType || null;
    itemDraft.actionTargetId = null;
    if (!itemDraft.actionType) {
      actionTargets.value = [];
      loadedActionType.value = null;
      targetRequestVersion += 1;
      return;
    }
    await loadTargets(itemDraft.actionType);
  }

  async function loadLatestDispatch(todoId: number) {
    const requestVersion = ++latestDispatchRequestVersion;
    const response = (await invokeToolByChannel("tool:action-center:dispatch-latest", {
      triggerType: "todo_item",
      triggerId: String(todoId),
    })) as { dispatch?: ActionDispatchSummary | null };
    const dispatch = response.dispatch || null;
    if (requestVersion === latestDispatchRequestVersion) {
      latestDispatch.value = dispatch;
    }
    return dispatch;
  }

  function clearLatestDispatch() {
    latestDispatchRequestVersion += 1;
    latestDispatch.value = null;
  }

  function isAvailableTarget(actionType: string | null, targetId: string | null) {
    if (!actionType || !targetId || loadedActionType.value !== actionType) return false;
    return actionTargets.value.some((target) => target.id === targetId && target.available);
  }

  async function dispatchTodoAction(
    item: TodoActionTrigger,
    options: DispatchOptions = {},
  ) {
    const requestVersion = ++latestDispatchRequestVersion;
    const payload: Record<string, string> = {
      triggerType: "todo_item",
      triggerId: String(item.id),
    };
    if (options.triggerEventId) payload.triggerEventId = options.triggerEventId;
    const dispatch = (await invokeToolByChannel(
      "tool:action-center:dispatch",
      payload,
    )) as ActionDispatchSummary;
    if (requestVersion === latestDispatchRequestVersion) {
      latestDispatch.value = dispatch;
    }
    return dispatch;
  }

  return {
    actionDefinitions,
    actionTargets,
    latestDispatch,
    loadDefinitions,
    loadTargets,
    onActionTypeChange,
    loadLatestDispatch,
    clearLatestDispatch,
    isAvailableTarget,
    dispatchTodoAction,
  };
}

import { computed, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import { APP_EVENTS } from "../bridge/events";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  ActionCombinationDetail,
  ActionCombinationDraft,
  ActionCombinationRunDetail,
  ActionCombinationRunUpdatedEvent,
  ActionCombinationSummary,
  ActionCombinationTarget,
  CombinationAtomicDefinition,
} from "../types/action-center";
import {
  createCombinationDraft,
  createEmptyCombinationDraft,
  isCombinationRunTerminal,
  moveCombinationStep,
  toCombinationSaveInput,
} from "../utils/actionCombination";

interface UseActionCombinationsOptions {
  pollIntervalMs?: number;
}

const DEFAULT_POLL_INTERVAL_MS = 1_000;

function draftFingerprint(draft: ActionCombinationDraft | null): string {
  return draft ? JSON.stringify(toCombinationSaveInput(draft)) : "";
}

export function useActionCombinations(options: UseActionCombinationsOptions = {}) {
  const pollIntervalMs = options.pollIntervalMs ?? DEFAULT_POLL_INTERVAL_MS;
  const definitions = ref<CombinationAtomicDefinition[]>([]);
  const combinations = ref<ActionCombinationSummary[]>([]);
  const selectedId = ref<number | null>(null);
  const selectedCombination = ref<ActionCombinationDetail | null>(null);
  const draft = ref<ActionCombinationDraft | null>(null);
  const savedFingerprint = ref("");
  const stepTargets = ref(new Map<string, ActionCombinationTarget[]>());
  const activeRun = ref<ActionCombinationRunDetail | null>(null);
  const runHistory = ref<ActionCombinationRunDetail[]>([]);
  const targetRequestVersions = new Map<string, number>();
  let selectionVersion = 0;
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let unlisten: UnlistenFn | null = null;
  let listenerPromise: Promise<void> | null = null;
  let runRefreshVersion = 0;

  const dirty = computed(() => draftFingerprint(draft.value) !== savedFingerprint.value);
  const runActive = computed(
    () => activeRun.value !== null && !isCombinationRunTerminal(activeRun.value.status),
  );

  function replaceDraft(next: ActionCombinationDraft | null, markSaved: boolean): void {
    draft.value = next;
    savedFingerprint.value = markSaved ? draftFingerprint(next) : "";
    stepTargets.value = new Map();
    targetRequestVersions.clear();
  }

  function clearPoll(): void {
    if (pollTimer !== null) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  async function loadDefinitions(): Promise<void> {
    const response = (await invokeToolByChannel(
      "tool:action-center:combination-definition-list",
      {},
    )) as { definitions: CombinationAtomicDefinition[] };
    definitions.value = response.definitions;
  }

  async function loadCombinations(): Promise<void> {
    const response = (await invokeToolByChannel(
      "tool:action-center:combination-list",
      {},
    )) as { combinations: ActionCombinationSummary[] };
    combinations.value = response.combinations;
  }

  async function loadRunHistory(combinationId: number): Promise<void> {
    const response = (await invokeToolByChannel(
      "tool:action-center:combination-run-list",
      { combinationId },
    )) as { runs: ActionCombinationRunDetail[] };
    runHistory.value = response.runs;
  }

  async function refreshActiveRun(runId: string): Promise<void> {
    const version = ++runRefreshVersion;
    const run = (await invokeToolByChannel(
      "tool:action-center:combination-run-get",
      { runId },
    )) as ActionCombinationRunDetail;
    if (version !== runRefreshVersion || activeRun.value?.id !== runId) return;
    activeRun.value = run;
    if (!isCombinationRunTerminal(run.status)) return;
    clearPoll();
    if (typeof run.combinationId === "number") {
      await loadRunHistory(run.combinationId);
    }
  }

  function schedulePoll(runId: string): void {
    clearPoll();
    pollTimer = setInterval(() => {
      void refreshActiveRun(runId);
    }, pollIntervalMs);
  }

  async function trackRun(run: ActionCombinationRunDetail): Promise<void> {
    runRefreshVersion += 1;
    activeRun.value = run;
    if (isCombinationRunTerminal(run.status)) {
      clearPoll();
      if (typeof run.combinationId === "number") {
        await loadRunHistory(run.combinationId);
      }
      return;
    }
    schedulePoll(run.id);
  }

  async function ensureListener(): Promise<void> {
    if (!listenerPromise) {
      listenerPromise = listen<ActionCombinationRunUpdatedEvent>(
        APP_EVENTS.ACTION_CENTER_COMBINATION_RUN_UPDATED,
        ({ payload }) => {
          if (activeRun.value?.id !== payload.runId) return;
          void refreshActiveRun(payload.runId);
        },
      ).then((dispose) => {
        unlisten = dispose;
      });
    }
    return listenerPromise;
  }

  async function start(): Promise<void> {
    await Promise.all([ensureListener(), loadDefinitions(), loadCombinations()]);
  }

  function stop(): void {
    runRefreshVersion += 1;
    clearPoll();
    if (unlisten) {
      unlisten();
      unlisten = null;
    } else if (listenerPromise) {
      void listenerPromise.then(() => {
        unlisten?.();
        unlisten = null;
      });
    }
    listenerPromise = null;
  }

  async function selectCombination(combinationId: number): Promise<void> {
    const version = ++selectionVersion;
    const detail = (await invokeToolByChannel(
      "tool:action-center:combination-get",
      { combinationId },
    )) as ActionCombinationDetail;
    if (version !== selectionVersion) return;
    selectedId.value = detail.id;
    selectedCombination.value = detail;
    replaceDraft(createCombinationDraft(detail), true);
    await loadRunHistory(detail.id);
  }

  function createCombination(): void {
    selectionVersion += 1;
    selectedId.value = null;
    selectedCombination.value = null;
    runHistory.value = [];
    replaceDraft(createEmptyCombinationDraft(), false);
  }

  function copyCombination(): void {
    if (!draft.value) return;
    replaceDraft(
      {
        ...draft.value,
        id: undefined,
        name: `${draft.value.name} 副本`,
        steps: draft.value.steps.map((step) => ({ ...step })),
      },
      false,
    );
    selectedId.value = null;
    selectedCombination.value = null;
    runHistory.value = [];
  }

  async function loadStepTargets(localStepId: string, actionType: string): Promise<void> {
    const version = (targetRequestVersions.get(localStepId) ?? 0) + 1;
    targetRequestVersions.set(localStepId, version);
    const response = (await invokeToolByChannel(
      "tool:action-center:combination-target-list",
      { actionType },
    )) as { targets: ActionCombinationTarget[] };
    if (targetRequestVersions.get(localStepId) !== version) return;
    const next = new Map(stepTargets.value);
    next.set(localStepId, response.targets);
    stepTargets.value = next;
  }

  function reorderSteps(fromIndex: number, toIndex: number): void {
    if (!draft.value) return;
    draft.value.steps = moveCombinationStep(draft.value.steps, fromIndex, toIndex);
  }

  async function saveCombination(): Promise<number> {
    if (!draft.value) throw new Error("没有可保存的组合动作");
    const input = toCombinationSaveInput(draft.value);
    const response = (await invokeToolByChannel(
      "tool:action-center:combination-save",
      { ...input },
    )) as { id: number };
    await loadCombinations();
    await selectCombination(response.id);
    return response.id;
  }

  async function deleteCombination(combinationId: number): Promise<void> {
    await invokeToolByChannel("tool:action-center:combination-delete", { combinationId });
    if (selectedId.value === combinationId) createCombination();
    await loadCombinations();
  }

  async function runCombination(combinationId: number): Promise<ActionCombinationRunDetail> {
    const run = (await invokeToolByChannel(
      "tool:action-center:combination-run",
      { combinationId },
    )) as ActionCombinationRunDetail;
    await trackRun(run);
    return run;
  }

  return {
    definitions,
    combinations,
    selectedId,
    selectedCombination,
    draft,
    dirty,
    stepTargets,
    activeRun,
    runHistory,
    runActive,
    start,
    stop,
    loadDefinitions,
    loadCombinations,
    loadRunHistory,
    loadStepTargets,
    refreshActiveRun,
    trackRun,
    selectCombination,
    createCombination,
    copyCombination,
    reorderSteps,
    saveCombination,
    deleteCombination,
    runCombination,
  };
}

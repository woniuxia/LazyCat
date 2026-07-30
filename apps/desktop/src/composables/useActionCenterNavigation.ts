import { ref } from "vue";

export type ActionCenterNavigationTarget =
  | { kind: "combination"; combinationId: number }
  | { kind: "run"; runId: string };

const pendingTarget = ref<ActionCenterNavigationTarget | null>(null);

export function useActionCenterNavigation() {
  function requestCombination(combinationId: number): void {
    if (!Number.isSafeInteger(combinationId) || combinationId <= 0) return;
    pendingTarget.value = { kind: "combination", combinationId };
  }

  function requestRun(runId: string): void {
    const normalized = runId.trim();
    if (!normalized) return;
    pendingTarget.value = { kind: "run", runId: normalized };
  }

  function consume(target: ActionCenterNavigationTarget): void {
    if (pendingTarget.value === target) pendingTarget.value = null;
  }

  return { pendingTarget, requestCombination, requestRun, consume };
}

import { useNavigationHandoff } from "./useNavigationHandoff";

import type { ActionCenterNavigationTarget } from "../types/navigation-handoff";
export type { ActionCenterNavigationTarget } from "../types/navigation-handoff";

export function useActionCenterNavigation() {
  const handoff = useNavigationHandoff();

  function requestCombination(combinationId: number): void {
    handoff.requestCombination(combinationId);
  }

  function requestRun(runId: string): void {
    handoff.requestRun(runId);
  }

  function consume(target: ActionCenterNavigationTarget): void {
    handoff.consumeActionCenterTarget(target);
  }

  return {
    pendingTarget: handoff.pendingActionCenterTarget,
    requestCombination,
    requestRun,
    consume,
  };
}

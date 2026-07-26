import type {
  ActionCombinationDetail,
  ActionCombinationDraft,
  ActionCombinationDraftStep,
  ActionCombinationRunStatus,
  ActionCombinationSaveInput,
  ActionCombinationStepStatus,
  ActionCombinationTarget,
} from "../types/action-center";

let localStepSequence = 0;

function nextLocalStepId(): string {
  localStepSequence += 1;
  return `action-step-${localStepSequence}`;
}

export function createEmptyCombinationDraft(): ActionCombinationDraft {
  return {
    name: "",
    executionMode: "serial",
    steps: [],
  };
}

export function createCombinationDraft(
  detail: ActionCombinationDetail,
): ActionCombinationDraft {
  return {
    id: detail.id,
    name: detail.name,
    executionMode: detail.executionMode,
    steps: detail.steps.map((step) => ({
      localId: nextLocalStepId(),
      actionType: step.actionType,
      targetId: step.targetId,
      targetLabel: step.targetLabel,
      available: step.available,
      unavailableReason: step.unavailableReason,
    })),
  };
}

export function resolveCombinationStepTargets(
  step: ActionCombinationDraftStep,
  liveTargets: readonly ActionCombinationTarget[],
): { options: ActionCombinationTarget[]; selected?: ActionCombinationTarget } {
  const options = [...liveTargets];
  const selected = options.find((target) => target.id === step.targetId);
  if (selected || !step.targetId) return { options, selected };

  const snapshot = {
    id: step.targetId,
    label: step.targetLabel?.trim() || step.targetId,
    available: false,
    unavailableReason: step.unavailableReason || "目标已失效",
  };
  return { options: [snapshot, ...options], selected: snapshot };
}

export function createEmptyCombinationStep(): ActionCombinationDraftStep {
  return {
    localId: nextLocalStepId(),
    actionType: "",
    targetId: "",
  };
}

export function toCombinationSaveInput(
  draft: ActionCombinationDraft,
): ActionCombinationSaveInput {
  return {
    ...(draft.id === undefined ? {} : { id: draft.id }),
    name: draft.name.trim(),
    executionMode: draft.executionMode,
    steps: draft.steps.map(({ actionType, targetId }) => ({
      actionType,
      targetId,
    })),
  };
}

export function moveCombinationStep(
  source: readonly ActionCombinationDraftStep[],
  fromIndex: number,
  toIndex: number,
): ActionCombinationDraftStep[] {
  const moved = [...source];
  if (
    fromIndex < 0
    || fromIndex >= moved.length
    || toIndex < 0
    || toIndex >= moved.length
    || fromIndex === toIndex
  ) {
    return moved;
  }
  const [step] = moved.splice(fromIndex, 1);
  moved.splice(toIndex, 0, step);
  return moved;
}

export function isCombinationRunTerminal(status: ActionCombinationRunStatus): boolean {
  return status === "succeeded" || status === "partially_succeeded" || status === "failed";
}

export function combinationRunStatusLabel(status: ActionCombinationRunStatus): string {
  return {
    pending: "等待运行",
    running: "运行中",
    succeeded: "全部成功",
    partially_succeeded: "部分成功",
    failed: "运行失败",
  }[status];
}

export function combinationStepStatusLabel(status: ActionCombinationStepStatus): string {
  return {
    pending: "等待",
    running: "运行中",
    succeeded: "成功",
    already_satisfied: "已满足",
    failed: "失败",
  }[status];
}

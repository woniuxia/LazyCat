export type ActionDispatchStatus =
  | "pending_confirmation"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled";

export interface ActionDefinition {
  actionType: string;
  label: string;
  triggerTypes: string[];
  targetKind: string;
  targetToolId: string;
  executionMode: "open_and_confirm" | "direct" | "background";
  completionPolicy: "on_started" | "on_succeeded" | "manual";
  supportsCombination: boolean;
}

export interface ActionTargetOption {
  id: string;
  label: string;
  available: boolean;
  unavailableReason?: string;
}

export interface ActionBindingInput {
  actionType: string;
  targetId: string;
}

export interface ActionBindingSummary {
  id: number;
  actionType: string;
  actionLabel: string;
  targetId: string;
  targetLabel: string;
  available: boolean;
  unavailableReason?: string;
}

export interface ActionDispatchSummary {
  id: string;
  triggerType: string;
  triggerId: string;
  actionType: string;
  targetId: string;
  status: ActionDispatchStatus;
  resultCode?: string;
  error?: string;
  createdAt: string;
  startedAt?: string;
  finishedAt?: string;
}

export interface ActionDispatchRequest {
  dispatchId: string;
  actionType: string;
  targetToolId: string;
  targetId: string;
}

export type ActionCombinationExecutionMode = "serial" | "parallel";

export type ActionCombinationRunStatus =
  | "pending"
  | "running"
  | "succeeded"
  | "partially_succeeded"
  | "failed";

export type ActionCombinationStepStatus =
  | "pending"
  | "running"
  | "succeeded"
  | "already_satisfied"
  | "failed";

export interface CombinationAtomicDefinition {
  actionType: string;
  label: string;
  targetKind: string;
  targetToolId: string;
}

export interface ActionCombinationTarget {
  id: string;
  label: string;
  available: boolean;
  unavailableReason?: string;
}

export interface ActionCombinationStepInput {
  actionType: string;
  targetId: string;
}

export interface ActionCombinationSaveInput {
  id?: number;
  name: string;
  executionMode: ActionCombinationExecutionMode;
  steps: ActionCombinationStepInput[];
}

export interface ActionCombinationStep {
  id: number;
  actionType: string;
  targetId: string;
  sortOrder: number;
  targetLabel?: string;
  available?: boolean;
  unavailableReason?: string;
  createdAt: string;
  updatedAt: string;
}

export interface ActionCombinationDetail {
  id: number;
  name: string;
  executionMode: ActionCombinationExecutionMode;
  steps: ActionCombinationStep[];
  createdAt: string;
  updatedAt: string;
}

export interface ActionCombinationSummary {
  id: number;
  name: string;
  executionMode: ActionCombinationExecutionMode;
  stepCount: number;
  latestRunStatus?: ActionCombinationRunStatus;
  updatedAt: string;
}

export interface ActionCombinationRunStep {
  id: number;
  actionType: string;
  actionLabel: string;
  targetId: string;
  targetLabel: string;
  sortOrder: number;
  status: ActionCombinationStepStatus;
  resultCode?: string;
  message?: string;
  startedAt?: string;
  finishedAt?: string;
}

export interface ActionCombinationRunDetail {
  id: string;
  combinationId?: number | null;
  combinationName: string;
  executionMode: ActionCombinationExecutionMode;
  status: ActionCombinationRunStatus;
  resultCode?: string;
  error?: string;
  createdAt: string;
  startedAt?: string;
  finishedAt?: string;
  steps: ActionCombinationRunStep[];
}

export interface ActionCombinationDraftStep extends ActionCombinationStepInput {
  localId: string;
  targetLabel?: string;
  available?: boolean;
  unavailableReason?: string;
}

export interface ActionCombinationDraft {
  id?: number;
  name: string;
  executionMode: ActionCombinationExecutionMode;
  steps: ActionCombinationDraftStep[];
}

export interface ActionCombinationRunUpdatedEvent {
  runId: string;
  status: ActionCombinationRunStatus;
}

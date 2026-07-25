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

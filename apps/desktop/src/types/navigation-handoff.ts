export type PendingInputSource = "clipboard-suggestion" | "inbox";

export interface TodoPendingDraft {
  title?: string;
  description?: string;
}

export interface VaultPendingDraft {
  category?: "app" | "server" | "database";
  title?: string;
  environment?: string;
  fields?: Record<string, unknown>;
  tags?: string[];
}

export interface PendingToolInput {
  toolId: string;
  text: string;
  source?: PendingInputSource;
  label?: string;
  todoDraft?: TodoPendingDraft;
  vaultDraft?: VaultPendingDraft;
  meta?: Record<string, unknown>;
}

export interface WidgetNavigatePayload {
  kind: string;
  toolId?: string;
}

export type WidgetNavigationIntent =
  | { kind: "open-tool"; toolId: string }
  | { kind: "open-todo-create" };

export type ActionCenterNavigationTarget =
  | { kind: "combination"; combinationId: number }
  | { kind: "run"; runId: string };

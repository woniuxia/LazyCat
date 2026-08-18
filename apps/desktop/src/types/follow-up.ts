import type { TodoAssignee, TodoPriority } from "./todo";

export type FollowUpAttentionStatus = "active" | "ended";
export type FollowUpExternalResult = "unknown" | "completed" | "canceled";
export type FollowUpEndingMode = "result_confirmed" | "stopped_following" | null;
export type FollowUpProgressKind =
  | "progress"
  | "continued"
  | "completed"
  | "canceled"
  | "stopped_following"
  | "reopened";

export interface FollowUpLink {
  id?: number;
  url: string;
  title: string;
}
export interface FollowUpProgress {
  id: number;
  kind: FollowUpProgressKind;
  content: string;
  occurredAt: string;
  updatedAt: string;
}
export interface FollowUpItem {
  id: number;
  title: string;
  description: string;
  expectedOutcome: string;
  priority: TodoPriority;
  attentionStatus: FollowUpAttentionStatus;
  externalResult: FollowUpExternalResult;
  endingMode: FollowUpEndingMode;
  personId: number | null;
  personName: string;
  personNameSnapshot: string;
  reviewAt: string | null;
  expectedCompletionAt: string | null;
  snoozeUntil: string | null;
  lastNotifiedReviewAt: string | null;
  endedAt: string | null;
  createdAt: string;
  updatedAt: string;
  latestProgress: FollowUpProgress | null;
  progress: FollowUpProgress[];
  links: FollowUpLink[];
}
export interface FollowUpDraft {
  id: number | null;
  title: string;
  description: string;
  expectedOutcome: string;
  priority: TodoPriority;
  personId: number | null;
  reviewAt: string;
  expectedCompletionAt: string;
  links: FollowUpLink[];
}
export interface FollowUpFilters {
  keyword: string;
  personId: number | null;
  priority: TodoPriority | null;
  attentionStatus: FollowUpAttentionStatus | null;
}
export type FollowUpGroup = "due" | "soon" | "later" | "ended";
export type { TodoAssignee };

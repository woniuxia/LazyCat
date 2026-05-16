import type { SearchField } from "../utils/fuzzy-match";

export type SpotlightProviderId = "tool" | "vault" | "hosts" | "todo" | "pm" | "suggestion";

export type StatusTone = "danger" | "warn" | "info" | "success" | "muted" | "primary";

export interface SpotlightStatus {
  text: string;
  tone: StatusTone;
}

export interface SpotlightBadge {
  short: string;
  tone: StatusTone;
}

export interface SpotlightAction {
  id: string;
  label: string;
  icon?: string;
  danger?: boolean;
  needsConfirm?: boolean;
  needsMasterPassword?: boolean;
  shortcut?: string;
}

export interface SpotlightItem {
  providerId: SpotlightProviderId;
  itemId: string;
  title: string;
  subtitle?: string;
  badge?: SpotlightBadge;
  status?: SpotlightStatus;
  searchFields: SearchField[];
  weight?: number;
  payload?: Record<string, unknown>;
}

export interface SpotlightExecuteResult {
  closeSpotlight?: boolean;
  toast?: { message: string; type?: "success" | "error" | "warning" | "info" };
  errorMessage?: string;
}

export interface SpotlightExecuteContext {
  query: string;
  requestMasterPassword: (entryTitle: string) => Promise<string | null>;
}

export interface SpotlightProvider {
  id: SpotlightProviderId;
  scopeKeys: string[];
  badgeShort: string;
  badgeTone: StatusTone;
  weight: number;
  prefetch: () => Promise<SpotlightItem[]>;
  defaultAction: (
    item: SpotlightItem,
    ctx: SpotlightExecuteContext,
  ) => Promise<SpotlightExecuteResult>;
  buildActions?: (item: SpotlightItem) => SpotlightAction[];
  executeAction?: (
    item: SpotlightItem,
    actionId: string,
    ctx: SpotlightExecuteContext,
  ) => Promise<SpotlightExecuteResult>;
}

export interface ScopeParseResult {
  scope: SpotlightProviderId | null;
  query: string;
}

export interface ScoredSpotlightItem {
  item: SpotlightItem;
  score: number;
}

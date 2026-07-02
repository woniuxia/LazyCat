import type { SearchField } from "../utils/fuzzy-match";

export type SpotlightProviderId =
  | "tool"
  | "vault"
  | "hosts"
  | "todo"
  | "pm"
  | "data-dictionary"
  | "browser-profiles"
  | "suggestion"
  | "launcher"
  | "__keyword__";

export type QuickCommandId = "todo-create" | "calc" | "calc-eq";

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
  ensureVaultUnlocked: (entryTitle: string) => Promise<boolean>;
}

export interface SpotlightSearchContext {
  scope: SpotlightProviderId | null;
}

export interface QuickCommandDescriptor {
  id: QuickCommandId;
  name: string;
  description: string;
  trigger:
    | { type: "prefix"; value: string }
    | { type: "keyword"; value: string };
  defaultEnabled: boolean;
}

export interface ProviderDescriptor {
  id: SpotlightProviderId;
  name: string;
  description: string;
  badgeShort: string;
  badgeTone: StatusTone;
  weight: number;
  defaultAliases: string[];
  defaultEnabled: boolean;
  hiddenInSettings?: boolean;
  quickCommands?: QuickCommandDescriptor[];
  prefetch: () => Promise<SpotlightItem[]>;
  search?: (
    query: string,
    ctx: SpotlightSearchContext,
  ) => Promise<SpotlightItem[]>;
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

export interface SpotlightConfigProviderOverride {
  enabled?: boolean;
  aliases?: string[];
}

export interface SpotlightConfigQuickCommandOverride {
  enabled?: boolean;
}

export interface SpotlightConfig {
  version: 1;
  providers: Partial<Record<SpotlightProviderId, SpotlightConfigProviderOverride>>;
  quickCommands: Partial<Record<QuickCommandId, SpotlightConfigQuickCommandOverride>>;
  keywordCommands?: SpotlightConfigKeywordCommands;
}

export type ResolvedProvider = ProviderDescriptor & {
  enabled: boolean;
  aliases: string[];
};

export interface SpotlightView {
  providers: ResolvedProvider[];
  aliasMap: Map<string, SpotlightProviderId>;
  enabledQuickCommands: Set<QuickCommandId>;
  quickCommands: QuickCommandDescriptor[];
  keywordCommands: KeywordCommandDescriptor[];
  keywordIndex: Map<string, KeywordCommandDescriptor>;
}

// ── KeywordCommand ─────────────────────────────────────────────────────
//
// `;<keyword> [args]` 触发的用户级命令。与 ScopeAlias (裸前缀+空格)
// 和 QuickCommand (`+ ` / `calc`) 三条命名空间互不重叠。

export type KeywordCommandKind = "open-tool" | "show-value" | "vault-tag" | "snippet-tag";

export type KeywordValueProducerId =
  | "local-ip"
  | "uuid-v4"
  | "timestamp-now"
  | "hash-text";

export interface KeywordCommandDescriptor {
  id: string;
  keyword: string;
  name: string;
  description: string;
  kind: KeywordCommandKind;
  origin: "builtin" | "custom";
  // open-tool
  toolId?: string;
  forwardArgs?: boolean;
  // vault-tag / snippet-tag
  targetTag?: string;
  // show-value(内置专用)
  valueProducer?: KeywordValueProducerId;
  defaultEnabled: boolean;
}

export interface KeywordCommandCustom {
  id: string;
  keyword: string;
  name: string;
  description: string;
  kind: "open-tool" | "vault-tag" | "snippet-tag";
  toolId?: string;
  forwardArgs?: boolean;
  targetTag?: string;
  enabled: boolean;
}

export interface SpotlightConfigKeywordCommands {
  builtins?: Partial<Record<string, { enabled: boolean }>>;
  custom?: KeywordCommandCustom[];
}

export interface KeywordCommandInvocation {
  kind: "keyword";
  command: KeywordCommandDescriptor;
  args: string;
}

export interface ScopeParseResult {
  scope: SpotlightProviderId | null;
  query: string;
}

export interface ScoredSpotlightItem {
  item: SpotlightItem;
  score: number;
}

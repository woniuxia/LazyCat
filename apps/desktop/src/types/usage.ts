export interface UsageSummary {
  totalCount: number;
  windowCount: number;
  lastUsedAt: number | null;
  actionCounts: Record<string, number>;
}

export type ToolUsageMap = Record<string, UsageSummary>;

export interface UsageRef {
  resourceType: string;
  scopeId?: string;
  resourceId: string;
  actions: string[];
}

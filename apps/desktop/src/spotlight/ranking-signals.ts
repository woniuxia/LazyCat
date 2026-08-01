import type { UsageRef, UsageSummary } from "../types/usage";
import type { SpotlightItem } from "./types";

export function usageRefKey(ref: UsageRef): string {
  return JSON.stringify([
    ref.resourceType,
    ref.scopeId ?? "",
    ref.resourceId,
    [...ref.actions].sort(),
  ]);
}

export function normalizeUsageScore(summary: UsageSummary | undefined, now = Date.now()): number {
  if (!summary) return 0;
  const historicalCount = Math.max(0, summary.totalCount - summary.windowCount);
  const effectiveCount = Math.max(0, summary.windowCount) + Math.min(historicalCount, 30) * 0.15;
  const frequency = Math.min(1, Math.log1p(effectiveCount) / Math.log1p(30));
  const ageDays =
    summary.lastUsedAt == null
      ? Number.POSITIVE_INFINITY
      : Math.max(0, now - summary.lastUsedAt) / 86_400_000;
  const recency = Number.isFinite(ageDays) ? Math.exp(-ageDays / 14) : 0;
  return frequency * 0.75 + recency * 0.25;
}

export function itemUsageSummary(
  item: SpotlightItem,
  summaries: ReadonlyMap<string, UsageSummary>,
): UsageSummary | undefined {
  const ref = item.ranking?.usageRef;
  return ref ? summaries.get(usageRefKey(ref)) : undefined;
}

export function businessScore(item: SpotlightItem): number {
  const ranking = item.ranking;
  if (!ranking) return 0;
  return Math.min(
    1,
    Number(ranking.favorite === true) * 0.6 + Number(ranking.pinned === true) * 0.4,
  );
}

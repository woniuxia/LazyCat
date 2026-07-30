import type { UsageRef, UsageSummary } from "../types/usage";
import type {
  ProviderDescriptor,
  ScoredSpotlightItem,
  SpotlightItem,
  SpotlightProviderId,
} from "./types";

const MAX_USAGE_BOOST = 48;
const MAX_BUSINESS_BOOST = 28;

export function usageRefKey(ref: UsageRef): string {
  return JSON.stringify([
    ref.resourceType,
    ref.scopeId ?? "",
    ref.resourceId,
    [...ref.actions].sort(),
  ]);
}

export function normalizeUsageScore(
  summary: UsageSummary | undefined,
  now = Date.now(),
): number {
  if (!summary) return 0;
  const historicalCount = Math.max(0, summary.totalCount - summary.windowCount);
  const effectiveCount = Math.max(0, summary.windowCount) + Math.min(historicalCount, 30) * 0.15;
  const frequency = Math.min(1, Math.log1p(effectiveCount) / Math.log1p(30));
  const ageDays = summary.lastUsedAt == null
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
    Number(ranking.favorite === true) * 0.55 +
      Number(ranking.pinned === true) * 0.35 +
      Number(ranking.enabled === true) * 0.1,
  );
}

export function rankSearchCandidate(
  relevance: number,
  item: SpotlightItem,
  summaries: ReadonlyMap<string, UsageSummary>,
): number {
  return relevance +
    normalizeUsageScore(itemUsageSummary(item, summaries)) * MAX_USAGE_BOOST +
    businessScore(item) * MAX_BUSINESS_BOOST;
}

export function rankEmptyItems(
  itemsByProvider: Map<SpotlightProviderId, SpotlightItem[]>,
  providers: readonly ProviderDescriptor[],
  summaries: ReadonlyMap<string, UsageSummary>,
  limit: number,
): ScoredSpotlightItem[] {
  const providerOrder = new Map(providers.map((provider, index) => [provider.id, index]));
  const providerMap = new Map(providers.map((provider) => [provider.id, provider]));
  const candidates = [...itemsByProvider.entries()].flatMap(([providerId, items]) =>
    items.map((item, index) => {
      const usage = itemUsageSummary(item, summaries);
      const usageScore = normalizeUsageScore(usage);
      const score = businessScore(item) * 2 + usageScore;
      return {
        item,
        score,
        contextual: item.ranking?.contextual === true,
        usageScore,
        lastUsedAt: usage?.lastUsedAt ?? 0,
        sourceOrder: item.ranking?.sourceOrder ?? index,
        providerOrder: providerOrder.get(providerId) ?? Number.MAX_SAFE_INTEGER,
      };
    }),
  );

  candidates.sort((left, right) =>
    Number(right.contextual) - Number(left.contextual) ||
    right.score - left.score ||
    right.usageScore - left.usageScore ||
    right.lastUsedAt - left.lastUsedAt ||
    left.providerOrder - right.providerOrder ||
    left.sourceOrder - right.sourceOrder ||
    left.item.itemId.localeCompare(right.item.itemId),
  );

  const counts = new Map<SpotlightProviderId, number>();
  const result: ScoredSpotlightItem[] = [];
  for (const candidate of candidates) {
    const providerId = candidate.item.providerId;
    const quota = providerMap.get(providerId)?.emptyQueryQuota ?? (providerId === "tool" ? 8 : 2);
    const count = counts.get(providerId) ?? 0;
    if (count >= quota) continue;
    counts.set(providerId, count + 1);
    result.push({ item: candidate.item, score: candidate.score });
    if (result.length >= limit) break;
  }
  return result;
}

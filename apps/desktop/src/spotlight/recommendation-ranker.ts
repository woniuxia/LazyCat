import type { UsageSummary } from "../types/usage";
import { businessScore, itemUsageSummary, normalizeUsageScore } from "./ranking-signals";
import type {
  ProviderDescriptor,
  ScoredSpotlightItem,
  SpotlightItem,
  SpotlightProviderId,
} from "./types";

export class RecommendationRanker {
  private readonly providerOrder: Map<SpotlightProviderId, number>;
  private readonly providerMap: Map<SpotlightProviderId, ProviderDescriptor>;

  constructor(
    providers: readonly ProviderDescriptor[],
    private readonly summaries: ReadonlyMap<string, UsageSummary>,
  ) {
    this.providerOrder = new Map(providers.map((provider, index) => [provider.id, index]));
    this.providerMap = new Map(providers.map((provider) => [provider.id, provider]));
  }

  rank(
    itemsByProvider: Map<SpotlightProviderId, SpotlightItem[]>,
    limit: number,
  ): ScoredSpotlightItem[] {
    const candidates = [...itemsByProvider.entries()].flatMap(([providerId, items]) =>
      items.map((item, index) => {
        const usage = itemUsageSummary(item, this.summaries);
        const usageScore = normalizeUsageScore(usage);
        const contextual = item.ranking?.contextual === true;
        return {
          item,
          score: Number(contextual) * 3 + businessScore(item) * 2 + usageScore,
          contextual,
          usageScore,
          lastUsedAt: usage?.lastUsedAt ?? 0,
          sourceOrder: item.ranking?.sourceOrder ?? index,
          providerOrder: this.providerOrder.get(providerId) ?? Number.MAX_SAFE_INTEGER,
        };
      }),
    );

    candidates.sort(
      (left, right) =>
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
      const quota =
        this.providerMap.get(providerId)?.emptyQueryQuota ?? (providerId === "tool" ? 8 : 2);
      const count = counts.get(providerId) ?? 0;
      if (count >= quota) continue;
      counts.set(providerId, count + 1);
      result.push({ item: candidate.item, score: candidate.score });
      if (result.length >= limit) break;
    }
    return result;
  }
}

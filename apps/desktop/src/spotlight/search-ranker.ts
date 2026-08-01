import type { UsageSummary } from "../types/usage";
import { businessScore, itemUsageSummary, normalizeUsageScore } from "./ranking-signals";
import type { ScoredSpotlightItem, SpotlightItem } from "./types";

const MAX_SEARCH_USAGE_BOOST = 12;
const MAX_SEARCH_BUSINESS_BOOST = 6;
const SEARCH_CONTEXTUAL_BOOST = 2;

interface RankedSearchItem extends ScoredSpotlightItem {
  relevance: number;
}

export class SearchRanker {
  constructor(private readonly summaries: ReadonlyMap<string, UsageSummary>) {}

  rank(item: SpotlightItem, localRelevance: number): RankedSearchItem | null {
    const backendRelevance = Number.isFinite(item.recallScore) ? (item.recallScore ?? 0) : 0;
    const relevance = Math.max(localRelevance, backendRelevance);
    if (relevance <= 0) return null;

    const usageBoost =
      normalizeUsageScore(itemUsageSummary(item, this.summaries)) * MAX_SEARCH_USAGE_BOOST;
    const businessBoost = businessScore(item) * MAX_SEARCH_BUSINESS_BOOST;
    const contextualBoost = item.ranking?.contextual === true ? SEARCH_CONTEXTUAL_BOOST : 0;
    return {
      item,
      relevance,
      score: relevance + usageBoost + businessBoost + contextualBoost,
    };
  }

  static compare(left: RankedSearchItem, right: RankedSearchItem): number {
    return (
      right.score - left.score ||
      right.relevance - left.relevance ||
      left.item.providerId.localeCompare(right.item.providerId) ||
      left.item.itemId.localeCompare(right.item.itemId)
    );
  }
}

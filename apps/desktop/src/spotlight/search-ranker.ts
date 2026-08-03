import type { UsageSummary } from "../types/usage";
import { normalizeSearchText } from "../utils/fuzzy-match";
import { businessScore, itemUsageSummary, normalizeUsageScore } from "./ranking-signals";
import type { ScoredSpotlightItem, SpotlightItem } from "./types";

const MAX_SEARCH_RELEVANCE = 2000;
const SEARCH_RELEVANCE_WEIGHT = 0.7;
const SEARCH_USAGE_WEIGHT = 0.2;
const SEARCH_BUSINESS_WEIGHT = 0.08;
const SEARCH_CONTEXTUAL_WEIGHT = 0.02;

interface RankedSearchItem extends ScoredSpotlightItem {
  relevance: number;
  exactTitleMatch: boolean;
}

export class SearchRanker {
  constructor(private readonly summaries: ReadonlyMap<string, UsageSummary>) {}

  rank(
    item: SpotlightItem,
    localRelevance: number,
    normalizedQuery: string,
  ): RankedSearchItem | null {
    const backendRelevance = Number.isFinite(item.recallScore) ? (item.recallScore ?? 0) : 0;
    const relevance = Math.max(localRelevance, backendRelevance);
    if (relevance <= 0) return null;

    // Keep the relevance reference scale explicit so other signals remain effective.
    const normalizedRelevance = Math.min(1, Math.max(0, relevance) / MAX_SEARCH_RELEVANCE);
    const usageScore = normalizeUsageScore(itemUsageSummary(item, this.summaries));
    const itemBusinessScore = businessScore(item);
    const contextualScore = Number(item.ranking?.contextual === true);
    return {
      item,
      relevance,
      exactTitleMatch:
        normalizedQuery.length > 0 && normalizeSearchText(item.title) === normalizedQuery,
      score:
        normalizedRelevance * SEARCH_RELEVANCE_WEIGHT +
        usageScore * SEARCH_USAGE_WEIGHT +
        itemBusinessScore * SEARCH_BUSINESS_WEIGHT +
        contextualScore * SEARCH_CONTEXTUAL_WEIGHT,
    };
  }

  static compare(left: RankedSearchItem, right: RankedSearchItem): number {
    return (
      Number(right.exactTitleMatch) - Number(left.exactTitleMatch) ||
      right.score - left.score ||
      right.relevance - left.relevance ||
      left.item.providerId.localeCompare(right.item.providerId) ||
      left.item.itemId.localeCompare(right.item.itemId)
    );
  }
}

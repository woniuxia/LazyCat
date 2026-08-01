import type {
  ProviderDescriptor,
  ScoredSpotlightItem,
  SpotlightItem,
  SpotlightProviderId,
} from "./types";
import { matchPreparedQuery, prepareSearchQuery } from "../utils/fuzzy-match";
import type { UsageSummary } from "../types/usage";
import { SearchRanker } from "./search-ranker";
import { toolProvider } from "./providers/tool";

const DESCRIPTORS = new Map<SpotlightProviderId, ProviderDescriptor>();

export function registerProvider(descriptor: ProviderDescriptor): void {
  DESCRIPTORS.set(descriptor.id, descriptor);
}

export function getProvider(id: SpotlightProviderId): ProviderDescriptor | undefined {
  return DESCRIPTORS.get(id);
}

export function getDescriptor(id: SpotlightProviderId): ProviderDescriptor | undefined {
  return DESCRIPTORS.get(id);
}

export function listProviders(): ProviderDescriptor[] {
  return Array.from(DESCRIPTORS.values());
}

export function listDescriptors(includeHidden = false): ProviderDescriptor[] {
  const all = Array.from(DESCRIPTORS.values());
  return includeHidden ? all : all.filter((d) => !d.hiddenInSettings);
}

// 默认注册：工具源。其它 provider 由其模块导入时主动注册。
registerProvider(toolProvider);

export function searchItems(
  query: string,
  itemsByProvider: Map<SpotlightProviderId, SpotlightItem[]>,
  options: {
    scope: SpotlightProviderId | null;
    limit: number;
    enabledIds?: Set<SpotlightProviderId>;
    usageSummaries?: ReadonlyMap<string, UsageSummary>;
  },
): ScoredSpotlightItem[] {
  const { scope, limit, enabledIds, usageSummaries = new Map() } = options;
  const queryIndex = prepareSearchQuery(query);
  if (queryIndex.tokens.length === 0) return [];
  const ranker = new SearchRanker(usageSummaries);
  const scored: Array<NonNullable<ReturnType<SearchRanker["rank"]>>> = [];

  for (const provider of listProviders()) {
    if (scope && provider.id !== scope) continue;
    if (enabledIds && !enabledIds.has(provider.id)) continue;
    const items = itemsByProvider.get(provider.id) ?? [];
    for (const item of items) {
      const relevance = matchPreparedQuery(queryIndex, item.searchFields);
      const ranked = ranker.rank(item, relevance);
      if (ranked) scored.push(ranked);
    }
  }

  scored.sort(SearchRanker.compare);
  return scored.slice(0, limit);
}

import type { ScoredSpotlightItem, SpotlightItem, SpotlightProvider, SpotlightProviderId } from "./types";
import { matchScore } from "../utils/fuzzy-match";
import { toolProvider } from "./providers/tool";

const PROVIDERS = new Map<SpotlightProviderId, SpotlightProvider>();

export function registerProvider(provider: SpotlightProvider): void {
  PROVIDERS.set(provider.id, provider);
}

export function getProvider(id: SpotlightProviderId): SpotlightProvider | undefined {
  return PROVIDERS.get(id);
}

export function listProviders(): SpotlightProvider[] {
  return Array.from(PROVIDERS.values());
}

// 默认注册：工具源。其它 provider 由其模块导入时主动注册。
registerProvider(toolProvider);

export function scoreItem(query: string, item: SpotlightItem, providerWeight: number): number {
  if (!query.trim()) return 0;
  const baseScore = matchScore(query, item.searchFields);
  if (baseScore <= 0) return 0;
  const itemWeight = item.weight ?? 1;
  return baseScore * providerWeight * itemWeight;
}

export function searchItems(
  query: string,
  itemsByProvider: Map<SpotlightProviderId, SpotlightItem[]>,
  options: { scope: SpotlightProviderId | null; limit: number },
): ScoredSpotlightItem[] {
  const { scope, limit } = options;
  const scored: ScoredSpotlightItem[] = [];

  for (const provider of listProviders()) {
    if (scope && provider.id !== scope) continue;
    const items = itemsByProvider.get(provider.id) ?? [];
    for (const item of items) {
      const score = scoreItem(query, item, provider.weight);
      if (score > 0) {
        scored.push({ item, score });
      }
    }
  }

  scored.sort((a, b) => b.score - a.score);
  return scored.slice(0, limit);
}

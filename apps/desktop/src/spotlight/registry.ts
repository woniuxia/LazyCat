import type {
  ProviderDescriptor,
  ScoredSpotlightItem,
  SpotlightItem,
  SpotlightProviderId,
} from "./types";
import { matchScore } from "../utils/fuzzy-match";
import type { UsageSummary } from "../types/usage";
import { rankSearchCandidate } from "./ranking";
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

export function scoreItem(
  query: string,
  item: SpotlightItem,
  _providerWeight: number,
  usageSummaries: ReadonlyMap<string, UsageSummary> = new Map(),
): number {
  if (!query.trim()) return 0;
  const baseScore = matchScore(query, item.searchFields);
  if (baseScore <= 0) return 0;
  return rankSearchCandidate(baseScore, item, usageSummaries);
}

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
  const scored: ScoredSpotlightItem[] = [];

  for (const provider of listProviders()) {
    if (scope && provider.id !== scope) continue;
    if (enabledIds && !enabledIds.has(provider.id)) continue;
    const items = itemsByProvider.get(provider.id) ?? [];
    for (const item of items) {
      const score = scoreItem(query, item, provider.weight, usageSummaries);
      if (score > 0) {
        scored.push({ item, score });
      }
    }
  }

  scored.sort((a, b) =>
    Number(b.item.ranking?.contextual === true) - Number(a.item.ranking?.contextual === true) ||
    b.score - a.score ||
    a.item.providerId.localeCompare(b.item.providerId) ||
    a.item.itemId.localeCompare(b.item.itemId),
  );
  return scored.slice(0, limit);
}

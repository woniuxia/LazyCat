import type { SpotlightItem, SpotlightProviderId } from "./types";

export function shouldRunQueryProvider(
  query: string,
  scope: SpotlightProviderId | null,
  providerId: SpotlightProviderId,
): boolean {
  const text = query.trim();
  if (!text) return false;
  if (scope) return scope === providerId && text.length >= 1;
  return text.length >= 2;
}

export function mergeSpotlightProviderItems(
  prefetched: Map<SpotlightProviderId, SpotlightItem[]>,
  queryTime: Map<SpotlightProviderId, SpotlightItem[]>,
): Map<SpotlightProviderId, SpotlightItem[]> {
  const merged = new Map<SpotlightProviderId, SpotlightItem[]>();
  const providerIds = new Set<SpotlightProviderId>([...prefetched.keys(), ...queryTime.keys()]);

  for (const providerId of providerIds) {
    const seen = new Set<string>();
    const items: SpotlightItem[] = [];
    for (const source of [prefetched.get(providerId) ?? [], queryTime.get(providerId) ?? []]) {
      for (const item of source) {
        const key = `${item.providerId}:${item.itemId}`;
        if (seen.has(key)) continue;
        seen.add(key);
        items.push(item);
      }
    }
    merged.set(providerId, items);
  }

  return merged;
}

export function createQueryTimeResultGuard() {
  let seq = 0;
  let latestQuery = "";
  let latestScope: SpotlightProviderId | null = null;
  return {
    next(query: string, scope: SpotlightProviderId | null): number {
      seq += 1;
      latestQuery = query;
      latestScope = scope;
      return seq;
    },
    isCurrent(requestSeq: number, query: string, scope: SpotlightProviderId | null): boolean {
      if (requestSeq !== seq) return false;
      return query === latestQuery && scope === latestScope;
    },
  };
}

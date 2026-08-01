import { invokeToolByChannel } from "../bridge/tauri";
import type { UsageRef, UsageSummary } from "../types/usage";
import { usageRefKey } from "./ranking-signals";
import type { SpotlightItem, SpotlightProviderId } from "./types";

export type SpotlightItemsMap = Map<SpotlightProviderId, SpotlightItem[]>;

const USAGE_SUMMARY_BATCH_SIZE = 256;

export function collectUsageRefs(itemsByProvider: SpotlightItemsMap): Map<string, UsageRef> {
  const refs = new Map<string, UsageRef>();
  for (const items of itemsByProvider.values()) {
    for (const item of items) {
      const usageRef = item.ranking?.usageRef;
      if (usageRef) refs.set(usageRefKey(usageRef), usageRef);
    }
  }
  return refs;
}

export function usageRefsSignature(refs: ReadonlyMap<string, UsageRef>): string {
  return [...refs.keys()].sort().join("\n");
}

export async function loadUsageSummaries(
  refs: ReadonlyMap<string, UsageRef>,
): Promise<Map<string, UsageSummary>> {
  const values = [...refs.values()];
  const batches: UsageRef[][] = [];
  for (let index = 0; index < values.length; index += USAGE_SUMMARY_BATCH_SIZE) {
    batches.push(values.slice(index, index + USAGE_SUMMARY_BATCH_SIZE));
  }

  const responses = await Promise.all(
    batches.map((batch) => invokeToolByChannel("tool:usage:summaries", { refs: batch })) as Array<
      Promise<{ items: Array<UsageRef & { summary: UsageSummary }> }>
    >,
  );
  return new Map(
    responses
      .flatMap((response) => response.items)
      .map((item) => [usageRefKey(item), item.summary]),
  );
}

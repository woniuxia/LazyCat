import { describe, expect, it } from "vitest";
import type { SpotlightItem, SpotlightProviderId } from "./types";
import {
  createQueryTimeResultGuard,
  mergeSpotlightProviderItems,
  shouldRunQueryProvider,
} from "./search";

function item(providerId: SpotlightItem["providerId"], itemId: string): SpotlightItem {
  return {
    providerId,
    itemId,
    title: `${providerId}:${itemId}`,
    searchFields: [],
  };
}

describe("shouldRunQueryProvider", () => {
  it("does not run query provider for empty input", () => {
    expect(shouldRunQueryProvider("", null, "data-dictionary")).toBe(false);
    expect(shouldRunQueryProvider("   ", "data-dictionary", "data-dictionary")).toBe(false);
  });

  it("requires two characters in global search", () => {
    expect(shouldRunQueryProvider("a", null, "data-dictionary")).toBe(false);
    expect(shouldRunQueryProvider("ab", null, "data-dictionary")).toBe(true);
  });

  it("allows one character when scoped to the same provider", () => {
    expect(shouldRunQueryProvider("a", "data-dictionary", "data-dictionary")).toBe(true);
    expect(shouldRunQueryProvider("a", "todo", "data-dictionary")).toBe(false);
  });
});

describe("mergeSpotlightProviderItems", () => {
  it("keeps query-time items and dedupes by provider item key", () => {
    const prefetched: Map<SpotlightProviderId, SpotlightItem[]> = new Map([
      ["tool", [item("tool", "json")]],
      ["todo", [item("todo", "1")]],
    ]);
    const queryTime: Map<SpotlightProviderId, SpotlightItem[]> = new Map([
      ["todo", [item("todo", "1"), item("todo", "2")]],
      ["data-dictionary", [item("data-dictionary", "9")]],
    ]);

    const merged = mergeSpotlightProviderItems(prefetched, queryTime);

    expect(merged.get("tool")?.map((entry) => entry.itemId)).toEqual(["json"]);
    expect(merged.get("todo")?.map((entry) => entry.itemId)).toEqual(["1", "2"]);
    expect(merged.get("data-dictionary")?.map((entry) => entry.itemId)).toEqual(["9"]);
  });
});

describe("createQueryTimeResultGuard", () => {
  it("accepts only the latest query signature", () => {
    const guard = createQueryTimeResultGuard();
    const first = guard.next("a", null);
    const second = guard.next("ab", null);

    expect(guard.isCurrent(first, "a", null)).toBe(false);
    expect(guard.isCurrent(second, "ab", null)).toBe(true);
    expect(guard.isCurrent(second, "ab", "data-dictionary")).toBe(false);
  });
});

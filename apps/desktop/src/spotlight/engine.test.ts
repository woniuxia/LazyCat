import { afterEach, describe, expect, it, vi } from "vitest";
import type { UsageSummary } from "../types/usage";
import type { ProviderDescriptor, SpotlightItem } from "./types";
import { createSpotlightEngine } from "./engine";
import { usageRefKey } from "./ranking-signals";

function item(title: string): SpotlightItem {
  return {
    providerId: "tool",
    itemId: title,
    title,
    searchFields: [{ text: title, initials: "", weight: 1 }],
  };
}

function provider(overrides: Partial<ProviderDescriptor> = {}): ProviderDescriptor {
  return {
    id: "tool",
    name: "工具",
    description: "工具",
    badgeShort: "工具",
    badgeTone: "primary",
    defaultAliases: [],
    defaultEnabled: true,
    prefetch: async () => [],
    defaultAction: async () => ({}),
    ...overrides,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

afterEach(() => {
  vi.useRealTimers();
});

describe("SpotlightEngine provider snapshots", () => {
  it("retains the last successful snapshot when a refresh fails", async () => {
    const prefetch = vi
      .fn<() => Promise<SpotlightItem[]>>()
      .mockResolvedValueOnce([item("stable")])
      .mockRejectedValueOnce(new Error("offline"));
    const engine = createSpotlightEngine();
    const source = provider({ prefetch });

    expect(await engine.refreshProvider(source)).toBe(true);
    expect(await engine.refreshProvider(source)).toBe(false);

    expect(engine.prefetchedItems.value.get("tool")?.[0]?.title).toBe("stable");
    expect([...engine.providerErrors.value.values()][0]).toMatchObject({
      sourceId: "tool",
      phase: "prefetch",
      message: "offline",
    });
    engine.dispose();
  });

  it("prevents an older provider response from overwriting a newer snapshot", async () => {
    const first = deferred<SpotlightItem[]>();
    const second = deferred<SpotlightItem[]>();
    const source = provider({
      prefetch: vi
        .fn<() => Promise<SpotlightItem[]>>()
        .mockReturnValueOnce(first.promise)
        .mockReturnValueOnce(second.promise),
    });
    const engine = createSpotlightEngine();

    const oldRequest = engine.refreshProvider(source);
    const newRequest = engine.refreshProvider(source);
    second.resolve([item("new")]);
    await newRequest;
    first.resolve([item("old")]);
    await oldRequest;

    expect(engine.prefetchedItems.value.get("tool")?.[0]?.title).toBe("new");
    engine.dispose();
  });
});

describe("SpotlightEngine query scheduling", () => {
  it("debounces query providers and runs only the latest intent", async () => {
    vi.useFakeTimers();
    const search = vi.fn(async (query: string) => [item(query)]);
    const source = provider({ search });
    const engine = createSpotlightEngine({ queryDebounceMs: 120 });

    engine.scheduleQueryRefresh("ab", null, [source]);
    await vi.advanceTimersByTimeAsync(60);
    engine.scheduleQueryRefresh("abc", null, [source]);
    await vi.advanceTimersByTimeAsync(120);

    expect(search).toHaveBeenCalledTimes(1);
    expect(search.mock.calls[0]?.[0]).toBe("abc");
    expect(engine.queryItems.value.get("tool")?.[0]?.title).toBe("abc");
    engine.dispose();
  });
});

describe("SpotlightEngine usage summaries", () => {
  it("preserves the last successful summaries when a refresh fails", async () => {
    const usageRef = { resourceType: "tool", resourceId: "json", actions: ["open"] };
    const summary: UsageSummary = {
      totalCount: 4,
      windowCount: 4,
      lastUsedAt: 100,
      actionCounts: { open: 4 },
    };
    const loadUsage = vi
      .fn()
      .mockResolvedValueOnce(new Map([[usageRefKey(usageRef), summary]]))
      .mockRejectedValueOnce(new Error("usage unavailable"));
    const engine = createSpotlightEngine({ loadUsage });
    const items = new Map([
      [
        "tool" as const,
        [
          {
            ...item("json"),
            ranking: { usageRef },
          },
        ],
      ],
    ]);

    await engine.refreshUsage(items, true);
    await engine.refreshUsage(items, true);

    expect(engine.usageSummaries.value.get(usageRefKey(usageRef))).toEqual(summary);
    expect([...engine.providerErrors.value.values()][0]).toMatchObject({
      sourceId: "usage",
      phase: "usage",
    });
    engine.dispose();
  });
});

import { describe, expect, it } from "vitest";
import type { UsageSummary } from "../types/usage";
import type { ProviderDescriptor, SpotlightItem } from "./types";
import { RecommendationRanker } from "./recommendation-ranker";
import { businessScore, normalizeUsageScore, usageRefKey } from "./ranking-signals";
import { SearchRanker } from "./search-ranker";

const now = Date.UTC(2026, 6, 30);

function summary(overrides: Partial<UsageSummary> = {}): UsageSummary {
  return {
    totalCount: 0,
    windowCount: 0,
    lastUsedAt: null,
    actionCounts: {},
    ...overrides,
  };
}

function item(id: string, providerId: SpotlightItem["providerId"] = "tool"): SpotlightItem {
  return {
    providerId,
    itemId: id,
    title: id,
    searchFields: [],
    ranking: {
      usageRef: {
        resourceType: providerId === "launcher" ? "launcher-entry" : "tool",
        resourceId: id,
        actions: [providerId === "launcher" ? "launch" : "open"],
      },
    },
  };
}

function provider(id: ProviderDescriptor["id"], quota: number): ProviderDescriptor {
  return {
    id,
    name: id,
    description: id,
    badgeShort: id,
    badgeTone: "muted",
    emptyQueryQuota: quota,
    defaultAliases: [],
    defaultEnabled: true,
    prefetch: async () => [],
    defaultAction: async () => ({}),
  };
}

describe("Spotlight unified ranking", () => {
  it("uses live window activity first and keeps legacy totals as a bounded baseline", () => {
    const live = normalizeUsageScore(
      summary({ totalCount: 8, windowCount: 8, lastUsedAt: now }),
      now,
    );
    const legacy = normalizeUsageScore(summary({ totalCount: 100, windowCount: 0 }), now);

    expect(live).toBeGreaterThan(legacy);
    expect(legacy).toBeGreaterThan(0);
  });

  it("lets strong usage and business signals overtake a close relevance score", () => {
    const frequent = item("frequent");
    frequent.ranking!.favorite = true;
    const summaries = new Map([
      [
        usageRefKey(frequent.ranking!.usageRef!),
        summary({ totalCount: 500, windowCount: 500, lastUsedAt: now }),
      ],
    ]);

    const ranker = new SearchRanker(summaries);
    const frequentRank = ranker.rank(frequent, 970, "search")!;
    const relevantRank = ranker.rank(item("relevant"), 1000, "search")!;

    expect(frequentRank.score).toBeGreaterThan(relevantRank.score);
    expect([relevantRank, frequentRank].sort(SearchRanker.compare)[0].item.itemId).toBe("frequent");
  });

  it("keeps a clearly better relevance score ahead despite ranking signals", () => {
    const frequent = item("frequent");
    frequent.ranking!.favorite = true;
    const summaries = new Map([
      [
        usageRefKey(frequent.ranking!.usageRef!),
        summary({ totalCount: 500, windowCount: 500, lastUsedAt: now }),
      ],
    ]);

    const ranker = new SearchRanker(summaries);
    const frequentRank = ranker.rank(frequent, 970, "search")!;
    const relevantRank = ranker.rank(item("relevant"), 2000, "search")!;

    expect(relevantRank.score).toBeGreaterThan(frequentRank.score);
    expect([frequentRank, relevantRank].sort(SearchRanker.compare)[0].item.itemId).toBe("relevant");
  });

  it("protects a normalized exact title match from higher-scoring non-exact results", () => {
    const exact = item("exact");
    exact.title = "Open-API";
    const nonExact = item("non-exact");
    nonExact.title = "Open API docs";
    nonExact.ranking!.favorite = true;
    nonExact.ranking!.pinned = true;
    const summaries = new Map([
      [
        usageRefKey(nonExact.ranking!.usageRef!),
        summary({ totalCount: 500, windowCount: 500, lastUsedAt: now }),
      ],
    ]);

    const ranker = new SearchRanker(summaries);
    const exactRank = ranker.rank(exact, 700, "open api")!;
    const nonExactRank = ranker.rank(nonExact, 2000, "open api")!;

    expect(nonExactRank.score).toBeGreaterThan(exactRank.score);
    expect([nonExactRank, exactRank].sort(SearchRanker.compare)[0].item.itemId).toBe("exact");
  });

  it("does not treat an already enabled Hosts profile as a positive recommendation signal", () => {
    const hosts = item("hosts", "hosts");
    hosts.ranking = {};

    expect(businessScore(hosts)).toBe(0);
  });

  it("applies recommendation eligibility only to empty-query ranking", () => {
    const hiddenTodo = item("hidden-todo", "todo");
    hiddenTodo.ranking!.recommendationEligible = false;
    hiddenTodo.ranking!.pinned = true;
    const visibleTodo = item("visible-todo", "todo");
    const summaries = new Map([
      [
        usageRefKey(hiddenTodo.ranking!.usageRef!),
        summary({ totalCount: 30, windowCount: 30, lastUsedAt: now }),
      ],
    ]);

    const recommendations = new RecommendationRanker([provider("todo", 2)], summaries).rank(
      new Map([["todo", [hiddenTodo, visibleTodo]]]),
      2,
    );

    expect(recommendations.map((entry) => entry.item.itemId)).toEqual(["visible-todo"]);
    expect(new SearchRanker(summaries).rank(hiddenTodo, 1000, "hidden todo")?.item.itemId).toBe(
      "hidden-todo",
    );
  });

  it("applies global usage order with per-provider diversity quotas", () => {
    const toolA = item("tool-a");
    const toolB = item("tool-b");
    const launcher = item("launcher-a", "launcher");
    const summaries = new Map<string, UsageSummary>([
      [
        usageRefKey(toolA.ranking!.usageRef!),
        summary({ totalCount: 10, windowCount: 10, lastUsedAt: now }),
      ],
      [
        usageRefKey(toolB.ranking!.usageRef!),
        summary({ totalCount: 9, windowCount: 9, lastUsedAt: now }),
      ],
      [
        usageRefKey(launcher.ranking!.usageRef!),
        summary({ totalCount: 8, windowCount: 8, lastUsedAt: now }),
      ],
    ]);

    const result = new RecommendationRanker(
      [provider("tool", 1), provider("launcher", 1)],
      summaries,
    ).rank(
      new Map([
        ["tool", [toolA, toolB]],
        ["launcher", [launcher]],
      ]),
      3,
    );

    expect(result.map((entry) => entry.item.itemId)).toEqual(["tool-a", "launcher-a"]);
  });
});

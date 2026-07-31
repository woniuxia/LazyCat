import { describe, expect, it } from "vitest";
import type { UsageSummary } from "../types/usage";
import type { ProviderDescriptor, SpotlightItem } from "./types";
import { normalizeUsageScore, rankEmptyItems, rankSearchCandidate, usageRefKey } from "./ranking";

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
    weight: 1,
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

  it("caps usage and business boosts so relevance remains primary", () => {
    const frequent = item("frequent");
    frequent.ranking!.favorite = true;
    const summaries = new Map([
      [
        usageRefKey(frequent.ranking!.usageRef!),
        summary({ totalCount: 500, windowCount: 500, lastUsedAt: now }),
      ],
    ]);

    expect(rankSearchCandidate(980, frequent, summaries)).toBeLessThan(1200);
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

    const result = rankEmptyItems(
      new Map([
        ["tool", [toolA, toolB]],
        ["launcher", [launcher]],
      ]),
      [provider("tool", 1), provider("launcher", 1)],
      summaries,
      3,
    );

    expect(result.map((entry) => entry.item.itemId)).toEqual(["tool-a", "launcher-a"]);
  });
});

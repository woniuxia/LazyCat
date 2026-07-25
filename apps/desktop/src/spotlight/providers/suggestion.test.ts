import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke, showReferenceCard } = vi.hoisted(() => ({
  invoke: vi.fn(),
  showReferenceCard: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("../../bridge/tauri", () => ({ showReferenceCard }));

import type { SpotlightItem } from "../types";
import { suggestionProvider } from "./suggestion";

function createSuggestionItem(suggestionAction: unknown): SpotlightItem {
  return {
    providerId: "suggestion",
    itemId: "suggestion:test",
    title: "剪贴板建议",
    searchFields: [],
    payload: { suggestionAction },
  };
}

describe("suggestionProvider.defaultAction", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("直接通过 bridge 打开参考卡并关闭 Spotlight", async () => {
    const result = await suggestionProvider.defaultAction(
      createSuggestionItem({ kind: "open-reference-card", text: "demo" }),
      {} as never,
    );

    expect(showReferenceCard).toHaveBeenCalledWith("demo");
    expect(invoke).not.toHaveBeenCalledWith("spotlight_pick", expect.anything());
    expect(result).toEqual({ closeSpotlight: true });
  });

  it("沿用 spotlight_pick 执行工具建议并关闭 Spotlight", async () => {
    const result = await suggestionProvider.defaultAction(
      createSuggestionItem({ kind: "open-tool", toolId: "formatter", text: "demo" }),
      {} as never,
    );

    expect(invoke).toHaveBeenCalledWith("spotlight_pick", {
      target: "formatter",
      text: "demo",
      source: "clipboard-suggestion",
    });
    expect(showReferenceCard).not.toHaveBeenCalled();
    expect(result).toEqual({ closeSpotlight: true });
  });

  it("拒绝未知动作类型", async () => {
    const result = await suggestionProvider.defaultAction(
      createSuggestionItem({ kind: "unknown", text: "demo" }),
      {} as never,
    );

    expect(result.errorMessage).toContain("无效");
    expect(invoke).not.toHaveBeenCalled();
    expect(showReferenceCard).not.toHaveBeenCalled();
  });
});

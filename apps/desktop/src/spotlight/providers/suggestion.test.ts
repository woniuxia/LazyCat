import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke, showReferenceCard } = vi.hoisted(() => ({
  invoke: vi.fn(),
  showReferenceCard: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("../../bridge/tauri", () => ({ showReferenceCard }));

import type { SpotlightItem } from "../types";
import { MAX_REFERENCE_CARD_TEXT_BYTES } from "../../utils/monacoLanguages";
import { buildClipboardSuggestionItems } from "../clipboard-suggestions";
import { registerProvider, searchItems } from "../registry";
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
    registerProvider(suggestionProvider);
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

  it.each([
    ["虚构工具", { kind: "open-tool", toolId: "not-a-real-tool", text: "demo" }],
    ["空白参考文本", { kind: "open-reference-card", text: " \r\n " }],
    [
      "超限工具文本",
      {
        kind: "open-tool",
        toolId: "formatter",
        text: "a".repeat(MAX_REFERENCE_CARD_TEXT_BYTES + 1),
      },
    ],
  ])("拒绝%s", async (_name, action) => {
    const result = await suggestionProvider.defaultAction(
      createSuggestionItem(action),
      {} as never,
    );

    expect(result.errorMessage).toContain("无效");
    expect(invoke).not.toHaveBeenCalled();
    expect(showReferenceCard).not.toHaveBeenCalled();
  });

  it("不吞掉参考卡 bridge 的 rejection", async () => {
    showReferenceCard.mockRejectedValueOnce(new Error("bridge failed"));

    await expect(
      suggestionProvider.defaultAction(
        createSuggestionItem({ kind: "open-reference-card", text: "demo" }),
        {} as never,
      ),
    ).rejects.toThrow("bridge failed");
  });

  it.each(["参考", "置顶", "zdck", "reference"])("通过真实搜索路径用 %s 命中参考卡", (query) => {
    const items = buildClipboardSuggestionItems('{"port":8080}');
    const itemsByProvider = new Map([["suggestion" as const, items]]);

    const results = searchItems(query, itemsByProvider, {
      scope: null,
      limit: 9,
    });

    expect(results[0]?.item.itemId).toBe("suggestion:reference-card");
  });

  it("空查询建议按构建顺序和显式上下文顺序保持工具优先", () => {
    const items = buildClipboardSuggestionItems('{"port":8080}');

    expect(items.map((item) => item.itemId)).toEqual([
      "suggestion:tool:formatter",
      "suggestion:reference-card",
    ]);
    expect(items[0].ranking).toMatchObject({ contextual: true, sourceOrder: 0 });
    expect(items[1].ranking).toMatchObject({ contextual: true, sourceOrder: 1 });
  });
});

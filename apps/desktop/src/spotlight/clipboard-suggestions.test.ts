import { describe, expect, it } from "vitest";

import { MAX_REFERENCE_CARD_TEXT_BYTES } from "../utils/monacoLanguages";
import type { SpotlightItem } from "./types";
import {
  buildClipboardSuggestionItems,
  createClipboardSuggestionRefreshCoordinator,
  mergeClipboardSuggestionItems,
} from "./clipboard-suggestions";

function createItem(itemId: string, title: string): SpotlightItem {
  return {
    providerId: "suggestion",
    itemId,
    title,
    searchFields: [],
  };
}

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe("buildClipboardSuggestionItems", () => {
  it("为已识别 JSON 依次生成工具建议和参考卡建议", () => {
    const text = '{"port":8080}';

    const items = buildClipboardSuggestionItems(text);

    expect(items).toHaveLength(2);
    expect(items[0].payload?.suggestionAction).toEqual({
      kind: "open-tool",
      toolId: "formatter",
      text,
    });
    expect(items[1].payload?.suggestionAction).toEqual({
      kind: "open-reference-card",
      text,
    });
    expect(items.map((item) => item.ranking)).toEqual([
      { contextual: true, sourceOrder: 0 },
      { contextual: true, sourceOrder: 1 },
    ]);
  });

  it("为未识别的有效文本只生成参考卡建议", () => {
    const items = buildClipboardSuggestionItems("临时对照内容");

    expect(items).toHaveLength(1);
    expect(items[0].payload?.suggestionAction).toEqual({
      kind: "open-reference-card",
      text: "临时对照内容",
    });
  });

  it("为参考卡建议提供中英文搜索字段", () => {
    const [item] = buildClipboardSuggestionItems("临时对照内容");

    expect(item.searchFields.map((field) => field.text)).toEqual([
      "置顶参考卡",
      "参考",
      "置顶",
      "卡片",
      "clipboard",
      "reference",
    ]);
  });

  it("忽略空白文本和超过 8 MiB 的文本", () => {
    expect(buildClipboardSuggestionItems(" \r\n\t ")).toEqual([]);
    expect(
      buildClipboardSuggestionItems("a".repeat(MAX_REFERENCE_CARD_TEXT_BYTES + 1)),
    ).toEqual([]);
  });

  it("接近 8 MiB 的有效文本只生成带截断预览的参考卡", () => {
    const text = "a".repeat(MAX_REFERENCE_CARD_TEXT_BYTES);

    const items = buildClipboardSuggestionItems(text);

    expect(items).toHaveLength(1);
    expect(items[0].itemId).toBe("suggestion:reference-card");
    expect(items[0].title).toBe(`创建置顶参考卡（剪贴板：${"a".repeat(32)}…）`);
  });

  it("合并时保留已有建议并用同 key 的剪贴板项更新旧快照", () => {
    const existing = [
      createItem("existing", "已有建议"),
      createItem("suggestion:reference-card", "旧参考卡快照"),
    ];
    const clipboard = [createItem("suggestion:reference-card", "新参考卡快照")];

    const merged = mergeClipboardSuggestionItems(existing, clipboard);

    expect(merged.map((item) => item.title)).toEqual(["已有建议", "新参考卡快照"]);
  });

  it.each(["resolve", "reject"] as const)(
    "刷新只允许最新请求写回，旧请求 %s 不得覆盖",
    async (staleOutcome) => {
      const snapshots: SpotlightItem[][] = [];
      const coordinator = createClipboardSuggestionRefreshCoordinator((items) => {
        snapshots.push(items);
      });
      const stale = createDeferred<string | null>();
      const latest = createDeferred<string | null>();

      const staleRefresh = coordinator.refresh(() => stale.promise);
      expect(snapshots.at(-1)).toEqual([]);
      const latestRefresh = coordinator.refresh(() => latest.promise);
      latest.resolve("最新对照内容");
      await latestRefresh;
      const latestSnapshot = snapshots.at(-1);

      if (staleOutcome === "resolve") {
        stale.resolve("过期对照内容");
      } else {
        stale.reject(new Error("过期读取失败"));
      }
      await staleRefresh;

      expect(snapshots.at(-1)).toBe(latestSnapshot);
      expect(latestSnapshot?.[0].payload?.suggestionAction).toEqual({
        kind: "open-reference-card",
        text: "最新对照内容",
      });
    },
  );
});

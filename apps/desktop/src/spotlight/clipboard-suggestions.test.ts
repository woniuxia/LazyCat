import { describe, expect, it } from "vitest";

import { MAX_REFERENCE_CARD_TEXT_BYTES } from "../utils/monacoLanguages";
import { buildClipboardSuggestionItems } from "./clipboard-suggestions";

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
    expect(items[0].weight).toBeGreaterThan(items[1].weight ?? 0);
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
});

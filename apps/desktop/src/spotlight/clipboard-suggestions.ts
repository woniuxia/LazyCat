import { isRealToolId } from "../composables/toolCatalog";
import { detectClipboardContent } from "../utils/clipboard-detect";
import { toPinyinInitials } from "../utils/fuzzy-match";
import { validateReferenceCardText } from "../utils/monacoLanguages";
import type { SpotlightItem } from "./types";

export type ClipboardSuggestionAction =
  | { kind: "open-tool"; toolId: string; text: string }
  | { kind: "open-reference-card"; text: string };

const REFERENCE_CARD_SEARCH_TERMS = [
  "置顶参考卡",
  "参考",
  "置顶",
  "卡片",
  "clipboard",
  "reference",
] as const;

function buildPreview(text: string): string {
  const oneLine = text.replace(/\s+/g, " ").trim();
  return oneLine.length > 32 ? `${oneLine.slice(0, 32)}…` : oneLine;
}

export function buildClipboardSuggestionItems(text: string): SpotlightItem[] {
  if (!validateReferenceCardText(text).ok) return [];

  const preview = buildPreview(text);
  const detected = detectClipboardContent(text);
  const toolAction = detected?.actions.find(
    (action) => action.kind === "tool" && isRealToolId(action.toolId),
  );
  const items: SpotlightItem[] = [];

  if (toolAction?.kind === "tool") {
    const suggestionAction: ClipboardSuggestionAction = {
      kind: "open-tool",
      toolId: toolAction.toolId,
      text,
    };
    items.push({
      providerId: "suggestion",
      itemId: `suggestion:tool:${toolAction.toolId}`,
      title: `${toolAction.toolName}（剪贴板：${preview}）`,
      subtitle: "Enter 打开并预填剪贴板内容",
      badge: { short: "建议", tone: "warn" },
      searchFields: [],
      weight: 2,
      payload: { suggestionAction },
    });
  }

  const suggestionAction: ClipboardSuggestionAction = {
    kind: "open-reference-card",
    text,
  };
  items.push({
    providerId: "suggestion",
    itemId: "suggestion:reference-card",
    title: `创建置顶参考卡（剪贴板：${preview}）`,
    subtitle: detected
      ? `${detected.label} · Enter 创建或聚焦参考卡`
      : "Enter 创建或聚焦参考卡",
    badge: { short: "参考", tone: "primary" },
    searchFields: REFERENCE_CARD_SEARCH_TERMS.map((term) => ({
      text: term,
      initials: toPinyinInitials(term),
      weight: 1,
    })),
    weight: 1.5,
    payload: { suggestionAction },
  });

  return items;
}

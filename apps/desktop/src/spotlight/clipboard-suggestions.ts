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

// 必须与 detectClipboardContent 的文本长度上限保持一致，避免超长文本进入其全文 trim。
const CLIPBOARD_DETECTION_MAX_CHARACTERS = 100_000;

type ClipboardSuggestionItemsWriter = (items: SpotlightItem[]) => void;

function buildPreview(text: string): string {
  const characters: string[] = [];
  let pendingSpace = false;

  for (const character of text) {
    if (/\s/u.test(character)) {
      pendingSpace = characters.length > 0;
      continue;
    }
    if (pendingSpace && characters.length < 33) characters.push(" ");
    pendingSpace = false;
    if (characters.length < 33) characters.push(character);
    if (characters.length >= 33) break;
  }

  return characters.length > 32
    ? `${characters.slice(0, 32).join("")}…`
    : characters.join("");
}

export function buildClipboardSuggestionItems(text: string): SpotlightItem[] {
  if (!validateReferenceCardText(text).ok) return [];

  const preview = buildPreview(text);
  const detected = text.length <= CLIPBOARD_DETECTION_MAX_CHARACTERS
    ? detectClipboardContent(text)
    : null;
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

function suggestionItemKey(item: SpotlightItem): string {
  return `${item.providerId}:${item.itemId}`;
}

export function mergeClipboardSuggestionItems(
  existingItems: SpotlightItem[],
  clipboardItems: SpotlightItem[],
): SpotlightItem[] {
  const merged = new Map(
    existingItems.map((item) => [suggestionItemKey(item), item]),
  );
  for (const item of clipboardItems) {
    merged.set(suggestionItemKey(item), item);
  }
  return Array.from(merged.values());
}

export function createClipboardSuggestionRefreshCoordinator(
  writeItems: ClipboardSuggestionItemsWriter,
) {
  let latestRequestId = 0;

  return {
    async refresh(readText: () => Promise<string | null>): Promise<void> {
      const requestId = ++latestRequestId;
      writeItems([]);
      try {
        const text = await readText();
        if (requestId !== latestRequestId) return;
        writeItems(text === null ? [] : buildClipboardSuggestionItems(text));
      } catch {
        if (requestId === latestRequestId) writeItems([]);
      }
    },
  };
}

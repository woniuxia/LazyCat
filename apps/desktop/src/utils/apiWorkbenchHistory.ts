import type {
  ApiWorkbenchHistoryDetail,
  ApiWorkbenchHistoryItem,
  ApiWorkbenchRequestDraft,
} from "../types/api-workbench";
import { normalizeApiWorkbenchDraft } from "./apiWorkbench";

export function canReplayApiWorkbenchHistory(item: ApiWorkbenchHistoryItem): boolean {
  return item.hasExecutedRequestSnapshot;
}

export function buildApiWorkbenchDraftFromHistory(
  item: ApiWorkbenchHistoryDetail,
): { draft: ApiWorkbenchRequestDraft; degraded: boolean } {
  if (item.requestSnapshot) {
    return {
      draft: normalizeApiWorkbenchDraft(item.requestSnapshot),
      degraded: false,
    };
  }
  return {
    draft: normalizeApiWorkbenchDraft({
      method: item.method,
      url: item.url,
      query: [],
      headers: [],
      bodyType: "none",
      body: "",
      form: [],
      timeoutMs: 10000,
    }),
    degraded: true,
  };
}

export function defaultApiWorkbenchHistoryDisplayName(item: ApiWorkbenchHistoryItem): string {
  const explicit = item.name.trim();
  if (explicit) return explicit;
  const raw = item.url.trim() || item.finalUrl.trim();
  try {
    const parsed = new URL(raw);
    return `${item.method} ${parsed.pathname || "/"}`;
  } catch {
    return `${item.method} ${raw || item.finalUrl || item.url}`.trim();
  }
}

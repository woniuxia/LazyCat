import type {
  ApiWorkbenchCollection,
  ApiWorkbenchPersistedTab,
  ApiWorkbenchTab,
  ApiWorkbenchTabsPersist,
} from "../types/api-workbench";
import { createApiWorkbenchBlankDraft, normalizeApiWorkbenchDraft } from "./apiWorkbench";

export const API_WORKBENCH_MAX_TABS = 20;
export const API_WORKBENCH_TABS_SETTING_KEY = "api-workbench:tabs";

export function createApiWorkbenchTab(
  input: Partial<ApiWorkbenchTab> & Pick<ApiWorkbenchTab, "id" | "kind">,
): ApiWorkbenchTab {
  return {
    requestId: null,
    collectionId: null,
    folderId: null,
    name: "",
    description: "",
    draft: createApiWorkbenchBlankDraft(),
    response: null,
    savedSnapshot: null,
    sourceHistoryId: null,
    editorTab: "query",
    responseTab: "response",
    ...input,
  };
}

function snapshotFingerprint(name: string, draft: ApiWorkbenchTab["draft"]): string {
  return JSON.stringify({ name: name.trim(), draft: normalizeApiWorkbenchDraft(draft) });
}

export function isApiWorkbenchTabDirty(tab: ApiWorkbenchTab): boolean {
  if (tab.kind === "request") {
    if (!tab.savedSnapshot) return true;
    return (
      snapshotFingerprint(tab.name, tab.draft) !==
      snapshotFingerprint(tab.savedSnapshot.name, tab.savedSnapshot.draft)
    );
  }
  if (tab.name.trim() !== "") return true;
  return (
    JSON.stringify(normalizeApiWorkbenchDraft(tab.draft)) !==
    JSON.stringify(createApiWorkbenchBlankDraft())
  );
}

export function pickApiWorkbenchNeighborTabId(
  tabs: ApiWorkbenchTab[],
  closingId: number,
): number | null {
  const index = tabs.findIndex((tab) => tab.id === closingId);
  if (index === -1) return null;
  if (index + 1 < tabs.length) return tabs[index + 1].id;
  if (index - 1 >= 0) return tabs[index - 1].id;
  return null;
}

export function toApiWorkbenchPersistedTab(tab: ApiWorkbenchTab): ApiWorkbenchPersistedTab {
  return {
    id: tab.id,
    kind: tab.kind,
    requestId: tab.requestId,
    collectionId: tab.collectionId,
    folderId: tab.folderId,
    name: tab.name,
    description: tab.description,
    draft: normalizeApiWorkbenchDraft(tab.draft),
    savedSnapshot: tab.savedSnapshot
      ? {
          name: tab.savedSnapshot.name,
          draft: normalizeApiWorkbenchDraft(tab.savedSnapshot.draft),
        }
      : null,
    sourceHistoryId: tab.sourceHistoryId,
  };
}

export function buildApiWorkbenchTabsPersist(
  tabs: ApiWorkbenchTab[],
  activeTabId: number | null,
): ApiWorkbenchTabsPersist {
  return {
    version: 1,
    activeTabId,
    tabs: tabs.map(toApiWorkbenchPersistedTab),
  };
}

export interface ApiWorkbenchTabsRestoreContext {
  collectionIds: Set<number>;
  requestIds: Set<number>;
  fallbackCollectionId: number | null;
}

export interface ApiWorkbenchTabsRestoreResult {
  tabs: ApiWorkbenchTab[];
  activeTabId: number | null;
}

function restoreSingleTab(
  raw: unknown,
  ctx: ApiWorkbenchTabsRestoreContext,
): ApiWorkbenchTab | null {
  if (typeof raw !== "object" || raw === null) return null;
  const record = raw as Partial<ApiWorkbenchPersistedTab>;
  if (typeof record.id !== "number") return null;
  if (record.kind !== "request" && record.kind !== "temp") return null;
  if (typeof record.draft !== "object" || record.draft === null) return null;

  const snapshot =
    record.savedSnapshot && typeof record.savedSnapshot === "object"
      ? {
          name: String(record.savedSnapshot.name ?? ""),
          draft: normalizeApiWorkbenchDraft(record.savedSnapshot.draft ?? {}),
        }
      : null;
  let tab = createApiWorkbenchTab({
    id: record.id,
    kind: record.kind,
    requestId: typeof record.requestId === "number" ? record.requestId : null,
    collectionId: typeof record.collectionId === "number" ? record.collectionId : null,
    folderId: typeof record.folderId === "number" ? record.folderId : null,
    name: String(record.name ?? ""),
    description: String(record.description ?? ""),
    draft: normalizeApiWorkbenchDraft(record.draft),
    savedSnapshot: snapshot,
    sourceHistoryId: typeof record.sourceHistoryId === "number" ? record.sourceHistoryId : null,
  });

  if (tab.kind === "request" && (tab.requestId === null || !ctx.requestIds.has(tab.requestId))) {
    tab = { ...tab, kind: "temp", requestId: null, savedSnapshot: null };
  }
  if (tab.collectionId !== null && !ctx.collectionIds.has(tab.collectionId)) {
    tab = {
      ...tab,
      kind: "temp",
      requestId: null,
      savedSnapshot: null,
      collectionId: ctx.fallbackCollectionId,
      folderId: null,
    };
  }
  return tab;
}

export function normalizeApiWorkbenchRestoredTabs(
  raw: unknown,
  ctx: ApiWorkbenchTabsRestoreContext,
): ApiWorkbenchTabsRestoreResult {
  if (typeof raw !== "object" || raw === null) return { tabs: [], activeTabId: null };
  const persist = raw as Partial<ApiWorkbenchTabsPersist>;
  if (persist.version !== 1 || !Array.isArray(persist.tabs)) {
    return { tabs: [], activeTabId: null };
  }
  const tabs: ApiWorkbenchTab[] = [];
  const seenIds = new Set<number>();
  for (const rawTab of persist.tabs) {
    if (tabs.length >= API_WORKBENCH_MAX_TABS) break;
    const tab = restoreSingleTab(rawTab, ctx);
    if (!tab || seenIds.has(tab.id)) continue;
    seenIds.add(tab.id);
    tabs.push(tab);
  }
  const activeTabId =
    typeof persist.activeTabId === "number" && tabs.some((tab) => tab.id === persist.activeTabId)
      ? persist.activeTabId
      : (tabs[0]?.id ?? null);
  return { tabs, activeTabId };
}

export function backfillApiWorkbenchTabFolderIds(
  tabs: ApiWorkbenchTab[],
  collections: ApiWorkbenchCollection[],
): ApiWorkbenchTab[] {
  const folderByRequestId = new Map<number, number | null>();
  for (const collection of collections) {
    for (const request of collection.requests) {
      folderByRequestId.set(request.id, request.folderId);
    }
  }
  return tabs.map((tab) => {
    if (tab.kind !== "request" || tab.requestId === null) return tab;
    const folderId = folderByRequestId.has(tab.requestId)
      ? (folderByRequestId.get(tab.requestId) ?? null)
      : null;
    return tab.folderId === folderId ? tab : { ...tab, folderId };
  });
}

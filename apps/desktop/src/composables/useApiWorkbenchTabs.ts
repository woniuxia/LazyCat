import { computed, ref, watch } from "vue";
import type { ApiWorkbenchTab } from "../types/api-workbench";
import { getSettingJson, setSettingJson } from "./useSettings";
import {
  API_WORKBENCH_MAX_TABS,
  API_WORKBENCH_TABS_SETTING_KEY,
  buildApiWorkbenchTabsPersist,
  createApiWorkbenchTab,
  isApiWorkbenchTabDirty,
  normalizeApiWorkbenchRestoredTabs,
  pickApiWorkbenchNeighborTabId,
  type ApiWorkbenchTabsRestoreContext,
} from "../utils/apiWorkbenchTabs";

const tabs = ref<ApiWorkbenchTab[]>([]);
const activeTabId = ref<number | null>(null);
let nextTabId = 1;
let persistTimer: ReturnType<typeof setTimeout> | null = null;
let persistEnabled = false;

const activeTab = computed(
  () => tabs.value.find((tab) => tab.id === activeTabId.value) ?? null,
);

function allocateTabId(): number {
  return nextTabId++;
}

function persistNow() {
  if (!persistEnabled) return;
  setSettingJson(
    API_WORKBENCH_TABS_SETTING_KEY,
    buildApiWorkbenchTabsPersist(tabs.value, activeTabId.value),
  );
}

function schedulePersist() {
  if (!persistEnabled) return;
  if (persistTimer) clearTimeout(persistTimer);
  persistTimer = setTimeout(persistNow, 500);
}

watch(
  () =>
    tabs.value.map((tab) => ({
      id: tab.id,
      kind: tab.kind,
      requestId: tab.requestId,
      collectionId: tab.collectionId,
      folderId: tab.folderId,
      name: tab.name,
      description: tab.description,
      draft: tab.draft,
      savedSnapshot: tab.savedSnapshot,
      sourceHistoryId: tab.sourceHistoryId,
    })),
  schedulePersist,
  { deep: true },
);
watch(activeTabId, schedulePersist);

export interface ApiWorkbenchOpenRequestDetail {
  requestId: number;
  collectionId: number;
  folderId: number | null;
  name: string;
  description: string;
  draft: ApiWorkbenchTab["draft"];
}

export function useApiWorkbenchTabs() {
  function canOpenMoreTabs(): boolean {
    return tabs.value.length < API_WORKBENCH_MAX_TABS;
  }

  function activateTab(tabId: number) {
    if (tabs.value.some((tab) => tab.id === tabId)) {
      activeTabId.value = tabId;
    }
  }

  /** 已开则激活，未开则新建；返回 null 表示超出上限被拒绝 */
  function openRequestTab(detail: ApiWorkbenchOpenRequestDetail): ApiWorkbenchTab | null {
    const existing = tabs.value.find(
      (tab) => tab.kind === "request" && tab.requestId === detail.requestId,
    );
    if (existing) {
      activeTabId.value = existing.id;
      return existing;
    }
    if (!canOpenMoreTabs()) return null;
    const tab = createApiWorkbenchTab({
      id: allocateTabId(),
      kind: "request",
      requestId: detail.requestId,
      collectionId: detail.collectionId,
      folderId: detail.folderId,
      name: detail.name,
      description: detail.description,
      draft: detail.draft,
      savedSnapshot: { name: detail.name, draft: detail.draft },
    });
    tabs.value.push(tab);
    activeTabId.value = tab.id;
    return tab;
  }

  /** 新建临时标签；返回 null 表示超出上限被拒绝 */
  function openTempTab(
    init: Partial<Pick<ApiWorkbenchTab, "collectionId" | "folderId" | "name" | "draft" | "sourceHistoryId" | "response">> = {},
  ): ApiWorkbenchTab | null {
    if (!canOpenMoreTabs()) return null;
    const tab = createApiWorkbenchTab({
      id: allocateTabId(),
      kind: "temp",
      ...init,
    });
    tabs.value.push(tab);
    activeTabId.value = tab.id;
    return tab;
  }

  function closeTab(tabId: number) {
    const index = tabs.value.findIndex((tab) => tab.id === tabId);
    if (index === -1) return;
    const neighborId =
      activeTabId.value === tabId ? pickApiWorkbenchNeighborTabId(tabs.value, tabId) : null;
    tabs.value.splice(index, 1);
    if (activeTabId.value === tabId) {
      activeTabId.value = neighborId;
    }
  }

  /** 批量关闭；skip 返回 true 的标签保留，返回被跳过的数量 */
  function closeTabs(targetIds: number[], skip: (tab: ApiWorkbenchTab) => boolean): number {
    let skipped = 0;
    for (const id of targetIds) {
      const tab = tabs.value.find((item) => item.id === id);
      if (!tab) continue;
      if (skip(tab)) {
        skipped += 1;
        continue;
      }
      closeTab(id);
    }
    return skipped;
  }

  function tabIdsOtherThan(tabId: number): number[] {
    return tabs.value.filter((tab) => tab.id !== tabId).map((tab) => tab.id);
  }

  function tabIdsToLeft(tabId: number): number[] {
    const index = tabs.value.findIndex((tab) => tab.id === tabId);
    return index <= 0 ? [] : tabs.value.slice(0, index).map((tab) => tab.id);
  }

  function tabIdsToRight(tabId: number): number[] {
    const index = tabs.value.findIndex((tab) => tab.id === tabId);
    return index === -1 ? [] : tabs.value.slice(index + 1).map((tab) => tab.id);
  }

  function markSaved(
    tabId: number,
    saved: {
      requestId: number;
      collectionId: number;
      folderId: number | null;
      name: string;
      draft: ApiWorkbenchTab["draft"];
    },
  ) {
    const tab = tabs.value.find((item) => item.id === tabId);
    if (!tab) return;
    tab.kind = "request";
    tab.requestId = saved.requestId;
    tab.collectionId = saved.collectionId;
    tab.folderId = saved.folderId;
    tab.name = saved.name;
    tab.savedSnapshot = { name: saved.name, draft: saved.draft };
  }

  function replaceTabs(nextTabs: ApiWorkbenchTab[], nextActiveTabId: number | null) {
    tabs.value = nextTabs;
    activeTabId.value = nextActiveTabId;
    nextTabId = Math.max(0, ...nextTabs.map((tab) => tab.id)) + 1;
  }

  function restoreFromSettings(ctx: ApiWorkbenchTabsRestoreContext) {
    const raw = getSettingJson<unknown>(API_WORKBENCH_TABS_SETTING_KEY, null);
    const result = normalizeApiWorkbenchRestoredTabs(raw, ctx);
    replaceTabs(result.tabs, result.activeTabId);
    persistEnabled = true;
  }

  return {
    tabs,
    activeTabId,
    activeTab,
    isTabDirty: isApiWorkbenchTabDirty,
    canOpenMoreTabs,
    activateTab,
    openRequestTab,
    openTempTab,
    closeTab,
    closeTabs,
    tabIdsOtherThan,
    tabIdsToLeft,
    tabIdsToRight,
    markSaved,
    replaceTabs,
    restoreFromSettings,
    persistNow,
  };
}

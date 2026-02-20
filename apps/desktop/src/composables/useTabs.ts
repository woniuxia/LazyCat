import { ref, type Ref } from "vue";
import type { TabItem } from "../types/tabs";

const HOME_TAB: TabItem = { id: "home", name: "首页", pinned: true };

const openTabs: Ref<TabItem[]> = ref([{ ...HOME_TAB }]);
const activeTabId = ref("home");

export function useTabs() {
  function openTab(id: string, name: string) {
    const existing = openTabs.value.find((t) => t.id === id);
    if (existing) {
      activeTabId.value = id;
      return;
    }
    openTabs.value.push({ id, name, pinned: false });
    activeTabId.value = id;
  }

  function closeTab(id: string) {
    const tab = openTabs.value.find((t) => t.id === id);
    if (!tab || tab.pinned) return;

    const idx = openTabs.value.findIndex((t) => t.id === id);
    openTabs.value.splice(idx, 1);

    if (activeTabId.value === id) {
      // Switch to right neighbor > left neighbor > home
      const next =
        openTabs.value[idx] ?? openTabs.value[idx - 1] ?? openTabs.value[0];
      activeTabId.value = next.id;
    }
  }

  function closeOthers(keepId: string) {
    openTabs.value = openTabs.value.filter(
      (t) => t.pinned || t.id === keepId
    );
    if (!openTabs.value.find((t) => t.id === activeTabId.value)) {
      activeTabId.value = keepId;
    }
  }

  function closeToLeft(targetId: string) {
    const idx = openTabs.value.findIndex((t) => t.id === targetId);
    if (idx <= 0) return;
    const removed = openTabs.value
      .slice(0, idx)
      .filter((t) => !t.pinned)
      .map((t) => t.id);
    openTabs.value = openTabs.value.filter((t) => !removed.includes(t.id));
    if (!openTabs.value.find((t) => t.id === activeTabId.value)) {
      activeTabId.value = targetId;
    }
  }

  function closeToRight(targetId: string) {
    const idx = openTabs.value.findIndex((t) => t.id === targetId);
    if (idx < 0 || idx >= openTabs.value.length - 1) return;
    const removed = openTabs.value
      .slice(idx + 1)
      .filter((t) => !t.pinned)
      .map((t) => t.id);
    openTabs.value = openTabs.value.filter((t) => !removed.includes(t.id));
    if (!openTabs.value.find((t) => t.id === activeTabId.value)) {
      activeTabId.value = targetId;
    }
  }

  return {
    openTabs,
    activeTabId,
    openTab,
    closeTab,
    closeOthers,
    closeToLeft,
    closeToRight,
  };
}

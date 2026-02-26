import { computed, ref } from "vue";
import type { ComputedRef, Ref } from "vue";
import type { SidebarItem, ToolSearchMetaMap } from "../types";
import { getSettingJson, setSettingJson } from "./useSettings";

const STORAGE_KEY = "menu_visibility";
const SEARCH_META_KEY = "tool_search_meta_v1";

const hiddenToolIds: Ref<Set<string>> = ref(new Set());
const toolSearchMetaMap = ref<ToolSearchMetaMap>({});

export function useMenuVisibility(sortedSidebarItems: ComputedRef<SidebarItem[]>) {
  /** 过滤隐藏项 + 自动提升后的菜单 */
  const visibleSidebarItems = computed<SidebarItem[]>(() => {
    const hidden = hiddenToolIds.value;
    const result: SidebarItem[] = [];

    for (const item of sortedSidebarItems.value) {
      if (item.kind === "tool") {
        if (!hidden.has(item.tool.id)) {
          result.push(item);
        }
      } else {
        const visibleTools = item.group.tools.filter((t) => !hidden.has(t.id));
        if (visibleTools.length === 0) continue;
        if (visibleTools.length === 1) {
          result.push({ kind: "tool", tool: visibleTools[0] });
        } else {
          result.push({ kind: "group", group: { ...item.group, tools: visibleTools } });
        }
      }
    }
    return result;
  });

  function getHiddenIds(): string[] {
    return [...hiddenToolIds.value];
  }

  function setHiddenIds(ids: string[]) {
    hiddenToolIds.value = new Set(ids);
    setSettingJson(STORAGE_KEY, ids);
  }

  function getToolSearchMetaMap(): ToolSearchMetaMap {
    return toolSearchMetaMap.value;
  }

  function setToolSearchMetaMap(map: ToolSearchMetaMap) {
    toolSearchMetaMap.value = map;
    setSettingJson(SEARCH_META_KEY, map);
  }

  function loadMenuVisibility() {
    const ids = getSettingJson<string[]>(STORAGE_KEY, []);
    hiddenToolIds.value = new Set(ids);
    toolSearchMetaMap.value = getSettingJson<ToolSearchMetaMap>(SEARCH_META_KEY, {});
  }

  return {
    visibleSidebarItems,
    getHiddenIds,
    setHiddenIds,
    getToolSearchMetaMap,
    setToolSearchMetaMap,
    loadMenuVisibility,
  };
}

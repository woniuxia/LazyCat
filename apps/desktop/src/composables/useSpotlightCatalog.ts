import { computed, ref } from "vue";
import { initSettings, getSettingJson } from "./useSettings";
import { getAllTools, getSidebarItems, isRealToolId } from "./toolCatalog";
import { buildToolIndex } from "../utils/search-index";
import { matchScore } from "../utils/fuzzy-match";
import type { ToolClickHistory, ToolDef, ToolSearchMetaMap } from "../types";

const CLICK_WINDOW_MS = 30 * 24 * 60 * 60 * 1000;

export interface SpotlightEntry {
  tool: ToolDef;
  isFavorite: boolean;
  count: number;
  score?: number;
}

export function useSpotlightCatalog() {
  const allTools = getAllTools();
  const sidebarItems = getSidebarItems();

  const favoriteIds = ref<string[]>([]);
  const clickHistory = ref<ToolClickHistory>({});
  const homeTopLimit = ref<number>(12);
  const metaMap = ref<ToolSearchMetaMap>({});

  function recentCount(toolId: string): number {
    const cutoff = Date.now() - CLICK_WINDOW_MS;
    return (clickHistory.value[toolId] ?? []).filter((ts) => ts >= cutoff).length;
  }

  const mergedHomeTools = computed<SpotlightEntry[]>(() => {
    const favoriteSet = new Set(favoriteIds.value);
    const limit = Math.max(1, homeTopLimit.value);

    const favItems: SpotlightEntry[] = favoriteIds.value
      .map((id) => allTools.find((t) => t.id === id))
      .filter((t): t is ToolDef => Boolean(t))
      .map((tool) => ({ tool, isFavorite: true, count: recentCount(tool.id) }));

    const remaining = Math.max(0, limit - favItems.length);
    if (remaining === 0) return favItems;

    const nonFav = allTools
      .filter((tool) => !favoriteSet.has(tool.id))
      .map<SpotlightEntry>((tool) => ({
        tool,
        isFavorite: false,
        count: recentCount(tool.id),
      }))
      .filter((item) => item.count > 0)
      .sort((a, b) => b.count - a.count)
      .slice(0, remaining);

    return [...favItems, ...nonFav];
  });

  const indexedTools = computed(() => buildToolIndex(sidebarItems, metaMap.value));

  function searchTools(query: string, limit = 8): SpotlightEntry[] {
    const trimmed = query.trim();
    if (!trimmed) return mergedHomeTools.value.slice(0, limit);

    const favoriteSet = new Set(favoriteIds.value);
    const scored = indexedTools.value
      .map((entry) => {
        const score = matchScore(trimmed, entry.fields);
        return score > 0
          ? {
              tool: entry.tool,
              isFavorite: favoriteSet.has(entry.tool.id),
              count: recentCount(entry.tool.id),
              score,
            }
          : null;
      })
      .filter((item): item is SpotlightEntry & { score: number } => Boolean(item))
      .sort((a, b) => {
        if (b.score !== a.score) return b.score - a.score;
        if (a.isFavorite !== b.isFavorite) return a.isFavorite ? -1 : 1;
        if (b.count !== a.count) return b.count - a.count;
        return 0;
      })
      .slice(0, limit);

    return scored;
  }

  function loadFromStorage() {
    const rawFav = getSettingJson<string[]>("favorites", []);
    favoriteIds.value = Array.isArray(rawFav)
      ? rawFav.filter((id): id is string => typeof id === "string" && isRealToolId(id))
      : [];

    const rawClicks = getSettingJson<ToolClickHistory>("tool_clicks", {});
    const cutoff = Date.now() - CLICK_WINDOW_MS;
    const cleaned: ToolClickHistory = {};
    for (const [id, list] of Object.entries(rawClicks)) {
      if (!isRealToolId(id) || !Array.isArray(list)) continue;
      const valid = list.filter(
        (item): item is number => typeof item === "number" && Number.isFinite(item) && item >= cutoff,
      );
      if (valid.length) cleaned[id] = valid;
    }
    clickHistory.value = cleaned;

    const rawLimit = getSettingJson<string>("home_top_limit", "12");
    const parsed = parseInt(rawLimit, 10);
    homeTopLimit.value = Number.isFinite(parsed) && parsed > 0 ? parsed : 12;

    metaMap.value = getSettingJson<ToolSearchMetaMap>("tool_search_meta_v1", {});
  }

  async function ensureLoaded() {
    await initSettings();
    loadFromStorage();
  }

  return {
    favoriteIds,
    clickHistory,
    homeTopLimit,
    mergedHomeTools,
    searchTools,
    loadFromStorage,
    ensureLoaded,
  };
}

import { computed, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import type { ToolDef, ToolClickHistory } from "../types";
import { getSettingJson, setSettingJson } from "./useSettings";

const CLICK_WINDOW_MS = 30 * 24 * 60 * 60 * 1000;
const MAX_CLICK_HISTORY_PER_TOOL = 500;

export function useFavorites(allTools: ToolDef[], isRealToolId: (id: string) => boolean) {
  const favoriteToolIds = ref<string[]>([]);
  const toolClickHistory = ref<ToolClickHistory>({});
  const homeTopLimit = ref<6 | 12>(12);

  const allToolMap = new Map(allTools.map((t) => [t.id, t]));

  const favoriteTools = computed(() =>
    favoriteToolIds.value
      .map((id) => allToolMap.get(id))
      .filter((item): item is ToolDef => Boolean(item)),
  );

  const topMonthlyTools = computed(() => {
    const cutoff = Date.now() - CLICK_WINDOW_MS;
    const favoriteSet = new Set(favoriteToolIds.value);
    const stats = allTools
      .filter((tool) => !favoriteSet.has(tool.id))
      .map((tool) => {
        const clicks = (toolClickHistory.value[tool.id] ?? []).filter(
          (ts) => ts >= cutoff,
        ).length;
        return { tool, count: clicks };
      })
      .filter((item) => item.count > 0)
      .sort((a, b) => b.count - a.count);
    return stats.slice(0, homeTopLimit.value);
  });

  function isFavorite(id: string) {
    return favoriteToolIds.value.includes(id);
  }

  function toggleFavorite(id: string) {
    if (!isRealToolId(id)) return;
    if (isFavorite(id)) {
      favoriteToolIds.value = favoriteToolIds.value.filter((toolId) => toolId !== id);
      ElMessage.success("已取消收藏");
      return;
    }
    favoriteToolIds.value = [...favoriteToolIds.value, id];
    ElMessage.success("已加入收藏");
  }

  function pruneClicks(history: ToolClickHistory): ToolClickHistory {
    const cutoff = Date.now() - CLICK_WINDOW_MS;
    const result: ToolClickHistory = {};
    for (const [toolId, timestamps] of Object.entries(history)) {
      if (!isRealToolId(toolId) || !Array.isArray(timestamps)) continue;
      const valid = timestamps
        .filter(
          (item): item is number =>
            typeof item === "number" && Number.isFinite(item) && item >= cutoff,
        )
        .sort((a, b) => a - b)
        .slice(-MAX_CLICK_HISTORY_PER_TOOL);
      if (valid.length) result[toolId] = valid;
    }
    return result;
  }

  function recordToolClick(id: string) {
    if (!isRealToolId(id)) return;
    const next = { ...toolClickHistory.value };
    const history = [...(next[id] ?? []), Date.now()];
    next[id] = history.slice(-MAX_CLICK_HISTORY_PER_TOOL);
    toolClickHistory.value = next;
  }

  function loadFromStorage() {
    // Favorites
    const rawFav = getSettingJson<string[]>("favorites", []);
    favoriteToolIds.value = Array.isArray(rawFav)
      ? rawFav.filter((id): id is string => typeof id === "string" && isRealToolId(id))
      : [];

    // Click history
    const rawClicks = getSettingJson<ToolClickHistory>("tool_clicks", {});
    toolClickHistory.value = pruneClicks(rawClicks);

    // Home top limit
    const rawLimit = getSettingJson<string>("home_top_limit", "12");
    homeTopLimit.value = rawLimit === "6" ? 6 : 12;
  }

  // Auto-persist to SQLite via useSettings
  watch(favoriteToolIds, () => setSettingJson("favorites", favoriteToolIds.value), {
    deep: true,
  });
  watch(
    toolClickHistory,
    () => setSettingJson("tool_clicks", pruneClicks(toolClickHistory.value)),
    { deep: true },
  );
  watch(homeTopLimit, (v) => setSettingJson("home_top_limit", String(v)));

  return {
    favoriteToolIds,
    toolClickHistory,
    homeTopLimit,
    favoriteTools,
    topMonthlyTools,
    isFavorite,
    toggleFavorite,
    recordToolClick,
    loadFromStorage,
  };
}

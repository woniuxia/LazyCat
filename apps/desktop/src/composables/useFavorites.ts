import { computed, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type { ToolDef, ToolUsageMap, UsageSummary } from "../types";
import { getSettingJson, setSettingJson } from "./useSettings";

export interface MergedHomeTool {
  tool: ToolDef;
  isFavorite: boolean;
  count: number; // 近 30 天点击次数
}

export const TODO_TOOL_ID = "todo";
export const TODO_FAVORITE_SEEDED_KEY = "favorites_todo_seeded";

export interface FavoriteBootstrapResult {
  favoriteToolIds: string[];
  shouldMarkTodoSeeded: boolean;
}

export function normalizeFavoriteToolIds(
  rawFavorites: unknown,
  isRealToolId: (id: string) => boolean,
): string[] {
  if (!Array.isArray(rawFavorites)) return [];

  const favoriteToolIds: string[] = [];
  const seen = new Set<string>();
  for (const item of rawFavorites) {
    if (typeof item !== "string" || !isRealToolId(item) || seen.has(item)) continue;
    seen.add(item);
    favoriteToolIds.push(item);
  }

  return favoriteToolIds;
}

export function bootstrapFavoriteToolIds(
  rawFavorites: unknown,
  hasSeededTodoFavorite: boolean,
  isRealToolId: (id: string) => boolean,
): FavoriteBootstrapResult {
  const favoriteToolIds = normalizeFavoriteToolIds(rawFavorites, isRealToolId);

  if (!isRealToolId(TODO_TOOL_ID) || hasSeededTodoFavorite) {
    return {
      favoriteToolIds,
      shouldMarkTodoSeeded: false,
    };
  }

  if (favoriteToolIds.includes(TODO_TOOL_ID)) {
    return {
      favoriteToolIds,
      shouldMarkTodoSeeded: true,
    };
  }

  return {
    favoriteToolIds: [TODO_TOOL_ID, ...favoriteToolIds],
    shouldMarkTodoSeeded: true,
  };
}

export function useFavorites(allTools: ToolDef[], isRealToolId: (id: string) => boolean) {
  const favoriteToolIds = ref<string[]>([]);
  const toolUsage = ref<ToolUsageMap>({});
  const homeTopLimit = ref<number>(12);

  const allToolMap = new Map(allTools.map((t) => [t.id, t]));

  const favoriteTools = computed(() =>
    favoriteToolIds.value
      .map((id) => allToolMap.get(id))
      .filter((item): item is ToolDef => Boolean(item)),
  );

  const mergedHomeTools = computed<MergedHomeTool[]>(() => {
    const favoriteSet = new Set(favoriteToolIds.value);

    // 收藏工具按存储顺序全部显示
    const favItems: MergedHomeTool[] = favoriteTools.value.map((tool) => ({
      tool,
      isFavorite: true,
      count: toolUsage.value[tool.id]?.windowCount ?? 0,
    }));

    // 非收藏工具填充剩余槽位
    const remaining = Math.max(0, homeTopLimit.value - favItems.length);
    if (remaining === 0) return favItems;

    const nonFavItems: MergedHomeTool[] = allTools
      .filter((tool) => !favoriteSet.has(tool.id))
      .map((tool) => ({
        tool,
        isFavorite: false,
        count: toolUsage.value[tool.id]?.windowCount ?? 0,
      }))
      .filter((item) => item.count > 0)
      .sort((a, b) => b.count - a.count)
      .slice(0, remaining);

    return [...favItems, ...nonFavItems];
  });

  function reorderFavorites(newIds: string[]) {
    const currentSet = new Set(favoriteToolIds.value);
    if (
      newIds.length === favoriteToolIds.value.length &&
      newIds.every((id) => currentSet.has(id))
    ) {
      favoriteToolIds.value = newIds;
      // watch 自动持久化到 SQLite
    }
  }

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

  async function recordToolOpen(id: string) {
    if (!isRealToolId(id)) return;
    try {
      const result = (await invokeToolByChannel("tool:usage:record-tool-open", {
        toolId: id,
      })) as { resourceId: string; summary: UsageSummary };
      toolUsage.value = {
        ...toolUsage.value,
        [result.resourceId]: result.summary,
      };
    } catch (error) {
      console.warn("record tool usage failed", error);
    }
  }

  async function loadFromStorage() {
    // Favorites
    const rawFav = getSettingJson<string[]>("favorites", []);
    const hasSeededTodoFavorite = getSettingJson<boolean>(TODO_FAVORITE_SEEDED_KEY, false);
    const favoriteBootstrap = bootstrapFavoriteToolIds(rawFav, hasSeededTodoFavorite, isRealToolId);
    favoriteToolIds.value = favoriteBootstrap.favoriteToolIds;
    if (favoriteBootstrap.shouldMarkTodoSeeded) {
      setSettingJson(TODO_FAVORITE_SEEDED_KEY, true);
    }

    try {
      const result = (await invokeToolByChannel("tool:usage:tool-summaries", {
        toolIds: allTools.map((tool) => tool.id),
      })) as { items: Array<{ resourceId: string; summary: UsageSummary }> };
      toolUsage.value = Object.fromEntries(
        result.items
          .filter((item) => isRealToolId(item.resourceId))
          .map((item) => [item.resourceId, item.summary]),
      );
    } catch (error) {
      console.warn("load tool usage failed", error);
      toolUsage.value = {};
    }

    // Home top limit
    const rawLimit = getSettingJson<string>("home_top_limit", "12");
    const parsed = parseInt(rawLimit, 10);
    homeTopLimit.value = Number.isFinite(parsed) && parsed > 0 ? parsed : 12;
  }

  // Auto-persist to SQLite via useSettings
  watch(favoriteToolIds, () => setSettingJson("favorites", favoriteToolIds.value), {
    deep: true,
  });
  watch(homeTopLimit, (v) => setSettingJson("home_top_limit", String(v)));

  return {
    favoriteToolIds,
    toolUsage,
    homeTopLimit,
    favoriteTools,
    mergedHomeTools,
    isFavorite,
    toggleFavorite,
    reorderFavorites,
    recordToolOpen,
    loadFromStorage,
  };
}

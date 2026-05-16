import { invoke } from "@tauri-apps/api/core";
import { getSidebarItems, isRealToolId } from "../../composables/toolCatalog";
import { initSettings, getSettingJson } from "../../composables/useSettings";
import { buildToolIndex } from "../../utils/search-index";
import type { ToolClickHistory, ToolSearchMetaMap } from "../../types";
import type {
  SpotlightExecuteContext,
  SpotlightExecuteResult,
  SpotlightItem,
  SpotlightProvider,
} from "../types";

const CLICK_WINDOW_MS = 30 * 24 * 60 * 60 * 1000;

function recentCount(history: ToolClickHistory, toolId: string): number {
  const cutoff = Date.now() - CLICK_WINDOW_MS;
  return (history[toolId] ?? []).filter((ts) => ts >= cutoff).length;
}

async function prefetchTools(): Promise<SpotlightItem[]> {
  await initSettings();
  const metaMap = getSettingJson<ToolSearchMetaMap>("tool_search_meta_v1", {});
  const clickHistory = getSettingJson<ToolClickHistory>("tool_clicks", {});
  const favoriteIds = getSettingJson<string[]>("favorites", []);
  const favoriteSet = new Set(
    Array.isArray(favoriteIds)
      ? favoriteIds.filter((id): id is string => typeof id === "string" && isRealToolId(id))
      : [],
  );

  const indexed = buildToolIndex(getSidebarItems(), metaMap);

  return indexed.map<SpotlightItem>((entry) => {
    const id = entry.tool.id;
    const count = recentCount(clickHistory, id);
    const isFav = favoriteSet.has(id);
    return {
      providerId: "tool",
      itemId: id,
      title: entry.tool.name,
      subtitle: entry.tool.desc,
      badge: { short: "工具", tone: "primary" },
      status: isFav
        ? { text: "收藏", tone: "warn" }
        : count > 0
        ? { text: `高频 ${count}`, tone: "info" }
        : undefined,
      searchFields: entry.fields,
      weight: isFav ? 1.15 : count > 0 ? 1 + Math.min(count, 20) * 0.01 : 1,
      payload: { toolId: id },
    };
  });
}

async function defaultAction(
  item: SpotlightItem,
  _ctx: SpotlightExecuteContext,
): Promise<SpotlightExecuteResult> {
  const toolId = (item.payload?.toolId as string | undefined) ?? item.itemId;
  await invoke("spotlight_pick", { target: toolId });
  return { closeSpotlight: true };
}

export const toolProvider: SpotlightProvider = {
  id: "tool",
  scopeKeys: [],
  badgeShort: "工具",
  badgeTone: "primary",
  weight: 1.0,
  prefetch: prefetchTools,
  defaultAction,
};

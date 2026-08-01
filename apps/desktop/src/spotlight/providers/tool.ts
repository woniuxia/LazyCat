import { invoke } from "@tauri-apps/api/core";
import { getSidebarItems, isRealToolId } from "../../composables/toolCatalog";
import { initSettings, getSettingJson } from "../../composables/useSettings";
import { buildToolIndex } from "../../utils/search-index";
import type { ToolSearchMetaMap } from "../../types";
import type {
  ProviderDescriptor,
  SpotlightExecuteContext,
  SpotlightExecuteResult,
  SpotlightItem,
} from "../types";

async function prefetchTools(): Promise<SpotlightItem[]> {
  await initSettings();
  const metaMap = getSettingJson<ToolSearchMetaMap>("tool_search_meta_v1", {});
  const favoriteIds = getSettingJson<string[]>("favorites", []);
  const favoriteSet = new Set(
    Array.isArray(favoriteIds)
      ? favoriteIds.filter((id): id is string => typeof id === "string" && isRealToolId(id))
      : [],
  );

  const indexed = buildToolIndex(getSidebarItems(), metaMap);
  return indexed.map((entry, index) => {
    const id = entry.tool.id;
    const isFav = favoriteSet.has(id);
    return {
      providerId: "tool",
      itemId: id,
      title: entry.tool.name,
      subtitle: entry.tool.desc,
      badge: { short: "工具", tone: "primary" },
      status: isFav ? { text: "收藏", tone: "warn" } : undefined,
      searchFields: entry.fields,
      ranking: {
        favorite: isFav,
        sourceOrder: index,
        usageRef: {
          resourceType: "tool",
          resourceId: id,
          actions: ["open"],
        },
      },
      payload: { toolId: id },
    } satisfies SpotlightItem;
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

export const toolProvider: ProviderDescriptor = {
  id: "tool",
  name: "工具",
  description: "在所有内置工具中检索",
  badgeShort: "工具",
  badgeTone: "primary",
  emptyQueryQuota: 8,
  defaultAliases: [],
  defaultEnabled: true,
  prefetch: prefetchTools,
  defaultAction,
};

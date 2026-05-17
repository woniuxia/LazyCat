import { invoke } from "@tauri-apps/api/core";
import { registerProvider } from "../registry";
import type {
  ProviderDescriptor,
  SpotlightExecuteContext,
  SpotlightExecuteResult,
  SpotlightItem,
} from "../types";

async function defaultAction(
  item: SpotlightItem,
  _ctx: SpotlightExecuteContext,
): Promise<SpotlightExecuteResult> {
  const toolId = (item.payload?.toolId as string | undefined) ?? "";
  const text = (item.payload?.text as string | undefined) ?? "";
  if (!toolId) return { errorMessage: "建议项缺少工具 ID" };
  await invoke("spotlight_pick", {
    target: toolId,
    text,
    source: "clipboard-suggestion",
  });
  return { closeSpotlight: true };
}

export const suggestionProvider: ProviderDescriptor = {
  id: "suggestion",
  name: "剪贴板建议",
  description: "剪贴板内容智能匹配",
  badgeShort: "建议",
  badgeTone: "warn",
  weight: 100,
  defaultAliases: [],
  defaultEnabled: true,
  hiddenInSettings: true,
  prefetch: async () => [],
  defaultAction,
};

registerProvider(suggestionProvider);

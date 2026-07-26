import { invoke } from "@tauri-apps/api/core";
import { showReferenceCard } from "../../bridge/tauri";
import { isRealToolId } from "../../composables/toolCatalog";
import { validateReferenceCardText } from "../../utils/monacoLanguages";
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
  const action = item.payload?.suggestionAction;
  if (!action || typeof action !== "object") {
    return { errorMessage: "无效的剪贴板建议" };
  }

  if (
    "kind" in action &&
    action.kind === "open-reference-card" &&
    "text" in action &&
    typeof action.text === "string" &&
    validateReferenceCardText(action.text).ok
  ) {
    await showReferenceCard(action.text);
    return { closeSpotlight: true };
  }

  if (
    "kind" in action &&
    action.kind === "open-tool" &&
    "toolId" in action &&
    typeof action.toolId === "string" &&
    isRealToolId(action.toolId) &&
    "text" in action &&
    typeof action.text === "string" &&
    validateReferenceCardText(action.text).ok
  ) {
    await invoke("spotlight_pick", {
      target: action.toolId,
      text: action.text,
      source: "clipboard-suggestion",
    });
    return { closeSpotlight: true };
  }

  return { errorMessage: "无效的剪贴板建议" };
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

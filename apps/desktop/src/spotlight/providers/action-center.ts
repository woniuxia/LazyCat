import { invoke } from "@tauri-apps/api/core";

import { invokeToolByChannel } from "../../bridge/tauri";
import type {
  ActionCombinationRunStatus,
  ActionCombinationSummary,
} from "../../types/action-center";
import { createSearchField } from "../../utils/fuzzy-match";
import { registerProvider } from "../registry";
import type {
  ProviderDescriptor,
  SpotlightExecuteResult,
  SpotlightItem,
  SpotlightStatus,
} from "../types";

function runStatus(status: ActionCombinationRunStatus | undefined): SpotlightStatus | undefined {
  switch (status) {
    case "pending":
    case "running":
      return { text: "运行中", tone: "primary" };
    case "succeeded":
      return { text: "成功", tone: "success" };
    case "partially_succeeded":
      return { text: "部分成功", tone: "warn" };
    case "failed":
      return { text: "失败", tone: "danger" };
    default:
      return undefined;
  }
}

export function buildActionCombinationSpotlightItem(
  combination: ActionCombinationSummary,
): SpotlightItem {
  const modeLabel = combination.executionMode === "parallel" ? "并行" : "串行";
  return {
    providerId: "action-center",
    itemId: String(combination.id),
    title: combination.name,
    subtitle: `${modeLabel} · ${combination.stepCount} 个步骤`,
    badge: { short: "动", tone: "primary" },
    status: runStatus(combination.latestRunStatus),
    searchFields: [
      createSearchField(combination.name, 1.2),
      createSearchField("动作中心 组合动作", 0.65),
      createSearchField(modeLabel, 0.4),
    ],
    ranking: {
      usageRef: {
        resourceType: "action-combination",
        resourceId: String(combination.id),
        actions: ["run"],
      },
    },
    payload: {
      combinationId: combination.id,
      combinationName: combination.name,
    },
  };
}

function combinationIdentity(item: SpotlightItem): { id: number; name: string } | null {
  const id = item.payload?.combinationId;
  const name = item.payload?.combinationName;
  if (typeof id !== "number" || !Number.isSafeInteger(id) || id <= 0) return null;
  if (typeof name !== "string" || !name.trim()) return null;
  return { id, name };
}

async function prefetchCombinations(): Promise<SpotlightItem[]> {
  const response = (await invokeToolByChannel("tool:action-center:combination-list", {})) as {
    combinations?: ActionCombinationSummary[];
  };
  return (response.combinations ?? []).map(buildActionCombinationSpotlightItem);
}

async function runCombination(item: SpotlightItem): Promise<SpotlightExecuteResult> {
  const combination = combinationIdentity(item);
  if (!combination) return { errorMessage: "动作组合数据无效" };
  try {
    await invokeToolByChannel("tool:action-center:combination-run", {
      combinationId: combination.id,
      notifyOnCompletion: true,
    });
    return {
      closeSpotlight: true,
      toast: { message: `已开始运行 ${combination.name}`, type: "success" },
    };
  } catch (error) {
    return { errorMessage: error instanceof Error ? error.message : String(error) };
  }
}

async function openCombination(item: SpotlightItem): Promise<SpotlightExecuteResult> {
  const combination = combinationIdentity(item);
  if (!combination) return { errorMessage: "动作组合数据无效" };
  await invoke("spotlight_pick", {
    target: "action-center",
    itemId: String(combination.id),
    view: "combination",
  });
  return { closeSpotlight: true };
}

export const actionCenterProvider: ProviderDescriptor = {
  id: "action-center",
  name: "动作组合",
  description: "搜索并运行动作中心中保存的组合动作",
  badgeShort: "动",
  badgeTone: "primary",
  defaultAliases: ["ac"],
  defaultEnabled: true,
  prefetch: prefetchCombinations,
  defaultAction: runCombination,
  buildActions: () => [
    { id: "run", label: "运行", icon: "play", shortcut: "Enter" },
    { id: "open", label: "打开动作中心", icon: "external" },
  ],
  executeAction: (item, actionId) => {
    if (actionId === "run") return runCombination(item);
    if (actionId === "open") return openCombination(item);
    return Promise.resolve({ errorMessage: `未知动作 ${actionId}` });
  },
};

registerProvider(actionCenterProvider);

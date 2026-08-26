import { invoke } from "@tauri-apps/api/core";
import { invokeToolByChannel } from "../../bridge/tauri";
import { createSearchField } from "../../utils/fuzzy-match";
import type { FollowUpItem } from "../../types/follow-up";
import { registerProvider } from "../registry";
import type { ProviderDescriptor, SpotlightExecuteResult, SpotlightItem } from "../types";

function reviewStatus(item: FollowUpItem): SpotlightItem["status"] {
  if (item.attentionStatus === "ended") return { text: "已结束", tone: "muted" };
  if (!item.reviewAt) return undefined;
  const reviewAt = new Date(item.reviewAt);
  if (Number.isNaN(reviewAt.getTime())) return undefined;
  if (reviewAt.getTime() <= Date.now()) return { text: "待复查", tone: "warn" };
  return { text: reviewAt.toLocaleDateString(), tone: "info" };
}

async function prefetchFollowUps(): Promise<SpotlightItem[]> {
  const list = await invokeToolByChannel<FollowUpItem[]>("tool:follow-up:item-list", {});
  if (!Array.isArray(list)) throw new Error("关注事项列表返回格式无效");
  const entry: SpotlightItem = {
    providerId: "follow-up",
    itemId: "feature",
    title: "关注事项",
    subtitle: "打开关注事项列表",
    badge: { short: "关", tone: "primary" },
    searchFields: [createSearchField("关注事项", 1.4), createSearchField("follow-up", 1)],
    payload: { feature: true },
  };
  return [entry, ...list.map((item) => {
    const subtitle = [item.personNameSnapshot || item.personName, item.description].filter(Boolean).join(" · ");
    const due = item.attentionStatus === "active" && !!item.reviewAt && new Date(item.reviewAt).getTime() <= Date.now();
    return {
      providerId: "follow-up",
      itemId: String(item.id),
      title: item.title || "(无标题)",
      subtitle: subtitle || undefined,
      badge: { short: "关", tone: "primary" },
      status: reviewStatus(item),
      searchFields: [
        createSearchField(item.title, 1.2),
        createSearchField(item.description, 0.8),
        createSearchField(item.expectedOutcome, 0.6),
        createSearchField(item.personNameSnapshot || item.personName, 0.6),
      ],
      ranking: { contextual: due, recommendationEligible: due },
      payload: { followUpId: item.id },
    } satisfies SpotlightItem;
  })];
}

async function defaultAction(item: SpotlightItem): Promise<SpotlightExecuteResult> {
  if (item.payload?.feature === true) {
    await invoke("spotlight_pick", { target: "follow-up" });
    return { closeSpotlight: true };
  }
  const followUpId = item.payload?.followUpId as number | undefined;
  if (!followUpId) return { errorMessage: "无效关注事项" };
  await invoke("spotlight_pick", { target: "follow-up", itemId: String(followUpId) });
  return { closeSpotlight: true };
}

export const followUpProvider: ProviderDescriptor = {
  id: "follow-up",
  name: "关注事项",
  description: "关注事项与复查计划",
  badgeShort: "关",
  badgeTone: "primary",
  defaultAliases: ["f", "follow", "follow-up"],
  defaultEnabled: true,
  prefetch: prefetchFollowUps,
  defaultAction,
};

registerProvider(followUpProvider);
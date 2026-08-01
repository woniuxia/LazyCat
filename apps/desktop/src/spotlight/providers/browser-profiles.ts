import { invoke } from "@tauri-apps/api/core";
import { invokeToolByChannel } from "../../bridge/tauri";
import type { BrowserProfileItem, BrowserProfilesListResponse } from "../../types/browser-profiles";
import {
  buildBrowserProfileSearchFields,
  getBrowserProfileDisplayName,
} from "../../utils/browserProfiles";
import { notifyBrowserProfilesChanged } from "../browser-profiles-events";
import { registerProvider } from "../registry";
import type { ProviderDescriptor, SpotlightExecuteResult, SpotlightItem } from "../types";

interface BrowserProfilePayload {
  browser: "edge" | "chrome";
  profileDir: string;
  displayName: string;
}

export function buildBrowserProfileSpotlightItem(profile: BrowserProfileItem): SpotlightItem {
  const displayName = getBrowserProfileDisplayName(profile);
  const browserLabel = profile.browser === "chrome" ? "Chrome" : "Edge";
  const subtitleParts = [profile.edgeDisplayName.trim(), profile.profileDir.trim()].filter(Boolean);

  return {
    providerId: "browser-profiles",
    itemId: `${profile.browser}:${profile.profileDir}`,
    title: displayName,
    subtitle: subtitleParts.join(" · "),
    badge: { short: browserLabel, tone: "primary" },
    searchFields: buildBrowserProfileSearchFields(profile),
    ranking: {
      usageRef: {
        resourceType: "browser-profile",
        resourceId: JSON.stringify([profile.browser, profile.profileDir]),
        actions: ["launch"],
      },
    },
    payload: {
      browser: profile.browser,
      profileDir: profile.profileDir,
      displayName,
    },
  };
}

async function prefetchBrowserProfiles(): Promise<SpotlightItem[]> {
  const result = (await invokeToolByChannel(
    "tool:browser-profiles:list",
    {},
  )) as BrowserProfilesListResponse;
  if (!Array.isArray(result?.profiles)) throw new Error("浏览器身份列表返回格式无效");
  return result.profiles
    .filter((profile) => !profile.hidden)
    .map(buildBrowserProfileSpotlightItem);
}

function payloadOf(item: SpotlightItem): BrowserProfilePayload | null {
  if (item.payload?.browser !== "edge" && item.payload?.browser !== "chrome") return null;
  if (typeof item.payload.profileDir !== "string") return null;
  if (typeof item.payload.displayName !== "string") return null;
  return {
    browser: item.payload.browser,
    profileDir: item.payload.profileDir,
    displayName: item.payload.displayName,
  };
}

async function launchProfile(item: SpotlightItem): Promise<SpotlightExecuteResult> {
  const payload = payloadOf(item);
  if (!payload) return { errorMessage: "无效的浏览器身份结果" };

  try {
    await invokeToolByChannel("tool:browser-profiles:launch", {
      browser: payload.browser,
      profileDir: payload.profileDir,
    });
    try {
      await notifyBrowserProfilesChanged("launch");
    } catch {
      /* Spotlight cache refresh notification is best-effort. */
    }
    return {
      closeSpotlight: true,
      toast: {
        message: `已打开 ${payload.browser === "chrome" ? "Chrome" : "Edge"}：${payload.displayName}`,
        type: "success",
      },
    };
  } catch (err) {
    return { errorMessage: err instanceof Error ? err.message : String(err) };
  }
}

async function openBrowserProfilesTool(): Promise<SpotlightExecuteResult> {
  await invoke("spotlight_pick", { target: "browser-profiles" });
  return { closeSpotlight: true };
}

function buildActions() {
  return [
    { id: "launch", label: "启动", icon: "play", shortcut: "Enter" },
    { id: "open_tool", label: "跳转到浏览器身份", icon: "external" },
  ];
}

async function executeAction(
  item: SpotlightItem,
  actionId: string,
): Promise<SpotlightExecuteResult> {
  if (actionId === "launch") return launchProfile(item);
  if (actionId === "open_tool") return openBrowserProfilesTool();
  return { errorMessage: `未知动作 ${actionId}` };
}

export const browserProfilesProvider: ProviderDescriptor = {
  id: "browser-profiles",
  name: "浏览器身份",
  description: "启动 Edge 或 Chrome 用户身份窗口",
  badgeShort: "浏览器",
  badgeTone: "primary",
  weight: 1.02,
  defaultAliases: [],
  defaultEnabled: true,
  prefetch: prefetchBrowserProfiles,
  defaultAction: launchProfile,
  buildActions,
  executeAction,
};

registerProvider(browserProfilesProvider);

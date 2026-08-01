import { invokeToolByChannel } from "../../bridge/tauri";
import { emit } from "@tauri-apps/api/event";
import { APP_EVENTS } from "../../bridge/events";
import { createSearchField } from "../../utils/fuzzy-match";
import { registerProvider } from "../registry";
import type {
  ProviderDescriptor,
  SpotlightExecuteContext,
  SpotlightExecuteResult,
  SpotlightItem,
} from "../types";

interface HostsProfile {
  id: number;
  name: string;
  content: string;
  enabled: boolean | number;
  updatedAt?: string;
  sortOrder?: number;
}

function firstCommentLine(content: string): string {
  const lines = content.split(/\r?\n/);
  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith("#")) return trimmed.replace(/^#+\s*/, "");
  }
  return "";
}

async function prefetchHosts(): Promise<SpotlightItem[]> {
  const raw = (await invokeToolByChannel("tool:hosts:list", {})) as HostsProfile[];
  if (!Array.isArray(raw)) throw new Error("Hosts 列表返回格式无效");
  const list = raw;

  return list.map<SpotlightItem>((profile) => {
    const enabled = !!profile.enabled;
    const comment = firstCommentLine(profile.content ?? "");
    return {
      providerId: "hosts",
      itemId: String(profile.id),
      title: profile.name || "(未命名)",
      subtitle: comment || (enabled ? "当前启用" : "未启用"),
      badge: { short: "主", tone: "info" },
      status: enabled ? { text: "已启用", tone: "success" } : { text: "未启用", tone: "muted" },
      searchFields: [createSearchField(profile.name, 1.2), createSearchField(comment, 0.85)],
      payload: { profileName: profile.name, content: profile.content, enabled },
    };
  });
}

async function copyContent(content: string): Promise<void> {
  await navigator.clipboard.writeText(content ?? "");
}

async function defaultAction(
  item: SpotlightItem,
  _ctx: SpotlightExecuteContext,
): Promise<SpotlightExecuteResult> {
  const profileName = item.payload?.profileName as string | undefined;
  if (!profileName) return { errorMessage: "无效 hosts profile" };
  try {
    await invokeToolByChannel("tool:hosts:activate", { profileName });
    try {
      await emit(APP_EVENTS.HOSTS_APPLIED, { name: profileName });
    } catch {
      /* event emit failure is non-fatal */
    }
    return {
      closeSpotlight: true,
      toast: { message: `已切换到 ${profileName}（可在 Hosts 工具撤销）`, type: "success" },
    };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    return { errorMessage: `切换失败：${msg}` };
  }
}

function buildActions() {
  return [
    { id: "activate", label: "应用 profile", icon: "check", shortcut: "Enter" },
    { id: "copy_content", label: "仅复制内容（不应用）", icon: "copy" },
    { id: "open_hosts", label: "跳转到 Hosts 工具", icon: "external" },
  ];
}

async function executeAction(
  item: SpotlightItem,
  actionId: string,
  ctx: SpotlightExecuteContext,
): Promise<SpotlightExecuteResult> {
  if (actionId === "activate") return defaultAction(item, ctx);
  if (actionId === "copy_content") {
    const content = (item.payload?.content as string | undefined) ?? "";
    await copyContent(content);
    return { closeSpotlight: true, toast: { message: "hosts 内容已复制", type: "success" } };
  }
  if (actionId === "open_hosts") {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("spotlight_pick", { target: "hosts" });
    return { closeSpotlight: true };
  }
  return { errorMessage: `未知动作 ${actionId}` };
}

export const hostsProvider: ProviderDescriptor = {
  id: "hosts",
  name: "Hosts",
  description: "切换 hosts profile",
  badgeShort: "主",
  badgeTone: "info",
  defaultAliases: ["h", "hosts"],
  defaultEnabled: true,
  prefetch: prefetchHosts,
  defaultAction,
  buildActions,
  executeAction,
};

registerProvider(hostsProvider);

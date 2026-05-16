import { invokeToolByChannel } from "../../bridge/tauri";
import { toPinyinInitials } from "../../utils/fuzzy-match";
import { registerProvider } from "../registry";
import type {
  SpotlightExecuteContext,
  SpotlightExecuteResult,
  SpotlightItem,
  SpotlightProvider,
} from "../types";

interface HostsProfile {
  id: number;
  name: string;
  content: string;
  enabled: boolean | number;
  updatedAt?: string;
  sortOrder?: number;
}

function makeField(text: string, weight: number) {
  const cleaned = text.trim();
  return { text: cleaned, initials: toPinyinInitials(cleaned), weight };
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
  let list: HostsProfile[] = [];
  try {
    const raw = (await invokeToolByChannel("tool:hosts:list", {})) as HostsProfile[];
    list = Array.isArray(raw) ? raw : [];
  } catch {
    return [];
  }

  return list.map<SpotlightItem>((profile) => {
    const enabled = !!profile.enabled;
    const comment = firstCommentLine(profile.content ?? "");
    return {
      providerId: "hosts",
      itemId: String(profile.id),
      title: profile.name || "(未命名)",
      subtitle: comment || (enabled ? "当前启用" : "未启用"),
      badge: { short: "主", tone: "info" },
      status: enabled
        ? { text: "已启用", tone: "success" }
        : { text: "未启用", tone: "muted" },
      searchFields: [
        makeField(profile.name, 1.2),
        makeField(comment, 0.85),
      ],
      weight: enabled ? 1.1 : 1,
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
    return {
      closeSpotlight: true,
      toast: { message: `已切换到 ${profileName}`, type: "success" },
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

export const hostsProvider: SpotlightProvider = {
  id: "hosts",
  scopeKeys: ["h", "hosts"],
  badgeShort: "主",
  badgeTone: "info",
  weight: 0.75,
  prefetch: prefetchHosts,
  defaultAction,
  buildActions,
  executeAction,
};

registerProvider(hostsProvider);

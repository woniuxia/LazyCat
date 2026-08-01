import { invoke } from "@tauri-apps/api/core";
import { invokeToolByChannel } from "../../bridge/tauri";
import { toPinyinInitials } from "../../utils/fuzzy-match";
import { registerProvider } from "../registry";
import type { ProviderDescriptor, SpotlightExecuteResult, SpotlightItem } from "../types";

interface LauncherEntry {
  id: number;
  name: string;
  exe_path: string;
  arguments?: string;
  group_name?: string;
}

function makeField(text: string, weight: number) {
  const cleaned = text.trim();
  return { text: cleaned, initials: toPinyinInitials(cleaned), weight };
}

function isDirPath(p: string): boolean {
  // 后端 launch_app 用 Path::is_dir;前端粗略判断:无文件扩展名视为目录,仅影响展示文案
  return !/\.[a-zA-Z0-9]{1,5}$/.test(p);
}

async function prefetchLauncher(): Promise<SpotlightItem[]> {
  const raw = (await invokeToolByChannel("tool:launcher:spotlight-list", {})) as
    | { items?: LauncherEntry[] }
    | LauncherEntry[]
    | null;
  const list = Array.isArray(raw) ? raw : raw?.items;
  if (!Array.isArray(list)) throw new Error("快捷启动列表返回格式无效");

  return list.map<SpotlightItem>((e) => {
    const isDir = isDirPath(e.exe_path);
    const stem =
      e.exe_path
        .split(/[\\/]/)
        .pop()
        ?.replace(/\.[^.]+$/, "") ?? "";
    return {
      providerId: "launcher",
      itemId: String(e.id),
      title: e.name,
      subtitle: e.group_name || (isDir ? "文件夹" : "应用"),
      badge: { short: "启", tone: "primary" },
      searchFields: [makeField(e.name, 1.2), makeField(stem, 0.6)],
      ranking: {
        usageRef: {
          resourceType: "launcher-entry",
          resourceId: String(e.id),
          actions: ["launch"],
        },
      },
      payload: {
        exePath: e.exe_path,
        arguments: e.arguments ?? "",
        isDir,
        name: e.name,
      },
    };
  });
}

async function launchEntry(item: SpotlightItem, admin: boolean): Promise<SpotlightExecuteResult> {
  try {
    await invokeToolByChannel("tool:launcher:launch", {
      exe_path: item.payload?.exePath,
      arguments: item.payload?.arguments ?? "",
      admin,
    });
    return {
      closeSpotlight: true,
      toast: { message: `已启动 ${item.payload?.name}`, type: "success" },
    };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    return { errorMessage: msg };
  }
}

async function openFolder(item: SpotlightItem): Promise<SpotlightExecuteResult> {
  try {
    await invokeToolByChannel("tool:launcher:open-folder", {
      exe_path: item.payload?.exePath,
    });
    return { closeSpotlight: true };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    return { errorMessage: msg };
  }
}

async function openLauncher(): Promise<SpotlightExecuteResult> {
  await invoke("spotlight_pick", { target: "launcher" });
  return { closeSpotlight: true };
}

async function defaultAction(item: SpotlightItem): Promise<SpotlightExecuteResult> {
  return launchEntry(item, false);
}

function buildActions() {
  return [
    { id: "launch", label: "启动", icon: "play", shortcut: "Enter" },
    { id: "launch_admin", label: "以管理员身份启动", icon: "shield" },
    { id: "open_folder", label: "打开所在目录", icon: "folder" },
    { id: "open_launcher", label: "跳转到快捷启动", icon: "external" },
  ];
}

async function executeAction(
  item: SpotlightItem,
  actionId: string,
): Promise<SpotlightExecuteResult> {
  if (actionId === "launch") return launchEntry(item, false);
  if (actionId === "launch_admin") return launchEntry(item, true);
  if (actionId === "open_folder") return openFolder(item);
  if (actionId === "open_launcher") return openLauncher();
  return { errorMessage: `未知动作 ${actionId}` };
}

export const launcherProvider: ProviderDescriptor = {
  id: "launcher",
  name: "快捷启动",
  description: "通过 Spotlight 启动已注册的应用与文件夹",
  badgeShort: "启",
  badgeTone: "primary",
  weight: 0.95,
  defaultAliases: [],
  defaultEnabled: true,
  prefetch: prefetchLauncher,
  defaultAction,
  buildActions,
  executeAction,
};

registerProvider(launcherProvider);

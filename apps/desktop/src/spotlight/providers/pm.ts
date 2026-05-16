import { invoke } from "@tauri-apps/api/core";
import { invokeToolByChannel } from "../../bridge/tauri";
import { toPinyinInitials } from "../../utils/fuzzy-match";
import { registerProvider } from "../registry";
import type {
  SpotlightExecuteContext,
  SpotlightExecuteResult,
  SpotlightItem,
  SpotlightProvider,
  StatusTone,
} from "../types";

interface PmListItem {
  id: number;
  projectId: number;
  title: string;
  status: string;
  priority?: string;
  endAt?: string | null;
  pinned?: boolean;
  tags?: string[];
}

interface PmProject {
  id: number;
  name: string;
  color?: string | null;
}

function makeField(text: string, weight: number) {
  const cleaned = text.trim();
  return { text: cleaned, initials: toPinyinInitials(cleaned), weight };
}

function dueStatus(item: PmListItem): { text: string; tone: StatusTone } | undefined {
  if (item.status === "done" || item.status === "completed") {
    return { text: "已完成", tone: "muted" };
  }
  if (!item.endAt) return undefined;
  const due = new Date(item.endAt);
  if (Number.isNaN(due.getTime())) return undefined;
  const now = new Date();
  if (due.getTime() < now.getTime()) return { text: "已逾期", tone: "danger" };
  const startOfTomorrow = new Date(
    now.getFullYear(),
    now.getMonth(),
    now.getDate() + 1,
  );
  if (due.getTime() < startOfTomorrow.getTime()) return { text: "今日", tone: "warn" };
  return { text: due.toLocaleDateString(), tone: "info" };
}

async function loadProjectMap(): Promise<Map<number, PmProject>> {
  try {
    const list = (await invokeToolByChannel("tool:pm:project-list", {})) as PmProject[];
    const map = new Map<number, PmProject>();
    if (Array.isArray(list)) {
      for (const p of list) map.set(p.id, p);
    }
    return map;
  } catch {
    return new Map();
  }
}

async function prefetchPm(): Promise<SpotlightItem[]> {
  const projectMap = await loadProjectMap();
  let list: PmListItem[] = [];
  try {
    const raw = (await invokeToolByChannel("tool:pm:item-list", {})) as PmListItem[];
    list = Array.isArray(raw) ? raw : [];
  } catch {
    return [];
  }

  return list.map<SpotlightItem>((it) => {
    const project = projectMap.get(it.projectId);
    const projectName = project?.name ?? "";
    const tagsField = it.tags?.length ? it.tags.join(" ") : "";
    return {
      providerId: "pm",
      itemId: String(it.id),
      title: it.title || "(无标题)",
      subtitle: projectName,
      badge: { short: "项", tone: "primary" },
      status: dueStatus(it),
      searchFields: [
        makeField(it.title, 1.2),
        makeField(projectName, 0.7),
        makeField(tagsField, 0.85),
      ],
      weight: it.pinned ? 1.2 : 1,
      payload: {
        pmId: it.id,
        projectId: it.projectId,
        projectName,
        status: it.status,
      },
    };
  });
}

async function jumpToPm(
  pmId: number,
  pmView?: string,
): Promise<SpotlightExecuteResult> {
  await invoke("spotlight_pick", { target: "pm", itemId: String(pmId), view: pmView });
  return { closeSpotlight: true };
}

async function defaultAction(item: SpotlightItem): Promise<SpotlightExecuteResult> {
  const pmId = item.payload?.pmId as number | undefined;
  if (!pmId) return { errorMessage: "无效 PM 项" };
  return jumpToPm(pmId);
}

function buildActions() {
  return [
    { id: "open_default", label: "跳转到默认视图", icon: "external", shortcut: "Enter" },
    { id: "open_kanban", label: "在看板视图打开", icon: "board" },
    { id: "open_today", label: "在今日视图打开", icon: "calendar" },
    { id: "open_matrix", label: "在四象限打开", icon: "matrix" },
    { id: "open_list", label: "在列表视图打开", icon: "list" },
  ];
}

async function executeAction(
  item: SpotlightItem,
  actionId: string,
): Promise<SpotlightExecuteResult> {
  const pmId = item.payload?.pmId as number | undefined;
  if (!pmId) return { errorMessage: "无效 PM 项" };
  if (actionId === "open_default") return jumpToPm(pmId);
  if (actionId === "open_kanban") return jumpToPm(pmId, "kanban");
  if (actionId === "open_today") return jumpToPm(pmId, "today");
  if (actionId === "open_matrix") return jumpToPm(pmId, "matrix");
  if (actionId === "open_list") return jumpToPm(pmId, "list");
  return { errorMessage: `未知动作 ${actionId}` };
}

export const pmProvider: SpotlightProvider = {
  id: "pm",
  scopeKeys: ["p", "pm"],
  badgeShort: "项",
  badgeTone: "primary",
  weight: 0.75,
  prefetch: prefetchPm,
  defaultAction,
  buildActions,
  executeAction,
};

registerProvider(pmProvider);

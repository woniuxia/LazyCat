import { invoke } from "@tauri-apps/api/core";
import { invokeToolByChannel } from "../../bridge/tauri";
import { createSearchField } from "../../utils/fuzzy-match";
import { registerProvider } from "../registry";
import type {
  ProviderDescriptor,
  SpotlightExecuteContext,
  SpotlightExecuteResult,
  SpotlightItem,
  StatusTone,
} from "../types";

interface TodoListItem {
  id: number;
  title: string;
  eventAt?: string | null;
  isOverdue?: boolean;
  pinned?: boolean;
  typeName?: string | null;
}

function isSameLocalDay(value: Date, reference: Date): boolean {
  return (
    value.getFullYear() === reference.getFullYear() &&
    value.getMonth() === reference.getMonth() &&
    value.getDate() === reference.getDate()
  );
}

function isDueToday(item: TodoListItem, now: Date): boolean {
  if (item.isOverdue || !item.eventAt) return false;
  const due = new Date(item.eventAt);
  return !Number.isNaN(due.getTime()) && isSameLocalDay(due, now);
}

function dueStatus(item: TodoListItem, now: Date): { text: string; tone: StatusTone } | undefined {
  if (item.isOverdue) return { text: "已逾期", tone: "danger" };

  if (!item.eventAt) return undefined;
  const due = new Date(item.eventAt);
  if (Number.isNaN(due.getTime())) return undefined;
  if (isSameLocalDay(due, now)) return { text: "今日", tone: "warn" };
  return { text: due.toLocaleDateString(), tone: "info" };
}

async function prefetchTodo(): Promise<SpotlightItem[]> {
  const raw = (await invokeToolByChannel("tool:todo:spotlight-list", {})) as
    | { items?: TodoListItem[] }
    | TodoListItem[]
    | null;
  const list = Array.isArray(raw) ? raw : raw?.items;
  if (!Array.isArray(list)) throw new Error("任务列表返回格式无效");

  const now = new Date();
  return list.map<SpotlightItem>((todo) => {
    const dueToday = isDueToday(todo, now);
    const status = dueStatus(todo, now);
    const contextual = todo.pinned === true || todo.isOverdue === true || dueToday;
    return {
      providerId: "todo",
      itemId: String(todo.id),
      title: todo.title || "(无标题)",
      subtitle: todo.typeName ?? undefined,
      badge: { short: "待", tone: "success" },
      status,
      searchFields: [
        createSearchField(todo.title, 1.2),
        createSearchField(todo.typeName ?? "", 0.6),
      ],
      ranking: {
        pinned: todo.pinned,
        contextual,
        recommendationEligible: dueToday,
        usageRef: {
          resourceType: "todo-item",
          resourceId: String(todo.id),
          actions: ["open"],
        },
      },
      payload: {
        todoId: todo.id,
        title: todo.title,
      },
    };
  });
}

async function jumpToTodo(todoId: number): Promise<SpotlightExecuteResult> {
  await invoke("spotlight_pick", { target: "todo", itemId: String(todoId) });
  try {
    await invokeToolByChannel("tool:todo:item-record-open", { id: todoId });
  } catch (error) {
    console.warn(`[Spotlight] record Todo ${todoId} open failed:`, error);
  }
  return { closeSpotlight: true };
}

async function defaultAction(item: SpotlightItem): Promise<SpotlightExecuteResult> {
  const todoId = item.payload?.todoId as number | undefined;
  if (!todoId) return { errorMessage: "无效 todo" };
  return jumpToTodo(todoId);
}

function buildActions() {
  return [
    { id: "open_todo", label: "跳转到任务详情", icon: "external", shortcut: "Enter" },
    { id: "mark_done", label: "标记完成", icon: "check" },
  ];
}

async function markDone(todoId: number): Promise<SpotlightExecuteResult> {
  try {
    await invokeToolByChannel("tool:todo:item-change-status", { id: todoId, status: "done" });
    return {
      refreshProvider: true,
      toast: { message: "已标记完成", type: "success" },
    };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    return { errorMessage: msg };
  }
}

async function executeAction(
  item: SpotlightItem,
  actionId: string,
): Promise<SpotlightExecuteResult> {
  const todoId = item.payload?.todoId as number | undefined;
  if (!todoId) return { errorMessage: "无效 todo" };
  if (actionId === "open_todo") return jumpToTodo(todoId);
  if (actionId === "mark_done") return markDone(todoId);
  return { errorMessage: `未知动作 ${actionId}` };
}

export async function createTodoDraft(text: string): Promise<SpotlightExecuteResult> {
  const title = text.trim();
  if (!title) return { errorMessage: "请输入要新建的任务标题" };
  try {
    await invokeToolByChannel("tool:todo:item-create", { title });
    return {
      closeSpotlight: true,
      toast: { message: `已创建：${title}`, type: "success" },
    };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    return { errorMessage: msg };
  }
}

export const todoProvider: ProviderDescriptor = {
  id: "todo",
  name: "任务",
  description: "任务清单与速建",
  badgeShort: "待",
  badgeTone: "success",
  defaultAliases: ["t", "todo"],
  defaultEnabled: true,
  prefetch: prefetchTodo,
  defaultAction,
  buildActions,
  executeAction,
};

registerProvider(todoProvider);

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

interface TodoListItem {
  id: number;
  title: string;
  status: string;
  priority?: string;
  displayAt?: string | null;
  eventAt?: string | null;
  isOverdue?: boolean;
  pinned?: boolean;
  typeName?: string | null;
}

function makeField(text: string, weight: number) {
  const cleaned = text.trim();
  return { text: cleaned, initials: toPinyinInitials(cleaned), weight };
}

function dueStatus(item: TodoListItem): { text: string; tone: StatusTone } | undefined {
  if (item.status === "done" || item.status === "completed") {
    return { text: "已完成", tone: "muted" };
  }
  if (item.isOverdue) return { text: "已逾期", tone: "danger" };

  const at = item.eventAt ?? item.displayAt;
  if (!at) return undefined;
  const due = new Date(at);
  if (Number.isNaN(due.getTime())) return undefined;
  const now = new Date();
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const startOfTomorrow = new Date(startOfToday.getTime() + 24 * 60 * 60 * 1000);
  if (due < startOfTomorrow) return { text: "今日", tone: "warn" };
  return { text: due.toLocaleDateString(), tone: "info" };
}

async function prefetchTodo(): Promise<SpotlightItem[]> {
  let list: TodoListItem[] = [];
  try {
    const raw = (await invokeToolByChannel("tool:todo:item-list", {
      includeInactive: false,
    })) as TodoListItem[];
    list = Array.isArray(raw) ? raw : [];
  } catch {
    return [];
  }

  return list.map<SpotlightItem>((todo) => {
    const status = dueStatus(todo);
    const isDone = todo.status === "done" || todo.status === "completed";
    return {
      providerId: "todo",
      itemId: String(todo.id),
      title: todo.title || "(无标题)",
      subtitle: todo.typeName ?? undefined,
      badge: { short: "待", tone: "success" },
      status,
      searchFields: [makeField(todo.title, 1.2), makeField(todo.typeName ?? "", 0.6)],
      weight: todo.pinned ? 1.2 : isDone ? 0.85 : 1,
      payload: {
        todoId: todo.id,
        status: todo.status,
        title: todo.title,
        isDone,
      },
    };
  });
}

async function jumpToTodo(todoId: number): Promise<SpotlightExecuteResult> {
  await invoke("spotlight_pick", { target: "todo", itemId: String(todoId) });
  return { closeSpotlight: true };
}

async function defaultAction(item: SpotlightItem): Promise<SpotlightExecuteResult> {
  const todoId = item.payload?.todoId as number | undefined;
  if (!todoId) return { errorMessage: "无效 todo" };
  return jumpToTodo(todoId);
}

function buildActions(item: SpotlightItem) {
  const isDone = !!item.payload?.isDone;
  return [
    { id: "open_todo", label: "跳转到任务详情", icon: "external", shortcut: "Enter" },
    isDone
      ? { id: "reopen", label: "重新打开", icon: "rotate" }
      : { id: "mark_done", label: "标记完成", icon: "check" },
  ];
}

async function changeStatus(todoId: number, status: string): Promise<SpotlightExecuteResult> {
  try {
    await invokeToolByChannel("tool:todo:item-change-status", { id: todoId, status });
    return {
      toast: { message: status === "done" ? "已标记完成" : "已重新打开", type: "success" },
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
  if (actionId === "mark_done") return changeStatus(todoId, "done");
  if (actionId === "reopen") return changeStatus(todoId, "pending");
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

export const todoProvider: SpotlightProvider = {
  id: "todo",
  scopeKeys: ["t", "todo"],
  badgeShort: "待",
  badgeTone: "success",
  weight: 0.85,
  prefetch: prefetchTodo,
  defaultAction,
  buildActions,
  executeAction,
};

registerProvider(todoProvider);

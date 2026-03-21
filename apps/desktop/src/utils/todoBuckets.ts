import type { TodoItem } from "../types";

function isActionableStatus(status: TodoItem["status"]) {
  return status === "pending" || status === "in_progress";
}

function isDoneStatus(status: TodoItem["status"]) {
  return status === "completed";
}

function itemSortTime(item: TodoItem) {
  return item.eventAt || "";
}

function compareActiveItems(left: TodoItem, right: TodoItem) {
  // 1. 置顶优先
  if (left.pinned !== right.pinned) {
    return left.pinned ? -1 : 1;
  }

  // 2. 优先级排序（P0 > P1 > P2 > P3）
  const priorityOrder = { P0: 0, P1: 1, P2: 2, P3: 3 };
  const leftPriority = priorityOrder[left.priority];
  const rightPriority = priorityOrder[right.priority];
  if (leftPriority !== rightPriority) {
    return leftPriority - rightPriority;
  }

  // 3. 到期时间排序（早到期的排前面）
  const leftTime = itemSortTime(left);
  const rightTime = itemSortTime(right);
  if (!leftTime && !rightTime) return right.id - left.id;
  if (!leftTime) return 1;
  if (!rightTime) return -1;
  return leftTime.localeCompare(rightTime) || right.id - left.id;
}

function compareDoneItemsByTime(left: TodoItem, right: TodoItem) {
  const leftTime = left.completedAt || "";
  const rightTime = right.completedAt || "";
  if (!leftTime && !rightTime) return right.id - left.id;
  if (!leftTime) return 1;
  if (!rightTime) return -1;
  return rightTime.localeCompare(leftTime) || right.id - left.id;
}

/**
 * 计算「最近一周已办」的起始日期：严格按过去 7 天窗口（含今天）计算。
 * 返回 ISO 日期字符串 "YYYY-MM-DD"。
 */
export function getRecentWeekStart(): string {
  const today = new Date();
  const start = new Date(today);
  start.setHours(0, 0, 0, 0);
  start.setDate(start.getDate() - 6);
  const y = start.getFullYear();
  const m = String(start.getMonth() + 1).padStart(2, "0");
  const d = String(start.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

export function groupTodoItemsByBucket(items: TodoItem[]) {
  const activeItems: TodoItem[] = [];
  const doneItems: TodoItem[] = [];
  const recentWeekItems: TodoItem[] = [];
  const recentWeekStart = getRecentWeekStart();

  for (const item of items) {
    if (isDoneStatus(item.status)) {
      const doneTime = (item.completedAt || "").slice(0, 10);
      if (doneTime && doneTime >= recentWeekStart) {
        recentWeekItems.push(item);
      } else {
        doneItems.push(item);
      }
      continue;
    }

    if (isActionableStatus(item.status)) {
      activeItems.push(item);
    }
  }

  activeItems.sort(compareActiveItems);
  recentWeekItems.sort(compareDoneItemsByTime);
  doneItems.sort(compareDoneItemsByTime);

  return {
    activeItems,
    recentWeekItems,
    doneItems,
  };
}

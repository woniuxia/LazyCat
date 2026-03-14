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
  const leftTime = left.updatedAt || left.createdAt || "";
  const rightTime = right.updatedAt || right.createdAt || "";
  if (!leftTime && !rightTime) return right.id - left.id;
  if (!leftTime) return 1;
  if (!rightTime) return -1;
  return rightTime.localeCompare(leftTime) || right.id - left.id;
}

/**
 * 计算「最近一周」的起始日期：从今天往前找到最近的周五（含今天）。
 * 若今天是周四(4)，则起始 = 今天 - 6 天（上周五）。
 * 返回 ISO 日期字符串 "YYYY-MM-DD"。
 */
export function getRecentWeekStart(): string {
  const today = new Date();
  // 0=周日,1=周一,...,5=周五,6=周六
  const dow = today.getDay();
  // 从今天往前，到上一个周五（或今天就是周五）
  // 今天是周六(6) -> 1天前(周五)
  // 今天是周五(5) -> 0天前
  // 今天是周四(4) -> 6天前
  // 今天是周三(3) -> 5天前 ... 依此类推
  const daysBack = dow >= 5 ? dow - 5 : dow + 2;
  const start = new Date(today);
  start.setDate(today.getDate() - daysBack);
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
      // 用 updatedAt 判断是否在最近一周内
      const doneTime = (item.updatedAt || item.createdAt || "").slice(0, 10);
      if (doneTime >= recentWeekStart) {
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

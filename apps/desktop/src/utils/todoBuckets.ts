import type { TodoItem } from "../types";

function isActionableStatus(status: TodoItem["status"]) {
  return status === "pending" || status === "in_progress";
}

function isDoneStatus(status: TodoItem["status"]) {
  return status === "completed" || status === "canceled";
}

function isRecurringRootItem(item: TodoItem) {
  return item.kind === "recurring" && item.recordRole === "root";
}

function itemSortTime(item: TodoItem) {
  if (isRecurringRootItem(item)) {
    return item.displayAt || "";
  }
  return item.eventAt || "";
}

function compareActiveItems(left: TodoItem, right: TodoItem) {
  if (left.pinned !== right.pinned) {
    return left.pinned ? -1 : 1;
  }
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

export function groupTodoItemsByBucket(items: TodoItem[]) {
  const activeItems: TodoItem[] = [];
  const doneItems: TodoItem[] = [];

  for (const item of items) {
    if (isDoneStatus(item.status)) {
      doneItems.push(item);
      continue;
    }

    if (isActionableStatus(item.status) || isRecurringRootItem(item)) {
      activeItems.push(item);
    }
  }

  activeItems.sort(compareActiveItems);
  doneItems.sort(compareDoneItemsByTime);

  return {
    activeItems,
    doneItems,
  };
}

import type { TodoItem } from "../types";

function isRecurringRootItem(item: TodoItem) {
  return item.kind === "recurring" && item.recordRole === "root";
}

function isActionableStatus(status: TodoItem["status"]) {
  return status === "pending" || status === "in_progress";
}

function isDoneStatus(status: TodoItem["status"]) {
  return status === "completed" || status === "canceled";
}

export function groupTodoItemsByBucket(items: TodoItem[]) {
  const overdueItems: TodoItem[] = [];
  const pendingItems: TodoItem[] = [];
  const doneItems: TodoItem[] = [];

  for (const item of items) {
    if (isRecurringRootItem(item)) {
      pendingItems.push(item);
      continue;
    }

    if (isDoneStatus(item.status)) {
      doneItems.push(item);
      continue;
    }

    if (isActionableStatus(item.status) && item.isOverdue) {
      overdueItems.push(item);
      continue;
    }

    if (isActionableStatus(item.status)) {
      pendingItems.push(item);
    }
  }

  return {
    overdueItems,
    pendingItems,
    doneItems,
  };
}

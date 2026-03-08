import { describe, expect, it } from "vitest";
import type { TodoItem } from "../types";
import { groupTodoItemsByBucket } from "./todoBuckets";

function createItem(overrides: Partial<TodoItem> = {}): TodoItem {
  return {
    id: 1,
    rootId: 1,
    kind: "one_off",
    recordRole: "root",
    title: "事项",
    typeId: null,
    typeName: null,
    typeColor: null,
    priority: "P2",
    description: "",
    status: "pending",
    eventAt: "2026-03-08T09:00:00.000Z",
    reminderPresets: [],
    snoozeUntil: null,
    lastNotifiedAt: null,
    displayAt: "2026-03-08T09:00:00.000Z",
    assignees: [],
    isOverdue: false,
    canEditFuture: false,
    nextTaskReminderId: null,
    nextReminderPreset: null,
    recurrence: null,
    createdAt: "2026-03-08T00:00:00.000Z",
    updatedAt: "2026-03-08T00:00:00.000Z",
    ...overrides,
  };
}

describe("todoBuckets", () => {
  it("places overdue actionable items into the overdue bucket", () => {
    const overdueItem = createItem({ id: 11, isOverdue: true, status: "pending" });

    const result = groupTodoItemsByBucket([overdueItem]);

    expect(result.overdueItems.map((item) => item.id)).toEqual([11]);
    expect(result.pendingItems).toHaveLength(0);
    expect(result.doneItems).toHaveLength(0);
  });

  it("places non-overdue actionable items into the pending bucket", () => {
    const pendingItem = createItem({ id: 12, status: "in_progress", isOverdue: false });

    const result = groupTodoItemsByBucket([pendingItem]);

    expect(result.overdueItems).toHaveLength(0);
    expect(result.pendingItems.map((item) => item.id)).toEqual([12]);
    expect(result.doneItems).toHaveLength(0);
  });

  it("places completed and canceled items into the done bucket", () => {
    const completedItem = createItem({ id: 13, status: "completed" });
    const canceledItem = createItem({ id: 14, status: "canceled" });

    const result = groupTodoItemsByBucket([completedItem, canceledItem]);

    expect(result.overdueItems).toHaveLength(0);
    expect(result.pendingItems).toHaveLength(0);
    expect(result.doneItems.map((item) => item.id)).toEqual([13, 14]);
  });

  it("keeps recurring root items in the pending bucket", () => {
    const recurringRootItem = createItem({
      id: 15,
      kind: "recurring",
      recordRole: "root",
      status: null,
      isOverdue: true,
    });

    const result = groupTodoItemsByBucket([recurringRootItem]);

    expect(result.overdueItems).toHaveLength(0);
    expect(result.pendingItems.map((item) => item.id)).toEqual([15]);
    expect(result.doneItems).toHaveLength(0);
  });
});

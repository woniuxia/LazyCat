import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TodoItem } from "../types";
import { getRecentWeekStart, groupTodoItemsByBucket } from "./todoBuckets";

beforeEach(() => {
  vi.useFakeTimers();
  // 固定“今天”为周四：2026-03-12
  // 注意：使用本地时间构造，避免不同时区导致的日期漂移。
  vi.setSystemTime(new Date(2026, 2, 12, 12, 0, 0));
});

afterEach(() => {
  vi.useRealTimers();
});

function createItem(overrides: Partial<TodoItem> = {}): TodoItem {
  return {
    id: 1,
    rootId: 1,
    kind: "one_off",
    recordRole: "root",
    pinned: false,
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
    links: [],
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
  it("computes recent week start as last Friday when today is Thursday", () => {
    expect(getRecentWeekStart()).toBe("2026-03-06");
  });

  it("keeps actionable items in the active bucket and sorts by time ascending", () => {
    const laterItem = createItem({ id: 11, status: "pending", eventAt: "2026-03-08T10:00:00.000Z", displayAt: "2026-03-08T10:00:00.000Z" });
    const earlierItem = createItem({ id: 12, status: "in_progress", eventAt: "2026-03-08T08:00:00.000Z", displayAt: "2026-03-08T08:00:00.000Z" });

    const result = groupTodoItemsByBucket([laterItem, earlierItem]);

    expect(result.activeItems.map((item) => item.id)).toEqual([12, 11]);
    expect(result.recentWeekItems).toHaveLength(0);
    expect(result.doneItems).toHaveLength(0);
  });

  it("puts one-off items without eventAt at the end", () => {
    const displayOnlyOneOffItem = createItem({
      id: 13,
      kind: "one_off",
      recordRole: "root",
      eventAt: null,
      displayAt: "2026-03-08T07:00:00.000Z",
    });
    const datedItem = createItem({ id: 14, eventAt: "2026-03-08T09:00:00.000Z", displayAt: null });
    const undatedItem = createItem({ id: 15, eventAt: null, displayAt: null });

    const result = groupTodoItemsByBucket([undatedItem, datedItem, displayOnlyOneOffItem]);

    expect(result.activeItems.map((item) => item.id)).toEqual([14, 15, 13]);
  });

  it("puts pinned active items before unpinned items", () => {
    const pinnedLaterItem = createItem({
      id: 19,
      pinned: true,
      eventAt: "2026-03-08T10:00:00.000Z",
      displayAt: "2026-03-08T10:00:00.000Z",
    });
    const unpinnedEarlierItem = createItem({
      id: 20,
      pinned: false,
      eventAt: "2026-03-08T08:00:00.000Z",
      displayAt: "2026-03-08T08:00:00.000Z",
    });

    const result = groupTodoItemsByBucket([unpinnedEarlierItem, pinnedLaterItem]);

    expect(result.activeItems.map((item) => item.id)).toEqual([19, 20]);
  });

  it("places completed and canceled items into the done bucket", () => {
    const completedItem = createItem({
      id: 16,
      status: "completed",
      updatedAt: "2026-03-10T08:00:00.000Z",
    });
    const canceledItem = createItem({
      id: 17,
      status: "canceled",
      updatedAt: "2026-03-01T10:00:00.000Z",
    });

    const result = groupTodoItemsByBucket([completedItem, canceledItem]);
    const doneSeries = [...result.recentWeekItems, ...result.doneItems];

    expect(result.activeItems).toHaveLength(0);
    expect(doneSeries).toHaveLength(2);
    expect(
      doneSeries.every((item) => item.status === "completed" || item.status === "canceled"),
    ).toBe(true);
  });

  it("routes done items into recent week and done buckets by updatedAt", () => {
    const inRecentWeek = createItem({
      id: 31,
      status: "completed",
      updatedAt: "2026-03-06T09:00:00.000Z",
    });
    const olderThanRecentWeek = createItem({
      id: 32,
      status: "canceled",
      updatedAt: "2026-03-05T23:59:59.000Z",
    });

    const result = groupTodoItemsByBucket([olderThanRecentWeek, inRecentWeek]);

    expect(result.recentWeekItems.map((item) => item.id)).toEqual([31]);
    expect(result.doneItems.map((item) => item.id)).toEqual([32]);
  });

  it("sorts done items by updatedAt descending within each bucket", () => {
    const recentWeekOlderItem = createItem({
      id: 21,
      status: "completed",
      updatedAt: "2026-03-06T08:00:00.000Z",
    });
    const recentWeekNewerItem = createItem({
      id: 22,
      status: "canceled",
      updatedAt: "2026-03-10T10:00:00.000Z",
    });
    const doneOlderItem = createItem({
      id: 23,
      status: "completed",
      updatedAt: "2026-03-01T10:00:00.000Z",
    });
    const doneNewerItem = createItem({
      id: 24,
      status: "canceled",
      updatedAt: "2026-03-05T10:00:00.000Z",
    });

    const result = groupTodoItemsByBucket([
      doneOlderItem,
      recentWeekNewerItem,
      recentWeekOlderItem,
      doneNewerItem,
    ]);

    expect(result.recentWeekItems.map((item) => item.id)).toEqual([22, 21]);
    expect(result.doneItems.map((item) => item.id)).toEqual([24, 23]);
  });

  it("keeps recurring root items in the active bucket", () => {
    const recurringRootItem = createItem({
      id: 18,
      kind: "recurring",
      recordRole: "root",
      status: null,
      eventAt: null,
      displayAt: "2026-03-08T11:00:00.000Z",
    });

    const result = groupTodoItemsByBucket([recurringRootItem]);

    expect(result.activeItems.map((item) => item.id)).toEqual([18]);
    expect(result.doneItems).toHaveLength(0);
  });

  it("sorts recurring root items by displayAt", () => {
    const laterRecurringRootItem = createItem({
      id: 23,
      kind: "recurring",
      recordRole: "root",
      status: null,
      eventAt: null,
      displayAt: "2026-03-08T11:00:00.000Z",
    });
    const earlierRecurringRootItem = createItem({
      id: 24,
      kind: "recurring",
      recordRole: "root",
      status: null,
      eventAt: null,
      displayAt: "2026-03-08T08:00:00.000Z",
    });

    const result = groupTodoItemsByBucket([laterRecurringRootItem, earlierRecurringRootItem]);

    expect(result.activeItems.map((item) => item.id)).toEqual([24, 23]);
  });
});

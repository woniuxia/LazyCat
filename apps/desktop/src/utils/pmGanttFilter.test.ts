import { describe, expect, it } from "vitest";

import type { PmItem, PmItemStatus } from "../types/pm";

import {
  clearPmGanttStatuses,
  coercePmItemStatusForGanttFilter,
  filterPmItemsByGanttStatuses,
  getPmGanttDefaultStatuses,
  normalizePmGanttSelectedStatuses,
  selectAllPmGanttStatuses,
  togglePmGanttStatus,
} from "./pmGanttFilter";

const baseItem: PmItem = {
  id: 1,
  projectId: 1,
  title: "状态过滤测试",
  description: "",
  linkUrl: null,
  itemType: "task",
  priority: "P2",
  status: "todo",
  startAt: "2026-04-01",
  endAt: "2026-04-02",
  pinned: false,
  sortOrder: 0,
  tags: [],
  siyuanPrimaryPage: null,
  siyuanExtraPages: [],
  completedAt: null,
  startedAt: null,
  testingAt: null,
  createdAt: "2026-04-01T00:00:00.000Z",
  updatedAt: "2026-04-01T00:00:00.000Z",
};

function createItem(id: number, status: unknown): PmItem {
  return {
    ...baseItem,
    id,
    status: status as PmItemStatus,
  };
}

describe("pmGanttFilter", () => {
  it("默认返回全部状态且顺序稳定", () => {
    expect(getPmGanttDefaultStatuses()).toEqual(["todo", "in_progress", "testing"]);
    expect(selectAllPmGanttStatuses()).toEqual(["todo", "in_progress", "testing", "done"]);
  });

  it("归一化时去重并按正式顺序输出", () => {
    expect(
      normalizePmGanttSelectedStatuses(["done", "todo", "done", "testing", "invalid"]),
    ).toEqual(["todo", "testing", "done"]);
  });

  it("切换状态时保持稳定顺序", () => {
    expect(togglePmGanttStatus(["todo", "testing"], "done")).toEqual(["todo", "testing", "done"]);
    expect(togglePmGanttStatus(["todo", "in_progress", "testing"], "in_progress")).toEqual(["todo", "testing"]);
  });

  it("清空后保持空数组，不自动恢复默认值", () => {
    expect(clearPmGanttStatuses()).toEqual([]);
    expect(normalizePmGanttSelectedStatuses(clearPmGanttStatuses())).toEqual([]);
  });

  it("未知状态在甘特筛选链路中按 todo 兜底", () => {
    expect(coercePmItemStatusForGanttFilter("blocked")).toBe("todo");
    expect(coercePmItemStatusForGanttFilter(null)).toBe("todo");
    expect(coercePmItemStatusForGanttFilter("testing")).toBe("testing");
  });

  it("仅选 todo 时未知状态可见", () => {
    const items = [
      createItem(1, "todo"),
      createItem(2, "blocked"),
      createItem(3, "done"),
    ];

    expect(filterPmItemsByGanttStatuses(items, ["todo"]).map((item) => item.id)).toEqual([1, 2]);
  });

  it("取消 todo 后未知状态随之不可见", () => {
    const items = [
      createItem(1, "todo"),
      createItem(2, "blocked"),
      createItem(3, "done"),
    ];

    expect(filterPmItemsByGanttStatuses(items, ["done"]).map((item) => item.id)).toEqual([3]);
  });

  it("未选任何状态时直接返回空结果", () => {
    const items = [
      createItem(1, "todo"),
      createItem(2, "done"),
    ];

    expect(filterPmItemsByGanttStatuses(items, [])).toEqual([]);
  });
});

import { describe, expect, it } from "vitest";

import type { PmItem, PmItemStatus } from "../types/pm";

import {
  clearPmStatuses,
  coercePmItemStatusForFilter,
  filterPmItemsBySelectedStatuses,
  getPmDefaultSelectedStatuses,
  getVisiblePmStatusColumns,
  groupPmItemsByStatus,
  normalizePmSelectedStatuses,
  selectAllPmStatuses,
  togglePmSelectedStatus,
} from "./pmStatusFilter";

const baseItem: PmItem = {
  id: 1,
  projectId: 1,
  title: "状态过滤测试",
  description: "",
  linkUrl: null,
  refCode: null,
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

describe("pmStatusFilter", () => {
  it("默认返回当前产品的共享状态集合且顺序稳定", () => {
    expect(getPmDefaultSelectedStatuses()).toEqual(["todo", "in_progress", "testing"]);
    expect(selectAllPmStatuses()).toEqual(["todo", "in_progress", "testing", "done"]);
  });

  it("归一化时去重并按正式顺序输出", () => {
    expect(normalizePmSelectedStatuses(["done", "todo", "done", "testing", "invalid"])).toEqual([
      "todo",
      "testing",
      "done",
    ]);
  });

  it("切换状态时保持稳定顺序", () => {
    expect(togglePmSelectedStatus(["todo", "testing"], "done")).toEqual([
      "todo",
      "testing",
      "done",
    ]);
    expect(togglePmSelectedStatus(["todo", "in_progress", "testing"], "in_progress")).toEqual([
      "todo",
      "testing",
    ]);
  });

  it("清空后保持空数组，不自动恢复默认值", () => {
    expect(clearPmStatuses()).toEqual([]);
    expect(normalizePmSelectedStatuses(clearPmStatuses())).toEqual([]);
  });

  it("未知状态在共享筛选链路中按 todo 兜底", () => {
    expect(coercePmItemStatusForFilter("blocked")).toBe("todo");
    expect(coercePmItemStatusForFilter(null)).toBe("todo");
    expect(coercePmItemStatusForFilter("testing")).toBe("testing");
  });

  it("仅选 todo 时未知状态可见", () => {
    const items = [createItem(1, "todo"), createItem(2, "blocked"), createItem(3, "done")];

    expect(filterPmItemsBySelectedStatuses(items, ["todo"]).map((item) => item.id)).toEqual([1, 2]);
  });

  it("取消 todo 后未知状态随之不可见", () => {
    const items = [createItem(1, "todo"), createItem(2, "blocked"), createItem(3, "done")];

    expect(filterPmItemsBySelectedStatuses(items, ["done"]).map((item) => item.id)).toEqual([3]);
  });

  it("未选任何状态时直接返回空结果", () => {
    const items = [createItem(1, "todo"), createItem(2, "done")];

    expect(filterPmItemsBySelectedStatuses(items, [])).toEqual([]);
  });

  it("可见状态列只返回当前选中的列且保持正式顺序", () => {
    expect(getVisiblePmStatusColumns(["testing", "todo"]).map((column) => column.key)).toEqual([
      "todo",
      "testing",
    ]);
    expect(getVisiblePmStatusColumns([])).toEqual([]);
  });

  it("看板分组时将未知状态工作项归入 todo 列", () => {
    const grouped = groupPmItemsByStatus([
      createItem(1, "blocked"),
      createItem(2, "in_progress"),
      createItem(3, "done"),
    ]);

    expect(grouped.get("todo")?.map((item) => item.id)).toEqual([1]);
    expect(grouped.get("in_progress")?.map((item) => item.id)).toEqual([2]);
    expect(grouped.get("done")?.map((item) => item.id)).toEqual([3]);
  });
});

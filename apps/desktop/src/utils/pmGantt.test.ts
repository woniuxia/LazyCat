import { beforeEach, describe, expect, it, vi } from "vitest";

import type { PmItem } from "../types/pm";

import {
  buildPmGanttPopupHtml,
  buildPmGanttTask,
  buildPmGanttTasks,
  clampPmGanttPopupPosition,
  countPmGanttUnscheduledItems,
  getPmGanttProgress,
} from "./pmGantt";

const baseItem: PmItem = {
  id: 7,
  projectId: 3,
  title: "补齐甘特图交互",
  description: "让甘特图和看板能力对齐",
  linkUrl: null,
  itemType: "improvement",
  priority: "P1",
  status: "in_progress",
  startAt: "2026-03-28",
  endAt: "2026-03-30",
  pinned: true,
  sortOrder: 0,
  tags: ["pm", "gantt"],
  siyuanPrimaryPage: null,
  siyuanExtraPages: [],
  completedAt: null,
  createdAt: "2026-03-20T08:00:00.000Z",
  updatedAt: "2026-03-20T08:00:00.000Z",
  projectName: "Lazycat",
  projectColor: "#409eff",
};

describe("pmGantt", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-03-29T12:00:00.000Z"));
  });

  it("按状态映射甘特图进度", () => {
    expect(getPmGanttProgress("todo")).toBe(0);
    expect(getPmGanttProgress("in_progress")).toBe(40);
    expect(getPmGanttProgress("testing")).toBe(75);
    expect(getPmGanttProgress("done")).toBe(100);
  });

  it("构建带置顶和逾期标记的甘特任务", () => {
    const task = buildPmGanttTask(
      {
        ...baseItem,
        endAt: "2026-03-28",
      },
    );

    expect(task.start).toBe("2026-03-28");
    expect(task.end).toBe("2026-03-28");
    expect(task.custom_class).toBe("gantt-p1");
    expect(task.pinned).toBe(true);
    expect(task.overdue).toBe(true);
    expect(task.statusLabel).toBe("进行中");
    expect(task.priorityLabel).toBe("P1 高");
  });

  it("用开始/截止日期的兜底值构建甘特任务", () => {
    const task = buildPmGanttTask(
      {
        ...baseItem,
        id: 8,
        startAt: null,
        endAt: "2026-04-01",
      },
    );

    expect(task.start).toBe("2026-04-01");
    expect(task.end).toBe("2026-04-01");
  });

  it("开始日期晚于截止日期时自动交换顺序", () => {
    const task = buildPmGanttTask(
      {
        ...baseItem,
        id: 11,
        startAt: "2026-04-09",
        endAt: "2026-04-03",
      },
    );

    expect(task.start).toBe("2026-04-03");
    expect(task.end).toBe("2026-04-09");
    expect(task.startLabel).toBe("2026-04-03");
    expect(task.endLabel).toBe("2026-04-09");
  });

  it("统计未排期事项并只输出已排期任务", () => {
    const items: PmItem[] = [
      baseItem,
      { ...baseItem, id: 9, startAt: null, endAt: null },
      { ...baseItem, id: 10, startAt: null, endAt: "2026-04-03" },
      { ...baseItem, id: 12, startAt: "invalid", endAt: null },
    ];

    expect(countPmGanttUnscheduledItems(items)).toBe(2);
    expect(buildPmGanttTasks(items).map((task) => task.itemId)).toEqual([7, 10]);
  });

  it("忽略带时间部分与非法值的历史日期差异", () => {
    const task = buildPmGanttTask({
      ...baseItem,
      id: 13,
      startAt: "2026-04-02T08:30:00.000Z",
      endAt: "invalid",
    });

    expect(task.start).toBe("2026-04-02");
    expect(task.end).toBe("2026-04-02");
  });

  it("按总览模式生成带项目元信息的悬浮卡", () => {
    const html = buildPmGanttPopupHtml(buildPmGanttTask(baseItem), { showProjectMeta: true });

    expect(html).toContain("补齐甘特图交互");
    expect(html).toContain("Lazycat");
    expect(html).toContain("P1 高");
    expect(html).toContain("进行中");
    expect(html).toContain("已置顶");
    expect(html).toContain("2026-03-28 ~ 2026-03-30");
  });

  it("悬浮卡靠近底部时向上翻转并钳制在可视区内", () => {
    const position = clampPmGanttPopupPosition({
      anchorX: 420,
      anchorY: 380,
      popupWidth: 220,
      popupHeight: 140,
      viewportWidth: 640,
      viewportHeight: 400,
      scrollLeft: 120,
      scrollTop: 80,
    });

    expect(position.left).toBe(430);
    expect(position.top).toBe(230);
  });

  it("悬浮卡靠近右侧时向左回退并保持边距", () => {
    const position = clampPmGanttPopupPosition({
      anchorX: 530,
      anchorY: 180,
      popupWidth: 220,
      popupHeight: 120,
      viewportWidth: 560,
      viewportHeight: 360,
      scrollLeft: 120,
      scrollTop: 40,
    });

    expect(position.left).toBe(300);
    expect(position.top).toBe(170);
  });
});

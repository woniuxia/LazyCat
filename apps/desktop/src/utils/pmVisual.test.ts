import { describe, expect, it } from "vitest";

import type { PmProject } from "../types/pm";

import {
  getPmPendingCount,
  getPmTotalCount,
  sortPmProjectsForSidebar,
  summarizePmItemTags,
} from "./pmVisual";

const createProject = (id: number, name: string, status: PmProject["status"], sortOrder: number): PmProject => ({
  id,
  name,
  description: "",
  color: "#409eff",
  status,
  siyuanLocationOverride: null,
  sortOrder,
  createdAt: "2026-04-04T00:00:00.000Z",
  updatedAt: "2026-04-04T00:00:00.000Z",
});

describe("pmVisual", () => {
  it("正确计算待办数与总数", () => {
    expect(getPmPendingCount({ total: 8, done: 3 })).toBe(5);
    expect(getPmPendingCount({ total: 1, done: 3 })).toBe(0);
    expect(getPmPendingCount()).toBe(0);

    expect(getPmTotalCount({ total: 8, done: 3 })).toBe(8);
    expect(getPmTotalCount({ total: -1, done: 0 })).toBe(0);
    expect(getPmTotalCount()).toBe(0);
  });

  it("active 项目按总任务数排序，archived 项目统一置后并走稳定次序", () => {
    const projects = [
      createProject(1, "数据中台升级", "active", 2),
      createProject(2, "官网重构", "active", 1),
      createProject(3, "老版本迁移", "archived", 4),
      createProject(4, "移动端改版", "active", 3),
      createProject(5, "历史需求池", "archived", 0),
    ];

    const sorted = sortPmProjectsForSidebar(projects, {
      1: { total: 7, done: 4 },
      2: { total: 11, done: 3 },
      3: { total: 4, done: 4 },
      4: { total: 11, done: 6 },
      5: { total: 99, done: 97 },
    });

    expect(sorted.map((project) => project.id)).toEqual([2, 4, 1, 5, 3]);
    expect(sorted[0].pendingCount).toBe(8);
    expect(sorted[2].pendingCount).toBe(3);
    expect(sorted[3].status).toBe("archived");
    expect(sorted[3].pendingCount).toBe(2);
  });

  it("缺失计数时按 0 参与排序", () => {
    const projects = [
      createProject(1, "甲", "active", 2),
      createProject(2, "乙", "active", 1),
    ];

    const sorted = sortPmProjectsForSidebar(projects, {});
    expect(sorted.map((project) => project.id)).toEqual([2, 1]);
  });

  it("标签摘要最多保留前两个标签并输出隐藏数量", () => {
    expect(summarizePmItemTags(["前端", "登录"])).toEqual({
      visibleTags: ["前端", "登录"],
      hiddenCount: 0,
    });

    expect(summarizePmItemTags(["前端", "登录", "接口", "风控"])).toEqual({
      visibleTags: ["前端", "登录"],
      hiddenCount: 2,
    });
  });
});

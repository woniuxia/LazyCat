import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invokeToolByChannel = vi.fn();
const invoke = vi.fn();

vi.mock("../../bridge/tauri", () => ({
  invokeToolByChannel: (...args: unknown[]) => invokeToolByChannel(...args),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import { launcherProvider } from "./launcher";
import { pmProvider } from "./pm";
import { todoProvider } from "./todo";

beforeEach(() => {
  invokeToolByChannel.mockReset();
  invoke.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("Spotlight lightweight provider sources", () => {
  it("loads launcher entries without the full launcher projection", async () => {
    invokeToolByChannel.mockResolvedValue({
      items: [
        {
          id: 7,
          name: "IDE",
          exe_path: "C:\\Tools\\ide.exe",
          arguments: "--reuse-window",
          group_name: "开发",
        },
      ],
    });

    const items = await launcherProvider.prefetch();

    expect(invokeToolByChannel).toHaveBeenCalledWith("tool:launcher:spotlight-list", {});
    expect(items[0]).toMatchObject({
      itemId: "7",
      title: "IDE",
      subtitle: "开发",
      payload: { arguments: "--reuse-window" },
    });
  });

  it("loads the normalized Todo Spotlight projection", async () => {
    invokeToolByChannel.mockResolvedValue({
      items: [
        {
          id: 3,
          title: "发布版本",
          status: "pending",
          priority: "P0",
          pinned: true,
          typeName: "发布",
          isOverdue: true,
        },
      ],
    });

    const items = await todoProvider.prefetch();

    expect(invokeToolByChannel).toHaveBeenCalledWith("tool:todo:spotlight-list", {});
    expect(items[0]).toMatchObject({
      itemId: "3",
      title: "发布版本",
      subtitle: "发布",
      status: { text: "已逾期", tone: "danger" },
      ranking: {
        pinned: true,
        contextual: true,
        recommendationEligible: false,
        usageRef: {
          resourceType: "todo-item",
          resourceId: "3",
          actions: ["open"],
        },
      },
    });
  });

  it("recommends only non-overdue Todo items due today without dropping search candidates", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 7, 3, 10, 0, 0));
    invokeToolByChannel.mockResolvedValue({
      items: [
        {
          id: 1,
          title: "今日待发布",
          eventAt: new Date(2026, 7, 3, 18, 0, 0).toISOString(),
          isOverdue: false,
        },
        {
          id: 2,
          title: "今日已逾期",
          eventAt: new Date(2026, 7, 3, 9, 0, 0).toISOString(),
          isOverdue: true,
        },
        {
          id: 3,
          title: "明日任务",
          eventAt: new Date(2026, 7, 4, 9, 0, 0).toISOString(),
          isOverdue: false,
        },
        {
          id: 4,
          title: "无日期任务",
          displayAt: new Date(2026, 7, 3, 8, 0, 0).toISOString(),
          isOverdue: false,
        },
      ],
    });

    const items = await todoProvider.prefetch();

    expect(items).toHaveLength(4);
    expect(items.map((item) => item.ranking?.recommendationEligible)).toEqual([
      true,
      false,
      false,
      false,
    ]);
    expect(items[0].status).toEqual({ text: "今日", tone: "warn" });
    expect(items[1].status).toEqual({ text: "已逾期", tone: "danger" });
    expect(items[3].status).toBeUndefined();
  });

  it("marks Todo items completed and requests a provider refresh", async () => {
    invokeToolByChannel.mockResolvedValue({ ok: true });
    const item = {
      providerId: "todo" as const,
      itemId: "3",
      title: "发布版本",
      searchFields: [],
      payload: { todoId: 3 },
    };

    expect(todoProvider.buildActions?.(item).map((action) => action.id)).toEqual([
      "open_todo",
      "mark_done",
    ]);
    const result = await todoProvider.executeAction?.(item, "mark_done", {} as never);

    expect(invokeToolByChannel).toHaveBeenCalledWith("tool:todo:item-change-status", {
      id: 3,
      status: "done",
    });
    expect(result).toMatchObject({
      refreshProvider: true,
      toast: { message: "已标记完成", type: "success" },
    });
  });

  it("uses the PM projection project name without a second project request", async () => {
    invokeToolByChannel.mockResolvedValue([
      {
        id: 11,
        projectId: 2,
        projectName: "桌面端",
        title: "优化 Spotlight",
        status: "in_progress",
        priority: "P1",
        pinned: false,
        tags: ["搜索", "性能"],
      },
    ]);

    const items = await pmProvider.prefetch();

    expect(invokeToolByChannel).toHaveBeenCalledTimes(1);
    expect(invokeToolByChannel).toHaveBeenCalledWith("tool:pm:spotlight-list", {});
    expect(items[0]).toMatchObject({
      itemId: "11",
      title: "优化 Spotlight",
      subtitle: "桌面端",
      payload: { projectId: 2, projectName: "桌面端" },
      ranking: {
        usageRef: {
          resourceType: "pm-item",
          resourceId: "11",
          actions: ["open"],
        },
      },
    });
    expect(items[0].searchFields.some((field) => field.text === "搜索 性能")).toBe(true);
  });

  it("records Todo and PM opens only after navigation succeeds", async () => {
    invoke.mockResolvedValue(undefined);
    invokeToolByChannel.mockResolvedValue({ ok: true });
    const todoItem = {
      providerId: "todo" as const,
      itemId: "3",
      title: "发布版本",
      searchFields: [],
      payload: { todoId: 3 },
    };
    const pmItem = {
      providerId: "pm" as const,
      itemId: "11",
      title: "优化 Spotlight",
      searchFields: [],
      payload: { pmId: 11, projectId: 2 },
    };

    await todoProvider.defaultAction(todoItem, {} as never);
    await pmProvider.defaultAction(pmItem, {} as never);

    expect(invokeToolByChannel).toHaveBeenNthCalledWith(1, "tool:todo:item-record-open", { id: 3 });
    expect(invokeToolByChannel).toHaveBeenNthCalledWith(2, "tool:pm:item-record-open", { id: 11 });
    expect(invoke.mock.invocationCallOrder[0]).toBeLessThan(
      invokeToolByChannel.mock.invocationCallOrder[0],
    );

    invoke.mockReset();
    invokeToolByChannel.mockReset();
    invoke.mockRejectedValue(new Error("主窗口不可用"));
    await expect(todoProvider.defaultAction(todoItem, {} as never)).rejects.toThrow("主窗口不可用");
    expect(invokeToolByChannel).not.toHaveBeenCalled();
  });
});

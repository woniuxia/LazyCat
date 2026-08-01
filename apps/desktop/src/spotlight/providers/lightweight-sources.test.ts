import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeToolByChannel = vi.fn();

vi.mock("../../bridge/tauri", () => ({
  invokeToolByChannel: (...args: unknown[]) => invokeToolByChannel(...args),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { launcherProvider } from "./launcher";
import { pmProvider } from "./pm";
import { todoProvider } from "./todo";

beforeEach(() => {
  invokeToolByChannel.mockReset();
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
      ranking: { pinned: true },
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
    });
    expect(items[0].searchFields.some((field) => field.text === "搜索 性能")).toBe(true);
  });
});

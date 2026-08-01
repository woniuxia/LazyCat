import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeToolByChannel = vi.fn();
const invoke = vi.fn();

vi.mock("../../bridge/tauri", () => ({
  invokeToolByChannel: (...args: unknown[]) => invokeToolByChannel(...args),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import { actionCenterProvider, buildActionCombinationSpotlightItem } from "./action-center";

beforeEach(() => {
  invoke.mockReset();
  invokeToolByChannel.mockReset();
});

describe("actionCenterProvider", () => {
  it("maps combination summaries to searchable status items", () => {
    const item = buildActionCombinationSpotlightItem({
      id: 7,
      name: "客户门户开发环境",
      executionMode: "serial",
      stepCount: 4,
      latestRunStatus: "partially_succeeded",
      updatedAt: "2026-07-30T10:00:00+08:00",
    });

    expect(item.providerId).toBe("action-center");
    expect(item.itemId).toBe("7");
    expect(item.subtitle).toBe("串行 · 4 个步骤");
    expect(item.status).toEqual({ text: "部分成功", tone: "warn" });
    expect(item.searchFields.map((field) => field.text)).toContain("客户门户开发环境");
    expect(item.ranking?.usageRef).toEqual({
      resourceType: "action-combination",
      resourceId: "7",
      actions: ["run"],
    });
  });

  it("prefetches saved combinations", async () => {
    invokeToolByChannel.mockResolvedValue({
      combinations: [
        {
          id: 7,
          name: "开发环境",
          executionMode: "parallel",
          stepCount: 3,
          latestRunStatus: "succeeded",
          updatedAt: "2026-07-30T10:00:00+08:00",
        },
      ],
    });

    const items = await actionCenterProvider.prefetch();

    expect(invokeToolByChannel).toHaveBeenCalledWith("tool:action-center:combination-list", {});
    expect(items[0].subtitle).toBe("并行 · 3 个步骤");
  });

  it("starts a combination with terminal notification enabled", async () => {
    const item = buildActionCombinationSpotlightItem({
      id: 7,
      name: "开发环境",
      executionMode: "serial",
      stepCount: 3,
      updatedAt: "2026-07-30T10:00:00+08:00",
    });

    const result = await actionCenterProvider.defaultAction(item, {} as never);

    expect(invokeToolByChannel).toHaveBeenCalledWith("tool:action-center:combination-run", {
      combinationId: 7,
      notifyOnCompletion: true,
    });
    expect(result).toEqual({
      closeSpotlight: true,
      toast: { message: "已开始运行 开发环境", type: "success" },
    });
  });

  it("opens the selected combination in action center", async () => {
    const item = buildActionCombinationSpotlightItem({
      id: 7,
      name: "开发环境",
      executionMode: "serial",
      stepCount: 3,
      updatedAt: "2026-07-30T10:00:00+08:00",
    });

    const result = await actionCenterProvider.executeAction?.(item, "open", {} as never);

    expect(invoke).toHaveBeenCalledWith("spotlight_pick", {
      target: "action-center",
      itemId: "7",
      view: "combination",
    });
    expect(result).toEqual({ closeSpotlight: true });
  });

  it("surfaces active-run and malformed-item failures", async () => {
    invokeToolByChannel.mockRejectedValueOnce(new Error("已有组合动作正在运行"));
    const item = buildActionCombinationSpotlightItem({
      id: 7,
      name: "开发环境",
      executionMode: "serial",
      stepCount: 3,
      updatedAt: "2026-07-30T10:00:00+08:00",
    });

    await expect(actionCenterProvider.defaultAction(item, {} as never)).resolves.toEqual({
      errorMessage: "已有组合动作正在运行",
    });
    await expect(
      actionCenterProvider.defaultAction(
        {
          providerId: "action-center",
          itemId: "bad",
          title: "bad",
          searchFields: [],
          payload: {},
        },
        {} as never,
      ),
    ).resolves.toEqual({ errorMessage: "动作组合数据无效" });
  });
});

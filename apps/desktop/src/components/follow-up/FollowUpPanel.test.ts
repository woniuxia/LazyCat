// @vitest-environment happy-dom
import { createApp, nextTick } from "vue";
import ElementPlus from "element-plus";
import { ElMessageBox } from "element-plus";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FollowUpItem } from "../../types/follow-up";

const bridge = vi.hoisted(() => ({ invoke: vi.fn() }));
const eventBridge = vi.hoisted(() => ({ listen: vi.fn() }));
vi.mock("../../bridge/tauri", () => ({ invokeToolByChannel: bridge.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: eventBridge.listen }));

import FollowUpPanel from "./FollowUpPanel.vue";

function followUp(
  id: number,
  title: string,
  reviewAt: string | null,
  endedAt: string | null = null,
): FollowUpItem {
  return {
    id,
    title,
    description: "上下文",
    expectedOutcome: "通过验收",
    priority: id === 1 ? "P0" : "P2",
    attentionStatus: endedAt ? "ended" : "active",
    externalResult: "unknown",
    endingMode: endedAt ? "stopped_following" : null,
    personId: 1,
    personName: "张三",
    personNameSnapshot: "张三",
    reviewAt,
    expectedCompletionAt: id === 1 ? "2026-08-18T08:00:00+08:00" : null,
    snoozeUntil: null,
    lastNotifiedReviewAt: null,
    endedAt,
    createdAt: "2026-08-01T00:00:00Z",
    updatedAt: "2026-08-01T00:00:00Z",
    latestProgress:
      id === 1
        ? {
            id: 10,
            kind: "progress",
            content: "等待最终确认",
            occurredAt: "2026-08-18T07:00:00Z",
            updatedAt: "2026-08-18T07:00:00Z",
          }
        : null,
    progress: [],
    links: [],
  };
}

const items = [
  followUp(3, "以后事项", "2026-08-27T10:00:00+08:00"),
  followUp(2, "近期事项", "2026-08-20T10:00:00+08:00"),
  followUp(1, "现在复查", "2026-08-18T09:00:00+08:00"),
  followUp(4, "已结束事项", null, "2026-08-17T10:00:00+08:00"),
];

async function mountPanel() {
  const root = document.createElement("div");
  document.body.append(root);
  const app = createApp(FollowUpPanel);
  app.use(ElementPlus);
  const panel = app.mount(root) as unknown as { loadItems: () => Promise<void> };
  await nextTick();
  await vi.runAllTimersAsync();
  await nextTick();
  return { app, panel, root };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function buttonByText(container: ParentNode, text: string) {
  return Array.from(container.querySelectorAll<HTMLButtonElement>("button")).find(
    (button) => button.textContent?.trim() === text,
  );
}

describe("FollowUpPanel", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-18T10:00:00+08:00"));
    bridge.invoke.mockImplementation((channel: string) => {
      if (channel === "tool:todo:assignee-list")
        return Promise.resolve({ items: [{ id: 1, name: "张三", createdAt: "", updatedAt: "" }] });
      if (channel === "tool:follow-up:item-list") return Promise.resolve(items);
      return Promise.resolve({ ok: true });
    });
    eventBridge.listen.mockResolvedValue(vi.fn());
  });
  afterEach(() => {
    vi.useRealTimers();
    document.body.innerHTML = "";
    bridge.invoke.mockReset();
    eventBridge.listen.mockReset();
    vi.restoreAllMocks();
  });

  it("defaults to all items and switches to a review group", async () => {
    const { app, root } = await mountPanel();
    const sectionButtons = Array.from(root.querySelectorAll<HTMLButtonElement>(".section-button"));
    expect(sectionButtons[0]?.textContent).toContain("全部");
    expect(sectionButtons[0]?.textContent).toContain("4");
    expect(sectionButtons[0]?.classList.contains("active")).toBe(true);
    expect(root.textContent).toContain("待复查");
    expect(root.textContent).toContain("现在复查");
    expect(root.textContent).toContain("以后事项");
    expect(root.textContent).toContain("已结束事项");
    expect(root.textContent).toContain("等待最终确认");
    expect(root.textContent).toContain("外部期限已到");
    const firstCard = root.querySelector(".follow-up-card");
    expect(firstCard?.querySelector(".card-title-row > .priority-tag")?.textContent).toBe("P0");
    expect(firstCard?.querySelector(".card-supporting .el-tag--danger")?.textContent).toBe(
      "外部期限已到",
    );
    const later = Array.from(root.querySelectorAll("button")).find((button) =>
      button.textContent?.includes("以后复查"),
    );
    later?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    await nextTick();
    expect(root.textContent).toContain("以后事项");
    expect(root.querySelector(".follow-up-scroll")?.textContent).not.toContain("现在复查");
    app.unmount();
  });

  it("opens the selected detail and keeps lifecycle information visible", async () => {
    const { app, root } = await mountPanel();
    const card = root.querySelector<HTMLButtonElement>(".follow-up-card");
    card?.click();
    await nextTick();
    expect(root.querySelector(".follow-up-detail-pane")?.textContent).toContain("现在复查");
    expect(root.querySelector(".follow-up-detail-pane")?.textContent).toContain("预期结果");
    expect(root.querySelector(".follow-up-detail-pane")?.textContent).toContain("继续关注");
    expect(
      root.querySelector<HTMLButtonElement>('.mobile-back[title="返回关注事项列表"]'),
    ).not.toBeNull();
    app.unmount();
  });

  it("keeps the newest list response when requests finish out of order", async () => {
    const { app, panel, root } = await mountPanel();
    const older = deferred<FollowUpItem[]>();
    const newer = deferred<FollowUpItem[]>();
    bridge.invoke
      .mockImplementationOnce(() => older.promise)
      .mockImplementationOnce(() => newer.promise);

    const olderLoad = panel.loadItems();
    const newerLoad = panel.loadItems();
    newer.resolve([followUp(11, "新筛选结果", "2026-08-18T09:30:00+08:00")]);
    await newerLoad;
    older.resolve([followUp(12, "旧筛选结果", "2026-08-18T09:00:00+08:00")]);
    await olderLoad;
    await nextTick();

    expect(root.textContent).toContain("新筛选结果");
    expect(root.textContent).not.toContain("旧筛选结果");
    app.unmount();
  });

  it("reopens failed progress input with the user's content intact", async () => {
    const prompt = vi
      .spyOn(ElMessageBox, "prompt")
      .mockResolvedValueOnce({ value: "尚待对方确认", action: "confirm" } as never)
      .mockRejectedValueOnce("cancel");
    bridge.invoke.mockImplementation((channel: string) => {
      if (channel === "tool:todo:assignee-list")
        return Promise.resolve({ items: [{ id: 1, name: "张三", createdAt: "", updatedAt: "" }] });
      if (channel === "tool:follow-up:item-list") return Promise.resolve(items);
      if (channel === "tool:follow-up:progress-add") return Promise.reject(new Error("写入失败"));
      return Promise.resolve({ ok: true });
    });
    const { app, root } = await mountPanel();
    root.querySelector<HTMLButtonElement>(".follow-up-card")?.click();
    await nextTick();
    buttonByText(root, "记录进展")?.click();
    await vi.waitFor(() => expect(prompt).toHaveBeenCalledTimes(2));

    expect(prompt.mock.calls[1]?.[2]).toMatchObject({ inputValue: "尚待对方确认" });
    app.unmount();
  });

  it("submits lifecycle actions through the confirmed domain channel", async () => {
    const { app, root } = await mountPanel();
    root.querySelector<HTMLButtonElement>(".follow-up-card")?.click();
    await nextTick();
    buttonByText(root, "继续关注")?.click();
    await nextTick();
    const dialog = document.body.querySelector<HTMLElement>(".el-dialog");
    const textarea = dialog?.querySelector<HTMLTextAreaElement>("textarea");
    if (!dialog || !textarea) throw new Error("lifecycle dialog did not open");
    textarea.value = "继续等待验收";
    textarea.dispatchEvent(new Event("input", { bubbles: true }));
    buttonByText(dialog, "确认")?.click();
    await vi.waitFor(() =>
      expect(bridge.invoke).toHaveBeenCalledWith(
        "tool:follow-up:continue",
        expect.objectContaining({ id: 1, content: "继续等待验收" }),
      ),
    );
    app.unmount();
  });
});

// @vitest-environment happy-dom
import { createApp, nextTick } from "vue";
import ElementPlus from "element-plus";
import { ElMessageBox } from "element-plus";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FollowUpItem } from "../../types/follow-up";
import { APP_EVENTS } from "../../bridge/events";

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

async function mountPanel(props: Record<string, unknown> = {}) {
  const root = document.createElement("div");
  document.body.append(root);
  const app = createApp(FollowUpPanel, props);
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

function buttonIncluding(container: ParentNode, text: string) {
  return Array.from(container.querySelectorAll<HTMLButtonElement>("button")).find((button) =>
    button.textContent?.includes(text),
  );
}

function sectionButton(root: HTMLElement, label: string) {
  return Array.from(root.querySelectorAll<HTMLButtonElement>(".section-button")).find((button) =>
    button.textContent?.includes(label),
  );
}

function cardIncluding(root: HTMLElement, text: string) {
  return Array.from(root.querySelectorAll<HTMLButtonElement>(".follow-up-card")).find((card) =>
    card.textContent?.includes(text),
  );
}

function mockUpdatingList(apply: (channel: string, list: FollowUpItem[]) => FollowUpItem[]) {
  let list = [...items];
  bridge.invoke.mockImplementation((channel: string) => {
    if (channel === "tool:todo:assignee-list")
      return Promise.resolve({ items: [{ id: 1, name: "张三", createdAt: "", updatedAt: "" }] });
    if (channel === "tool:follow-up:item-list") return Promise.resolve(list);
    if (channel === "tool:settings:get") return Promise.resolve({ value: null });
    if (channel.startsWith("tool:follow-up:")) {
      list = apply(channel, list);
      return Promise.resolve(list.find((entry) => entry.id === 1) ?? { ok: true });
    }
    return Promise.resolve({ ok: true });
  });
}

async function switchToDueGroup(root: HTMLElement) {
  sectionButton(root, "待复查")?.click();
  await nextTick();
}

async function selectDueItem(root: HTMLElement) {
  cardIncluding(root, "现在复查")?.click();
  await nextTick();
}

async function submitLifecycleDialog(root: ParentNode, triggerText: string, content: string) {
  buttonByText(root, triggerText)?.click();
  await nextTick();
  const dialog = document.body.querySelector<HTMLElement>(".el-dialog");
  const textarea = dialog?.querySelector<HTMLTextAreaElement>("textarea");
  if (!dialog || !textarea) throw new Error("lifecycle dialog did not open");
  textarea.value = content;
  textarea.dispatchEvent(new Event("input", { bubbles: true }));
  buttonByText(dialog, "确认")?.click();
  await nextTick();
}

async function clickDetailDropdownItem(root: HTMLElement, trigger: string, label: string) {
  const triggerButton =
    root.querySelector<HTMLButtonElement>(`button[title="${trigger}"]`) ??
    buttonIncluding(root, trigger);
  triggerButton?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  await nextTick();
  const item = Array.from(document.body.querySelectorAll<HTMLElement>(".el-dropdown-menu__item"))
    .find((option) => option.textContent?.trim() === label);
  if (!item) throw new Error(`dropdown item ${label} did not render`);
  item.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  await nextTick();
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

  it("reports the unfiltered due count for the task view badge", async () => {
    const onDueCountChange = vi.fn();
    const { app } = await mountPanel({ onDueCountChange });

    expect(onDueCountChange).toHaveBeenCalledWith(1);
    app.unmount();
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
    expect(firstCard?.querySelector(".card-supporting .el-tag--warning")?.textContent).toBe(
      "待复查",
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

  it("shows the due state even when an item has no secondary metadata", async () => {
    const dueOnly = followUp(20, "仅待复查", "2026-08-18T09:00:00+08:00");
    bridge.invoke.mockImplementation((channel: string) => {
      if (channel === "tool:todo:assignee-list")
        return Promise.resolve({ items: [{ id: 1, name: "张三", createdAt: "", updatedAt: "" }] });
      if (channel === "tool:follow-up:item-list") return Promise.resolve([dueOnly]);
      return Promise.resolve({ ok: true });
    });

    const { app, root } = await mountPanel();
    const card = root.querySelector(".follow-up-card");
    expect(card?.querySelector(".card-supporting")?.textContent).toContain("待复查");
    app.unmount();
  });

  it("opens the selected detail and keeps lifecycle information visible", async () => {
    const { app, root } = await mountPanel();
    const card = root.querySelector<HTMLButtonElement>(".follow-up-card");
    card?.click();
    await nextTick();
    expect(root.querySelector(".follow-up-detail-pane")?.textContent).toContain("现在复查");
    expect(root.querySelector(".follow-up-detail-pane")?.textContent).toContain("预期结果");
    expect(root.querySelector(".follow-up-detail-pane")?.textContent).toContain("关注状态");
    expect(root.querySelector(".follow-up-detail-pane")?.textContent).toContain("外部结果");
    expect(root.querySelector(".follow-up-detail-pane")?.textContent).toContain("继续关注");
    expect(root.querySelector(".follow-up-detail-pane")?.textContent).toContain("确认结果");
    expect(root.querySelector(".follow-up-detail-pane")?.textContent).toContain("记录第一条进展");
    const backButton = root.querySelector<HTMLButtonElement>(
      '.task-workspace-detail-backbar button[title="返回列表"]',
    );
    expect(backButton).not.toBeNull();
    backButton?.click();
    await nextTick();
    expect(root.querySelector(".follow-up-detail-pane")?.textContent).toContain("选择一项查看详情");
    app.unmount();
  });

  it("keeps ended attention separate from an unknown external result", async () => {
    const { app, root } = await mountPanel();
    const endedCard = Array.from(root.querySelectorAll<HTMLButtonElement>(".follow-up-card")).find(
      (card) => card.textContent?.includes("已结束事项"),
    );
    endedCard?.click();
    await nextTick();

    const detail = root.querySelector(".follow-up-detail-pane");
    expect(detail?.textContent).toContain("已结束关注");
    expect(detail?.textContent).toContain("外部结果");
    expect(detail?.textContent).toContain("结果未知");
    expect(detail?.textContent).toContain("不再复查");
    app.unmount();
  });

  it("rejects missing required input and keeps the create dialog open", async () => {
    const { app, root } = await mountPanel();
    buttonByText(root, "新增关注事项")?.click();
    await nextTick();

    const dialog = document.body.querySelector<HTMLElement>(".el-dialog");
    const saveButton = buttonByText(dialog ?? document.body, "保存");
    if (!saveButton) throw new Error("save button did not render");
    saveButton.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    await nextTick();

    expect(bridge.invoke).not.toHaveBeenCalledWith("tool:follow-up:item-create", expect.anything());
    expect(document.body.querySelector(".el-dialog")).not.toBeNull();
    app.unmount();
  });

  it("uses stepped controls for quarter-hour review selection", async () => {
    const { app, root } = await mountPanel();
    buttonByText(root, "新增关注事项")?.click();
    await nextTick();

    const dialog = document.body.querySelector<HTMLElement>(".el-dialog");
    const dateInput = dialog?.querySelector<HTMLInputElement>(".review-date-time input");
    const timeInput = dialog?.querySelector<HTMLInputElement>(".follow-up-review-time input");
    const selectedTimes = Array.from(
      dialog?.querySelectorAll<HTMLElement>(".follow-up-review-time .el-select__selected-item") ??
        [],
      (item) => item.textContent?.trim(),
    );
    if (!dateInput || !timeInput) throw new Error("review date and time inputs did not render");
    expect(dateInput.value).toBe("2026-08-19");
    expect(selectedTimes).toContain("09:00");
    timeInput.click();
    await nextTick();

    const options = Array.from(
      document.body.querySelectorAll<HTMLElement>(".el-select-dropdown__item"),
      (option) => option.textContent?.trim(),
    ).filter((option): option is string => Boolean(option && /^\d{2}:\d{2}$/.test(option)));
    expect(options.slice(0, 5)).toEqual(["00:00", "00:15", "00:30", "00:45", "01:00"]);
    expect(options).not.toContain("00:01");
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

  it("keeps progress input open after a failed save", async () => {
    bridge.invoke.mockImplementation((channel: string) => {
      if (channel === "tool:todo:assignee-list")
        return Promise.resolve({ items: [{ id: 1, name: "张三", createdAt: "", updatedAt: "" }] });
      if (channel === "tool:follow-up:item-list") return Promise.resolve(items);
      if (channel === "tool:settings:get") return Promise.resolve({ value: null });
      if (channel === "tool:follow-up:progress-add") return Promise.reject(new Error("写入失败"));
      return Promise.resolve({ ok: true });
    });
    const { app, root } = await mountPanel();
    root.querySelector<HTMLButtonElement>(".follow-up-card")?.click();
    await nextTick();
    buttonByText(root, "记录进展")?.click();
    await nextTick();
    const dialog = document.body.querySelector<HTMLElement>(".el-dialog");
    const textarea = dialog?.querySelector<HTMLTextAreaElement>("textarea");
    if (!dialog || !textarea) throw new Error("progress dialog did not open");
    textarea.value = "尚待对方确认";
    textarea.dispatchEvent(new Event("input", { bubbles: true }));
    buttonByText(dialog, "保存")?.click();
    await vi.waitFor(() =>
      expect(bridge.invoke).toHaveBeenCalledWith("tool:follow-up:progress-add", {
        id: 1,
        content: "尚待对方确认",
      }),
    );
    expect(textarea.value).toBe("尚待对方确认");
    app.unmount();
  });

  it("appends shared quick inputs in progress and lifecycle dialogs", async () => {
    bridge.invoke.mockImplementation((channel: string) => {
      if (channel === "tool:todo:assignee-list")
        return Promise.resolve({ items: [{ id: 1, name: "张三", createdAt: "", updatedAt: "" }] });
      if (channel === "tool:follow-up:item-list") return Promise.resolve(items);
      if (channel === "tool:settings:get") return Promise.resolve({ value: null });
      return Promise.resolve({ ok: true });
    });
    const { app, root } = await mountPanel();
    root.querySelector<HTMLButtonElement>(".follow-up-card")?.click();
    await nextTick();
    buttonByText(root, "记录进展")?.click();
    await nextTick();
    let dialog = document.body.querySelector<HTMLElement>(".el-dialog");
    buttonByText(dialog ?? document.body, "等待对方反馈")?.click();
    await nextTick();
    expect(dialog?.querySelector<HTMLTextAreaElement>("textarea")?.value).toBe("等待对方反馈");
    buttonByText(dialog ?? document.body, "取消")?.click();
    await nextTick();
    buttonByText(root, "继续关注")?.click();
    await nextTick();
    dialog = document.body.querySelector<HTMLElement>(".el-dialog");
    const textarea = dialog?.querySelector<HTMLTextAreaElement>("textarea");
    if (!textarea) throw new Error("lifecycle input did not render");
    textarea.value = "已有内容";
    textarea.dispatchEvent(new Event("input", { bubbles: true }));
    buttonByText(dialog ?? document.body, "等待对方反馈")?.click();
    await nextTick();
    expect(textarea.value).toBe("已有内容\n等待对方反馈");
    expect(bridge.invoke).toHaveBeenCalledWith(
      "tool:settings:set",
      expect.objectContaining({ key: "follow-up.quick-inputs" }),
    );
    app.unmount();
  });
  it("adds, edits and deletes a custom quick input with confirmation", async () => {
    const prompt = vi
      .spyOn(ElMessageBox, "prompt")
      .mockResolvedValueOnce({ value: "自定义内容", action: "confirm" } as never)
      .mockResolvedValueOnce({ value: "已修改内容", action: "confirm" } as never);
    const confirm = vi.spyOn(ElMessageBox, "confirm").mockResolvedValue("confirm" as never);
    bridge.invoke.mockImplementation((channel: string) => {
      if (channel === "tool:todo:assignee-list")
        return Promise.resolve({ items: [{ id: 1, name: "张三", createdAt: "", updatedAt: "" }] });
      if (channel === "tool:follow-up:item-list") return Promise.resolve(items);
      if (channel === "tool:settings:get") return Promise.resolve({ value: "[]" });
      return Promise.resolve({ ok: true });
    });
    const { app, root } = await mountPanel();
    root.querySelector<HTMLButtonElement>(".follow-up-card")?.click();
    await nextTick();
    buttonByText(root, "记录进展")?.click();
    await nextTick();
    const dialog = document.body.querySelector<HTMLElement>(".el-dialog");
    buttonByText(dialog ?? document.body, "添加")?.click();
    await vi.waitFor(() =>
      expect(buttonByText(dialog ?? document.body, "自定义内容")).toBeDefined(),
    );

    let item = Array.from(dialog?.querySelectorAll<HTMLElement>(".quick-inputs__item") ?? []).find(
      (candidate) => candidate.textContent?.includes("自定义内容"),
    );
    item?.querySelector<HTMLButtonElement>('button[title="编辑快速输入"]')?.click();
    await vi.waitFor(() => expect(prompt).toHaveBeenCalledTimes(2));
    await vi.waitFor(() =>
      expect(buttonByText(dialog ?? document.body, "已修改内容")).toBeDefined(),
    );

    item = Array.from(dialog?.querySelectorAll<HTMLElement>(".quick-inputs__item") ?? []).find(
      (candidate) => candidate.textContent?.includes("已修改内容"),
    );
    item?.querySelector<HTMLButtonElement>('button[title="删除快速输入"]')?.click();
    await vi.waitFor(() =>
      expect(confirm).toHaveBeenCalledWith("确定删除这条快速输入吗？", "删除快速输入", {
        type: "warning",
      }),
    );
    await vi.waitFor(() =>
      expect(buttonByText(dialog ?? document.body, "已修改内容")).toBeUndefined(),
    );
    const settingWrites = bridge.invoke.mock.calls.filter(
      ([channel]) => channel === "tool:settings:set",
    );
    expect(settingWrites.at(-1)?.[1]).toMatchObject({ value: "[]" });
    app.unmount();
  });
  it("serializes rapid usage updates without losing counts", async () => {
    const firstSave = deferred<unknown>();
    let settingWrites = 0;
    bridge.invoke.mockImplementation((channel: string, payload: { value?: string }) => {
      if (channel === "tool:todo:assignee-list")
        return Promise.resolve({ items: [{ id: 1, name: "张三", createdAt: "", updatedAt: "" }] });
      if (channel === "tool:follow-up:item-list") return Promise.resolve(items);
      if (channel === "tool:settings:get")
        return Promise.resolve({
          value: JSON.stringify([
            { id: "quick-1", text: "常用内容", usageCount: 0, lastUsedAt: null, createdAt: 1 },
          ]),
        });
      if (channel === "tool:settings:set") {
        settingWrites += 1;
        return settingWrites === 1 ? firstSave.promise : Promise.resolve(payload);
      }
      return Promise.resolve({ ok: true });
    });
    const { app, root } = await mountPanel();
    root.querySelector<HTMLButtonElement>(".follow-up-card")?.click();
    await nextTick();
    buttonByText(root, "记录进展")?.click();
    await nextTick();
    const dialog = document.body.querySelector<HTMLElement>(".el-dialog");
    const quickButton = buttonByText(dialog ?? document.body, "常用内容");
    quickButton?.click();
    quickButton?.click();
    await nextTick();

    const writes = () =>
      bridge.invoke.mock.calls.filter(([channel]) => channel === "tool:settings:set");
    expect(writes()).toHaveLength(1);
    expect(JSON.parse(writes()[0]![1].value)[0].usageCount).toBe(1);
    firstSave.resolve({ ok: true });
    await vi.waitFor(() => expect(writes()).toHaveLength(2));
    expect(JSON.parse(writes()[1]![1].value)[0].usageCount).toBe(2);
    app.unmount();
  });

  it("does not offer result quick inputs when stopping attention", async () => {
    const { app, root } = await mountPanel();
    root.querySelector<HTMLButtonElement>(".follow-up-card")?.click();
    await nextTick();
    buttonByText(root, "结束关注")?.click();
    await nextTick();
    const dialog = document.body.querySelector<HTMLElement>(".el-dialog");
    expect(dialog?.textContent).toContain("结束原因");
    expect(dialog?.querySelector(".quick-inputs")).toBeNull();
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

  it("keeps the due group open and fades the continued item in place", async () => {
    mockUpdatingList((channel, list) =>
      channel === "tool:follow-up:continue"
        ? list.map((entry) =>
            entry.id === 1 ? followUp(1, "现在复查", "2026-08-20T10:00:00+08:00") : entry,
          )
        : list,
    );
    const { app, root } = await mountPanel();
    await switchToDueGroup(root);
    await selectDueItem(root);
    await submitLifecycleDialog(root, "继续关注", "对方已推进");

    await vi.waitFor(() => {
      const faded = root.querySelector<HTMLButtonElement>(".follow-up-card.processed");
      expect(faded?.textContent).toContain("现在复查");
    });
    expect(sectionButton(root, "待复查")?.classList.contains("active")).toBe(true);
    expect(root.querySelector(".follow-up-card.processed .processed-tag")?.textContent).toBe(
      "已复查",
    );
    expect(root.querySelector(".group-heading")?.textContent).toContain("0");
    app.unmount();
  });

  it("marks a confirmed result as ended without leaving the due group", async () => {
    mockUpdatingList((channel, list) =>
      channel === "tool:follow-up:confirm-completed"
        ? list.map((entry) =>
            entry.id === 1 ? followUp(1, "现在复查", null, "2026-08-18T10:30:00+08:00") : entry,
          )
        : list,
    );
    const { app, root } = await mountPanel();
    await switchToDueGroup(root);
    await selectDueItem(root);
    await clickDetailDropdownItem(root, "确认结果", "确认完成");
    const dialog = document.body.querySelector<HTMLElement>(".el-dialog");
    const textarea = dialog?.querySelector<HTMLTextAreaElement>("textarea");
    if (!dialog || !textarea) throw new Error("result dialog did not open");
    textarea.value = "验收通过";
    textarea.dispatchEvent(new Event("input", { bubbles: true }));
    buttonByText(dialog, "确认")?.click();
    await nextTick();

    await vi.waitFor(() =>
      expect(root.querySelector(".follow-up-card.processed .processed-tag")?.textContent).toBe(
        "已结束",
      ),
    );
    expect(sectionButton(root, "待复查")?.classList.contains("active")).toBe(true);
    expect(root.querySelector(".follow-up-scroll")?.textContent).toContain("现在复查");
    app.unmount();
  });

  it("restores the real grouping after switching away and back", async () => {
    mockUpdatingList((channel, list) =>
      channel === "tool:follow-up:continue"
        ? list.map((entry) =>
            entry.id === 1 ? followUp(1, "现在复查", "2026-08-20T10:00:00+08:00") : entry,
          )
        : list,
    );
    const { app, root } = await mountPanel();
    await switchToDueGroup(root);
    await selectDueItem(root);
    await submitLifecycleDialog(root, "继续关注", "对方已推进");
    await vi.waitFor(() =>
      expect(root.querySelector(".follow-up-card.processed")).not.toBeNull(),
    );

    sectionButton(root, "全部")?.click();
    await nextTick();
    await switchToDueGroup(root);

    expect(root.querySelector(".follow-up-scroll")?.textContent).not.toContain("现在复查");
    expect(root.querySelector(".follow-up-scroll")?.textContent).toContain("暂无关注事项");
    app.unmount();
  });

  it("clears processed marks when a background refresh reloads items", async () => {
    mockUpdatingList((channel, list) =>
      channel === "tool:follow-up:continue"
        ? list.map((entry) =>
            entry.id === 1 ? followUp(1, "现在复查", "2026-08-20T10:00:00+08:00") : entry,
          )
        : list,
    );
    const { app, root } = await mountPanel();
    await switchToDueGroup(root);
    await selectDueItem(root);
    await submitLifecycleDialog(root, "继续关注", "对方已推进");
    await vi.waitFor(() =>
      expect(root.querySelector(".follow-up-card.processed")).not.toBeNull(),
    );

    const listener = eventBridge.listen.mock.calls.find(
      ([name]) => name === APP_EVENTS.FOLLOW_UP_REVIEW_DUE,
    )?.[1] as () => void;
    listener?.();
    await vi.waitFor(() =>
      expect(root.querySelector(".follow-up-card.processed")).toBeNull(),
    );
    app.unmount();
  });

  it("fades an edited item that leaves the current group", async () => {
    mockUpdatingList((channel, list) =>
      channel === "tool:follow-up:item-update"
        ? list.map((entry) =>
            entry.id === 1 ? followUp(1, "现在复查", "2026-08-25T09:00:00+08:00") : entry,
          )
        : list,
    );
    const { app, root } = await mountPanel();
    await switchToDueGroup(root);
    await selectDueItem(root);
    await clickDetailDropdownItem(root, "更多操作", "编辑");
    const dialog = document.body.querySelector<HTMLElement>(".el-dialog");
    if (!dialog) throw new Error("edit dialog did not open");
    buttonByText(dialog, "1 周后")?.click();
    await nextTick();
    buttonByText(dialog, "保存")?.click();
    await nextTick();

    await vi.waitFor(() =>
      expect(root.querySelector(".follow-up-card.processed .processed-tag")?.textContent).toBe(
        "已更新",
      ),
    );
    expect(sectionButton(root, "待复查")?.classList.contains("active")).toBe(true);
    expect(root.querySelector(".group-heading")?.textContent).toContain("0");
    app.unmount();
  });
});

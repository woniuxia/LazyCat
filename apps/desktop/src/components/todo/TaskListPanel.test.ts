// @vitest-environment happy-dom
import { createApp, nextTick } from "vue";
import ElementPlus from "element-plus";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useNavigationHandoff } from "../../composables/useNavigationHandoff";
import { useTodoNavigation } from "../../composables/useTodoNavigation";

const child = vi.hoisted(() => ({ focus: vi.fn() }));

vi.mock("./TodoPanel.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "TodoPanelStub",
      setup(_, { slots }) {
        return () => h("div", { class: "todo-stub" }, [slots["view-switch"]?.(), "Todo view"]);
      },
    }),
  };
});

vi.mock("../follow-up/FollowUpPanel.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "FollowUpPanelStub",
      emits: ["createTodo", "dueCountChange"],
      setup(_, { emit, expose, slots }) {
        expose({ focus: child.focus });
        return () =>
          h("div", { class: "follow-up-stub" }, [
            slots["view-switch"]?.(),
            h(
              "button",
              {
                class: "create-todo-stub",
                onClick: () =>
                  emit("createTodo", {
                    id: 7,
                    title: "确认接口交付",
                    description: "等待联调",
                    expectedOutcome: "验收通过",
                    latestProgress: null,
                    links: [],
                  }),
              },
              "Follow-up view",
            ),
            h(
              "button",
              { class: "due-count-stub", onClick: () => emit("dueCountChange", 120) },
              "Update due count",
            ),
          ]);
      },
    }),
  };
});

import TaskListPanel from "./TaskListPanel.vue";

async function mountPanel(waitForFocus = true) {
  const root = document.createElement("div");
  document.body.append(root);
  const app = createApp(TaskListPanel);
  app.use(ElementPlus);
  app.mount(root);
  await nextTick();
  if (waitForFocus) await vi.waitFor(() => expect(child.focus).toHaveBeenCalled());
  return { app, root };
}

describe("TaskListPanel", () => {
  beforeEach(() => {
    child.focus.mockReset();
    useNavigationHandoff().reset();
    useTodoNavigation().consumeFollowUpFocus();
  });

  it("switches views from the sidebar control and caps the due badge", async () => {
    const { app, root } = await mountPanel(false);

    expect(root.querySelector(".task-view-switch")?.textContent).toContain("我的任务");
    expect(root.querySelector(".task-view-switch")?.textContent).toContain("关注事项");
    const followUpButton = Array.from(
      root.querySelectorAll<HTMLButtonElement>(".task-view-switch button"),
    ).find((button) => button.textContent?.includes("关注事项"));
    followUpButton?.click();
    await nextTick();
    expect(root.querySelector<HTMLElement>(".todo-stub")?.style.display).toBe("none");
    expect(root.querySelector<HTMLElement>(".follow-up-stub")?.style.display).not.toBe("none");

    root.querySelector<HTMLButtonElement>(".due-count-stub")?.click();
    await nextTick();
    expect(root.querySelector(".follow-up-stub .due-count")?.textContent?.trim()).toBe("99+");
    app.unmount();
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("opens the follow-up view and focuses notification navigation", async () => {
    useTodoNavigation().requestFollowUp(7, false);
    const { app, root } = await mountPanel();

    expect(child.focus).toHaveBeenCalledWith(7, false);
    expect(root.querySelector<HTMLElement>(".todo-stub")?.style.display).toBe("none");
    expect(root.querySelector<HTMLElement>(".follow-up-stub")?.style.display).not.toBe("none");
    app.unmount();
  });

  it("prefills a transient Todo draft and returns to the Todo view", async () => {
    useTodoNavigation().requestFollowUp(7, true);
    const { app, root } = await mountPanel();
    root.querySelector<HTMLButtonElement>(".create-todo-stub")?.click();
    await nextTick();

    expect(useNavigationHandoff().consumePendingToolInput("todo")).toMatchObject({
      toolId: "todo",
      text: "确认接口交付",
      todoDraft: {
        title: "确认接口交付",
        description: "预期结果：验收通过\n\n等待联调",
      },
    });
    expect(root.querySelector<HTMLElement>(".todo-stub")?.style.display).not.toBe("none");
    app.unmount();
  });
});

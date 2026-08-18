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
      setup: () => () => h("div", { class: "todo-stub" }, "Todo view"),
    }),
  };
});

vi.mock("../follow-up/FollowUpPanel.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "FollowUpPanelStub",
      emits: ["createTodo"],
      setup(_, { emit, expose }) {
        expose({ focus: child.focus });
        return () =>
          h(
            "button",
            {
              class: "follow-up-stub",
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
          );
      },
    }),
  };
});

import TaskListPanel from "./TaskListPanel.vue";

async function mountPanel() {
  const root = document.createElement("div");
  document.body.append(root);
  const app = createApp(TaskListPanel);
  app.use(ElementPlus);
  app.mount(root);
  await nextTick();
  await vi.waitFor(() => expect(child.focus).toHaveBeenCalled());
  return { app, root };
}

describe("TaskListPanel", () => {
  beforeEach(() => {
    child.focus.mockReset();
    useNavigationHandoff().reset();
    useTodoNavigation().consumeFollowUpFocus();
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
    root.querySelector<HTMLButtonElement>(".follow-up-stub")?.click();
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

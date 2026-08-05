// @vitest-environment happy-dom
import { createApp, defineComponent, h, nextTick, type App } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import HttpStatusPanel from "./HttpStatusPanel.vue";

const harness = vi.hoisted(() => ({
  invoke: vi.fn(),
  error: vi.fn(),
}));

vi.mock("../bridge/tauri", () => ({
  invokeToolByChannel: (...args: unknown[]) => harness.invoke(...args),
}));

vi.mock("element-plus", () => ({
  ElMessage: { error: (...args: unknown[]) => harness.error(...args) },
}));

vi.mock("./HttpStatusTable.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "HttpStatusTable",
      props: {
        data: { type: Array, required: true },
        expandedCodes: { type: Array, required: true },
      },
      emits: ["expand-change"],
      setup(props, { emit }) {
        return () =>
          h(
            "div",
            { class: "http-status-table" },
            (props.data as Array<{ code: number }>).map((row) => {
              const expanded = (props.expandedCodes as number[]).includes(row.code);
              const expandedRows = expanded
                ? (props.data as Array<{ code: number }>).filter((item) => item.code !== row.code)
                : [
                    ...(props.data as Array<{ code: number }>).filter(
                      (item) => item.code !== row.code,
                    ),
                    row,
                  ];
              return h(
                "button",
                {
                  type: "button",
                  "data-code": String(row.code),
                  "data-expanded": String(expanded),
                  onClick: () => emit("expand-change", row, expandedRows),
                },
                String(row.code),
              );
            }),
          );
      },
    }),
  };
});

const InputStub = defineComponent({
  props: { modelValue: { type: String, default: "" } },
  emits: ["update:modelValue"],
  setup(props, { emit }) {
    return () =>
      h("input", {
        value: props.modelValue,
        onInput: (event: Event) =>
          emit("update:modelValue", (event.target as HTMLInputElement).value),
      });
  },
});

function status(code: number, overrides: Record<string, unknown> = {}) {
  return {
    code,
    name: code === 404 ? "Not Found" : "Too Many Requests",
    desc: code === 404 ? "未找到" : "请求过多",
    usage: "测试场景",
    causes: "测试原因",
    explanation: "测试解释",
    troubleshooting: "测试排查",
    responseHeaders: [],
    ...overrides,
  };
}

const groups = [
  {
    category: "4xx",
    name: "客户端错误",
    codes: [status(404), status(429)],
  },
];

const mountedApps: App[] = [];

function mountPanel(): HTMLElement {
  const root = document.createElement("div");
  document.body.appendChild(root);
  const app = createApp(HttpStatusPanel);
  app.component("ElInput", InputStub);
  app.mount(root);
  mountedApps.push(app);
  return root;
}

async function settle(): Promise<void> {
  await nextTick();
  await Promise.resolve();
  await nextTick();
}

beforeEach(() => {
  vi.useFakeTimers();
  harness.invoke.mockReset();
  harness.error.mockReset();
  harness.invoke.mockImplementation(async (channel: string) => {
    if (channel === "tool:network:http-status-list") return { groups };
    return { results: [], classificationHint: null };
  });
});

afterEach(() => {
  for (const app of mountedApps.splice(0)) app.unmount();
  document.body.innerHTML = "";
  vi.useRealTimers();
});

describe("HttpStatusPanel", () => {
  it("keeps multiple status-code details expanded", async () => {
    const root = mountPanel();
    await settle();

    const buttons = () => Array.from(root.querySelectorAll<HTMLButtonElement>("[data-code]"));
    buttons()[0].click();
    await nextTick();
    buttons()[1].click();
    await nextTick();

    expect(buttons().map((button) => button.dataset.expanded)).toEqual(["true", "true"]);
    buttons()[0].click();
    await nextTick();
    expect(buttons().map((button) => button.dataset.expanded)).toEqual(["false", "true"]);
  });

  it("shows an unknown-code classification hint without adding a result row", async () => {
    harness.invoke.mockImplementation(async (channel: string, payload: { query?: string }) => {
      if (channel === "tool:network:http-status-list") return { groups };
      if (payload.query === "599") {
        return {
          results: [],
          classificationHint: {
            code: 599,
            category: "5xx",
            name: "服务器错误",
            message:
              "599 属于 5xx 服务器错误范围，但该具体状态码未在标准条目中收录，具体含义未定义",
          },
        };
      }
      return { results: [], classificationHint: null };
    });

    const root = mountPanel();
    await settle();
    const input = root.querySelector("input") as HTMLInputElement;
    input.value = "599";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
    await vi.advanceTimersByTimeAsync(300);
    await settle();

    expect(root.textContent).toContain("具体含义未定义");
    expect(root.querySelectorAll("[data-code]")).toHaveLength(0);
  });

  it("shows a normal no-match state for an empty lookup", async () => {
    const root = mountPanel();
    await settle();
    const input = root.querySelector("input") as HTMLInputElement;
    input.value = "not-a-status";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
    await vi.advanceTimersByTimeAsync(300);
    await settle();

    expect(root.textContent).toContain("未找到匹配的状态码");
    expect(root.querySelectorAll("[data-code]")).toHaveLength(0);
  });

  it("retains expansion by code while switching search and grouped views", async () => {
    harness.invoke.mockImplementation(async (channel: string, payload: { query?: string }) => {
      if (channel === "tool:network:http-status-list") return { groups };
      if (payload.query === "404") {
        return { results: [status(404)], classificationHint: null };
      }
      return { results: [], classificationHint: null };
    });

    const root = mountPanel();
    await settle();
    const buttons = () => Array.from(root.querySelectorAll<HTMLButtonElement>("[data-code]"));
    buttons()[0].click();
    await nextTick();

    const input = root.querySelector("input") as HTMLInputElement;
    input.value = "404";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
    await vi.advanceTimersByTimeAsync(300);
    await settle();

    expect(buttons()).toHaveLength(1);
    expect(buttons()[0].dataset.code).toBe("404");
    expect(buttons()[0].dataset.expanded).toBe("true");

    input.value = "";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
    expect(buttons().map((button) => button.dataset.expanded)).toEqual(["true", "false"]);
  });

  it("keeps an initial list failure visible", async () => {
    harness.invoke.mockImplementation(async (channel: string) => {
      if (channel === "tool:network:http-status-list") throw new Error("列表不可用");
      return { results: [], classificationHint: null };
    });

    const root = mountPanel();
    await settle();

    expect(root.textContent).toContain("状态码列表加载失败：列表不可用");
    expect(harness.error).toHaveBeenCalledWith("列表不可用");
  });

  it("keeps a lookup failure distinct from an empty result", async () => {
    harness.invoke.mockImplementation(async (channel: string) => {
      if (channel === "tool:network:http-status-list") return { groups };
      throw new Error("搜索服务不可用");
    });

    const root = mountPanel();
    await settle();
    const input = root.querySelector("input") as HTMLInputElement;
    input.value = "404";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
    await vi.advanceTimersByTimeAsync(300);
    await settle();

    expect(root.textContent).toContain("状态码搜索失败：搜索服务不可用");
    expect(root.textContent).not.toContain("未找到匹配的状态码");
    expect(harness.error).toHaveBeenCalledWith("搜索服务不可用");
  });
});

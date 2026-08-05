// @vitest-environment happy-dom
import { createApp, defineComponent, h, nextTick, type App } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import JsonArrayFilterPanel from "./JsonArrayFilterPanel.vue";

const harness = vi.hoisted(() => ({
  success: vi.fn(),
  warning: vi.fn(),
  error: vi.fn(),
}));

vi.mock("element-plus", () => ({
  ElMessage: {
    success: (...args: unknown[]) => harness.success(...args),
    warning: (...args: unknown[]) => harness.warning(...args),
    error: (...args: unknown[]) => harness.error(...args),
  },
}));

const InputStub = defineComponent({
  inheritAttrs: false,
  props: {
    modelValue: { type: String, default: "" },
    readonly: { type: Boolean, default: false },
  },
  emits: ["update:modelValue"],
  setup(props, { attrs, emit }) {
    return () =>
      h("textarea", {
        ...attrs,
        value: props.modelValue,
        readOnly: props.readonly,
        onInput: (event: Event) =>
          emit("update:modelValue", (event.target as HTMLTextAreaElement).value),
      });
  },
});

const ButtonStub = defineComponent({
  inheritAttrs: false,
  props: { disabled: { type: Boolean, default: false } },
  emits: ["click"],
  setup(props, { attrs, emit, slots }) {
    return () =>
      h(
        "button",
        {
          ...attrs,
          disabled: props.disabled,
          onClick: () => emit("click"),
        },
        slots.default?.(),
      );
  },
});

const CheckboxStub = defineComponent({
  inheritAttrs: false,
  props: {
    modelValue: { type: Boolean, default: false },
    label: { type: String, required: true },
  },
  emits: ["change"],
  setup(props, { attrs, emit, slots }) {
    return () =>
      h("label", { ...attrs }, [
        h("input", {
          type: "checkbox",
          checked: props.modelValue,
          "data-property": props.label,
          onChange: (event: Event) => emit("change", (event.target as HTMLInputElement).checked),
        }),
        slots.default?.(),
      ]);
  },
});

const EmptyStub = defineComponent({
  props: { description: { type: String, default: "" } },
  setup(props) {
    return () => h("div", { class: "empty-state" }, props.description);
  },
});

const mountedApps: Array<{ app: App; root: HTMLElement }> = [];
let clipboardWrite: ReturnType<typeof vi.fn>;

function mountPanel(): HTMLElement {
  const root = document.createElement("div");
  document.body.appendChild(root);
  const app = createApp(JsonArrayFilterPanel);
  app.component("ElInput", InputStub);
  app.component("ElButton", ButtonStub);
  app.component("ElCheckbox", CheckboxStub);
  app.component("ElEmpty", EmptyStub);
  app.mount(root);
  mountedApps.push({ app, root });
  return root;
}

async function settle() {
  await nextTick();
  await Promise.resolve();
  await nextTick();
}

async function setInput(root: HTMLElement, value: string) {
  const input = root.querySelectorAll<HTMLTextAreaElement>("textarea")[0];
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  await nextTick();
}

async function parseInput(root: HTMLElement, value: string) {
  await setInput(root, value);
  await vi.advanceTimersByTimeAsync(300);
  await settle();
}

beforeEach(() => {
  vi.useFakeTimers();
  harness.success.mockReset();
  harness.warning.mockReset();
  harness.error.mockReset();
  clipboardWrite = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: clipboardWrite },
  });
});

afterEach(() => {
  for (const { app, root } of mountedApps.splice(0)) {
    root.querySelector<HTMLButtonElement>('[data-action="clear-input"]')?.click();
    app.unmount();
  }
  document.body.innerHTML = "";
  vi.useRealTimers();
});

describe("JsonArrayFilterPanel", () => {
  it("debounces parsing and displays the first usable array path with all fields selected", async () => {
    const root = mountPanel();
    await setInput(root, '{"meta":true,"records":[{"id":1,"name":"Ada"}]}');

    expect(root.textContent).not.toContain("$.records");
    await vi.advanceTimersByTimeAsync(299);
    expect(root.textContent).not.toContain("$.records");

    await vi.advanceTimersByTimeAsync(1);
    await settle();

    expect(root.textContent).toContain("$.records");
    expect(root.textContent).toContain("已选 2 / 2 个字段");
    expect(root.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')).toHaveLength(2);
    expect(root.querySelectorAll<HTMLTextAreaElement>("textarea")[1].value).toContain('"id": 1');
  });

  it("clears stale results while editing and shows a visible parse error", async () => {
    const root = mountPanel();
    await parseInput(root, '[{"id":1}]');
    const meta = root.querySelector(".filter-meta");

    await setInput(root, "{invalid");
    expect(root.querySelectorAll<HTMLTextAreaElement>("textarea")[1].value).toBe("");
    expect(root.textContent).not.toContain("$\n");
    expect(root.querySelector(".filter-meta")).toBe(meta);
    expect(root.querySelector(".parse-alert")).toBeNull();

    await vi.advanceTimersByTimeAsync(300);
    await settle();

    expect(root.querySelector('[role="alert"]')?.textContent).toContain("JSON 解析失败");
    expect(root.querySelector(".parse-error")?.getAttribute("title")).toContain("JSON 解析失败");
    expect(root.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')).toHaveLength(0);
  });

  it("updates the root-array projection when a property is unchecked", async () => {
    const root = mountPanel();
    await parseInput(root, '[{"id":1,"name":"Ada","nested":{"ok":true}},{"id":2}]');

    const idCheckbox = root.querySelector<HTMLInputElement>('input[data-property="id"]');
    expect(idCheckbox?.checked).toBe(true);
    idCheckbox?.click();
    await settle();

    const output = root.querySelectorAll<HTMLTextAreaElement>("textarea")[1].value;
    expect(output).not.toContain('"id"');
    expect(output).toContain('"name": "Ada"');
    expect(output).toContain('"nested": {');
    expect(JSON.parse(output)).toEqual([{ name: "Ada", nested: { ok: true } }, {}]);
    expect(root.textContent).toContain("已选 2 / 3 个字段");
  });

  it("supports clearing and restoring all output fields", async () => {
    const root = mountPanel();
    await parseInput(root, '[{"id":1,"name":"Ada","active":true}]');

    root.querySelector<HTMLButtonElement>('[data-action="clear-properties"]')?.click();
    await settle();
    expect(root.querySelectorAll<HTMLTextAreaElement>("textarea")[1].value).toBe("[\n  {}\n]");
    expect(root.textContent).toContain("已选 0 / 3 个字段");

    root.querySelector<HTMLButtonElement>('[data-action="toggle-all-properties"]')?.click();
    await settle();
    expect(root.querySelectorAll<HTMLTextAreaElement>("textarea")[1].value).toContain('"id": 1');
    expect(root.textContent).toContain("已选 3 / 3 个字段");
  });

  it("filters long property lists without changing the selection set", async () => {
    const root = mountPanel();
    await parseInput(
      root,
      '[{"id":1,"name":"Ada","email":"a@example.com","active":true,"createdAt":"today","role":"admin"}]',
    );

    const search = root.querySelector<HTMLTextAreaElement>('textarea[placeholder="搜索字段名"]');
    expect(search).toBeTruthy();
    search!.value = "email";
    search!.dispatchEvent(new Event("input", { bubbles: true }));
    await settle();

    expect(root.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')).toHaveLength(1);
    expect(root.textContent).toContain("匹配 1 个");
    expect(root.querySelectorAll<HTMLTextAreaElement>("textarea")[1].value).toContain('"name"');
  });

  it("shows an explicit empty state when the field search has no match", async () => {
    const root = mountPanel();
    await parseInput(
      root,
      '[{"id":1,"name":"Ada","email":"a@example.com","active":true,"createdAt":"today","role":"admin"}]',
    );

    const search = root.querySelector<HTMLTextAreaElement>('textarea[placeholder="搜索字段名"]');
    search!.value = "missing";
    search!.dispatchEvent(new Event("input", { bubbles: true }));
    await settle();

    expect(root.textContent).toContain("没有匹配字段");
    expect(root.textContent).toContain("匹配 0 个");
  });

  it("distinguishes a valid document without an object array", async () => {
    const root = mountPanel();
    await parseInput(root, '{"values":[1,null,"text"]}');

    expect(root.textContent).toContain("未找到可过滤的对象数组");
    expect(root.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')).toHaveLength(0);
    expect(root.querySelectorAll<HTMLTextAreaElement>("textarea")[1].value).toBe("");
  });

  it("keeps an empty object array valid and shows its zero-property count", async () => {
    const root = mountPanel();
    await parseInput(root, '{"records":[]}');

    expect(root.textContent).toContain("$.records");
    expect(root.textContent).toContain("已选 0 / 0 个字段");
    expect(root.querySelectorAll<HTMLTextAreaElement>("textarea")[1].value).toBe("[]");
  });

  it("copies the result and exposes clipboard failures", async () => {
    const root = mountPanel();
    await parseInput(root, '[{"id":1}]');

    root.querySelector<HTMLButtonElement>('[data-action="copy-result"]')?.click();
    await settle();
    expect(clipboardWrite).toHaveBeenCalledWith(expect.stringContaining('"id": 1'));
    expect(harness.success).toHaveBeenCalledWith("数组过滤结果已复制");

    clipboardWrite.mockRejectedValueOnce(new Error("权限不足"));
    root.querySelector<HTMLButtonElement>('[data-action="copy-result"]')?.click();
    await settle();
    expect(harness.error).toHaveBeenCalledWith("复制数组过滤结果失败：权限不足");
  });

  it("clears input and retains runtime state across remounts", async () => {
    const firstRoot = mountPanel();
    await parseInput(firstRoot, '[{"id":1,"name":"Ada"}]');
    const firstApp = mountedApps.shift()!;
    firstApp.app.unmount();

    const secondRoot = mountPanel();
    expect(secondRoot.querySelectorAll<HTMLTextAreaElement>("textarea")[0].value).toBe(
      '[{"id":1,"name":"Ada"}]',
    );
    expect(secondRoot.querySelectorAll<HTMLTextAreaElement>("textarea")[1].value).toContain(
      '"name": "Ada"',
    );

    secondRoot.querySelector<HTMLButtonElement>('[data-action="clear-input"]')?.click();
    await settle();
    expect(secondRoot.querySelectorAll<HTMLTextAreaElement>("textarea")[0].value).toBe("");
    expect(secondRoot.querySelectorAll<HTMLTextAreaElement>("textarea")[1].value).toBe("");
    expect(secondRoot.textContent).toContain("输入 JSON 文档后自动解析");
  });
});

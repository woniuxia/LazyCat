// @vitest-environment happy-dom
import { createApp, defineComponent, h, nextTick, type App } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import EscapeUnescapePanel from "./EscapeUnescapePanel.vue";

const harness = vi.hoisted(() => ({
  error: vi.fn(),
  success: vi.fn(),
}));

vi.mock("element-plus", () => ({
  ElMessage: {
    error: (...args: unknown[]) => harness.error(...args),
    success: (...args: unknown[]) => harness.success(...args),
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
  emits: ["click"],
  setup(_, { attrs, emit, slots }) {
    return () => h("button", { ...attrs, onClick: () => emit("click") }, slots.default?.());
  },
});

const PassthroughStub = defineComponent({
  setup(_, { slots }) {
    return () => h("div", slots.default?.());
  },
});

const mountedApps: Array<{ app: App; root: HTMLElement }> = [];

function mountPanel(): HTMLElement {
  const root = document.createElement("div");
  document.body.appendChild(root);
  const app = createApp(EscapeUnescapePanel);
  app.component("ElInput", InputStub);
  app.component("ElButton", ButtonStub);
  app.component("ElRadioGroup", PassthroughStub);
  app.component("ElRadioButton", PassthroughStub);
  app.component("ElSpace", PassthroughStub);
  app.mount(root);
  mountedApps.push({ app, root });
  return root;
}

async function setInput(root: HTMLElement, value: string) {
  const input = root.querySelector<HTMLTextAreaElement>("textarea");
  if (!input) throw new Error("input textarea not found");
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  await nextTick();
}

function buttonByLabel(root: HTMLElement, label: string): HTMLButtonElement {
  const button = Array.from(root.querySelectorAll<HTMLButtonElement>("button")).find(
    (candidate) => candidate.textContent === label,
  );
  if (!button) throw new Error(`button not found: ${label}`);
  return button;
}

beforeEach(() => {
  harness.error.mockReset();
  harness.success.mockReset();
});

afterEach(() => {
  for (const { app } of mountedApps.splice(0)) app.unmount();
  document.body.innerHTML = "";
});

describe("EscapeUnescapePanel", () => {
  it("can unescape the JSON text produced by escape", async () => {
    const root = mountPanel();
    const source = '{\n  "id": 1,\n  "name": "lazycat"\n}';
    const escaped = '{\\n  \\"id\\": 1,\\n  \\"name\\": \\"lazycat\\"\\n}';

    await setInput(root, source);
    buttonByLabel(root, "转义").click();
    await nextTick();

    const textareas = root.querySelectorAll<HTMLTextAreaElement>("textarea");
    expect(textareas[1]?.value).toBe(escaped);

    buttonByLabel(root, "互换").click();
    await nextTick();
    buttonByLabel(root, "反转义").click();
    await nextTick();

    expect(textareas[1]?.value).toBe(source);
    expect(harness.error).not.toHaveBeenCalled();
  });
});

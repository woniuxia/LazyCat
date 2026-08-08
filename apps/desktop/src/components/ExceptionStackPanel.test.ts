// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { createRenderer, defineComponent, h, nextTick } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ElMessage } from "element-plus";

const dialogHarness = vi.hoisted(() => ({
  open: vi.fn(),
  save: vi.fn(),
}));
const bridgeHarness = vi.hoisted(() => ({
  invokeToolByChannel: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => dialogHarness);
vi.mock("../bridge/tauri", () => bridgeHarness);

interface HostNode {
  type: string;
  text: string;
  props: Record<string, unknown>;
  children: HostNode[];
  parent: HostNode | null;
}

function hostNode(type: string, text = ""): HostNode {
  return { type, text, props: {}, children: [], parent: null };
}

function createPanelRenderer() {
  return createRenderer<HostNode, HostNode>({
    patchProp(node, key, _previous, value) {
      node.props[key] = value;
    },
    insert(node, parent, anchor) {
      node.parent = parent;
      const index = anchor ? parent.children.indexOf(anchor) : -1;
      if (index >= 0) parent.children.splice(index, 0, node);
      else parent.children.push(node);
    },
    remove(node) {
      const index = node.parent?.children.indexOf(node) ?? -1;
      if (index >= 0) node.parent?.children.splice(index, 1);
      node.parent = null;
    },
    createElement: (type) => hostNode(type),
    createText: (text) => hostNode("#text", text),
    createComment: (text) => hostNode("#comment", text),
    setText(node, text) {
      node.text = text;
    },
    setElementText(node, text) {
      node.text = text;
      node.children = [];
    },
    parentNode: (node) => node.parent,
    nextSibling(node) {
      const siblings = node.parent?.children ?? [];
      const index = siblings.indexOf(node);
      return index >= 0 ? (siblings[index + 1] ?? null) : null;
    },
    setScopeId() {},
    insertStaticContent(content, parent, anchor) {
      const node = hostNode("#static", content);
      node.parent = parent;
      const index = anchor ? parent.children.indexOf(anchor) : -1;
      if (index >= 0) parent.children.splice(index, 0, node);
      else parent.children.push(node);
      return [node, node];
    },
  });
}

function registerElementStubs(
  app: ReturnType<ReturnType<typeof createPanelRenderer>["createApp"]>,
) {
  const generic = defineComponent({
    inheritAttrs: false,
    setup(_props, { attrs, slots }) {
      return () =>
        h(
          "div",
          attrs,
          Object.values(slots).flatMap((slot) => slot?.() ?? []),
        );
    },
  });
  for (const name of ["el-select", "el-option", "el-input", "el-icon"]) {
    app.component(name, generic);
  }
  app.component(
    "el-button",
    defineComponent({
      inheritAttrs: false,
      setup(_props, { attrs, slots }) {
        return () => h("button", attrs, slots.default?.());
      },
    }),
  );
}

function nodeText(node: HostNode): string {
  return `${node.text}${node.children.map(nodeText).join("")}`;
}

function findNode(root: HostNode, predicate: (node: HostNode) => boolean): HostNode | null {
  let result: HostNode | null = null;
  const visit = (node: HostNode) => {
    if (!result && predicate(node)) result = node;
    node.children.forEach(visit);
  };
  visit(root);
  return result;
}

function findButton(root: HostNode, text: string): HostNode | null {
  return findNode(root, (node) => node.type === "button" && nodeText(node).includes(text));
}

function findByAriaLabel(root: HostNode, label: string): HostNode | null {
  return findNode(root, (node) => node.props["aria-label"] === label);
}

function findByModelValue(root: HostNode, value: unknown): HostNode | null {
  return findNode(root, (node) => node.props.modelValue === value);
}

function modelValue(node: HostNode | null): unknown {
  return node?.props.modelValue ?? node?.props["model-value"];
}

async function flushPanel(): Promise<void> {
  for (let index = 0; index < 6; index += 1) {
    await Promise.resolve();
    await nextTick();
  }
}

async function mountPanel() {
  const { default: ExceptionStackPanel } = await import("./ExceptionStackPanel.vue");
  const renderer = createPanelRenderer();
  const root = hostNode("root");
  const app = renderer.createApp(ExceptionStackPanel);
  registerElementStubs(app);
  app.mount(root);
  await flushPanel();
  return { app, root };
}

const nodeStack = [
  "TypeError: Cannot read properties of undefined",
  "    at loadUser (C:\\work\\app.ts:12:7)",
].join("\n");

const source = readFileSync(
  resolve(process.cwd(), "src/components/ExceptionStackPanel.vue"),
  "utf8",
);

beforeEach(() => {
  dialogHarness.open.mockReset();
  dialogHarness.save.mockReset();
  bridgeHarness.invokeToolByChannel.mockReset();
  bridgeHarness.invokeToolByChannel.mockResolvedValue({});
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("ExceptionStackPanel source structure", () => {
  it("keeps parsing explicit and uses the existing local file channels", () => {
    expect(source).toContain("parseExceptionStack");
    expect(source).toContain('@keydown.ctrl.enter.prevent="parseStack"');
    expect(source).toContain('"tool:file:read-text"');
    expect(source).toContain('"tool:file:write-text"');
    expect(source).toContain("navigator.clipboard.writeText(result.value.summary)");
    expect(source).not.toContain("useSettings");
    expect(source).toContain("@container exception-stack (max-width: 760px)");
  });
});

describe("ExceptionStackPanel behavior", () => {
  it("does not parse while editing and parses on the explicit action", async () => {
    const { app, root } = await mountPanel();
    const input = findByAriaLabel(root, "原始异常堆栈");

    (input?.props["onUpdate:modelValue"] as (value: string) => void)(nodeStack);
    await flushPanel();
    expect(bridgeHarness.invokeToolByChannel).not.toHaveBeenCalled();
    expect(nodeText(root)).toContain("尚未生成解析结果");

    await (findButton(root, "解析")?.props.onClick as () => Promise<void> | void)();
    await flushPanel();
    expect(nodeText(root)).toContain("TypeError");
    expect(nodeText(root)).toContain("loadUser");
    app.unmount();
  });

  it("loads a file without parsing it and exposes parse failures without stale output", async () => {
    dialogHarness.open.mockResolvedValue("C:\\logs\\error.log");
    bridgeHarness.invokeToolByChannel.mockResolvedValue({
      content: nodeStack,
      path: "C:\\logs\\error.log",
    });
    const { app, root } = await mountPanel();

    await (findButton(root, "打开文件")?.props.onClick as () => Promise<void>)();
    await flushPanel();
    expect(bridgeHarness.invokeToolByChannel).toHaveBeenCalledWith("tool:file:read-text", {
      path: "C:\\logs\\error.log",
    });
    expect(modelValue(findByAriaLabel(root, "原始异常堆栈"))).toBe(nodeStack);
    expect(nodeText(root)).toContain("尚未生成解析结果");

    const input = findByAriaLabel(root, "原始异常堆栈");
    (input?.props["onUpdate:modelValue"] as (value: string) => void)("not a stack");
    await (findButton(root, "解析")?.props.onClick as () => Promise<void> | void)();
    await flushPanel();
    expect(nodeText(root)).toContain("无法识别堆栈格式");
    expect(nodeText(root)).not.toContain("loadUser");
    app.unmount();
  });

  it("surfaces file open failures and leaves cancellation quiet", async () => {
    const error = vi.spyOn(ElMessage, "error").mockReturnValue(undefined as never);
    const { app, root } = await mountPanel();

    dialogHarness.open.mockResolvedValueOnce(null);
    await (findButton(root, "打开文件")?.props.onClick as () => Promise<void>)();
    await flushPanel();
    expect(error).not.toHaveBeenCalled();
    expect(nodeText(root)).toContain("尚未生成解析结果");

    dialogHarness.open.mockRejectedValueOnce(new Error("文件读取被拒绝"));
    await (findButton(root, "打开文件")?.props.onClick as () => Promise<void>)();
    await flushPanel();
    expect(error).toHaveBeenCalledWith("打开文件失败：文件读取被拒绝");
    expect(nodeText(root)).toContain("打开文件失败：文件读取被拒绝");
    app.unmount();
  });

  it("surfaces copy failures and saves only to an explicit destination", async () => {
    const success = vi.spyOn(ElMessage, "success").mockReturnValue(undefined as never);
    const error = vi.spyOn(ElMessage, "error").mockReturnValue(undefined as never);
    const clipboard = navigator.clipboard.writeText as ReturnType<typeof vi.fn>;
    clipboard.mockRejectedValueOnce(new Error("剪贴板不可用"));
    dialogHarness.save.mockResolvedValue("C:\\out\\summary.txt");
    const { app, root } = await mountPanel();
    const input = findByAriaLabel(root, "原始异常堆栈");
    (input?.props["onUpdate:modelValue"] as (value: string) => void)(nodeStack);
    await (findButton(root, "解析")?.props.onClick as () => Promise<void> | void)();
    await flushPanel();

    await (findButton(root, "复制")?.props.onClick as () => Promise<void>)();
    expect(error).toHaveBeenCalledWith(expect.stringContaining("复制摘要失败"));

    await (findButton(root, "另存为")?.props.onClick as () => Promise<void>)();
    expect(bridgeHarness.invokeToolByChannel).toHaveBeenCalledWith(
      "tool:file:write-text",
      expect.objectContaining({
        path: "C:\\out\\summary.txt",
        content: expect.stringContaining("TypeError"),
      }),
    );
    expect(success).toHaveBeenCalledWith("已保存 summary.txt");
    app.unmount();
  });

  it("rejects source overwrite and surfaces save failures", async () => {
    const error = vi.spyOn(ElMessage, "error").mockReturnValue(undefined as never);
    dialogHarness.open.mockResolvedValue("C:\\logs\\error.log");
    bridgeHarness.invokeToolByChannel.mockResolvedValueOnce({
      content: nodeStack,
      path: "C:\\logs\\error.log",
    });
    const { app, root } = await mountPanel();

    await (findButton(root, "打开文件")?.props.onClick as () => Promise<void>)();
    await (findButton(root, "解析")?.props.onClick as () => Promise<void> | void)();
    await flushPanel();

    dialogHarness.save.mockResolvedValueOnce("c:/logs/error.log");
    await (findButton(root, "另存为")?.props.onClick as () => Promise<void>)();
    expect(error).toHaveBeenCalledWith(expect.stringContaining("目标路径不能覆盖原始堆栈文件"));
    expect(bridgeHarness.invokeToolByChannel).toHaveBeenCalledTimes(1);

    dialogHarness.save.mockResolvedValueOnce("C:\\out\\summary.txt");
    bridgeHarness.invokeToolByChannel.mockRejectedValueOnce(new Error("磁盘不可写"));
    await (findButton(root, "另存为")?.props.onClick as () => Promise<void>)();
    expect(error).toHaveBeenCalledWith("另存为失败：磁盘不可写");
    expect(modelValue(findByAriaLabel(root, "原始异常堆栈"))).toBe(nodeStack);
    app.unmount();
  });

  it("blocks delivery of a result after the input or format changes", async () => {
    const { app, root } = await mountPanel();
    const input = findByAriaLabel(root, "原始异常堆栈");
    (input?.props["onUpdate:modelValue"] as (value: string) => void)(nodeStack);
    await (findButton(root, "解析")?.props.onClick as () => Promise<void> | void)();
    await flushPanel();

    const formatSelect = findByModelValue(root, "auto");
    (formatSelect?.props["onUpdate:modelValue"] as (value: "auto" | "java" | "javascript") => void)(
      "java",
    );
    await flushPanel();
    expect(nodeText(root)).toContain("原文或格式已修改，等待重新解析");
    expect(findButton(root, "复制")?.props.disabled).toBe(true);
    expect(findButton(root, "另存为")?.props.disabled).toBe(true);

    (input?.props["onUpdate:modelValue"] as (value: string) => void)("changed");
    await flushPanel();
    expect(findButton(root, "复制")?.props.disabled).toBe(true);
    app.unmount();
  });

  it("clears the raw input and derived result together", async () => {
    const { app, root } = await mountPanel();
    const input = findByAriaLabel(root, "原始异常堆栈");
    (input?.props["onUpdate:modelValue"] as (value: string) => void)(nodeStack);
    await (findButton(root, "解析")?.props.onClick as () => Promise<void> | void)();
    await flushPanel();

    await (findButton(root, "清空")?.props.onClick as () => Promise<void>)();
    await flushPanel();
    expect(modelValue(findByAriaLabel(root, "原始异常堆栈"))).toBe("");
    expect(nodeText(root)).toContain("尚未生成解析结果");
    app.unmount();
  });
});

// @vitest-environment happy-dom
import { createRenderer, defineComponent, h, nextTick, ref } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ElMessageBox } from "element-plus";
import type { RequestForwardRule, RequestForwardRuntimeStatus } from "../types/request-forward";

const panelHarness = vi.hoisted(() => ({
  invoke: vi.fn(),
}));
const dialogHarness = vi.hoisted(() => ({
  open: vi.fn(),
  save: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => dialogHarness);
vi.mock("../composables/useSettings", () => ({
  getSetting: vi.fn(() => null),
  setSetting: vi.fn(),
}));
vi.mock("../composables/useToolInvoke", () => ({
  useToolInvoke: () => ({ loading: ref(false), invoke: panelHarness.invoke }),
}));

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
      return () => h("div", attrs, slots.default?.());
    },
  });
  for (const name of [
    "el-checkbox",
    "el-date-picker",
    "el-dialog",
    "el-dropdown",
    "el-dropdown-item",
    "el-dropdown-menu",
    "el-form",
    "el-form-item",
    "el-icon",
    "el-input",
    "el-input-number",
    "el-option",
    "el-popover",
    "el-select",
    "el-switch",
    "el-tag",
    "el-tooltip",
  ]) {
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
  app.directive("loading", () => undefined);
}

function nodeText(node: HostNode): string {
  return `${node.text}${node.children.map(nodeText).join("")}`;
}

function findButton(root: HostNode, text: string): HostNode | null {
  let result: HostNode | null = null;
  const visit = (node: HostNode) => {
    if (!result && node.type === "button" && nodeText(node).trim() === text) {
      result = node;
    }
    node.children.forEach(visit);
  };
  visit(root);
  return result;
}

function findButtonByAriaLabel(root: HostNode, label: string): HostNode | null {
  let result: HostNode | null = null;
  const visit = (node: HostNode) => {
    if (!result && node.type === "button" && node.props["aria-label"] === label) {
      result = node;
    }
    node.children.forEach(visit);
  };
  visit(root);
  return result;
}

async function flushPanel(): Promise<void> {
  for (let index = 0; index < 8; index += 1) {
    await Promise.resolve();
    await nextTick();
  }
}

const rule: RequestForwardRule = {
  id: 7,
  name: "API 转发",
  protocol: "http",
  bindHost: "127.0.0.1",
  listenPort: 8080,
  targetUrl: "http://127.0.0.1:3000",
  targetHost: null,
  targetPort: null,
  captureHttpHeaders: true,
  captureHttpBody: true,
  autoStart: false,
  createdAt: "2026-07-30 00:00:00",
  updatedAt: "2026-07-30 00:00:00",
};

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("RequestForwardPanel log capture behavior", () => {
  it("does not poll logs while paused and loads them after explicit capture enable", async () => {
    vi.useFakeTimers();
    vi.stubGlobal(
      "ResizeObserver",
      class {
        observe() {}
        disconnect() {}
      },
    );
    let captureEnabled = false;
    const status = (): RequestForwardRuntimeStatus => ({
      ruleId: rule.id,
      state: "running",
      lastError: null,
      lastObservabilityError: null,
      logCaptureEnabled: captureEnabled,
    });
    panelHarness.invoke.mockReset();
    panelHarness.invoke.mockImplementation(async (channel: string) => {
      if (channel === "tool:request-forward:list") return { items: [rule] };
      if (channel === "tool:request-forward:status") return { items: [status()] };
      if (channel === "tool:request-forward:stats-get") {
        return {
          item: {
            ruleId: rule.id,
            eventCount: 0,
            uploadBytes: 0,
            downloadBytes: 0,
            errorCount: 0,
            updatedAt: "2026-07-30 00:00:00",
          },
        };
      }
      if (channel === "tool:request-forward:log-list") {
        return { items: [], total: 0, latestId: null };
      }
      if (channel === "tool:request-forward:log-capture-update") {
        captureEnabled = true;
        return { item: status() };
      }
      throw new Error(`unexpected invoke: ${channel}`);
    });

    const { default: RequestForwardPanel } = await import("./RequestForwardPanel.vue");
    const renderer = createPanelRenderer();
    const root = hostNode("root");
    const app = renderer.createApp(RequestForwardPanel);
    registerElementStubs(app);
    app.mount(root);
    await flushPanel();

    const initialLogCalls = panelHarness.invoke.mock.calls.filter(
      ([channel]) => channel === "tool:request-forward:log-list",
    ).length;
    expect(initialLogCalls).toBe(1);

    await vi.advanceTimersByTimeAsync(2_000);
    await flushPanel();
    expect(
      panelHarness.invoke.mock.calls.filter(
        ([channel]) => channel === "tool:request-forward:log-list",
      ),
    ).toHaveLength(initialLogCalls);

    const enableButton = findButton(root, "实时采集");
    expect(enableButton?.props["aria-pressed"]).toBe(false);
    await (enableButton?.props.onClick as () => Promise<void>)();
    await flushPanel();

    expect(panelHarness.invoke).toHaveBeenCalledWith("tool:request-forward:log-capture-update", {
      id: rule.id,
      enabled: true,
    });
    expect(
      panelHarness.invoke.mock.calls.filter(
        ([channel]) => channel === "tool:request-forward:log-list",
      ),
    ).toHaveLength(initialLogCalls + 1);
    expect(findButton(root, "实时采集")?.props["aria-pressed"]).toBe(true);
    app.unmount();
  }, 10_000);

  it("imports a validated bundle atomically and selects the first imported rule", async () => {
    vi.stubGlobal(
      "ResizeObserver",
      class {
        observe() {}
        disconnect() {}
      },
    );
    const importedRule = { ...rule, id: 8, name: "导入规则" };
    let imported = false;
    dialogHarness.open.mockReset();
    dialogHarness.open.mockResolvedValue("E:\\tmp\\request-forward-rules.json");
    vi.spyOn(ElMessageBox, "confirm").mockResolvedValue("confirm" as never);
    panelHarness.invoke.mockReset();
    panelHarness.invoke.mockImplementation(async (channel: string, payload: unknown) => {
      if (channel === "tool:request-forward:list") {
        return { items: imported ? [rule, importedRule] : [rule] };
      }
      if (channel === "tool:request-forward:status") {
        const statusFor = (id: number): RequestForwardRuntimeStatus => ({
          ruleId: id,
          state: "stopped",
          lastError: null,
          lastObservabilityError: null,
          logCaptureEnabled: false,
        });
        return {
          items: (imported ? [rule, importedRule] : [rule]).map((item) => statusFor(item.id)),
        };
      }
      if (channel === "tool:request-forward:stats-get") {
        const id = (payload as { id: number }).id;
        return {
          item: {
            ruleId: id,
            eventCount: 0,
            uploadBytes: 0,
            downloadBytes: 0,
            errorCount: 0,
            updatedAt: "",
          },
        };
      }
      if (channel === "tool:request-forward:log-list") return { items: [], total: 0 };
      if (channel === "tool:file:read-text") {
        return {
          content: JSON.stringify({
            format: "lazycat.request-forward.rules",
            version: 1,
            exportedAt: "2026-07-31T08:00:00Z",
            rules: [
              {
                name: "导入规则",
                protocol: "http",
                bindHost: "127.0.0.1",
                listenPort: 8081,
                targetUrl: "http://127.0.0.1:3001",
                targetHost: null,
                targetPort: null,
                captureHttpHeaders: true,
                captureHttpBody: true,
              },
            ],
          }),
        };
      }
      if (channel === "tool:request-forward:bundle-import") {
        imported = true;
        return { imported: 1, items: [importedRule] };
      }
      throw new Error(`unexpected invoke: ${channel}`);
    });

    const { default: RequestForwardPanel } = await import("./RequestForwardPanel.vue");
    const renderer = createPanelRenderer();
    const root = hostNode("root");
    const app = renderer.createApp(RequestForwardPanel);
    registerElementStubs(app);
    app.mount(root);
    await flushPanel();

    const importButton = findButtonByAriaLabel(root, "导入规则包");
    await (importButton?.props.onClick as () => Promise<void>)();
    await flushPanel();

    expect(dialogHarness.open).toHaveBeenCalled();
    expect(panelHarness.invoke).toHaveBeenCalledWith("tool:file:read-text", {
      path: "E:\\tmp\\request-forward-rules.json",
    });
    expect(panelHarness.invoke).toHaveBeenCalledWith(
      "tool:request-forward:bundle-import",
      expect.objectContaining({ bundle: expect.objectContaining({ version: 1 }) }),
    );
    expect(nodeText(root)).toContain("导入规则");
    app.unmount();
  }, 10_000);
});

// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { createRenderer, defineComponent, h, nextTick } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ElMessage, ElMessageBox } from "element-plus";
import {
  BUILTIN_TEST_EMAIL_TEMPLATE_ID,
  DEFAULT_TEST_EMAIL_TEMPLATE,
} from "../utils/testEmailAssistant";

const settingsHarness = vi.hoisted(() => ({
  getSettingJson: vi.fn(),
  setSettingAndWait: vi.fn(),
}));
const dialogHarness = vi.hoisted(() => ({
  open: vi.fn(),
}));
const bridgeHarness = vi.hoisted(() => ({
  invokeToolByChannel: vi.fn(),
}));

vi.mock("../composables/useSettings", () => settingsHarness);
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
      return () => h("div", attrs, slots.default?.());
    },
  });
  for (const name of [
    "el-select",
    "el-option",
    "el-input",
    "el-tag",
    "el-tooltip",
    "el-form",
    "el-form-item",
    "el-empty",
    "el-icon",
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
  return findNode(root, (node) => node.type === "button" && nodeText(node).trim() === text);
}

function findButtonByAriaLabel(root: HostNode, label: string): HostNode | null {
  return findNode(
    root,
    (node) => node.type === "button" && node.props["aria-label"] === label,
  );
}

function findNodeByAriaLabel(root: HostNode, label: string): HostNode | null {
  return findNode(root, (node) => node.props["aria-label"] === label);
}

function modelValue(node: HostNode | null): unknown {
  if (!node) return undefined;
  return "modelValue" in node.props ? node.props.modelValue : node.props["model-value"];
}

async function flushPanel(): Promise<void> {
  for (let index = 0; index < 8; index += 1) {
    await Promise.resolve();
    await nextTick();
  }
}

async function mountPanel(templates: unknown[]) {
  settingsHarness.getSettingJson.mockReturnValue(templates);
  const { default: TestEmailAssistantPanel } = await import("./TestEmailAssistantPanel.vue");
  const renderer = createPanelRenderer();
  const root = hostNode("root");
  const app = renderer.createApp(TestEmailAssistantPanel);
  registerElementStubs(app);
  app.mount(root);
  await flushPanel();
  return { app, root };
}

const storedTemplates = [
  { id: "custom-one", name: "模板一", content: "自定义正文一" },
  { id: "custom-two", name: "模板二", content: "自定义正文二" },
];

beforeEach(() => {
  settingsHarness.getSettingJson.mockReset();
  settingsHarness.setSettingAndWait.mockReset();
  settingsHarness.setSettingAndWait.mockResolvedValue(undefined);
  dialogHarness.open.mockReset();
  bridgeHarness.invokeToolByChannel.mockReset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

const source = readFileSync(
  resolve(process.cwd(), "src/components/TestEmailAssistantPanel.vue"),
  "utf8",
);

describe("TestEmailAssistantPanel source structure", () => {
  it("supports template inspection, dynamic fields, and the two output actions", () => {
    expect(source).toContain("tool:test-email-assistant:inspect-template");
    expect(source).toContain("tool:test-email-assistant:generate-document");
    expect(source).toContain("v-for=\"name in allPlaceholders\"");
    expect(source).toContain("isMultilineFieldName(name)");
    expect(source).toContain(":autosize=\"isMultilineFieldName(name)");
    expect(source).toContain("navigator.clipboard.writeText(emailPreview.value)");
    expect(source).toContain("tool:system:reveal-in-folder");
  });

  it("keeps cancellation quiet and exposes real failures", () => {
    expect(source).toContain("if (!selected) return");
    expect(source).toContain("errorMessage.value = error instanceof Error ? error.message : String(error)");
    expect(source).toContain('error === "cancel" || error === "close"');
    expect(source).toContain("role=\"alert\"");
  });

  it("persists a normalized custom email template library through the dedicated setting", () => {
    expect(source).toContain('"test-email-assistant:email-templates:v1"');
    expect(source).toContain("getSettingJson<unknown>(EMAIL_BODY_TEMPLATES_SETTING_KEY, [])");
    expect(source).toContain("normalizeTestEmailBodyTemplates");
    expect(source).toContain(
      "await setSettingAndWait(EMAIL_BODY_TEMPLATES_SETTING_KEY, JSON.stringify(nextTemplates))",
    );
    expect(source).not.toContain("setSettingJson(");
  });

  it("uses a controlled selector and exposes explicit template commands", () => {
    expect(source).toContain(':model-value="activeEmailTemplateId"');
    expect(source).toContain('@change="selectEmailTemplate"');
    expect(source).not.toContain('v-model="activeEmailTemplateId"');
    expect(source).toContain("默认模板（内置）");
    expect(source).toContain('v-for="template in customEmailTemplates"');
    expect(source).toContain("另存为");
    expect(source).toContain("保存修改");
    expect(source).toContain('aria-label="重命名当前模板"');
    expect(source).toContain('aria-label="删除当前模板"');
  });

  it("protects dirty content and commits local template state only after persistence", () => {
    expect(source).toContain("emailTemplate.value !== loadedEmailTemplateContent.value");
    expect(source).toContain("当前邮件正文尚未保存，切换模板会丢失这些修改");
    expect(source).toContain("当前未保存的正文修改也会丢失");
    expect(source).toContain("if (!(await persistEmailTemplates(nextTemplates");
    expect(source).toContain("customEmailTemplates.value = nextTemplates");
    expect(source).toContain("applyEmailTemplate(findEmailTemplateById(BUILTIN_TEST_EMAIL_TEMPLATE_ID))");
    expect(source).toContain(':disabled="templatePersistencePending"');
  });

  it("only adds newly visible fields and keeps hidden values in the session", () => {
    expect(source).toContain("if (!(name in values)) values[name] = \"\";");
    expect(source).not.toContain("delete values");
  });

  it("leaves output naming to the backend and shows the generated path", () => {
    expect(source).not.toContain("建议文件名");
    expect(source).not.toContain("buildSuggestedDocumentFileName");
    expect(source).toContain(":title=\"outputPath\"");
  });

  it("uses compact responsive layout with long-content wrapping", () => {
    expect(source).toContain("overflow-wrap: anywhere");
    expect(source).toContain("@media (max-width: 760px)");
    expect(source).toContain("grid-template-columns: minmax(260px, 0.92fr) minmax(0, 1.08fr)");
    expect(source).toContain(".email-template-toolbar");
    expect(source).toContain(".template-library-actions");
    expect(source).toContain("flex-basis: 220px");
    expect(source).toContain("text-overflow: ellipsis");
  });
});

describe("TestEmailAssistantPanel template library behavior", () => {
  it("loads custom templates and protects a dirty draft before switching", async () => {
    const confirm = vi
      .spyOn(ElMessageBox, "confirm")
      .mockRejectedValueOnce("cancel")
      .mockResolvedValueOnce("confirm" as never);
    vi.spyOn(ElMessage, "success").mockReturnValue(undefined as never);
    const { app, root } = await mountPanel(storedTemplates);

    expect(settingsHarness.getSettingJson).toHaveBeenCalledWith(
      "test-email-assistant:email-templates:v1",
      [],
    );
    expect(findNode(root, (node) => node.props.value === "custom-one")?.props.label).toBe("模板一");
    expect(findNode(root, (node) => node.props.value === "custom-two")?.props.label).toBe("模板二");

    const emailInput = findNodeByAriaLabel(root, "邮件正文模板");
    (emailInput?.props["onUpdate:modelValue"] as (value: string) => void)("未保存草稿");
    await flushPanel();

    const selector = findNodeByAriaLabel(root, "选择邮件正文模板");
    await (selector?.props.onChange as (value: string) => Promise<void>)("custom-one");
    await flushPanel();

    expect(modelValue(findNodeByAriaLabel(root, "选择邮件正文模板"))).toBe(
      BUILTIN_TEST_EMAIL_TEMPLATE_ID,
    );
    expect(modelValue(findNodeByAriaLabel(root, "邮件正文模板"))).toBe("未保存草稿");

    await (selector?.props.onChange as (value: string) => Promise<void>)("custom-two");
    await flushPanel();

    expect(confirm).toHaveBeenCalledTimes(2);
    expect(modelValue(findNodeByAriaLabel(root, "选择邮件正文模板"))).toBe("custom-two");
    expect(modelValue(findNodeByAriaLabel(root, "邮件正文模板"))).toBe("自定义正文二");
    app.unmount();
  });

  it("commits a saved-as template only after persistence succeeds", async () => {
    vi.spyOn(ElMessageBox, "prompt").mockResolvedValue({ value: "新模板" } as never);
    const success = vi.spyOn(ElMessage, "success").mockReturnValue(undefined as never);
    let releasePersistence!: () => void;
    settingsHarness.setSettingAndWait.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        releasePersistence = resolve;
      }),
    );
    const { app, root } = await mountPanel([]);

    const emailInput = findNodeByAriaLabel(root, "邮件正文模板");
    (emailInput?.props["onUpdate:modelValue"] as (value: string) => void)("另存正文");
    await flushPanel();

    const savePromise = (findButton(root, "另存为")?.props.onClick as () => Promise<void>)();
    await flushPanel();

    expect(settingsHarness.setSettingAndWait).toHaveBeenCalledTimes(1);
    const [settingKey, serialized] = settingsHarness.setSettingAndWait.mock.calls[0] as [
      string,
      string,
    ];
    const persisted = JSON.parse(serialized) as Array<{
      id: string;
      name: string;
      content: string;
    }>;
    expect(settingKey).toBe("test-email-assistant:email-templates:v1");
    expect(persisted).toHaveLength(1);
    expect(persisted[0]).toMatchObject({ name: "新模板", content: "另存正文" });
    expect(modelValue(findNodeByAriaLabel(root, "选择邮件正文模板"))).toBe(
      BUILTIN_TEST_EMAIL_TEMPLATE_ID,
    );
    expect(nodeText(root)).toContain("自定义 0 个");

    releasePersistence();
    await savePromise;
    await flushPanel();

    expect(modelValue(findNodeByAriaLabel(root, "选择邮件正文模板"))).toBe(persisted[0]?.id);
    expect(modelValue(findNodeByAriaLabel(root, "邮件正文模板"))).toBe("另存正文");
    expect(findNode(root, (node) => node.props.label === "新模板")).not.toBeNull();
    expect(findButton(root, "保存修改")?.props.disabled).toBe(true);
    expect(nodeText(root)).toContain("无修改");
    expect(success).toHaveBeenCalledWith("邮件正文模板已保存");
    app.unmount();
  });

  it("keeps the active dirty draft when saving changes fails", async () => {
    const confirm = vi.spyOn(ElMessageBox, "confirm").mockResolvedValue("confirm" as never);
    const success = vi.spyOn(ElMessage, "success").mockReturnValue(undefined as never);
    settingsHarness.setSettingAndWait.mockRejectedValueOnce(new Error("数据库不可写"));
    const { app, root } = await mountPanel(storedTemplates);

    await (
      findNodeByAriaLabel(root, "选择邮件正文模板")?.props.onChange as (
        value: string,
      ) => Promise<void>
    )("custom-one");
    await flushPanel();
    const emailInput = findNodeByAriaLabel(root, "邮件正文模板");
    (emailInput?.props["onUpdate:modelValue"] as (value: string) => void)("失败后的正文");
    await flushPanel();

    expect(findButton(root, "保存修改")?.props.disabled).toBe(false);
    await (findButton(root, "保存修改")?.props.onClick as () => Promise<void>)();
    await flushPanel();

    const [, serialized] = settingsHarness.setSettingAndWait.mock.calls[0] as [string, string];
    expect(
      (JSON.parse(serialized) as Array<{ id: string; content: string }>).find(
        (template) => template.id === "custom-one",
      )?.content,
    ).toBe("失败后的正文");
    expect(confirm).not.toHaveBeenCalled();
    expect(success).not.toHaveBeenCalled();
    expect(modelValue(findNodeByAriaLabel(root, "选择邮件正文模板"))).toBe("custom-one");
    expect(modelValue(findNodeByAriaLabel(root, "邮件正文模板"))).toBe("失败后的正文");
    expect(nodeText(root)).toContain("保存邮件正文模板失败：数据库不可写");
    expect(nodeText(root)).toContain("未保存");
    expect(findButton(root, "保存修改")?.props.disabled).toBe(false);
    app.unmount();
  });

  it("persists rename and delete before returning to the built-in template", async () => {
    vi.spyOn(ElMessageBox, "prompt").mockResolvedValue({ value: "已重命名" } as never);
    const confirm = vi.spyOn(ElMessageBox, "confirm").mockResolvedValue("confirm" as never);
    const success = vi.spyOn(ElMessage, "success").mockReturnValue(undefined as never);
    const { app, root } = await mountPanel(storedTemplates);

    await (
      findNodeByAriaLabel(root, "选择邮件正文模板")?.props.onChange as (
        value: string,
      ) => Promise<void>
    )("custom-one");
    await flushPanel();
    await (
      findButtonByAriaLabel(root, "重命名当前模板")?.props.onClick as () => Promise<void>
    )();
    await flushPanel();

    const [, renameSerialized] = settingsHarness.setSettingAndWait.mock.calls[0] as [string, string];
    const renamed = JSON.parse(renameSerialized) as Array<{ id: string; name: string }>;
    expect(renamed.find((template) => template.id === "custom-one")?.name).toBe("已重命名");
    expect(modelValue(findNodeByAriaLabel(root, "选择邮件正文模板"))).toBe("custom-one");
    expect(findNode(root, (node) => node.props.label === "已重命名")).not.toBeNull();

    await (
      findButtonByAriaLabel(root, "删除当前模板")?.props.onClick as () => Promise<void>
    )();
    await flushPanel();

    expect(settingsHarness.setSettingAndWait).toHaveBeenCalledTimes(2);
    const [, deleteSerialized] = settingsHarness.setSettingAndWait.mock.calls[1] as [string, string];
    const remaining = JSON.parse(deleteSerialized) as Array<{ id: string; name: string }>;
    expect(remaining.map((template) => template.id)).toEqual(["custom-two"]);
    expect(confirm).toHaveBeenCalledWith(
      expect.stringContaining("已重命名"),
      "删除邮件正文模板",
      expect.objectContaining({ confirmButtonText: "删除" }),
    );
    expect(modelValue(findNodeByAriaLabel(root, "选择邮件正文模板"))).toBe(
      BUILTIN_TEST_EMAIL_TEMPLATE_ID,
    );
    expect(modelValue(findNodeByAriaLabel(root, "邮件正文模板"))).toBe(
      DEFAULT_TEST_EMAIL_TEMPLATE,
    );
    expect(success).toHaveBeenNthCalledWith(1, "邮件正文模板已重命名");
    expect(success).toHaveBeenNthCalledWith(2, "邮件正文模板已删除");
    app.unmount();
  });
});

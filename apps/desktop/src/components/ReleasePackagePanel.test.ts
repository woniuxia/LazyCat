// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { createRenderer, defineComponent, h, nextTick } from "vue";
import { describe, expect, it, vi } from "vitest";
import { APP_EVENTS } from "../bridge/events";
import type { ReleasePackageProject, ReleasePackageStatusEvent } from "../types/release-package";
import { createEmptyReleasePackageEnvironmentDraft } from "../utils/releasePackage";

const panelHarness = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: ReleasePackageStatusEvent }) => void>(),
  invoke: vi.fn(),
  focusInput: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    async (event: string, callback: (event: { payload: ReleasePackageStatusEvent }) => void) => {
      panelHarness.listeners.set(event, callback);
      return () => undefined;
    },
  ),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("../bridge/tauri", () => ({ invokeToolByChannel: panelHarness.invoke }));

const source = readFileSync(
  resolve(process.cwd(), "src/components/ReleasePackagePanel.vue"),
  "utf8",
);
const appSource = readFileSync(resolve(process.cwd(), "src/App.vue"), "utf8");
const handoffSource = readFileSync(
  resolve(process.cwd(), "src/composables/useNavigationHandoff.ts"),
  "utf8",
);
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
    createElement(type) {
      return hostNode(type);
    },
    createText(text) {
      return hostNode("#text", text);
    },
    createComment(text) {
      return hostNode("#comment", text);
    },
    setText(node, text) {
      node.text = text;
    },
    setElementText(node, text) {
      node.text = text;
      node.children = [];
    },
    parentNode(node) {
      return node.parent;
    },
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
): void {
  for (const name of [
    "el-checkbox",
    "el-checkbox-group",
    "el-collapse",
    "el-collapse-item",
    "el-dialog",
    "el-form",
    "el-form-item",
    "el-input",
    "el-input-number",
    "el-option",
    "el-popover",
    "el-progress",
    "el-radio-button",
    "el-radio-group",
    "el-select",
    "el-switch",
  ]) {
    app.component(
      name,
      defineComponent({
        inheritAttrs: false,
        setup(_props, { attrs, slots, expose }) {
          if (name === "el-input") expose({ focus: panelHarness.focusInput });
          return () =>
            h("div", attrs, [
              slots.default?.(),
              name === "el-dialog" && attrs.modelValue ? slots.footer?.() : null,
            ]);
        },
      }),
    );
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
  app.component(
    "el-tag",
    defineComponent({
      inheritAttrs: false,
      setup(_props, { attrs, slots }) {
        return () => h("span", attrs, slots.default?.());
      },
    }),
  );
}

function nodeText(node: HostNode): string {
  return `${node.text}${node.children.map(nodeText).join("")}`;
}

function buttonTexts(root: HostNode): string[] {
  const result: string[] = [];
  const visit = (node: HostNode) => {
    if (node.type === "button") result.push(nodeText(node).trim());
    node.children.forEach(visit);
  };
  visit(root);
  return result;
}

function findButton(root: HostNode, text: string): HostNode | null {
  let result: HostNode | null = null;
  const visit = (node: HostNode) => {
    if (!result && node.type === "button" && nodeText(node).trim() === text) result = node;
    node.children.forEach(visit);
  };
  visit(root);
  return result;
}

function findButtons(root: HostNode, text: string): HostNode[] {
  const result: HostNode[] = [];
  const visit = (node: HostNode) => {
    if (node.type === "button" && nodeText(node).trim() === text) result.push(node);
    node.children.forEach(visit);
  };
  visit(root);
  return result;
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

function modelValues(root: HostNode): unknown[] {
  const result: unknown[] = [];
  const visit = (node: HostNode) => {
    if ("modelValue" in node.props) result.push(node.props.modelValue);
    node.children.forEach(visit);
  };
  visit(root);
  return result;
}

async function flushMountedPanel(): Promise<void> {
  for (let index = 0; index < 6; index += 1) {
    await Promise.resolve();
    await nextTick();
  }
}

const mountedProject: ReleasePackageProject = {
  id: 7,
  name: "Portal",
  recentUsageCount: 0,
  frontendProjectPath: "C:\\portal\\web",
  backendProjectPath: "C:\\portal\\api",
  environments: [
    {
      ...createEmptyReleasePackageEnvironmentDraft(),
      id: 41,
      projectId: 7,
      environment: "test",
      configured: true,
      packageType: "local_archive",
      outputRoot: "D:\\releases-test",
      frontendBuildCommand: "pnpm build:test",
      frontendArtifactPath: "dist",
      backendBuildCommand: "mvn package -Ptest",
      backendArtifactPath: "target/portal.jar",
      createdAt: "2026-07-28T00:00:00Z",
      updatedAt: "2026-07-28T00:00:00Z",
    },
    {
      ...createEmptyReleasePackageEnvironmentDraft(),
      id: 42,
      projectId: 7,
      environment: "production",
      configured: true,
      packageType: "server_upload",
      frontendBuildCommand: "pnpm build:prod",
      frontendArtifactPath: "dist",
      backendBuildCommand: "mvn package -Pprod",
      backendArtifactPath: "target/portal.jar",
      sshAuthType: "private_key",
      sshHost: "deploy.internal",
      sshPort: 22,
      sshUsername: "deploy",
      sshPrivateKeyPath: "C:\\keys\\deploy",
      frontendRemoteDir: "/srv/portal/web",
      backendRemotePath: "/srv/portal/portal.jar",
      createdAt: "2026-07-28T00:00:00Z",
      updatedAt: "2026-07-28T00:00:00Z",
    },
  ],
  createdAt: "2026-07-28T00:00:00Z",
  updatedAt: "2026-07-28T00:00:00Z",
};

describe("ReleasePackagePanel", () => {
  it("requires the final production confirmation and sends it only for production", () => {
    expect(source).toContain("检查分支并确认");
    expect(source).toContain('"tool:release-package:branch-check"');
    expect(source).toContain('selectedEnvironmentKind.value === "production"');
    expect(source).toContain("async function confirmStart");
    expect(source).toContain("productionConfirmed");
    expect(source).toContain("selectedEnvironment.value.id");
  });

  it("configures independent production branches and skips checks for retries", () => {
    expect(source).toContain("environmentDraft.frontendExpectedBranch");
    expect(source).toContain("environmentDraft.backendExpectedBranch");
    expect(source).toContain("if (retryMode.value)");
    expect(source).toContain("confirmProductionRetry()");
  });

  it("selects an action target by environment id without keeping the default test environment", () => {
    expect(source).toContain("async function applyActionDispatchIntent");
    expect(source).toContain("findEnvironmentById");
    expect(source).toContain("selectedEnvironmentKind.value = target.environment");
    expect(source).toContain("await prepareStart()");
  });

  it("renders fixed test and production environments and defaults to test", () => {
    expect(source).toContain('value="test"');
    expect(source).toContain('value="production"');
    expect(source).toContain(
      'const selectedEnvironmentKind = ref<ReleasePackageEnvironmentKind>("test")',
    );
    expect(source).toContain('environment.configured ? "已配置" : "待配置"');
  });

  it("disables save for a clean draft and enables it after an edit", async () => {
    panelHarness.invoke.mockReset();
    panelHarness.invoke.mockImplementation(async (channel: string) => {
      if (channel === "tool:release-package:project-list") return { projects: [mountedProject] };
      if (channel === "tool:vault:meta-list") return [];
      return {};
    });
    const { default: ReleasePackagePanel } = await import("./ReleasePackagePanel.vue");
    const renderer = createPanelRenderer();
    const root = hostNode("root");
    const app = renderer.createApp(ReleasePackagePanel);
    registerElementStubs(app);
    app.mount(root);
    await flushMountedPanel();

    expect(findButton(root, "保存配置")?.props.disabled).toBe(true);
    const commandInput = findNode(root, (node) => node.props.modelValue === "pnpm build:test");
    expect(commandInput).not.toBeNull();
    (commandInput?.props["onUpdate:modelValue"] as (value: string) => void)(
      "pnpm build:test --changed",
    );
    await nextTick();

    expect(findButton(root, "保存配置")?.props.disabled).toBe(false);
    app.unmount();
  });

  it("copies a saved environment into the opposite environment as an unsaved draft", async () => {
    panelHarness.invoke.mockReset();
    panelHarness.invoke.mockImplementation(async (channel: string) => {
      if (channel === "tool:release-package:project-list") return { projects: [mountedProject] };
      if (channel === "tool:vault:meta-list") return [];
      return {};
    });
    const { default: ReleasePackagePanel } = await import("./ReleasePackagePanel.vue");
    const renderer = createPanelRenderer();
    const root = hostNode("root");
    const app = renderer.createApp(ReleasePackagePanel);
    registerElementStubs(app);
    app.mount(root);
    await flushMountedPanel();

    const copyToProduction = findButton(root, "复制到生产环境");
    expect(copyToProduction?.props.disabled).toBe(false);
    await (copyToProduction?.props.onClick as () => Promise<void>)();
    await nextTick();

    expect(findNode(root, (node) => node.props["model-value"] === "production")).not.toBeNull();
    expect(modelValues(root)).toContain("pnpm build:test");
    expect(modelValues(root)).not.toContain("pnpm build:prod");
    expect(findButton(root, "复制到测试环境")).not.toBeNull();
    expect(findButton(root, "保存配置")?.props.disabled).toBe(false);
    expect(panelHarness.invoke).not.toHaveBeenCalledWith(
      "tool:release-package:project-update",
      expect.anything(),
    );
    app.unmount();
  });

  it("supports copying production configuration back to test", async () => {
    panelHarness.invoke.mockReset();
    panelHarness.invoke.mockImplementation(async (channel: string) => {
      if (channel === "tool:release-package:project-list") return { projects: [mountedProject] };
      if (channel === "tool:vault:meta-list") return [];
      return {};
    });
    const { default: ReleasePackagePanel } = await import("./ReleasePackagePanel.vue");
    const renderer = createPanelRenderer();
    const root = hostNode("root");
    const app = renderer.createApp(ReleasePackagePanel);
    registerElementStubs(app);
    app.mount(root);
    await flushMountedPanel();

    const environmentControl = findNode(root, (node) => node.props["model-value"] === "test");
    await (environmentControl?.props.onChange as (value: string) => Promise<void>)("production");
    await nextTick();
    await (findButton(root, "复制到测试环境")?.props.onClick as () => Promise<void>)();
    await nextTick();

    expect(findNode(root, (node) => node.props["model-value"] === "test")).not.toBeNull();
    expect(modelValues(root)).toContain("pnpm build:prod");
    expect(modelValues(root)).not.toContain("pnpm build:test");
    expect(findButton(root, "保存配置")?.props.disabled).toBe(false);
    app.unmount();
  });

  it("guards dirty environment switches and saves shared plus selected environment", () => {
    expect(source).toContain("async function selectEnvironment");
    expect(source).toContain("await confirmDiscardChanges()");
    expect(source).toContain("selectedEnvironmentKind.value = environment");
    expect(source).toContain("async function saveProject");
    expect(source).toContain("projectDraft");
    expect(source).toContain("environmentDraft");
    expect(source).toContain("environmentId");
    expect(source).toContain("normalizeReleasePackageProjectDraft(projectDraft)");
    expect(source).toContain("normalizeReleasePackageEnvironmentDraft(environmentDraft)");
    expect(source).toMatch(/const targetEnvironmentKind = identity\.wasCreating\s*\? "test"/u);
    expect(source).toContain('class="release-package-form" :disabled="editorLocked"');
    expect(source).toContain("if (editorLocked.value) return;");
  });

  it("mounts the test environment without leaking production commands", async () => {
    panelHarness.invoke.mockImplementation(async (channel: string) => {
      if (channel === "tool:release-package:project-list") return { projects: [mountedProject] };
      if (channel === "tool:vault:meta-list") return [];
      return {};
    });
    const { default: ReleasePackagePanel } = await import("./ReleasePackagePanel.vue");
    const renderer = createPanelRenderer();
    const root = hostNode("root");
    const app = renderer.createApp(ReleasePackagePanel);
    registerElementStubs(app);
    app.mount(root);
    await flushMountedPanel();

    expect(nodeText(root)).toContain("测试环境");
    expect(nodeText(root)).toContain("生产环境");
    expect(modelValues(root)).toContain("pnpm build:test");
    expect(modelValues(root)).not.toContain("pnpm build:prod");
    app.unmount();
  });

  it("saves shared project fields with only the selected environment", async () => {
    panelHarness.invoke.mockReset();
    panelHarness.invoke.mockImplementation(async (channel: string) => {
      if (channel === "tool:release-package:project-list") return { projects: [mountedProject] };
      if (channel === "tool:release-package:project-update") return { id: 7, environmentId: 41 };
      if (channel === "tool:vault:meta-list") return [];
      return {};
    });
    const { default: ReleasePackagePanel } = await import("./ReleasePackagePanel.vue");
    const renderer = createPanelRenderer();
    const root = hostNode("root");
    const app = renderer.createApp(ReleasePackagePanel);
    registerElementStubs(app);
    app.mount(root);
    await flushMountedPanel();

    const saveButton = findButton(root, "保存配置");
    expect(saveButton).not.toBeNull();
    await (saveButton?.props.onClick as () => Promise<void>)();

    const {
      id: _id,
      projectId: _projectId,
      environment: _environment,
      configured: _configured,
      createdAt: _createdAt,
      updatedAt: _updatedAt,
      ...testEnvironmentConfig
    } = mountedProject.environments[0];
    expect(panelHarness.invoke).toHaveBeenCalledWith("tool:release-package:project-update", {
      id: 7,
      environmentId: 41,
      environment: "test",
      project: {
        name: "Portal",
        frontendProjectPath: "C:\\portal\\web",
        backendProjectPath: "C:\\portal\\api",
      },
      environmentConfig: testEnvironmentConfig,
    });
    expect(JSON.stringify(panelHarness.invoke.mock.calls)).not.toContain("pnpm build:prod");
    app.unmount();
  });

  it("locks project and environment transitions while a save is pending", async () => {
    let resolveUpdate!: (value: { id: number; environmentId: number }) => void;
    panelHarness.invoke.mockReset();
    panelHarness.invoke.mockImplementation(async (channel: string) => {
      if (channel === "tool:release-package:project-list") return { projects: [mountedProject] };
      if (channel === "tool:release-package:project-update") {
        return new Promise((resolve) => {
          resolveUpdate = resolve;
        });
      }
      if (channel === "tool:vault:meta-list") return [];
      return {};
    });
    const { default: ReleasePackagePanel } = await import("./ReleasePackagePanel.vue");
    const renderer = createPanelRenderer();
    const root = hostNode("root");
    const app = renderer.createApp(ReleasePackagePanel);
    registerElementStubs(app);
    app.mount(root);
    await flushMountedPanel();

    const savePromise = (findButton(root, "保存配置")?.props.onClick as () => Promise<void>)();
    await nextTick();
    const environmentControl = findNode(root, (node) => node.props["model-value"] === "test");
    expect(environmentControl).not.toBeNull();

    await (environmentControl?.props.onChange as (value: string) => Promise<void>)("production");
    resolveUpdate({ id: 7, environmentId: 41 });
    await savePromise;
    expect(findNode(root, (node) => node.props["model-value"] === "test")).not.toBeNull();
    expect(modelValues(root)).not.toContain("pnpm build:prod");
    app.unmount();
  });

  it("retries only the list refresh after a committed save", async () => {
    let projectListCalls = 0;
    let failRefresh = true;
    panelHarness.invoke.mockReset();
    panelHarness.invoke.mockImplementation(async (channel: string) => {
      if (channel === "tool:release-package:project-list") {
        projectListCalls += 1;
        if (projectListCalls > 1 && failRefresh) throw new Error("refresh failed");
        return { projects: [mountedProject] };
      }
      if (channel === "tool:release-package:project-update") return { id: 7, environmentId: 41 };
      if (channel === "tool:vault:meta-list") return [];
      return {};
    });
    const { default: ReleasePackagePanel } = await import("./ReleasePackagePanel.vue");
    const renderer = createPanelRenderer();
    const root = hostNode("root");
    const app = renderer.createApp(ReleasePackagePanel);
    registerElementStubs(app);
    app.mount(root);
    await flushMountedPanel();

    await (findButton(root, "保存配置")?.props.onClick as () => Promise<void>)();
    expect(findButton(root, "重试刷新")).not.toBeNull();
    expect(
      panelHarness.invoke.mock.calls.filter(
        ([channel]) => channel === "tool:release-package:project-update",
      ),
    ).toHaveLength(1);

    failRefresh = false;
    await (findButton(root, "重试刷新")?.props.onClick as () => Promise<void>)();
    expect(findButton(root, "保存配置")).not.toBeNull();
    expect(
      panelHarness.invoke.mock.calls.filter(
        ([channel]) => channel === "tool:release-package:project-update",
      ),
    ).toHaveLength(1);
    app.unmount();
  });

  it("uses a master-detail workspace and explicit run confirmation", () => {
    expect(source).toContain('class="release-package-projects"');
    expect(source).toContain('class="release-package-editor"');
    expect(source).toMatch(
      /<section class="project-overview">[\s\S]*<header class="editor-header">[\s\S]*<div class="project-basics">/u,
    );
    expect(source).toContain('class="release-package-log"');
    expect(source).toContain("确认本地归档");
    expect(source).toContain("确认上传");
    expect(source).toContain("终止打包");
  });

  it("records a successful project switch and immediately reorders by recent usage", async () => {
    const adminProject: ReleasePackageProject = {
      ...mountedProject,
      id: 8,
      name: "Admin",
      environments: mountedProject.environments.map((environment) => ({
        ...environment,
        id: environment.id + 10,
        projectId: 8,
      })),
    };
    panelHarness.invoke.mockReset();
    panelHarness.invoke.mockImplementation(async (channel: string) => {
      if (channel === "tool:release-package:project-list") {
        return { projects: [mountedProject, adminProject] };
      }
      if (channel === "tool:release-package:project-record-open") {
        return {
          resourceId: "8",
          summary: { totalCount: 4, windowCount: 4, lastUsedAt: 1, actionCounts: { open: 4 } },
        };
      }
      if (channel === "tool:vault:meta-list") return [];
      return {};
    });
    const { default: ReleasePackagePanel } = await import("./ReleasePackagePanel.vue");
    const renderer = createPanelRenderer();
    const root = hostNode("root");
    const app = renderer.createApp(ReleasePackagePanel);
    registerElementStubs(app);
    app.mount(root);
    await flushMountedPanel();

    const adminButton = findNode(
      root,
      (node) =>
        node.type === "button" &&
        String(node.props.class).includes("project-item") &&
        nodeText(node).includes("Admin"),
    );
    await (adminButton?.props.onClick as () => Promise<void>)();
    await nextTick();

    expect(panelHarness.invoke).toHaveBeenCalledWith("tool:release-package:project-record-open", {
      id: 8,
    });
    const projectButtons = buttonTexts(root).filter((text) =>
      ["Portal", "Admin"].some((name) => text.includes(name)),
    );
    expect(projectButtons[0]).toContain("Admin");
    app.unmount();
  });

  it("edits the project name from the header without a duplicate basics field", () => {
    expect(source).not.toContain('class="editor-hint"');
    expect(source).not.toContain('<el-form-item label="项目名称"');
    expect(source).toContain('ref="projectTitleInput"');
    expect(source).toContain('v-model="projectDraft.name"');
    expect(source).toContain('@dblclick="startTitleEdit"');
    expect(source).toContain('@keydown.enter.prevent="startTitleEdit"');
    expect(source).toContain('@blur="finishTitleEdit"');
    expect(source).toContain('@keydown.enter.stop.prevent="finishTitleEdit"');
  });

  it("uses all release-package actions without global setting persistence", () => {
    for (const channel of [
      "project-list",
      "project-record-open",
      "project-create",
      "project-update",
      "project-delete",
      "prepare",
      "target-check",
      "start",
    ]) {
      expect(source).toContain(`tool:release-package:${channel}`);
    }
    expect(source).not.toContain("setSettingAndWait");
    expect(source).not.toContain("release_package.output_root");
    expect(source).toContain("useReleasePackageRuntime");
    expect(source).toContain("tool:system:open-local-path");
  });

  it("keeps runtime listeners alive across panel navigation", () => {
    expect(source).toContain("await runtime.ensureListeners()");
    expect(source).not.toContain("onUnmounted");
  });

  it("does not persist logs or expose a persistent overwrite preference", () => {
    expect(source).not.toContain("localStorage");
    expect(source).not.toContain('v-model="overwriteExisting"');
  });

  it("checks an existing target before start and requires explicit overwrite confirmation", () => {
    expect(source).toContain("tool:release-package:target-check");
    expect(source).toContain(
      "目标归档目录已存在。直接覆盖将完整替换其中的所有文件，此操作无法撤销。",
    );
    expect(source).toContain('confirmButtonText: "直接覆盖"');
    expect(source).toContain('cancelButtonText: "取消"');
    expect(source).toContain("overwriteExisting");
    const confirmStart = source.slice(
      source.indexOf("async function confirmStart"),
      source.indexOf("async function cancelRun"),
    );
    expect(confirmStart.indexOf("confirmArchiveOverwrite")).toBeLessThan(
      confirmStart.indexOf("runtime.beginStart"),
    );
  });

  it("stores the archive root in each project and validates Windows folder names", () => {
    expect(source).toContain('v-model="environmentDraft.outputRoot"');
    expect(source).toContain("readonly");
    expect(source).toContain("environmentDraft.outputRoot = path");
    expect(source).toContain("validateArchiveFolderName");
    expect(source).toContain("COM[1-9]");
    expect(source).toContain("cancelPendingStart");
  });

  it("restores the active runtime project and uses prepare paths after refresh", () => {
    expect(source).toContain("runtime.activeEnvironmentId");
    expect(source).toMatch(
      /const preferActiveProject\s*=\s*\(selectedId\.value === null && !dirty\.value\)\s*\|\|\s*runtime\.status\.value === "running"/,
    );
    expect(source).toContain('prepareResult.value?.packageType !== "local_archive"');
    expect(source).toContain("prepareResult.value.outputRoot");
    expect(source).toContain("prepareResult.value.archivePath");
    expect(source).toContain("const refreshed = await loadProjects()");
  });

  it("clears a deleted project before attempting to refresh", () => {
    const deleteStart = source.indexOf("async function deleteProject");
    const removeProject = source.indexOf("projects.value = projects.value.filter", deleteStart);
    const clearSelection = source.indexOf("selectedId.value = null", deleteStart);
    const clearDraft = source.indexOf("restoreSelectedDrafts()", deleteStart);
    const refresh = source.indexOf("const refreshed = await loadProjects()", deleteStart);

    expect(deleteStart).toBeGreaterThan(-1);
    expect(removeProject).toBeGreaterThan(deleteStart);
    expect(clearSelection).toBeGreaterThan(removeProject);
    expect(clearDraft).toBeGreaterThan(clearSelection);
    expect(refresh).toBeGreaterThan(clearDraft);
  });

  it("uses a responsive engineering workspace with multiline command editors", () => {
    expect(source).toContain('class="engineering-grid"');
    expect(source).toContain('class="engineering-card frontend-card"');
    expect(source).toContain('class="engineering-card backend-card"');
    expect(source).toMatch(
      /\.engineering-grid\s*\{[^}]*grid-template-columns:\s*repeat\(auto-fit,\s*minmax\(min\(100%,\s*380px\),\s*1fr\)\);/s,
    );
    expect(source.match(/type="textarea"/g)).toHaveLength(4);
    expect(source.match(/:autosize="\{ minRows: 3, maxRows: 8 \}"/g)).toHaveLength(2);
    expect(source.match(/:autosize="\{ minRows: 4, maxRows: 9 \}"/g)).toHaveLength(2);
    expect(source).toContain("同一 PowerShell 会话中顺序执行");
    expect(source).toContain("$LASTEXITCODE");
  });

  it("lets the outer page scroll through the full workspace", () => {
    expect(source).toMatch(/\.release-package-panel\s*\{[^}]*flex:\s*0 0 auto;/s);
    expect(source).toMatch(/\.release-package-workspace\s*\{[^}]*overflow:\s*visible;/s);
  });

  it("renders command examples and reports clipboard failures", () => {
    const copyFunctionStart = source.indexOf("async function copyCommandExample");
    const nextAsyncFunction = source.indexOf("\nasync function", copyFunctionStart + 1);
    const copyFunctionSource = source.slice(copyFunctionStart, nextAsyncFunction);

    expect(source).toContain("RELEASE_PACKAGE_COMMAND_EXAMPLES");
    expect(source).toContain("RELEASE_PACKAGE_BACKEND_COMMAND_EXAMPLES");
    expect(source.match(/常用示例/g)?.length).toBeGreaterThanOrEqual(2);
    expect(source).toContain("CopyDocument");
    expect(copyFunctionStart).toBeGreaterThan(-1);
    expect(nextAsyncFunction).toBeGreaterThan(copyFunctionStart);
    expect(copyFunctionSource).toContain("await writeReleasePackageCommand(");
    expect(copyFunctionSource).toContain("(value) => navigator.clipboard.writeText(value)");
    expect(copyFunctionSource).toContain('ElMessage.success("命令示例已复制")');
    expect(copyFunctionSource).toContain("showError(error)");
    expect(source.match(/popper-class="release-package-command-examples"/g) ?? []).toHaveLength(2);
    expect(source.match(/:aria-label="`复制\$\{example\.title\}命令`"/g) ?? []).toHaveLength(2);
    expect(source).toContain(":global(.release-package-command-examples)");
  });

  it("wraps logs in a white status card", () => {
    expect(source).toContain('class="release-package-log-card release-package-project-log"');
    expect(source).toContain('class="log-status"');
    expect(source).toContain("computed(() => releasePackageRunStatusLabel(status.value))");
    expect(source).toContain("{{ statusLabel }}");
    expect(source).toMatch(
      /<el-tag\s+class="log-status"\s+role="status"\s+aria-live="polite"\s+aria-atomic="true"/u,
    );
    expect(source).toMatch(/\.release-package-log\s*\{[^}]*background:\s*#fff;/s);
    expect(source).toMatch(/\.log-card-header p\s*\{[^}]*color:\s*#5f6b7a;/s);
    expect(source).toMatch(/\.log-meta\s*\{[^}]*color:\s*#5f6b7a;/s);
    for (const [variant, textColor] of [
      ["primary", "#1d4ed8"],
      ["success", "#237a3b"],
      ["info", "#4b5563"],
      ["warning", "#8a4b08"],
      ["danger", "#b42318"],
    ]) {
      expect(source).toContain(`:deep(.log-status.el-tag--${variant})`);
      expect(source).toContain(`--el-tag-text-color: ${textColor};`);
    }
    expect(source).toContain('ref="frontendLogContainer"');
    expect(source).toContain('ref="backendLogContainer"');
    expect(source).toContain("@scroll=\"handleLogScroll('frontend')\"");
    expect(source).toContain("@scroll=\"handleLogScroll('backend')\"");
    expect(source).toContain("@scroll=\"handleLogScroll('upload')\"");
    expect(source).toContain("const logFollowState = reactive");
    expect(source).toContain("container.scrollTop = container.scrollHeight");
    expect(source).toContain('scrollLogToBottom("frontend", true)');
    expect(source).toContain('scrollLogToBottom("backend", true)');
    expect(source).toContain('scrollLogToBottom("upload", true)');
    expect(source).toContain('aria-live="polite"');
  });

  it("keeps overall, target, and upload errors visible in the log card", () => {
    expect(source).toContain(
      'const overallError = computed(() => currentProjectRuntime.value?.error ?? "")',
    );
    expect(source).toContain(
      'const frontendError = computed(() => currentProjectRuntime.value?.targetErrors.frontend ?? "")',
    );
    expect(source).toContain(
      'const backendError = computed(() => currentProjectRuntime.value?.targetErrors.backend ?? "")',
    );
    expect(source).toMatch(
      /<header class="log-card-header">[\s\S]*v-if="overallError"[\s\S]*\{\{ overallError \}\}[\s\S]*<\/header>/u,
    );
    expect(source).toMatch(
      /<strong>前端<\/strong>[\s\S]*v-if="frontendError"[\s\S]*\{\{ frontendError \}\}/u,
    );
    expect(source).toMatch(
      /<strong>后端<\/strong>[\s\S]*v-if="backendError"[\s\S]*\{\{ backendError \}\}/u,
    );
    expect(source).toMatch(
      /class="release-package-log-lane upload-log-lane"[\s\S]*v-if="overallError"[\s\S]*\{\{ overallError \}\}/u,
    );
    expect(source.match(/class="log-error-summary[^"]*"\s+role="alert"/gu) ?? []).toHaveLength(4);
    expect(source).toMatch(/\.log-error-summary\s*\{[^}]*white-space:\s*pre-wrap;/su);
  });

  it("selects artifact paths, run targets, and renders project-scoped log columns", () => {
    expect(source).toContain("chooseFrontendArtifact");
    expect(source).toContain("chooseBackendArtifact");
    expect(source).toContain('chooseDirectory("选择前端产物目录")');
    expect(source).toContain('chooseFile("选择后端产物文件")');
    expect(source.indexOf("归档目录名")).toBeLessThan(source.indexOf("本次打包内容"));
    expect(source).toContain('label="前端包"');
    expect(source).toContain('label="后端包"');
    expect(source).toContain("createDefaultReleasePackageTargets()");
    expect(source).toContain("createReleasePackageStartPayload(packageType");
    expect(source).toContain("targets: selectedTargets.value");
    expect(source).toContain("release-package-project-log");
    expect(source).toContain('class="release-package-log-columns"');
    expect(source).toContain('ref="frontendLogContainer"');
    expect(source).toContain('ref="backendLogContainer"');
  });

  it("adds an accessible archive shortcut to both log lanes after a successful archive", () => {
    expect(source.match(/class="log-lane-actions"/g) ?? []).toHaveLength(2);
    expect(source.match(/aria-label="打开归档目录"/g) ?? []).toHaveLength(2);
    expect(source.match(/@click="openArchive"/g) ?? []).toHaveLength(3);
  });

  it("configures upload separately and preflights before runtime start", () => {
    expect(source).toContain('v-model="environmentDraft.packageType"');
    for (const model of [
      "environmentDraft.sshHost",
      "environmentDraft.sshPort",
      "environmentDraft.sshUsername",
      "environmentDraft.sshAuthType",
      "environmentDraft.sshPrivateKeyPath",
      "environmentDraft.frontendRemoteDir",
      "environmentDraft.backendRemotePath",
    ]) {
      expect(source).toContain(`v-model="${model}"`);
    }
    expect(source).toContain("useReleasePackageUploadPreflight");
    expect(source).toContain("tool:release-package:upload-retry");
    const confirmStart = source.slice(
      source.indexOf("async function confirmStart"),
      source.indexOf("async function cancelRun"),
    );
    expect(confirmStart.indexOf("runUploadPreflight")).toBeLessThan(
      confirmStart.indexOf("runtime.beginStart"),
    );
    expect(source).toContain('type="password"');
    expect(source).toContain('credentialSecret.value = ""');
    expect(source).not.toContain("environmentDraft.password");
  });

  it("unlocks a locked password-bound upload in the current confirmation dialog and resumes it", async () => {
    panelHarness.invoke.mockReset();
    panelHarness.focusInput.mockReset();
    const uploadProject: ReleasePackageProject = {
      ...mountedProject,
      environments: [
        {
          ...mountedProject.environments[0],
          packageType: "server_upload",
          sshAuthType: "password",
          vaultEntryId: 11,
          frontendRemoteDir: "/srv/portal/web",
          backendRemotePath: "/srv/portal/portal.jar",
        },
        mountedProject.environments[1],
      ],
    };
    let vaultUnlocked = false;
    let unlockAttempts = 0;
    panelHarness.invoke.mockImplementation(
      async (channel: string, payload: Record<string, unknown>) => {
        if (channel === "tool:release-package:project-list") return { projects: [uploadProject] };
        if (channel === "tool:vault:meta-list") {
          return [
            {
              id: 11,
              category: "server",
              title: "生产服务器",
              plainFields: { address: "deploy.internal", port: 22, account: "deploy" },
            },
          ];
        }
        if (channel === "tool:release-package:prepare") return { packageType: "server_upload" };
        if (channel === "tool:release-package:remote-probe") {
          return {
            probeToken: "probe-1",
            host: "deploy.internal",
            port: 22,
            keyType: "ed25519",
            fingerprintSha256: "SHA256:key",
            trust: "trusted",
          };
        }
        if (channel === "tool:vault:status") return { setup: true, unlocked: vaultUnlocked };
        if (channel === "tool:vault:unlock") {
          unlockAttempts += 1;
          if (unlockAttempts === 1) throw new Error("wrong_password");
          vaultUnlocked = true;
          return { unlocked: true };
        }
        if (channel === "tool:release-package:remote-preflight") {
          return { preflightToken: "preflight-1", expiresAt: "later", targets: [] };
        }
        if (channel === "tool:release-package:start") {
          expect(payload.preflightToken).toBe("preflight-1");
          return { runId: "run-1" };
        }
        return {};
      },
    );

    const [{ default: ReleasePackagePanel }, { useReleasePackageRuntime }] = await Promise.all([
      import("./ReleasePackagePanel.vue"),
      import("../composables/useReleasePackageRuntime"),
    ]);
    const runtime = useReleasePackageRuntime();
    runtime.reset();
    const renderer = createPanelRenderer();
    const root = hostNode("root");
    const app = renderer.createApp(ReleasePackagePanel);
    registerElementStubs(app);
    app.mount(root);
    await flushMountedPanel();

    await (findButton(root, "开始打包")?.props.onClick as () => Promise<void>)();
    await flushMountedPanel();
    const confirmUpload = findButton(root, "确认构建并上传");
    expect(
      confirmUpload,
      `${buttonTexts(root).join(" | ")} calls=${panelHarness.invoke.mock.calls.map((call) => call[0]).join(",")}`,
    ).not.toBeNull();
    await (confirmUpload?.props.onClick as () => Promise<void>)();
    await flushMountedPanel();
    expect(buttonTexts(root)).toContain("解锁并继续");
    expect(buttonTexts(root)).not.toContain("确认构建并上传");

    const passwordInput = findNode(root, (node) => node.props.placeholder === "输入主密码");
    (passwordInput?.props["onUpdate:modelValue"] as (value: string) => void)("wrong");
    await nextTick();
    await (findButton(root, "解锁并继续")?.props.onClick as () => Promise<void>)();
    await flushMountedPanel();
    expect(findNode(root, (node) => node.props.error === "主密码错误，请重试")).not.toBeNull();
    const clearedInput = findNode(root, (node) => node.props.placeholder === "输入主密码");
    expect(clearedInput?.props.modelValue ?? clearedInput?.props["model-value"]).toBe("");
    expect(panelHarness.focusInput).toHaveBeenCalled();

    const correctedInput = findNode(root, (node) => node.props.placeholder === "输入主密码");
    (correctedInput?.props["onUpdate:modelValue"] as (value: string) => void)("master");
    await nextTick();
    const submitOnEnter = correctedInput?.props.onKeyup as (event: KeyboardEvent) => void;
    expect(submitOnEnter).toBeTypeOf("function");
    submitOnEnter(new KeyboardEvent("keyup", { key: "Enter" }));
    for (let index = 0; index < 4; index += 1) await flushMountedPanel();

    expect(panelHarness.invoke).toHaveBeenCalledWith("tool:vault:unlock", {
      masterPassword: "master",
    });
    const channels = panelHarness.invoke.mock.calls.map((call) => call[0]);
    expect(channels.indexOf("tool:release-package:remote-probe")).toBeLessThan(
      channels.indexOf("tool:vault:status"),
    );
    expect(channels.indexOf("tool:vault:unlock")).toBeLessThan(
      channels.indexOf("tool:release-package:remote-preflight"),
    );
    expect(channels.indexOf("tool:release-package:remote-preflight")).toBeLessThan(
      channels.indexOf("tool:release-package:start"),
    );
    expect(buttonTexts(root)).not.toContain("解锁并继续");
    app.unmount();
    runtime.reset();
  });

  it.each([
    {
      flow: "upload retry",
      status: "package_succeeded_upload_failed" as const,
      actionLabel: "重试上传",
      confirmLabel: "确认重试",
      preflightChannel: "tool:release-package:remote-preflight",
      startChannel: "tool:release-package:upload-retry",
    },
    {
      flow: "command retry",
      status: "upload_succeeded_command_failed" as const,
      actionLabel: "仅重试失败命令",
      confirmLabel: "仅重试失败命令",
      preflightChannel: "tool:release-package:command-retry-preflight",
      startChannel: "tool:release-package:command-retry-start",
    },
  ])("unlocks and resumes $flow inside its existing dialog", async (scenario) => {
    panelHarness.invoke.mockReset();
    const uploadProject: ReleasePackageProject = {
      ...mountedProject,
      environments: [
        {
          ...mountedProject.environments[0],
          packageType: "server_upload",
          sshAuthType: "password",
          vaultEntryId: 11,
          frontendRemoteDir: "/srv/portal/web",
          backendRemotePath: "/srv/portal/portal.jar",
        },
        mountedProject.environments[1],
      ],
    };
    let vaultUnlocked = scenario.flow === "upload retry";
    let preflightAttempts = 0;
    panelHarness.invoke.mockImplementation(async (channel: string) => {
      if (channel === "tool:release-package:project-list") return { projects: [uploadProject] };
      if (channel === "tool:vault:meta-list") {
        return [
          {
            id: 11,
            category: "server",
            title: "生产服务器",
            plainFields: { address: "deploy.internal", port: 22, account: "deploy" },
          },
        ];
      }
      if (
        channel === "tool:release-package:remote-probe" ||
        channel === "tool:release-package:command-retry-prepare"
      ) {
        return {
          probeToken: "probe-retry",
          host: "deploy.internal",
          port: 22,
          username: "deploy",
          keyType: "ed25519",
          fingerprintSha256: "SHA256:key",
          trust: "trusted",
          authType: "password",
          targets: ["backend"],
        };
      }
      if (channel === "tool:vault:status") return { setup: true, unlocked: vaultUnlocked };
      if (channel === "tool:vault:unlock") {
        vaultUnlocked = true;
        return { unlocked: true };
      }
      if (channel === "tool:release-package:remote-preflight") {
        preflightAttempts += 1;
        if (scenario.flow === "upload retry" && preflightAttempts === 1) {
          vaultUnlocked = false;
          throw new Error("vault_locked");
        }
        return { preflightToken: "preflight-retry", expiresAt: "later", targets: [] };
      }
      if (channel === "tool:release-package:command-retry-preflight") {
        return { authToken: "auth-retry", expiresAt: "later" };
      }
      if (channel === scenario.startChannel) return { runId: `run-${scenario.flow}` };
      return {};
    });

    const [{ default: ReleasePackagePanel }, { useReleasePackageRuntime }] = await Promise.all([
      import("./ReleasePackagePanel.vue"),
      import("../composables/useReleasePackageRuntime"),
    ]);
    const runtime = useReleasePackageRuntime();
    runtime.reset();
    const renderer = createPanelRenderer();
    const root = hostNode("root");
    const app = renderer.createApp(ReleasePackagePanel);
    registerElementStubs(app);
    app.mount(root);
    await flushMountedPanel();

    runtime.beginStart(41, ["frontend", "backend"]);
    panelHarness.listeners.get(APP_EVENTS.RELEASE_PACKAGE_STATUS)?.({
      payload: {
        runId: "failed-run",
        environmentId: 41,
        projectId: uploadProject.id,
        environment: "test",
        status: scenario.status,
        phase: "overall",
        retryToken: "upload-retry",
        commandRetryToken: "command-retry",
      },
    });
    await nextTick();
    await (findButton(root, scenario.actionLabel)?.props.onClick as () => Promise<void>)();
    await flushMountedPanel();
    const confirmationButtons = findButtons(root, scenario.confirmLabel);
    const confirmationButton = confirmationButtons.at(-1);
    expect(confirmationButton).toBeDefined();
    await (confirmationButton?.props.onClick as () => Promise<void>)();
    await flushMountedPanel();
    expect(buttonTexts(root)).toContain("解锁并继续");

    const passwordInput = findNode(root, (node) => node.props.placeholder === "输入主密码");
    (passwordInput?.props["onUpdate:modelValue"] as (value: string) => void)("master");
    await nextTick();
    await (findButton(root, "解锁并继续")?.props.onClick as () => Promise<void>)();
    for (let index = 0; index < 4; index += 1) await flushMountedPanel();

    const channels = panelHarness.invoke.mock.calls.map((call) => call[0]);
    expect(channels.indexOf("tool:vault:unlock")).toBeLessThan(
      channels.lastIndexOf(scenario.preflightChannel),
    );
    expect(channels.lastIndexOf(scenario.preflightChannel)).toBeLessThan(
      channels.indexOf(scenario.startChannel),
    );
    if (scenario.flow === "upload retry") {
      expect(
        channels.filter((channel) => channel === "tool:release-package:remote-probe"),
      ).toHaveLength(1);
    }
    expect(buttonTexts(root)).not.toContain("解锁并继续");
    app.unmount();
    runtime.reset();
  });

  it("awaits remote token revocation on dialog reset and terminal paths", () => {
    const clearStart = source.slice(
      source.indexOf("async function clearSensitiveStartState"),
      source.indexOf("async function prepareStart"),
    );
    const confirmStart = source.slice(
      source.indexOf("async function confirmStart"),
      source.indexOf("async function cancelRun"),
    );

    expect(clearStart).toContain("await uploadPreflight.reset()");
    expect(clearStart).toContain("async function resetStartDialog");
    expect(clearStart).toContain("async function closeStartDialog");
    expect(source).toContain("await resetStartDialog()");
    expect(confirmStart).toMatch(/finally\s*\{[\s\S]*await clearSensitiveStartState\(\)/u);
  });

  it("binds a Vault server credential for password auth without rendering a password field", () => {
    expect(source).toContain('label="密码库凭据"');
    expect(source).toContain('v-model="environmentDraft.vaultEntryId"');
    expect(source).toContain("tool:vault:meta-list");
    expect(source).toContain("v-if=\"environmentDraft.sshAuthType === 'password'\"");
    expect(source).toContain("密码由密码库提供");
    expect(source).not.toContain("请输入服务器密码");
    expect(source).not.toContain("? { password: credentialSecret.value }");
  });

  it("uses the Vault server port for password auth and keeps manual port input private-key only", () => {
    const mobileStyles = source.slice(source.indexOf("@media (max-width: 640px)"));

    expect(source).toMatch(
      /<el-form-item\s+v-if="environmentDraft\.sshAuthType === 'private_key'"\s+label="SSH 端口"\s+required\s*>/,
    );
    expect(source).not.toContain('<el-form-item label="SSH 端口" required>');
    expect(source).toContain("port?: unknown");
    expect(source).toContain("normalizeVaultServerPort(entry.plainFields?.port)");
    expect(source).toContain("complete: Boolean(address && account && port !== null)");
    expect(source).toContain(':disabled="!option.complete"');
    expect(source).toMatch(
      /&&\s*\(!selectedVaultCredential\.value\s*\|\|\s*!selectedVaultCredential\.value\.complete\)/,
    );
    expect(source).toContain("{{ selectedVaultCredential.port }}");
    expect(source).toContain("缺少地址、端口、账号或密码");
    expect(source).toMatch(
      /\.vault-credential-summary\s*\{[^}]*grid-template-columns:\s*repeat\(3,\s*minmax\(0,\s*1fr\)\);/s,
    );
    expect(mobileStyles).toMatch(
      /\.vault-credential-summary\s*\{[^}]*grid-template-columns:\s*1fr;/s,
    );
  });

  it("keeps only the private-key passphrase input in the start dialog", () => {
    expect(source).toContain("environmentDraft.sshAuthType === 'private_key'");
    expect(source).toContain("privateKeyPassphrase: credentialSecret.value || undefined");
  });

  it("opens the Vault through the application tool navigation event", () => {
    expect(source).toContain('emit("open-tool", "vault")');
    expect(appSource).toContain('@open-tool="onSelect"');
  });

  it("routes action dispatch intents before selecting the target tool", () => {
    const listenerStart = handoffSource.indexOf("APP_EVENTS.ACTION_CENTER_DISPATCH_REQUEST");
    const listenerEnd = handoffSource.indexOf("\n    ),", listenerStart);
    const listenerSource = handoffSource.slice(listenerStart, listenerEnd);
    const dispatchStart = appSource.indexOf("onActionCenterDispatch:");
    const dispatchEnd = appSource.indexOf("onInvalidActionCenterDispatch:", dispatchStart);
    const dispatchSource = appSource.slice(dispatchStart, dispatchEnd);

    expect(listenerStart).toBeGreaterThan(-1);
    expect(listenerSource).toContain("normalizeActionDispatchRequest(payload");
    expect(dispatchSource).toContain("navigationHandoff.setPendingIntent(request)");
    expect(dispatchSource).toContain("onSelect(request.targetToolId)");
    expect(dispatchSource.indexOf("navigationHandoff.setPendingIntent(request)")).toBeLessThan(
      dispatchSource.indexOf("onSelect(request.targetToolId)"),
    );
    expect(dispatchSource).not.toContain("setPendingToolInput");
  });

  it("consumes release-package intents without overwriting a dirty or running editor", () => {
    const applyStart = source.indexOf("async function applyActionDispatchIntent");
    const applyEnd = source.indexOf("\nasync function", applyStart + 1);
    const applySource = source.slice(applyStart, applyEnd);
    const dirtyStart = applySource.indexOf("if (dirty.value)");
    const runningStart = applySource.indexOf("if (running.value)");
    const dirtyBranch = applySource.slice(dirtyStart, runningStart);

    expect(source).toContain('watchPendingIntent("release-package", applyActionDispatchIntent)');
    expect(applySource).toContain('intent.actionType !== "release_package.run"');
    expect(dirtyBranch).toContain('stopPendingActionDispatch("failed"');
    expect(dirtyBranch).not.toContain("selectedId.value =");
    expect(dirtyBranch).not.toContain("restoreSelectedDrafts");
    expect(applySource).toContain(
      'stopPendingActionDispatch("failed", "已有发布打包任务正在运行")',
    );
  });

  it("reloads and selects the exact intent target before using the existing prepare flow", () => {
    const applyStart = source.indexOf("async function applyActionDispatchIntent");
    const applyEnd = source.indexOf("\nasync function", applyStart + 1);
    const applySource = source.slice(applyStart, applyEnd);

    expect(applySource).toContain("const loaded = await loadProjects({ preserveEditor: true })");
    expect(applySource).toContain("findEnvironmentById(intent.targetId)");
    expect(applySource).toContain('stopPendingActionDispatch("failed", "上线包环境配置不存在")');
    expect(applySource).toContain('stopPendingActionDispatch("failed", "上线包环境配置不完整")');
    expect(applySource).not.toContain("projects.value[0]");
    expect(applySource).toContain("selectedId.value = targetLocation.project.id");
    expect(applySource).toContain("selectedEnvironmentKind.value = target.environment");
    expect(applySource).toContain("restoreSelectedDrafts()");
    expect(applySource).toContain("const prepareError = await prepareStart()");
  });

  it("finishes pending dispatches explicitly on cancellation, failure, or successful start", () => {
    const stopStart = source.indexOf("async function stopPendingActionDispatch");
    const stopEnd = source.indexOf("\nasync function", stopStart + 1);
    const stopSource = source.slice(stopStart, stopEnd);
    const confirmStart = source.slice(
      source.indexOf("async function confirmStart"),
      source.indexOf("async function cancelRun"),
    );

    expect(stopSource).toContain('"tool:action-center:dispatch-cancel"');
    expect(stopSource).toContain("dispatchId");
    expect(stopSource).toContain("outcome");
    expect(source).toContain(':before-close="beforeCloseStartDialog"');
    expect(source).toContain('await stopPendingActionDispatch("cancelled")');
    expect(confirmStart).toMatch(/await stopPendingActionDispatch\(\s*"failed"/u);
    expect(confirmStart).toContain("pendingActionDispatchId.value = null");
  });

  it("passes the dispatch id only to a fresh package start", () => {
    const confirmStart = source.slice(
      source.indexOf("async function confirmStart"),
      source.indexOf("async function cancelRun"),
    );
    const retryStart = confirmStart.indexOf('"tool:release-package:upload-retry"');
    const normalStart = confirmStart.indexOf('"tool:release-package:start"');

    expect(confirmStart).toContain("actionDispatchId: pendingActionDispatchId.value ?? undefined");
    expect(retryStart).toBeGreaterThan(-1);
    expect(normalStart).toBeGreaterThan(retryStart);
    expect(confirmStart.slice(retryStart, normalStart)).not.toContain("actionDispatchId");
  });

  it("returns dialog reset failures to the pending dispatch flow", () => {
    const prepareStart = source.slice(
      source.indexOf("async function prepareStart"),
      source.indexOf("async function applyActionDispatchIntent"),
    );

    expect(prepareStart.indexOf("try {")).toBeLessThan(
      prepareStart.indexOf("await resetStartDialog()"),
    );
    expect(prepareStart).toContain(
      "return error instanceof Error ? error : new Error(String(error))",
    );
  });

  it("renders an explicit state when the saved Vault binding no longer exists", () => {
    expect(source).toContain('class="vault-binding-invalid"');
    expect(source).toContain("绑定的密码库凭据已失效，请重新选择");
  });

  it("renders a separate upload lane and explicit remote replacement confirmation", () => {
    expect(source).toContain("上传日志");
    expect(source).toMatch(
      /<section\s+v-if="environmentDraft\.packageType === 'server_upload'"\s+class="release-package-log-lane upload-log-lane"\s*>/,
    );
    expect(source).toContain(
      ":class=\"{ 'has-upload-lane': environmentDraft.packageType === 'server_upload' }\"",
    );
    expect(source).toMatch(
      /\.release-package-log-columns\.has-upload-lane\s*\{[^}]*grid-template-columns:\s*repeat\(2,\s*minmax\(0,\s*1fr\)\);/su,
    );
    expect(source).toMatch(
      /\.release-package-log-columns\.has-upload-lane\s+\.upload-log-lane\s*\{[^}]*grid-column:\s*1\s*\/\s*-1;[^}]*border-top:\s*1px\s+solid/su,
    );
    expect(source).toContain("uploadProgress");
    expect(source).toContain("完整替换以上远程目标");
    expect(source).toContain("package_succeeded_upload_failed");
    expect(source).toContain("重试上传");
  });

  it("configures independent build checks and post-upload commands", () => {
    for (const model of [
      "environmentDraft.frontendSuccessKeyword",
      "environmentDraft.backendSuccessKeyword",
      "environmentDraft.frontendPostUploadCommand",
      "environmentDraft.backendPostUploadCommand",
      "environmentDraft.postUploadCommandTimeoutSeconds",
    ]) {
      expect(source).toContain(`v-model="${model}"`);
    }
    expect(source).toContain("同时匹配 stdout 和 stderr，区分大小写；留空不检测。");
    expect(source).toContain("全部选中目标上传成功后执行；不自动注入 sudo、工作目录或路径变量。");
    expect(source).toContain("每条命令单独计时；超时会关闭 SSH 通道，远端进程状态标记为未知。");
    expect(source).toContain("upload_succeeded_command_failed");
    expect(source).toContain("仅重试失败命令");
    expect(source).toContain("重试上传后命令");
    expect(source).toContain("服务器文件已上传");
    expect(source).toContain("useReleasePackageCommandRetry");
    expect(source).toContain("await commandRetry.prepare(");
    expect(source).toContain("await commandRetry.trustHost(");
    expect(source).toContain("await commandRetry.preflight()");
    expect(source).toContain("runtime.beginStart(environmentId, commandTargets)");
    expect(source).toContain("await commandRetry.start(productionConfirmed.value)");
    expect(source).toContain("runtime.bindStartedRun(result.runId, environmentId)");
    expect(source).toContain('v-model="commandRetry.privateKeyPassphrase.value"');
    expect(source).toContain('@closed="resetCommandRetryDialog"');
  });

  it("configures deployment health checks after post-upload commands", () => {
    expect(source).toContain("部署后健康检查");
    expect(source).toMatch(
      /<div class="deployment-validation-row">[\s\S]*上传后命令最长运行时间（秒）[\s\S]*<div class="health-check-header">/u,
    );
    expect(source).toMatch(
      /\.deployment-validation-row\s*\{[^}]*grid-template-columns:\s*repeat\(2,\s*minmax\(0,\s*1fr\)\);/su,
    );
    expect(source).toContain('v-model="environmentDraft.healthCheckEnabled"');
    expect(source).toContain('v-model="environmentDraft.healthCheckUrl"');
    expect(source).toContain('v-model="environmentDraft.healthCheckMaxRetries"');
    expect(source).toContain("首次失败后每隔 10 秒重试");
    expect(source).toContain("deployed_health_check_failed");
  });

  it("mounts with mutually exclusive upload and command retry actions", async () => {
    const retryProject: ReleasePackageProject = {
      ...mountedProject,
      environments: mountedProject.environments.map((environment) =>
        environment.environment === "test"
          ? { ...environment, packageType: "server_upload" }
          : environment,
      ),
    };
    panelHarness.invoke.mockImplementation(async (channel: string) => {
      if (channel === "tool:release-package:project-list") return { projects: [retryProject] };
      if (channel === "tool:vault:meta-list") return [];
      return {};
    });
    const [{ default: ReleasePackagePanel }, { useReleasePackageRuntime }] = await Promise.all([
      import("./ReleasePackagePanel.vue"),
      import("../composables/useReleasePackageRuntime"),
    ]);
    const runtime = useReleasePackageRuntime();
    runtime.reset();
    runtime.beginStart(41, ["frontend", "backend"]);
    const renderer = createPanelRenderer();
    const root = hostNode("root");
    const app = renderer.createApp(ReleasePackagePanel);
    registerElementStubs(app);
    app.mount(root);
    await flushMountedPanel();

    const statusListener = panelHarness.listeners.get(APP_EVENTS.RELEASE_PACKAGE_STATUS);
    expect(statusListener).toBeTypeOf("function");
    statusListener?.({
      payload: {
        runId: "upload-run",
        environmentId: 41,
        projectId: mountedProject.id,
        environment: "test",
        status: "package_succeeded_upload_failed",
        phase: "overall",
        retryToken: "upload-retry",
      },
    });
    await nextTick();
    expect(buttonTexts(root)).toContain("重试上传");
    expect(buttonTexts(root)).not.toContain("仅重试失败命令");

    runtime.beginStart(41, ["frontend", "backend"]);
    statusListener?.({
      payload: {
        runId: "command-run",
        environmentId: 41,
        projectId: mountedProject.id,
        environment: "test",
        status: "upload_succeeded_command_failed",
        phase: "overall",
        error: "服务器文件已上传，但上传后命令未全部成功",
        commandRetryToken: "command-retry",
      },
    });
    await nextTick();
    expect(buttonTexts(root)).not.toContain("重试上传");
    expect(buttonTexts(root)).toContain("仅重试失败命令");
    app.unmount();
    runtime.reset();
  });
  it("renders mutually exclusive package types and type-specific fields", () => {
    expect(source).toContain('v-model="environmentDraft.packageType"');
    expect(source).toContain('value="local_archive"');
    expect(source).toContain('value="server_upload"');
    expect(source).toContain("environmentDraft.packageType === 'local_archive'");
    expect(source).toContain("environmentDraft.packageType === 'server_upload'");
    expect(source).not.toContain("environmentDraft.uploadEnabled");
    expect(source).not.toContain("startMode");
  });

  it("keeps conditional configuration inside stable layout sections", () => {
    expect(source).toMatch(
      /\.project-basics-grid\s*\{[^}]*grid-template-columns:\s*minmax\(240px,\s*320px\)\s+minmax\(0,\s*1fr\);/su,
    );
    expect(source).not.toMatch(/\.project-basics-grid\s*\{[^}]*auto-fit/su);

    expect(source).toContain('class="server-config-section server-auth-section"');
    expect(source).toContain('class="server-auth-details"');
    expect(source).toContain('class="private-key-config-grid"');
    expect(source).toContain('class="server-config-section server-target-section"');
    expect(source).toContain('class="server-target-grid"');

    const authDetailsStart = source.indexOf('class="server-auth-details"');
    const targetSectionStart = source.indexOf(
      'class="server-config-section server-target-section"',
    );
    expect(authDetailsStart).toBeGreaterThan(-1);
    expect(targetSectionStart).toBeGreaterThan(authDetailsStart);
    expect(source.slice(targetSectionStart)).toContain('label="前端远程目录"');
    expect(source.slice(targetSectionStart)).toContain('label="后端远程文件"');

    expect(source).toMatch(
      /\.private-key-config-grid\s*\{[^}]*grid-template-columns:\s*repeat\(3,\s*minmax\(0,\s*1fr\)\);/su,
    );
    expect(source).toMatch(
      /\.server-target-grid\s*\{[^}]*grid-template-columns:\s*repeat\(2,\s*minmax\(0,\s*1fr\)\);/su,
    );

    const tabletStyles = source.slice(source.indexOf("@media (max-width: 960px)"));
    expect(tabletStyles).toMatch(
      /\.private-key-config-grid\s*\{[^}]*grid-template-columns:\s*repeat\(2,\s*minmax\(0,\s*1fr\)\);/su,
    );
    const mobileStyles = source.slice(source.indexOf("@media (max-width: 640px)"));
    expect(mobileStyles).toMatch(
      /\.private-key-config-grid\s*\{[^}]*grid-template-columns:\s*1fr;/su,
    );
    expect(mobileStyles).toMatch(/\.server-target-grid\s*\{[^}]*grid-template-columns:\s*1fr;/su);
  });

  it("runs only the delivery checks required by the prepared package type", () => {
    const start = source.slice(source.indexOf("async function confirmStart"));
    expect(start).toContain("confirmArchiveOverwrite");
    expect(start).toContain("runUploadPreflight");
    expect(source).not.toContain("mode: startMode.value");
  });
});

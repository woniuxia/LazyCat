// @vitest-environment happy-dom
import { createApp, defineComponent, h, nextTick, type App } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useClipboardSuggestion } from "../composables/useClipboardSuggestion";
import JsonWorkbenchPanel from "./JsonWorkbenchPanel.vue";
import { workbenchTabState } from "./workbenchTabState";

const panelHarness = vi.hoisted(() => ({
  appliedInputs: [] as string[],
}));

vi.mock("./JsonProcessPanel.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "JsonProcessPanel",
      setup(_props, { expose }) {
        expose({
          applyExternalInput(text: string) {
            panelHarness.appliedInputs.push(text);
          },
        });
        return () => h("div");
      },
    }),
  };
});

vi.mock("./JsonSchemaPanel.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "JsonSchemaPanel",
      setup: () => () => h("div"),
    }),
  };
});

vi.mock("./JsonArrayFilterPanel.vue", async () => {
  const { defineComponent, h } = await import("vue");
  return {
    default: defineComponent({
      name: "JsonArrayFilterPanel",
      setup: () => () => h("div"),
    }),
  };
});

const mountedApps: App[] = [];

function mountWorkbench(): App {
  const passthrough = defineComponent({
    setup(_props, { slots }) {
      return () => h("div", slots.default?.());
    },
  });
  const root = document.createElement("div");
  document.body.appendChild(root);
  const app = createApp(JsonWorkbenchPanel);
  app.component("ElTabs", passthrough);
  app.component("ElTabPane", passthrough);
  app.mount(root);
  mountedApps.push(app);
  return app;
}

afterEach(() => {
  for (const app of mountedApps.splice(0)) app.unmount();
  document.body.innerHTML = "";
  panelHarness.appliedInputs = [];
  workbenchTabState.json = "process";
  useClipboardSuggestion().consumePendingToolInput("json-workbench");
});

describe("JsonWorkbenchPanel input routing", () => {
  it("consumes pending JSON on first open and switches to processing", async () => {
    workbenchTabState.json = "schema";
    useClipboardSuggestion().setPendingToolInput({
      toolId: "json-workbench",
      text: '{"first":true}',
    });

    mountWorkbench();
    await nextTick();
    await nextTick();

    expect(workbenchTabState.json).toBe("process");
    expect(panelHarness.appliedInputs).toEqual(['{"first":true}']);
  });

  it("applies JSON again while the workbench is already mounted", async () => {
    workbenchTabState.json = "schema";
    mountWorkbench();
    await nextTick();

    useClipboardSuggestion().setPendingToolInput({
      toolId: "json-workbench",
      text: '{"mounted":true}',
    });
    await nextTick();
    await nextTick();

    expect(workbenchTabState.json).toBe("process");
    expect(panelHarness.appliedInputs).toEqual(['{"mounted":true}']);
  });
});

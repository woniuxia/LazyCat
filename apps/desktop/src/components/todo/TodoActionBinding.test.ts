import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const editSource = readFileSync(new URL("./TodoDetailEdit.vue", import.meta.url), "utf-8");
const viewSource = readFileSync(new URL("./TodoDetailView.vue", import.meta.url), "utf-8");
const panelSource = readFileSync(new URL("./TodoPanel.vue", import.meta.url), "utf-8");

describe("Todo action binding UI wiring", () => {
  it("only renders action and package configuration fields for one-off drafts", () => {
    expect(editSource).toContain("draft.repeatPreset === 'none'");
    expect(editSource).toContain('label="执行动作"');
    expect(editSource).toContain('v-model="draft.actionType"');
    expect(editSource).toContain('label="打包配置"');
    expect(editSource).toContain('v-model="draft.actionTargetId"');
  });

  it("shows an empty package state and a route to the release package tool", () => {
    expect(editSource).toContain("暂无上线包配置");
    expect(editSource).toContain("前往上线包");
    expect(editSource).toContain("navigateToTool");
    expect(panelSource).toContain('openTab("release-package", "上线包打包")');
  });

  it("shows action status and dispatches from the detail view", () => {
    expect(viewSource).toContain("item.actionBinding");
    expect(viewSource).toContain("latestDispatch");
    expect(viewSource).toContain("最近状态");
    expect(viewSource).toContain("开始打包");
    expect(viewSource).toContain("dispatchAction");
  });

  it("connects the action composable to both edit and detail components", () => {
    expect(panelSource).toContain("useTodoActionBinding(itemDraft)");
    expect(panelSource).toContain(":action-definitions=\"actionDefinitions\"");
    expect(panelSource).toContain(":action-targets=\"actionTargets\"");
    expect(panelSource).toContain(":latest-dispatch=\"latestDispatch\"");
    expect(panelSource).toContain("dispatchTodoAction");
    expect(panelSource).toContain("loadLatestDispatch");
  });
});

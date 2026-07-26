import { describe, expect, it } from "vitest";

import type { ActionCombinationDraftStep, ActionCombinationTarget } from "../types/action-center";
import * as actionCombination from "./actionCombination";

import {
  createCombinationDraft,
  moveCombinationStep,
  toCombinationSaveInput,
} from "./actionCombination";

type ResolveStepTargets = (
  step: ActionCombinationDraftStep,
  liveTargets: ActionCombinationTarget[],
) => { options: ActionCombinationTarget[]; selected?: ActionCombinationTarget };

describe("action combination draft helpers", () => {
  it("normalizes a saved detail into an isolated editable draft", () => {
    const source = {
      id: 7,
      name: "开发环境",
      executionMode: "parallel" as const,
      steps: [
        {
          id: 11,
          actionType: "hosts.activate",
          targetId: "2",
          sortOrder: 0,
          targetLabel: "旧 Hosts 方案",
          available: false,
          unavailableReason: "目标配置已删除",
          createdAt: "2026-07-26 10:00:00",
          updatedAt: "2026-07-26 10:00:00",
        },
      ],
      createdAt: "2026-07-26 10:00:00",
      updatedAt: "2026-07-26 10:00:00",
    };

    const draft = createCombinationDraft(source);
    draft.steps[0].targetId = "3";

    expect(source.steps[0].targetId).toBe("2");
    expect(draft.steps[0]).toEqual(
      expect.objectContaining({
        targetLabel: "旧 Hosts 方案",
        available: false,
        unavailableReason: "目标配置已删除",
      }),
    );
    expect(toCombinationSaveInput(draft)).toEqual({
      id: 7,
      name: "开发环境",
      executionMode: "parallel",
      steps: [{ actionType: "hosts.activate", targetId: "3" }],
    });
  });

  it("keeps a missing selected target as a disabled snapshot option", () => {
    const resolveStepTargets = Reflect.get(actionCombination, "resolveCombinationStepTargets") as
      | ResolveStepTargets
      | undefined;
    const step: ActionCombinationDraftStep = {
      localId: "step-1",
      actionType: "hosts.activate",
      targetId: "missing-hosts",
      targetLabel: "已删除的 Hosts 方案",
      available: false,
      unavailableReason: "目标配置已删除",
    };
    expect(resolveStepTargets).toBeTypeOf("function");
    if (!resolveStepTargets) return;
    const state = resolveStepTargets(step, []);
    expect(state.options).toEqual([
      {
        id: "missing-hosts",
        label: "已删除的 Hosts 方案",
        available: false,
        unavailableReason: "目标配置已删除",
      },
    ]);
    expect(state.selected).toEqual(state.options[0]);
  });

  it("prefers a current live target over a stale detail snapshot", () => {
    const resolveStepTargets = Reflect.get(actionCombination, "resolveCombinationStepTargets") as
      | ResolveStepTargets
      | undefined;
    const step: ActionCombinationDraftStep = {
      localId: "step-1",
      actionType: "hosts.activate",
      targetId: "hosts-1",
      targetLabel: "旧名称",
      available: false,
      unavailableReason: "旧错误",
    };
    const liveTarget = { id: "hosts-1", label: "当前名称", available: true };
    expect(resolveStepTargets).toBeTypeOf("function");
    if (!resolveStepTargets) return;
    expect(resolveStepTargets(step, [liveTarget])).toEqual({
      options: [liveTarget],
      selected: liveTarget,
    });
  });

  it("moves steps without mutating the original array", () => {
    const source = [
      { localId: "a", actionType: "", targetId: "" },
      { localId: "b", actionType: "", targetId: "" },
      { localId: "c", actionType: "", targetId: "" },
    ];

    expect(moveCombinationStep(source, 2, 0).map((item) => item.localId)).toEqual(["c", "a", "b"]);
    expect(source.map((item) => item.localId)).toEqual(["a", "b", "c"]);
  });
});

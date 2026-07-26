import { describe, expect, it } from "vitest";

import {
  createCombinationDraft,
  moveCombinationStep,
  toCombinationSaveInput,
} from "./actionCombination";

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
    expect(toCombinationSaveInput(draft)).toEqual({
      id: 7,
      name: "开发环境",
      executionMode: "parallel",
      steps: [{ actionType: "hosts.activate", targetId: "3" }],
    });
  });

  it("moves steps without mutating the original array", () => {
    const source = [
      { localId: "a", actionType: "", targetId: "" },
      { localId: "b", actionType: "", targetId: "" },
      { localId: "c", actionType: "", targetId: "" },
    ];

    expect(moveCombinationStep(source, 2, 0).map((item) => item.localId)).toEqual([
      "c",
      "a",
      "b",
    ]);
    expect(source.map((item) => item.localId)).toEqual(["a", "b", "c"]);
  });
});

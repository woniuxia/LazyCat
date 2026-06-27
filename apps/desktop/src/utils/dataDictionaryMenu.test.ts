import { describe, expect, it } from "vitest";

import { dispatchDictionaryMenuCommand } from "./dataDictionaryMenu";

describe("dispatchDictionaryMenuCommand", () => {
  it("defers local overlay commands until after the dropdown click stack", () => {
    const calls: string[] = [];
    const scheduled: Array<() => void> = [];

    const handled = dispatchDictionaryMenuCommand(
      "replace",
      {
        replace: () => {
          calls.push("replace");
        },
        fields: () => {
          calls.push("fields");
        },
        rebuild: () => {
          calls.push("rebuild");
        },
        rename: () => {
          calls.push("rename");
        },
        remove: () => {
          calls.push("delete");
        },
      },
      (task) => scheduled.push(task),
    );

    expect(handled).toBe(true);
    expect(calls).toEqual([]);
    expect(scheduled).toHaveLength(1);

    scheduled[0]();
    expect(calls).toEqual(["replace"]);
  });

  it("runs message-box commands immediately", () => {
    const calls: string[] = [];
    const scheduled: Array<() => void> = [];

    const handled = dispatchDictionaryMenuCommand(
      "rename",
      {
        replace: () => {
          calls.push("replace");
        },
        fields: () => {
          calls.push("fields");
        },
        rebuild: () => {
          calls.push("rebuild");
        },
        rename: () => {
          calls.push("rename");
        },
        remove: () => {
          calls.push("delete");
        },
      },
      (task) => scheduled.push(task),
    );

    expect(handled).toBe(true);
    expect(calls).toEqual(["rename"]);
    expect(scheduled).toEqual([]);
  });

  it("runs rebuild immediately", () => {
    const calls: string[] = [];
    const scheduled: Array<() => void> = [];

    const handled = dispatchDictionaryMenuCommand(
      "rebuild",
      {
        replace: () => {
          calls.push("replace");
        },
        fields: () => {
          calls.push("fields");
        },
        rebuild: () => {
          calls.push("rebuild");
        },
        rename: () => {
          calls.push("rename");
        },
        remove: () => {
          calls.push("delete");
        },
      },
      (task) => scheduled.push(task),
    );

    expect(handled).toBe(true);
    expect(calls).toEqual(["rebuild"]);
    expect(scheduled).toEqual([]);
  });

  it("ignores unknown dropdown commands", () => {
    const calls: string[] = [];

    const handled = dispatchDictionaryMenuCommand("unknown", {
      replace: () => {
        calls.push("replace");
      },
      fields: () => {
        calls.push("fields");
      },
      rebuild: () => {
        calls.push("rebuild");
      },
      rename: () => {
        calls.push("rename");
      },
      remove: () => {
        calls.push("delete");
      },
    });

    expect(handled).toBe(false);
    expect(calls).toEqual([]);
  });
});

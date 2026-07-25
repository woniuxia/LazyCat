import { describe, expect, it } from "vitest";

import { normalizeTodoItem } from "./useTodoItem";

describe("normalizeTodoItem action binding", () => {
  it("normalizes an unavailable action binding without inventing a target", () => {
    const item = normalizeTodoItem({
      id: 1,
      kind: "one_off",
      actionBinding: {
        id: 9,
        actionType: "release_package.run",
        actionLabel: "开始打包",
        targetId: "404",
        targetLabel: "配置 #404",
        available: false,
        unavailableReason: "上线包配置不存在",
      },
    });

    expect(item.actionBinding?.available).toBe(false);
    expect(item.actionBinding?.targetId).toBe("404");
    expect(item.actionBinding?.targetLabel).toBe("配置 #404");
  });

  it("normalizes an empty action binding to null", () => {
    expect(normalizeTodoItem({ id: 1, actionBinding: null }).actionBinding).toBeNull();
  });
});

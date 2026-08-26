import { describe, expect, it } from "vitest";
import {
  appendFollowUpQuickInput,
  createDefaultFollowUpQuickInputs,
  parseFollowUpQuickInputs,
  sortFollowUpQuickInputs,
} from "./followUpQuickInputs";

describe("follow-up quick inputs", () => {
  it("creates editable default templates on first use", () => {
    expect(createDefaultFollowUpQuickInputs(100).map((item) => item.text)).toEqual([
      "暂无新进展",
      "正在处理中",
      "等待对方反馈",
      "已完成",
      "已取消",
    ]);
  });

  it("sorts by usage, recent use and creation order", () => {
    const items = createDefaultFollowUpQuickInputs(100);
    items[2]!.usageCount = 2;
    items[2]!.lastUsedAt = 200;
    items[3]!.usageCount = 2;
    items[3]!.lastUsedAt = 300;
    expect(sortFollowUpQuickInputs(items).map((item) => item.text)).toEqual([
      "已完成",
      "等待对方反馈",
      "暂无新进展",
      "正在处理中",
      "已取消",
    ]);
  });

  it("rejects malformed persisted templates instead of silently replacing them", () => {
    expect(() => parseFollowUpQuickInputs({ value: "not-json" })).toThrow(
      "快速输入配置不是有效的 JSON",
    );
  });

  it("parses wrapped settings values and appends without replacing input", () => {
    const items = createDefaultFollowUpQuickInputs(100);
    expect(parseFollowUpQuickInputs({ value: JSON.stringify(items) })).toEqual(items);
    expect(appendFollowUpQuickInput("已有内容", "等待反馈")).toBe("已有内容\n等待反馈");
    expect(appendFollowUpQuickInput("", "等待反馈")).toBe("等待反馈");
  });
});

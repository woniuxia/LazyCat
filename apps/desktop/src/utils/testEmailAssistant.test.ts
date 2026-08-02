import { describe, expect, it } from "vitest";
import {
  extractPlaceholders,
  getMissingPlaceholders,
  isMultilineFieldName,
  mergePlaceholders,
  renderEmailTemplate,
} from "./testEmailAssistant";

describe("test email assistant placeholder utilities", () => {
  it("normalizes whitespace and deduplicates placeholders in first-seen order", () => {
    expect(extractPlaceholders("{{ 名称 }} / {{名称}} / {{步骤}} / {{ 名称 }}")).toEqual([
      "名称",
      "步骤",
    ]);
    expect(mergePlaceholders("{{Word}} {{共享}}", "{{共享}} {{邮件}} {{Word}}")).toEqual([
      "Word",
      "共享",
      "邮件",
    ]);
  });

  it("rejects empty, multiline, and nested candidates", () => {
    expect(extractPlaceholders("{{}} {{  }} {{跨\n行}} {{外{{内}}}} {{ok}}")).toEqual(["ok"]);
  });

  it("reports only blank required values", () => {
    expect(getMissingPlaceholders(["称呼", "内容", "步骤"], { 称呼: "张三", 内容: " ", 步骤: "完成" })).toEqual([
      "内容",
    ]);
  });

  it("recognizes semantic fields that need multiline input", () => {
    expect(isMultilineFieldName("功能需求内容")).toBe(true);
    expect(isMultilineFieldName("测试步骤")).toBe(true);
    expect(isMultilineFieldName("补充说明")).toBe(true);
    expect(isMultilineFieldName("字段描述")).toBe(true);
    expect(isMultilineFieldName("备注信息")).toBe(true);
    expect(isMultilineFieldName("应用系统名称")).toBe(false);
  });

  it("renders values including special characters and multiline text", () => {
    expect(renderEmailTemplate("{{称呼}}：{{内容}}", { 称呼: "张三", 内容: "A & <B>\nC" })).toBe(
      "张三：A & <B>\nC",
    );
  });
});

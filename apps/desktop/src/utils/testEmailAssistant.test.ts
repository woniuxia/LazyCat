import { describe, expect, it } from "vitest";
import {
  BUILTIN_TEST_EMAIL_TEMPLATE_ID,
  BUILTIN_TEST_EMAIL_TEMPLATE_NAME,
  extractPlaceholders,
  getMissingPlaceholders,
  hasTestEmailTemplateNameConflict,
  isMultilineFieldName,
  mergePlaceholders,
  normalizeTestEmailBodyTemplates,
  renderEmailTemplate,
} from "./testEmailAssistant";

describe("test email assistant body template utilities", () => {
  it("filters invalid roots and entries", () => {
    expect(normalizeTestEmailBodyTemplates(null)).toEqual([]);
    expect(normalizeTestEmailBodyTemplates({})).toEqual([]);
    expect(
      normalizeTestEmailBodyTemplates([
        null,
        [],
        new Date(),
        { id: 1, name: "模板", content: "正文" },
        { id: "", name: "模板", content: "正文" },
        { id: "id-1", name: " ", content: "正文" },
        { id: "id-2", name: "模板", content: " \n " },
        { id: BUILTIN_TEST_EMAIL_TEMPLATE_ID, name: "伪默认模板", content: "默认正文" },
        { id: "id-3", name: "有效模板", content: "正文" },
      ]),
    ).toEqual([
      {
        id: BUILTIN_TEST_EMAIL_TEMPLATE_ID,
        name: BUILTIN_TEST_EMAIL_TEMPLATE_NAME,
        content: "默认正文",
      },
      { id: "id-3", name: "有效模板", content: "正文" },
    ]);
  });

  it("trims names while preserving content exactly", () => {
    expect(
      normalizeTestEmailBodyTemplates([
        { id: "id-1", name: "  发布通知  ", content: "  第一行\n第二行  " },
      ]),
    ).toEqual([{ id: "id-1", name: "发布通知", content: "  第一行\n第二行  " }]);
  });

  it("deduplicates ids and case-insensitive trimmed names in first-seen order", () => {
    expect(
      normalizeTestEmailBodyTemplates([
        { id: "id-1", name: "模板 A", content: "正文 1" },
        { id: "id-1", name: "模板 B", content: "重复 id" },
        { id: "id-2", name: "  模板 a ", content: "重复名称" },
        { id: "id-3", name: "模板 C", content: "正文 3" },
      ]),
    ).toEqual([
      { id: "id-1", name: "模板 A", content: "正文 1" },
      { id: "id-3", name: "模板 C", content: "正文 3" },
    ]);
  });

  it("detects normalized name conflicts and can exclude the current template", () => {
    const templates = [
      { id: "id-1", name: "发布通知", content: "正文 1" },
      { id: "id-2", name: "回归测试", content: "正文 2" },
    ];

    expect(hasTestEmailTemplateNameConflict(templates, "  发布通知  ")).toBe(true);
    expect(hasTestEmailTemplateNameConflict(templates, "回归测试", "id-2")).toBe(false);
    expect(hasTestEmailTemplateNameConflict(templates, "新模板")).toBe(false);
  });
});

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
    expect(
      getMissingPlaceholders(["称呼", "内容", "步骤"], { 称呼: "张三", 内容: " ", 步骤: "完成" }),
    ).toEqual(["内容"]);
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

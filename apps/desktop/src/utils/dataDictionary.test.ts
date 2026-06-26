import { describe, expect, it } from "vitest";
import {
  buildMatchLabels,
  buildResultSummary,
  dictionarySourceLabel,
  formatJsonValue,
} from "./dataDictionary";
import type { DataDictionaryField, DataDictionarySearchItem } from "../types/data-dictionary";

const fields: DataDictionaryField[] = [
  {
    fieldPath: "id",
    displayName: "编号",
    meaning: "",
    searchable: true,
    visible: true,
    sortOrder: 0,
    typeHint: "number",
    sampleValue: "1001",
    presentCount: 2,
  },
  {
    fieldPath: "user.name",
    displayName: "姓名",
    meaning: "用户真实姓名",
    searchable: true,
    visible: true,
    sortOrder: 1,
    typeHint: "string",
    sampleValue: "张三",
    presentCount: 2,
  },
  {
    fieldPath: "secret",
    displayName: "内部字段",
    meaning: "",
    searchable: false,
    visible: false,
    sortOrder: 2,
    typeHint: "string",
    sampleValue: "hidden",
    presentCount: 2,
  },
];

const item: DataDictionarySearchItem = {
  id: 10,
  dictionaryId: 2,
  dictionaryName: "用户字典",
  rowIndex: 0,
  rawJson: {
    id: 1001,
    user: { name: "张三" },
    secret: "hidden",
  },
  matches: [{ fieldPath: "user.name", value: "张三" }],
};

describe("dataDictionary utils", () => {
  it("uses display names before meanings for visible summary labels", () => {
    expect(buildResultSummary(item, fields)).toEqual([
      { fieldPath: "id", label: "编号", value: "1001" },
      { fieldPath: "user.name", label: "姓名", value: "张三" },
    ]);
  });

  it("skips visible summary fields that are missing from the record", () => {
    expect(
      buildResultSummary(item, [
        ...fields,
        {
          fieldPath: "user.email",
          displayName: "邮箱",
          meaning: "",
          searchable: true,
          visible: true,
          sortOrder: 3,
          typeHint: "string",
          sampleValue: "zhangsan@example.com",
          presentCount: 1,
        },
      ]),
    ).toEqual([
      { fieldPath: "id", label: "编号", value: "1001" },
      { fieldPath: "user.name", label: "姓名", value: "张三" },
    ]);
  });

  it("keeps valid falsy summary values", () => {
    expect(
      buildResultSummary(
        {
          ...item,
          rawJson: {
            count: 0,
            enabled: false,
            note: null,
          },
        },
        [
          {
            fieldPath: "count",
            displayName: "数量",
            meaning: "",
            searchable: true,
            visible: true,
            sortOrder: 0,
            typeHint: "number",
            sampleValue: "0",
            presentCount: 1,
          },
          {
            fieldPath: "enabled",
            displayName: "启用",
            meaning: "",
            searchable: true,
            visible: true,
            sortOrder: 1,
            typeHint: "boolean",
            sampleValue: "false",
            presentCount: 1,
          },
          {
            fieldPath: "note",
            displayName: "备注",
            meaning: "",
            searchable: true,
            visible: true,
            sortOrder: 2,
            typeHint: "null",
            sampleValue: "null",
            presentCount: 1,
          },
        ],
      ),
    ).toEqual([
      { fieldPath: "count", label: "数量", value: "0" },
      { fieldPath: "enabled", label: "启用", value: "false" },
      { fieldPath: "note", label: "备注", value: "null" },
    ]);
  });

  it("labels global search result source", () => {
    expect(dictionarySourceLabel(item)).toBe("用户字典 #1");
  });

  it("uses field display names for match labels", () => {
    expect(buildMatchLabels(item.matches, fields)).toEqual([
      { fieldPath: "user.name", label: "姓名", value: "张三" },
    ]);
  });

  it("formats nested json values compactly", () => {
    expect(formatJsonValue(["A", "B"])).toBe("[\"A\",\"B\"]");
    expect(formatJsonValue(null)).toBe("null");
  });
});

import { describe, expect, it } from "vitest";
import {
  buildResultTitle,
  buildMatchLabels,
  buildResultSummary,
  dictionarySourceLabel,
  formatJsonValue,
  mergePopularAndSearchItems,
  moveDataDictionaryFieldDraft,
  orderDataDictionaryFieldDrafts,
  pickInitialRecordItem,
  setDataDictionaryFieldVisibility,
  splitDataDictionaryFieldDrafts,
} from "./dataDictionary";
import type {
  DataDictionaryField,
  DataDictionaryPopularRecord,
  DataDictionarySearchItem,
} from "../types/data-dictionary";

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
  titleFieldPath: null,
  title: "用户字典 #1",
  summary: [],
};

function searchItem(id: number, title: string): DataDictionarySearchItem {
  return {
    id,
    dictionaryId: 1,
    dictionaryName: "Users",
    titleFieldPath: "name",
    rowIndex: id,
    matches: [],
    title,
    summary: [],
  };
}

function popularItem(id: number, title: string): DataDictionaryPopularRecord {
  return {
    ...searchItem(id, title),
    recordId: `u${id}`,
    normalizedValue: `u${id}`,
    usedCount: 3,
    lastUsedAt: "2026-06-28 10:00:00",
  };
}

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

  it("includes every visible summary field by default", () => {
    const visibleFields = Array.from({ length: 5 }, (_, index) => ({
      fieldPath: `field${index + 1}`,
      displayName: `字段${index + 1}`,
      meaning: "",
      searchable: true,
      visible: true,
      sortOrder: index,
      typeHint: "string",
      sampleValue: `value${index + 1}`,
      presentCount: 1,
    }));

    expect(
      buildResultSummary(
        {
          ...item,
          rawJson: {
            field1: "value1",
            field2: "value2",
            field3: "value3",
            field4: "value4",
            field5: "value5",
          },
        },
        visibleFields,
      ).map((part) => part.fieldPath),
    ).toEqual(["field1", "field2", "field3", "field4", "field5"]);
  });

  it("labels global search result source", () => {
    expect(dictionarySourceLabel(item)).toBe("用户字典 #1");
  });

  it("uses field display names for match labels", () => {
    expect(buildMatchLabels(item.matches, fields)).toEqual([
      { fieldPath: "user.name", label: "姓名", value: "张三" },
    ]);
  });

  it("uses the configured title field as the result heading", () => {
    expect(buildResultTitle({ ...item, titleFieldPath: "user.name" })).toBe("张三");
  });

  it("falls back to dictionary source when the configured title field is missing", () => {
    expect(buildResultTitle({ ...item, titleFieldPath: "user.email" })).toBe("用户字典 #1");
  });

  it("can omit the title field from visible summary parts", () => {
    expect(
      buildResultSummary({ ...item, titleFieldPath: "user.name" }, fields, "user.name"),
    ).toEqual([{ fieldPath: "id", label: "编号", value: "1001" }]);
  });

  it("formats nested json values compactly", () => {
    expect(formatJsonValue(["A", "B"])).toBe('["A","B"]');
    expect(formatJsonValue(null)).toBe("null");
  });

  it("orders visible field drafts before hidden drafts when no explicit order exists", () => {
    expect(
      orderDataDictionaryFieldDrafts([
        { ...fields[2], visible: false, sortOrder: 0 },
        { ...fields[1], visible: true, sortOrder: 0 },
        { ...fields[0], visible: true, sortOrder: 0 },
      ]).map((field) => [field.fieldPath, field.sortOrder]),
    ).toEqual([
      ["id", 0],
      ["user.name", 1],
      ["secret", 2],
    ]);
  });

  it("groups visible field drafts before hidden drafts even when persisted order is interleaved", () => {
    expect(
      orderDataDictionaryFieldDrafts([
        { ...fields[2], visible: false, sortOrder: 0 },
        { ...fields[0], visible: true, sortOrder: 1 },
        { ...fields[1], visible: true, sortOrder: 2 },
      ]).map((field) => [field.fieldPath, field.sortOrder]),
    ).toEqual([
      ["id", 0],
      ["user.name", 1],
      ["secret", 2],
    ]);
  });

  it("splits field drafts into visible and hidden lists after ordering", () => {
    const grouped = splitDataDictionaryFieldDrafts([
      { ...fields[2], visible: false, sortOrder: 0 },
      { ...fields[1], visible: true, sortOrder: 2 },
      { ...fields[0], visible: true, sortOrder: 1 },
    ]);

    expect(grouped.visibleFields.map((field) => [field.fieldPath, field.sortOrder])).toEqual([
      ["id", 0],
      ["user.name", 1],
    ]);
    expect(grouped.hiddenFields.map((field) => [field.fieldPath, field.sortOrder])).toEqual([
      ["secret", 2],
    ]);
  });

  it("reindexes visible field drafts after manual drag sorting without moving hidden drafts", () => {
    expect(
      moveDataDictionaryFieldDraft(
        [
          { ...fields[2], visible: false, sortOrder: 0 },
          { ...fields[0], visible: true, sortOrder: 1 },
          { ...fields[1], visible: true, sortOrder: 2 },
        ],
        1,
        0,
      ).map((field) => [field.fieldPath, field.sortOrder]),
    ).toEqual([
      ["user.name", 0],
      ["id", 1],
      ["secret", 2],
    ]);
  });

  it("moves fields between visible and hidden lists when visibility changes", () => {
    expect(
      setDataDictionaryFieldVisibility(fields, "secret", true).map((field) => [
        field.fieldPath,
        field.visible,
        field.sortOrder,
      ]),
    ).toEqual([
      ["id", true, 0],
      ["user.name", true, 1],
      ["secret", true, 2],
    ]);

    expect(
      setDataDictionaryFieldVisibility(fields, "id", false).map((field) => [
        field.fieldPath,
        field.visible,
        field.sortOrder,
      ]),
    ).toEqual([
      ["user.name", true, 0],
      ["secret", false, 1],
      ["id", false, 2],
    ]);
  });

  it("keeps popular records first and removes duplicate search items", () => {
    const result = mergePopularAndSearchItems(
      [popularItem(1, "Alice")],
      [searchItem(1, "Alice"), searchItem(2, "Bob")],
    );

    expect(result.map((entry) => entry.id)).toEqual([1, 2]);
  });

  it("picks first popular record before default search result", () => {
    const picked = pickInitialRecordItem([popularItem(1, "Alice")], [searchItem(2, "Bob")]);

    expect(picked?.id).toBe(1);
  });

  it("picks first search result when popular records are empty", () => {
    const picked = pickInitialRecordItem([], [searchItem(2, "Bob")]);

    expect(picked?.id).toBe(2);
  });
});

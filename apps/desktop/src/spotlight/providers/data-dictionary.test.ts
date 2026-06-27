import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeToolByChannel = vi.fn();
const invoke = vi.fn();
const writeText = vi.fn();

vi.mock("../../bridge/tauri", () => ({
  invokeToolByChannel: (...args: unknown[]) => invokeToolByChannel(...args),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import {
  buildDataDictionaryActions,
  buildDataDictionaryItem,
  dataDictionaryProvider,
} from "./data-dictionary";
import type { DataDictionarySearchItem } from "../../types/data-dictionary";

const searchItem: DataDictionarySearchItem = {
  id: 12,
  dictionaryId: 3,
  dictionaryName: "用户字典",
  titleFieldPath: "name",
  rowIndex: 0,
  matches: [{ fieldPath: "name", value: "张三" }],
  title: "张三",
  summary: [
    { fieldPath: "id", label: "编号", value: "1001" },
    { fieldPath: "dept", label: "部门", value: "研发" },
  ],
};

beforeEach(() => {
  invokeToolByChannel.mockReset();
  invoke.mockReset();
  writeText.mockReset();
  writeText.mockResolvedValue(undefined);
  vi.stubGlobal("navigator", {
    clipboard: {
      writeText,
    },
  });
});

describe("buildDataDictionaryItem", () => {
  it("maps backend search item to Spotlight item without rawJson payload", () => {
    const item = buildDataDictionaryItem({
      ...searchItem,
      rawJson: { id: 1001, name: "张三" },
    });

    expect(item.providerId).toBe("data-dictionary");
    expect(item.itemId).toBe("12");
    expect(item.title).toBe("张三");
    expect(item.subtitle).toBe("用户字典 · 编号：1001 · 部门：研发");
    expect(item.status).toEqual({ text: "2 字段", tone: "muted" });
    expect(item.payload?.recordId).toBe(12);
    expect(item.payload?.dictionaryId).toBe(3);
    expect(item.payload?.rawJson).toBeUndefined();
    expect(item.searchFields.map((field) => field.text)).toContain("张三");
    expect(item.searchFields.map((field) => field.text)).toContain("用户字典");
    expect(item.searchFields.map((field) => field.text)).toContain("编号 1001");
  });

  it("omits status when no summary fields exist", () => {
    const item = buildDataDictionaryItem({ ...searchItem, summary: [] });
    expect(item.status).toBeUndefined();
  });
});

describe("dataDictionaryProvider search", () => {
  it("requests all dictionaries with includeRawJson disabled", async () => {
    invokeToolByChannel.mockResolvedValue({ items: [searchItem] });

    const results = await dataDictionaryProvider.search?.("张", {
      scope: "data-dictionary",
    });

    expect(invokeToolByChannel).toHaveBeenCalledWith("tool:data-dictionary:search", {
      scope: "all",
      keyword: "张",
      limit: 50,
      includeRawJson: false,
    });
    expect(results?.[0].title).toBe("张三");
  });

  it("returns empty array for empty query and provider failures", async () => {
    expect(await dataDictionaryProvider.search?.("", { scope: null })).toEqual([]);
    invokeToolByChannel.mockRejectedValue(new Error("boom"));
    expect(await dataDictionaryProvider.search?.("张三", { scope: null })).toEqual([]);
  });
});

describe("data dictionary actions", () => {
  it("builds copy actions for visible fields plus full JSON", () => {
    const actions = buildDataDictionaryActions(buildDataDictionaryItem(searchItem));
    expect(actions.map((action) => action.id)).toEqual([
      "copy_field:0",
      "copy_field:1",
      "copy_raw_json",
    ]);
  });

  it("copies a summary field", async () => {
    const result = await dataDictionaryProvider.executeAction?.(
      buildDataDictionaryItem(searchItem),
      "copy_field:1",
      {} as never,
    );

    expect(writeText).toHaveBeenCalledWith("研发");
    expect(result).toEqual({
      closeSpotlight: true,
      toast: { message: "字段值已复制", type: "success" },
    });
  });

  it("loads record detail lazily before copying full JSON", async () => {
    invokeToolByChannel.mockResolvedValue({
      record: {
        id: 12,
        dictionaryId: 3,
        dictionaryName: "用户字典",
        title: "张三",
        rowIndex: 0,
        summary: [],
        rawJson: { id: 1001, name: "张三" },
      },
      fields: [],
      forwardRelations: [],
      reverseRelations: [],
    });

    const result = await dataDictionaryProvider.executeAction?.(
      buildDataDictionaryItem(searchItem),
      "copy_raw_json",
      {} as never,
    );

    expect(invokeToolByChannel).toHaveBeenCalledWith(
      "tool:data-dictionary:record-detail",
      { recordId: 12 },
    );
    expect(writeText).toHaveBeenCalledWith(
      JSON.stringify({ id: 1001, name: "张三" }, null, 2),
    );
    expect(result?.closeSpotlight).toBe(true);
  });
});

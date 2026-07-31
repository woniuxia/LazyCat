import { describe, expect, it } from "vitest";
import type { DataDictionaryRelationDraft, DataDictionarySummary } from "../types/data-dictionary";
import {
  duplicateRelationKeys,
  relationTargetPrimaryLabel,
  toRelationDrafts,
} from "./dataDictionaryRelations";

const dictionaries: DataDictionarySummary[] = [
  {
    id: 1,
    name: "人员",
    description: "",
    recordCount: 2,
    primaryFieldPath: "employeeNo",
    titleFieldPath: "name",
    sortFieldPath: null,
    sortDirection: "asc",
    navOrder: 0,
    createdAt: "",
    updatedAt: "",
  },
  {
    id: 2,
    name: "部门",
    description: "",
    recordCount: 1,
    primaryFieldPath: "id",
    titleFieldPath: "name",
    sortFieldPath: null,
    sortDirection: "asc",
    navOrder: 1,
    createdAt: "",
    updatedAt: "",
  },
  {
    id: 3,
    name: "岗位",
    description: "",
    recordCount: 1,
    primaryFieldPath: null,
    titleFieldPath: "name",
    sortFieldPath: null,
    sortDirection: "asc",
    navOrder: 2,
    createdAt: "",
    updatedAt: "",
  },
];

describe("dataDictionaryRelations", () => {
  it("detects duplicate source field and target dictionary pairs", () => {
    const drafts: DataDictionaryRelationDraft[] = [
      {
        sourceFieldPath: "deptId",
        targetDictionaryId: 2,
        relationName: "所属部门",
        reverseName: "部门人员",
      },
      {
        sourceFieldPath: "deptId",
        targetDictionaryId: 2,
        relationName: "部门",
        reverseName: "人员",
      },
      {
        sourceFieldPath: "positionId",
        targetDictionaryId: 2,
        relationName: "岗位",
        reverseName: "岗位人员",
      },
    ];

    expect(duplicateRelationKeys(drafts)).toEqual(["deptId::2"]);
  });

  it("formats target primary state for relation rows", () => {
    expect(relationTargetPrimaryLabel(2, dictionaries)).toBe("目标主键：id");
    expect(relationTargetPrimaryLabel(3, dictionaries)).toBe("目标字典未配置主键");
    expect(relationTargetPrimaryLabel(null, dictionaries)).toBe("请选择目标字典");
  });

  it("converts saved relations to editable drafts", () => {
    expect(
      toRelationDrafts([
        {
          id: 9,
          sourceDictionaryId: 1,
          sourceFieldPath: "deptId",
          targetDictionaryId: 2,
          targetDictionaryName: "部门",
          targetPrimaryFieldPath: "id",
          relationName: "所属部门",
          reverseName: "部门人员",
        },
      ]),
    ).toEqual([
      {
        sourceFieldPath: "deptId",
        targetDictionaryId: 2,
        relationName: "所属部门",
        reverseName: "部门人员",
      },
    ]);
  });
});

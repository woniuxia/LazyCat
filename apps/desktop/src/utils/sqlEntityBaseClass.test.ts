import { describe, expect, it } from "vitest";
import {
  parseBaseClassFields,
  reconcileBaseClassSelection,
  validateJavaQualifiedName,
} from "./sqlEntityBaseClass";

describe("sqlEntityBaseClass", () => {
  it("按逗号和换行拆分字段并保持首次出现顺序", () => {
    expect(parseBaseClassFields("id, createdAt\nupdatedAt, id")).toEqual([
      "id",
      "createdAt",
      "updatedAt",
    ]);
  });

  it("拒绝非法 Java 完整类名和字段名", () => {
    expect(validateJavaQualifiedName("com.example.BaseEntity")).toBe("");
    expect(validateJavaQualifiedName("com.example.1Base")).toBe(
      "完整类名包含非法 Java 标识符：1Base",
    );
    expect(() => parseBaseClassFields("created-at")).toThrow("非法 Java 字段名：created-at");
  });

  it("单选时自动设为父类，移除父类时回退到第一项", () => {
    expect(reconcileBaseClassSelection([2], null, [1, 2, 3])).toEqual({
      selectedIds: [2],
      parentId: 2,
    });
    expect(reconcileBaseClassSelection([1, 3], 2, [1, 3])).toEqual({
      selectedIds: [1, 3],
      parentId: 1,
    });
  });

  it("清理已经删除的基类选择", () => {
    expect(reconcileBaseClassSelection([1, 2], 2, [1])).toEqual({
      selectedIds: [1],
      parentId: 1,
    });
  });
});

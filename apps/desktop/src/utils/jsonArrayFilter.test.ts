import { describe, expect, it } from "vitest";
import {
  collectArrayProperties,
  findFirstObjectArray,
  projectObjectArray,
} from "./jsonArrayFilter";

describe("jsonArrayFilter", () => {
  it("selects a usable root object array first, including an empty array", () => {
    const empty = findFirstObjectArray([]);
    const root = findFirstObjectArray([{ id: 1 }]);

    expect(empty).toEqual({ path: "$", value: [] });
    expect(root).toEqual({ path: "$", value: [{ id: 1 }] });
  });

  it("finds the first usable array in depth-first document order", () => {
    const result = findFirstObjectArray({
      unsupported: [1, { nested: [{ id: "first" }] }],
      later: [{ id: "later" }],
    });

    expect(result).toEqual({
      path: "$.unsupported[1].nested",
      value: [{ id: "first" }],
    });
  });

  it("skips primitive, null, nested, and mixed arrays", () => {
    const result = findFirstObjectArray({
      primitives: [1, 2],
      nulls: [null],
      nested: [[1]],
      mixed: [{ id: 2 }, "nope"],
      usable: [{ id: 3 }],
    });

    expect(result?.path).toBe("$.usable");
    expect(findFirstObjectArray({ values: [1, null, [1], { id: 4 }] })).toBeNull();
  });

  it("collects top-level properties in first-seen order", () => {
    expect(
      collectArrayProperties([
        { id: 1, name: "first" },
        { name: "second", nested: { ok: true } },
        { id: 3, active: true, nested: [] },
      ]),
    ).toEqual(["id", "name", "nested", "active"]);
  });

  it("projects selected fields without mutating records or nested values", () => {
    const nested = { role: "admin" };
    const source = [
      { id: 1, name: "first", nested },
      { id: 2, name: "second", active: true },
    ];
    const snapshot = JSON.parse(JSON.stringify(source));

    const result = projectObjectArray(source, new Set(["nested", "active", "missing"]));

    expect(result).toEqual([{ nested }, { active: true }]);
    expect(result[0]).toEqual({ nested });
    expect(Object.keys(result[0])).toEqual(["nested"]);
    expect(source).toEqual(snapshot);
  });

  it("preserves each record's original key order and supports empty selections", () => {
    const source = [
      { z: 1, a: 2 },
      { a: 3, z: 4 },
    ];

    expect(projectObjectArray(source, new Set(["a"]))).toEqual([{ a: 2 }, { a: 3 }]);
    expect(Object.keys(projectObjectArray(source, new Set(["a"]))[0])).toEqual(["a"]);
    expect(projectObjectArray(source, new Set())).toEqual([{}, {}]);
    expect(Object.keys(projectObjectArray(source, new Set(["z", "a"]))[0])).toEqual(["z", "a"]);
  });

  it("keeps special JSON object keys as own enumerable fields", () => {
    const source = JSON.parse('[{"__proto__":{"ok":true},"constructor":1}]') as Array<
      Record<string, unknown>
    >;

    const result = projectObjectArray(source, new Set(["__proto__", "constructor"]));

    expect(Object.keys(result[0])).toEqual(["__proto__", "constructor"]);
    expect(result[0]["__proto__"]).toEqual({ ok: true });
    expect(JSON.stringify(result)).toBe('[{"__proto__":{"ok":true},"constructor":1}]');
  });
});

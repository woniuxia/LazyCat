import { describe, expect, it } from "vitest";
import { stringifyJsonWithSortedKeys } from "./jsonProcess";

describe("stringifyJsonWithSortedKeys", () => {
  it("sorts object fields recursively", () => {
    const value = {
      zebra: 1,
      alpha: { second: 2, first: 1 },
    };

    expect(stringifyJsonWithSortedKeys(value)).toBe(`{
  "alpha": {
    "first": 1,
    "second": 2
  },
  "zebra": 1
}`);
  });

  it("preserves array order while sorting fields in array items", () => {
    const value = [
      { b: 2, a: 1 },
      { d: 4, c: 3 },
    ];

    expect(JSON.parse(stringifyJsonWithSortedKeys(value))).toEqual([
      { a: 1, b: 2 },
      { c: 3, d: 4 },
    ]);
  });

  it("sorts numeric-looking field names lexicographically", () => {
    expect(stringifyJsonWithSortedKeys({ "2": "two", "10": "ten" })).toBe(`{
  "10": "ten",
  "2": "two"
}`);
  });

  it("serializes primitive JSON values unchanged", () => {
    expect(stringifyJsonWithSortedKeys(null)).toBe("null");
    expect(stringifyJsonWithSortedKeys("text")).toBe('"text"');
    expect(stringifyJsonWithSortedKeys(42)).toBe("42");
  });
});

import { describe, expect, it } from "vitest";
import {
  formatJsonDocument,
  parseJsonErrorLocation,
  pointerLastToken,
  rootCombination,
} from "./jsonSchema";

describe("jsonSchema utils", () => {
  it("formats valid JSON and rejects invalid JSON", () => {
    expect(formatJsonDocument('{"name":"lazycat"}')).toBe('{\n  "name": "lazycat"\n}');
    expect(() => formatJsonDocument("{")).toThrow();
  });

  it("detects root oneOf and anyOf branches", () => {
    expect(rootCombination('{"oneOf":[{},{}]}')).toEqual({ keyword: "oneOf", count: 2 });
    expect(rootCombination('{"anyOf":[{}]}')).toEqual({ keyword: "anyOf", count: 1 });
    expect(rootCombination("invalid")).toBeNull();
  });

  it("extracts backend JSON locations", () => {
    expect(parseJsonErrorLocation("错误（第 3 行，第 8 列）")).toEqual({ line: 3, column: 8 });
    expect(parseJsonErrorLocation("at line 2 column 7")).toEqual({ line: 2, column: 7 });
  });

  it("decodes the final JSON pointer token", () => {
    expect(pointerLastToken("/profile/user~1name")).toBe("user/name");
    expect(pointerLastToken("/a~0b")).toBe("a~b");
  });
});

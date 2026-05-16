import { describe, expect, it } from "vitest";
import {
  calculateExpression,
  formatCalcResult,
  getCalcPreview,
  normalizeCalcExpression,
} from "./calc";

describe("normalizeCalcExpression", () => {
  it("strips thousand separators", () => {
    expect(normalizeCalcExpression("1,000+2,000")).toBe("1000+2000");
    expect(normalizeCalcExpression("1，000+2，000")).toBe("1000+2000");
  });

  it("converts full-width and chinese operators", () => {
    expect(normalizeCalcExpression("3×4")).toBe("3*4");
    expect(normalizeCalcExpression("10÷2")).toBe("10/2");
    expect(normalizeCalcExpression("\uFF081+2\uFF09*3")).toBe("(1+2)*3");
  });

  it("expands percent literals", () => {
    expect(normalizeCalcExpression("23.7%*100")).toBe("(23.7/100)*100");
    expect(normalizeCalcExpression("50%")).toBe("(50/100)");
  });

  it("trims whitespace", () => {
    expect(normalizeCalcExpression("1 + 2 * 3")).toBe("1+2*3");
  });
});

describe("calculateExpression", () => {
  it("computes basic arithmetic", () => {
    expect(calculateExpression("1+2*3").rawValue).toBe("7");
    expect(calculateExpression("(1+2)*3").rawValue).toBe("9");
  });

  it("formats the display value", () => {
    expect(calculateExpression("1000000+1").displayValue).toBe("1,000,001");
  });

  it("rejects empty input", () => {
    expect(() => calculateExpression("")).toThrow("请输入计算公式");
    expect(() => calculateExpression("   ")).toThrow("请输入计算公式");
  });

  it("rejects disallowed characters", () => {
    expect(() => calculateExpression("alert(1)")).toThrow(/仅支持/);
    expect(() => calculateExpression("1+a")).toThrow(/仅支持/);
  });

  it("rejects malformed formulas", () => {
    expect(() => calculateExpression("1+")).toThrow("公式格式不正确");
    expect(() => calculateExpression("(1+2")).toThrow("公式格式不正确");
  });

  it("rejects non-finite results", () => {
    expect(() => calculateExpression("1/0")).toThrow("计算结果无效");
  });
});

describe("getCalcPreview", () => {
  it("returns empty string for blank input", () => {
    expect(getCalcPreview("")).toBe("");
    expect(getCalcPreview("   ")).toBe("");
  });

  it("returns the display value when expression is complete", () => {
    expect(getCalcPreview("1+2")).toBe("3");
  });

  it("falls back by trimming trailing operators", () => {
    expect(getCalcPreview("1+2+")).toBe("3");
    expect(getCalcPreview("1+2*")).toBe("3");
  });

  it("returns empty string when fallback cannot recover", () => {
    expect(getCalcPreview("+")).toBe("");
  });
});

describe("formatCalcResult", () => {
  it("uses zh-CN locale grouping", () => {
    expect(formatCalcResult(1234567)).toBe("1,234,567");
    expect(formatCalcResult(1.5)).toBe("1.5");
  });
});

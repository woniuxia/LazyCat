export interface CalcResult {
  rawValue: string;
  displayValue: string;
}

export function normalizeCalcExpression(input: string): string {
  return input
    .replace(/[,，]/g, "")
    .replace(/、/g, "/")
    .replace(/[×xX]/g, "*")
    .replace(/÷/g, "/")
    .replace(/\uFF08/g, "(")
    .replace(/\uFF09/g, ")")
    .replace(/\s+/g, "")
    .replace(/(\d+(?:\.\d+)?)%/g, "($1/100)");
}

export function formatCalcResult(value: number): string {
  return new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 15 }).format(value);
}

export function calculateExpression(input: string): CalcResult {
  const normalized = normalizeCalcExpression(input);
  if (!normalized) throw new Error("请输入计算公式");
  if (!/^[0-9+\-*/().]+$/.test(normalized)) {
    throw new Error("仅支持数字和 + - * / ( ) 运算符");
  }
  let result: unknown;
  try {
    result = Function(`"use strict"; return (${normalized});`)();
  } catch {
    throw new Error("公式格式不正确");
  }
  if (typeof result !== "number" || !Number.isFinite(result)) {
    throw new Error("计算结果无效");
  }
  return { rawValue: result.toString(), displayValue: formatCalcResult(result) };
}

export function getCalcPreview(input: string): string {
  const source = input.trim();
  if (!source) return "";
  try {
    return calculateExpression(source).displayValue;
  } catch {
    /* incomplete expression, try trimming trailing operators */
  }
  const fallbackSource = source.replace(/[+\-*/xX×÷、(]+$/, "").trim();
  if (!fallbackSource) return "";
  try {
    return calculateExpression(fallbackSource).displayValue;
  } catch {
    return "";
  }
}

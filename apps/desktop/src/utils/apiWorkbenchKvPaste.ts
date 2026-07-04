import type { ApiWorkbenchKeyValueRow } from "../types/api-workbench";

export interface ApiWorkbenchKvPasteResult {
  rows: ApiWorkbenchKeyValueRow[];
}

function splitLineBySeparator(line: string): ApiWorkbenchKeyValueRow {
  const colonIndex = line.indexOf(":");
  const equalsIndex = line.indexOf("=");
  let separatorIndex = -1;
  if (colonIndex >= 0 && equalsIndex >= 0) {
    separatorIndex = Math.min(colonIndex, equalsIndex);
  } else {
    separatorIndex = Math.max(colonIndex, equalsIndex);
  }
  if (separatorIndex < 0) {
    return { enabled: true, key: line.trim(), value: "" };
  }
  return {
    enabled: true,
    key: line.slice(0, separatorIndex).trim(),
    value: line.slice(separatorIndex + 1).replace(/^\s+/, ""),
  };
}

function splitQueryString(line: string): ApiWorkbenchKeyValueRow[] {
  return line
    .split("&")
    .filter((segment) => segment.trim() !== "")
    .map((segment) => {
      const equalsIndex = segment.indexOf("=");
      if (equalsIndex < 0) {
        return { enabled: true, key: segment.trim(), value: "" };
      }
      return {
        enabled: true,
        key: segment.slice(0, equalsIndex).trim(),
        value: segment.slice(equalsIndex + 1),
      };
    });
}

/** 返回 null 表示不满足拆分条件（按普通粘贴处理） */
export function parseApiWorkbenchKvPaste(text: string): ApiWorkbenchKvPasteResult | null {
  const lines = text
    .replace(/\r\n?/g, "\n")
    .split("\n")
    .filter((line) => line.trim() !== "");
  if (lines.length === 0) return null;
  if (lines.length === 1) {
    const line = lines[0];
    if (line.includes("&")) {
      const rows = splitQueryString(line);
      return rows.length > 0 ? { rows } : null;
    }
    if (!line.includes("=") && !line.includes(":")) return null;
    return { rows: [splitLineBySeparator(line)] };
  }
  return { rows: lines.map(splitLineBySeparator) };
}

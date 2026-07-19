export interface JsonLocation {
  line: number;
  column: number;
}

export interface RootCombination {
  keyword: "oneOf" | "anyOf";
  count: number;
}

export function formatJsonDocument(input: string): string {
  return JSON.stringify(JSON.parse(input), null, 2);
}

export function rootCombination(input: string): RootCombination | null {
  try {
    const schema = JSON.parse(input) as Record<string, unknown>;
    for (const keyword of ["oneOf", "anyOf"] as const) {
      const branches = schema?.[keyword];
      if (Array.isArray(branches) && branches.length > 0) {
        return { keyword, count: branches.length };
      }
    }
  } catch {
    // 编辑期间允许 Schema 暂时不完整，Monaco 会负责语法提示。
  }
  return null;
}

export function parseJsonErrorLocation(message: string): JsonLocation | null {
  const chinese = message.match(/第\s*(\d+)\s*行[^\d]+第\s*(\d+)\s*列/);
  const english = message.match(/line\s+(\d+)\s+column\s+(\d+)/i);
  const match = chinese ?? english;
  if (!match) return null;
  return { line: Number(match[1]), column: Number(match[2]) };
}

export function pointerLastToken(pointer: string): string {
  const token = pointer.split("/").filter(Boolean).at(-1) ?? "";
  return token.replace(/~1/g, "/").replace(/~0/g, "~");
}

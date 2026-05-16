import type { ScopeParseResult, SpotlightProviderId } from "../spotlight/types";

const SCOPE_PREFIX_MAP: Record<string, SpotlightProviderId> = {
  t: "todo",
  todo: "todo",
  v: "vault",
  vault: "vault",
  h: "hosts",
  hosts: "hosts",
  p: "pm",
  pm: "pm",
};

export function parseSpotlightQuery(raw: string): ScopeParseResult {
  const trimmed = raw.replace(/^\s+/, "");
  if (!trimmed) return { scope: null, query: "" };

  // 必须以"前缀 + 空格"开头才视为作用域；单纯输入字母不应锁定作用域
  const spaceIdx = trimmed.indexOf(" ");
  if (spaceIdx <= 0) return { scope: null, query: trimmed };

  const head = trimmed.slice(0, spaceIdx).toLowerCase();
  const scope = SCOPE_PREFIX_MAP[head];
  if (!scope) return { scope: null, query: trimmed };

  const rest = trimmed.slice(spaceIdx + 1).trimStart();
  return { scope, query: rest };
}

export function dropScopePrefix(raw: string): string {
  const parsed = parseSpotlightQuery(raw);
  return parsed.scope ? parsed.query : raw;
}

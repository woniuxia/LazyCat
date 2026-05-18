import type { QuickCommandId, ScopeParseResult, SpotlightProviderId } from "../spotlight/types";

export function parseSpotlightQuery(
  raw: string,
  aliasMap?: Map<string, SpotlightProviderId>,
): ScopeParseResult {
  const trimmed = raw.replace(/^\s+/, "");
  if (!trimmed) return { scope: null, query: "" };

  // 必须以"前缀 + 空格"开头才视为作用域;单纯输入字母不应锁定作用域
  const spaceIdx = trimmed.indexOf(" ");
  if (spaceIdx <= 0) return { scope: null, query: trimmed };

  const head = trimmed.slice(0, spaceIdx).toLowerCase();
  const map = aliasMap ?? DEFAULT_ALIAS_MAP;
  const scope = map.get(head);
  if (!scope) return { scope: null, query: trimmed };

  const rest = trimmed.slice(spaceIdx + 1).trimStart();
  return { scope, query: rest };
}

export function dropScopePrefix(
  raw: string,
  aliasMap?: Map<string, SpotlightProviderId>,
): string {
  const parsed = parseSpotlightQuery(raw, aliasMap);
  return parsed.scope ? parsed.query : raw;
}

export interface QuickCommandTodoCreate {
  kind: "todo-create";
  text: string;
}

export interface QuickCommandCalc {
  kind: "calc";
  text: string;
}

export type QuickCommand = QuickCommandTodoCreate | QuickCommandCalc;

const DEFAULT_ENABLED_QUICK_COMMANDS = new Set<QuickCommandId>(["todo-create", "calc"]);

export function parseQuickCommand(
  raw: string,
  enabledIds?: Set<QuickCommandId>,
): QuickCommand | null {
  const enabled = enabledIds ?? DEFAULT_ENABLED_QUICK_COMMANDS;
  const trimmedLeft = raw.replace(/^\s+/, "");
  if (trimmedLeft.startsWith("+ ")) {
    if (!enabled.has("todo-create")) return null;
    return { kind: "todo-create", text: trimmedLeft.slice(2).trim() };
  }
  // 允许 "calc"(进入空 calc 卡)或 "calc <expr>";"calcXXX" 等非词边界仍拒绝
  const calcMatch = /^calc(?:\s([\s\S]*))?$/i.exec(trimmedLeft);
  if (calcMatch) {
    if (!enabled.has("calc")) return null;
    return { kind: "calc", text: (calcMatch[1] ?? "").trim() };
  }
  return null;
}

// 默认 alias map(沿用 v0.6 行为,兼容现有调用点与单测)
const DEFAULT_ALIAS_MAP = new Map<string, SpotlightProviderId>([
  ["t", "todo"],
  ["todo", "todo"],
  ["v", "vault"],
  ["vault", "vault"],
  ["h", "hosts"],
  ["hosts", "hosts"],
  ["p", "pm"],
  ["pm", "pm"],
]);

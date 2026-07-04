import type {
  ApiWorkbenchKeyValueRow,
  ApiWorkbenchRequestDraft,
  ApiWorkbenchVariable,
} from "../types/api-workbench";
import { extractApiWorkbenchVariables } from "./apiWorkbench";

export interface ApiWorkbenchVariableUsage {
  name: string;
  source: "environment" | "global" | "missing";
}

function collectFromText(text: string, out: string[], seen: Set<string>) {
  for (const name of extractApiWorkbenchVariables(text)) {
    if (seen.has(name)) continue;
    seen.add(name);
    out.push(name);
  }
}

function collectFromRows(rows: ApiWorkbenchKeyValueRow[], out: string[], seen: Set<string>) {
  for (const row of rows) {
    if (!row.enabled) continue;
    collectFromText(row.key, out, seen);
    collectFromText(row.value, out, seen);
  }
}

export function summarizeApiWorkbenchVariables(input: {
  draft: ApiWorkbenchRequestDraft;
  environmentVariables: ApiWorkbenchVariable[];
  globalVariables: ApiWorkbenchVariable[];
}): ApiWorkbenchVariableUsage[] {
  const names: string[] = [];
  const seen = new Set<string>();
  collectFromText(input.draft.url, names, seen);
  collectFromRows(input.draft.query, names, seen);
  collectFromRows(input.draft.headers, names, seen);
  if (input.draft.bodyType === "json" || input.draft.bodyType === "text") {
    collectFromText(input.draft.body, names, seen);
  }
  if (input.draft.bodyType === "form-urlencoded") {
    collectFromRows(input.draft.form, names, seen);
  }

  const environmentNames = new Set(input.environmentVariables.map((item) => item.name));
  const globalNames = new Set(input.globalVariables.map((item) => item.name));
  return names.map((name) => ({
    name,
    source: environmentNames.has(name) ? "environment" : globalNames.has(name) ? "global" : "missing",
  }));
}

/** variables 优先级从高到低（如 [环境, 全局]）；缺失变量保留 {{NAME}} 原文 */
export function resolveApiWorkbenchTemplate(
  text: string,
  variables: ApiWorkbenchVariable[][],
): { text: string; missing: string[] } {
  const lookup = new Map<string, string>();
  for (const group of variables) {
    for (const item of group) {
      if (!lookup.has(item.name)) lookup.set(item.name, item.value);
    }
  }
  const missing: string[] = [];
  const missingSeen = new Set<string>();
  const resolved = text.replace(/\{\{\s*([^{}]+?)\s*\}\}/g, (raw, rawName: string) => {
    const name = rawName.trim();
    if (lookup.has(name)) return lookup.get(name) as string;
    if (!missingSeen.has(name)) {
      missingSeen.add(name);
      missing.push(name);
    }
    return raw;
  });
  return { text: resolved, missing };
}

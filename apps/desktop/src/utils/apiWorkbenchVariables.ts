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

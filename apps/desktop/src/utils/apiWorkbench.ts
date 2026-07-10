import type {
  ApiWorkbenchBodyType,
  ApiWorkbenchCollection,
  ApiWorkbenchKeyValueRow,
  ApiWorkbenchMethod,
  ApiWorkbenchRequestDraft,
  ApiWorkbenchSendResult,
  ApiWorkbenchVariable,
} from "../types/api-workbench";
import { splitApiWorkbenchQueryPairs } from "./apiWorkbenchKvPaste";

export const API_WORKBENCH_METHODS: ApiWorkbenchMethod[] = [
  "GET",
  "POST",
  "PUT",
  "PATCH",
  "DELETE",
  "HEAD",
  "OPTIONS",
];

export const API_WORKBENCH_BODY_TYPES: ApiWorkbenchBodyType[] = [
  "none",
  "json",
  "text",
  "form-urlencoded",
];

export const API_WORKBENCH_ENVIRONMENT_MANAGER_VALUE =
  "__api_workbench_environment_manager__";

export const DEFAULT_API_WORKBENCH_DRAFT: ApiWorkbenchRequestDraft = {
  method: "GET",
  url: "",
  query: [],
  headers: [],
  bodyType: "none",
  body: "",
  form: [],
  timeoutMs: 10000,
  followRedirects: false,
};

export function validateApiWorkbenchVariableName(name: string): boolean {
  return /^[A-Za-z0-9_-]{1,64}$/.test(name);
}

export function extractApiWorkbenchVariables(input: string): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  const re = /\{\{\s*([^{}]+?)\s*\}\}/g;
  let match: RegExpExecArray | null;
  while ((match = re.exec(input))) {
    const name = match[1].trim();
    if (!seen.has(name)) {
      seen.add(name);
      out.push(name);
    }
  }
  return out;
}

function encodeQuery(rows: ApiWorkbenchKeyValueRow[]): string {
  return rows
    .filter((row) => row.enabled && row.key.trim())
    .map((row) => `${encodeURIComponent(row.key.trim())}=${encodeURIComponent(row.value)}`)
    .join("&");
}

export function buildApiWorkbenchPreviewUrl(
  baseUrl: string,
  rawUrl: string,
  query: ApiWorkbenchKeyValueRow[],
): string {
  const url = rawUrl.trim();
  const isAbsolute = /^https?:\/\//i.test(url);
  const joined = isAbsolute
    ? url
    : `${baseUrl.trim().replace(/\/+$/, "")}/${url.replace(/^\/+/, "")}`;
  const qs = encodeQuery(query);
  if (!qs) return joined;
  return `${joined}${joined.includes("?") ? "&" : "?"}${qs}`;
}

function normalizeRows(rows: unknown): ApiWorkbenchKeyValueRow[] {
  if (!Array.isArray(rows)) return [];
  return rows
    .map((row) => row as Partial<ApiWorkbenchKeyValueRow>)
    .filter((row) => typeof row.key === "string" || typeof row.value === "string")
    .map((row) => ({
      enabled: row.enabled !== false,
      key: String(row.key ?? "").trim(),
      value: String(row.value ?? ""),
    }))
    .filter((row) => row.key.trim() !== "" || row.value.trim() !== "");
}

type DraftInput = Partial<Record<keyof ApiWorkbenchRequestDraft, unknown>>;

export function normalizeApiWorkbenchDraft(input: DraftInput): ApiWorkbenchRequestDraft {
  const method = API_WORKBENCH_METHODS.includes(input.method as ApiWorkbenchMethod)
    ? (input.method as ApiWorkbenchMethod)
    : "GET";
  const bodyType = API_WORKBENCH_BODY_TYPES.includes(input.bodyType as ApiWorkbenchBodyType)
    ? (input.bodyType as ApiWorkbenchBodyType)
    : "none";
  const timeoutMs = Math.min(120000, Math.max(100, Number(input.timeoutMs || 10000)));
  return {
    method,
    url: String(input.url ?? "").trim(),
    query: normalizeRows(input.query),
    headers: normalizeRows(input.headers),
    bodyType,
    body: String(input.body ?? ""),
    form: normalizeRows(input.form),
    timeoutMs,
    followRedirects: input.followRedirects === true,
  };
}

export function createApiWorkbenchBlankDraft(): ApiWorkbenchRequestDraft {
  return normalizeApiWorkbenchDraft({});
}

export interface ApiWorkbenchUrlSplitResult {
  url: string;
  rows: ApiWorkbenchKeyValueRow[];
}

export function countApiWorkbenchActiveRows(rows: ApiWorkbenchKeyValueRow[]): number {
  return rows.filter((row) => row.enabled && row.key.trim() !== "").length;
}

export function hasApiWorkbenchBody(draft: ApiWorkbenchRequestDraft): boolean {
  if (draft.bodyType === "none") return false;
  if (draft.bodyType === "form-urlencoded") return countApiWorkbenchActiveRows(draft.form) > 0;
  return draft.body.trim() !== "";
}

/** 无 ? 或 ? 后为空时返回 null；不做 URL 解码 */
export function splitApiWorkbenchUrlQuery(rawUrl: string): ApiWorkbenchUrlSplitResult | null {
  const questionIndex = rawUrl.indexOf("?");
  if (questionIndex < 0) return null;
  const queryText = rawUrl.slice(questionIndex + 1);
  if (queryText.trim() === "") return null;
  const rows = splitApiWorkbenchQueryPairs(queryText);
  if (rows.length === 0) return null;
  return { url: rawUrl.slice(0, questionIndex), rows };
}

export function resolveApiWorkbenchEnvironmentSelect(
  value: unknown,
  currentEnvironmentId: number | null,
):
  | { kind: "environment"; environmentId: number | null }
  | { kind: "manage"; environmentId: number | null } {
  if (value === API_WORKBENCH_ENVIRONMENT_MANAGER_VALUE) {
    return { kind: "manage", environmentId: currentEnvironmentId };
  }
  if (typeof value === "number" && Number.isFinite(value)) {
    return { kind: "environment", environmentId: value };
  }
  return { kind: "environment", environmentId: currentEnvironmentId };
}

export function buildApiWorkbenchSelectionState(input: {
  nextCollection: Pick<ApiWorkbenchCollection, "id" | "activeEnvironmentId"> | null;
}): {
  selectedCollectionId: number | null;
  selectedEnvironmentId: number | null;
  selectedRequestId: number | null;
  requestName: string;
  draft: ApiWorkbenchRequestDraft;
  response: ApiWorkbenchSendResult | null;
} {
  const nextCollectionId = input.nextCollection?.id ?? null;
  return {
    selectedCollectionId: nextCollectionId,
    selectedEnvironmentId: input.nextCollection?.activeEnvironmentId ?? null,
    selectedRequestId: null,
    requestName: "",
    draft: createApiWorkbenchBlankDraft(),
    response: null,
  };
}

export function buildApiWorkbenchNewRequestState(input: { folderId: number | null }): {
  selectedRequestId: number | null;
  selectedRequestFolderId: number | null;
  requestName: string;
  requestDescription: string;
  draft: ApiWorkbenchRequestDraft;
  response: ApiWorkbenchSendResult | null;
} {
  return {
    selectedRequestId: null,
    selectedRequestFolderId: input.folderId,
    requestName: "",
    requestDescription: "",
    draft: createApiWorkbenchBlankDraft(),
    response: null,
  };
}

export function draftApiWorkbenchEnvironmentRows(
  variables: ApiWorkbenchVariable[],
): ApiWorkbenchKeyValueRow[] {
  const rows = variables.map((item) => ({
    enabled: true,
    key: item.name,
    value: item.value,
  }));
  if (!rows.some((row) => row.key === "BASE_URL")) {
    rows.unshift({ enabled: true, key: "BASE_URL", value: "" });
  }
  return rows;
}

export function serializeApiWorkbenchEnvironmentRows(
  rows: ApiWorkbenchKeyValueRow[],
): ApiWorkbenchVariable[] {
  return rows
    .filter((row) => row.enabled && row.key.trim())
    .map((row) => ({
      name: row.key.trim(),
      value: row.value,
      isSecret: false,
    }));
}

export function findDuplicateApiWorkbenchEnvironmentVariableNames(
  rows: ApiWorkbenchKeyValueRow[],
): string[] {
  const seen = new Set<string>();
  const duplicateSet = new Set<string>();
  const duplicates: string[] = [];
  for (const row of rows) {
    if (!row.enabled) continue;
    const name = row.key.trim();
    if (!name) continue;
    if (seen.has(name)) {
      if (!duplicateSet.has(name)) {
        duplicateSet.add(name);
        duplicates.push(name);
      }
      continue;
    }
    seen.add(name);
  }
  return duplicates;
}

export function buildApiWorkbenchEnvironmentDraftSummary(
  rows: ApiWorkbenchKeyValueRow[],
  savedVariables: ApiWorkbenchVariable[],
): {
  variableCount: number;
  hasBaseUrl: boolean;
  duplicateNames: string[];
  changed: boolean;
} {
  const variables = serializeApiWorkbenchEnvironmentRows(rows);
  const savedPairs = savedVariables.map((item) => ({
    name: item.name,
    value: item.value,
  }));
  return {
    variableCount: variables.length,
    hasBaseUrl: variables.some((item) => item.name === "BASE_URL" && item.value.trim()),
    duplicateNames: findDuplicateApiWorkbenchEnvironmentVariableNames(rows),
    changed: JSON.stringify(variables.map(({ name, value }) => ({ name, value }))) !== JSON.stringify(savedPairs),
  };
}

export function formatApiWorkbenchResponseBody(body: string, contentType: string): string {
  if (!/json/i.test(contentType)) return body;
  try {
    return JSON.stringify(JSON.parse(body), null, 2);
  } catch {
    return body;
  }
}

export type ApiWorkbenchStatusTone = "success" | "warning" | "danger" | "info";
export function getApiWorkbenchStatusTone(
  status: number | null,
  error: string | null,
): ApiWorkbenchStatusTone {
  if (status === null || error) return "info";
  if (status >= 200 && status < 300) return "success";
  if (status >= 300 && status < 400) return "warning";
  if (status >= 400 && status < 600) return "danger";
  return "info";
}

const API_WORKBENCH_METHOD_CLASSES = new Set([
  "get",
  "post",
  "put",
  "patch",
  "delete",
  "head",
  "options",
]);

export function getApiWorkbenchMethodClass(method: string): string {
  const normalized = method.trim().toLowerCase();
  return API_WORKBENCH_METHOD_CLASSES.has(normalized)
    ? `method-${normalized}`
    : "method-default";
}

export type ApiWorkbenchAuthInput =
  | { type: "bearer"; token: string }
  | { type: "basic"; username: string; password: string };

export function buildApiWorkbenchAuthHeader(input: ApiWorkbenchAuthInput): string {
  if (input.type === "bearer") {
    return `Bearer ${input.token.trim()}`;
  }
  const bytes = new TextEncoder().encode(`${input.username}:${input.password}`);
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return `Basic ${btoa(binary)}`;
}

export function upsertApiWorkbenchHeaderRow(
  rows: ApiWorkbenchKeyValueRow[],
  key: string,
  value: string,
): ApiWorkbenchKeyValueRow[] {
  const lowered = key.trim().toLowerCase();
  const index = rows.findIndex((row) => row.key.trim().toLowerCase() === lowered);
  if (index === -1) {
    return [...rows, { enabled: true, key, value }];
  }
  return rows.map((row, i) => (i === index ? { ...row, enabled: true, value } : row));
}

export interface ApiWorkbenchVariablePrefixMatch {
  start: number;
  query: string;
}

export function matchApiWorkbenchVariablePrefix(
  text: string,
  cursor: number,
): ApiWorkbenchVariablePrefixMatch | null {
  const before = text.slice(0, Math.max(0, cursor));
  const match = /\{\{([A-Za-z0-9_.-]*)$/.exec(before);
  if (!match) return null;
  return { start: match.index, query: match[1] };
}

export function applyApiWorkbenchVariableCompletion(
  text: string,
  cursor: number,
  name: string,
): { text: string; cursor: number } | null {
  const match = matchApiWorkbenchVariablePrefix(text, cursor);
  if (!match) return null;
  const after = text.slice(cursor);
  const closing = after.startsWith("}}") ? "" : "}}";
  const nextText = `${text.slice(0, match.start)}{{${name}${closing}${after}`;
  return { text: nextText, cursor: match.start + name.length + 4 };
}

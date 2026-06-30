import type {
  ApiWorkbenchBodyType,
  ApiWorkbenchCollection,
  ApiWorkbenchKeyValueRow,
  ApiWorkbenchMethod,
  ApiWorkbenchRequestDraft,
  ApiWorkbenchSendResult,
  ApiWorkbenchVariable,
} from "../types/api-workbench";

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
    }));
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
  };
}

export function createApiWorkbenchBlankDraft(): ApiWorkbenchRequestDraft {
  return normalizeApiWorkbenchDraft({});
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

export function formatApiWorkbenchResponseBody(body: string, contentType: string): string {
  if (!/json/i.test(contentType)) return body;
  try {
    return JSON.stringify(JSON.parse(body), null, 2);
  } catch {
    return body;
  }
}

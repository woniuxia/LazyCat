import type {
  ApiMockCorsConfig,
  ApiMockHeaderRow,
  ApiMockMethod,
  ApiMockProjectSummary,
  ApiMockResponseKind,
  ApiMockRuntimeSnapshot,
  ApiMockRuntimeState,
} from "../types/api-mock";

export const API_MOCK_METHODS: ApiMockMethod[] = [
  "GET",
  "POST",
  "PUT",
  "PATCH",
  "DELETE",
  "HEAD",
  "OPTIONS",
];

export const DEFAULT_API_MOCK_CONTENT_TYPE = "application/json; charset=utf-8";

export const DEFAULT_API_MOCK_CORS: ApiMockCorsConfig = {
  enabled: true,
  allowOrigin: "*",
  allowMethods: [],
  allowHeaders: "*",
  exposeHeaders: "",
  allowCredentials: false,
  maxAgeSeconds: 600,
};

export type ApiMockValidationResult = { ok: true; message: "" } | { ok: false; message: string };
export type ApiMockRuntimeAction = "start" | "stop" | "restart";

function ok(): ApiMockValidationResult {
  return { ok: true, message: "" };
}

function fail(message: string): ApiMockValidationResult {
  return { ok: false, message };
}

function isValidParamName(name: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_-]*$/.test(name);
}

export function validateMockPathPattern(pattern: string): ApiMockValidationResult {
  const value = pattern.trim();
  if (!value.startsWith("/")) {
    return fail("路径必须以 / 开头");
  }
  if (value.length > 1 && value.endsWith("/")) {
    return fail("路径不能以 / 结尾");
  }

  const segments = value.split("/").slice(1);
  for (let index = 0; index < segments.length; index += 1) {
    const segment = segments[index];
    if (!segment) return fail("路径不能包含空路径段");
    if (segment === "*") {
      return index === segments.length - 1 ? ok() : fail("通配符 * 只能作为最后一个完整路径段");
    }
    if (segment.includes("*")) {
      return fail("通配符 * 只能作为完整路径段");
    }
    if (segment.startsWith(":")) {
      const name = segment.slice(1);
      if (!name || !isValidParamName(name)) {
        return fail("参数名只能包含字母、数字、下划线和短横线，且首字符必须是字母或下划线");
      }
      continue;
    }
    if (segment.includes(":")) {
      return fail("路径参数必须占用一个完整路径段");
    }
  }
  return ok();
}

export function validateMockCorsConfig(config: ApiMockCorsConfig): ApiMockValidationResult {
  if (!config.enabled) return ok();
  if (config.allowCredentials && config.allowOrigin.trim() === "*") {
    return fail("允许携带凭据时，Allow-Origin 不能为 *");
  }
  if (config.maxAgeSeconds !== null && (!Number.isInteger(config.maxAgeSeconds) || config.maxAgeSeconds < 0)) {
    return fail("Max-Age 必须为空或非负整数");
  }
  return ok();
}

export function normalizeMockHeaderRows(rows: ApiMockHeaderRow[]): ApiMockHeaderRow[] {
  return rows
    .filter((row) => row.enabled !== false)
    .map((row) => ({
      enabled: true,
      key: row.key.trim(),
      value: row.value,
    }))
    .filter((row) => row.key.length > 0);
}

export function buildMockRouteSummary(input: {
  method: ApiMockMethod;
  pathPattern: string;
  statusCode: number;
  responseKind: ApiMockResponseKind;
}): string {
  const kind = input.responseKind === "file" ? "file" : "static";
  return `${input.method} ${input.pathPattern} -> ${input.statusCode} ${kind}`;
}

export function deriveMockProjectRuntimeState(project: ApiMockProjectSummary): ApiMockRuntimeState {
  if (project.runtime.running && project.runtime.restartRequired) return "restart-required";
  if (project.runtime.running) return "running";
  if (project.runtime.lastError) return "error";
  return "stopped";
}

export function getMockProjectRuntimeAction(project: ApiMockProjectSummary): ApiMockRuntimeAction {
  return deriveMockProjectRuntimeState(project) === "restart-required"
    ? "restart"
    : project.runtime.running
      ? "stop"
      : "start";
}

export function getMockProjectAccessUrl(project: Pick<ApiMockProjectSummary, "host" | "port">): string {
  const host = project.host === "0.0.0.0" ? "127.0.0.1" : project.host;
  return `http://${host}:${project.port}`;
}

export function isMockProjectRestartRequired(
  running: ApiMockRuntimeSnapshot | null,
  current: ApiMockRuntimeSnapshot,
): boolean {
  if (!running) return false;
  return (
    running.host !== current.host ||
    running.port !== current.port ||
    running.routeSignature !== current.routeSignature
  );
}

export function getMockRouteSpecificityLabel(pattern: string): "精确" | "参数" | "通配" {
  if (pattern.split("/").includes("*")) return "通配";
  if (pattern.split("/").some((segment) => segment.startsWith(":"))) return "参数";
  return "精确";
}

export function resolveMockFileContentType(currentContentType: string, fileName: string): string {
  const current = currentContentType.trim();
  const inferred = inferMockContentTypeFromFileName(fileName);
  if (current && current !== DEFAULT_API_MOCK_CONTENT_TYPE && current !== "application/octet-stream") {
    return current;
  }
  return inferred || current || "application/octet-stream";
}

export function inferMockContentTypeFromFileName(fileName: string): string {
  const extension = fileName.split(/[\\/]/).pop()?.split(".").pop()?.toLowerCase() ?? "";
  const types: Record<string, string> = {
    json: "application/json",
    txt: "text/plain; charset=utf-8",
    html: "text/html; charset=utf-8",
    htm: "text/html; charset=utf-8",
    css: "text/css; charset=utf-8",
    js: "text/javascript; charset=utf-8",
    xml: "application/xml",
    csv: "text/csv; charset=utf-8",
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    gif: "image/gif",
    webp: "image/webp",
    svg: "image/svg+xml",
    pdf: "application/pdf",
    zip: "application/zip",
    wasm: "application/wasm",
  };
  return types[extension] ?? "";
}

export function formatMockFileSize(size: number): string {
  if (!Number.isFinite(size) || size <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = size;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  const formatted = Number.isInteger(value) ? String(value) : value.toFixed(1).replace(/\.0$/, "");
  return `${formatted} ${units[unitIndex]}`;
}

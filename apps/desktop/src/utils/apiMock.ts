import type {
  ApiMockCorsConfig,
  ApiMockHeaderRow,
  ApiMockMethod,
  ApiMockProjectSummary,
  ApiMockResponseKind,
  ApiMockRuntimeSnapshot,
  ApiMockRuntimeState,
} from "../types/api-mock";
import { formatByteSize } from "./format";

export const API_MOCK_METHODS: ApiMockMethod[] = [
  "GET",
  "POST",
  "PUT",
  "PATCH",
  "DELETE",
  "HEAD",
  "OPTIONS",
];

/** HTTP 方法语义配色（浅色背景下对比度 >= 4.5:1），列表与表单共用 */
export const API_MOCK_METHOD_COLORS: Record<ApiMockMethod, string> = {
  GET: "#047857",
  POST: "#1d4ed8",
  PUT: "#b45309",
  PATCH: "#6d28d9",
  DELETE: "#b91c1c",
  HEAD: "#475569",
  OPTIONS: "#475569",
};

export function getMockMethodColor(method: string): string {
  return API_MOCK_METHOD_COLORS[method as ApiMockMethod] ?? "#475569";
}

export const DEFAULT_API_MOCK_CONTENT_TYPE = "application/json; charset=utf-8";

export interface ApiMockContentTypePreset {
  label: string;
  value: string;
}

export type ApiMockContentValidationNotice = {
  level: "error" | "warning";
  message: string;
};

export const API_MOCK_CONTENT_TYPE_PRESETS: ApiMockContentTypePreset[] = [
  { label: "JSON UTF-8", value: "application/json; charset=utf-8" },
  { label: "JSON", value: "application/json" },
  { label: "Plain Text", value: "text/plain; charset=utf-8" },
  { label: "HTML", value: "text/html; charset=utf-8" },
  { label: "XML", value: "application/xml" },
  { label: "XML Text", value: "text/xml; charset=utf-8" },
  { label: "CSV", value: "text/csv; charset=utf-8" },
  { label: "Form URL Encoded", value: "application/x-www-form-urlencoded" },
  { label: "Multipart Form", value: "multipart/form-data" },
  { label: "PNG", value: "image/png" },
  { label: "JPEG", value: "image/jpeg" },
  { label: "SVG", value: "image/svg+xml" },
  { label: "WebP", value: "image/webp" },
  { label: "GIF", value: "image/gif" },
  { label: "PDF", value: "application/pdf" },
  { label: "ZIP", value: "application/zip" },
  { label: "WASM", value: "application/wasm" },
  { label: "Binary", value: "application/octet-stream" },
  { label: "CSS", value: "text/css; charset=utf-8" },
  { label: "JavaScript", value: "text/javascript; charset=utf-8" },
];

export const DEFAULT_API_MOCK_CORS: ApiMockCorsConfig = {
  enabled: true,
  allowOrigin: "*",
  allowMethods: [],
  allowHeaders: "*",
  exposeHeaders: "",
  allowCredentials: false,
  maxAgeSeconds: 600,
};

export function createDefaultApiMockCors(enabled = true): ApiMockCorsConfig {
  return { ...DEFAULT_API_MOCK_CORS, allowMethods: [], enabled };
}

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

export function parseMockCorsOriginList(allowOrigin: string): string[] {
  return allowOrigin
    .split(",")
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

export function validateMockCorsConfig(config: ApiMockCorsConfig): ApiMockValidationResult {
  if (!config.enabled) return ok();
  if (config.allowCredentials) {
    const origins = parseMockCorsOriginList(config.allowOrigin);
    if (origins.length === 0 || origins.includes("*")) {
      return fail("允许携带凭据时，Allow-Origin 不能为 * 或留空");
    }
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

export function trimMockContentType(contentType: string): string {
  return contentType.trim();
}

export function normalizeMockContentType(contentType: string): string {
  return trimMockContentType(contentType).split(";")[0]?.trim().toLowerCase() ?? "";
}

export function validateMockContentTypeHeader(contentType: string): ApiMockValidationResult {
  const value = trimMockContentType(contentType);
  if (!value) return ok();
  if (/[\r\n]/.test(value)) return fail("Content-Type 不能包含换行符");

  const mime = value.split(";")[0]?.trim() ?? "";
  if (!/^[A-Za-z0-9!#$&^_.+-]+\/[A-Za-z0-9!#$&^_.+-]+$/.test(mime)) {
    return fail("Content-Type 必须是 type/subtype 格式");
  }

  return ok();
}

export function validateMockStaticResponseContent(input: {
  contentType: string;
  bodyText: string;
}): ApiMockContentValidationNotice | null {
  const mime = normalizeMockContentType(input.contentType);
  if (!mime) return null;

  if (mime === "application/json") {
    try {
      JSON.parse(input.bodyText);
      return null;
    } catch {
      return {
        level: "error",
        message: "当前 Content-Type 是 JSON，但响应 Body 不是合法 JSON",
      };
    }
  }

  if (mime === "application/xml" || mime === "text/xml") {
    return {
      level: "warning",
      message: "当前 Content-Type 是 XML，请确认响应 Body 是正确的 XML 内容",
    };
  }
  if (mime === "text/html") {
    return {
      level: "warning",
      message: "当前 Content-Type 是 HTML，请确认响应 Body 是 HTML 内容",
    };
  }
  if (mime === "application/x-www-form-urlencoded") {
    return {
      level: "warning",
      message: "application/x-www-form-urlencoded 通常用于请求体，作为响应 Content-Type 时请确认是否符合预期",
    };
  }
  if (mime === "multipart/form-data") {
    return {
      level: "warning",
      message: "multipart/form-data 通常用于请求体，作为响应 Content-Type 时请确认是否符合预期",
    };
  }

  return null;
}

export function getMockFileContentTypeWarning(input: { contentType: string; fileName: string }): string {
  const current = normalizeMockContentType(input.contentType);
  const inferred = normalizeMockContentType(inferMockContentTypeFromFileName(input.fileName));
  if (!current || !inferred || current === "application/octet-stream" || current === inferred) return "";
  return `上传文件看起来是 ${inferred}，当前 Content-Type 是 ${current}，请确认是否正确。`;
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
  return formatByteSize(size);
}

export const MAX_API_MOCK_DELAY_MS = 60000;

export interface ApiMockProjectFormSnapshot {
  name: string;
  description: string;
  host: string;
  port: number;
  enabledCorsDefault: boolean;
}

export interface ApiMockRouteFormSnapshot {
  name: string;
  method: ApiMockMethod;
  pathPattern: string;
  statusCode: number;
  responseKind: ApiMockResponseKind;
  contentType: string;
  headers: ApiMockHeaderRow[];
  bodyText: string;
  enabled: boolean;
  delayMs: number;
  fileId: number | null;
  cors: ApiMockCorsConfig;
}

export function serializeMockProjectForm(form: ApiMockProjectFormSnapshot): string {
  return JSON.stringify({
    name: form.name,
    description: form.description,
    host: form.host,
    port: form.port,
    enabledCorsDefault: form.enabledCorsDefault,
  });
}

export function serializeMockRouteForm(form: ApiMockRouteFormSnapshot): string {
  return JSON.stringify({
    name: form.name,
    method: form.method,
    pathPattern: form.pathPattern,
    statusCode: form.statusCode,
    responseKind: form.responseKind,
    contentType: form.contentType,
    headers: form.headers.map((row) => ({ enabled: row.enabled, key: row.key, value: row.value })),
    bodyText: form.bodyText,
    enabled: form.enabled,
    delayMs: form.delayMs,
    fileId: form.fileId,
    cors: {
      enabled: form.cors.enabled,
      allowOrigin: form.cors.allowOrigin,
      allowMethods: [...form.cors.allowMethods],
      allowHeaders: form.cors.allowHeaders,
      exposeHeaders: form.cors.exposeHeaders,
      allowCredentials: form.cors.allowCredentials,
      maxAgeSeconds: form.cors.maxAgeSeconds,
    },
  });
}

export function getMockBodyEditorLanguage(contentType: string): string {
  const mime = normalizeMockContentType(contentType);
  if (!mime) return "plaintext";
  if (mime === "application/json" || mime.endsWith("+json")) return "json";
  if (mime === "text/html") return "html";
  if (mime === "application/xml" || mime === "text/xml" || mime.endsWith("+xml")) return "xml";
  if (mime === "text/css") return "css";
  if (mime === "text/javascript" || mime === "application/javascript") return "javascript";
  return "plaintext";
}

const MOCK_BODY_FORMAT_LABELS: Record<string, string> = {
  json: "JSON",
  xml: "XML",
  html: "HTML",
  css: "CSS",
  javascript: "JavaScript",
};

export function getMockBodyFormatLabel(language: string): string {
  return MOCK_BODY_FORMAT_LABELS[language] ?? "";
}

export function findMockPortConflict(
  projects: Array<Pick<ApiMockProjectSummary, "id" | "name" | "port">>,
  currentId: number | null,
  port: number,
): Pick<ApiMockProjectSummary, "id" | "name" | "port"> | null {
  return projects.find((project) => project.id !== currentId && project.port === port) ?? null;
}

export type ApiMockLogRowTone = "alert" | "normal";

export function getMockLogRowTone(log: { routeId: number | null; error: string | null }): ApiMockLogRowTone {
  if (log.routeId === null || (log.error !== null && log.error !== "")) return "alert";
  return "normal";
}

export function buildMockRouteUrl(
  project: Pick<ApiMockProjectSummary, "host" | "port">,
  pathPattern: string,
): string {
  const base = getMockProjectAccessUrl(project);
  const path = pathPattern.startsWith("/") ? pathPattern : `/${pathPattern}`;
  return `${base}${path}`;
}

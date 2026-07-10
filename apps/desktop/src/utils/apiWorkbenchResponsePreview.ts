import type {
  ApiWorkbenchHistoryDetail,
  ApiWorkbenchKeyValueRow,
  ApiWorkbenchSendResult,
} from "../types/api-workbench";

export type ApiWorkbenchResponseViewerKind =
  | "empty"
  | "json"
  | "html"
  | "image"
  | "pdf"
  | "office-word"
  | "office-sheet"
  | "office-slides"
  | "text"
  | "binary";

export type ApiWorkbenchResponseMode = "preview" | "raw" | "meta";

export interface ApiWorkbenchExampleResponse {
  status: number | null;
  statusText: string;
  contentType: string;
  headers: ApiWorkbenchKeyValueRow[];
  bodyText: string;
  bodySize: number;
  bodyTruncated: boolean;
  bodyStorage: ApiWorkbenchSendResult["bodyStorage"];
  bodyFileName?: string;
  bodyExtension?: string;
  bodyPreviewError?: string | null;
  savedAt: string;
}

export function normalizeApiWorkbenchMime(contentType: string): string {
  return contentType.split(";")[0]?.trim().toLowerCase() ?? "";
}

function lowerExtension(response: Pick<ApiWorkbenchSendResult, "bodyExtension" | "bodyFileName">): string {
  const explicit = response.bodyExtension.trim().toLowerCase();
  if (explicit) return explicit;
  const fileName = response.bodyFileName.trim().toLowerCase();
  const dot = fileName.lastIndexOf(".");
  return dot >= 0 ? fileName.slice(dot + 1) : "";
}

function isJsonResponse(response: ApiWorkbenchSendResult): boolean {
  const mime = normalizeApiWorkbenchMime(response.contentType);
  if (mime === "application/json" || mime.endsWith("+json")) return true;
  if (!response.bodyText.trim()) return false;
  try {
    JSON.parse(response.bodyText);
    return true;
  } catch {
    return false;
  }
}

export function getApiWorkbenchResponseViewerKind(
  response: ApiWorkbenchSendResult,
): ApiWorkbenchResponseViewerKind {
  if (response.bodyStorage === "empty" || response.bodySize === 0) return "empty";
  if (response.bodyStorage === "truncated-binary") return "binary";
  if (response.bodyStorage === "file" && !response.bodyFilePath.trim()) return "binary";

  const mime = normalizeApiWorkbenchMime(response.contentType);
  const extension = lowerExtension(response);

  if (isJsonResponse(response)) return "json";
  if (mime === "text/html" || mime === "application/xhtml+xml" || extension === "html") {
    return "html";
  }
  if (mime.startsWith("image/") || ["png", "jpg", "jpeg", "gif", "webp", "bmp"].includes(extension)) {
    return "image";
  }
  if (mime === "application/pdf" || extension === "pdf") return "pdf";
  if (
    [
      "docx",
      "doc",
      "rtf",
      "odt",
    ].includes(extension)
  ) {
    return "office-word";
  }
  if (["xlsx", "xls", "ods", "csv"].includes(extension)) return "office-sheet";
  if (["pptx", "ppt", "odp"].includes(extension)) return "office-slides";
  if (response.bodyStorage === "file") return "binary";
  return "text";
}

export function getDefaultApiWorkbenchResponseMode(
  response: ApiWorkbenchSendResult,
): ApiWorkbenchResponseMode {
  const kind = getApiWorkbenchResponseViewerKind(response);
  return kind === "binary" || kind === "empty" ? "meta" : "preview";
}

export function getApiWorkbenchRawLanguage(
  kind: ApiWorkbenchResponseViewerKind,
  contentType: string,
): string {
  if (kind === "json") return "json";
  if (kind === "html") return "html";
  const mime = normalizeApiWorkbenchMime(contentType);
  if (mime === "application/xml" || mime === "text/xml" || mime.endsWith("+xml")) return "xml";
  return "plaintext";
}

export const API_WORKBENCH_JSON_PREVIEW_MAX_BYTES = 1_000_000;

export type ApiWorkbenchJsonPreviewResult =
  | { ok: true; value: unknown }
  | { ok: false; reason: string };

export function parseApiWorkbenchJsonPreview(bodyText: string): ApiWorkbenchJsonPreviewResult {
  if (bodyText.length > API_WORKBENCH_JSON_PREVIEW_MAX_BYTES) {
    return { ok: false, reason: "响应体超过 1 MB，已切换原文模式" };
  }
  try {
    return { ok: true, value: JSON.parse(bodyText) };
  } catch {
    return { ok: false, reason: "JSON 解析失败，已切换原文模式" };
  }
}

export function formatApiWorkbenchPreviewBody(response: ApiWorkbenchSendResult): string {
  if (!response.bodyText && response.bodyStorage === "file") {
    return `二进制响应：${response.bodyFileName || response.bodyExtension || "文件"}，${response.bodySize} bytes`;
  }
  if (!response.bodyText && response.bodyStorage === "truncated-binary") {
    return `二进制响应已截断，${response.bodySize} bytes`;
  }
  if (!response.bodyText) return "";
  if (!isJsonResponse(response)) return response.bodyText;
  try {
    return JSON.stringify(JSON.parse(response.bodyText), null, 2);
  } catch {
    return response.bodyText;
  }
}

export function buildApiWorkbenchResponseFromHistory(
  item: ApiWorkbenchHistoryDetail,
): ApiWorkbenchSendResult {
  return {
    finalUrl: item.finalUrl,
    status: item.status,
    statusText: "",
    ok: item.ok,
    durationMs: item.durationMs,
    requestHeaders: [],
    responseHeaders: [],
    bodyText: item.bodyPreview,
    bodySize: item.bodySize,
    bodyTruncated: item.bodyTruncated,
    contentType: item.contentType,
    bodyStorage: item.bodyStorage,
    bodyFilePath: item.bodyFilePath,
    bodyFileName: item.bodyFileName,
    bodyExtension: item.bodyExtension,
    bodyHash: item.bodyHash,
    bodyPreviewError: item.bodyPreviewError,
    error: item.error,
  };
}

export function buildApiWorkbenchExampleResponse(
  response: ApiWorkbenchSendResult,
  savedAt = new Date().toISOString(),
): ApiWorkbenchExampleResponse {
  const base: ApiWorkbenchExampleResponse = {
    status: response.status,
    statusText: response.statusText,
    contentType: response.contentType,
    headers: response.responseHeaders.map((row) => ({
      enabled: true,
      key: row.key,
      value: row.value,
    })),
    bodyText: response.bodyText,
    bodySize: response.bodySize,
    bodyTruncated: response.bodyTruncated,
    bodyStorage: response.bodyStorage,
    savedAt,
  };

  if (response.bodyStorage === "file" || response.bodyStorage === "truncated-binary") {
    const fileLabel = response.bodyFileName || response.bodyExtension || "二进制文件";
    return {
      ...base,
      bodyText:
        response.bodyStorage === "truncated-binary"
          ? `二进制响应已截断，仅保存元信息摘要（${response.bodySize} bytes）。`
          : `二进制响应，仅保存元信息摘要（${fileLabel}，${response.bodySize} bytes）。`,
      bodyFileName: response.bodyFileName || undefined,
      bodyExtension: response.bodyExtension || undefined,
      bodyPreviewError: response.bodyPreviewError,
    };
  }

  return base;
}

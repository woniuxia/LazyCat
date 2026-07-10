import { describe, expect, it } from "vitest";
import type {
  ApiWorkbenchHistoryDetail,
  ApiWorkbenchSendResult,
} from "../types/api-workbench";
import {
  buildApiWorkbenchExampleResponse,
  buildApiWorkbenchResponseFromHistory,
  formatApiWorkbenchPreviewBody,
  getApiWorkbenchResponseViewerKind,
  getApiWorkbenchRawLanguage,
  getDefaultApiWorkbenchResponseMode,
  parseApiWorkbenchJsonPreview,
} from "./apiWorkbenchResponsePreview";

function response(overrides: Partial<ApiWorkbenchSendResult> = {}): ApiWorkbenchSendResult {
  return {
    finalUrl: "http://127.0.0.1/api",
    status: 200,
    statusText: "OK",
    ok: true,
    durationMs: 12,
    requestHeaders: [],
    responseHeaders: [],
    bodyText: "ok",
    bodySize: 2,
    bodyTruncated: false,
    contentType: "text/plain",
    bodyStorage: "text",
    bodyFilePath: "",
    bodyFileName: "",
    bodyExtension: "",
    bodyHash: "",
    bodyPreviewError: null,
    error: null,
    ...overrides,
  } as ApiWorkbenchSendResult;
}

describe("apiWorkbenchResponsePreview", () => {
  it("classifies common response preview kinds", () => {
    expect(
      getApiWorkbenchResponseViewerKind(
        response({ contentType: "application/json", bodyText: "{\"ok\":true}" }),
      ),
    ).toBe("json");
    expect(getApiWorkbenchResponseViewerKind(response({ contentType: "text/html" }))).toBe(
      "html",
    );
    expect(
      getApiWorkbenchResponseViewerKind(
        response({
          bodyStorage: "file",
          bodyFilePath: "E:/tmp/lazycat/image.png",
          bodyExtension: "png",
          contentType: "image/png",
        }),
      ),
    ).toBe("image");
    expect(
      getApiWorkbenchResponseViewerKind(
        response({
          bodyStorage: "file",
          bodyFilePath: "E:/tmp/lazycat/report.pdf",
          bodyExtension: "pdf",
          contentType: "application/pdf",
        }),
      ),
    ).toBe("pdf");
    expect(
      getApiWorkbenchResponseViewerKind(
        response({
          bodyStorage: "file",
          bodyFilePath: "E:/tmp/lazycat/doc.docx",
          bodyExtension: "docx",
        }),
      ),
    ).toBe("office-word");
    expect(
      getApiWorkbenchResponseViewerKind(
        response({
          bodyStorage: "file",
          bodyFilePath: "E:/tmp/lazycat/sheet.xlsx",
          bodyExtension: "xlsx",
        }),
      ),
    ).toBe("office-sheet");
    expect(
      getApiWorkbenchResponseViewerKind(
        response({
          bodyStorage: "file",
          bodyFilePath: "E:/tmp/lazycat/deck.pptx",
          bodyExtension: "pptx",
        }),
      ),
    ).toBe("office-slides");
    expect(
      getApiWorkbenchResponseViewerKind(
        response({ bodyStorage: "file", bodyExtension: "bin", contentType: "application/octet-stream" }),
      ),
    ).toBe("binary");
    expect(getApiWorkbenchResponseViewerKind(response({ bodyStorage: "empty", bodyText: "" }))).toBe(
      "empty",
    );
  });

  it("chooses preview mode for renderable responses and metadata for binary fallback", () => {
    expect(getDefaultApiWorkbenchResponseMode(response({ contentType: "text/html" }))).toBe(
      "preview",
    );
    expect(
      getDefaultApiWorkbenchResponseMode(
        response({
          bodyStorage: "file",
          bodyFilePath: "E:/tmp/lazycat/file.bin",
          bodyExtension: "bin",
          contentType: "application/octet-stream",
        }),
      ),
    ).toBe("meta");
  });

  it("formats json preview body and falls back to raw text", () => {
    expect(
      formatApiWorkbenchPreviewBody(response({ contentType: "application/json", bodyText: "{\"ok\":true}" })),
    ).toBe("{\n  \"ok\": true\n}");
    expect(formatApiWorkbenchPreviewBody(response({ bodyText: "plain" }))).toBe("plain");
  });

  it("maps viewer kind and mime to monaco raw language", () => {
    expect(getApiWorkbenchRawLanguage("json", "application/json")).toBe("json");
    expect(getApiWorkbenchRawLanguage("html", "text/html")).toBe("html");
    expect(getApiWorkbenchRawLanguage("text", "application/xml")).toBe("xml");
    expect(getApiWorkbenchRawLanguage("text", "text/xml; charset=utf-8")).toBe("xml");
    expect(getApiWorkbenchRawLanguage("text", "application/atom+xml")).toBe("xml");
    expect(getApiWorkbenchRawLanguage("text", "text/plain")).toBe("plaintext");
    expect(getApiWorkbenchRawLanguage("binary", "application/octet-stream")).toBe("plaintext");
  });

  it("parses json preview with size guard and failure reasons", () => {
    expect(parseApiWorkbenchJsonPreview("{\"ok\":true}")).toEqual({
      ok: true,
      value: { ok: true },
    });
    const tooLarge = parseApiWorkbenchJsonPreview("x".repeat(1_000_001));
    expect(tooLarge.ok).toBe(false);
    if (!tooLarge.ok) expect(tooLarge.reason).toContain("1 MB");
    const invalid = parseApiWorkbenchJsonPreview("{oops}");
    expect(invalid.ok).toBe(false);
    if (!invalid.ok) expect(invalid.reason).toContain("JSON 解析失败");
  });

  it("rebuilds a previewable response from history detail", () => {
    const history = {
      id: 1,
      collectionId: null,
      environmentId: null,
      requestId: null,
      replayedFromHistoryId: null,
      name: "",
      note: "",
      pinned: false,
      method: "GET",
      url: "/file",
      finalUrl: "http://127.0.0.1/file",
      status: 200,
      durationMs: 3,
      ok: true,
      error: null,
      contentType: "application/pdf",
      bodySize: 128,
      bodyPreview: "",
      bodyTruncated: false,
      bodyStorage: "file",
      bodyFilePath: "E:/tmp/lazycat/report.pdf",
      bodyFileName: "report.pdf",
      bodyExtension: "pdf",
      bodyHash: "abc",
      bodyPreviewError: null,
      hasRequestSnapshot: false,
      hasExecutedRequestSnapshot: false,
      createdAt: "2026-07-01 00:00:00",
      requestSnapshot: null,
    } as ApiWorkbenchHistoryDetail;

    const rebuilt = buildApiWorkbenchResponseFromHistory(history);

    expect(rebuilt.bodyStorage).toBe("file");
    expect(rebuilt.bodyFilePath).toBe("E:/tmp/lazycat/report.pdf");
    expect(getApiWorkbenchResponseViewerKind(rebuilt)).toBe("pdf");
  });

  it("stores binary example responses as metadata summaries without cache references", () => {
    const example = buildApiWorkbenchExampleResponse(
      response({
        contentType: "application/pdf",
        bodyStorage: "file",
        bodyFilePath: "E:/tmp/lazycat/report.pdf",
        bodyFileName: "report.pdf",
        bodyExtension: "pdf",
        bodyHash: "abc",
        bodyText: "",
        bodySize: 128,
      }),
      "2026-07-01T00:00:00.000Z",
    ) as unknown as Record<string, unknown>;

    expect(example.bodyStorage).toBe("file");
    expect(example.bodyFilePath).toBeUndefined();
    expect(example.bodyHash).toBeUndefined();
    expect(example.bodyText).toContain("二进制响应");
  });
});

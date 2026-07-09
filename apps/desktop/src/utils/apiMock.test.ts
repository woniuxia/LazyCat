import { describe, expect, it } from "vitest";
import type { ApiMockProjectSummary } from "../types/api-mock";
import {
  API_MOCK_CONTENT_TYPE_PRESETS,
  buildMockRouteSummary,
  buildMockRouteUrl,
  createDefaultApiMockCors,
  deriveMockProjectRuntimeState,
  findMockPortConflict,
  formatMockFileSize,
  getMockBodyEditorLanguage,
  getMockFileContentTypeWarning,
  getMockLogRowTone,
  getMockProjectAccessUrl,
  getMockProjectRuntimeAction,
  getMockRouteSpecificityLabel,
  isMockProjectRestartRequired,
  normalizeMockContentType,
  resolveMockFileContentType,
  serializeMockProjectForm,
  serializeMockRouteForm,
  trimMockContentType,
  normalizeMockHeaderRows,
  validateMockContentTypeHeader,
  validateMockCorsConfig,
  validateMockPathPattern,
  validateMockStaticResponseContent,
} from "./apiMock";
import type { ApiMockRouteFormSnapshot } from "./apiMock";

describe("apiMock utils", () => {
  it("validates supported path patterns", () => {
    expect(validateMockPathPattern("/api/users").ok).toBe(true);
    expect(validateMockPathPattern("/api/users/:id").ok).toBe(true);
    expect(validateMockPathPattern("/files/*").ok).toBe(true);
  });

  it("rejects invalid path patterns", () => {
    expect(validateMockPathPattern("api/users").message).toContain("/");
    expect(validateMockPathPattern("/users/:").message).toContain("参数名");
    expect(validateMockPathPattern("/users/user-:id").message).toContain("完整路径段");
    expect(validateMockPathPattern("/files/*/raw").message).toContain("最后");
    expect(validateMockPathPattern("/bad/:1").message).toContain("参数名");
  });

  it("validates CORS credentials origin rule", () => {
    const base = {
      enabled: true,
      allowMethods: [],
      allowHeaders: "*",
      exposeHeaders: "",
      allowCredentials: true,
      maxAgeSeconds: 600,
    } as const;

    expect(validateMockCorsConfig({ ...base, allowOrigin: "*", allowMethods: [] })).toEqual({
      ok: false,
      message: "允许携带凭据时，Allow-Origin 不能为 * 或留空",
    });
    // 空 Origin 会在后端兜底为 *，校验必须同样拦截。
    expect(validateMockCorsConfig({ ...base, allowOrigin: "  ", allowMethods: [] }).ok).toBe(false);
    // 多值列表中混入 * 同样拦截。
    expect(validateMockCorsConfig({ ...base, allowOrigin: "http://a.com, *", allowMethods: [] }).ok).toBe(false);
    // 合法多值列表放行。
    expect(
      validateMockCorsConfig({ ...base, allowOrigin: "http://a.com, http://b.com", allowMethods: [] }).ok,
    ).toBe(true);
  });

  it("creates independent CORS defaults honoring project switch", () => {
    const enabled = createDefaultApiMockCors();
    const disabled = createDefaultApiMockCors(false);
    expect(enabled.enabled).toBe(true);
    expect(disabled.enabled).toBe(false);

    enabled.allowMethods.push("GET");
    expect(createDefaultApiMockCors().allowMethods).toEqual([]);
  });

  it("normalizes header rows", () => {
    expect(
      normalizeMockHeaderRows([
        { enabled: true, key: " X-Trace ", value: "abc" },
        { enabled: true, key: "", value: "ignored" },
        { enabled: false, key: "X-Off", value: "0" },
      ]),
    ).toEqual([{ enabled: true, key: "X-Trace", value: "abc" }]);
  });

  it("builds route summaries", () => {
    expect(
      buildMockRouteSummary({
        method: "GET",
        pathPattern: "/api/users/:id",
        statusCode: 201,
        responseKind: "static_body",
      }),
    ).toBe("GET /api/users/:id -> 201 static");
  });

  it("derives project runtime state", () => {
    const project: ApiMockProjectSummary = {
      id: 1,
      name: "Demo",
      description: "",
      host: "127.0.0.1",
      port: 18080,
      enabledCorsDefault: true,
      sortOrder: 0,
      routeCount: 1,
      enabledRouteCount: 1,
      runtime: { running: false, restartRequired: false, lastError: null, startedAt: null },
    };

    expect(deriveMockProjectRuntimeState(project)).toBe("stopped");
    expect(
      deriveMockProjectRuntimeState({
        ...project,
        runtime: { running: true, restartRequired: false, lastError: null, startedAt: "2026-07-02T00:00:00Z" },
      }),
    ).toBe("running");
    expect(
      deriveMockProjectRuntimeState({
        ...project,
        runtime: { running: true, restartRequired: true, lastError: null, startedAt: "2026-07-02T00:00:00Z" },
      }),
    ).toBe("restart-required");
    expect(
      deriveMockProjectRuntimeState({
        ...project,
        runtime: { running: false, restartRequired: false, lastError: "bind failed", startedAt: null },
      }),
    ).toBe("error");
  });

  it("derives the primary runtime action for a project", () => {
    const project: ApiMockProjectSummary = {
      id: 1,
      name: "Demo",
      description: "",
      host: "127.0.0.1",
      port: 18080,
      enabledCorsDefault: true,
      sortOrder: 0,
      routeCount: 1,
      enabledRouteCount: 1,
      runtime: { running: false, restartRequired: false, lastError: null, startedAt: null },
    };

    expect(getMockProjectRuntimeAction(project)).toBe("start");
    expect(
      getMockProjectRuntimeAction({
        ...project,
        runtime: { running: true, restartRequired: false, lastError: null, startedAt: "2026-07-02T00:00:00Z" },
      }),
    ).toBe("stop");
    expect(
      getMockProjectRuntimeAction({
        ...project,
        runtime: { running: true, restartRequired: true, lastError: null, startedAt: "2026-07-02T00:00:00Z" },
      }),
    ).toBe("restart");
  });

  it("builds a usable access URL for wildcard listeners", () => {
    const project: ApiMockProjectSummary = {
      id: 1,
      name: "Demo",
      description: "",
      host: "0.0.0.0",
      port: 18080,
      enabledCorsDefault: true,
      sortOrder: 0,
      routeCount: 0,
      enabledRouteCount: 0,
      runtime: { running: false, restartRequired: false, lastError: null, startedAt: null },
    };

    expect(getMockProjectAccessUrl(project)).toBe("http://127.0.0.1:18080");
  });

  it("resolves file response content type from the selected file when the form still uses the default", () => {
    expect(resolveMockFileContentType("application/json; charset=utf-8", "avatar.png")).toBe("image/png");
    expect(resolveMockFileContentType("application/vnd.custom", "avatar.png")).toBe("application/vnd.custom");
    expect(resolveMockFileContentType("", "archive.unknown")).toBe("application/octet-stream");
  });

  it("exposes common content type presets", () => {
    const values = API_MOCK_CONTENT_TYPE_PRESETS.map((item) => item.value);

    expect(values).toEqual(
      expect.arrayContaining([
        "application/json; charset=utf-8",
        "application/json",
        "text/plain; charset=utf-8",
        "text/html; charset=utf-8",
        "application/xml",
        "text/xml; charset=utf-8",
        "text/csv; charset=utf-8",
        "application/x-www-form-urlencoded",
        "multipart/form-data",
        "image/png",
        "image/jpeg",
        "image/svg+xml",
        "image/webp",
        "image/gif",
        "application/pdf",
        "application/zip",
        "application/wasm",
        "application/octet-stream",
        "text/css; charset=utf-8",
        "text/javascript; charset=utf-8",
      ]),
    );
    expect(new Set(values).size).toBe(values.length);
  });

  it("normalizes content type MIME without parameters", () => {
    expect(normalizeMockContentType(" Application/JSON; Charset=UTF-8 ")).toBe("application/json");
    expect(normalizeMockContentType("application/vnd.lazycat.mock+json; version=1")).toBe(
      "application/vnd.lazycat.mock+json",
    );
    expect(normalizeMockContentType("")).toBe("");
  });

  it("trims content type before saving", () => {
    expect(trimMockContentType("  application/json; charset=utf-8  ")).toBe("application/json; charset=utf-8");
  });

  it("rejects unsafe or malformed content type values", () => {
    expect(validateMockContentTypeHeader("").ok).toBe(true);
    expect(validateMockContentTypeHeader(" application/vnd.lazycat.mock+json; version=1 ").ok).toBe(true);
    expect(validateMockContentTypeHeader("application/json\r\nX-Bad: 1")).toEqual({
      ok: false,
      message: "Content-Type 不能包含换行符",
    });
    expect(validateMockContentTypeHeader("json")).toEqual({
      ok: false,
      message: "Content-Type 必须是 type/subtype 格式",
    });
  });

  it("blocks invalid JSON when the response content type is JSON", () => {
    expect(
      validateMockStaticResponseContent({
        contentType: "application/json; charset=utf-8",
        bodyText: "{ bad json",
      }),
    ).toEqual({
      level: "error",
      message: "当前 Content-Type 是 JSON，但响应 Body 不是合法 JSON",
    });
    expect(
      validateMockStaticResponseContent({
        contentType: "application/json",
        bodyText: '{ "ok": true }',
      }),
    ).toBeNull();
  });

  it("warns for response content types that need user confirmation", () => {
    expect(
      validateMockStaticResponseContent({
        contentType: "application/xml",
        bodyText: "<root>",
      }),
    ).toEqual({
      level: "warning",
      message: "当前 Content-Type 是 XML，请确认响应 Body 是正确的 XML 内容",
    });
    expect(
      validateMockStaticResponseContent({
        contentType: "text/html; charset=utf-8",
        bodyText: "<main>",
      }),
    ).toEqual({
      level: "warning",
      message: "当前 Content-Type 是 HTML，请确认响应 Body 是 HTML 内容",
    });
    expect(
      validateMockStaticResponseContent({
        contentType: "multipart/form-data",
        bodyText: "",
      }),
    ).toEqual({
      level: "warning",
      message: "multipart/form-data 通常用于请求体，作为响应 Content-Type 时请确认是否符合预期",
    });
    expect(
      validateMockStaticResponseContent({
        contentType: "application/x-www-form-urlencoded",
        bodyText: "ok=true",
      }),
    ).toEqual({
      level: "warning",
      message: "application/x-www-form-urlencoded 通常用于请求体，作为响应 Content-Type 时请确认是否符合预期",
    });
  });

  it("warns when selected content type and imported file extension disagree", () => {
    expect(getMockFileContentTypeWarning({ contentType: "application/pdf", fileName: "avatar.png" })).toBe(
      "上传文件看起来是 image/png，当前 Content-Type 是 application/pdf，请确认是否正确。",
    );
    expect(
      getMockFileContentTypeWarning({ contentType: "text/plain; charset=utf-8", fileName: "readme.txt" }),
    ).toBe("");
    expect(getMockFileContentTypeWarning({ contentType: "application/octet-stream", fileName: "avatar.png" })).toBe(
      "",
    );
  });

  it("formats mock file sizes for compact display", () => {
    expect(formatMockFileSize(512)).toBe("512 B");
    expect(formatMockFileSize(1536)).toBe("1.5 KB");
    expect(formatMockFileSize(2 * 1024 * 1024)).toBe("2 MB");
  });

  it("detects restart-required configuration changes", () => {
    expect(
      isMockProjectRestartRequired(
        { host: "127.0.0.1", port: 18080, routeSignature: "a" },
        { host: "127.0.0.1", port: 18080, routeSignature: "a" },
      ),
    ).toBe(false);
    expect(
      isMockProjectRestartRequired(
        { host: "127.0.0.1", port: 18080, routeSignature: "a" },
        { host: "127.0.0.1", port: 18081, routeSignature: "a" },
      ),
    ).toBe(true);
    expect(isMockProjectRestartRequired(null, { host: "127.0.0.1", port: 18080, routeSignature: "a" })).toBe(false);
  });

  it("labels route specificity", () => {
    expect(getMockRouteSpecificityLabel("/api/users")).toBe("精确");
    expect(getMockRouteSpecificityLabel("/api/users/:id")).toBe("参数");
    expect(getMockRouteSpecificityLabel("/files/*")).toBe("通配");
  });

  it("serializes project forms so dirty detection ignores nothing and matches identical state", () => {
    const form = {
      name: "Demo",
      description: "",
      host: "127.0.0.1",
      port: 18080,
      enabledCorsDefault: true,
    };
    const baseline = serializeMockProjectForm(form);

    expect(serializeMockProjectForm({ ...form })).toBe(baseline);
    expect(serializeMockProjectForm({ ...form, port: 18081 })).not.toBe(baseline);
    expect(serializeMockProjectForm({ ...form, description: "x" })).not.toBe(baseline);
  });

  it("serializes route forms including headers, delay, file and cors", () => {
    const form: ApiMockRouteFormSnapshot = {
      name: "User",
      method: "GET",
      pathPattern: "/api/users/:id",
      statusCode: 200,
      responseKind: "static_body",
      contentType: "application/json; charset=utf-8",
      headers: [{ enabled: true, key: "X-Trace", value: "abc" }],
      bodyText: "{}",
      enabled: true,
      delayMs: 0,
      fileId: null,
      cors: {
        enabled: true,
        allowOrigin: "*",
        allowMethods: [],
        allowHeaders: "*",
        exposeHeaders: "",
        allowCredentials: false,
        maxAgeSeconds: 600,
      },
    };
    const baseline = serializeMockRouteForm(form);

    expect(serializeMockRouteForm({ ...form, headers: [{ enabled: true, key: "X-Trace", value: "abc" }] })).toBe(
      baseline,
    );
    expect(serializeMockRouteForm({ ...form, delayMs: 300 })).not.toBe(baseline);
    expect(serializeMockRouteForm({ ...form, enabled: false })).not.toBe(baseline);
    expect(serializeMockRouteForm({ ...form, fileId: 3 })).not.toBe(baseline);
    expect(
      serializeMockRouteForm({ ...form, headers: [{ enabled: false, key: "X-Trace", value: "abc" }] }),
    ).not.toBe(baseline);
    expect(
      serializeMockRouteForm({ ...form, cors: { ...form.cors, allowOrigin: "http://localhost:5173" } }),
    ).not.toBe(baseline);
  });

  it("maps content types to monaco editor languages", () => {
    expect(getMockBodyEditorLanguage("application/json; charset=utf-8")).toBe("json");
    expect(getMockBodyEditorLanguage("application/vnd.custom+json")).toBe("json");
    expect(getMockBodyEditorLanguage("text/html; charset=utf-8")).toBe("html");
    expect(getMockBodyEditorLanguage("application/xml")).toBe("xml");
    expect(getMockBodyEditorLanguage("text/xml; charset=utf-8")).toBe("xml");
    expect(getMockBodyEditorLanguage("image/svg+xml")).toBe("xml");
    expect(getMockBodyEditorLanguage("text/css")).toBe("css");
    expect(getMockBodyEditorLanguage("text/javascript; charset=utf-8")).toBe("javascript");
    expect(getMockBodyEditorLanguage("text/plain")).toBe("plaintext");
    expect(getMockBodyEditorLanguage("")).toBe("plaintext");
  });

  it("finds port conflicts among other projects only", () => {
    const projects = [
      { id: 1, name: "A", port: 18080 },
      { id: 2, name: "B", port: 18081 },
    ];

    expect(findMockPortConflict(projects, 1, 18080)).toBeNull();
    expect(findMockPortConflict(projects, 1, 18081)).toEqual({ id: 2, name: "B", port: 18081 });
    expect(findMockPortConflict(projects, null, 18080)).toEqual({ id: 1, name: "A", port: 18080 });
    expect(findMockPortConflict(projects, null, 18999)).toBeNull();
  });

  it("derives alert tone for missed or failed log rows", () => {
    expect(getMockLogRowTone({ routeId: null, error: null })).toBe("alert");
    expect(getMockLogRowTone({ routeId: 1, error: "file missing" })).toBe("alert");
    expect(getMockLogRowTone({ routeId: null, error: "并发超限" })).toBe("alert");
    expect(getMockLogRowTone({ routeId: 1, error: null })).toBe("normal");
    expect(getMockLogRowTone({ routeId: 1, error: "" })).toBe("normal");
  });

  it("builds full route URLs from the access URL and path pattern", () => {
    expect(buildMockRouteUrl({ host: "127.0.0.1", port: 18080 }, "/api/users/:id")).toBe(
      "http://127.0.0.1:18080/api/users/:id",
    );
    expect(buildMockRouteUrl({ host: "0.0.0.0", port: 8080 }, "/files/*")).toBe("http://127.0.0.1:8080/files/*");
  });
});

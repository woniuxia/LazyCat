import { describe, expect, it } from "vitest";
import type { ApiMockProjectSummary } from "../types/api-mock";
import {
  buildMockRouteSummary,
  deriveMockProjectRuntimeState,
  getMockRouteSpecificityLabel,
  isMockProjectRestartRequired,
  normalizeMockHeaderRows,
  validateMockCorsConfig,
  validateMockPathPattern,
} from "./apiMock";

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
    expect(
      validateMockCorsConfig({
        enabled: true,
        allowOrigin: "*",
        allowMethods: [],
        allowHeaders: "*",
        exposeHeaders: "",
        allowCredentials: true,
        maxAgeSeconds: 600,
      }),
    ).toEqual({
      ok: false,
      message: "允许携带凭据时，Allow-Origin 不能为 *",
    });
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
});

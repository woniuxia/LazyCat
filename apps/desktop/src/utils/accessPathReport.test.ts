import { describe, expect, it } from "vitest";
import { formatAccessPathReport, sanitizeAccessPathReport } from "./accessPathReport";
import type { AccessPathReport } from "../types/access-path-diagnostics";

const base: AccessPathReport = {
  schemaVersion: 1,
  reportId: "r1",
  input: {
    rawInput: "https://example.com/?token=abc&ok=1",
    protocol: "https",
    hostname: "example.com",
    targetKind: "hostname",
    port: 443,
    path: "/?token=abc&ok=1",
    url: "https://example.com/?token=abc&ok=1",
    sni: "example.com",
    verifyHostname: "example.com",
    httpHost: "example.com",
    connectionIp: null,
  },
  steps: [{ id: "proxy", lifecycle: "completed", evidenceIds: ["e1"] }],
  evidence: [
    {
      id: "e1",
      stepId: "proxy",
      kind: "headers",
      value: {
        Authorization: "Bearer abc",
        Cookie: "sid=abc",
        proxyUrl: "http://u:p@example.com:8080",
      },
    },
  ],
  conclusions: [],
  recommendations: [],
  startedAt: "2026-01-01T00:00:00Z",
};

describe("access path report", () => {
  it("redacts credentials, sensitive headers and query parameters", () => {
    const sanitized = sanitizeAccessPathReport(base);
    expect(JSON.stringify(sanitized)).not.toContain("Bearer abc");
    expect(JSON.stringify(sanitized)).not.toContain("sid=abc");
    expect(JSON.stringify(sanitized)).not.toContain("u:p@");
    expect(sanitized.input.url).toContain("token=%5BREDACTED%5D");
  });

  it("formats a stable JSON export", () => {
    const text = formatAccessPathReport(base);
    expect(text.startsWith("{\n")).toBe(true);
    expect(text).toContain("[REDACTED]");
  });
  it("redacts sensitive parameters even when the value is not a valid URL", () => {
    const malformed = {
      ...base,
      reportId: "malformed",
      input: {
        ...base.input,
        rawInput: "not a url?x-api-key=secret&client_secret=foo&access_token=bar",
        url: "not a url?x-api-key=secret&client_secret=foo&access_token=bar",
      },
    };
    const text = formatAccessPathReport(malformed);
    expect(text).not.toContain("x-api-key=secret");
    expect(text).not.toContain("client_secret=foo");
    expect(text).not.toContain("access_token=bar");
  });
});

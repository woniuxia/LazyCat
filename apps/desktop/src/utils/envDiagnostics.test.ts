import { describe, expect, it } from "vitest";
import {
  buildEnvironmentReport,
  environmentSummaryType,
  type EnvDetectResponse,
} from "./envDiagnostics";

function result(overrides: Partial<EnvDetectResponse["summary"]> = {}): EnvDetectResponse {
  return {
    platform: "windows",
    arch: "x86_64",
    durationMs: 120,
    summary: { total: 1, installed: 1, missing: 0, problems: 0, warnings: 0, ...overrides },
    tools: [{
      key: "node",
      name: "Node.js",
      installed: true,
      status: "ok",
      version: "v22.0.0",
      path: "C:/node.exe",
      paths: ["C:/node.exe"],
      error: null,
      suggestion: null,
    }],
    environment: [{ key: "JAVA_HOME", value: "", status: "missing", detail: "未配置" }],
    diagnostics: [],
  };
}

describe("envDiagnostics", () => {
  it("prioritizes errors over warnings in the summary", () => {
    expect(environmentSummaryType(result())).toBe("success");
    expect(environmentSummaryType(result({ warnings: 1 }))).toBe("warning");
    expect(environmentSummaryType(result({ problems: 1, warnings: 1 }))).toBe("error");
  });

  it("builds a readable support report", () => {
    const report = buildEnvironmentReport(result());
    expect(report).toContain("LazyCat 开发环境检测报告");
    expect(report).toContain("Node.js：正常；v22.0.0");
    expect(report).toContain("JAVA_HOME：未配置");
  });
});

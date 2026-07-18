import { describe, expect, it } from "vitest";
import {
  appendDiagnosticReport,
  createNetworkDiagnosticsSettings,
  MAX_DIAGNOSTIC_REPORT_BYTES,
  migrateNetworkDiagnosticsSettings,
  normalizeNetworkHistory,
} from "./networkDiagnosticsPersistence";
import type { AccessPathReport } from "../types/access-path-diagnostics";

const report: AccessPathReport = {
  schemaVersion: 1,
  reportId: "r1",
  input: {
    rawInput: "https://[2001:db8::1]:443/?token=secret",
    protocol: "https",
    hostname: "2001:db8::1",
    targetKind: "ipv6",
    port: 443,
    path: "/?token=secret",
    url: "https://[2001:db8::1]/?token=secret",
    sni: null,
    verifyHostname: "2001:db8::1",
    httpHost: "[2001:db8::1]",
    connectionIp: null,
  },
  steps: [],
  evidence: [],
  conclusions: [],
  recommendations: [],
  startedAt: "2026-01-01T00:00:00Z",
};

describe("network diagnostics persistence", () => {
  it("migrates legacy keys into a versioned envelope without dropping rows", () => {
    const result = migrateNetworkDiagnosticsSettings({
      current: null,
      legacyFavorites: [
        {
          id: "f1",
          name: "IPv6",
          protocol: "tcp",
          host: "2001:db8::1",
          port: 443,
          timeoutMs: 1000,
          createdAt: 1,
        },
      ],
      legacyHistory: [
        {
          id: "h1",
          checkedAt: 2,
          protocol: "tcp",
          target: "[2001:db8::1]:443",
          timeoutMs: 1000,
          reachable: true,
          latencyMs: 4,
        },
      ],
    });
    expect(result.migrated).toBe(true);
    expect(result.settings.schemaVersion).toBe(1);
    expect(result.settings.favorites[0].host).toBe("2001:db8::1");
    expect(result.settings.history[0].target).toBe("[2001:db8::1]:443");
  });

  it("is idempotent for an existing envelope and drops malformed rows", () => {
    const first = createNetworkDiagnosticsSettings({
      reports: [{ reportId: "malformed" }],
      history: [
        {
          id: "ok",
          checkedAt: 1,
          protocol: "ping",
          target: "host",
          timeoutMs: 1000,
          reachable: true,
          latencyMs: 1,
        },
        null,
      ],
    });
    const second = migrateNetworkDiagnosticsSettings({
      current: first,
      legacyFavorites: [{ id: "ignored" }],
      legacyHistory: [{ id: "ignored" }],
    });
    expect(second.migrated).toBe(false);
    expect(second.settings).toEqual(first);
    expect(second.settings.reports).toEqual([]);
    expect(normalizeNetworkHistory([null, { id: "bad" }])).toEqual([]);
  });

  it("keeps at most ten detailed reports and de-duplicates by report id", () => {
    let settings = createNetworkDiagnosticsSettings();
    for (let index = 0; index < 12; index += 1) {
      settings = appendDiagnosticReport(settings, { ...report, reportId: `r${index}` });
    }
    expect(settings.reports).toHaveLength(10);
    settings = appendDiagnosticReport(settings, report);
    expect(settings.reports.filter((item) => item.reportId === "r1")).toHaveLength(1);
  });
  it("bounds nested evidence and keeps evidence references valid", () => {
    const evidence = Array.from({ length: 100 }, (_, index) => ({
      id: `large-${index}`,
      stepId: "http" as const,
      kind: "fixture",
      value: { nested: "x".repeat(10_000) },
    }));
    const large = {
      ...report,
      steps: [
        {
          id: "http" as const,
          lifecycle: "completed" as const,
          evidenceIds: evidence.map((item) => item.id),
        },
      ],
      evidence,
    };

    const settings = appendDiagnosticReport(createNetworkDiagnosticsSettings(), large);
    const stored = settings.reports[0];
    expect(JSON.stringify(stored).length).toBeLessThanOrEqual(MAX_DIAGNOSTIC_REPORT_BYTES);
    const ids = new Set(stored.evidence.map((item) => item.id));
    expect(stored.steps[0].evidenceIds.every((id) => ids.has(id))).toBe(true);
  });
});

import { describe, expect, it } from "vitest";
import type { AccessPathDiagnosisRunSnapshot } from "../types/access-path-diagnostics";
import {
  acceptDiagnosisSnapshot,
  parseDnsServerList,
  parseQuickTarget,
  quickProbeAssessment,
  stepStateLabel,
} from "./accessPathDiagnosticsView";

function snapshot(runId: string, sequence: number): AccessPathDiagnosisRunSnapshot {
  return {
    runId,
    sequence,
    status: "running",
    report: {
      schemaVersion: 1,
      reportId: "report-1",
      runId,
      input: {
        rawInput: "example.test",
        protocol: "https",
        hostname: "example.test",
        targetKind: "hostname",
        port: 443,
        path: "/",
        url: "https://example.test/",
        sni: "example.test",
        verifyHostname: "example.test",
        httpHost: "example.test",
        connectionIp: null,
      },
      steps: [],
      evidence: [],
      conclusions: [],
      recommendations: [],
      startedAt: "2026-07-18T00:00:00Z",
    },
  };
}

describe("access path diagnostics view helpers", () => {
  it("accepts only the active run with a newer sequence", () => {
    const current = snapshot("run-1", 4);
    expect(acceptDiagnosisSnapshot(current, snapshot("run-1", 5), "run-1")?.sequence).toBe(5);
    expect(acceptDiagnosisSnapshot(current, snapshot("run-1", 4), "run-1")).toBe(current);
    expect(acceptDiagnosisSnapshot(current, snapshot("run-2", 9), "run-1")).toBe(current);
  });

  it("deduplicates comma, semicolon and whitespace separated DNS servers", () => {
    expect(parseDnsServerList("10.0.0.53, 1.1.1.1\n10.0.0.53;[::1]:5353")).toEqual([
      "10.0.0.53",
      "1.1.1.1",
      "[::1]:5353",
    ]);
  });

  it("keeps UDP silence unverified instead of reachable", () => {
    expect(
      quickProbeAssessment("udp", {
        reachable: true,
        note: "UDP 无响应（这是正常行为，端口可能开放）",
      }),
    ).toBe("unverified");
    expect(quickProbeAssessment("udp", { reachable: false, error: "端口不可达" })).toBe("rejected");
    expect(quickProbeAssessment("udp", { reachable: true })).toBe("confirmed");
  });

  it("parses IPv4, hostnames and bracketed IPv6 targets without splitting IPv6", () => {
    expect(parseQuickTarget("example.test:443")).toEqual({ host: "example.test", port: 443 });
    expect(parseQuickTarget("[2001:db8::1]:8443")).toEqual({
      host: "2001:db8::1",
      port: 8443,
    });
    expect(parseQuickTarget("2001:db8::1")).toEqual({ host: "2001:db8::1", port: null });
  });

  it("keeps lifecycle and outcome labels separate", () => {
    expect(
      stepStateLabel({
        id: "tcp",
        lifecycle: "completed",
        outcome: "unverified",
        evidenceIds: [],
      }),
    ).toBe("无法验证");
    expect(stepStateLabel({ id: "http", lifecycle: "blocked", evidenceIds: [] })).toBe("已阻断");
  });
});

import { describe, expect, it } from "vitest";
import type { AccessPathDiagnosisRunSnapshot } from "../types/access-path-diagnostics";
import {
  acceptDiagnosisSnapshot,
  buildDiagnosisGuide,
  DIAGNOSIS_PHASES,
  diagnosisPhaseState,
  orderDiagnosisRecommendations,
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

  it("groups checks into route, transport and application phases", () => {
    expect(DIAGNOSIS_PHASES.map((phase) => phase.stepIds)).toEqual([
      ["proxy", "hosts", "dns"],
      ["tcp", "tls"],
      ["http"],
    ]);
    expect(
      diagnosisPhaseState(DIAGNOSIS_PHASES[0], [
        { id: "proxy", lifecycle: "completed", outcome: "success", evidenceIds: [] },
        { id: "hosts", lifecycle: "completed", outcome: "warning", evidenceIds: [] },
        { id: "dns", lifecycle: "pending", evidenceIds: [] },
      ]),
    ).toBe("warning");
    expect(
      diagnosisPhaseState(DIAGNOSIS_PHASES[1], [
        { id: "tcp", lifecycle: "completed", outcome: "failed", evidenceIds: [] },
        { id: "tls", lifecycle: "blocked", evidenceIds: [] },
      ]),
    ).toBe("failed");
    expect(
      diagnosisPhaseState(DIAGNOSIS_PHASES[2], [
        { id: "http", lifecycle: "cancelled", evidenceIds: [] },
      ]),
    ).toBe("cancelled");
  });

  it("guides users to the first failed step before later warnings", () => {
    const value = snapshot("run-1", 6);
    value.status = "completed";
    value.report.steps = [
      { id: "proxy", lifecycle: "completed", outcome: "warning", evidenceIds: ["proxy-e"] },
      { id: "hosts", lifecycle: "completed", outcome: "success", evidenceIds: [] },
      {
        id: "dns",
        lifecycle: "completed",
        outcome: "failed",
        evidenceIds: ["dns-e"],
        error: { code: "nxdomain", message: "目标域名不存在", retriable: false },
      },
      { id: "tcp", lifecycle: "blocked", evidenceIds: [] },
    ];

    expect(buildDiagnosisGuide(value.report, value.status)).toMatchObject({
      stepId: "dns",
      tone: "failed",
      eyebrow: "优先排查",
      title: "DNS 解析未通过",
      description: "目标域名不存在",
    });
  });

  it("reports the active step while a diagnosis is running", () => {
    const value = snapshot("run-1", 2);
    value.report.steps = [
      { id: "proxy", lifecycle: "completed", outcome: "success", evidenceIds: [] },
      { id: "hosts", lifecycle: "running", evidenceIds: [] },
      { id: "dns", lifecycle: "pending", evidenceIds: [] },
    ];
    expect(buildDiagnosisGuide(value.report, value.status)).toMatchObject({
      stepId: "hosts",
      tone: "running",
      title: "正在检查 Hosts",
    });
  });

  it("moves recommendations for the focus step first without reordering the rest", () => {
    const value = snapshot("run-1", 6);
    value.report.steps = [
      { id: "proxy", lifecycle: "completed", outcome: "warning", evidenceIds: ["proxy-e"] },
      { id: "dns", lifecycle: "completed", outcome: "failed", evidenceIds: ["dns-e"] },
      { id: "http", lifecycle: "completed", outcome: "warning", evidenceIds: ["http-e"] },
    ];
    value.report.recommendations = [
      { id: "proxy-r", title: "代理", action: "检查代理", evidenceIds: ["proxy-e"] },
      { id: "dns-r", title: "DNS", action: "检查 DNS", evidenceIds: ["dns-e"] },
      { id: "http-r", title: "HTTP", action: "检查 HTTP", evidenceIds: ["http-e"] },
    ];
    expect(orderDiagnosisRecommendations(value.report, "dns").map((item) => item.id)).toEqual([
      "dns-r",
      "proxy-r",
      "http-r",
    ]);
  });
});

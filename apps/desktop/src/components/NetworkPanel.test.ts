import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const panelSource = readFileSync(new URL("./NetworkPanel.vue", import.meta.url), "utf8");
const diagnosisSource = readFileSync(
  new URL("./network/NetworkDiagnosisWorkspace.vue", import.meta.url),
  "utf8",
);
const quickProbeSource = readFileSync(
  new URL("./network/NetworkQuickProbe.vue", import.meta.url),
  "utf8",
);

describe("NetworkPanel source structure", () => {
  it("defaults to diagnosis and keeps both workspaces mounted across mode switches", () => {
    expect(panelSource).toContain('ref<NetworkMode>("diagnosis")');
    expect(panelSource).toContain("访问链路诊断");
    expect(panelSource).toContain("链路诊断");
    expect(panelSource).toContain("单项探测");
    expect(panelSource).toContain("<NetworkDiagnosisWorkspace />");
    expect(panelSource).toContain("<NetworkQuickProbe />");
    expect(panelSource).toContain("v-show=\"activeMode === 'diagnosis'\"");
    expect(panelSource).toContain("v-show=\"activeMode === 'quick'\"");
    expect(panelSource).not.toMatch(/v-if="activeMode === '(?:diagnosis|quick)'"/);
  });

  it("registers the diagnosis listener before start and recovers with get", () => {
    const startBody =
      diagnosisSource.match(
        /async function startDiagnosis\(\): Promise<void> \{[\s\S]*?^}/m,
      )?.[0] ?? "";

    expect(startBody).toContain("await ensureListener()");
    expect(startBody).toContain("await diagnosisStart");
    expect(startBody.indexOf("await ensureListener()")).toBeLessThan(
      startBody.indexOf("await diagnosisStart"),
    );
    expect(startBody).toContain("await diagnosisGet(response.runId)");
  });

  it("isolates snapshots by run id and monotonic sequence", () => {
    expect(diagnosisSource).toContain(
      "event.runId !== event.snapshot.runId || event.sequence !== event.snapshot.sequence",
    );
    expect(diagnosisSource).toContain("event.runId === activeRunId.value");
    expect(diagnosisSource).toContain("incoming.sequence > current.sequence");
    expect(diagnosisSource).toContain(
      "acceptDiagnosisSnapshot(snapshot.value, incoming, activeRunId.value)",
    );
  });

  it("uses serial polling, supports cancellation and announces progress", () => {
    expect(diagnosisSource).toContain("setTimeout");
    expect(diagnosisSource).not.toContain("setInterval");
    expect(diagnosisSource).toContain("diagnosisCancel(runId)");
    expect(diagnosisSource).toContain('aria-live="polite"');
    expect(diagnosisSource).toContain("onUnmounted");
  });

  it("keeps the three legacy quick-probe channels and settings keys", () => {
    expect(quickProbeSource).toContain('"tool:network:ping-test"');
    expect(quickProbeSource).toContain('"tool:network:tcp-test"');
    expect(quickProbeSource).toContain('"tool:network:udp-test"');
    expect(quickProbeSource).toContain('"network_test_history"');
    expect(quickProbeSource).toContain('"network_test_favorites"');
  });

  it("renders UDP no-response as unverified and parses IPv6 without split colon", () => {
    expect(quickProbeSource).toContain("无响应，无法判断");
    expect(quickProbeSource).toContain("parseSocketTarget");
    expect(quickProbeSource).toContain("formatSocketTarget");
    expect(quickProbeSource).not.toMatch(/split\(\s*["']:/);
  });
  it("migrates legacy settings and persists bounded diagnosis reports", () => {
    expect(quickProbeSource).toContain("migrateNetworkDiagnosticsSettings");
    expect(quickProbeSource).toContain("NETWORK_DIAGNOSTICS_SETTINGS_KEY");
    expect(diagnosisSource).toContain("appendDiagnosticReport");
    expect(diagnosisSource).toContain("formatAccessPathReport");
    expect(diagnosisSource).toContain("复制报告");
    expect(diagnosisSource).toContain("导出报告");
  });
});

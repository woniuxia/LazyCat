import type {
  AccessPathDiagnosisRunSnapshot,
  AccessPathRunStatus,
  AccessPathStepLifecycle,
  AccessPathStepOutcome,
  AccessPathStepSnapshot,
} from "../types/access-path-diagnostics";

export type QuickProbeProtocol = "ping" | "tcp" | "udp";
export type QuickProbeAssessment = "confirmed" | "rejected" | "unverified";

export interface QuickProbeResultLike {
  reachable: boolean;
  note?: string | null;
  error?: string | null;
}

export function acceptDiagnosisSnapshot(
  current: AccessPathDiagnosisRunSnapshot | null,
  incoming: AccessPathDiagnosisRunSnapshot,
  activeRunId: string | null,
): AccessPathDiagnosisRunSnapshot | null {
  if (!activeRunId || incoming.runId !== activeRunId) return current;
  if (incoming.report.runId && incoming.report.runId !== activeRunId) return current;
  if (current?.runId === activeRunId && incoming.sequence <= current.sequence) return current;
  return incoming;
}

export function parseDnsServerList(value: string): string[] {
  const seen = new Set<string>();
  return value
    .split(/[\s,;]+/)
    .map((item) => item.trim())
    .filter((item) => item.length > 0 && !seen.has(item) && seen.add(item));
}

export function quickProbeAssessment(
  protocol: QuickProbeProtocol,
  result: QuickProbeResultLike,
): QuickProbeAssessment {
  if (protocol === "udp" && result.note) return "unverified";
  return result.reachable ? "confirmed" : "rejected";
}

export function parseQuickTarget(target: string): { host: string; port: number | null } {
  const value = target.trim();
  const bracketed = /^\[([^\]]+)](?::(\d+))?$/.exec(value);
  if (bracketed) {
    return { host: bracketed[1], port: parsePort(bracketed[2]) };
  }
  if ((value.match(/:/g) ?? []).length === 1) {
    const separator = value.lastIndexOf(":");
    const port = parsePort(value.slice(separator + 1));
    if (port !== null) return { host: value.slice(0, separator), port };
  }
  return { host: value, port: null };
}

export function diagnosisStatusLabel(status: AccessPathRunStatus): string {
  return {
    running: "诊断中",
    completed: "已完成",
    cancelled: "已取消",
    timed_out: "已超时",
    failed: "运行失败",
  }[status];
}

export function stepStateLabel(step: AccessPathStepSnapshot): string {
  if (step.lifecycle === "completed" && step.outcome) return outcomeLabel(step.outcome);
  return lifecycleLabel(step.lifecycle);
}

export function lifecycleLabel(lifecycle: AccessPathStepLifecycle): string {
  return {
    pending: "等待",
    running: "检测中",
    completed: "已完成",
    blocked: "已阻断",
    skipped: "已跳过",
    cancelled: "已取消",
  }[lifecycle];
}

export function outcomeLabel(outcome: AccessPathStepOutcome): string {
  return {
    success: "成功",
    warning: "警告",
    failed: "失败",
    unverified: "无法验证",
  }[outcome];
}

export function stepTone(step: AccessPathStepSnapshot): string {
  if (step.lifecycle === "running") return "running";
  if (step.lifecycle === "blocked" || step.lifecycle === "cancelled") return step.lifecycle;
  if (step.lifecycle === "skipped" || step.lifecycle === "pending") return "neutral";
  return step.outcome ?? "neutral";
}

function parsePort(value: string | undefined): number | null {
  if (!value || !/^\d+$/.test(value)) return null;
  const port = Number(value);
  return Number.isInteger(port) && port >= 1 && port <= 65535 ? port : null;
}

import type {
  AccessPathDiagnosisRunSnapshot,
  AccessPathReport,
  AccessPathRecommendation,
  AccessPathRunStatus,
  AccessPathStepId,
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

export type DiagnosisPhaseId = "route" | "transport" | "application";
export type DiagnosisPhaseState =
  | "pending"
  | "running"
  | "success"
  | "warning"
  | "failed"
  | "cancelled";
export type DiagnosisGuideTone = "neutral" | "running" | "success" | "warning" | "failed";

export interface DiagnosisPhaseDefinition {
  id: DiagnosisPhaseId;
  order: number;
  label: string;
  description: string;
  stepIds: readonly AccessPathStepId[];
}

export interface DiagnosisGuide {
  stepId: AccessPathStepId | null;
  tone: DiagnosisGuideTone;
  eyebrow: string;
  title: string;
  description: string;
}

export const DIAGNOSIS_PHASES: readonly DiagnosisPhaseDefinition[] = [
  {
    id: "route",
    order: 1,
    label: "路径判定",
    description: "先确认代理策略、Hosts 覆盖和名称解析",
    stepIds: ["proxy", "hosts", "dns"],
  },
  {
    id: "transport",
    order: 2,
    label: "连接建立",
    description: "再验证目标端口、TLS 握手和证书身份",
    stepIds: ["tcp", "tls"],
  },
  {
    id: "application",
    order: 3,
    label: "服务响应",
    description: "最后检查 HTTP 状态、重定向和服务响应",
    stepIds: ["http"],
  },
] as const;

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

export function accessPathStepLabel(stepId: AccessPathStepId): string {
  return {
    proxy: "代理决策",
    hosts: "Hosts",
    dns: "DNS 解析",
    tcp: "TCP 连接",
    tls: "TLS 握手",
    http: "HTTP 请求",
  }[stepId];
}

export function accessPathStepDescription(stepId: AccessPathStepId): string {
  return {
    proxy: "配置来源与当前目标的直连 / 代理路径",
    hosts: "本机 Hosts 命中、重复、冲突和格式",
    dns: "系统解析与指定 DNS 查询结果",
    tcp: "逐地址族建立连接并保留原始错误",
    tls: "SNI、证书链、主机名校验与信任结果",
    http: "受限 HEAD / GET、重定向与响应状态",
  }[stepId];
}

export function diagnosisPhaseState(
  phase: DiagnosisPhaseDefinition,
  steps: readonly AccessPathStepSnapshot[],
): DiagnosisPhaseState {
  const phaseSteps = phase.stepIds
    .map((stepId) => steps.find((step) => step.id === stepId))
    .filter((step): step is AccessPathStepSnapshot => Boolean(step));
  if (phaseSteps.some((step) => step.lifecycle === "running")) return "running";
  if (phaseSteps.some((step) => step.lifecycle === "blocked" || step.outcome === "failed"))
    return "failed";
  if (phaseSteps.some((step) => step.outcome === "warning" || step.outcome === "unverified"))
    return "warning";
  if (phaseSteps.some((step) => step.lifecycle === "cancelled")) return "cancelled";
  if (
    phaseSteps.length > 0 &&
    phaseSteps.every((step) => ["completed", "skipped"].includes(step.lifecycle))
  )
    return "success";
  return "pending";
}

export function diagnosisPhaseStateLabel(state: DiagnosisPhaseState): string {
  return {
    pending: "等待",
    running: "进行中",
    success: "已通过",
    warning: "需关注",
    failed: "已定位异常",
    cancelled: "未完成",
  }[state];
}

export function buildDiagnosisGuide(
  report: AccessPathReport,
  status: AccessPathRunStatus,
): DiagnosisGuide {
  const activeStep = report.steps.find((step) => step.lifecycle === "running");
  if (activeStep) {
    return {
      stepId: activeStep.id,
      tone: "running",
      eyebrow: "当前进度",
      title: `正在检查 ${accessPathStepLabel(activeStep.id)}`,
      description: accessPathStepDescription(activeStep.id),
    };
  }

  const failedStep = report.steps.find(
    (step) => step.lifecycle === "blocked" || step.outcome === "failed",
  );
  if (failedStep) return guideForFinding(report, failedStep, "failed", "优先排查");

  const warningStep = report.steps.find(
    (step) => step.outcome === "warning" || step.outcome === "unverified",
  );
  if (warningStep) return guideForFinding(report, warningStep, "warning", "优先确认");

  const cancelledStep = report.steps.find((step) => step.lifecycle === "cancelled");
  if (cancelledStep || status === "cancelled") {
    return {
      stepId: cancelledStep?.id ?? null,
      tone: "neutral",
      eyebrow: "诊断中止",
      title: cancelledStep
        ? `诊断在 ${accessPathStepLabel(cancelledStep.id)} 前停止`
        : "诊断已取消",
      description: "已完成步骤仍保留在报告中，可重新运行以补齐后续证据。",
    };
  }

  if (status === "running") {
    return {
      stepId: null,
      tone: "running",
      eyebrow: "当前进度",
      title: "正在准备下一项检查",
      description: "诊断将按照路径判定、连接建立、服务响应的顺序继续。",
    };
  }

  return {
    stepId: null,
    tone: "success",
    eyebrow: "定位结果",
    title: "访问链路完整通过",
    description: "当前参数下未发现失败、阻断或需要确认的步骤。",
  };
}

export function orderDiagnosisRecommendations(
  report: AccessPathReport,
  focusStepId: AccessPathStepId | null,
): AccessPathRecommendation[] {
  if (!focusStepId) return report.recommendations;
  const focusEvidenceIds = new Set(
    report.steps.find((step) => step.id === focusStepId)?.evidenceIds ?? [],
  );
  if (focusEvidenceIds.size === 0) return report.recommendations;
  return report.recommendations
    .map((item, index) => ({
      item,
      index,
      focused: item.evidenceIds.some((evidenceId) => focusEvidenceIds.has(evidenceId)),
    }))
    .sort((left, right) => Number(right.focused) - Number(left.focused) || left.index - right.index)
    .map(({ item }) => item);
}

function guideForFinding(
  report: AccessPathReport,
  step: AccessPathStepSnapshot,
  tone: "warning" | "failed",
  eyebrow: string,
): DiagnosisGuide {
  const evidenceIds = new Set(step.evidenceIds);
  const conclusion = report.conclusions.find((item) =>
    item.evidenceIds.some((evidenceId) => evidenceIds.has(evidenceId)),
  );
  return {
    stepId: step.id,
    tone,
    eyebrow,
    title: `${accessPathStepLabel(step.id)}${tone === "failed" ? "未通过" : "需要确认"}`,
    description:
      step.error?.message ??
      conclusion?.message ??
      `${accessPathStepLabel(step.id)}未形成完整的成功证据，请展开该步骤核对证据。`,
  };
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

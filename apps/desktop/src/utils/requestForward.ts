import type {
  RequestForwardLogOutcome,
  RequestForwardProtocol,
  RequestForwardRuleForm,
  RequestForwardRuleWriteInput,
  RequestForwardRuntimeState,
} from "../types/request-forward";

export const DEFAULT_REQUEST_FORWARD_FORM: RequestForwardRuleForm = {
  name: "",
  protocol: "http",
  bindHost: "127.0.0.1",
  listenPort: 8080,
  targetUrl: "",
  targetHost: null,
  targetPort: null,
  captureHttpHeaders: true,
  captureHttpBody: false,
};

export function getDefaultRequestForwardForm(): RequestForwardRuleForm {
  return { ...DEFAULT_REQUEST_FORWARD_FORM };
}

export interface RequestForwardSelectionIntentState {
  selectionToken: number;
  selectedId: number | null;
  draft: boolean;
}

export interface RequestForwardMutationIntent extends RequestForwardSelectionIntentState {
  targetId: number | null;
}

export function captureRequestForwardMutationIntent(
  selection: RequestForwardSelectionIntentState,
  targetId: number | null,
): RequestForwardMutationIntent {
  return { ...selection, targetId };
}

export function isRequestForwardMutationIntentCurrent(
  intent: RequestForwardMutationIntent,
  current: RequestForwardSelectionIntentState,
): boolean {
  return (
    intent.selectionToken === current.selectionToken &&
    intent.selectedId === current.selectedId &&
    intent.draft === current.draft
  );
}

export async function applyRequestForwardMutationResult<T>(
  operation: Promise<T>,
  intent: RequestForwardMutationIntent,
  current: () => RequestForwardSelectionIntentState,
  apply: (value: T) => void | Promise<void>,
): Promise<{ value: T; applied: boolean }> {
  const value = await operation;
  const applied = isRequestForwardMutationIntentCurrent(intent, current());
  if (applied) await apply(value);
  return { value, applied };
}

export function normalizeRequestForwardRuleForm(
  form: RequestForwardRuleForm,
): RequestForwardRuleForm {
  const normalized: RequestForwardRuleForm = {
    ...form,
    name: form.name.trim(),
    bindHost: form.bindHost.trim(),
    targetUrl: form.targetUrl?.trim() || null,
    targetHost: form.targetHost?.trim() || null,
  };
  if (normalized.protocol === "http") {
    normalized.targetHost = null;
    normalized.targetPort = null;
  } else {
    normalized.targetUrl = null;
  }
  return normalized;
}

export function validateRequestForwardRuleForm(form: RequestForwardRuleForm): string[] {
  const normalized = normalizeRequestForwardRuleForm(form);
  const missing: string[] = [];
  if (!normalized.name) missing.push("name");
  if (!isIpLiteral(normalized.bindHost)) missing.push("bindHost");
  if (
    !Number.isInteger(normalized.listenPort) ||
    normalized.listenPort < 1 ||
    normalized.listenPort > 65535
  ) {
    missing.push("listenPort");
  }
  if (normalized.protocol === "http") {
    if (!isValidHttpTargetUrl(normalized.targetUrl)) missing.push("targetUrl");
  } else {
    if (!normalized.targetHost) missing.push("targetHost");
    if (
      normalized.targetPort == null ||
      !Number.isInteger(normalized.targetPort) ||
      normalized.targetPort < 1 ||
      normalized.targetPort > 65535
    ) {
      missing.push("targetPort");
    }
  }
  return missing;
}

export function toRequestForwardRuleWriteInput(
  form: RequestForwardRuleForm,
): RequestForwardRuleWriteInput {
  const normalized = normalizeRequestForwardRuleForm(form);
  return {
    name: normalized.name,
    protocol: normalized.protocol,
    bindHost: normalized.bindHost,
    listenPort: normalized.listenPort,
    targetUrl: normalized.targetUrl,
    targetHost: normalized.targetHost,
    targetPort: normalized.targetPort,
    captureHttpHeaders: normalized.captureHttpHeaders,
    captureHttpBody: normalized.captureHttpBody,
  };
}

export function isRequestForwardRuleReadonly(state: RequestForwardRuntimeState): boolean {
  return state === "starting" || state === "running" || state === "stopping";
}

export function isExposedForwardBindHost(bindHost: string): boolean {
  const host = bindHost.trim();
  if (!isIpLiteral(host)) return false;
  if (isIpv4Literal(host)) return Number(host.split(".")[0]) !== 127;
  return normalizeIpv6Literal(host) !== "::1";
}

export function formatRequestForwardEndpoint(
  host?: string | null,
  port?: number | null,
): string {
  const trimmedHost = host?.trim();
  if (!trimmedHost || port == null || !Number.isInteger(port) || port < 1 || port > 65535) {
    return "—";
  }
  return trimmedHost.includes(":") && !trimmedHost.startsWith("[")
    ? `[${trimmedHost}]:${port}`
    : `${trimmedHost}:${port}`;
}

export function formatRequestForwardRuleSummary(form: RequestForwardRuleForm): string {
  const source = formatRequestForwardEndpoint(form.bindHost, form.listenPort);
  const target =
    form.protocol === "http"
      ? form.targetUrl?.trim() || "—"
      : formatRequestForwardEndpoint(form.targetHost, form.targetPort);
  return `${source} → ${target}`;
}

export function getForwardEventLabel(protocol: RequestForwardProtocol): string {
  switch (protocol) {
    case "http":
      return "请求数";
    case "tcp":
      return "连接数";
    case "udp":
      return "数据报数";
  }
}

type RequestForwardBatchOperation = "start" | "stop";

export function getRequestForwardBatchMessage(
  operation: RequestForwardBatchOperation,
  result: { requested: number; succeeded: number; failed: number },
): string {
  const action = operation === "start" ? "启动" : "停止";
  if (result.requested === 0) return `没有可${action}的规则`;
  if (result.failed === 0) return `已${action} ${result.succeeded} 条规则`;
  return `已${action} ${result.succeeded} 条规则，${result.failed} 条失败`;
}

export type RequestForwardLogTone = "success" | "danger";

export function getRequestForwardLogTone(
  status: RequestForwardLogOutcome,
): RequestForwardLogTone {
  return status === "success" ? "success" : "danger";
}

function isValidHttpTargetUrl(value: string | null): boolean {
  const normalized = value?.trim();
  if (!normalized || normalized.includes("?") || normalized.includes("#")) return false;
  try {
    const url = new URL(normalized);
    return (
      (url.protocol === "http:" || url.protocol === "https:") &&
      Boolean(url.hostname) &&
      !url.search &&
      !url.hash &&
      url.port !== "0"
    );
  } catch {
    return false;
  }
}

function isIpLiteral(value: string): boolean {
  return isIpv4Literal(value) || normalizeIpv6Literal(value) !== null;
}

function isIpv4Literal(value: string): boolean {
  const parts = value.split(".");
  return (
    parts.length === 4 &&
    parts.every((part) => /^\d{1,3}$/.test(part) && Number(part) >= 0 && Number(part) <= 255)
  );
}

function normalizeIpv6Literal(value: string): string | null {
  if (!value.includes(":") || value.includes("[") || value.includes("]")) return null;
  try {
    return new URL(`http://[${value}]/`).hostname.slice(1, -1).toLowerCase();
  } catch {
    return null;
  }
}

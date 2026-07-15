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
  if (!normalized.bindHost) missing.push("bindHost");
  if (!normalized.listenPort || normalized.listenPort < 1 || normalized.listenPort > 65535) {
    missing.push("listenPort");
  }
  if (normalized.protocol === "http") {
    if (!normalized.targetUrl) missing.push("targetUrl");
  } else {
    if (!normalized.targetHost) missing.push("targetHost");
    if (!normalized.targetPort || normalized.targetPort < 1 || normalized.targetPort > 65535) {
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
  const host = bindHost.trim().toLowerCase();
  return host === "0.0.0.0" || host === "::";
}

export function formatRequestForwardEndpoint(host: string, port: number): string {
  const trimmedHost = host.trim();
  return trimmedHost.includes(":") && !trimmedHost.startsWith("[")
    ? `[${trimmedHost}]:${port}`
    : `${trimmedHost}:${port}`;
}

export function formatRequestForwardRuleSummary(form: RequestForwardRuleForm): string {
  const source = formatRequestForwardEndpoint(form.bindHost, form.listenPort);
  const target =
    form.protocol === "http"
      ? form.targetUrl?.trim() || "—"
      : formatRequestForwardEndpoint(form.targetHost?.trim() || "—", form.targetPort ?? 0);
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

export function getRequestForwardBatchMessage(result: {
  requested: number;
  succeeded: number;
  failed: number;
}): string {
  if (result.requested === 0) return "没有可处理的规则";
  if (result.failed === 0) return `已启动 ${result.succeeded} 条规则`;
  return `已启动 ${result.succeeded} 条规则，${result.failed} 条失败`;
}

export type RequestForwardLogTone = "success" | "danger" | "warning" | "info";

export function getRequestForwardLogTone(
  status: RequestForwardLogOutcome | "warn" | "info",
): RequestForwardLogTone {
  switch (status) {
    case "success":
      return "success";
    case "error":
      return "danger";
    case "warn":
      return "warning";
    default:
      return "info";
  }
}

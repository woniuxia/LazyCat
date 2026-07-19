import type {
  RequestForwardError,
  RequestForwardErrorCode,
  RequestForwardLogOutcome,
  RequestForwardProtocol,
  RequestForwardRule,
  RequestForwardRuleForm,
  RequestForwardRuleWriteInput,
  RequestForwardRuntimeState,
  RequestForwardRecoveryAction,
} from "../types/request-forward";

const REQUEST_FORWARD_ERROR_MARKER = "lazycat.request_forward.error";
const REQUEST_FORWARD_ERROR_CODES = new Set<RequestForwardErrorCode>([
  "listener_in_use",
  "dns_failed",
  "target_unreachable",
  "tls_failed",
  "self_forward",
  "invalid_config",
  "lifecycle_conflict",
  "persistence_failed",
  "unknown",
]);
const REQUEST_FORWARD_RUNTIME_STATES = new Set<RequestForwardRuntimeState>([
  "stopped",
  "starting",
  "running",
  "stopping",
  "failed",
]);

export function parseRequestForwardError(
  input: unknown,
  fallbackState: RequestForwardRuntimeState,
): RequestForwardError {
  const original = input instanceof Error ? input.message : String(input);
  for (const candidate of requestForwardErrorJsonCandidates(original)) {
    try {
      const parsed = JSON.parse(candidate) as Record<string, unknown>;
      if (
        parsed.marker === REQUEST_FORWARD_ERROR_MARKER &&
        parsed.version === 1 &&
        typeof parsed.code === "string" &&
        REQUEST_FORWARD_ERROR_CODES.has(parsed.code as RequestForwardErrorCode) &&
        typeof parsed.message === "string" &&
        typeof parsed.state === "string" &&
        REQUEST_FORWARD_RUNTIME_STATES.has(parsed.state as RequestForwardRuntimeState)
      ) {
        return {
          code: parsed.code as RequestForwardErrorCode,
          message: parsed.message,
          state: parsed.state as RequestForwardRuntimeState,
        };
      }
    } catch {
      // Historical text and unrelated JS errors remain readable below.
    }
  }
  return { code: "unknown", message: original, state: fallbackState };
}

export function getRequestForwardRecoveryActions(
  error: RequestForwardError,
  suggestedListenPort: number | null,
): RequestForwardRecoveryAction[] {
  switch (error.code) {
    case "listener_in_use":
      return suggestedListenPort == null
        ? ["restart", "edit", "check_target"]
        : ["restart", "edit", "check_target", "use_suggested_port"];
    case "dns_failed":
    case "target_unreachable":
    case "tls_failed":
      return ["restart", "edit", "check_target"];
    case "self_forward":
    case "invalid_config":
      return ["edit", "check_target"];
    case "lifecycle_conflict":
    case "persistence_failed":
    case "unknown":
      return ["restart", "edit"];
  }
}

export function getRequestForwardErrorSummary(code: RequestForwardErrorCode): string {
  const summaries: Record<RequestForwardErrorCode, string> = {
    listener_in_use: "监听端口已被占用",
    dns_failed: "目标域名解析失败",
    target_unreachable: "目标服务不可达",
    tls_failed: "目标 TLS 校验失败",
    self_forward: "目标指向当前监听地址",
    invalid_config: "规则配置无效",
    lifecycle_conflict: "运行状态冲突",
    persistence_failed: "运行意图保存失败",
    unknown: "运行时发生未知错误",
  };
  return summaries[code];
}

function requestForwardErrorJsonCandidates(value: string): string[] {
  const candidates = [value];
  let start = value.indexOf("{");
  while (start >= 0) {
    let depth = 0;
    let inString = false;
    let escaped = false;
    for (let index = start; index < value.length; index += 1) {
      const character = value[index];
      if (inString) {
        if (escaped) escaped = false;
        else if (character === "\\") escaped = true;
        else if (character === '"') inString = false;
        continue;
      }
      if (character === '"') inString = true;
      else if (character === "{") depth += 1;
      else if (character === "}" && --depth === 0) {
        candidates.push(value.slice(start, index + 1));
        break;
      }
    }
    start = value.indexOf("{", start + 1);
  }
  return candidates;
}

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

const REQUEST_FORWARD_LOG_PAGE_SIZE = 30;
const REQUEST_FORWARD_LOG_LIMIT = 1000;

export const DEFAULT_REQUEST_FORWARD_INSPECTOR_WIDTH = 420;
export const MIN_REQUEST_FORWARD_INSPECTOR_WIDTH = 320;
export const DEFAULT_REQUEST_FORWARD_RULE_LIST_WIDTH = 260;
export const MIN_REQUEST_FORWARD_RULE_LIST_WIDTH = 220;
export const MAX_REQUEST_FORWARD_RULE_LIST_WIDTH = 420;

export function clampRequestForwardRuleListWidth(
  preferred: unknown,
  availableWidth: number,
): number {
  const parsed = typeof preferred === "number" ? preferred : Number(preferred);
  const width = Number.isFinite(parsed)
    ? parsed
    : DEFAULT_REQUEST_FORWARD_RULE_LIST_WIDTH;
  const safeAvailable = Number.isFinite(availableWidth)
    ? Math.max(0, availableWidth)
    : DEFAULT_REQUEST_FORWARD_RULE_LIST_WIDTH * 3;
  const maximum = Math.max(
    MIN_REQUEST_FORWARD_RULE_LIST_WIDTH,
    Math.min(MAX_REQUEST_FORWARD_RULE_LIST_WIDTH, Math.floor(safeAvailable - 480)),
  );
  return Math.min(
    maximum,
    Math.max(MIN_REQUEST_FORWARD_RULE_LIST_WIDTH, Math.round(width)),
  );
}

export function clampRequestForwardInspectorWidth(
  preferred: unknown,
  availableWidth: number,
): number {
  const parsed = typeof preferred === "number" ? preferred : Number(preferred);
  const width = Number.isFinite(parsed)
    ? parsed
    : DEFAULT_REQUEST_FORWARD_INSPECTOR_WIDTH;
  const safeAvailable = Number.isFinite(availableWidth)
    ? Math.max(0, availableWidth)
    : DEFAULT_REQUEST_FORWARD_INSPECTOR_WIDTH * 2;
  const maximum = Math.max(
    MIN_REQUEST_FORWARD_INSPECTOR_WIDTH,
    Math.floor(safeAvailable * 0.5),
  );
  return Math.min(
    maximum,
    Math.max(MIN_REQUEST_FORWARD_INSPECTOR_WIDTH, Math.round(width)),
  );
}

export function retainRequestForwardSelectedLogId(
  selectedId: number | null,
  items: Array<{ id: number }>,
): number | null {
  if (selectedId == null) return null;
  return items.some((item) => item.id === selectedId) ? selectedId : null;
}

export function getRequestForwardLogProbeLimit(loadedCount: number): number {
  return Math.min(
    REQUEST_FORWARD_LOG_LIMIT,
    Math.max(REQUEST_FORWARD_LOG_PAGE_SIZE, loadedCount + REQUEST_FORWARD_LOG_PAGE_SIZE),
  );
}

export function getRequestForwardLogTargetCount(input: {
  loadedCount: number;
  previousTotal: number;
  nextTotal: number;
}): number {
  const added = Math.max(0, input.nextTotal - input.previousTotal);
  return Math.min(
    REQUEST_FORWARD_LOG_LIMIT,
    input.nextTotal,
    Math.max(REQUEST_FORWARD_LOG_PAGE_SIZE, input.loadedCount + added),
  );
}

export function getDefaultRequestForwardForm(): RequestForwardRuleForm {
  return { ...DEFAULT_REQUEST_FORWARD_FORM };
}

export function duplicateRequestForwardRuleForm(
  rule: RequestForwardRule,
  listenPort: number,
): RequestForwardRuleForm {
  const suffix = " 副本";
  const maximumBaseLength = 80 - suffix.length;
  let baseName = "";
  for (const character of rule.name.trim()) {
    if (baseName.length + character.length > maximumBaseLength) break;
    baseName += character;
  }
  const name = `${baseName}${suffix}`;
  return toRequestForwardRuleWriteInput({ ...rule, name, listenPort });
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

export interface RequestForwardCommandExamples {
  powershell: string;
  curl: string;
}

export function getRequestForwardLocalEndpoint(
  form: RequestForwardRuleForm,
): string {
  return formatRequestForwardEndpoint(form.bindHost, form.listenPort);
}

export function getRequestForwardLocalUrl(
  form: RequestForwardRuleForm,
): string | null {
  if (form.protocol !== "http") return null;
  const bindHost = form.bindHost.trim();
  const accessHost = bindHost === "0.0.0.0"
    ? "127.0.0.1"
    : bindHost === "::"
      ? "::1"
      : bindHost;
  const endpoint = formatRequestForwardEndpoint(accessHost, form.listenPort);
  return endpoint === "—" ? null : `http://${endpoint}`;
}

export function getRequestForwardCommandExamples(
  form: RequestForwardRuleForm,
): RequestForwardCommandExamples | null {
  const url = getRequestForwardLocalUrl(form);
  if (!url) return null;
  const powershellUrl = url.replaceAll("'", "''");
  const shellUrl = url.replaceAll("'", `'"'"'`);
  return {
    powershell: `Invoke-WebRequest -UseBasicParsing -Uri '${powershellUrl}'`,
    curl: `curl --url '${shellUrl}'`,
  };
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

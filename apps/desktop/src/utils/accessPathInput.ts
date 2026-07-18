import type {
  AccessPathInputOptions,
  AccessPathProtocol,
  AccessPathTargetKind,
  NormalizedAccessPathTarget,
} from "../types/access-path-diagnostics";

const DEFAULT_PROTOCOL: AccessPathProtocol = "https";
const DEFAULT_PORTS: Record<AccessPathProtocol, number> = { http: 80, https: 443 };

export class AccessPathInputError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "AccessPathInputError";
    this.code = code;
  }
}

function isIpv4(value: string): boolean {
  const parts = value.split(".");
  return (
    parts.length === 4 &&
    parts.every((part) => /^(?:0|[1-9]\d*)$/.test(part) && Number(part) >= 0 && Number(part) <= 255)
  );
}

function isIpv6(value: string): boolean {
  if (!value || value.includes("%")) return false;
  try {
    return new URL("http://[" + value + "]/").hostname.startsWith("[");
  } catch {
    return false;
  }
}

function isValidHostname(value: string): boolean {
  const hostname = value.replace(/\.$/, "");
  return (
    hostname.length > 0 &&
    hostname.length <= 253 &&
    hostname
      .split(".")
      .every(
        (label) => label.length <= 63 && /^[a-zA-Z0-9](?:[a-zA-Z0-9-]*[a-zA-Z0-9])?$/.test(label),
      )
  );
}

function getTargetKind(hostname: string): AccessPathTargetKind {
  if (isIpv4(hostname)) return "ipv4";
  if (isIpv6(hostname)) return "ipv6";
  return "hostname";
}

function normalizeHostname(hostname: string): {
  hostname: string;
  targetKind: AccessPathTargetKind;
} {
  const normalized = hostname.trim().replace(/^\[|\]$/g, "");
  if (!normalized) throw new AccessPathInputError("empty_host", "请输入域名或 IP 地址");
  const targetKind = getTargetKind(normalized);
  if (targetKind === "hostname") {
    if (!isValidHostname(normalized)) {
      throw new AccessPathInputError("invalid_host", "主机名格式无效");
    }
    return { hostname: normalized.replace(/\.$/, "").toLowerCase(), targetKind };
  }
  if (targetKind === "ipv6") {
    const hostname = new URL("http://[" + normalized + "]/").hostname.slice(1, -1).toLowerCase();
    return { hostname, targetKind };
  }
  return { hostname: normalized.toLowerCase(), targetKind };
}

function normalizePort(value: string | number | undefined, protocol: AccessPathProtocol): number {
  if (value === undefined || value === "") return DEFAULT_PORTS[protocol];
  const port = typeof value === "number" ? value : Number(value);
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new AccessPathInputError("invalid_port", "端口范围必须是 1-65535");
  }
  return port;
}

function normalizeProtocol(
  value: string | undefined,
  fallback: AccessPathProtocol,
): AccessPathProtocol {
  const protocol = (value ?? fallback).replace(/:$/, "").toLowerCase();
  if (protocol !== "http" && protocol !== "https") {
    throw new AccessPathInputError("unsupported_protocol", "仅支持 HTTP 或 HTTPS");
  }
  return protocol;
}

function normalizeOverride(value: string | null | undefined, label: string): string | null {
  if (value === undefined || value === null || !value.trim()) return null;
  const normalized = value.trim();
  if (/\s/.test(normalized)) {
    throw new AccessPathInputError("invalid_override", label + "不能包含空白字符");
  }
  return normalized;
}

function normalizeSni(value: string | null | undefined): string | null {
  const normalized = normalizeOverride(value, "SNI");
  if (normalized === null) return null;
  const host = normalizeHostname(normalized);
  if (host.targetKind !== "hostname") {
    throw new AccessPathInputError("invalid_sni", "SNI 必须是主机名");
  }
  return host.hostname;
}

function normalizeVerifyHostname(value: string | null | undefined): string | null {
  const normalized = normalizeOverride(value, "证书校验名");
  if (normalized === null) return null;
  return normalizeHostname(normalized).hostname;
}

function normalizeHttpHost(value: string | null | undefined): string | null {
  const normalized = normalizeOverride(value, "HTTP Host");
  if (normalized === null) return null;

  let hostValue = normalized;
  let portValue: string | undefined;
  if (normalized.startsWith("[")) {
    const match = /^\[([^\]]+)\](?::(\d+))?$/.exec(normalized);
    if (!match) throw new AccessPathInputError("invalid_http_host", "HTTP Host 格式无效");
    hostValue = match[1];
    portValue = match[2];
  } else {
    const firstColon = normalized.indexOf(":");
    const lastColon = normalized.lastIndexOf(":");
    if (firstColon !== lastColon) {
      throw new AccessPathInputError("invalid_http_host", "HTTP Host 中的 IPv6 地址必须使用方括号");
    }
    if (lastColon >= 0) {
      hostValue = normalized.slice(0, lastColon);
      portValue = normalized.slice(lastColon + 1);
    }
  }

  const host = normalizeHostname(hostValue);
  const authority = formatAuthority(host.hostname, host.targetKind);
  return portValue === undefined ? authority : authority + ":" + normalizePort(portValue, "http");
}

function normalizeConnectionIp(value: string | null | undefined): string | null {
  const normalized = normalizeOverride(value, "连接 IP");
  if (normalized === null) return null;
  const host = normalizeHostname(normalized);
  if (host.targetKind === "hostname") {
    throw new AccessPathInputError("invalid_connection_ip", "连接 IP 必须是 IPv4 或 IPv6 地址");
  }
  return host.hostname;
}

function formatAuthority(hostname: string, targetKind: AccessPathTargetKind): string {
  return targetKind === "ipv6" ? "[" + hostname + "]" : hostname;
}

function formatHttpHost(
  hostname: string,
  targetKind: AccessPathTargetKind,
  protocol: AccessPathProtocol,
  port: number,
): string {
  const authority = formatAuthority(hostname, targetKind);
  return port === DEFAULT_PORTS[protocol] ? authority : authority + ":" + port;
}

function parseInputUrl(rawInput: string, defaultProtocol: AccessPathProtocol): URL {
  const hasScheme = /^[a-zA-Z][a-zA-Z\d+.-]*:\/\//.test(rawInput);
  const authority = isIpv6(rawInput) ? "[" + rawInput + "]" : rawInput;
  const candidate = hasScheme ? rawInput : defaultProtocol + "://" + authority;
  let parsed: URL;
  try {
    parsed = new URL(candidate);
  } catch {
    const portMatch = /:(\d+)(?:[/?#]|$)/.exec(rawInput);
    if (portMatch && Number(portMatch[1]) > 65535) {
      throw new AccessPathInputError("invalid_port", "端口范围必须是 1-65535");
    }
    throw new AccessPathInputError("invalid_input", "无法解析目标地址");
  }
  if (parsed.username || parsed.password) {
    throw new AccessPathInputError("credentials_not_allowed", "目标地址不能包含用户名或密码");
  }
  return parsed;
}

export function normalizeAccessPathInput(
  input: string,
  options: AccessPathInputOptions = {},
): NormalizedAccessPathTarget {
  const rawInput = input.trim();
  if (!rawInput) throw new AccessPathInputError("empty_input", "请输入目标地址");

  const defaultProtocol = options.defaultProtocol ?? DEFAULT_PROTOCOL;
  const parsed = parseInputUrl(rawInput, defaultProtocol);
  const protocol = normalizeProtocol(parsed.protocol, defaultProtocol);
  const { hostname, targetKind } = normalizeHostname(parsed.hostname);
  const port = normalizePort(parsed.port, protocol);
  const path = (parsed.pathname || "/") + parsed.search;
  const hostHeader = formatHttpHost(hostname, targetKind, protocol, port);
  const sniOverride = normalizeSni(options.sni);
  const verifyHostnameOverride = normalizeVerifyHostname(options.verifyHostname);
  const httpHostOverride = normalizeHttpHost(options.httpHost);
  const connectionIp = normalizeConnectionIp(options.connectionIp);

  const sni = sniOverride ?? (targetKind === "hostname" ? hostname : null);
  const verifyHostname = verifyHostnameOverride ?? hostname;
  const httpHost = httpHostOverride ?? hostHeader;
  const authority = formatAuthority(hostname, targetKind);
  const url =
    protocol + "://" + authority + (port === DEFAULT_PORTS[protocol] ? "" : ":" + port) + path;

  return {
    rawInput,
    protocol,
    hostname,
    targetKind,
    port,
    path,
    url,
    sni,
    verifyHostname,
    httpHost,
    connectionIp,
  };
}

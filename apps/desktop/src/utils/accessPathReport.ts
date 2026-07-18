import type { AccessPathJsonValue, AccessPathReport } from "../types/access-path-diagnostics";

const SENSITIVE_QUERY =
  /(?:token|key|secret|password|passwd|auth|authorization|cookie|session|sig|signature|api[_-]?key|access[_-]?token|client[_-]?secret)/i;
const SENSITIVE_KEY = /authorization|cookie|set-cookie|password|credential|proxy[_-]?url/i;

function redactUrl(value: string): string {
  const proxySafe = value.replace(/(https?:\/\/)([^/@\s]+):([^/@\s]+)@/gi, "$1***:***@");
  try {
    const url = new URL(proxySafe);
    for (const key of [...url.searchParams.keys()]) {
      if (SENSITIVE_QUERY.test(key)) url.searchParams.set(key, "[REDACTED]");
    }
    return url.toString();
  } catch {
    return proxySafe.replace(/([?&])([^=&#\s]+)=([^&#\s]*)/g, (match, prefix, key) => {
      let decodedKey = key;
      try {
        decodedKey = decodeURIComponent(key);
      } catch {
        // Keep the raw key for matching malformed input.
      }
      return SENSITIVE_QUERY.test(decodedKey) ? `${prefix}${key}=[REDACTED]` : match;
    });
  }
}

function sanitizeValue(value: unknown, key?: string): AccessPathJsonValue {
  if (key && SENSITIVE_KEY.test(key)) return "[REDACTED]";
  if (typeof value === "string") return redactUrl(value);
  if (Array.isArray(value)) return value.map((item) => sanitizeValue(item));
  if (value && typeof value === "object") {
    const result: Record<string, AccessPathJsonValue> = {};
    for (const [childKey, childValue] of Object.entries(value)) {
      result[childKey] = sanitizeValue(childValue, childKey);
    }
    return result;
  }
  if (value === null || typeof value === "boolean" || typeof value === "number") return value;
  return String(value);
}

export function sanitizeAccessPathReport(report: AccessPathReport): AccessPathReport {
  return sanitizeValue(report) as unknown as AccessPathReport;
}

export function formatAccessPathReport(report: AccessPathReport): string {
  return JSON.stringify(sanitizeAccessPathReport(report), null, 2);
}

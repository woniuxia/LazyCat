import type {
  AccessPathProtocol,
  AccessPathProxyProfile,
  AccessPathReport,
} from "../types/access-path-diagnostics";
import { sanitizeAccessPathReport } from "./accessPathReport";
import {
  normalizeNetworkFavorites,
  type NetworkFavoriteItem,
  type NetworkFavoriteProtocol,
} from "./networkFavorites";

export const NETWORK_DIAGNOSTICS_SETTINGS_KEY = "network_diagnostics";
export const NETWORK_DIAGNOSTICS_SCHEMA_VERSION = 1;
export const MAX_NETWORK_HISTORY = 50;
export const MAX_DIAGNOSTIC_REPORTS = 10;
export const MAX_DIAGNOSTIC_REPORT_BYTES = 256 * 1024;
export const MAX_DIAGNOSTIC_TOTAL_BYTES = 1024 * 1024;

export interface NetworkDiagnosisAdvancedParams {
  defaultProtocol: AccessPathProtocol;
  connectionIp: string;
  sni: string;
  verifyHostname: string;
  httpHost: string;
  dnsServers: string;
  proxyProfile: AccessPathProxyProfile;
  stepTimeoutMs: number;
  overallTimeoutMs: number;
}

export const DEFAULT_NETWORK_DIAGNOSIS_ADVANCED_PARAMS: NetworkDiagnosisAdvancedParams = {
  defaultProtocol: "https",
  connectionIp: "",
  sni: "",
  verifyHostname: "",
  httpHost: "",
  dnsServers: "",
  proxyProfile: "auto",
  stepTimeoutMs: 5000,
  overallTimeoutMs: 30000,
};

export interface NetworkHistorySummary {
  id: string;
  checkedAt: number;
  protocol: NetworkFavoriteProtocol;
  target: string;
  timeoutMs: number;
  reachable: boolean;
  latencyMs: number;
  statusCode: number | null;
  error: string | null;
  note?: string | null;
  outcome?: "success" | "failed" | "unverified";
}

export interface NetworkDiagnosticsSettings {
  schemaVersion: 1;
  favorites: NetworkFavoriteItem[];
  history: NetworkHistorySummary[];
  reports: AccessPathReport[];
  diagnosisAdvancedParams: NetworkDiagnosisAdvancedParams;
}

function normalizeText(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function normalizeTimeout(
  value: unknown,
  minimum: number,
  maximum: number,
  fallback: number,
): number {
  return typeof value === "number" &&
    Number.isInteger(value) &&
    value >= minimum &&
    value <= maximum
    ? value
    : fallback;
}

export function normalizeNetworkDiagnosisAdvancedParams(
  raw: unknown,
): NetworkDiagnosisAdvancedParams {
  const value = raw && typeof raw === "object" ? (raw as Record<string, unknown>) : {};
  return {
    defaultProtocol: value.defaultProtocol === "http" ? "http" : "https",
    connectionIp: normalizeText(value.connectionIp),
    sni: normalizeText(value.sni),
    verifyHostname: normalizeText(value.verifyHostname),
    httpHost: normalizeText(value.httpHost),
    dnsServers: normalizeText(value.dnsServers),
    proxyProfile:
      value.proxyProfile === "environment" ||
      value.proxyProfile === "windows_user" ||
      value.proxyProfile === "winhttp" ||
      value.proxyProfile === "direct"
        ? value.proxyProfile
        : "auto",
    stepTimeoutMs: normalizeTimeout(value.stepTimeoutMs, 500, 60000, 5000),
    overallTimeoutMs: normalizeTimeout(value.overallTimeoutMs, 1000, 300000, 30000),
  };
}

function isProtocol(value: unknown): value is NetworkFavoriteProtocol {
  return value === "tcp" || value === "udp" || value === "ping";
}

function isOutcome(value: unknown): value is NonNullable<NetworkHistorySummary["outcome"]> {
  return value === "success" || value === "failed" || value === "unverified";
}

export function normalizeNetworkHistory(
  raw: unknown,
  limit = MAX_NETWORK_HISTORY,
): NetworkHistorySummary[] {
  if (!Array.isArray(raw)) return [];
  const rows: NetworkHistorySummary[] = [];
  for (const item of raw) {
    const value = item as Record<string, unknown> | null;
    if (!value || typeof value !== "object") continue;
    if (
      typeof value.id !== "string" ||
      typeof value.checkedAt !== "number" ||
      !isProtocol(value.protocol) ||
      typeof value.target !== "string" ||
      typeof value.timeoutMs !== "number" ||
      typeof value.reachable !== "boolean" ||
      typeof value.latencyMs !== "number"
    )
      continue;
    rows.push({
      id: value.id,
      checkedAt: value.checkedAt,
      protocol: value.protocol,
      target: value.target.trim(),
      timeoutMs: value.timeoutMs,
      reachable: value.reachable,
      latencyMs: value.latencyMs,
      statusCode: typeof value.statusCode === "number" ? value.statusCode : null,
      error: typeof value.error === "string" ? value.error : null,
      note: typeof value.note === "string" ? value.note : null,
      outcome: isOutcome(value.outcome) ? value.outcome : undefined,
    });
  }
  rows.sort((a, b) => b.checkedAt - a.checkedAt);
  return rows.slice(0, limit);
}

function isReport(value: unknown): value is AccessPathReport {
  if (!value || typeof value !== "object") return false;
  const report = value as Partial<AccessPathReport>;
  return (
    typeof report.schemaVersion === "number" &&
    typeof report.reportId === "string" &&
    Boolean(report.input && typeof report.input === "object") &&
    Array.isArray(report.steps) &&
    Array.isArray(report.evidence) &&
    Array.isArray(report.conclusions) &&
    Array.isArray(report.recommendations) &&
    typeof report.startedAt === "string"
  );
}

function normalizeReports(raw: unknown): AccessPathReport[] {
  if (!Array.isArray(raw)) return [];
  return raw.filter(isReport).slice(0, MAX_DIAGNOSTIC_REPORTS);
}

function isSettingsEnvelope(value: unknown): value is NetworkDiagnosticsSettings {
  return Boolean(
    value &&
    typeof value === "object" &&
    (value as NetworkDiagnosticsSettings).schemaVersion === NETWORK_DIAGNOSTICS_SCHEMA_VERSION,
  );
}

export function createNetworkDiagnosticsSettings(
  input: {
    favorites?: unknown;
    history?: unknown;
    reports?: unknown;
    diagnosisAdvancedParams?: unknown;
  } = {},
): NetworkDiagnosticsSettings {
  return {
    schemaVersion: NETWORK_DIAGNOSTICS_SCHEMA_VERSION,
    favorites: normalizeNetworkFavorites(input.favorites),
    history: normalizeNetworkHistory(input.history),
    reports: normalizeReports(input.reports),
    diagnosisAdvancedParams: normalizeNetworkDiagnosisAdvancedParams(input.diagnosisAdvancedParams),
  };
}

export function migrateNetworkDiagnosticsSettings(input: {
  current: unknown;
  legacyFavorites: unknown;
  legacyHistory: unknown;
}): { settings: NetworkDiagnosticsSettings; migrated: boolean } {
  if (isSettingsEnvelope(input.current)) {
    const settings = createNetworkDiagnosticsSettings(input.current);
    return { settings, migrated: false };
  }
  return {
    settings: createNetworkDiagnosticsSettings({
      favorites: input.legacyFavorites,
      history: input.legacyHistory,
    }),
    migrated: true,
  };
}

function cloneJson<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function truncateJsonStrings(value: unknown): unknown {
  if (typeof value === "string") {
    return value.length > 4096 ? value.slice(0, 4096) + "...[truncated]" : value;
  }
  if (Array.isArray(value)) return value.map(truncateJsonStrings);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, child]) => [key, truncateJsonStrings(child)]),
    );
  }
  return value;
}

function pruneEvidenceReferences(report: AccessPathReport): void {
  const ids = new Set(report.evidence.map((item) => item.id));
  for (const step of report.steps) {
    step.evidenceIds = step.evidenceIds.filter((id) => ids.has(id));
  }
  for (const conclusion of report.conclusions) {
    conclusion.evidenceIds = conclusion.evidenceIds.filter((id) => ids.has(id));
  }
  for (const recommendation of report.recommendations) {
    recommendation.evidenceIds = recommendation.evidenceIds.filter((id) => ids.has(id));
  }
}

function truncateReport(report: AccessPathReport): AccessPathReport {
  const copy = truncateJsonStrings(cloneJson(report)) as AccessPathReport;
  while (JSON.stringify(copy).length > MAX_DIAGNOSTIC_REPORT_BYTES && copy.evidence.length) {
    copy.evidence.pop();
  }
  pruneEvidenceReferences(copy);
  return copy;
}
export function appendDiagnosticReport(
  settings: NetworkDiagnosticsSettings,
  report: AccessPathReport,
): NetworkDiagnosticsSettings {
  const next = truncateReport(sanitizeAccessPathReport(report));
  const reports = [
    next,
    ...settings.reports.filter((item) => item.reportId !== next.reportId),
  ].slice(0, MAX_DIAGNOSTIC_REPORTS);
  while (reports.length > 1 && JSON.stringify(reports).length > MAX_DIAGNOSTIC_TOTAL_BYTES) {
    reports.pop();
  }
  return { ...settings, schemaVersion: NETWORK_DIAGNOSTICS_SCHEMA_VERSION, reports };
}

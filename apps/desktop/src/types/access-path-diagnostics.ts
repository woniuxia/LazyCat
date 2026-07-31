export type AccessPathProtocol = "http" | "https";

export type AccessPathTargetKind = "hostname" | "ipv4" | "ipv6";

export type AccessPathStepId = "proxy" | "hosts" | "dns" | "tcp" | "tls" | "http";

export type AccessPathStepLifecycle =
  | "pending"
  | "running"
  | "completed"
  | "blocked"
  | "skipped"
  | "cancelled";

export type AccessPathStepOutcome = "success" | "warning" | "failed" | "unverified";

export type AccessPathConclusionSeverity = "info" | "warning" | "error";

export type AccessPathRunStatus = "running" | "completed" | "cancelled" | "timed_out" | "failed";

export type AccessPathProxyProfile = "auto" | "environment" | "windows_user" | "winhttp" | "direct";

export type AccessPathJsonValue =
  | null
  | boolean
  | number
  | string
  | AccessPathJsonValue[]
  | { [key: string]: AccessPathJsonValue };

export interface NormalizedAccessPathTarget {
  rawInput: string;
  protocol: AccessPathProtocol;
  hostname: string;
  targetKind: AccessPathTargetKind;
  port: number;
  path: string;
  url: string;
  sni: string | null;
  verifyHostname: string;
  httpHost: string;
  connectionIp: string | null;
}

export interface AccessPathInputOptions {
  defaultProtocol?: AccessPathProtocol;
  sni?: string | null;
  verifyHostname?: string | null;
  httpHost?: string | null;
  connectionIp?: string | null;
}

export interface AccessPathError {
  code: string;
  message: string;
  details?: AccessPathJsonValue;
  retriable: boolean;
}

export interface AccessPathEvidence {
  id: string;
  stepId: AccessPathStepId;
  kind: string;
  value: AccessPathJsonValue;
  observedAt?: string;
}

export interface AccessPathConclusion {
  id: string;
  severity: AccessPathConclusionSeverity;
  message: string;
  evidenceIds: string[];
  recommendationIds: string[];
}

export interface AccessPathRecommendation {
  id: string;
  title: string;
  action: string;
  evidenceIds: string[];
}

export interface AccessPathStepSnapshot {
  id: AccessPathStepId;
  lifecycle: AccessPathStepLifecycle;
  outcome?: AccessPathStepOutcome;
  evidenceIds: string[];
  error?: AccessPathError;
  startedAt?: string;
  finishedAt?: string;
}

export interface AccessPathReport {
  schemaVersion: number;
  reportId: string;
  runId?: string;
  input: NormalizedAccessPathTarget;
  steps: AccessPathStepSnapshot[];
  evidence: AccessPathEvidence[];
  conclusions: AccessPathConclusion[];
  recommendations: AccessPathRecommendation[];
  startedAt: string;
  finishedAt?: string;
}

export interface AccessPathDiagnosisStartRequest {
  input: NormalizedAccessPathTarget;
  overallTimeoutMs?: number;
  stepTimeoutMs?: number;
  dnsServers?: string[];
  proxyProfile?: AccessPathProxyProfile;
}

export interface AccessPathDiagnosisStartResponse {
  runId: string;
}

export interface AccessPathDiagnosisRunSnapshot {
  runId: string;
  sequence: number;
  status: AccessPathRunStatus;
  report: AccessPathReport;
}

export interface AccessPathDiagnosisSnapshotEvent {
  runId: string;
  sequence: number;
  snapshot: AccessPathDiagnosisRunSnapshot;
}

export interface AccessPathDiagnosisCancelResponse {
  runId: string;
  cancelled: boolean;
  snapshot: AccessPathDiagnosisRunSnapshot;
}

export const ACCESS_PATH_REPORT_SCHEMA_VERSION = 1;

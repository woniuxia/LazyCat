export type ApiMockMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS";
export type ApiMockResponseKind = "static_body" | "file";
export type ApiMockRuntimeState = "stopped" | "running" | "restart-required" | "error";

export interface ApiMockHeaderRow {
  enabled: boolean;
  key: string;
  value: string;
}

export interface ApiMockCorsConfig {
  enabled: boolean;
  allowOrigin: string;
  allowMethods: ApiMockMethod[];
  allowHeaders: string;
  exposeHeaders: string;
  allowCredentials: boolean;
  maxAgeSeconds: number | null;
}

export interface ApiMockRuntimeSummary {
  running: boolean;
  restartRequired: boolean;
  lastError: string | null;
  startedAt: string | null;
  snapshot?: ApiMockRuntimeSnapshot | null;
}

export interface ApiMockRuntimeSnapshot {
  host: string;
  port: number;
  routeSignature: string;
}

export interface ApiMockProjectSummary {
  id: number;
  name: string;
  description: string;
  host: "127.0.0.1" | "0.0.0.0";
  port: number;
  enabledCorsDefault: boolean;
  sortOrder: number;
  routeCount: number;
  enabledRouteCount: number;
  runtime: ApiMockRuntimeSummary;
}

export interface ApiMockFileInfo {
  id: number;
  originalName: string;
  storedPath?: string;
  contentType: string;
  size: number;
  hash: string;
  createdAt: string;
}

export interface ApiMockRouteSummary {
  id: number;
  projectId: number;
  name: string;
  method: ApiMockMethod;
  pathPattern: string;
  statusCode: number;
  responseKind: ApiMockResponseKind;
  contentType: string;
  enabled: boolean;
  sortOrder: number;
  file: ApiMockFileInfo | null;
}

export interface ApiMockRouteDetail extends ApiMockRouteSummary {
  headers: ApiMockHeaderRow[];
  bodyText: string;
  cors: ApiMockCorsConfig;
}

export interface ApiMockRequestLog {
  id: number;
  time: string;
  method: string;
  path: string;
  status: number;
  routeId: number | null;
  routeName: string | null;
  durationMs: number;
  error: string | null;
}

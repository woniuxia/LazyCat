export type RequestForwardProtocol = "http" | "tcp" | "udp";

export type RequestForwardRuntimeState =
  | "stopped"
  | "starting"
  | "running"
  | "stopping"
  | "failed";

export interface RequestForwardRuleWriteInput {
  name: string;
  protocol: RequestForwardProtocol;
  bindHost: string;
  listenPort: number;
  targetUrl: string | null;
  targetHost: string | null;
  targetPort: number | null;
  captureHttpHeaders: boolean;
  captureHttpBody: boolean;
}

export interface RequestForwardRuleForm extends RequestForwardRuleWriteInput {}

export interface RequestForwardRule extends RequestForwardRuleWriteInput {
  id: number;
  autoStart: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface RequestForwardRuntimeStatus {
  ruleId: number;
  state: RequestForwardRuntimeState;
  lastError: string | null;
  lastObservabilityError: string | null;
}

export interface RequestForwardBatchOperationResult {
  ruleId: number;
  ok: boolean;
  error: string | null;
  state: RequestForwardRuntimeState;
}

export interface RequestForwardLogQuery {
  ruleId: number;
  keyword?: string | null;
  mode?: RequestForwardLogOutcome | null;
  offset?: number;
  limit?: number;
}

export type RequestForwardLogOutcome = "success" | "error";

export interface RequestForwardLogRow {
  id: number;
  ruleId: number;
  protocol: RequestForwardProtocol;
  clientAddr: string | null;
  targetAddr: string;
  method: string | null;
  path: string | null;
  statusCode: number | null;
  durationMs: number | null;
  uploadBytes: number;
  downloadBytes: number;
  requestHeaders: [string, string][] | null;
  responseHeaders: [string, string][] | null;
  requestBodyPreview: string | null;
  responseBodyPreview: string | null;
  requestBodyTruncated: boolean;
  responseBodyTruncated: boolean;
  error: string | null;
  createdAt: string;
}

export interface RequestForwardLogResult {
  items: RequestForwardLogRow[];
  total: number;
}

export interface RequestForwardStats {
  ruleId: number;
  eventCount: number;
  uploadBytes: number;
  downloadBytes: number;
  errorCount: number;
  updatedAt: string;
}

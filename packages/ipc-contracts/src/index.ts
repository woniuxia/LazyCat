// IPC contract types for the Tauri bridge.
// The actual IPC implementation is in apps/desktop/src/bridge/tauri.ts.

export interface ToolRequest {
  request_id: string;
  domain: string;
  action: string;
  payload: Record<string, unknown>;
}

export interface ToolError {
  code: string;
  message: string;
  details?: unknown;
}

export interface ToolMeta {
  duration_ms: number;
  warnings?: string[];
}

export interface ToolResponse {
  request_id: string;
  ok: boolean;
  data?: unknown;
  error?: ToolError;
  meta?: ToolMeta;
}

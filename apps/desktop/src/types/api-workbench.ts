export type ApiWorkbenchBodyType = "none" | "json" | "text" | "form-urlencoded";
export type ApiWorkbenchMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS";

export interface ApiWorkbenchKeyValueRow {
  enabled: boolean;
  key: string;
  value: string;
}

export interface ApiWorkbenchRequestDraft {
  method: ApiWorkbenchMethod;
  url: string;
  query: ApiWorkbenchKeyValueRow[];
  headers: ApiWorkbenchKeyValueRow[];
  bodyType: ApiWorkbenchBodyType;
  body: string;
  form: ApiWorkbenchKeyValueRow[];
  timeoutMs: number;
}

export interface ApiWorkbenchCollection {
  id: number;
  name: string;
  description: string;
  activeEnvironmentId: number | null;
  folders: ApiWorkbenchFolder[];
  requests: ApiWorkbenchRequestSummary[];
}

export interface ApiWorkbenchFolder {
  id: number;
  collectionId: number;
  parentId: number | null;
  name: string;
  sortOrder: number;
}

export interface ApiWorkbenchRequestSummary {
  id: number;
  collectionId: number;
  folderId: number | null;
  name: string;
  method: ApiWorkbenchMethod;
  url: string;
  sortOrder: number;
}

export interface ApiWorkbenchRequestDetail extends ApiWorkbenchRequestSummary {
  description: string;
  draft: ApiWorkbenchRequestDraft;
  exampleResponse?: string | null;
}

export interface ApiWorkbenchVariable {
  name: string;
  value: string;
  isSecret?: boolean;
}

export interface ApiWorkbenchEnvironment {
  id: number;
  collectionId: number;
  name: string;
  variables: ApiWorkbenchVariable[];
}

export interface ApiWorkbenchSendResult {
  finalUrl: string;
  status: number | null;
  statusText: string;
  ok: boolean;
  durationMs: number;
  requestHeaders: ApiWorkbenchKeyValueRow[];
  responseHeaders: ApiWorkbenchKeyValueRow[];
  bodyText: string;
  bodySize: number;
  bodyTruncated: boolean;
  contentType: string;
  error: string | null;
}

export interface ApiWorkbenchHistoryItem {
  id: number;
  collectionId: number | null;
  environmentId: number | null;
  requestId: number | null;
  name: string;
  method: ApiWorkbenchMethod;
  url: string;
  finalUrl: string;
  status: number | null;
  durationMs: number;
  ok: boolean;
  error: string | null;
  contentType: string;
  bodySize: number;
  bodyPreview: string;
  bodyTruncated: boolean;
  createdAt: string;
}

export interface ApiWorkbenchListResult {
  collections: ApiWorkbenchCollection[];
  history: ApiWorkbenchHistoryItem[];
}

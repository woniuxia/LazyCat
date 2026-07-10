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

export interface ApiWorkbenchTreeRequestNode extends ApiWorkbenchRequestSummary {}

export interface ApiWorkbenchTreeFolderNode extends ApiWorkbenchFolder {
  children: ApiWorkbenchTreeFolderNode[];
  requests: ApiWorkbenchTreeRequestNode[];
}

export interface ApiWorkbenchTree {
  collectionId: number;
  roots: ApiWorkbenchTreeFolderNode[];
  unassigned: {
    folderId: null;
    name: string;
    requests: ApiWorkbenchTreeRequestNode[];
  };
  foldersById: Map<number, ApiWorkbenchTreeFolderNode>;
  requestsById: Map<number, ApiWorkbenchTreeRequestNode>;
}

export interface ApiWorkbenchMoveTarget {
  folderId: number | null;
  label: string;
  depth: number;
}

export type ApiWorkbenchOrderDirection = "up" | "down";

export interface ApiWorkbenchMenuItem {
  key: string;
  label: string;
  danger?: boolean;
  disabled?: boolean;
  divider?: boolean;
}

export type ApiWorkbenchNavTarget =
  | { type: "blank" }
  | { type: "collection"; collectionId: number }
  | { type: "folder"; collectionId: number; folderId: number }
  | { type: "request"; collectionId: number; requestId: number; folderId: number | null };

export type ApiWorkbenchNavCommand =
  | "collection:create"
  | "collection:select"
  | "collection:rename"
  | "collection:delete"
  | "collection:export"
  | "folder:create-root"
  | "folder:create-child"
  | "folder:rename"
  | "folder:delete"
  | "folder:move"
  | "folder:up"
  | "folder:down"
  | "request:import-curl"
  | "request:create"
  | "request:open"
  | "request:duplicate"
  | "request:rename"
  | "request:delete"
  | "request:move"
  | "request:up"
  | "request:down";

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
  bodyStorage: "text" | "file" | "empty" | "truncated-binary";
  bodyFilePath: string;
  bodyFileName: string;
  bodyExtension: string;
  bodyHash: string;
  bodyPreviewError: string | null;
  error: string | null;
}

export interface ApiWorkbenchHistoryRequestSnapshot extends ApiWorkbenchRequestDraft {}

export interface ApiWorkbenchExecutedRequestSnapshot {
  method: ApiWorkbenchMethod;
  finalUrl: string;
  headers: ApiWorkbenchKeyValueRow[];
  bodyType: ApiWorkbenchBodyType;
  body: string;
  form: ApiWorkbenchKeyValueRow[];
  timeoutMs: number;
}

export interface ApiWorkbenchHistoryItem {
  id: number;
  collectionId: number | null;
  environmentId: number | null;
  requestId: number | null;
  replayedFromHistoryId: number | null;
  name: string;
  note: string;
  pinned: boolean;
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
  bodyStorage: "text" | "file" | "empty" | "truncated-binary";
  bodyFilePath: string;
  bodyFileName: string;
  bodyExtension: string;
  bodyHash: string;
  bodyPreviewError: string | null;
  hasRequestSnapshot: boolean;
  hasExecutedRequestSnapshot: boolean;
  createdAt: string;
}

export interface ApiWorkbenchHistoryDetail extends ApiWorkbenchHistoryItem {
  requestSnapshot: ApiWorkbenchHistoryRequestSnapshot | null;
}

export interface ApiWorkbenchListResult {
  collections: ApiWorkbenchCollection[];
  history: ApiWorkbenchHistoryItem[];
}

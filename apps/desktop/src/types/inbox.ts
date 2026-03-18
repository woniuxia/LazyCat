export type InboxItemType = "text" | "html" | "rtf" | "image" | "file" | "unknown";

export type InboxStorageKind = "inline" | "external" | "metadata_only";

export type InboxBucket = "history" | "inbox" | "archived";

export interface InboxFileRef {
  id?: number;
  filePath: string;
  fileName: string;
  fileSize: number | null;
  modifiedAt: string | null;
  exists?: boolean;
  isDirectory?: boolean;
}

export interface InboxItemSummary {
  id: number;
  bucket: InboxBucket;
  itemType: InboxItemType;
  storageKind: InboxStorageKind;
  title: string;
  preview: string;
  byteSize: number;
  capturedAt: string;
  lastSeenAt: string;
  seenCount: number;
  starred: boolean;
  hasNote: boolean;
  metaJson: Record<string, unknown> | null;
}

export interface InboxItemDetail extends InboxItemSummary {
  note: string;
  searchText: string;
  payloadText: string | null;
  payloadDataUrl: string | null;
  fileRefs: InboxFileRef[];
  openPath: string | null;
  canOpenPath: boolean;
}

export interface InboxListQuery {
  bucket?: InboxBucket | "all";
  itemType?: InboxItemType | "";
  starredOnly?: boolean;
  externalOnly?: boolean;
  summaryOnly?: boolean;
  keyword?: string;
  limit?: number;
  offset?: number;
}

export interface InboxFacetCounts {
  buckets: Record<InboxBucket, number>;
  types: Partial<Record<InboxItemType, number>>;
  starred: number;
  external: number;
  summaryOnly: number;
}

export interface InboxListResult {
  items: InboxItemSummary[];
  hasMore: boolean;
  nextOffset: number;
  total: number;
  facets: InboxFacetCounts;
}

export interface InboxCaptureStatus {
  monitorRunning: boolean;
  consentAck: boolean;
  captureEnabled: boolean;
  captureWhenHidden: boolean;
  historyRetentionDays: number;
  paused: boolean;
  pausedUntil: string | null;
}

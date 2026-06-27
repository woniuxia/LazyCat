export interface DataDictionarySummary {
  id: number;
  name: string;
  description: string;
  recordCount: number;
  primaryFieldPath: string | null;
  titleFieldPath: string | null;
  sortFieldPath: string | null;
  sortDirection: DataDictionarySortDirection;
  navOrder: number;
  createdAt: string;
  updatedAt: string;
}

export type DataDictionarySortDirection = "asc" | "desc";

export interface DataDictionaryField {
  fieldPath: string;
  displayName: string;
  meaning: string;
  searchable: boolean;
  visible: boolean;
  sortOrder: number;
  typeHint: string;
  sampleValue: string;
  presentCount: number;
}

export interface DataDictionaryImportPreview {
  recordCount: number;
  fields: Array<{
    fieldPath: string;
    displayName: string;
    typeHint: string;
    sampleValue: string;
    presentCount: number;
    sortOrder: number;
  }>;
}

export interface DataDictionaryMatch {
  fieldPath: string;
  value: string;
}

export interface DataDictionaryRecordSummaryPart {
  fieldPath: string;
  label: string;
  value: string;
}

export interface DataDictionarySearchItem {
  id: number;
  dictionaryId: number;
  dictionaryName: string;
  titleFieldPath: string | null;
  rowIndex: number;
  rawJson?: unknown;
  matches: DataDictionaryMatch[];
  title: string;
  summary: DataDictionaryRecordSummaryPart[];
}

export interface DataDictionarySearchResult {
  items: DataDictionarySearchItem[];
  hasMore: boolean;
}

export type DataDictionarySearchScope = "current" | "all";

export interface DataDictionarySearchRequest {
  scope: DataDictionarySearchScope;
  dictionaryId?: number;
  keyword?: string;
  limit?: number;
  includeRawJson?: boolean;
}

export interface DataDictionaryRelation {
  id: number;
  sourceDictionaryId: number;
  sourceFieldPath: string;
  targetDictionaryId: number;
  targetDictionaryName: string;
  targetPrimaryFieldPath: string | null;
  relationName: string;
  reverseName: string;
}

export interface DataDictionaryRelationDraft {
  sourceFieldPath: string;
  targetDictionaryId: number | null;
  relationName: string;
  reverseName: string;
}

export interface DataDictionaryRecordBrief {
  id: number;
  dictionaryId: number;
  dictionaryName: string;
  title: string;
  rowIndex: number;
  summary: DataDictionaryRecordSummaryPart[];
}

export interface DataDictionaryRecordFull extends DataDictionaryRecordBrief {
  rawJson: unknown;
}

export interface DataDictionaryRelationGroup {
  relationId: number;
  name: string;
  direction: "forward" | "reverse";
  sourceDictionaryId: number;
  targetDictionaryId: number;
  itemCount: number;
  items: DataDictionaryRecordBrief[];
}

export interface DataDictionaryRecordDetail {
  record: DataDictionaryRecordFull;
  fields: DataDictionaryField[];
  forwardRelations: DataDictionaryRelationGroup[];
  reverseRelations: DataDictionaryRelationGroup[];
}

export interface DataDictionaryImportWriteResult {
  ok: true;
  id?: number;
  recordCount: number;
  skippedPrimaryRecordCount: number;
  skippedPrimaryInvalidCount: number;
  skippedPrimaryDuplicateCount: number;
}

export interface RebuildDataDictionaryIndexesResult {
  recordCount: number;
  valueCount: number;
  skippedPrimaryRecordCount: number;
  skippedPrimaryInvalidCount: number;
  skippedPrimaryDuplicateCount: number;
}

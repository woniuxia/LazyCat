export interface DataDictionarySummary {
  id: number;
  name: string;
  description: string;
  recordCount: number;
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

export interface DataDictionarySearchItem {
  id: number;
  dictionaryId: number;
  dictionaryName: string;
  titleFieldPath: string | null;
  rowIndex: number;
  rawJson: unknown;
  matches: DataDictionaryMatch[];
}

export interface DataDictionarySearchResult {
  items: DataDictionarySearchItem[];
  hasMore: boolean;
}

export type DataDictionarySearchScope = "current" | "all";

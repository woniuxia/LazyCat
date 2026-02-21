export type TextLineEnding = "keep" | "lf" | "crlf";
export type TextMatchMode = "contains" | "equals" | "regex";
export type TextSortOrder = "asc" | "desc";

export type TextOperationType =
  | "trim"
  | "remove_empty"
  | "dedupe"
  | "sort"
  | "include_filter"
  | "exclude_filter"
  | "replace"
  | "add_prefix"
  | "add_suffix"
  | "extract_column";

export interface TextOperation {
  type: TextOperationType;
  enabled: boolean;
  caseSensitive?: boolean;
  pattern?: string;
  replacement?: string;
  matchMode?: TextMatchMode;
  sortOrder?: TextSortOrder;
  delimiter?: string;
  columnIndex?: number;
  keepUnmatched?: boolean;
}

export interface TextProcessRequest {
  input: string;
  lineEnding: TextLineEnding;
  operations: TextOperation[];
  previewLimit?: number;
}

export interface TextPreviewSample {
  before: string;
  after: string;
  line: number;
}

export interface TextProcessStats {
  inputLines: number;
  outputLines: number;
  changedLines: number;
  inputChars: number;
  outputChars: number;
  durationMs: number;
}

export interface TextProcessResponse {
  output: string;
  stats: TextProcessStats;
  preview: {
    changed: number;
    samples: TextPreviewSample[];
  };
  warnings: string[];
}

export interface TextPreset {
  id: string;
  name: string;
  description: string;
  operations: TextOperation[];
}

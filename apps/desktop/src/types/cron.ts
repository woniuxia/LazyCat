export type CronStandard = "linux5" | "spring6" | "quartz";

export interface CronFieldParts {
  second: string;
  minute: string;
  hour: string;
  dayOfMonth: string;
  month: string;
  dayOfWeek: string;
}

export interface CronNormalizeResponse {
  ok: boolean;
  normalizedExpression: string;
  fieldCount: number;
  canonicalFieldCount: number;
  standard: CronStandard;
  parts: CronFieldParts;
  warnings: string[];
}

export interface CronPreviewItem {
  iso: string;
  display: string;
  epochMs: number;
}

export interface CronPreviewV2Response {
  normalizedExpression: string;
  standard: CronStandard;
  timezone: string;
  items: CronPreviewItem[];
  warnings: string[];
}

export interface CronDescribeResponse {
  normalizedExpression: string;
  standard: CronStandard;
  summary: string;
  details: string[];
  warnings: string[];
}

export interface PmNormalizedDateRange {
  startAt: string | null;
  endAt: string | null;
}

export interface PmDateRangeFormatOptions {
  mode?: "full" | "short";
  emptyText?: string;
  separator?: string;
}

interface PmDateParts {
  year: number;
  monthIndex: number;
  day: number;
}

const PM_DATE_PREFIX_PATTERN = /^(\d{4})-(\d{2})-(\d{2})(?:$|T|\s)/;
const PM_DATE_EXACT_PATTERN = /^(\d{4})-(\d{2})-(\d{2})$/;

function extractPmDateParts(value: string | null | undefined): PmDateParts | null {
  if (typeof value !== "string") {
    return null;
  }

  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }

  const match = PM_DATE_EXACT_PATTERN.exec(trimmed) ?? PM_DATE_PREFIX_PATTERN.exec(trimmed);
  if (!match) {
    return null;
  }

  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (!Number.isInteger(year) || !Number.isInteger(month) || !Number.isInteger(day)) {
    return null;
  }
  if (year <= 0 || month < 1 || month > 12 || day < 1 || day > 31) {
    return null;
  }

  const localDate = new Date(year, month - 1, day);
  if (
    localDate.getFullYear() !== year ||
    localDate.getMonth() !== month - 1 ||
    localDate.getDate() !== day
  ) {
    return null;
  }

  return {
    year,
    monthIndex: month - 1,
    day,
  };
}

function formatPmDateParts(parts: PmDateParts): string {
  return `${String(parts.year).padStart(4, "0")}-${String(parts.monthIndex + 1).padStart(2, "0")}-${String(parts.day).padStart(2, "0")}`;
}

function formatPmNormalizedDate(date: string, mode: "full" | "short"): string {
  return mode === "short" ? date.slice(5) : date;
}

export function normalizePmDateString(value: string | null | undefined): string | null {
  const parts = extractPmDateParts(value);
  return parts ? formatPmDateParts(parts) : null;
}

export function parsePmDateAtLocalStart(date: string | null | undefined): Date | null {
  const parts = extractPmDateParts(date);
  if (!parts) {
    return null;
  }

  return new Date(parts.year, parts.monthIndex, parts.day, 0, 0, 0, 0);
}

export function parsePmDateAtLocalEnd(date: string | null | undefined): Date | null {
  const parts = extractPmDateParts(date);
  if (!parts) {
    return null;
  }

  return new Date(parts.year, parts.monthIndex, parts.day, 23, 59, 59, 999);
}

export function hasPmDateSchedule(
  startAt: string | null | undefined,
  endAt: string | null | undefined,
): boolean {
  return Boolean(normalizePmDateString(startAt) || normalizePmDateString(endAt));
}

export function normalizePmDateRangeForDraft(
  startAt: string | null | undefined,
  endAt: string | null | undefined,
): PmNormalizedDateRange {
  const normalizedStart = normalizePmDateString(startAt);
  const normalizedEnd = normalizePmDateString(endAt);

  if (normalizedStart && normalizedEnd) {
    if (normalizedStart <= normalizedEnd) {
      return {
        startAt: normalizedStart,
        endAt: normalizedEnd,
      };
    }
    return {
      startAt: normalizedEnd,
      endAt: normalizedStart,
    };
  }

  const singleDate = normalizedStart ?? normalizedEnd;
  if (!singleDate) {
    return {
      startAt: null,
      endAt: null,
    };
  }

  return {
    startAt: singleDate,
    endAt: singleDate,
  };
}

export function getPmDateRangeValue(
  startAt: string | null | undefined,
  endAt: string | null | undefined,
): [string, string] | null {
  const normalizedRange = normalizePmDateRangeForDraft(startAt, endAt);
  if (!normalizedRange.startAt || !normalizedRange.endAt) {
    return null;
  }
  return [normalizedRange.startAt, normalizedRange.endAt];
}

export function formatPmDateForDisplay(
  value: string | null | undefined,
  mode: "full" | "short" = "full",
): string {
  const normalized = normalizePmDateString(value);
  return normalized ? formatPmNormalizedDate(normalized, mode) : "";
}

export function formatPmDateRangeForDisplay(
  startAt: string | null | undefined,
  endAt: string | null | undefined,
  options: PmDateRangeFormatOptions = {},
): string {
  const mode = options.mode ?? "full";
  const emptyText = options.emptyText ?? "-";
  const separator = options.separator ?? " ~ ";
  const start = normalizePmDateString(startAt);
  const end = normalizePmDateString(endAt);

  if (start && end) {
    return `${formatPmNormalizedDate(start, mode)}${separator}${formatPmNormalizedDate(end, mode)}`;
  }
  if (start) {
    return formatPmNormalizedDate(start, mode);
  }
  if (end) {
    return formatPmNormalizedDate(end, mode);
  }
  return emptyText;
}

export function isPmItemOverdue(item: { endAt: string | null; status: string }): boolean {
  if (!item.endAt || item.status === "done") {
    return false;
  }
  const end = parsePmDateAtLocalEnd(item.endAt);
  if (!end) {
    return false;
  }
  return end.getTime() < Date.now();
}

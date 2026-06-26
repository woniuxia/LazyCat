import type {
  DataDictionaryField,
  DataDictionaryMatch,
  DataDictionarySearchItem,
} from "../types/data-dictionary";

export interface DataDictionarySummaryPart {
  fieldPath: string;
  label: string;
  value: string;
}

export function formatJsonValue(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean" || value === null) {
    return String(value);
  }
  try {
    return JSON.stringify(value);
  } catch {
    return "";
  }
}

export function buildResultSummary(
  item: DataDictionarySearchItem,
  fields: DataDictionaryField[],
  maxFields = 4,
): DataDictionarySummaryPart[] {
  return fields
    .filter((field) => field.visible)
    .sort((a, b) => a.sortOrder - b.sortOrder || a.fieldPath.localeCompare(b.fieldPath))
    .flatMap((field) => {
      const value = getValueByFieldPath(item.rawJson, field.fieldPath);
      if (value === undefined) return [];
      return {
        fieldPath: field.fieldPath,
        label: summaryFieldLabel(field),
        value: formatJsonValue(value),
      };
    })
    .filter((part) => part.value !== "undefined" && part.value !== "")
    .slice(0, maxFields);
}

export function buildMatchLabels(
  matches: DataDictionaryMatch[],
  fields: DataDictionaryField[],
): DataDictionarySummaryPart[] {
  const fieldMap = new Map(fields.map((field) => [field.fieldPath, field]));
  return matches.map((match) => {
    const field = fieldMap.get(match.fieldPath);
    return {
      fieldPath: match.fieldPath,
      label: field ? fieldLabel(field) : match.fieldPath,
      value: match.value,
    };
  });
}

export function dictionarySourceLabel(item: DataDictionarySearchItem): string {
  return `${item.dictionaryName} #${item.rowIndex + 1}`;
}

export function formatJsonDocument(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return "";
  }
}

function fieldLabel(field: DataDictionaryField): string {
  return field.displayName.trim() || field.fieldPath;
}

function summaryFieldLabel(field: DataDictionaryField): string {
  return field.displayName.trim() || field.meaning.trim() || field.fieldPath;
}

function getValueByFieldPath(source: unknown, fieldPath: string): unknown {
  const parts = splitEscapedPath(fieldPath);
  let current: unknown = source;
  for (const part of parts) {
    if (!current || typeof current !== "object" || Array.isArray(current)) return undefined;
    current = (current as Record<string, unknown>)[part];
  }
  return current;
}

function splitEscapedPath(fieldPath: string): string[] {
  const parts: string[] = [];
  let current = "";
  let escaped = false;
  for (const char of fieldPath) {
    if (escaped) {
      current += char;
      escaped = false;
      continue;
    }
    if (char === "\\") {
      escaped = true;
      continue;
    }
    if (char === ".") {
      parts.push(current);
      current = "";
      continue;
    }
    current += char;
  }
  if (escaped) current += "\\";
  parts.push(current);
  return parts;
}

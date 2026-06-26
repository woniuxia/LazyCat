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

export interface DataDictionaryFieldDraftGroups {
  visibleFields: DataDictionaryField[];
  hiddenFields: DataDictionaryField[];
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
  excludedFieldPath?: string | null,
): DataDictionarySummaryPart[] {
  return fields
    .filter((field) => field.visible && field.fieldPath !== excludedFieldPath)
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
}

export function buildResultTitle(item: DataDictionarySearchItem): string {
  const titlePath = item.titleFieldPath?.trim();
  if (titlePath) {
    const value = getValueByFieldPath(item.rawJson, titlePath);
    const title = value === undefined ? "" : formatJsonValue(value).trim();
    if (title) return title;
  }
  return dictionarySourceLabel(item);
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

export function orderDataDictionaryFieldDrafts(
  fields: DataDictionaryField[],
): DataDictionaryField[] {
  return reindexDataDictionaryFieldDrafts(
    fields
      .slice()
      .sort(
        (a, b) =>
          Number(b.visible) - Number(a.visible) ||
          a.sortOrder - b.sortOrder ||
          a.fieldPath.localeCompare(b.fieldPath),
      ),
  );
}

export function splitDataDictionaryFieldDrafts(
  fields: DataDictionaryField[],
): DataDictionaryFieldDraftGroups {
  const ordered = orderDataDictionaryFieldDrafts(fields);
  return {
    visibleFields: ordered.filter((field) => field.visible),
    hiddenFields: ordered.filter((field) => !field.visible),
  };
}

export function moveDataDictionaryFieldDraft(
  fields: DataDictionaryField[],
  oldIndex: number,
  newIndex: number,
): DataDictionaryField[] {
  const grouped = splitDataDictionaryFieldDrafts(fields);
  if (
    oldIndex < 0 ||
    newIndex < 0 ||
    oldIndex >= grouped.visibleFields.length ||
    newIndex >= grouped.visibleFields.length ||
    oldIndex === newIndex
  ) {
    return reindexDataDictionaryFieldDrafts([
      ...grouped.visibleFields,
      ...grouped.hiddenFields,
    ]);
  }
  const nextVisible = grouped.visibleFields.slice();
  const [moved] = nextVisible.splice(oldIndex, 1);
  if (!moved) {
    return reindexDataDictionaryFieldDrafts([
      ...grouped.visibleFields,
      ...grouped.hiddenFields,
    ]);
  }
  nextVisible.splice(newIndex, 0, moved);
  return reindexDataDictionaryFieldDrafts([...nextVisible, ...grouped.hiddenFields]);
}

export function setDataDictionaryFieldVisibility(
  fields: DataDictionaryField[],
  fieldPath: string,
  visible: boolean,
): DataDictionaryField[] {
  const ordered = orderDataDictionaryFieldDrafts(fields);
  const target = ordered.find((field) => field.fieldPath === fieldPath);
  if (!target || target.visible === visible) return ordered;

  const withoutTarget = ordered.filter((field) => field.fieldPath !== fieldPath);
  const visibleFields = withoutTarget.filter((field) => field.visible);
  const hiddenFields = withoutTarget.filter((field) => !field.visible);
  const updatedTarget = { ...target, visible };
  return reindexDataDictionaryFieldDrafts(
    visible
      ? [...visibleFields, updatedTarget, ...hiddenFields]
      : [...visibleFields, ...hiddenFields, updatedTarget],
  );
}

export function reindexDataDictionaryFieldDrafts(
  fields: DataDictionaryField[],
): DataDictionaryField[] {
  return fields.map((field, index) => ({ ...field, sortOrder: index }));
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

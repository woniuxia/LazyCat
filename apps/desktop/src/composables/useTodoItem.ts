/**
 * Todo item data normalization and parsing utilities.
 * Pure functions with no reactive state dependencies.
 */
import type {
  TodoAssignee,
  TodoEndMode,
  TodoItem,
  TodoKind,
  TodoLink,
  TodoPriority,
  TodoRecurrence,
  TodoReminderPreset,
  TodoRule,
  TodoRuleMode,
  TodoSimpleRule,
  TodoStatus,
} from "../types";
import { normalizeEndMode } from "../utils/todoSchedule";

// ---------------------------------------------------------------------------
// Reminder preset mapping
// ---------------------------------------------------------------------------

const reminderPresetToMinutesMap: Record<TodoReminderPreset, number | null> = {
  "0m": 0,
  none: null,
  "5m": 5,
  "10m": 10,
  "30m": 30,
  "1h": 60,
  "1d": 24 * 60,
  "2d": 2 * 24 * 60,
};

export function reminderPresetFromMinutes(minutes: number | null): TodoReminderPreset {
  if (minutes == null) return "none";
  const matched = Object.entries(reminderPresetToMinutesMap).find(
    ([, value]) => value === minutes,
  )?.[0];
  return (matched as TodoReminderPreset | undefined) || "none";
}

export function reminderPresetToMinutes(preset: TodoReminderPreset) {
  return reminderPresetToMinutesMap[preset] ?? null;
}

// ---------------------------------------------------------------------------
// Reminder preset normalization
// ---------------------------------------------------------------------------

function normalizeReminderPreset(value: unknown): TodoReminderPreset | null {
  if (typeof value !== "string") return null;
  const normalized = value.trim().toLowerCase();
  if (["0m", "none", "5m", "10m", "30m", "1h", "1d", "2d"].includes(normalized)) {
    return normalized as TodoReminderPreset;
  }
  return null;
}

function sortReminderPresets(presets: TodoReminderPreset[]) {
  const order: TodoReminderPreset[] = ["none", "0m", "5m", "10m", "30m", "1h", "1d", "2d"];
  presets.sort((left, right) => order.indexOf(left) - order.indexOf(right));
}

export function normalizeReminderPresets(values: unknown[]) {
  const presets: TodoReminderPreset[] = [];
  let hasNone = false;
  for (const value of values) {
    const normalized = normalizeReminderPreset(value);
    if (!normalized) continue;
    if (normalized === "none") {
      hasNone = true;
      continue;
    }
    if (!presets.includes(normalized)) presets.push(normalized);
  }
  sortReminderPresets(presets);
  if (hasNone && presets.length === 0) return ["none"] as TodoReminderPreset[];
  return presets;
}

export function effectiveReminderPresets(values: TodoReminderPreset[]) {
  return normalizeReminderPresets(values).filter((preset) => preset !== "none");
}

export function toDraftReminderPresets(values?: TodoReminderPreset[] | null) {
  const normalized = normalizeReminderPresets(values || []);
  return normalized.length > 0 ? normalized : (["none"] as TodoReminderPreset[]);
}

// ---------------------------------------------------------------------------
// Generic record readers
// ---------------------------------------------------------------------------

export function asRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" ? (value as Record<string, unknown>) : {};
}

function readUnknown(record: Record<string, unknown>, keys: string[]) {
  for (const key of keys) {
    if (key in record) return record[key];
  }
  return undefined;
}

function readString(record: Record<string, unknown>, keys: string[], fallback = "") {
  const value = readUnknown(record, keys);
  return typeof value === "string" ? value : fallback;
}

function readNullableString(record: Record<string, unknown>, keys: string[]) {
  const value = readUnknown(record, keys);
  if (typeof value === "string") return value;
  if (typeof value === "number" && Number.isFinite(value)) return String(value);
  return null;
}

function readNumber(record: Record<string, unknown>, keys: string[], fallback = 0) {
  const value = readUnknown(record, keys);
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string") {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return fallback;
}

export function readNullableNumber(record: Record<string, unknown>, keys: string[]) {
  const value = readUnknown(record, keys);
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return null;
}

function readBoolean(record: Record<string, unknown>, keys: string[], fallback = false) {
  const value = readUnknown(record, keys);
  if (typeof value === "boolean") return value;
  if (typeof value === "number") return value !== 0;
  if (typeof value === "string") {
    const normalized = value.trim().toLowerCase();
    if (["true", "1", "yes", "enabled", "active"].includes(normalized)) return true;
    if (["false", "0", "no", "disabled", "inactive"].includes(normalized)) return false;
  }
  return fallback;
}

function readArray(record: Record<string, unknown>, keys: string[]) {
  const value = readUnknown(record, keys);
  return Array.isArray(value) ? value : [];
}

// ---------------------------------------------------------------------------
// Enum normalizers
// ---------------------------------------------------------------------------

function normalizePriority(value: string): TodoPriority {
  return ["P0", "P1", "P2", "P3"].includes(value) ? (value as TodoPriority) : "P2";
}

function normalizeStatus(value: string): TodoStatus {
  return ["pending", "in_progress", "completed"].includes(value)
    ? (value as TodoStatus)
    : "pending";
}

function normalizeKind(value: unknown): TodoKind {
  if (value === "recurring") return "recurring";
  return "one_off";
}

function normalizeRuleMode(value: string): TodoRuleMode {
  return value === "cron" ? "cron" : "simple";
}

// ---------------------------------------------------------------------------
// Sub-structure normalizers
// ---------------------------------------------------------------------------

function normalizeAssignees(value: unknown): TodoAssignee[] {
  if (!Array.isArray(value)) return [];
  return value
    .map((item) => {
      if (typeof item === "string") {
        return { id: 0, name: item, createdAt: "", updatedAt: "" } satisfies TodoAssignee;
      }
      const record = asRecord(item);
      const name = readString(record, ["name", "label", "assigneeName"]);
      if (!name) return null;
      return {
        id: readNumber(record, ["id", "assigneeId", "userId"], 0),
        name,
        createdAt: readString(record, ["createdAt"], ""),
        updatedAt: readString(record, ["updatedAt"], ""),
      } satisfies TodoAssignee;
    })
    .filter((item): item is TodoAssignee => Boolean(item));
}

function normalizeLinks(value: unknown): TodoLink[] {
  if (!Array.isArray(value)) return [];
  return value
    .map((item) => {
      const record = asRecord(item);
      const url = readString(record, ["url"]);
      if (!url) return null;
      return {
        id: readNumber(record, ["id"], 0),
        url,
        title: readString(record, ["title"], ""),
      } satisfies TodoLink;
    })
    .filter((item): item is TodoLink => Boolean(item));
}

function normalizeRule(
  rawRule: unknown,
  ruleMode: TodoRuleMode,
  fallbackCronExpression = "",
): TodoRule {
  if (ruleMode === "cron") {
    const expressionRecord = asRecord(rawRule);
    const expression =
      typeof rawRule === "string"
        ? rawRule
        : readString(expressionRecord, ["expression", "cronExpression"], fallbackCronExpression);
    return { expression };
  }

  const source =
    typeof rawRule === "string"
      ? (() => {
          try {
            return asRecord(JSON.parse(rawRule));
          } catch {
            return {};
          }
        })()
      : asRecord(rawRule);

  const frequency = (["daily", "weekly", "monthly"] as const).includes(
    readString(source, ["frequency"], "daily") as "daily" | "weekly" | "monthly",
  )
    ? (readString(source, ["frequency"], "daily") as "daily" | "weekly" | "monthly")
    : "daily";
  const interval = Math.max(1, readNumber(source, ["interval"], 1));
  const time = readString(source, ["time"], "09:00");
  const weekdays = readArray(source, ["weekdays"])
    .map((item) => Number(item))
    .filter((day) => Number.isInteger(day) && day >= 1 && day <= 7);
  const dayOfMonth = Math.min(31, Math.max(1, readNumber(source, ["dayOfMonth"], 1)));
  if (frequency === "weekly")
    return {
      frequency,
      interval,
      time,
      weekdays: weekdays.length > 0 ? weekdays : [1, 2, 3, 4, 5],
    };
  if (frequency === "monthly") return { frequency, interval, time, dayOfMonth };
  return { frequency, interval, time };
}

// ---------------------------------------------------------------------------
// Main normalizer
// ---------------------------------------------------------------------------

export function getResponseItems(payload: unknown) {
  const record = asRecord(payload);
  const items = record.items;
  return Array.isArray(items) ? items : [];
}

export function getRootItemId(item: TodoItem) {
  return item.rootId || item.id;
}

export function normalizeTodoItem(raw: unknown): TodoItem {
  const record = asRecord(raw);
  const eventAt = typeof record.eventAt === "string" ? record.eventAt : null;
  const kind = normalizeKind(record.kind);
  const recurrenceRecord = asRecord(record.recurrence);
  const hasRecurrence =
    kind === "recurring" &&
    ("ruleMode" in recurrenceRecord || "rule" in recurrenceRecord || "cronExpression" in recurrenceRecord);
  const recurrenceRuleMode = normalizeRuleMode(
    typeof recurrenceRecord.ruleMode === "string" ? recurrenceRecord.ruleMode : "simple",
  );
  const recurrenceCronExpression =
    typeof recurrenceRecord.cronExpression === "string" ? recurrenceRecord.cronExpression : "";
  const recurrence = hasRecurrence
    ? ({
        startAt: typeof recurrenceRecord.startAt === "string" ? recurrenceRecord.startAt : null,
        ruleMode: recurrenceRuleMode,
        rule: normalizeRule(recurrenceRecord.rule, recurrenceRuleMode, recurrenceCronExpression),
        cronExpression: recurrenceCronExpression,
        timezone: typeof recurrenceRecord.timezone === "string" ? recurrenceRecord.timezone : "local",
        endMode: normalizeEndMode(
          typeof recurrenceRecord.endMode === "string" ? recurrenceRecord.endMode : "never",
        ),
        endValue:
          recurrenceRecord.endValue == null ||
          typeof recurrenceRecord.endValue === "string" ||
          typeof recurrenceRecord.endValue === "number"
            ? (recurrenceRecord.endValue as string | number | null)
            : null,
        occurrenceIndex:
          typeof recurrenceRecord.occurrenceIndex === "number" ? recurrenceRecord.occurrenceIndex : 0,
        active: typeof recurrenceRecord.active === "boolean" ? recurrenceRecord.active : true,
      } satisfies TodoRecurrence)
    : null;
  const id = typeof record.id === "number" ? record.id : 0;
  const rootId = typeof record.rootId === "number" ? record.rootId : id;
  const normalizedStatus =
    typeof record.status === "string" ? normalizeStatus(record.status) : ("pending" satisfies TodoStatus);
  return {
    id,
    rootId,
    kind,
    pinned: record.pinned === true,
    title: typeof record.title === "string" ? record.title : "",
    typeId: typeof record.typeId === "number" ? record.typeId : null,
    typeName: typeof record.typeName === "string" ? record.typeName : null,
    typeColor: typeof record.typeColor === "string" ? record.typeColor : null,
    priority: normalizePriority(typeof record.priority === "string" ? record.priority : "P2"),
    description: typeof record.description === "string" ? record.description : "",
    status: normalizedStatus,
    eventAt,
    reminderPresets: deriveReminderPresets(record),
    snoozeUntil: typeof record.snoozeUntil === "string" ? record.snoozeUntil : null,
    lastNotifiedAt: typeof record.lastNotifiedAt === "string" ? record.lastNotifiedAt : null,
    displayAt: typeof record.displayAt === "string" ? record.displayAt : eventAt,
    assignees: normalizeAssignees(record.assignees),
    links: normalizeLinks(record.links),
    isOverdue: record.isOverdue === true,
    recurrence,
    nextTaskReminderId: typeof record.nextTaskReminderId === "number" ? record.nextTaskReminderId : null,
    nextReminderPreset: normalizeReminderPreset(record.nextReminderPreset),
    completedAt: typeof record.completedAt === "string" ? record.completedAt : null,
    createdAt: typeof record.createdAt === "string" ? record.createdAt : "",
    updatedAt: typeof record.updatedAt === "string" ? record.updatedAt : "",
    projectId: typeof record.projectId === "number" ? record.projectId : null,
    projectName: typeof record.projectName === "string" ? record.projectName : null,
    projectColor: typeof record.projectColor === "string" ? record.projectColor : null,
    pmItemId: typeof record.pmItemId === "number" ? record.pmItemId : null,
    pmItemTitle: typeof record.pmItemTitle === "string" ? record.pmItemTitle : null,
    pmItemProjectId: typeof record.pmItemProjectId === "number" ? record.pmItemProjectId : null,
    pmItemStatus: typeof record.pmItemStatus === "string" ? record.pmItemStatus : null,
  };
}

function deriveReminderPresets(record: Record<string, unknown>): TodoReminderPreset[] {
  const presetValues = record.reminderPresets;
  return Array.isArray(presetValues) ? effectiveReminderPresets(presetValues as TodoReminderPreset[]) : [];
}

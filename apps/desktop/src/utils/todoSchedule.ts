import type {
  TodoEndMode,
  TodoRecurrence,
  TodoRepeatPreset,
  TodoRule,
  TodoRuleMode,
  TodoSimpleFrequency,
  TodoSimpleRule,
} from "../types";

export interface TodoRepeatDraft {
  preset: TodoRepeatPreset;
  ruleMode: TodoRuleMode;
  cronExpression: string;
  timezone: string;
  frequency: TodoSimpleFrequency;
  interval: number;
  weekdays: number[];
  dayOfMonth: number;
  endMode: TodoEndMode;
  endValueDate: string;
  endValueCount: number;
}

export interface TodoDateTimeParts {
  date: string;
  time: string;
}

export const TODO_REPEAT_PRESET_OPTIONS: Array<{ label: string; value: TodoRepeatPreset }> = [
  { label: "不重复", value: "none" },
  { label: "每天", value: "daily" },
  { label: "工作日", value: "workday" },
  { label: "每周", value: "weekly" },
  { label: "每月", value: "monthly" },
  { label: "自定义", value: "custom" },
  { label: "高级 Cron", value: "cron" },
];

export const TODO_WEEKDAY_OPTIONS = [
  { label: "周一", value: 1 },
  { label: "周二", value: 2 },
  { label: "周三", value: 3 },
  { label: "周四", value: 4 },
  { label: "周五", value: 5 },
  { label: "周六", value: 6 },
  { label: "周日", value: 7 },
];

const DEFAULT_TIME = "09:00";
const WORKDAY_WEEKDAYS = [1, 2, 3, 4, 5];

function pad(value: number) {
  return String(value).padStart(2, "0");
}

export function getTodayDateString() {
  const now = new Date();
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

export function getCreateDraftDefaultDateTime(now = new Date()): TodoDateTimeParts {
  const tomorrow = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
  const minutesSinceMidnight = now.getHours() * 60 + now.getMinutes();
  const roundedMinutes = Math.floor(minutesSinceMidnight / 5) * 5 + 5;
  const normalizedMinutes = roundedMinutes % (24 * 60);

  return {
    date: `${tomorrow.getFullYear()}-${pad(tomorrow.getMonth() + 1)}-${pad(tomorrow.getDate())}`,
    time: `${pad(Math.floor(normalizedMinutes / 60))}:${pad(normalizedMinutes % 60)}`,
  };
}

export function isFiveMinuteTime(value: string) {
  const [hourText, minuteText, ...rest] = value.split(":");
  if (rest.length > 0) return false;
  const hour = Number(hourText);
  const minute = Number(minuteText);
  return Number.isInteger(hour)
    && hour >= 0
    && hour <= 23
    && Number.isInteger(minute)
    && minute >= 0
    && minute <= 59
    && minute % 5 === 0;
}

export function isFiveMinuteDateTime(value: string) {
  const date = new Date(value);
  return !Number.isNaN(date.getTime()) && date.getMinutes() % 5 === 0 && date.getSeconds() === 0;
}

export function splitDateTimeParts(value?: string | null): TodoDateTimeParts {
  if (!value) return { date: "", time: "" };
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return { date: "", time: "" };
  return {
    date: `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`,
    time: `${pad(date.getHours())}:${pad(date.getMinutes())}`,
  };
}

export function splitDateTime(value?: string | null, fallbackTime = DEFAULT_TIME): TodoDateTimeParts {
  const parts = splitDateTimeParts(value);
  return {
    date: parts.date,
    time: parts.time || fallbackTime,
  };
}

export function combineDateTimeParts(date: string, time: string) {
  const normalizedDate = date.trim();
  const normalizedTime = time.trim();
  if (!normalizedDate || !normalizedTime) return null;
  const match = normalizedDate.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  const timeMatch = normalizedTime.match(/^(\d{2}):(\d{2})$/);
  if (!match || !timeMatch) return null;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  const hour = Number(timeMatch[1]);
  const minute = Number(timeMatch[2]);
  const next = new Date(year, month - 1, day, hour, minute, 0, 0);
  if (
    Number.isNaN(next.getTime())
    || next.getFullYear() !== year
    || next.getMonth() !== month - 1
    || next.getDate() !== day
    || next.getHours() !== hour
    || next.getMinutes() !== minute
  ) {
    return null;
  }
  return next.toISOString();
}

export function combineLocalDateTime(date: string, time: string) {
  return combineDateTimeParts(date, time);
}

export function weekdayFromDate(date: string) {
  const value = combineDateTimeParts(date, "00:00");
  if (!value) return 1;
  const weekday = new Date(value).getDay();
  return weekday === 0 ? 7 : weekday;
}

export function dayOfMonthFromDate(date: string) {
  const value = combineDateTimeParts(date, "00:00");
  if (!value) return 1;
  const day = new Date(value).getDate();
  return Math.min(31, Math.max(1, day));
}

function normalizeWeekdays(weekdays?: number[], fallback: number[] = WORKDAY_WEEKDAYS) {
  const unique = [...new Set((weekdays || []).filter((day) => Number.isInteger(day) && day >= 1 && day <= 7))]
    .sort((left, right) => left - right);
  return unique.length > 0 ? unique : [...fallback];
}

export function inferRepeatPreset(recurrence: TodoRecurrence | null): TodoRepeatPreset {
  if (!recurrence) return "none";
  if (recurrence.ruleMode === "cron") return "cron";
  const rule = recurrence.rule as Partial<TodoSimpleRule>;
  if (rule.frequency === "daily" && Number(rule.interval || 1) === 1) return "daily";
  if (
    rule.frequency === "weekly"
    && Number(rule.interval || 1) === 1
    && normalizeWeekdays(rule.weekdays).join(",") === WORKDAY_WEEKDAYS.join(",")
  ) {
    return "workday";
  }
  if (rule.frequency === "weekly" && Number(rule.interval || 1) === 1) return "weekly";
  if (rule.frequency === "monthly" && Number(rule.interval || 1) === 1) return "monthly";
  return "custom";
}

export const deriveRepeatPreset = inferRepeatPreset;

export function createRepeatDraft(recurrence: TodoRecurrence | null, startAt?: string | null): TodoRepeatDraft {
  const startParts = splitDateTimeParts(startAt || recurrence?.startAt || null);
  const fallbackWeekday = weekdayFromDate(startParts.date);
  const fallbackDayOfMonth = dayOfMonthFromDate(startParts.date);
  const simpleRule = (recurrence?.rule || {}) as Partial<TodoSimpleRule>;
  return {
    preset: inferRepeatPreset(recurrence),
    ruleMode: recurrence?.ruleMode || "simple",
    cronExpression: (recurrence?.rule as { expression?: string } | undefined)?.expression || recurrence?.cronExpression || "0 0 9 * * 1-5",
    timezone: recurrence?.timezone || "local",
    frequency: (simpleRule.frequency || "daily") as TodoSimpleFrequency,
    interval: Math.max(1, Number(simpleRule.interval || 1)),
    weekdays: normalizeWeekdays(simpleRule.weekdays, [fallbackWeekday]),
    dayOfMonth: Math.min(31, Math.max(1, Number(simpleRule.dayOfMonth || fallbackDayOfMonth || 1))),
    endMode: recurrence?.endMode || "never",
    endValueDate: recurrence?.endMode === "until_date" ? String(recurrence.endValue || "") : "",
    endValueCount: recurrence?.endMode === "after_count" ? Math.max(1, Number(recurrence.endValue || 1)) : 1,
  };
}

export function buildRuleFromRepeatDraft(draft: TodoRepeatDraft, time: string): { ruleMode: TodoRuleMode; rule: TodoRule } {
  if (draft.preset === "cron") {
    return { ruleMode: "cron", rule: { expression: draft.cronExpression.trim() } };
  }
  if (draft.preset === "daily") {
    return { ruleMode: "simple", rule: { frequency: "daily", interval: 1, time } };
  }
  if (draft.preset === "workday") {
    return { ruleMode: "simple", rule: { frequency: "weekly", interval: 1, time, weekdays: [...WORKDAY_WEEKDAYS] } };
  }
  if (draft.preset === "weekly") {
    return {
      ruleMode: "simple",
      rule: {
        frequency: "weekly",
        interval: 1,
        time,
        weekdays: normalizeWeekdays(draft.weekdays, [1]),
      },
    };
  }
  if (draft.preset === "monthly") {
    return {
      ruleMode: "simple",
      rule: {
        frequency: "monthly",
        interval: 1,
        time,
        dayOfMonth: Math.min(31, Math.max(1, Number(draft.dayOfMonth || 1))),
      },
    };
  }
  if (draft.frequency === "weekly") {
    return {
      ruleMode: "simple",
      rule: {
        frequency: "weekly",
        interval: Math.max(1, Number(draft.interval || 1)),
        time,
        weekdays: normalizeWeekdays(draft.weekdays, [1]),
      },
    };
  }
  if (draft.frequency === "monthly") {
    return {
      ruleMode: "simple",
      rule: {
        frequency: "monthly",
        interval: Math.max(1, Number(draft.interval || 1)),
        time,
        dayOfMonth: Math.min(31, Math.max(1, Number(draft.dayOfMonth || 1))),
      },
    };
  }
  return {
    ruleMode: "simple",
    rule: {
      frequency: "daily",
      interval: Math.max(1, Number(draft.interval || 1)),
      time,
    },
  };
}

export function buildSimpleRuleFromPreset(input: {
  preset: TodoRepeatPreset;
  startDate?: string;
  time?: string;
  currentRule?: TodoRule | null;
}): TodoSimpleRule {
  const { preset, startDate = "", time = DEFAULT_TIME, currentRule } = input;
  const currentSimpleRule = currentRule && "frequency" in currentRule ? currentRule : null;
  switch (preset) {
    case "daily":
      return { frequency: "daily", interval: 1, time };
    case "workday":
      return { frequency: "weekly", interval: 1, time, weekdays: [...WORKDAY_WEEKDAYS] };
    case "weekly":
      return {
        frequency: "weekly",
        interval: 1,
        time,
        weekdays: normalizeWeekdays(
          currentSimpleRule?.frequency === "weekly" ? currentSimpleRule.weekdays : [weekdayFromDate(startDate)],
          [weekdayFromDate(startDate)],
        ),
      };
    case "monthly":
      return {
        frequency: "monthly",
        interval: 1,
        time,
        dayOfMonth: Math.min(
          31,
          Math.max(
            1,
            Number(
              currentSimpleRule?.frequency === "monthly"
                ? currentSimpleRule.dayOfMonth || dayOfMonthFromDate(startDate)
                : dayOfMonthFromDate(startDate),
            ),
          ),
        ),
      };
    case "custom":
      return {
        frequency: (currentSimpleRule?.frequency || "daily") as TodoSimpleFrequency,
        interval: Math.max(1, Number(currentSimpleRule?.interval || 1)),
        time,
        weekdays: currentSimpleRule?.frequency === "weekly" ? normalizeWeekdays(currentSimpleRule.weekdays) : undefined,
        dayOfMonth: currentSimpleRule?.frequency === "monthly"
          ? Math.min(31, Math.max(1, Number(currentSimpleRule.dayOfMonth || dayOfMonthFromDate(startDate))))
          : undefined,
      };
    default:
      return {
        frequency: (currentSimpleRule?.frequency || "daily") as TodoSimpleFrequency,
        interval: Math.max(1, Number(currentSimpleRule?.interval || 1)),
        time,
      };
  }
}

function formatWeekdayList(weekdays?: number[]) {
  return normalizeWeekdays(weekdays, [])
    .map((day) => TODO_WEEKDAY_OPTIONS.find((option) => option.value === day)?.label || `周${day}`)
    .join("、");
}

export function repeatPresetLabel(preset: TodoRepeatPreset) {
  return TODO_REPEAT_PRESET_OPTIONS.find((option) => option.value === preset)?.label || "自定义";
}

export function formatRecurrenceEndLabel(recurrence: TodoRecurrence | null, formatDateTime: (value?: string | null) => string) {
  if (!recurrence) return "持续生成";
  if (recurrence.endMode === "until_date") return recurrence.endValue ? `结束于 ${formatDateTime(String(recurrence.endValue))}` : "结束时间未设置";
  if (recurrence.endMode === "after_count") return recurrence.endValue ? `生成 ${recurrence.endValue} 次后停止` : "生成次数未设置";
  return "持续生成";
}

export function summarizeSimpleRule(rule: Partial<TodoSimpleRule>) {
  const time = rule.time || DEFAULT_TIME;
  if (rule.frequency === "weekly") return `每周 ${formatWeekdayList(rule.weekdays) || "周一"} ${time}`;
  if (rule.frequency === "monthly") return `每月 ${rule.dayOfMonth || 1} 号 ${time}`;
  return `每天 ${time}`;
}

export function summarizeRecurrenceRule(recurrence: TodoRecurrence | null, formatDateTime: (value?: string | null) => string) {
  if (!recurrence) return "-";
  const startText = recurrence.startAt ? `从 ${formatDateTime(recurrence.startAt)} 开始` : "未设置开始时间";
  const ruleText = recurrence.ruleMode === "cron"
    ? recurrence.cronExpression || (recurrence.rule as { expression?: string })?.expression || "-"
    : summarizeSimpleRule(recurrence.rule as Partial<TodoSimpleRule>);
  return `${startText} · ${ruleText} · ${formatRecurrenceEndLabel(recurrence, formatDateTime)}`;
}

export function normalizeEndMode(value: string): TodoEndMode {
  if (value === "until_date" || value === "after_count") return value;
  return "never";
}

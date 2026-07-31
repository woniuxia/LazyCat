import { computed, type Ref } from "vue";
import { ElMessageBox } from "element-plus";
import type {
  TodoEndMode,
  TodoItem,
  TodoKind,
  TodoPriority,
  TodoReminderPreset,
  TodoRepeatPreset,
  TodoRule,
  TodoRuleMode,
  TodoSimpleRule,
} from "../types";
import {
  TODO_REPEAT_PRESET_OPTIONS,
  TODO_WEEKDAY_OPTIONS,
  buildSimpleRuleFromPreset,
  combineLocalDateTime,
  deriveRepeatPreset,
  getCreateDraftDefaultDateTime,
} from "../utils/todoSchedule";
import { normalizeReminderPresets } from "./useTodoItem";

export type SelectTypeValue = number | string | undefined;
export type SelectAssigneeValue = number | string;

export interface TodoItemDraft {
  id: number;
  rootId: number;
  title: string;
  typeId: SelectTypeValue;
  priority: TodoPriority;
  description: string;
  assigneeIds: SelectAssigneeValue[];
  links: { url: string; title: string }[];
  eventDate: string;
  eventTime: string;
  reminderPresets: TodoReminderPreset[];
  repeatPreset: TodoRepeatPreset;
  ruleMode: TodoRuleMode;
  timezone: string;
  cronExpression: string;
  endMode: TodoEndMode;
  endValueDate: string;
  endValueCount: number;
  simple: {
    frequency: TodoSimpleRule["frequency"];
    interval: number;
    time: string;
    weekdays: number[];
    dayOfMonth: number;
  };
  projectId: number | null;
  pmItemId: number | null;
  pmItemTitle: string | null;
  pmItemProjectId: number | null;
  pmItemStatus: string | null;
  actionType: string | null;
  actionTargetId: string | null;
  /** 历史遗留：面板从未写入该字段，运行时恒为 undefined，仅为兼容既有判断保留 */
  kind?: TodoKind;
}

export interface TodoScheduleFieldsOptions {
  itemDraft: TodoItemDraft;
  lastReminderPresetSelection: Ref<TodoReminderPreset[]>;
  editingItemSnapshot: Ref<TodoItem | null>;
  itemDialogMode: Ref<"create" | "edit_item">;
  itemKindOf: (item: TodoItem) => TodoKind;
}

export function useTodoScheduleFields(options: TodoScheduleFieldsOptions) {
  const {
    itemDraft,
    lastReminderPresetSelection,
    editingItemSnapshot,
    itemDialogMode,
    itemKindOf,
  } = options;

  const reminderPresetOptions: Array<{ label: string; value: TodoReminderPreset }> = [
    { label: "不提醒", value: "none" },
    { label: "准时提醒", value: "0m" },
    { label: "提前五分钟", value: "5m" },
    { label: "提前十分钟", value: "10m" },
    { label: "提前半个小时", value: "30m" },
    { label: "提前一个小时", value: "1h" },
    { label: "提前一天", value: "1d" },
    { label: "提前两天", value: "2d" },
  ];

  function pad2(value: number) {
    return String(value).padStart(2, "0");
  }

  function splitDraftEventTime(value: string) {
    if (!value.trim()) return { hour: "", minute: "" };
    const [hourText = "", minuteText = ""] = value.split(":");
    const hour = Number(hourText);
    const minute = Number(minuteText);
    return {
      hour: Number.isInteger(hour) && hour >= 0 && hour <= 23 ? pad2(hour) : "",
      minute:
        Number.isInteger(minute) && minute >= 0 && minute <= 55 && minute % 5 === 0
          ? pad2(minute)
          : "",
    };
  }

  function composeDraftEventTime(hour: string, minute: string) {
    if (!hour && !minute) return "";
    return `${hour || "00"}:${minute || "00"}`;
  }

  const hourOptions = Array.from({ length: 24 }, (_item, index) => {
    const value = pad2(index);
    return { label: value, value };
  });

  const minuteOptions = Array.from({ length: 12 }, (_item, index) => {
    const value = pad2(index * 5);
    return { label: value, value };
  });

  const repeatPresetOptions = TODO_REPEAT_PRESET_OPTIONS;
  const weekdayOptions = TODO_WEEKDAY_OPTIONS;

  const isRepeating = computed(() => itemDraft.repeatPreset !== "none");

  const editingItemIsRecurring = computed(
    () => !!editingItemSnapshot.value && itemKindOf(editingItemSnapshot.value) === "recurring",
  );
  const showRecurrenceFields = computed(() => {
    return isRepeating.value;
  });
  const showCustomRepeatFields = computed(
    () => isRepeating.value && itemDraft.repeatPreset === "custom",
  );
  const showCronRepeatFields = computed(
    () => isRepeating.value && itemDraft.repeatPreset === "cron",
  );
  const eventHour = computed({
    get: () => splitDraftEventTime(itemDraft.eventTime).hour,
    set: (value: string) => {
      const { minute } = splitDraftEventTime(itemDraft.eventTime);
      itemDraft.eventTime = composeDraftEventTime(value, minute);
    },
  });
  const eventMinute = computed({
    get: () => splitDraftEventTime(itemDraft.eventTime).minute,
    set: (value: string) => {
      const { hour } = splitDraftEventTime(itemDraft.eventTime);
      itemDraft.eventTime = composeDraftEventTime(hour, value);
    },
  });

  function buildRulePayload(): TodoRule {
    if (itemDraft.repeatPreset === "cron" || itemDraft.ruleMode === "cron") {
      return { expression: itemDraft.cronExpression.trim() };
    }
    return buildSimpleRuleFromPreset({
      preset: itemDraft.repeatPreset,
      startDate: itemDraft.eventDate,
      time: itemDraft.eventTime,
      currentRule: {
        frequency: itemDraft.simple.frequency,
        interval: Math.max(1, Number(itemDraft.simple.interval || 1)),
        time: itemDraft.eventTime || itemDraft.simple.time || "09:00",
        weekdays: itemDraft.simple.weekdays,
        dayOfMonth: Math.min(31, Math.max(1, Number(itemDraft.simple.dayOfMonth || 1))),
      },
    });
  }

  function buildEndValue() {
    if (itemDraft.endMode === "until_date") return itemDraft.endValueDate || null;
    if (itemDraft.endMode === "after_count")
      return Math.max(1, Number(itemDraft.endValueCount || 1));
    return null;
  }

  function buildEventAt() {
    return combineLocalDateTime(itemDraft.eventDate, itemDraft.eventTime);
  }

  function syncSimpleDraftFromRule(rule: TodoRule) {
    if (!("frequency" in rule)) return;
    itemDraft.simple.frequency = rule.frequency;
    itemDraft.simple.interval = Math.max(1, Number(rule.interval || 1));
    itemDraft.simple.time = rule.time || "09:00";
    itemDraft.eventTime = rule.time || itemDraft.eventTime || "09:00";
    itemDraft.simple.weekdays =
      Array.isArray(rule.weekdays) && rule.weekdays.length > 0
        ? [...rule.weekdays]
        : [1, 2, 3, 4, 5];
    itemDraft.simple.dayOfMonth = Math.min(31, Math.max(1, Number(rule.dayOfMonth || 1)));
  }

  function applyRepeatPresetRule(preset: TodoRepeatPreset) {
    itemDraft.repeatPreset = preset;
    if (preset === "cron") {
      itemDraft.ruleMode = "cron";
      if (!itemDraft.eventDate) itemDraft.eventDate = getCreateDraftDefaultDateTime().date;
      return;
    }
    itemDraft.ruleMode = "simple";
    const nextRule = buildSimpleRuleFromPreset({
      preset,
      startDate: itemDraft.eventDate,
      time: itemDraft.eventTime,
      currentRule: {
        frequency: itemDraft.simple.frequency,
        interval: itemDraft.simple.interval,
        time: itemDraft.eventTime,
        weekdays: itemDraft.simple.weekdays,
        dayOfMonth: itemDraft.simple.dayOfMonth,
      },
    });
    syncSimpleDraftFromRule(nextRule);
  }

  async function onRepeatPresetChange(nextPreset: TodoRepeatPreset) {
    if (nextPreset === "none") {
      if (itemDialogMode.value === "edit_item" && editingItemIsRecurring.value) {
        try {
          await ElMessageBox.confirm(
            "将此重复事项改为不重复，后续将不再自动生成实例。确认吗？",
            "取消重复",
            { type: "warning" },
          );
        } catch {
          itemDraft.repeatPreset = deriveRepeatPreset(
            editingItemSnapshot.value?.recurrence || null,
          );
          return;
        }
      }
      itemDraft.repeatPreset = "none";
      itemDraft.ruleMode = "simple";
      return;
    }
    if (itemDraft.actionType) {
      try {
        await ElMessageBox.confirm(
          "切换为重复事项会解除已配置的执行动作。确认吗？",
          "解除执行动作",
          { type: "warning" },
        );
      } catch {
        itemDraft.repeatPreset = "none";
        return;
      }
      itemDraft.actionType = null;
      itemDraft.actionTargetId = null;
    }
    applyRepeatPresetRule(nextPreset);
  }

  function onCustomFrequencyChange() {
    if (itemDraft.repeatPreset !== "custom") return;
    applyRepeatPresetRule("custom");
  }

  function onReminderPresetsChange(values: TodoReminderPreset[]) {
    const previousSelection = lastReminderPresetSelection.value;
    const nextHasNone = values.includes("none");
    const previousHasNone = previousSelection.includes("none");

    let normalized = normalizeReminderPresets(values);
    if (nextHasNone && !previousHasNone) {
      normalized = ["none"];
    } else if (nextHasNone && previousHasNone) {
      normalized = normalizeReminderPresets(values.filter((value) => value !== "none"));
    }

    itemDraft.reminderPresets = normalized;
    lastReminderPresetSelection.value = [...normalized];
  }

  function resetReminderPresetsToNone() {
    itemDraft.reminderPresets = ["none"];
    lastReminderPresetSelection.value = ["none"];
  }

  function clearEventSchedule() {
    itemDraft.eventDate = "";
    itemDraft.eventTime = "";
    resetReminderPresetsToNone();
  }

  function fillDefaultDateTime() {
    const defaults = getCreateDraftDefaultDateTime();
    itemDraft.eventDate = defaults.date;
    itemDraft.eventTime = defaults.time;
    itemDraft.simple.time = defaults.time;
    if (itemDraft.reminderPresets.length === 1 && itemDraft.reminderPresets[0] === "none") {
      itemDraft.reminderPresets = ["0m"];
      lastReminderPresetSelection.value = ["0m"];
    }
  }

  function fillQuickDate(daysOffset: number) {
    const target = new Date();
    target.setDate(target.getDate() + daysOffset);
    const year = target.getFullYear();
    const month = pad2(target.getMonth() + 1);
    const day = pad2(target.getDate());
    itemDraft.eventDate = `${year}-${month}-${day}`;
    if (itemDraft.reminderPresets.length === 1 && itemDraft.reminderPresets[0] === "none") {
      itemDraft.reminderPresets = ["0m"];
      lastReminderPresetSelection.value = ["0m"];
    }
  }

  return {
    reminderPresetOptions,
    hourOptions,
    minuteOptions,
    repeatPresetOptions,
    weekdayOptions,
    splitDraftEventTime,
    composeDraftEventTime,
    isRepeating,
    showRecurrenceFields,
    showCustomRepeatFields,
    showCronRepeatFields,
    eventHour,
    eventMinute,
    buildRulePayload,
    buildEndValue,
    buildEventAt,
    syncSimpleDraftFromRule,
    onRepeatPresetChange,
    onCustomFrequencyChange,
    onReminderPresetsChange,
    clearEventSchedule,
    fillDefaultDateTime,
    fillQuickDate,
  };
}

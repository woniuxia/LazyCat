export type TodoPriority = "P0" | "P1" | "P2" | "P3";

export type TodoStatus = "pending" | "in_progress" | "completed" | "canceled";

export type TodoKind = "one_off" | "recurring";

export type TodoRecordRole = "root" | "occurrence";

export type TodoRuleMode = "simple" | "cron";

export type TodoEndMode = "never" | "until_date" | "after_count";

export type TodoEditScope = "this_instance" | "future_instances";

export type TodoReminderPreset = "0m" | "none" | "5m" | "10m" | "30m" | "1h" | "1d" | "2d";

export type TodoSimpleFrequency = "daily" | "weekly" | "monthly";

export type TodoRepeatPreset = "none" | "daily" | "workday" | "weekly" | "monthly" | "custom" | "cron";

export interface TodoType {
  id: number;
  name: string;
  color: string;
  builtin: boolean;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
}

export interface TodoAssignee {
  id: number;
  name: string;
  createdAt: string;
  updatedAt: string;
}

export interface TodoSimpleRule {
  frequency: TodoSimpleFrequency;
  interval: number;
  time: string;
  weekdays?: number[];
  dayOfMonth?: number;
}

export interface TodoCronRule {
  expression: string;
}

export type TodoRule = TodoSimpleRule | TodoCronRule;

export interface TodoRecurrence {
  startAt: string | null;
  ruleMode: TodoRuleMode;
  rule: TodoRule;
  cronExpression: string;
  timezone: string;
  endMode: TodoEndMode;
  endValue: string | number | null;
  nextOccurrenceAt: string | null;
  generatedCount: number;
  active: boolean;
}

export interface TodoItem {
  id: number;
  rootId: number;
  kind: TodoKind;
  recordRole: TodoRecordRole;
  pinned: boolean;
  title: string;
  typeId: number | null;
  typeName?: string | null;
  typeColor?: string | null;
  priority: TodoPriority;
  description: string;
  status: TodoStatus | null;
  eventAt: string | null;
  reminderPresets: TodoReminderPreset[];
  snoozeUntil: string | null;
  lastNotifiedAt: string | null;
  displayAt: string | null;
  assignees: TodoAssignee[];
  isOverdue: boolean;
  canEditFuture: boolean;
  nextTaskReminderId?: number | null;
  nextReminderPreset?: TodoReminderPreset | null;
  recurrence: TodoRecurrence | null;
  createdAt: string;
  updatedAt: string;
}

export interface TodoReminderEvent {
  id: number;
  taskId: number;
  taskReminderId?: number | null;
  title: string;
  body: string;
  fireAt: string;
  isRead: boolean;
  reminderPreset: TodoReminderPreset | "";
  createdAt: string;
}

export interface TodoReminderDispatch {
  eventId: number;
  taskId: number;
  taskReminderId: number;
  title: string;
  body: string;
  fireAt: string;
  reminderPreset: TodoReminderPreset | "";
  priority: TodoPriority;
}

export interface TodoRecurrenceInput {
  startAt?: string | null;
  ruleMode: TodoRuleMode;
  rule: TodoRule;
  timezone: string;
  endMode: TodoEndMode;
  endValue: string | number | null;
  active?: boolean;
}

export interface TodoItemUpsertPayload {
  id?: number;
  rootId?: number;
  kind: TodoKind;
  recordRole?: TodoRecordRole;
  title: string;
  typeId: number | null;
  priority: TodoPriority;
  description: string;
  assigneeIds: number[];
  eventAt?: string | null;
  reminderPresets?: TodoReminderPreset[];
  status?: TodoStatus;
  scope?: TodoEditScope;
  recurrence?: TodoRecurrenceInput | null;
}

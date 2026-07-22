import type {
  GlobalNotification,
  GlobalNotificationAction,
  ReleasePackageNotification,
  ReleasePackageNotificationStatus,
  TodoReminderNotification,
} from "../types/global-notification";

const TODO_PRIORITIES = new Set(["P0", "P1", "P2", "P3"]);
const TODO_REMINDER_PRESETS = new Set(["", "0m", "none", "5m", "10m", "30m", "1h", "1d", "2d"]);
const RELEASE_PACKAGE_STATUSES = new Set([
  "succeeded",
  "partially_succeeded",
  "package_succeeded_upload_failed",
  "failed",
  "cancelled",
]);

function invalidNotification(): never {
  throw new Error("无效的全局通知");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function isPositiveSafeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0;
}

function hasValidCommonFields(value: Record<string, unknown>): boolean {
  return isNonEmptyString(value.id) && isNonEmptyString(value.createdAt);
}

function isTodoReminderNotification(value: unknown): value is TodoReminderNotification {
  if (!isRecord(value)) return false;
  return value.kind === "todo-reminder"
    && hasValidCommonFields(value)
    && isPositiveSafeInteger(value.eventId)
    && value.id === `todo-reminder:${value.eventId}`
    && isPositiveSafeInteger(value.taskId)
    && isPositiveSafeInteger(value.taskReminderId)
    && typeof value.title === "string"
    && typeof value.body === "string"
    && isNonEmptyString(value.fireAt)
    && typeof value.reminderPreset === "string"
    && TODO_REMINDER_PRESETS.has(value.reminderPreset)
    && typeof value.priority === "string"
    && TODO_PRIORITIES.has(value.priority);
}

function isReleasePackageNotification(value: unknown): value is ReleasePackageNotification {
  if (!isRecord(value)) return false;
  return value.kind === "release-package"
    && hasValidCommonFields(value)
    && isNonEmptyString(value.runId)
    && value.id === `release-package:${value.runId}`
    && isPositiveSafeInteger(value.projectId)
    && isNonEmptyString(value.projectName)
    && typeof value.status === "string"
    && RELEASE_PACKAGE_STATUSES.has(value.status)
    && (value.archivePath === undefined || typeof value.archivePath === "string")
    && (value.error === undefined || typeof value.error === "string");
}

function parseGlobalNotification(value: unknown): GlobalNotification {
  if (isTodoReminderNotification(value)) return value;
  if (isReleasePackageNotification(value)) return value;
  return invalidNotification();
}

export function normalizeGlobalNotificationPayload(payload: unknown): GlobalNotification[] {
  if (payload === null || payload === undefined) return [];
  if (Array.isArray(payload)) return payload.map(parseGlobalNotification);
  return [parseGlobalNotification(payload)];
}

export function mergeGlobalNotificationQueue(
  current: readonly GlobalNotification[],
  incoming: readonly GlobalNotification[],
): GlobalNotification[] {
  const next = [...current];
  const knownIds = new Set(current.map((notification) => notification.id));

  for (const notification of incoming) {
    if (knownIds.has(notification.id)) continue;
    knownIds.add(notification.id);
    next.push(notification);
  }

  return next;
}

export function globalNotificationActions(
  notification: GlobalNotification,
): GlobalNotificationAction[] {
  if (notification.kind === "todo-reminder") {
    return ["complete", "dismiss", "snooze"];
  }

  const actions: GlobalNotificationAction[] = ["open-tool"];
  if (notification.status !== "failed" && notification.archivePath?.trim()) {
    actions.push("open-directory");
  }
  actions.push("acknowledge");
  return actions;
}

export function releasePackageNotificationCopy(
  status: ReleasePackageNotificationStatus,
): { title: string; detail: string } {
  if (status === "succeeded") {
    return { title: "上线包打包成功", detail: "所选产物已完成归档" };
  }
  if (status === "partially_succeeded") {
    return { title: "上线包部分成功", detail: "可用产物已归档，请查看失败日志" };
  }
  if (status === "package_succeeded_upload_failed") {
    return { title: "上线包上传失败", detail: "本地归档已完成，服务器上传失败" };
  }
  if (status === "cancelled") {
    return { title: "上线包任务已终止", detail: "任务已终止，请查看日志确认产物状态" };
  }
  return { title: "上线包打包失败", detail: "未生成可用归档，请查看打包日志" };
}

export function summarizeNotificationError(error: string | undefined, maxLength = 180): string {
  if (!error) return "";
  if (error.length <= maxLength) return error;
  if (maxLength <= 3) return "...".slice(0, Math.max(0, maxLength));
  return `${error.slice(0, maxLength - 3)}...`;
}

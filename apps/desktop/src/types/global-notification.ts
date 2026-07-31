import type { TodoPriority, TodoReminderActionSummary, TodoReminderPreset } from "./todo";
import type { ReleasePackageEnvironmentKind, ReleasePackageType } from "./release-package";

export type ReleasePackageNotificationStatus =
  | "succeeded"
  | "partially_succeeded"
  | "package_succeeded_upload_failed"
  | "upload_succeeded_command_failed"
  | "deployed_health_check_failed"
  | "failed"
  | "cancelled";

export type GlobalNotificationAction =
  | "complete"
  | "dispatch-action"
  | "dismiss"
  | "snooze"
  | "open-tool"
  | "open-directory"
  | "acknowledge";

interface GlobalNotificationBase {
  id: string;
  createdAt: string;
}

export interface TodoReminderNotification extends GlobalNotificationBase {
  kind: "todo-reminder";
  eventId: number;
  taskId: number;
  taskReminderId: number;
  title: string;
  body: string;
  fireAt: string;
  reminderPreset: TodoReminderPreset | "";
  priority: TodoPriority;
  action?: TodoReminderActionSummary;
}

export interface ReleasePackageNotification extends GlobalNotificationBase {
  kind: "release-package";
  runId: string;
  environmentId: number;
  environment: ReleasePackageEnvironmentKind;
  projectId: number;
  projectName: string;
  packageType: ReleasePackageType;
  status: ReleasePackageNotificationStatus;
  archivePath?: string;
  error?: string;
}

export interface ActionCombinationNotification extends GlobalNotificationBase {
  kind: "action-combination";
  runId: string;
  combinationId: number;
  combinationName: string;
  status: "succeeded" | "partially_succeeded" | "failed";
  failedStepLabels: string[];
  error?: string;
}

export type GlobalNotification =
  | TodoReminderNotification
  | ReleasePackageNotification
  | ActionCombinationNotification;

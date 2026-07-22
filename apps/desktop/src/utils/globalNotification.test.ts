import { describe, expect, it } from "vitest";

import {
  globalNotificationActions,
  mergeGlobalNotificationQueue,
  normalizeGlobalNotificationPayload,
  releasePackageNotificationCopy,
  summarizeNotificationError,
} from "./globalNotification";

const todoNotification = {
  kind: "todo-reminder" as const,
  id: "todo-reminder:41",
  createdAt: "2026-07-21T08:00:00.000Z",
  eventId: 41,
  taskId: 7,
  taskReminderId: 13,
  title: "提交周报",
  body: "今天 18:00 前提交",
  fireAt: "2026-07-21T10:00:00.000Z",
  reminderPreset: "10m" as const,
  priority: "P1" as const,
};

const succeededNotification = {
  kind: "release-package" as const,
  id: "release-package:run-1",
  createdAt: "2026-07-21T08:10:00.000Z",
  runId: "run-1",
  projectId: 9,
  projectName: "客户门户",
  status: "succeeded" as const,
  archivePath: "D:\\releases\\customer-portal",
};

describe("normalizeGlobalNotificationPayload", () => {
  it("normalizes a single notification", () => {
    expect(normalizeGlobalNotificationPayload(todoNotification)).toEqual([todoNotification]);
  });

  it("normalizes an array payload", () => {
    expect(normalizeGlobalNotificationPayload([todoNotification, succeededNotification])).toEqual([
      todoNotification,
      succeededNotification,
    ]);
  });

  it("normalizes null to an empty array", () => {
    expect(normalizeGlobalNotificationPayload(null)).toEqual([]);
  });

  it.each(["package_succeeded_upload_failed", "cancelled"] as const)(
    "accepts release terminal status %s",
    (status) => {
      expect(normalizeGlobalNotificationPayload({ ...succeededNotification, status })).toEqual([
        { ...succeededNotification, status },
      ]);
    },
  );

  it("throws explicitly for an invalid notification kind", () => {
    expect(() => normalizeGlobalNotificationPayload({ ...todoNotification, kind: "unknown" })).toThrow(
      "无效的全局通知",
    );
  });

  it("throws explicitly when required fields are missing", () => {
    const { taskId: _taskId, ...invalid } = todoNotification;

    expect(() => normalizeGlobalNotificationPayload(invalid)).toThrow("无效的全局通知");
  });

  it("accepts the empty reminder preset used by the existing reminder contract", () => {
    expect(normalizeGlobalNotificationPayload({ ...todoNotification, reminderPreset: "" })).toEqual([
      { ...todoNotification, reminderPreset: "" },
    ]);
  });

  it.each([
    ["eventId", 0],
    ["eventId", -1],
    ["eventId", 1.5],
    ["eventId", Number.MAX_SAFE_INTEGER + 1],
    ["taskId", 0],
    ["taskId", -1],
    ["taskId", 1.5],
    ["taskId", Number.MAX_SAFE_INTEGER + 1],
    ["taskReminderId", 0],
    ["taskReminderId", -1],
    ["taskReminderId", 1.5],
    ["taskReminderId", Number.MAX_SAFE_INTEGER + 1],
  ] as const)("rejects invalid todo identifier %s=%s", (field, value) => {
    const id = field === "eventId" ? `todo-reminder:${value}` : todoNotification.id;

    expect(() => normalizeGlobalNotificationPayload({ ...todoNotification, id, [field]: value }))
      .toThrow("无效的全局通知");
  });

  it.each([0, -1, 1.5, Number.MAX_SAFE_INTEGER + 1])(
    "rejects invalid release projectId=%s",
    (projectId) => {
      expect(() => normalizeGlobalNotificationPayload({ ...succeededNotification, projectId })).toThrow(
        "无效的全局通知",
      );
    },
  );

  it.each<{ notification: unknown; label: string }>([
    {
      notification: { ...todoNotification, id: "todo-reminder:42" },
      label: "mismatched todo identity",
    },
    {
      notification: { ...succeededNotification, id: "release-package:run-2" },
      label: "mismatched release identity",
    },
    {
      notification: { ...todoNotification, id: succeededNotification.id },
      label: "release id on a todo notification",
    },
    {
      notification: { ...succeededNotification, id: todoNotification.id },
      label: "todo id on a release notification",
    },
  ])("rejects $label", ({ notification }) => {
    expect(() => normalizeGlobalNotificationPayload(notification)).toThrow("无效的全局通知");
  });
});

describe("mergeGlobalNotificationQueue", () => {
  it("preserves FIFO order and de-duplicates notifications by id", () => {
    const incoming = {
      ...succeededNotification,
      id: "release-package:run-2",
      runId: "run-2",
    };

    expect(
      mergeGlobalNotificationQueue(
        [todoNotification],
        [todoNotification, incoming, incoming],
      ).map((notification) => notification.id),
    ).toEqual(["todo-reminder:41", "release-package:run-2"]);
  });
});

describe("globalNotificationActions", () => {
  it("returns todo reminder actions", () => {
    expect(globalNotificationActions(todoNotification)).toEqual(["complete", "dismiss", "snooze"]);
  });

  it("returns archive actions for a successful package with a non-empty archive path", () => {
    expect(globalNotificationActions(succeededNotification)).toEqual([
      "open-tool",
      "open-directory",
      "acknowledge",
    ]);
  });

  it("omits the directory action when a successful package has no archive path", () => {
    expect(
      globalNotificationActions({
        ...succeededNotification,
        status: "partially_succeeded",
        archivePath: "",
      }),
    ).toEqual(["open-tool", "acknowledge"]);
  });

  it("never returns the directory action for a failed package", () => {
    expect(
      globalNotificationActions({
        ...succeededNotification,
        status: "failed",
        archivePath: "D:\\unexpected",
        error: "构建失败",
      }),
    ).toEqual(["open-tool", "acknowledge"]);
  });

  it("keeps the local archive action when upload fails after packaging", () => {
    expect(globalNotificationActions({
      ...succeededNotification,
      status: "package_succeeded_upload_failed",
      error: "上传失败",
    })).toEqual(["open-tool", "open-directory", "acknowledge"]);
  });
});

describe("releasePackageNotificationCopy", () => {
  it.each([
    ["succeeded", "上线包打包成功", "所选产物已完成归档"],
    ["partially_succeeded", "上线包部分成功", "可用产物已归档，请查看失败日志"],
    ["package_succeeded_upload_failed", "上线包上传失败", "本地归档已完成，服务器上传失败"],
    ["failed", "上线包打包失败", "未生成可用归档，请查看打包日志"],
    ["cancelled", "上线包任务已终止", "任务已终止，请查看日志确认产物状态"],
  ] as const)("returns the Chinese copy for %s", (status, title, detail) => {
    expect(releasePackageNotificationCopy(status)).toEqual({ title, detail });
  });
});

describe("summarizeNotificationError", () => {
  it("returns an empty string for a missing error", () => {
    expect(summarizeNotificationError(undefined)).toBe("");
  });

  it("returns an empty error unchanged", () => {
    expect(summarizeNotificationError("")).toBe("");
  });

  it("returns a short error unchanged", () => {
    expect(summarizeNotificationError("构建命令退出码为 1", 30)).toBe("构建命令退出码为 1");
  });

  it("returns an error exactly at maxLength unchanged", () => {
    expect(summarizeNotificationError("12345", 5)).toBe("12345");
  });

  it("truncates a long error and includes the ellipsis in maxLength", () => {
    const summary = summarizeNotificationError("abcdefghijk", 8);

    expect(summary).toBe("abcde...");
    expect(summary).toHaveLength(8);
  });

  it.each([
    [0, ""],
    [1, "."],
    [2, ".."],
    [3, "..."],
  ] as const)("does not exceed maxLength=%s", (maxLength, expected) => {
    const summary = summarizeNotificationError("abcdef", maxLength);

    expect(summary).toBe(expected);
    expect(summary.length).toBeLessThanOrEqual(maxLength);
  });
});

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

const actionTodoNotification = {
  ...todoNotification,
  action: {
    bindingId: 9,
    actionType: "release_package.run",
    actionLabel: "开始打包",
    targetLabel: "客户门户",
    available: true,
  },
};

const succeededNotification = {
  kind: "release-package" as const,
  id: "release-package:run-1",
  createdAt: "2026-07-21T08:10:00.000Z",
  runId: "run-1",
  projectId: 9,
  projectName: "客户门户",
  packageType: "local_archive" as const,
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

  it.each(["cancelled"] as const)(
    "accepts release terminal status %s",
    (status) => {
      const notification = {
        ...succeededNotification,
        status,
      };
      expect(normalizeGlobalNotificationPayload(notification)).toEqual([notification]);
    },
  );

  it("accepts an upload failure notification without an archive path", () => {
    const { archivePath: _archivePath, ...uploadNotification } = succeededNotification;
    const notification = {
      ...uploadNotification,
      packageType: "server_upload" as const,
      status: "package_succeeded_upload_failed" as const,
    };

    expect(normalizeGlobalNotificationPayload(notification)).toEqual([notification]);
  });

  it("rejects an upload failure status for a local archive notification", () => {
    expect(() => normalizeGlobalNotificationPayload({
      ...succeededNotification,
      status: "package_succeeded_upload_failed",
    })).toThrow("无效的全局通知");
  });

  it("rejects an archive path on a server upload notification", () => {
    expect(() => normalizeGlobalNotificationPayload({
      ...succeededNotification,
      packageType: "server_upload",
    })).toThrow("无效的全局通知");
  });

  it.each([undefined, "archive_then_upload", ""])("rejects invalid package type %s", (packageType) => {
    expect(() => normalizeGlobalNotificationPayload({ ...succeededNotification, packageType }))
      .toThrow("无效的全局通知");
  });

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

  it("accepts a complete Todo action summary", () => {
    expect(normalizeGlobalNotificationPayload(actionTodoNotification)).toEqual([
      actionTodoNotification,
    ]);
  });

  it.each(["bindingId", "actionType", "actionLabel", "targetLabel", "available"] as const)(
    "rejects a Todo action summary without %s",
    (field) => {
      const action = { ...actionTodoNotification.action };
      delete action[field];

      expect(() =>
        normalizeGlobalNotificationPayload({ ...actionTodoNotification, action }),
      ).toThrow("无效的全局通知");
    },
  );

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

  it("replaces complete with dispatch-action for an action reminder", () => {
    expect(globalNotificationActions(actionTodoNotification)).toEqual([
      "dispatch-action",
      "dismiss",
      "snooze",
    ]);
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

  it.each(["succeeded", "failed"] as const)(
    "never returns the directory action for a server upload with status %s",
    (status) => {
      expect(globalNotificationActions({
      ...succeededNotification,
        packageType: "server_upload",
        status,
      })).toEqual(["open-tool", "acknowledge"]);
    },
  );
});

describe("releasePackageNotificationCopy", () => {
  it("uses the delivery type in release notification copy", () => {
    expect(releasePackageNotificationCopy("succeeded", "local_archive").detail).toContain("本地归档完成");
    expect(releasePackageNotificationCopy("succeeded", "server_upload").detail).toContain("服务器上传完成");
    expect(releasePackageNotificationCopy("package_succeeded_upload_failed", "server_upload").detail)
      .toContain("构建成功、上传失败");
  });

  it("describes partial server uploads as not uploaded", () => {
    const detail = releasePackageNotificationCopy("partially_succeeded", "server_upload").detail;

    expect(detail).toContain("未上传服务器");
    expect(detail).not.toContain("已上传服务器");
  });

  it("rejects impossible local archive upload failures", () => {
    expect(() => releasePackageNotificationCopy(
      "package_succeeded_upload_failed",
      "local_archive",
    )).toThrow("无效的上线包终态组合");
  });

  it.each([
    ["succeeded", "上线包打包成功", "所选产物本地归档完成"],
    ["partially_succeeded", "上线包部分成功", "可用产物本地归档完成，请查看失败日志"],
    ["failed", "上线包打包失败", "未生成可用归档，请查看打包日志"],
    ["cancelled", "上线包任务已终止", "任务已终止，请查看日志确认产物状态"],
  ] as const)("returns the Chinese copy for %s", (status, title, detail) => {
    expect(releasePackageNotificationCopy(status, "local_archive")).toEqual({ title, detail });
  });

  it.each([
    ["succeeded", "上线包上传成功", "所选产物服务器上传完成"],
    ["partially_succeeded", "上线包部分成功", "部分产物构建失败，未上传服务器"],
    ["package_succeeded_upload_failed", "上线包上传失败", "构建成功、上传失败，请查看上传日志"],
    ["failed", "上线包上传失败", "未完成可用服务器上传，请查看打包日志"],
    ["cancelled", "上线包任务已终止", "任务已终止，请查看日志确认产物状态"],
  ] as const)("returns the Chinese copy for server upload %s", (status, title, detail) => {
    expect(releasePackageNotificationCopy(status, "server_upload")).toEqual({ title, detail });
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

import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./GlobalNotificationPopup.vue", import.meta.url), "utf8");
const mainSource = readFileSync(new URL("../main.ts", import.meta.url), "utf8");

describe("GlobalNotificationPopup", () => {
  it("mounts a generic notification view and deduplicated queue", () => {
    expect(mainSource).toContain('currentView === "global-notification"');
    expect(mainSource).toContain('import("./GlobalNotificationApp")');
    expect(source).toContain("__LAZYCAT_NOTIFICATION_BOOTSTRAP__");
    expect(source).toContain("APP_EVENTS.GLOBAL_NOTIFICATION_PUSH");
    expect(source).toContain("mergeGlobalNotificationQueue");
  });

  it("keeps todo reminder actions", () => {
    expect(source).toContain('invoke("reminder_popup_complete"');
    expect(source).toContain('invoke("reminder_popup_dismiss"');
    expect(source).toContain('invoke("reminder_popup_snooze"');
  });

  it("dispatches an available Todo action from the reminder event", () => {
    expect(source).toContain("开始打包");
    expect(source).toContain("runCurrentReminderAction");
    expect(source).toContain('"tool:action-center:dispatch"');
    expect(source).toContain('triggerType: "todo_item"');
    expect(source).toContain("triggerId: String(item.taskId)");
    expect(source).toContain("triggerEventId: String(item.eventId)");
  });

  it("disables active or unavailable reminder actions with an explicit reason", () => {
    expect(source).toContain("打包待确认");
    expect(source).toContain("打包进行中");
    expect(source).toContain("unavailableReason");
    expect(source).toContain("todoPrimaryDisabled");
  });

  it("exposes package page, directory, and acknowledge actions", () => {
    expect(source).toContain('invoke("global_notification_open_tool"');
    expect(source).toContain('"tool:system:open-local-path"');
    expect(source).toContain("打开打包页面");
    expect(source).toContain("打开目标目录");
    expect(source).toContain("知道了");
    expect(source).toContain("summarizeNotificationError");
  });

  it("uses the package type when rendering the delivery result", () => {
    expect(source).toContain("releasePackageNotificationCopy(currentPackage.value.status, currentPackage.value.packageType)");
  });

  it("shows the directory action only for local archives", () => {
    expect(source).toContain('currentPackage.value?.packageType === "local_archive"');
  });

  it("removes only the current item after a successful action", () => {
    expect(source).toContain("async function removeCurrentNotification");
    expect(source).toMatch(
      /try\s*\{[\s\S]*await action\(\)[\s\S]*await removeCurrentNotification\(\)[\s\S]*\}\s*catch/s,
    );
    expect(source).toContain('@click="acknowledgeCurrent"');
  });
});

# 全局通知弹窗重构 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将任务提醒专用弹窗重构为统一全局通知窗口，并为上线包成功、部分成功和失败终态提供打开功能页、打开目标目录和“知道了”操作。

**Architecture:** Rust `global_notification` 模块拥有通知 payload、窗口生命周期和打包终态映射；任务调度与上线包运行时只向它提交通知。Vue 通知窗口用判别联合类型和纯函数维护 FIFO 去重队列，类型专属动作复用现有任务提醒命令、主窗口导航事件和 `system/open_local_path`。

**Tech Stack:** Tauri 2、Rust、Vue 3、TypeScript、Element Plus、Vitest

---

## File Map

- Create: `apps/desktop/src/types/global-notification.ts`
- Create: `apps/desktop/src/utils/globalNotification.ts`
- Create: `apps/desktop/src/utils/globalNotification.test.ts`
- Create: `apps/desktop/src-tauri/src/global_notification.rs`
- Rename: `apps/desktop/src/ReminderPopupApp.ts` -> `apps/desktop/src/GlobalNotificationApp.ts`
- Rename: `apps/desktop/src/components/ReminderPopup.vue` -> `apps/desktop/src/components/GlobalNotificationPopup.vue`
- Create: `apps/desktop/src/components/GlobalNotificationPopup.test.ts`
- Modify: `apps/desktop/src/main.ts`
- Modify: `apps/desktop/src/bridge/events.ts`
- Modify: `apps/desktop/src-tauri/src/events.rs`
- Modify: `apps/desktop/src-tauri/src/main.rs`
- Modify: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`
- Modify: `apps/desktop/src-tauri/capabilities/default.json`
- Modify: `process.md`

### Task 1: 前端通知模型与 FIFO 队列

**Files:**
- Create: `apps/desktop/src/types/global-notification.ts`
- Create: `apps/desktop/src/utils/globalNotification.ts`
- Test: `apps/desktop/src/utils/globalNotification.test.ts`

- [ ] **Step 1: 写入失败测试**

```ts
import { describe, expect, it } from "vitest";
import type { GlobalNotification } from "../types/global-notification";
import {
  globalNotificationActions,
  mergeGlobalNotificationQueue,
  normalizeGlobalNotificationPayload,
  releasePackageNotificationCopy,
  summarizeNotificationError,
} from "./globalNotification";

const todo: GlobalNotification = {
  id: "todo-reminder:9",
  kind: "todo-reminder",
  createdAt: "2026-07-21T08:00:00Z",
  eventId: 9,
  taskId: 3,
  taskReminderId: 4,
  title: "提交周报",
  body: "今天 18:00 前完成",
  fireAt: "2026-07-21T08:00:00Z",
  reminderPreset: "0m",
  priority: "P1",
};

const success: GlobalNotification = {
  id: "release-package:run-1",
  kind: "release-package",
  createdAt: "2026-07-21T08:05:00Z",
  runId: "run-1",
  projectId: 7,
  projectName: "客户门户",
  status: "succeeded",
  archivePath: "D:\\release\\20260723-客户门户",
};

describe("global notification model", () => {
  it("normalizes one notification or an array", () => {
    expect(normalizeGlobalNotificationPayload(todo)).toEqual([todo]);
    expect(normalizeGlobalNotificationPayload([todo, success])).toEqual([todo, success]);
    expect(normalizeGlobalNotificationPayload(null)).toEqual([]);
    expect(() => normalizeGlobalNotificationPayload({ kind: "unknown" } as never)).toThrow(
      "无效的全局通知",
    );
  });

  it("keeps FIFO order and ignores duplicate ids", () => {
    expect(mergeGlobalNotificationQueue([todo], [todo, success])).toEqual([todo, success]);
  });

  it("exposes actions by type and result", () => {
    expect(globalNotificationActions(todo)).toEqual(["complete", "dismiss", "snooze"]);
    expect(globalNotificationActions(success)).toEqual(["open-tool", "open-directory", "acknowledge"]);
    expect(globalNotificationActions({ ...success, status: "failed", archivePath: undefined })).toEqual([
      "open-tool",
      "acknowledge",
    ]);
    expect(globalNotificationActions({ ...success, status: "failed" })).toEqual([
      "open-tool",
      "acknowledge",
    ]);
  });

  it("uses explicit package terminal copy", () => {
    expect(releasePackageNotificationCopy("succeeded").title).toBe("上线包打包成功");
    expect(releasePackageNotificationCopy("partially_succeeded").title).toBe("上线包部分成功");
    expect(releasePackageNotificationCopy("failed").title).toBe("上线包打包失败");
  });

  it("limits long package errors without hiding short errors", () => {
    expect(summarizeNotificationError("exit 1", 12)).toBe("exit 1");
    expect(summarizeNotificationError("abcdefghijklmnop", 12)).toBe("abcdefghi...");
  });
});
```

- [ ] **Step 2: 运行测试确认 RED**

Run: `pnpm --filter @lazycat/desktop exec vitest run src/utils/globalNotification.test.ts`

Expected: FAIL，提示通知类型或纯函数模块不存在。

- [ ] **Step 3: 实现最小通知类型**

```ts
import type { TodoPriority, TodoReminderPreset } from "./todo";

export type ReleasePackageNotificationStatus = "succeeded" | "partially_succeeded" | "failed";
export type GlobalNotificationAction =
  | "complete" | "dismiss" | "snooze"
  | "open-tool" | "open-directory" | "acknowledge";

interface GlobalNotificationBase { id: string; createdAt: string }

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
}

export interface ReleasePackageNotification extends GlobalNotificationBase {
  kind: "release-package";
  runId: string;
  projectId: number;
  projectName: string;
  status: ReleasePackageNotificationStatus;
  archivePath?: string;
  error?: string;
}

export type GlobalNotification = TodoReminderNotification | ReleasePackageNotification;
```

- [ ] **Step 4: 实现最小纯函数**

```ts
export function normalizeGlobalNotificationPayload(payload: GlobalNotification | GlobalNotification[] | null | undefined) {
  if (!payload) return [];
  const values = Array.isArray(payload) ? payload : [payload];
  if (!values.every((value) => {
    if (!value || typeof value.id !== "string" || typeof value.createdAt !== "string") return false;
    if (value.kind === "todo-reminder") {
      return Number.isFinite(value.eventId) && Number.isFinite(value.taskId) && typeof value.title === "string";
    }
    return value.kind === "release-package"
      && typeof value.runId === "string"
      && Number.isFinite(value.projectId)
      && typeof value.projectName === "string"
      && ["succeeded", "partially_succeeded", "failed"].includes(value.status);
  })) {
    throw new Error("无效的全局通知");
  }
  return values;
}

export function mergeGlobalNotificationQueue(current: readonly GlobalNotification[], incoming: readonly GlobalNotification[]) {
  const next = [...current];
  const ids = new Set(next.map((item) => item.id));
  for (const item of incoming) {
    if (ids.has(item.id)) continue;
    ids.add(item.id);
    next.push(item);
  }
  return next;
}

export function globalNotificationActions(notification: GlobalNotification): GlobalNotificationAction[] {
  if (notification.kind === "todo-reminder") return ["complete", "dismiss", "snooze"];
  const actions: GlobalNotificationAction[] = ["open-tool"];
  if (notification.status !== "failed" && notification.archivePath) actions.push("open-directory");
  actions.push("acknowledge");
  return actions;
}

export function releasePackageNotificationCopy(status: ReleasePackageNotificationStatus) {
  if (status === "succeeded") return { title: "上线包打包成功", detail: "所选产物已完成归档" };
  if (status === "partially_succeeded") return { title: "上线包部分成功", detail: "可用产物已归档，请查看失败日志" };
  return { title: "上线包打包失败", detail: "未生成可用归档，请查看打包日志" };
}

export function summarizeNotificationError(error: string | undefined, maxLength = 180) {
  if (!error || error.length <= maxLength) return error ?? "";
  return `${error.slice(0, Math.max(0, maxLength - 3))}...`;
}
```

- [ ] **Step 5: 运行测试确认 GREEN**

Run: `pnpm --filter @lazycat/desktop exec vitest run src/utils/globalNotification.test.ts`

Expected: 5 tests PASS。

- [ ] **Step 6: 提交**

```powershell
git add apps/desktop/src/types/global-notification.ts apps/desktop/src/utils/globalNotification.ts apps/desktop/src/utils/globalNotification.test.ts
git commit -m "feat(notification): 添加全局通知模型"
```

### Task 2: Rust 通知模型与打包终态映射

**Files:**
- Create: `apps/desktop/src-tauri/src/global_notification.rs`
- Modify: `apps/desktop/src-tauri/src/main.rs`

- [ ] **Step 1: 声明模块并写入失败测试**

在 `main.rs` 增加 `mod global_notification;`。新文件先加入：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_notification_only_accepts_visible_overall_terminal_states() {
        for status in ["succeeded", "partially_succeeded", "failed"] {
            assert!(build_release_package_notification(
                "run-1", 7, "客户门户", "overall", status, Some("D:\\release".into()), None,
            ).is_some());
        }
        for (phase, status) in [
            ("frontend", "succeeded"),
            ("backend", "failed"),
            ("overall", "running"),
            ("overall", "cancelled"),
        ] {
            assert!(build_release_package_notification(
                "run-1", 7, "客户门户", phase, status, None, None,
            ).is_none());
        }
    }

    #[test]
    fn package_notification_keeps_project_snapshot_and_details() {
        let value = serde_json::to_value(build_release_package_notification(
            "run-9", 7, "客户门户", "overall", "partially_succeeded",
            Some("D:\\release\\target".into()), Some("frontend：exit 1".into()),
        ).unwrap()).unwrap();
        assert_eq!(value["id"], "release-package:run-9");
        assert_eq!(value["kind"], "release-package");
        assert_eq!(value["projectName"], "客户门户");
        assert_eq!(value["archivePath"], "D:\\release\\target");
    }
}
```

- [ ] **Step 2: 运行测试确认 RED**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml global_notification -- --nocapture`

Expected: FAIL，提示通知类型或构造函数不存在。

- [ ] **Step 3: 实现 Rust 判别联合和纯映射**

```rust
#[derive(Clone, Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub(crate) enum GlobalNotification {
    TodoReminder {
        id: String, created_at: String, event_id: i64, task_id: i64,
        task_reminder_id: i64, title: String, body: String, fire_at: String,
        priority: String, reminder_preset: String,
    },
    ReleasePackage {
        id: String, created_at: String, run_id: String, project_id: i64,
        project_name: String, status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        archive_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

pub(crate) fn build_release_package_notification(
    run_id: &str, project_id: i64, project_name: &str, phase: &str, status: &str,
    archive_path: Option<String>, error: Option<String>,
) -> Option<GlobalNotification> {
    if phase != "overall" || !matches!(status, "succeeded" | "partially_succeeded" | "failed") {
        return None;
    }
    Some(GlobalNotification::ReleasePackage {
        id: format!("release-package:{run_id}"),
        created_at: chrono::Local::now().to_rfc3339(),
        run_id: run_id.into(), project_id, project_name: project_name.into(), status: status.into(),
        archive_path, error,
    })
}
```

- [ ] **Step 4: 运行测试确认 GREEN**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml global_notification -- --nocapture`

Expected: 2 tests PASS。

- [ ] **Step 5: 提交**

```powershell
git add apps/desktop/src-tauri/src/global_notification.rs apps/desktop/src-tauri/src/main.rs
git commit -m "feat(notification): 定义打包终态通知"
```

### Task 3: 迁移通用通知窗口与主窗口导航

**Files:**
- Modify: `apps/desktop/src-tauri/src/global_notification.rs`
- Modify: `apps/desktop/src-tauri/src/main.rs`
- Modify: `apps/desktop/src-tauri/src/events.rs`
- Modify: `apps/desktop/src/bridge/events.ts`
- Modify: `apps/desktop/src-tauri/capabilities/default.json`
- Rename: `apps/desktop/src/ReminderPopupApp.ts` -> `apps/desktop/src/GlobalNotificationApp.ts`
- Rename: `apps/desktop/src/components/ReminderPopup.vue` -> `apps/desktop/src/components/GlobalNotificationPopup.vue`
- Modify: `apps/desktop/src/main.ts`
- Test: `apps/desktop/src/components/GlobalNotificationPopup.test.ts`

- [ ] **Step 1: 写入组件契约失败测试**

```ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./GlobalNotificationPopup.vue", import.meta.url), "utf8");
const mainSource = readFileSync(new URL("../main.ts", import.meta.url), "utf8");

describe("GlobalNotificationPopup", () => {
  it("mounts the generic view and queue", () => {
    expect(mainSource).toContain('currentView === "global-notification"');
    expect(source).toContain("__LAZYCAT_NOTIFICATION_BOOTSTRAP__");
    expect(source).toContain("APP_EVENTS.GLOBAL_NOTIFICATION_PUSH");
    expect(source).toContain("mergeGlobalNotificationQueue");
  });

  it("keeps todo actions and exposes package actions", () => {
    expect(source).toContain('invoke("reminder_popup_complete"');
    expect(source).toContain('invoke("reminder_popup_dismiss"');
    expect(source).toContain('invoke("reminder_popup_snooze"');
    expect(source).toContain('invoke("global_notification_open_tool"');
    expect(source).toContain('"tool:system:open-local-path"');
    expect(source).toContain("知道了");
    expect(source).toContain("打开打包页面");
    expect(source).toContain("打开目标目录");
  });

  it("removes one item only after a successful action", () => {
    expect(source).toContain("async function removeCurrentNotification");
    expect(source).toMatch(/try[\s\S]*await action\(\)[\s\S]*await removeCurrentNotification\(\)[\s\S]*catch/s);
  });
});
```

- [ ] **Step 2: 运行测试确认 RED**

Run: `pnpm --filter @lazycat/desktop exec vitest run src/components/GlobalNotificationPopup.test.ts`

Expected: FAIL，提示新组件不存在。

- [ ] **Step 3: 重命名入口与组件**

```powershell
git mv apps/desktop/src/ReminderPopupApp.ts apps/desktop/src/GlobalNotificationApp.ts
git mv apps/desktop/src/components/ReminderPopup.vue apps/desktop/src/components/GlobalNotificationPopup.vue
```

`GlobalNotificationApp.ts`：

```ts
import { createApp } from "vue";
import GlobalNotificationPopup from "./components/GlobalNotificationPopup.vue";
export default function mountGlobalNotificationApp() {
  createApp(GlobalNotificationPopup).mount("#app");
}
```

`main.ts` 将首个分支改为 `currentView === "global-notification"` 并动态导入 `GlobalNotificationApp`。

- [ ] **Step 4: 实现前端通用队列和动作**

在组件中用 `GlobalNotification[]` 替换 `TodoReminderDispatch[]`，核心控制流固定为：

```ts
const queue = ref<GlobalNotification[]>([]);
const currentNotification = computed(() => queue.value[0] ?? null);
const currentTodo = computed(() => currentNotification.value?.kind === "todo-reminder" ? currentNotification.value : null);
const currentPackage = computed(() => currentNotification.value?.kind === "release-package" ? currentNotification.value : null);

function mergeQueue(incoming: GlobalNotification[]) {
  queue.value = mergeGlobalNotificationQueue(queue.value, incoming);
}

async function removeCurrentNotification() {
  queue.value = queue.value.slice(1);
  if (!queue.value.length) await closePopup();
}

async function runAction(action: () => Promise<void>) {
  if (!currentNotification.value || actionPending.value) return;
  actionPending.value = true;
  try {
    await action();
    await removeCurrentNotification();
  } catch (error) {
    ElMessage.error((error as Error).message || "通知操作失败");
  } finally {
    actionPending.value = false;
  }
}

async function openReleasePackageTool() {
  await runAction(() => invoke("global_notification_open_tool", { toolId: "release-package" }));
}

async function openReleasePackageDirectory() {
  const path = currentPackage.value?.archivePath;
  if (!path) return;
  await runAction(() => invokeToolByChannel("tool:system:open-local-path", { path }).then(() => undefined));
}

async function acknowledgeCurrent() {
  await removeCurrentNotification();
}
```

任务提醒保留原命令。打包通知始终显示“打开打包页面”和“知道了”，仅状态不是 `failed` 且 `archivePath` 存在时显示“打开目标目录”；错误正文通过 `summarizeNotificationError` 限制为 180 个字符，完整错误留在打包页。右上角关闭调用 `acknowledgeCurrent`。监听改为 `APP_EVENTS.GLOBAL_NOTIFICATION_PUSH`，bootstrap 改为 `__LAZYCAT_NOTIFICATION_BOOTSTRAP__`。

- [ ] **Step 5: 实现 Rust 窗口管理与任务提醒转换**

把 `main.rs` 中 reminder popup 的常量、init script、URL、定位、复用窗口和创建窗口逻辑迁入 `global_notification.rs`，统一命名：

```rust
pub(crate) const GLOBAL_NOTIFICATION_LABEL: &str = "global-notification";
pub(crate) const GLOBAL_NOTIFICATION_TITLE: &str = "Lazycat 通知";
const GLOBAL_NOTIFICATION_WIDTH: i64 = 400;
const GLOBAL_NOTIFICATION_HEIGHT: i64 = 320;
const GLOBAL_NOTIFICATION_MARGIN: i64 = 16;
```

初始化脚本设置 `view=global-notification` 和 `window.__LAZYCAT_NOTIFICATION_BOOTSTRAP__`。已存在窗口发送 `EVENT_GLOBAL_NOTIFICATION_PUSH`。新增：

```rust
pub(crate) fn todo_notifications(reminders: Vec<ReminderDispatch>) -> Vec<GlobalNotification> {
    reminders.into_iter().map(|item| GlobalNotification::TodoReminder {
        id: format!("todo-reminder:{}", item.event_id),
        created_at: item.fire_at.clone(),
        event_id: item.event_id,
        task_id: item.task_id,
        task_reminder_id: item.task_reminder_id,
        title: item.title,
        body: item.body,
        fire_at: item.fire_at,
        priority: item.priority,
        reminder_preset: item.reminder_preset,
    }).collect()
}
```

窗口创建、聚焦或事件发送失败不返回给任务调度或打包线程。

- [ ] **Step 6: 增加通用打开工具命令**

在 `main.rs` 抽取 `navigate_main_window_to_tool(app, target)`：显示并聚焦主窗口，发送字段完整的 `HotkeyNavigatePayload`，`source` 为 `global-notification`。在通知模块注册：

```rust
#[tauri::command]
pub(crate) fn global_notification_open_tool(app: tauri::AppHandle, tool_id: String) -> Result<(), String> {
    crate::navigate_main_window_to_tool(&app, tool_id)
}
```

`spotlight_pick` 保持现有跨屏和焦点行为，避免扩大改动范围。

- [ ] **Step 7: 同步事件、任务调度和 capability**

用以下常量替换 `REMINDER_PUSH`：

```rust
pub const EVENT_GLOBAL_NOTIFICATION_PUSH: &str = "global-notification-push";
```

```ts
GLOBAL_NOTIFICATION_PUSH: "global-notification-push",
```

同步 `events::ALL`。`start_todo_scheduler` 调用：

```rust
global_notification::show_notifications(
    &app,
    global_notification::todo_notifications(reminders.clone()),
);
```

`capabilities/default.json` 将 `reminder-popup` 替换为 `global-notification`；`expected_window_title` 同步通用 label/title。

- [ ] **Step 8: 运行测试确认 GREEN**

```powershell
pnpm --filter @lazycat/desktop exec vitest run src/utils/globalNotification.test.ts src/components/GlobalNotificationPopup.test.ts
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml global_notification -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml contract_tests -- --nocapture
```

Expected: 前端 8 tests PASS；Rust 通知与契约测试 PASS。

- [ ] **Step 9: 提交**

```powershell
git add apps/desktop/src apps/desktop/src-tauri/src/global_notification.rs apps/desktop/src-tauri/src/main.rs apps/desktop/src-tauri/src/events.rs apps/desktop/src-tauri/capabilities/default.json
git commit -m "refactor(notification): 统一全局通知窗口"
```

### Task 4: 接入上线包 overall 终态

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`
- Test: `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`
- Test: `apps/desktop/src/composables/useReleasePackageRuntime.test.ts`

- [ ] **Step 1: 写入失败的旁路通知测试**

扩展测试 sink 并新增：

```rust
#[derive(Default)]
struct CollectingSink {
    statuses: Mutex<Vec<StatusEvent>>,
    notifications: Mutex<Vec<GlobalNotification>>,
}

#[test]
fn terminal_result_emits_status_and_one_package_notification() {
    let sink = CollectingSink::default();
    emit_terminal_result(&sink, "run-1", &project(), Ok(PipelineSummary {
        status: "succeeded",
        archive_path: Some(PathBuf::from("D:\\release\\target")),
        error: None,
    }));
    assert_eq!(sink.statuses.lock().unwrap().len(), 1);
    assert_eq!(sink.notifications.lock().unwrap().len(), 1);
}

#[test]
fn cancelled_result_emits_status_without_notification() {
    let sink = CollectingSink::default();
    emit_terminal_result(
        &sink,
        "run-1",
        &project(),
        Err(PipelineError::Cancelled { phase: "overall" }),
    );
    assert_eq!(sink.statuses.lock().unwrap()[0].status, "cancelled");
    assert!(sink.notifications.lock().unwrap().is_empty());
}
```

- [ ] **Step 2: 运行测试确认 RED**

Run: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml terminal_result -- --nocapture`

Expected: FAIL，提示 `notification` sink 方法或 `emit_terminal_result` 不存在。

- [ ] **Step 3: 扩展 EventSink**

```rust
trait EventSink: Send + Sync {
    fn log(&self, event: LogEvent);
    fn status(&self, event: StatusEvent);
    fn notification(&self, event: GlobalNotification);
}

impl EventSink for TauriEventSink {
    fn log(&self, event: LogEvent) { let _ = self.app.emit(EVENT_RELEASE_PACKAGE_LOG, event); }
    fn status(&self, event: StatusEvent) { let _ = self.app.emit(EVENT_RELEASE_PACKAGE_STATUS, event); }
    fn notification(&self, event: GlobalNotification) {
        crate::global_notification::show_notifications(&self.app, vec![event]);
    }
}
```

测试 `CollectingSink` 收集通知；无关测试 `Sink` 的 `notification` 为空实现。

- [ ] **Step 4: 集中终态状态和通知发送**

```rust
fn emit_terminal_result(
    sink: &dyn EventSink,
    run_id: &str,
    project: &ReleasePackageProjectConfig,
    result: Result<PipelineSummary, PipelineError>,
) {
    let (status, archive_path, error) = match result {
        Ok(summary) => (
            summary.status,
            summary.archive_path.map(|path| path.to_string_lossy().into_owned()),
            summary.error,
        ),
        Err(PipelineError::Cancelled { .. }) => ("cancelled", None, None),
        Err(PipelineError::Failed { message }) => ("failed", None, Some(message)),
    };
    emit_status(sink, run_id, project.id, status, "overall", archive_path.clone(), error.clone());
    if let Some(notification) = build_release_package_notification(
        run_id, project.id, &project.name, "overall", status, archive_path, error,
    ) {
        sink.notification(notification);
    }
}
```

线程启动前保留 `project.clone()` 作为终态快照。`claim_pipeline_result` 后只调用一次 `emit_terminal_result`，删除原有三分支重复 status 发送。

- [ ] **Step 5: 运行回归测试确认 GREEN**

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml terminal_result -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture
pnpm --filter @lazycat/desktop exec vitest run src/composables/useReleasePackageRuntime.test.ts src/components/ReleasePackagePanel.test.ts
```

Expected: 新增测试和既有上线包测试全部 PASS；status 先发送，取消无通知。

- [ ] **Step 6: 提交**

```powershell
git add apps/desktop/src-tauri/src/tools/release_package_runtime.rs
git commit -m "feat(release-package): 通知打包终态结果"
```

### Task 5: 经验记录与最终验证

**Files:**
- Modify: `process.md`
- Verify: all files above

- [ ] **Step 1: 在 process.md 记录经验**

追加：

```markdown
## 2026-07-21: 全局通知窗口统一任务提醒与长任务终态

**场景**: 在既有任务提醒独立窗口基础上增加上线包终态通知，并允许直接打开功能页和归档目录。

**解决**:
1. 将窗口生命周期和 FIFO 去重队列提升为全局通知能力，任务提醒与打包结果使用判别联合类型提供各自动作。
2. 打包运行时在真实 overall 终态落定后旁路发送通知；成功、部分成功、失败通知，主动取消不通知。
3. 打开工具页复用主窗口导航事件，打开目录复用 system 域绝对路径校验；操作失败保留通知供重试。

**关键点**:
- 长任务通知是结果旁路，窗口创建或事件发送失败不能改变真实任务终态或回滚产物。
- 通知唯一键来自稳定业务 ID；任务提醒使用 eventId，打包使用 runId，避免初始化与运行期事件重复入队。
- 类型专属动作留在展示层，窗口管理层只负责通知传输、聚焦、定位和生命周期。

**涉及文件**:
- `apps/desktop/src-tauri/src/global_notification.rs`
- `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`
- `apps/desktop/src/components/GlobalNotificationPopup.vue`
- `apps/desktop/src/utils/globalNotification.ts`

**验证**:
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture`
- `pnpm --filter @lazycat/desktop exec vitest run src/utils/globalNotification.test.ts src/components/GlobalNotificationPopup.test.ts src/composables/useReleasePackageRuntime.test.ts src/components/ReleasePackagePanel.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0
```

- [ ] **Step 2: 运行全部针对性测试**

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml global_notification -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml contract_tests -- --nocapture
pnpm --filter @lazycat/desktop exec vitest run src/utils/globalNotification.test.ts src/components/GlobalNotificationPopup.test.ts src/composables/useReleasePackageRuntime.test.ts src/components/ReleasePackagePanel.test.ts
```

Expected: 全部通过，无意外 stderr。

- [ ] **Step 3: 运行类型、构建和补丁检查**

```powershell
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml
git diff --check
```

Expected: 全部 exit code 0；Vite 只允许既有 chunk 大小警告。

- [ ] **Step 4: 检查最终范围**

Run: `git status --short`、`git diff --stat HEAD~4` 和：

```powershell
rg -n "reminder-popup|REMINDER_PUSH|ReminderPopup" apps/desktop/src apps/desktop/src-tauri/src apps/desktop/src-tauri/capabilities
```

Expected: 旧窗口名、旧推送事件和旧入口无残留；不含数据库、设置项或上线包页面布局改动；成功/部分成功有目录动作，失败无目录动作，三种结果均有“知道了”。

- [ ] **Step 5: 提交经验记录**

```powershell
git add process.md
git commit -m "docs: 记录全局通知重构经验"
```

use chrono::Local;
use serde::Serialize;
use std::sync::{
    atomic::{AtomicU8, Ordering},
    mpsc, Arc,
};
use std::time::Duration;
use tauri::{
    webview::PageLoadEvent, AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::events::EVENT_GLOBAL_NOTIFICATION_PUSH;
use crate::tools::action_center::CombinationRunDetail;
use crate::tools::follow_up::ReminderDispatch as FollowUpReminderDispatch;
use crate::tools::release_package::{ReleasePackageEnvironmentKind, ReleasePackageType};
use crate::tools::todo::{ReminderActionSummary, ReminderDispatch};

pub(crate) const GLOBAL_NOTIFICATION_LABEL: &str = "global-notification";
pub(crate) const GLOBAL_NOTIFICATION_TITLE: &str = "Lazycat 通知";
const GLOBAL_NOTIFICATION_WIDTH: i64 = 400;
const GLOBAL_NOTIFICATION_HEIGHT: i64 = 320;
const GLOBAL_NOTIFICATION_MARGIN: i64 = 16;
const GLOBAL_NOTIFICATION_VIEW_SCRIPT: &str = r#"
window.__LAZYCAT_VIEW__ = 'global-notification';
if (!window.location.search.includes('view=global-notification')) {
  const hash = window.location.hash || '';
  window.history.replaceState(window.history.state, '', `${window.location.pathname}?view=global-notification${hash}`);
}
"#;

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
#[allow(
    dead_code,
    reason = "serialized notification payload fields are consumed by the frontend"
)]
pub(crate) enum GlobalNotification {
    TodoReminder {
        id: String,
        created_at: String,
        event_id: i64,
        task_id: i64,
        task_reminder_id: i64,
        title: String,
        body: String,
        fire_at: String,
        priority: String,
        reminder_preset: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        action: Option<ReminderActionSummary>,
    },
    FollowUpReview {
        id: String,
        created_at: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        item_id: Option<i64>,
        due_count: usize,
        title: String,
        body: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        review_at: Option<String>,
    },
    ReleasePackage {
        id: String,
        created_at: String,
        run_id: String,
        environment_id: i64,
        environment: ReleasePackageEnvironmentKind,
        project_id: i64,
        project_name: String,
        package_type: ReleasePackageType,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        archive_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    ActionCombination {
        id: String,
        created_at: String,
        run_id: String,
        combination_id: i64,
        combination_name: String,
        status: String,
        failed_step_labels: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

pub(crate) fn build_action_combination_notification(
    run: &CombinationRunDetail,
) -> Option<GlobalNotification> {
    if !matches!(
        run.status.as_str(),
        "succeeded" | "partially_succeeded" | "failed"
    ) {
        return None;
    }
    let combination_id = run.combination_id?;
    let failed_steps = run
        .steps
        .iter()
        .filter(|step| step.status == "failed")
        .map(|step| format!("{} · {}", step.action_label, step.target_label))
        .collect::<Vec<_>>();
    let error = run.error.clone().or_else(|| {
        run.steps
            .iter()
            .find(|step| step.status == "failed")
            .and_then(|step| step.message.clone())
    });

    Some(GlobalNotification::ActionCombination {
        id: format!("action-combination:{}", run.id),
        created_at: Local::now().to_rfc3339(),
        run_id: run.id.clone(),
        combination_id,
        combination_name: run.combination_name.clone(),
        status: run.status.clone(),
        failed_step_labels: failed_steps,
        error,
    })
}

pub(crate) fn build_release_package_notification(
    run_id: &str,
    environment_id: i64,
    environment: ReleasePackageEnvironmentKind,
    project_id: i64,
    project_name: &str,
    package_type: ReleasePackageType,
    phase: &str,
    status: &str,
    archive_path: Option<String>,
    error: Option<String>,
) -> Option<GlobalNotification> {
    if phase != "overall"
        || !matches!(
            status,
            "succeeded"
                | "partially_succeeded"
                | "package_succeeded_upload_failed"
                | "upload_succeeded_command_failed"
                | "deployed_health_check_failed"
                | "failed"
                | "cancelled"
        )
    {
        return None;
    }

    Some(GlobalNotification::ReleasePackage {
        id: format!("release-package:{run_id}"),
        created_at: Local::now().to_rfc3339(),
        run_id: run_id.to_string(),
        environment_id,
        environment,
        project_id,
        project_name: project_name.to_string(),
        package_type,
        status: status.to_string(),
        archive_path: if package_type == ReleasePackageType::ServerUpload || status == "failed" {
            None
        } else {
            archive_path
        },
        error,
    })
}

pub(crate) fn todo_notifications(reminders: Vec<ReminderDispatch>) -> Vec<GlobalNotification> {
    let created_at = Local::now().to_rfc3339();
    reminders
        .into_iter()
        .map(|reminder| GlobalNotification::TodoReminder {
            id: format!("todo-reminder:{}", reminder.event_id),
            created_at: created_at.clone(),
            event_id: reminder.event_id,
            task_id: reminder.task_id,
            task_reminder_id: reminder.task_reminder_id,
            title: reminder.title,
            body: reminder.body,
            fire_at: reminder.fire_at,
            priority: reminder.priority,
            reminder_preset: reminder.reminder_preset,
            action: reminder.action,
        })
        .collect()
}

pub(crate) fn follow_up_notifications(
    reminders: Vec<FollowUpReminderDispatch>,
) -> Vec<GlobalNotification> {
    let created_at = Local::now().to_rfc3339();
    reminders
        .into_iter()
        .map(|reminder| {
            let identity = reminder
                .item_id
                .map(|id| format!("item:{id}:{}", reminder.review_at.as_deref().unwrap_or("")))
                .unwrap_or_else(|| format!("aggregate:{created_at}"));
            GlobalNotification::FollowUpReview {
                id: format!("follow-up-review:{identity}"),
                created_at: created_at.clone(),
                item_id: reminder.item_id,
                due_count: reminder.due_count,
                title: reminder.title,
                body: reminder.body,
                review_at: reminder.review_at,
            }
        })
        .collect()
}

fn notification_init_script(notifications: &[GlobalNotification]) -> String {
    let serialized = serde_json::to_string(notifications).unwrap_or_else(|_| "[]".to_string());
    format!(
        "{GLOBAL_NOTIFICATION_VIEW_SCRIPT}\nwindow.__LAZYCAT_NOTIFICATION_BOOTSTRAP__ = {serialized};"
    )
}

fn notification_url() -> WebviewUrl {
    if cfg!(debug_assertions) {
        WebviewUrl::External(
            "http://localhost:5173/?view=global-notification"
                .parse()
                .expect("valid global notification dev url"),
        )
    } else {
        WebviewUrl::App("index.html".into())
    }
}

fn position_notification_window(window: &WebviewWindow) {
    let Ok(Some(monitor)) = window.primary_monitor() else {
        return;
    };
    let work_area = monitor.work_area();
    let relative_x =
        (work_area.size.width as i64 - GLOBAL_NOTIFICATION_WIDTH - GLOBAL_NOTIFICATION_MARGIN)
            .max(0);
    let relative_y =
        (work_area.size.height as i64 - GLOBAL_NOTIFICATION_HEIGHT - GLOBAL_NOTIFICATION_MARGIN)
            .max(0);
    let x =
        (work_area.position.x as i64 + relative_x).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    let y =
        (work_area.position.y as i64 + relative_y).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

pub(crate) fn show_notifications(app: &AppHandle, notifications: Vec<GlobalNotification>) {
    if let Err(error) = try_show_notifications(app, notifications) {
        eprintln!("show global notifications failed: {error}");
    }
}

pub(crate) fn try_show_notifications(
    app: &AppHandle,
    notifications: Vec<GlobalNotification>,
) -> Result<(), String> {
    if notifications.is_empty() {
        return Ok(());
    }

    let app_handle = app.clone();
    let (sender, receiver) = mpsc::sync_channel(1);
    const DISPLAY_PENDING: u8 = 0;
    const DISPLAY_RUNNING: u8 = 1;
    const DISPLAY_CANCELLED: u8 = 2;
    let display_state = Arc::new(AtomicU8::new(DISPLAY_PENDING));
    let main_thread_state = Arc::clone(&display_state);
    app.run_on_main_thread(move || {
        if main_thread_state
            .compare_exchange(
                DISPLAY_PENDING,
                DISPLAY_RUNNING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            let _ = sender.send(Err(
                "notification display was cancelled before starting".into()
            ));
            return;
        }
        let result = (|| -> Result<(), String> {
            if let Some(window) = app_handle.get_webview_window(GLOBAL_NOTIFICATION_LABEL) {
                window
                    .emit(EVENT_GLOBAL_NOTIFICATION_PUSH, &notifications)
                    .map_err(|error| format!("emit notification payload failed: {error}"))?;
                position_notification_window(&window);
                window
                    .show()
                    .map_err(|error| format!("show notification window failed: {error}"))?;
                if let Err(error) = window.set_focus() {
                    eprintln!("focus notification window failed: {error}");
                }
                #[cfg(windows)]
                crate::force_foreground(&window);
                return Ok(());
            }

            let initial_notifications = notifications.clone();
            let builder = WebviewWindowBuilder::new(
                &app_handle,
                GLOBAL_NOTIFICATION_LABEL,
                notification_url(),
            )
            .title(GLOBAL_NOTIFICATION_TITLE)
            .inner_size(
                GLOBAL_NOTIFICATION_WIDTH as f64,
                GLOBAL_NOTIFICATION_HEIGHT as f64,
            )
            .decorations(false)
            .always_on_top(true)
            .resizable(false)
            .skip_taskbar(true)
            .focused(true)
            .transparent(false)
            .visible(false)
            .initialization_script(notification_init_script(&notifications))
            .on_page_load(move |window, payload| {
                if let PageLoadEvent::Finished = payload.event() {
                    if let Err(error) =
                        window.emit(EVENT_GLOBAL_NOTIFICATION_PUSH, &initial_notifications)
                    {
                        eprintln!("emit initial notification payload failed: {error}");
                    }
                }
            });

            let window = builder
                .build()
                .map_err(|error| format!("build notification window failed: {error}"))?;
            position_notification_window(&window);
            window
                .show()
                .map_err(|error| format!("show notification window failed: {error}"))?;
            if let Err(error) = window.set_focus() {
                eprintln!("focus notification window failed: {error}");
            }
            #[cfg(windows)]
            crate::force_foreground(&window);
            Ok(())
        })();
        let _ = sender.send(result);
    })
    .map_err(|error| format!("schedule notification display failed: {error}"))?;

    match receiver.recv_timeout(Duration::from_secs(5)) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            if display_state
                .compare_exchange(
                    DISPLAY_PENDING,
                    DISPLAY_CANCELLED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                return Err("wait for notification display timed out before it started".into());
            }
            receiver
                .recv()
                .map_err(|error| format!("wait for running notification display failed: {error}"))?
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("notification display result channel disconnected".into())
        }
    }
}

#[tauri::command]
pub(crate) fn global_notification_open_tool(app: AppHandle, tool_id: String) -> Result<(), String> {
    crate::navigate_main_window_to_tool(&app, &tool_id)
}

#[tauri::command]
pub(crate) fn global_notification_open_follow_up(
    app: AppHandle,
    item_id: Option<i64>,
) -> Result<(), String> {
    crate::navigate_main_window_to_tool_context(
        &app,
        "todo",
        item_id.map(|id| id.to_string()),
        Some(
            if item_id.is_some() {
                "follow-up"
            } else {
                "follow-up-due"
            }
            .to_string(),
        ),
    )
}

#[tauri::command]
pub(crate) fn global_notification_open_action_run(
    app: AppHandle,
    run_id: String,
) -> Result<(), String> {
    let run_id = run_id.trim();
    if run_id.is_empty() {
        return Err("runId 不能为空".into());
    }
    crate::navigate_main_window_to_tool_context(
        &app,
        "action-center",
        Some(run_id.to_string()),
        Some("run".into()),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        build_action_combination_notification, build_release_package_notification,
        follow_up_notifications, todo_notifications, GlobalNotification,
    };
    use crate::tools::action_center::{CombinationRunDetail, CombinationRunStep, ExecutionMode};
    use crate::tools::follow_up::ReminderDispatch as FollowUpReminderDispatch;
    use crate::tools::release_package::{ReleasePackageEnvironmentKind, ReleasePackageType};
    use crate::tools::todo::{ReminderActionSummary, ReminderDispatch};
    use serde_json::{json, Value};

    #[test]
    fn follow_up_notifications_keep_individual_and_aggregate_navigation_contracts() {
        let notifications = follow_up_notifications(vec![
            FollowUpReminderDispatch {
                item_id: Some(7),
                due_count: 1,
                title: "关注事项待复查".into(),
                body: "确认接口进度".into(),
                review_at: Some("2026-08-18T01:00:00+00:00".into()),
                dispatch_targets: vec![],
            },
            FollowUpReminderDispatch {
                item_id: None,
                due_count: 3,
                title: "关注事项待复查".into(),
                body: "有 3 项关注事项需要复查".into(),
                review_at: None,
                dispatch_targets: vec![],
            },
        ]);
        let individual = serde_json::to_value(&notifications[0]).unwrap();
        let aggregate = serde_json::to_value(&notifications[1]).unwrap();
        assert_eq!(individual["kind"], "follow-up-review");
        assert_eq!(individual["itemId"], 7);
        assert_eq!(individual["dueCount"], 1);
        assert_eq!(aggregate["dueCount"], 3);
        assert!(aggregate.get("itemId").is_none());
        assert!(aggregate.get("reviewAt").is_none());
    }

    fn release_payload(status: &str) -> Value {
        let notification = build_release_package_notification(
            "run-42",
            42,
            ReleasePackageEnvironmentKind::Production,
            7,
            "客户门户",
            ReleasePackageType::LocalArchive,
            "overall",
            status,
            Some("E:\\releases\\LazyCat.zip".to_string()),
            Some("packaging failed".to_string()),
        )
        .expect("overall terminal status should create a notification");

        serde_json::to_value(notification).expect("notification should serialize")
    }

    #[test]
    fn overall_notification_statuses_are_mapped() {
        for status in [
            "succeeded",
            "partially_succeeded",
            "package_succeeded_upload_failed",
            "upload_succeeded_command_failed",
            "deployed_health_check_failed",
            "failed",
            "cancelled",
        ] {
            assert!(build_release_package_notification(
                "run-42",
                42,
                ReleasePackageEnvironmentKind::Production,
                7,
                "LazyCat Desktop",
                ReleasePackageType::LocalArchive,
                "overall",
                status,
                None,
                None,
            )
            .is_some());
        }
    }

    #[test]
    fn non_terminal_or_subphase_results_are_ignored() {
        for (phase, status) in [
            ("frontend", "succeeded"),
            ("backend", "failed"),
            ("overall", "running"),
        ] {
            assert!(build_release_package_notification(
                "run-42",
                42,
                ReleasePackageEnvironmentKind::Production,
                7,
                "LazyCat Desktop",
                ReleasePackageType::LocalArchive,
                phase,
                status,
                None,
                None,
            )
            .is_none());
        }
    }

    #[test]
    fn release_payload_uses_frontend_contract_and_preserves_project_snapshot() {
        let payload = release_payload("partially_succeeded");

        assert_eq!(payload["id"], "release-package:run-42");
        assert_eq!(payload["kind"], "release-package");
        assert_eq!(payload["runId"], "run-42");
        assert_eq!(payload["environmentId"], 42);
        assert_eq!(payload["environment"], "production");
        assert_eq!(payload["projectId"], 7);
        assert_eq!(payload["projectName"], "客户门户");
        assert_eq!(payload["packageType"], "local_archive");
        assert_eq!(payload["status"], "partially_succeeded");
        assert_eq!(payload["archivePath"], "E:\\releases\\LazyCat.zip");
        assert_eq!(payload["error"], "packaging failed");
        assert!(payload["createdAt"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
    }

    #[test]
    fn failed_release_drops_archive_path_even_when_provided() {
        let payload = release_payload("failed");

        assert_eq!(payload["status"], "failed");
        assert_eq!(payload["error"], "packaging failed");
        assert!(!payload.as_object().unwrap().contains_key("archivePath"));
    }

    #[test]
    fn upload_release_serializes_type_without_archive_path() {
        let notification = build_release_package_notification(
            "run-upload",
            42,
            ReleasePackageEnvironmentKind::Production,
            7,
            "LazyCat Desktop",
            ReleasePackageType::ServerUpload,
            "overall",
            "succeeded",
            Some("E:\\unexpected-upload-archive".to_string()),
            None,
        )
        .unwrap();
        let payload = serde_json::to_value(notification).unwrap();

        assert_eq!(payload["packageType"], "server_upload");
        assert!(!payload.as_object().unwrap().contains_key("archivePath"));
    }

    #[test]
    fn uploaded_command_failure_notification_has_no_archive_path() {
        let notification = build_release_package_notification(
            "run-command-failed",
            42,
            ReleasePackageEnvironmentKind::Production,
            7,
            "Portal",
            ReleasePackageType::ServerUpload,
            "overall",
            "upload_succeeded_command_failed",
            None,
            Some("服务器文件已上传，但后置命令失败".into()),
        )
        .expect("command failure should create a notification");
        let payload = serde_json::to_value(notification).unwrap();

        assert_eq!(payload["status"], "upload_succeeded_command_failed");
        assert_eq!(payload["error"], "服务器文件已上传，但后置命令失败");
        assert!(!payload.as_object().unwrap().contains_key("archivePath"));
    }

    #[test]
    fn absent_optional_release_fields_are_not_serialized() {
        let notification = build_release_package_notification(
            "run-42",
            42,
            ReleasePackageEnvironmentKind::Production,
            7,
            "LazyCat Desktop",
            ReleasePackageType::LocalArchive,
            "overall",
            "succeeded",
            None,
            None,
        )
        .unwrap();
        let payload = serde_json::to_value(notification).unwrap();

        assert!(!payload.as_object().unwrap().contains_key("archivePath"));
        assert!(!payload.as_object().unwrap().contains_key("error"));
    }

    #[test]
    fn todo_reminder_payload_matches_frontend_field_names() {
        let notification = GlobalNotification::TodoReminder {
            id: "todo-reminder:12".to_string(),
            created_at: "2026-07-21T10:00:00+08:00".to_string(),
            event_id: 12,
            task_id: 11,
            task_reminder_id: 9,
            title: "Review release".to_string(),
            body: "Check the packaged application".to_string(),
            fire_at: "2026-07-21T10:05:00+08:00".to_string(),
            priority: "P1".to_string(),
            reminder_preset: "5m".to_string(),
            action: None,
        };

        assert_eq!(
            serde_json::to_value(notification).unwrap(),
            json!({
                "kind": "todo-reminder",
                "id": "todo-reminder:12",
                "createdAt": "2026-07-21T10:00:00+08:00",
                "eventId": 12,
                "taskId": 11,
                "taskReminderId": 9,
                "title": "Review release",
                "body": "Check the packaged application",
                "fireAt": "2026-07-21T10:05:00+08:00",
                "priority": "P1",
                "reminderPreset": "5m",
            })
        );
    }

    #[test]
    fn todo_action_reminder_keeps_the_read_only_action_summary() {
        let notification = todo_notifications(vec![ReminderDispatch {
            event_id: 12,
            task_id: 11,
            task_reminder_id: 9,
            title: "Review release".to_string(),
            body: String::new(),
            fire_at: "2026-07-21T10:05:00+08:00".to_string(),
            priority: "P1".to_string(),
            reminder_preset: "5m".to_string(),
            action: Some(ReminderActionSummary {
                binding_id: 3,
                action_type: "release_package.run".to_string(),
                action_label: "开始打包".to_string(),
                target_label: "客户门户".to_string(),
                available: true,
                unavailable_reason: None,
                active_dispatch_status: None,
            }),
        }])
        .pop()
        .expect("todo notification");
        let payload = serde_json::to_value(notification).unwrap();

        assert_eq!(
            payload["action"],
            json!({
                "bindingId": 3,
                "actionType": "release_package.run",
                "actionLabel": "开始打包",
                "targetLabel": "客户门户",
                "available": true,
            }),
        );
    }

    #[test]
    fn action_combination_notification_keeps_run_identity_and_failed_steps() {
        let notification = build_action_combination_notification(&CombinationRunDetail {
            id: "run-7".into(),
            combination_id: Some(7),
            combination_name: "客户门户开发环境".into(),
            execution_mode: ExecutionMode::Serial,
            status: "partially_succeeded".into(),
            result_code: None,
            error: None,
            created_at: "2026-07-30T10:00:00+08:00".into(),
            started_at: Some("2026-07-30T10:00:01+08:00".into()),
            finished_at: Some("2026-07-30T10:00:02+08:00".into()),
            steps: vec![CombinationRunStep {
                id: 11,
                action_type: "launcher.launch".into(),
                action_label: "快捷启动".into(),
                target_id: "18".into(),
                target_label: "IDE".into(),
                sort_order: 0,
                status: "failed".into(),
                result_code: None,
                message: Some("路径不存在".into()),
                started_at: Some("2026-07-30T10:00:01+08:00".into()),
                finished_at: Some("2026-07-30T10:00:02+08:00".into()),
            }],
        })
        .unwrap();
        let payload = serde_json::to_value(notification).unwrap();

        assert_eq!(payload["kind"], "action-combination");
        assert_eq!(payload["id"], "action-combination:run-7");
        assert_eq!(payload["runId"], "run-7");
        assert_eq!(payload["combinationId"], 7);
        assert_eq!(payload["combinationName"], "客户门户开发环境");
        assert_eq!(payload["status"], "partially_succeeded");
        assert_eq!(payload["failedStepLabels"], json!(["快捷启动 · IDE"]));
        assert_eq!(payload["error"], "路径不存在");
    }
}

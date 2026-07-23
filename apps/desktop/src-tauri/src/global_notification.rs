use chrono::Local;
use serde::Serialize;
use tauri::{
    webview::PageLoadEvent, AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::events::EVENT_GLOBAL_NOTIFICATION_PUSH;
use crate::tools::release_package::ReleasePackageType;
use crate::tools::todo::ReminderDispatch;

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
    },
    ReleasePackage {
        id: String,
        created_at: String,
        run_id: String,
        project_id: i64,
        project_name: String,
        package_type: ReleasePackageType,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        archive_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

pub(crate) fn build_release_package_notification(
    run_id: &str,
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
    if notifications.is_empty() {
        return;
    }

    let app_handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = app_handle.get_webview_window(GLOBAL_NOTIFICATION_LABEL) {
            position_notification_window(&window);
            let _ = window.show();
            let _ = window.set_focus();
            #[cfg(windows)]
            crate::force_foreground(&window);
            let _ = window.emit(EVENT_GLOBAL_NOTIFICATION_PUSH, &notifications);
            return;
        }

        let initial_notifications = notifications.clone();
        let builder =
            WebviewWindowBuilder::new(&app_handle, GLOBAL_NOTIFICATION_LABEL, notification_url())
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
                        let _ = window.emit(EVENT_GLOBAL_NOTIFICATION_PUSH, &initial_notifications);
                    }
                });

        let Ok(window) = builder.build() else {
            return;
        };
        position_notification_window(&window);
        let _ = window.show();
        let _ = window.set_focus();
        #[cfg(windows)]
        crate::force_foreground(&window);
    });
}

#[tauri::command]
pub(crate) fn global_notification_open_tool(app: AppHandle, tool_id: String) -> Result<(), String> {
    crate::navigate_main_window_to_tool(&app, &tool_id)
}

#[cfg(test)]
mod tests {
    use super::{build_release_package_notification, GlobalNotification};
    use crate::tools::release_package::ReleasePackageType;
    use serde_json::{json, Value};

    fn release_payload(status: &str) -> Value {
        let notification = build_release_package_notification(
            "run-42",
            7,
            "  LazyCat Desktop  ",
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
            "failed",
            "cancelled",
        ] {
            assert!(build_release_package_notification(
                "run-42",
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
        assert_eq!(payload["projectId"], 7);
        assert_eq!(payload["projectName"], "  LazyCat Desktop  ");
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
    fn absent_optional_release_fields_are_not_serialized() {
        let notification = build_release_package_notification(
            "run-42",
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
}

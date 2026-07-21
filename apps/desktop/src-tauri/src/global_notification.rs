use chrono::Local;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
#[allow(
    dead_code,
    reason = "notification producers are wired in follow-up tasks"
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
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        archive_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

#[allow(
    dead_code,
    reason = "release runtime integration is added in a follow-up task"
)]
pub(crate) fn build_release_package_notification(
    run_id: &str,
    project_id: i64,
    project_name: &str,
    phase: &str,
    status: &str,
    archive_path: Option<String>,
    error: Option<String>,
) -> Option<GlobalNotification> {
    if phase != "overall" || !matches!(status, "succeeded" | "partially_succeeded" | "failed") {
        return None;
    }

    Some(GlobalNotification::ReleasePackage {
        id: format!("release-package:{run_id}"),
        created_at: Local::now().to_rfc3339(),
        run_id: run_id.to_string(),
        project_id,
        project_name: project_name.to_string(),
        status: status.to_string(),
        archive_path: if status == "failed" {
            None
        } else {
            archive_path
        },
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::{build_release_package_notification, GlobalNotification};
    use serde_json::{json, Value};

    fn release_payload(status: &str) -> Value {
        let notification = build_release_package_notification(
            "run-42",
            7,
            "  LazyCat Desktop  ",
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
        for status in ["succeeded", "partially_succeeded", "failed"] {
            assert!(build_release_package_notification(
                "run-42",
                7,
                "LazyCat Desktop",
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
            ("overall", "cancelled"),
        ] {
            assert!(build_release_package_notification(
                "run-42",
                7,
                "LazyCat Desktop",
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
    fn absent_optional_release_fields_are_not_serialized() {
        let notification = build_release_package_notification(
            "run-42",
            7,
            "LazyCat Desktop",
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
            id: "todo-reminder:9".to_string(),
            created_at: "2026-07-21T10:00:00+08:00".to_string(),
            event_id: 12,
            task_id: 11,
            task_reminder_id: 9,
            title: "Review release".to_string(),
            body: "Check the packaged application".to_string(),
            fire_at: "2026-07-21T10:05:00+08:00".to_string(),
            priority: "high".to_string(),
            reminder_preset: "5m".to_string(),
        };

        assert_eq!(
            serde_json::to_value(notification).unwrap(),
            json!({
                "kind": "todo-reminder",
                "id": "todo-reminder:9",
                "createdAt": "2026-07-21T10:00:00+08:00",
                "eventId": 12,
                "taskId": 11,
                "taskReminderId": 9,
                "title": "Review release",
                "body": "Check the packaged application",
                "fireAt": "2026-07-21T10:05:00+08:00",
                "priority": "high",
                "reminderPreset": "5m",
            })
        );
    }
}

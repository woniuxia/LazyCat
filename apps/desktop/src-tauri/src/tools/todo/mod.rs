use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

use super::helpers::db_conn;

mod helpers;
mod items;
mod recurrence;
mod reminders;
mod taxonomy;
mod types;

use helpers::*;
use items::*;
use reminders::*;
use taxonomy::*;
use types::*;

pub use helpers::is_open_status;
pub use reminders::{compute_remind_at, reminder_configs_from_presets, sync_item_reminders};
pub use types::ReminderDispatch;


// ── Entry points ──────────────────────────────────────────

const ACTIONS: &[&str] = &[
    "type_list",
    "type_upsert",
    "type_delete",
    "assignee_list",
    "assignee_upsert",
    "assignee_delete",
    "item_list",
    "item_create",
    "item_update",
    "item_upsert",
    "item_change_status",
    "item_snooze",
    "item_toggle_pin",
    "item_toggle_active",
    "item_delete",
    "reminder_list_unread",
    "reminder_mark_read",
    "open_link",
    "pm_candidates",
    "item_set_pm_link",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported todo action: {action}"));
    }
    match action {
        "type_list" => type_list(),
        "type_upsert" => type_upsert(payload),
        "type_delete" => type_delete(payload),
        "assignee_list" => assignee_list(),
        "assignee_upsert" => assignee_upsert(payload),
        "assignee_delete" => assignee_delete(payload),
        "item_list" => item_list(payload),
        "item_create" => item_create(payload),
        "item_update" => item_update(payload),
        "item_upsert" => item_upsert(payload),
        "item_change_status" => item_change_status(payload),
        "item_snooze" => item_snooze(payload),
        "item_toggle_pin" => item_toggle_pin(payload),
        "item_toggle_active" => item_toggle_active(payload),
        "item_delete" => item_delete(payload),
        "reminder_list_unread" => reminder_list_unread(payload),
        "reminder_mark_read" => reminder_mark_read(payload),
        "open_link" => open_link(payload),
        "pm_candidates" => pm_candidates(payload),
        "item_set_pm_link" => item_set_pm_link(payload),
        _ => Err(format!("unsupported todo action: {action}")),
    }
}

pub fn scheduler_tick() -> Result<Vec<ReminderDispatch>, String> {
    let conn = db_conn()?;
    dispatch_due_reminders(&conn, Utc::now())
}


// ── PM-Todo linking (Todo side) ────────────────────────────

/// Return PM items that a Todo can be linked to (same project).
fn pm_candidates(payload: &Value) -> Result<Value, String> {
    let project_id = parse_i64(payload, "projectId").ok_or("projectId is required")?;
    let conn = db_conn()?;

    let mut stmt = conn
        .prepare(
            "SELECT i.id, i.title, i.status, i.priority, i.project_id,
                    p.name AS project_name, p.color AS project_color
             FROM pm_items i
             LEFT JOIN pm_projects p ON p.id = i.project_id
             WHERE i.project_id = ?1
             ORDER BY
                CASE i.status WHEN 'done' THEN 1 ELSE 0 END,
                CASE i.priority WHEN 'P0' THEN 0 WHEN 'P1' THEN 1 WHEN 'P2' THEN 2 ELSE 3 END,
                i.id DESC
             LIMIT 200",
        )
        .map_err(|e| format!("pm_candidates prepare: {e}"))?;

    let items: Vec<Value> = stmt
        .query_map(params![project_id], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "title": r.get::<_, String>(1)?,
                "status": r.get::<_, String>(2)?,
                "priority": r.get::<_, String>(3)?,
                "projectId": r.get::<_, i64>(4)?,
                "projectName": r.get::<_, Option<String>>(5)?,
                "projectColor": r.get::<_, Option<String>>(6)?,
            }))
        })
        .map_err(|e| format!("pm_candidates query: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(json!({ "items": items }))
}

/// Set or clear the PM link for a Todo item.
fn item_set_pm_link(payload: &Value) -> Result<Value, String> {
    let todo_item_id = parse_i64(payload, "todoItemId").ok_or("todoItemId is required")?;
    let new_pm_item_id = parse_i64(payload, "pmItemId"); // None = clear

    let conn = db_conn()?;

    // Verify todo exists and get kind + project_id
    let (kind, todo_project_id): (String, Option<i64>) = conn
        .query_row(
            "SELECT kind, project_id FROM todo_items WHERE id = ?1",
            params![todo_item_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| "事项不存在".to_string())?;

    // Only one_off allowed
    if kind != SERIES_KIND_ONE_OFF {
        return Err("重复事项暂不支持关联项目工作项".to_string());
    }

    if let Some(pm_id) = new_pm_item_id {
        // Setting or changing PM link
        // Verify PM item exists and get its project
        let pm_project_id: i64 = conn
            .query_row(
                "SELECT project_id FROM pm_items WHERE id = ?1",
                params![pm_id],
                |r| r.get(0),
            )
            .map_err(|e| format!("项目工作项不存在: {e}"))?;

        // Todo must have a project to link to PM
        let todo_pid = todo_project_id.ok_or_else(|| {
            "请先选择项目，或从项目管理工作项内绑定该任务".to_string()
        })?;

        // Same project required
        if todo_pid != pm_project_id {
            return Err("只能关联同一项目下的工作项，跨项目请先清除关联再改项目".to_string());
        }

        // Upsert: if already linked, change; otherwise insert
        let existing_link: Option<i64> = conn
            .query_row(
                "SELECT pm_item_id FROM pm_item_todo_links WHERE todo_item_id = ?1",
                params![todo_item_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("查询关联失败: {e}"))?;

        let now = chrono::Utc::now().to_rfc3339();
        if let Some(old_pm_id) = existing_link {
            if old_pm_id == pm_id {
                return Ok(json!({ "ok": true })); // already linked to same PM
            }
            // Change link
            conn.execute(
                "UPDATE pm_item_todo_links SET pm_item_id = ?1, updated_at = ?2 WHERE todo_item_id = ?3",
                params![pm_id, now, todo_item_id],
            )
            .map_err(|e| format!("改挂关联失败: {e}"))?;
        } else {
            // Insert new link
            conn.execute(
                "INSERT INTO pm_item_todo_links (pm_item_id, todo_item_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
                params![pm_id, todo_item_id, now, now],
            )
            .map_err(|e| format!("设置关联失败: {e}"))?;
        }
    } else {
        // Clear PM link (pmItemId = null or not provided)
        conn.execute(
            "DELETE FROM pm_item_todo_links WHERE todo_item_id = ?1",
            params![todo_item_id],
        )
        .map_err(|e| format!("清除关联失败: {e}"))?;
    }

    Ok(json!({ "ok": true }))
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::recurrence::*;
    use chrono::{DateTime, Timelike};
    use rusqlite::Connection;

    fn create_test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "
            CREATE TABLE todo_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                type_id INTEGER DEFAULT NULL,
                priority TEXT NOT NULL DEFAULT 'P2',
                description TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'pending',
                event_at TEXT DEFAULT NULL,
                pinned INTEGER NOT NULL DEFAULT 0,
                kind TEXT NOT NULL DEFAULT 'one_off',
                parent_id INTEGER DEFAULT NULL,
                series_id INTEGER DEFAULT NULL,
                remind_at TEXT DEFAULT NULL,
                snooze_until TEXT DEFAULT NULL,
                last_notified_at TEXT DEFAULT NULL,
                completed_at TEXT DEFAULT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE todo_series_rules (
                series_id INTEGER PRIMARY KEY,
                rule_mode TEXT NOT NULL DEFAULT 'simple',
                rule_json TEXT,
                cron_expression TEXT,
                timezone TEXT DEFAULT 'local',
                start_at TEXT DEFAULT NULL,
                end_mode TEXT NOT NULL DEFAULT 'never',
                end_value TEXT DEFAULT NULL,
                occurrence_index INTEGER NOT NULL DEFAULT 1,
                active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE todo_item_assignees (
                item_id INTEGER NOT NULL,
                assignee_id INTEGER NOT NULL,
                UNIQUE(item_id, assignee_id)
            );
            CREATE TABLE todo_item_reminders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                item_id INTEGER NOT NULL,
                reminder_preset TEXT NOT NULL,
                offset_minutes INTEGER NOT NULL,
                remind_at TEXT NOT NULL,
                snooze_until TEXT DEFAULT NULL,
                last_notified_at TEXT DEFAULT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE todo_item_links (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                item_id INTEGER NOT NULL,
                url TEXT NOT NULL,
                title TEXT NOT NULL DEFAULT '',
                sort_order INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE todo_reminder_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                task_reminder_id INTEGER DEFAULT NULL,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                fire_at TEXT NOT NULL,
                is_read INTEGER NOT NULL DEFAULT 0,
                reminder_preset TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            ",
        )
        .expect("create todo schema");
        conn
    }

    fn seed_series_rule(
        conn: &Connection,
        series_id: i64,
        occurrence_index: i64,
        end_mode: &str,
        end_value: Option<&str>,
    ) {
        let start = (Utc::now() - chrono::Duration::days(1)).format("%Y-%m-%dT09:00:00+00:00").to_string();
        conn.execute(
            "INSERT INTO todo_series_rules
             (series_id, rule_mode, rule_json, cron_expression, timezone, start_at, end_mode, end_value, occurrence_index, active)
             VALUES(?1, 'simple', '{}', ?2, 'UTC', ?3, ?4, ?5, ?6, 1)",
            params![
                series_id,
                "0 0 9 * * *",
                start,
                end_mode,
                end_value,
                occurrence_index,
            ],
        )
        .expect("seed series rule");
    }

    fn seed_recurring_item(
        conn: &Connection,
        item_id: i64,
        status: &str,
        event_at: &str,
        series_id: i64,
    ) {
        conn.execute(
            "INSERT INTO todo_items(id, title, priority, description, status, event_at, kind, series_id, completed_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL)",
            params![
                item_id,
                format!("实例 {item_id}"),
                "P1",
                "已生成实例",
                status,
                event_at,
                SERIES_KIND_RECURRING,
                series_id,
            ],
        )
        .expect("seed recurring item");
    }

    #[test]
    fn simple_daily_cron_should_build() {
        let expr = build_simple_cron_expression(&json!({
            "frequency": "daily",
            "interval": 1,
            "time": "09:30"
        }))
        .expect("daily");
        assert_eq!(expr, "0 30 9 * * *");
    }

    #[test]
    fn simple_weekly_cron_should_build_using_named_weekdays() {
        let expr = build_simple_cron_expression(&json!({
            "frequency": "weekly",
            "interval": 1,
            "time": "09:30",
            "weekdays": [1, 2, 3, 4, 5]
        }))
        .expect("weekly");
        assert_eq!(expr, "0 30 9 * * Mon-Fri");

        let expr = build_simple_cron_expression(&json!({
            "frequency": "weekly",
            "interval": 1,
            "time": "09:30",
            "weekdays": [7]
        }))
        .expect("weekly");
        assert_eq!(expr, "0 30 9 * * Sun");
    }

    #[test]
    fn workday_next_occurrence_should_be_friday_after_thursday() {
        let expr = build_simple_cron_expression(&json!({
            "frequency": "weekly",
            "interval": 1,
            "time": "09:00",
            "weekdays": [1, 2, 3, 4, 5]
        }))
        .expect("weekly");

        let after = DateTime::parse_from_rfc3339("2026-03-12T10:00:00+00:00")
            .expect("after")
            .with_timezone(&Utc);
        let next = compute_next_occurrence_with_start(
            &expr,
            "UTC",
            Some("2026-03-10T09:00:00+00:00"),
            after,
        )
        .expect("next occurrence")
        .expect("occurrence exists");

        assert_eq!(next.to_rfc3339(), "2026-03-13T09:00:00+00:00");
    }

    #[test]
    fn simple_time_should_reject_non_five_minute_step() {
        let error = build_simple_cron_expression(&json!({
            "frequency": "daily",
            "interval": 1,
            "time": "09:07"
        }))
        .expect_err("should reject");
        assert!(error.contains("5 分钟"));
    }

    #[test]
    fn simple_monthly_rule_should_keep_day_31() {
        let expr = build_simple_cron_expression(&json!({
            "frequency": "monthly",
            "interval": 1,
            "time": "09:30",
            "dayOfMonth": 31,
        }))
        .expect("monthly rule");
        assert_eq!(expr, "0 30 9 31 * *");
    }

    #[test]
    fn cron_expression_should_reject_non_five_minute_schedule() {
        let error = resolve_cron_expression(
            "cron",
            &json!({
                "expression": "3 9 * * *"
            }),
        )
        .expect_err("should reject");
        assert!(error.contains("5 分钟"));
    }

    #[test]
    fn reminder_requires_event_time() {
        assert!(compute_remind_at(None, Some(5)).is_err());
    }

    #[test]
    fn reminder_presets_should_normalize_multi_select() {
        let presets = parse_reminder_presets(&json!({
            "reminderPresets": ["none", "1d", "0m", "1d", "5m"]
        }))
        .expect("parse")
        .expect("has value");
        assert_eq!(
            presets,
            vec![
                REMINDER_PRESET_ON_TIME.to_string(),
                REMINDER_PRESET_5M.to_string(),
                REMINDER_PRESET_1D.to_string(),
            ]
        );
    }

    #[test]
    fn dispatch_due_reminders_should_include_priority_in_payload() {
        let conn = create_test_conn();
        conn.execute(
            "INSERT INTO todo_items(id, title, priority, description, status, event_at, kind, completed_at)
             VALUES(1, ?1, ?2, ?3, ?4, ?5, ?6, NULL)",
            params![
                "提醒事项",
                "P0",
                "",
                STATUS_PENDING,
                "2026-03-08T09:00:00+00:00",
                SERIES_KIND_ONE_OFF,
            ],
        )
        .expect("seed item");
        conn.execute(
            "INSERT INTO todo_item_reminders(id, item_id, reminder_preset, offset_minutes, remind_at)
             VALUES(11, 1, ?1, 0, ?2)",
            params![REMINDER_PRESET_ON_TIME, "2026-03-08T09:00:00+00:00"],
        )
        .expect("seed reminder");

        let reminders = dispatch_due_reminders(
            &conn,
            DateTime::parse_from_rfc3339("2026-03-08T09:00:00+00:00")
                .expect("parse now")
                .with_timezone(&Utc),
        )
        .expect("dispatch reminders");

        assert_eq!(reminders.len(), 1);
        assert_eq!(reminders[0].priority, "P0");
        assert_eq!(reminders[0].body, "");
    }

    #[test]
    fn mark_item_reminder_events_read_should_only_touch_target_item() {
        let conn = create_test_conn();
        conn.execute(
            "INSERT INTO todo_items(id, title, priority, status, kind)
             VALUES(1, '任务A', 'P2', 'pending', 'one_off'),
                   (2, '任务B', 'P2', 'pending', 'one_off')",
            [],
        )
        .expect("seed items");
        conn.execute(
            "INSERT INTO todo_reminder_events(task_id, title, body, fire_at, is_read)
             VALUES(1, '任务A', '', '2026-03-08T09:00:00+00:00', 0),
                   (1, '任务A', '', '2026-03-08T09:30:00+00:00', 1),
                   (2, '任务B', '', '2026-03-08T09:00:00+00:00', 0)",
            [],
        )
        .expect("seed events");

        mark_item_reminder_events_read(&conn, 1).expect("mark read");

        let unread_item1: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM todo_reminder_events WHERE task_id=1 AND is_read=0",
                [],
                |row| row.get(0),
            )
            .expect("count item1 unread");
        let unread_item2: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM todo_reminder_events WHERE task_id=2 AND is_read=0",
                [],
                |row| row.get(0),
            )
            .expect("count item2 unread");
        assert_eq!(unread_item1, 0);
        assert_eq!(unread_item2, 1);
    }

    #[test]
    fn status_transition_should_validate() {
        assert!(can_transit(STATUS_PENDING, STATUS_IN_PROGRESS));
        assert!(can_transit(STATUS_IN_PROGRESS, STATUS_COMPLETED));
        assert!(!can_transit(STATUS_COMPLETED, STATUS_PENDING));
    }

    #[test]
    fn status_transition_for_kind_should_block_recurring_done_to_pending() {
        assert!(!can_transit_for_kind(
            STATUS_COMPLETED,
            STATUS_PENDING,
            SERIES_KIND_RECURRING
        ));
        assert!(can_transit_for_kind(
            STATUS_COMPLETED,
            STATUS_PENDING,
            SERIES_KIND_ONE_OFF
        ));
    }

    #[test]
    fn sort_item_rows_should_prioritize_pinned_items() {
        let mut items = vec![
            json!({
                "id": 1,
                "pinned": false,
                "priority": "P0",
                "displayAt": "2026-03-08T08:00:00.000Z"
            }),
            json!({
                "id": 2,
                "pinned": true,
                "priority": "P3",
                "displayAt": "2026-03-08T12:00:00.000Z"
            }),
        ];

        sort_item_rows(&mut items);

        assert_eq!(items[0].get("id").and_then(Value::as_i64), Some(2));
    }

    #[test]
    fn item_sort_time_should_use_display_at_only() {
        let item = json!({
            "id": 1,
            "displayAt": Value::Null,
            "updatedAt": "2026-03-08T10:00:00.000Z"
        });

        assert_eq!(item_sort_time(&item), "");
    }

    #[test]
    fn parse_item_kind_should_support_payload_shapes() {
        assert_eq!(
            parse_item_kind(&json!({ "kind": "recurring" })),
            SERIES_KIND_RECURRING
        );
        assert_eq!(
            parse_item_kind(&json!({
                "recurrence": {
                    "ruleMode": "simple",
                    "rule": { "frequency": "daily", "interval": 1, "time": "09:00" },
                    "timezone": "local",
                    "endMode": "never",
                    "endValue": null
                }
            })),
            SERIES_KIND_RECURRING
        );
        assert_eq!(
            parse_item_kind(&json!({ "kind": "one_off" })),
            SERIES_KIND_ONE_OFF
        );
    }

    #[test]
    fn parse_end_rule_should_support_nested_recurrence_payload() {
        let (mode, end_value) = parse_end_rule(&json!({
            "recurrence": {
                "endMode": "after_count",
                "endValue": 5
            }
        }))
        .expect("nested recurrence end rule");
        assert_eq!(mode, "after_count");
        assert_eq!(end_value.as_deref(), Some("5"));
    }

    #[test]
    fn next_occurrence_should_respect_start_time_boundary() {
        let start_at = "2026-03-10T09:30:00+00:00";
        let next = compute_next_occurrence_with_start(
            "0 30 9 * * *",
            "UTC",
            Some(start_at),
            DateTime::parse_from_rfc3339("2026-03-07T00:00:00+00:00")
                .expect("after")
                .with_timezone(&Utc),
        )
        .expect("next occurrence")
        .expect("occurrence exists");

        assert_eq!(next.to_rfc3339(), start_at);
    }

    #[test]
    fn completing_recurring_item_should_generate_next_when_no_other_open() {
        let conn = create_test_conn();
        // Use a date far in the past so base_time falls back to now
        seed_series_rule(&conn, 7, 1, "never", None);
        seed_recurring_item(&conn, 1, STATUS_PENDING, "2020-01-01T09:00:00+00:00", 7);

        // Mark as completed
        conn.execute(
            "UPDATE todo_items SET status=?1 WHERE id=1",
            params![STATUS_COMPLETED],
        )
        .expect("mark completed");

        let next_id = generate_next_item(&conn, 7, 1, true)
            .expect("generate next")
            .expect("should generate");

        // Verify new item
        let (status, event_at, series_id): (String, String, i64) = conn
            .query_row(
                "SELECT status, event_at, series_id FROM todo_items WHERE id=?1",
                params![next_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("load new item");
        assert_eq!(status, STATUS_PENDING);
        assert_eq!(series_id, 7);

        // The next occurrence should be today or tomorrow at 09:00 UTC
        let next_dt = event_at.parse::<chrono::DateTime<Utc>>().expect("parse event_at");
        let now = Utc::now();
        let today_09 = now.date_naive().and_hms_opt(9, 0, 0).unwrap();
        let tomorrow_09 = (now.date_naive() + chrono::Duration::days(1)).and_hms_opt(9, 0, 0).unwrap();
        let next_naive = next_dt.date_naive().and_hms_opt(next_dt.hour(), next_dt.minute(), 0).unwrap();
        assert!(next_naive == today_09 || next_naive == tomorrow_09, "expected 09:00 today or tomorrow, got {event_at}");

        // Verify occurrence_index incremented
        let idx: i64 = conn
            .query_row(
                "SELECT occurrence_index FROM todo_series_rules WHERE series_id=7",
                [],
                |row| row.get(0),
            )
            .expect("load occurrence_index");
        assert_eq!(idx, 2);
    }

    #[test]
    fn completing_recurring_item_should_not_generate_when_other_open_exists() {
        let conn = create_test_conn();
        seed_series_rule(&conn, 7, 2, "never", None);
        seed_recurring_item(&conn, 1, STATUS_COMPLETED, "2026-03-07T09:00:00+00:00", 7);
        seed_recurring_item(&conn, 2, STATUS_PENDING, "2026-03-08T09:00:00+00:00", 7);

        let result = generate_next_item(&conn, 7, 1, true).expect("no generation");
        assert!(result.is_none());
    }

    #[test]
    fn completing_recurring_item_should_stop_when_end_limit_reached() {
        let conn = create_test_conn();
        seed_series_rule(&conn, 7, 1, "after_count", Some("1"));
        seed_recurring_item(&conn, 1, STATUS_COMPLETED, "2026-03-07T09:00:00+00:00", 7);

        let result = generate_next_item(&conn, 7, 1, true).expect("respect end limit");
        assert!(result.is_none());
    }

    #[test]
    fn should_stop_series_respects_until_date() {
        let rule = SeriesRuleRow {
            series_id: 1,
            rule_mode: "simple".to_string(),
            rule_json: "{}".to_string(),
            cron_expression: "0 0 9 * * *".to_string(),
            timezone: "UTC".to_string(),
            start_at: None,
            end_mode: "until_date".to_string(),
            end_value: Some("2026-03-07T09:00:00+00:00".to_string()),
            occurrence_index: 1,
            active: true,
        };
        let after = DateTime::parse_from_rfc3339("2026-03-08T09:00:00+00:00")
            .expect("parse")
            .with_timezone(&Utc);
        assert!(should_stop_series(&rule, after));
    }

    #[test]
    fn a1_normalization_should_map_in_progress_to_pending() {
        assert_eq!(normalize_status_a1("in_progress"), "pending");
        assert_eq!(normalize_status_a1("pending"), "pending");
        assert_eq!(normalize_status_a1("completed"), "completed");
    }
}

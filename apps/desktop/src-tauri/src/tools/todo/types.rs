use serde::Serialize;

pub(crate) const PRIORITIES: [&str; 4] = ["P0", "P1", "P2", "P3"];
pub(crate) const STATUS_PENDING: &str = "pending";
pub(crate) const STATUS_IN_PROGRESS: &str = "in_progress";
pub(crate) const STATUS_COMPLETED: &str = "completed";
pub(crate) const SERIES_KIND_ONE_OFF: &str = "one_off";
pub(crate) const SERIES_KIND_RECURRING: &str = "recurring";
pub(crate) const SCOPE_THIS_INSTANCE: &str = "this_instance";
pub(crate) const SCOPE_FUTURE_INSTANCES: &str = "future_instances";
pub(crate) const REMINDER_PRESET_ON_TIME: &str = "0m";
pub(crate) const REMINDER_PRESET_NONE: &str = "none";
pub(crate) const REMINDER_PRESET_5M: &str = "5m";
pub(crate) const REMINDER_PRESET_10M: &str = "10m";
pub(crate) const REMINDER_PRESET_30M: &str = "30m";
pub(crate) const REMINDER_PRESET_1H: &str = "1h";
pub(crate) const REMINDER_PRESET_1D: &str = "1d";
pub(crate) const REMINDER_PRESET_2D: &str = "2d";
pub(crate) const EVENT_TIME_MINUTE_STEP: u32 = 5;
pub(crate) const REMINDER_PRESET_OFFSETS: [(&str, i64); 7] = [
    (REMINDER_PRESET_ON_TIME, 0),
    (REMINDER_PRESET_5M, 5),
    (REMINDER_PRESET_10M, 10),
    (REMINDER_PRESET_30M, 30),
    (REMINDER_PRESET_1H, 60),
    (REMINDER_PRESET_1D, 24 * 60),
    (REMINDER_PRESET_2D, 2 * 24 * 60),
];

// ── Structs ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderActionSummary {
    pub binding_id: i64,
    pub action_type: String,
    pub action_label: String,
    pub target_label: String,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_dispatch_status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderDispatch {
    pub event_id: i64,
    pub task_id: i64,
    pub task_reminder_id: i64,
    pub title: String,
    pub body: String,
    pub fire_at: String,
    pub priority: String,
    pub reminder_preset: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<ReminderActionSummary>,
}

pub(crate) struct SeriesRuleRow {
    #[allow(dead_code)]
    pub(crate) series_id: i64,
    pub(crate) rule_mode: String,
    pub(crate) rule_json: String,
    pub(crate) cron_expression: String,
    pub(crate) timezone: String,
    pub(crate) start_at: Option<String>,
    pub(crate) end_mode: String,
    pub(crate) end_value: Option<String>,
    pub(crate) occurrence_index: i64,
    pub(crate) active: bool,
}

pub struct ReminderConfig {
    pub preset: String,
    pub offset_minutes: i64,
}

#[derive(Default)]
pub(crate) struct TaskReminderSummary {
    pub(crate) reminder_presets: Vec<String>,
    pub(crate) snooze_until: Option<String>,
    pub(crate) last_notified_at: Option<String>,
    pub(crate) next_task_reminder_id: Option<i64>,
    pub(crate) next_reminder_preset: Option<String>,
}

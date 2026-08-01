use chrono::{DateTime, Datelike, Local, NaiveTime};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::helpers::db_conn;

const CONFIG_KEY: &str = "pomodoro.config";
const SESSION_KEY: &str = "pomodoro.session";
const STATUS_PROMPTED: &str = "prompted";
const STATUS_RUNNING: &str = "running";
const STATUS_SKIPPED: &str = "skipped";
const STATUS_STOPPED: &str = "stopped";
const STATUS_COMPLETED: &str = "completed";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroConfig {
    pub enabled: bool,
    pub workday_start: String,
    pub workday_end: String,
    pub lunch_start: String,
    pub lunch_end: String,
    pub focus_minutes: u32,
    pub short_break_minutes: u32,
    pub weekdays: Vec<u32>,
}

impl Default for PomodoroConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            workday_start: "08:00".to_string(),
            workday_end: "17:00".to_string(),
            lunch_start: "12:00".to_string(),
            lunch_end: "13:30".to_string(),
            focus_minutes: 25,
            short_break_minutes: 5,
            weekdays: vec![1, 2, 3, 4, 5],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroSession {
    pub date: String,
    pub status: String,
    pub started_at: Option<String>,
    pub stopped_at: Option<String>,
    pub prompted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroPrompt {
    pub date: String,
    pub prompted_at: String,
}

const ACTIONS: &[&str] = &[
    "get_state",
    "set_enabled",
    "start_today",
    "skip_today",
    "stop_today",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported pomodoro action: {action}"));
    }
    match action {
        "get_state" => get_state(),
        "set_enabled" => set_enabled(payload),
        "start_today" => start_today(),
        "skip_today" => skip_today(),
        "stop_today" => stop_today(),
        _ => Err(format!("unsupported pomodoro action: {action}")),
    }
}

pub fn scheduler_tick(now: DateTime<Local>) -> Result<Option<PomodoroPrompt>, String> {
    let config = load_config()?;
    let session = load_session()?;
    if !should_prompt_today(&config, session.as_ref(), now) {
        maybe_complete_running_session(&config, session.as_ref(), now)?;
        return Ok(None);
    }

    let prompt = PomodoroPrompt {
        date: local_date_key(now),
        prompted_at: now.to_rfc3339(),
    };
    save_session(&PomodoroSession {
        date: prompt.date.clone(),
        status: STATUS_PROMPTED.to_string(),
        started_at: None,
        stopped_at: None,
        prompted_at: Some(prompt.prompted_at.clone()),
    })?;
    Ok(Some(prompt))
}

fn get_state() -> Result<Value, String> {
    let config = load_config()?;
    let session = normalized_today_session(load_session()?, Local::now());
    Ok(json!({
        "config": config,
        "session": session,
        "now": Local::now().to_rfc3339(),
    }))
}

fn set_enabled(payload: &Value) -> Result<Value, String> {
    let enabled = payload
        .get("enabled")
        .and_then(Value::as_bool)
        .ok_or("enabled is required")?;
    let mut config = load_config()?;
    config.enabled = enabled;
    save_config(&config)?;
    get_state()
}

pub fn start_today() -> Result<Value, String> {
    let now = Local::now();
    let session = PomodoroSession {
        date: local_date_key(now),
        status: STATUS_RUNNING.to_string(),
        started_at: Some(now.to_rfc3339()),
        stopped_at: None,
        prompted_at: load_session()?.and_then(|item| item.prompted_at),
    };
    save_session(&session)?;
    get_state()
}

pub fn skip_today() -> Result<Value, String> {
    save_terminal_session(STATUS_SKIPPED)?;
    get_state()
}

fn stop_today() -> Result<Value, String> {
    save_terminal_session(STATUS_STOPPED)?;
    get_state()
}

fn save_terminal_session(status: &str) -> Result<(), String> {
    let now = Local::now();
    let previous = load_session()?;
    save_session(&PomodoroSession {
        date: local_date_key(now),
        status: status.to_string(),
        started_at: previous.as_ref().and_then(|item| item.started_at.clone()),
        stopped_at: Some(now.to_rfc3339()),
        prompted_at: previous.and_then(|item| item.prompted_at),
    })
}

fn maybe_complete_running_session(
    config: &PomodoroConfig,
    session: Option<&PomodoroSession>,
    now: DateTime<Local>,
) -> Result<(), String> {
    let Some(session) = session else {
        return Ok(());
    };
    if session.date != local_date_key(now) || session.status != STATUS_RUNNING {
        return Ok(());
    }
    if let Some(end) = parse_time(&config.workday_end) {
        if now.time() >= end {
            save_terminal_session(STATUS_COMPLETED)?;
        }
    }
    Ok(())
}

fn normalized_today_session(
    session: Option<PomodoroSession>,
    now: DateTime<Local>,
) -> Option<PomodoroSession> {
    session.filter(|item| item.date == local_date_key(now))
}

fn should_prompt_today(
    config: &PomodoroConfig,
    session: Option<&PomodoroSession>,
    now: DateTime<Local>,
) -> bool {
    if !config.enabled || !is_configured_workday(config, now) {
        return false;
    }

    let Some(start) = parse_time(&config.workday_start) else {
        return false;
    };
    let Some(end) = parse_time(&config.workday_end) else {
        return false;
    };
    if now.time() < start || now.time() >= end {
        return false;
    }

    let today = local_date_key(now);
    if let Some(session) = session {
        if session.date == today {
            return false;
        }
    }

    true
}

fn is_configured_workday(config: &PomodoroConfig, now: DateTime<Local>) -> bool {
    config
        .weekdays
        .contains(&now.weekday().number_from_monday())
}

fn parse_time(value: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(value, "%H:%M").ok()
}

fn local_date_key(now: DateTime<Local>) -> String {
    now.format("%Y-%m-%d").to_string()
}

fn load_config() -> Result<PomodoroConfig, String> {
    let Some(raw) = load_setting(CONFIG_KEY)? else {
        return Ok(PomodoroConfig::default());
    };
    serde_json::from_str(&raw).or_else(|_| Ok(PomodoroConfig::default()))
}

fn save_config(config: &PomodoroConfig) -> Result<(), String> {
    let raw = serde_json::to_string(config)
        .map_err(|e| format!("serialize pomodoro config failed: {e}"))?;
    save_setting(CONFIG_KEY, &raw)
}

fn load_session() -> Result<Option<PomodoroSession>, String> {
    let Some(raw) = load_setting(SESSION_KEY)? else {
        return Ok(None);
    };
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|e| format!("parse pomodoro session failed: {e}"))
}

fn save_session(session: &PomodoroSession) -> Result<(), String> {
    let raw = serde_json::to_string(session)
        .map_err(|e| format!("serialize pomodoro session failed: {e}"))?;
    save_setting(SESSION_KEY, &raw)
}

fn load_setting(key: &str) -> Result<Option<String>, String> {
    let conn = db_conn()?;
    conn.query_row(
        "SELECT value FROM user_settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(|e| format!("load setting failed: {e}"))
}

fn save_setting(key: &str, value: &str) -> Result<(), String> {
    let conn = db_conn()?;
    conn.execute(
        "INSERT INTO user_settings(key, value, updated_at) VALUES(?1, ?2, CURRENT_TIMESTAMP)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=CURRENT_TIMESTAMP",
        params![key, value],
    )
    .map_err(|e| format!("save setting failed: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, TimeZone};

    #[test]
    fn should_prompt_on_workday_after_start_when_today_has_not_been_handled() {
        let config = PomodoroConfig::default();
        let now = Local.with_ymd_and_hms(2026, 6, 29, 8, 0, 0).unwrap();

        assert!(should_prompt_today(&config, None, now));
    }

    #[test]
    fn should_not_prompt_again_after_today_was_prompted_or_skipped() {
        let config = PomodoroConfig::default();
        let now = Local.with_ymd_and_hms(2026, 6, 29, 8, 30, 0).unwrap();

        assert!(!should_prompt_today(
            &config,
            Some(&PomodoroSession {
                date: "2026-06-29".to_string(),
                status: "prompted".to_string(),
                started_at: None,
                stopped_at: None,
                prompted_at: Some("2026-06-29T08:00:00+08:00".to_string()),
            }),
            now,
        ));

        assert!(!should_prompt_today(
            &config,
            Some(&PomodoroSession {
                date: "2026-06-29".to_string(),
                status: "skipped".to_string(),
                started_at: None,
                stopped_at: None,
                prompted_at: Some("2026-06-29T08:00:00+08:00".to_string()),
            }),
            now,
        ));
    }
}

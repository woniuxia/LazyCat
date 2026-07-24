use rusqlite::{Connection, OptionalExtension};

use super::widget::guards::SystemInputSnapshot;

const ACTIVITY_ENABLED_KEY: &str = "vault_activity_lock_enabled";
const ACTIVITY_MINUTES_KEY: &str = "vault_activity_lock_minutes";
const SYSTEM_IDLE_ENABLED_KEY: &str = "vault_system_idle_lock_enabled";
const SYSTEM_IDLE_MINUTES_KEY: &str = "vault_system_idle_lock_minutes";
const LEGACY_PROFILE_KEY: &str = "vault_lock_profile";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VaultLockConfig {
    pub activity_enabled: bool,
    pub activity_after_secs: u64,
    pub system_idle_enabled: bool,
    pub system_idle_after_secs: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LockReason {
    VaultActivity,
    SystemIdle,
}

fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT value FROM user_settings WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| format!("load Vault lock setting '{key}' failed: {error}"))
}

fn parse_boolean(raw: Option<&str>, fallback: bool) -> bool {
    match raw {
        Some("true") => true,
        Some("false") => false,
        _ => fallback,
    }
}

fn parse_minutes(raw: Option<&str>, fallback: u64) -> u64 {
    match raw.and_then(|value| value.parse::<u64>().ok()) {
        Some(value) if matches!(value, 5 | 10 | 15 | 30 | 60) => value,
        _ => fallback,
    }
}

fn legacy_activity_minutes(profile: Option<&str>) -> u64 {
    match profile {
        Some("strict") => 10,
        Some("convenient") => 60,
        _ => 30,
    }
}

pub(crate) fn load_config(conn: &Connection) -> Result<VaultLockConfig, String> {
    let legacy_profile = get_setting(conn, LEGACY_PROFILE_KEY)?;
    let legacy_minutes = legacy_activity_minutes(legacy_profile.as_deref());
    let activity_enabled = get_setting(conn, ACTIVITY_ENABLED_KEY)?;
    let activity_minutes = get_setting(conn, ACTIVITY_MINUTES_KEY)?;
    let system_idle_enabled = get_setting(conn, SYSTEM_IDLE_ENABLED_KEY)?;
    let system_idle_minutes = get_setting(conn, SYSTEM_IDLE_MINUTES_KEY)?;

    Ok(VaultLockConfig {
        activity_enabled: parse_boolean(activity_enabled.as_deref(), true),
        activity_after_secs: parse_minutes(activity_minutes.as_deref(), legacy_minutes) * 60,
        system_idle_enabled: parse_boolean(system_idle_enabled.as_deref(), true),
        system_idle_after_secs: parse_minutes(system_idle_minutes.as_deref(), 15) * 60,
    })
}

pub(crate) fn expired_reason(
    config: VaultLockConfig,
    vault_idle_secs: u64,
    current: Option<SystemInputSnapshot>,
    previous: Option<SystemInputSnapshot>,
) -> Option<LockReason> {
    if config.activity_enabled && vault_idle_secs >= config.activity_after_secs {
        return Some(LockReason::VaultActivity);
    }
    if !config.system_idle_enabled {
        return None;
    }
    let current = current?;
    if current.idle_secs >= config.system_idle_after_secs {
        return Some(LockReason::SystemIdle);
    }
    if let Some(previous) = previous {
        let between_inputs_ms = current
            .last_input_tick_ms
            .wrapping_sub(previous.last_input_tick_ms) as u64;
        if current.last_input_tick_ms != previous.last_input_tick_ms
            && between_inputs_ms >= config.system_idle_after_secs * 1_000
        {
            return Some(LockReason::SystemIdle);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> VaultLockConfig {
        VaultLockConfig {
            activity_enabled: true,
            activity_after_secs: 1_800,
            system_idle_enabled: true,
            system_idle_after_secs: 900,
        }
    }

    #[test]
    fn either_rule_expires_at_the_boundary() {
        let input = SystemInputSnapshot {
            last_input_tick_ms: 10,
            idle_secs: 900,
        };
        assert_eq!(
            expired_reason(config(), 10, Some(input), None),
            Some(LockReason::SystemIdle)
        );
        assert_eq!(
            expired_reason(config(), 1_800, None, None),
            Some(LockReason::VaultActivity)
        );
    }

    #[test]
    fn detects_threshold_crossed_before_input_reset() {
        let previous = SystemInputSnapshot {
            last_input_tick_ms: 1_000,
            idle_secs: 870,
        };
        let current = SystemInputSnapshot {
            last_input_tick_ms: 901_000,
            idle_secs: 1,
        };
        assert_eq!(
            expired_reason(config(), 0, Some(current), Some(previous)),
            Some(LockReason::SystemIdle)
        );
    }

    #[test]
    fn disabled_rules_and_missing_samples_do_not_expire() {
        let disabled = VaultLockConfig {
            activity_enabled: false,
            system_idle_enabled: false,
            ..config()
        };
        assert_eq!(expired_reason(disabled, 99_999, None, None), None);
    }

    #[test]
    fn loads_legacy_activity_minutes_and_system_idle_defaults() {
        let conn = rusqlite::Connection::open_in_memory().expect("open db");
        conn.execute_batch(
            "CREATE TABLE user_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);\
             INSERT INTO user_settings(key, value) VALUES ('vault_lock_profile', 'strict');",
        )
        .expect("seed settings");

        assert_eq!(
            load_config(&conn).expect("load config"),
            VaultLockConfig {
                activity_enabled: true,
                activity_after_secs: 600,
                system_idle_enabled: true,
                system_idle_after_secs: 900,
            }
        );
    }
}

//! 壁纸配置读写
//!
//! 默认值常量集中维护在此；前端读不到时回落 default。
//! 不动 user_settings schema，仅以 wallpaper.* 为 key 前缀写入既有表。

#![allow(dead_code)] // Phase 0 骨架：部分 KEY/helper 由后续 Phase 接入

use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::tools::helpers::db_conn;

// ── key 常量 ────────────────────────────────────────

pub const KEY_ENABLED: &str = "wallpaper.enabled";
pub const KEY_STYLE: &str = "wallpaper.style";
pub const KEY_POSITION: &str = "wallpaper.position";
pub const KEY_REFRESH_INTERVAL_MIN: &str = "wallpaper.refresh_interval_min";
pub const KEY_ORIGINAL_PATH: &str = "wallpaper.original_path";
pub const KEY_ORIGINAL_SET_METHOD: &str = "wallpaper.original_set_method";
pub const KEY_FULLSCREEN_BLACKLIST: &str = "wallpaper.fullscreen_blacklist";
pub const KEY_PRIVACY_MASK: &str = "wallpaper.privacy_mask";
pub const KEY_PRIVACY_MASK_UNTIL: &str = "wallpaper.privacy_mask_until";
pub const KEY_EXIT_BEHAVIOR: &str = "wallpaper.exit_behavior";
pub const KEY_BOSS_KEY: &str = "wallpaper.boss_key";
pub const KEY_IMAGE_FORMAT: &str = "wallpaper.image_format";
pub const KEY_KEEP_HISTORY_COUNT: &str = "wallpaper.keep_history_count";

// ── 默认值 ────────────────────────────────────────

pub const DEFAULT_STYLE: &str = "dashboard";
pub const DEFAULT_POSITION: &str = "right";
pub const DEFAULT_REFRESH_INTERVAL_MIN: i64 = 15;
pub const DEFAULT_EXIT_BEHAVIOR: &str = "restore_original";
pub const DEFAULT_BOSS_KEY: &str = "Ctrl+Alt+W";
pub const DEFAULT_IMAGE_FORMAT: &str = "jpeg";
pub const DEFAULT_KEEP_HISTORY_COUNT: i64 = 20;

/// 默认仅纳入演示 / 录屏 / 会议软件，避免 chrome / vlc 长期误切净。
pub fn default_fullscreen_blacklist() -> Vec<String> {
    vec![
        "obs64.exe".into(),
        "obs32.exe".into(),
        "powerpnt.exe".into(),
        "wpp.exe".into(),
        "zoom.exe".into(),
    ]
}

// ── 配置结构体 ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperConfig {
    pub enabled: bool,
    pub style: String,
    pub position: String,
    pub refresh_interval_min: i64,
    pub fullscreen_blacklist: Vec<String>,
    pub privacy_mask: bool,
    pub privacy_mask_until: Option<String>,
    pub exit_behavior: String,
    pub boss_key: String,
    pub image_format: String,
    pub keep_history_count: i64,
}

impl Default for WallpaperConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            style: DEFAULT_STYLE.into(),
            position: DEFAULT_POSITION.into(),
            refresh_interval_min: DEFAULT_REFRESH_INTERVAL_MIN,
            fullscreen_blacklist: default_fullscreen_blacklist(),
            privacy_mask: false,
            privacy_mask_until: None,
            exit_behavior: DEFAULT_EXIT_BEHAVIOR.into(),
            boss_key: DEFAULT_BOSS_KEY.into(),
            image_format: DEFAULT_IMAGE_FORMAT.into(),
            keep_history_count: DEFAULT_KEEP_HISTORY_COUNT,
        }
    }
}

// ── 读 ────────────────────────────────────────────

/// 读取整个 wallpaper 配置；任意 key 不存在或解析失败时回落默认。
pub fn read_config() -> WallpaperConfig {
    let mut cfg = WallpaperConfig::default();
    let Ok(conn) = db_conn() else { return cfg };

    if let Some(v) = read_string(&conn, KEY_ENABLED) {
        cfg.enabled = parse_bool(&v).unwrap_or(false);
    }
    if let Some(v) = read_string(&conn, KEY_STYLE) {
        cfg.style = v;
    }
    if let Some(v) = read_string(&conn, KEY_POSITION) {
        cfg.position = v;
    }
    if let Some(v) = read_string(&conn, KEY_REFRESH_INTERVAL_MIN) {
        if let Ok(n) = v.parse::<i64>() {
            cfg.refresh_interval_min = n;
        }
    }
    if let Some(v) = read_string(&conn, KEY_FULLSCREEN_BLACKLIST) {
        if let Ok(list) = serde_json::from_str::<Vec<String>>(&v) {
            cfg.fullscreen_blacklist = list;
        }
    }
    if let Some(v) = read_string(&conn, KEY_PRIVACY_MASK) {
        cfg.privacy_mask = parse_bool(&v).unwrap_or(false);
    }
    cfg.privacy_mask_until = read_string(&conn, KEY_PRIVACY_MASK_UNTIL).filter(|s| !s.is_empty());
    if let Some(v) = read_string(&conn, KEY_EXIT_BEHAVIOR) {
        cfg.exit_behavior = v;
    }
    if let Some(v) = read_string(&conn, KEY_BOSS_KEY) {
        cfg.boss_key = v;
    }
    if let Some(v) = read_string(&conn, KEY_IMAGE_FORMAT) {
        cfg.image_format = v;
    }
    if let Some(v) = read_string(&conn, KEY_KEEP_HISTORY_COUNT) {
        if let Ok(n) = v.parse::<i64>() {
            cfg.keep_history_count = n;
        }
    }
    cfg
}

pub fn read_string(conn: &rusqlite::Connection, key: &str) -> Option<String> {
    let mut stmt = conn
        .prepare("SELECT value FROM user_settings WHERE key = ?1")
        .ok()?;
    stmt.query_row(params![key], |row| row.get::<_, String>(0)).ok()
}

pub fn read_bool(conn: &rusqlite::Connection, key: &str, default: bool) -> bool {
    read_string(conn, key)
        .and_then(|s| parse_bool(&s))
        .unwrap_or(default)
}

pub fn read_string_or(conn: &rusqlite::Connection, key: &str, default: &str) -> String {
    read_string(conn, key).unwrap_or_else(|| default.into())
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

// ── 写 ────────────────────────────────────────────

pub fn set_string(key: &str, value: &str) -> Result<(), String> {
    let conn = db_conn()?;
    conn.execute(
        "INSERT INTO user_settings(key, value, updated_at) VALUES(?1, ?2, CURRENT_TIMESTAMP)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=CURRENT_TIMESTAMP",
        params![key, value],
    )
    .map_err(|e| format!("save wallpaper setting {key} failed: {e}"))?;
    Ok(())
}

/// 部分更新；payload 形如 `{ "style": "dashboard", "refreshIntervalMin": 30 }`。
pub fn set_config(payload: &Value) -> Result<Value, String> {
    let obj = payload
        .as_object()
        .ok_or("set_config payload must be object")?;

    for (key, val) in obj.iter() {
        match key.as_str() {
            // enabled 必须走 enable / disable 通道，避免绕过备份原图、销毁 hidden WebView 等副作用
            "enabled" => {
                return Err(
                    "enabled must be set via tool:wallpaper:enable / disable channels".into(),
                );
            }
            "style" => write_string(KEY_STYLE, val)?,
            "position" => write_string(KEY_POSITION, val)?,
            "refreshIntervalMin" => write_i64(KEY_REFRESH_INTERVAL_MIN, val)?,
            "fullscreenBlacklist" => write_string_array(KEY_FULLSCREEN_BLACKLIST, val)?,
            "privacyMask" => write_bool(KEY_PRIVACY_MASK, val)?,
            "privacyMaskUntil" => write_optional_string(KEY_PRIVACY_MASK_UNTIL, val)?,
            "exitBehavior" => write_string(KEY_EXIT_BEHAVIOR, val)?,
            "bossKey" => write_string(KEY_BOSS_KEY, val)?,
            "imageFormat" => write_string(KEY_IMAGE_FORMAT, val)?,
            "keepHistoryCount" => write_i64(KEY_KEEP_HISTORY_COUNT, val)?,
            other => {
                return Err(format!("unknown wallpaper config key: {other}"));
            }
        }
    }
    Ok(json!({ "ok": true }))
}

fn write_bool(key: &str, val: &Value) -> Result<(), String> {
    let b = val
        .as_bool()
        .ok_or_else(|| format!("{key} must be boolean"))?;
    set_string(key, if b { "true" } else { "false" })
}

fn write_string(key: &str, val: &Value) -> Result<(), String> {
    let s = val.as_str().ok_or_else(|| format!("{key} must be string"))?;
    set_string(key, s)
}

fn write_optional_string(key: &str, val: &Value) -> Result<(), String> {
    if val.is_null() {
        set_string(key, "")
    } else {
        let s = val
            .as_str()
            .ok_or_else(|| format!("{key} must be string or null"))?;
        set_string(key, s)
    }
}

fn write_i64(key: &str, val: &Value) -> Result<(), String> {
    let n = val
        .as_i64()
        .ok_or_else(|| format!("{key} must be integer"))?;
    set_string(key, &n.to_string())
}

fn write_string_array(key: &str, val: &Value) -> Result<(), String> {
    let arr = val
        .as_array()
        .ok_or_else(|| format!("{key} must be array"))?;
    let mut list: Vec<String> = Vec::with_capacity(arr.len());
    for v in arr {
        let s = v
            .as_str()
            .ok_or_else(|| format!("{key} element must be string"))?;
        list.push(s.to_string());
    }
    let json = serde_json::to_string(&list)
        .map_err(|e| format!("serialize {key} failed: {e}"))?;
    set_string(key, &json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_matches_design_v05() {
        let cfg = WallpaperConfig::default();
        assert_eq!(cfg.enabled, false);
        assert_eq!(cfg.style, "dashboard");
        assert_eq!(cfg.position, "right");
        assert_eq!(cfg.refresh_interval_min, 15);
        assert_eq!(cfg.exit_behavior, "restore_original");
        assert_eq!(cfg.boss_key, "Ctrl+Alt+W");
        assert_eq!(cfg.image_format, "jpeg");
        assert_eq!(cfg.keep_history_count, 20);
        assert_eq!(cfg.privacy_mask, false);
        assert!(cfg.privacy_mask_until.is_none());
    }

    #[test]
    fn default_blacklist_excludes_chrome_and_vlc() {
        let list = default_fullscreen_blacklist();
        assert!(list.contains(&"obs64.exe".to_string()));
        assert!(list.contains(&"powerpnt.exe".to_string()));
        assert!(list.contains(&"zoom.exe".to_string()));
        assert!(!list.contains(&"chrome.exe".to_string()));
        assert!(!list.contains(&"vlc.exe".to_string()));
    }

    #[test]
    fn parse_bool_handles_common_forms() {
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("FALSE"), Some(false));
        assert_eq!(parse_bool("1"), Some(true));
        assert_eq!(parse_bool("0"), Some(false));
        assert_eq!(parse_bool("yes"), Some(true));
        assert_eq!(parse_bool("nope"), None);
    }

    #[test]
    fn set_config_rejects_unknown_key() {
        let err = set_config(&json!({ "foo": "bar" })).expect_err("unknown key");
        assert!(err.contains("unknown wallpaper config key"));
    }

    #[test]
    fn set_config_rejects_enabled_key() {
        // enabled 必须走 enable / disable channel，避免绕过备份 / 销毁 WebView 副作用
        let err = set_config(&json!({ "enabled": false })).expect_err("must reject enabled");
        assert!(err.contains("enable / disable channels"));
    }

    #[test]
    fn set_config_requires_object() {
        let err = set_config(&json!("nope")).expect_err("must be object");
        assert!(err.contains("payload must be object"));
    }
}

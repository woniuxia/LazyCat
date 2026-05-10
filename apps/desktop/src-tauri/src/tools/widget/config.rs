//! 挂件配置读写
//!
//! 默认值常量集中维护在此；前端读不到时回落 default。
//! 不动 user_settings schema，仅以 widget.* 为 key 前缀写入既有表。
//!
//! v2（挂件改造）：原 PNG 链路相关字段（position / exit_behavior /
//! image_format / keep_history_count / original_path / original_set_method）
//! 已废弃，由 mod.rs::enable_widget 启动时一次性清理。

#![allow(dead_code)]

use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::tools::helpers::db_conn;
use crate::tools::widget::session;

// ── key 常量 ────────────────────────────────────────

pub const KEY_ENABLED: &str = "widget.enabled";
pub const KEY_STYLE: &str = "widget.style";
pub const KEY_REFRESH_INTERVAL_MIN: &str = "widget.refresh_interval_min";
pub const KEY_FULLSCREEN_BLACKLIST: &str = "widget.fullscreen_blacklist";
pub const KEY_PRIVACY_MASK: &str = "widget.privacy_mask";
pub const KEY_PRIVACY_MASK_UNTIL: &str = "widget.privacy_mask_until";
/// 挂件持久化 Y（物理像素，整数）；空 = 居中。X 始终贴右由 widget.rs 计算。
pub const KEY_WIDGET_Y: &str = "widget.widget_y";

// ── 默认值 ────────────────────────────────────────

pub const DEFAULT_STYLE: &str = "dashboard";
pub const DEFAULT_REFRESH_INTERVAL_MIN: i64 = 15;

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
pub struct WidgetConfig {
    pub enabled: bool,
    pub style: String,
    pub refresh_interval_min: i64,
    pub fullscreen_blacklist: Vec<String>,
    pub privacy_mask: bool,
    pub privacy_mask_until: Option<String>,
    /// 挂件 Y 位置（物理像素）；None = 居中
    pub widget_y: Option<i64>,
}

impl Default for WidgetConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            style: DEFAULT_STYLE.into(),
            refresh_interval_min: DEFAULT_REFRESH_INTERVAL_MIN,
            fullscreen_blacklist: default_fullscreen_blacklist(),
            privacy_mask: false,
            privacy_mask_until: None,
            widget_y: None,
        }
    }
}

// ── 读 ────────────────────────────────────────────

/// 读取整个挂件配置；任意 key 不存在或解析失败时回落默认。
pub fn read_config() -> WidgetConfig {
    let mut cfg = WidgetConfig::default();
    let Ok(conn) = db_conn() else { return cfg };

    if let Some(v) = read_string(&conn, KEY_ENABLED) {
        cfg.enabled = parse_bool(&v).unwrap_or(false);
    }
    if let Some(v) = read_string(&conn, KEY_STYLE) {
        cfg.style = v;
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
    cfg.privacy_mask_until = read_string(&conn, KEY_PRIVACY_MASK_UNTIL)
        .filter(|s| !s.is_empty());
    if let Some(v) = read_string(&conn, KEY_WIDGET_Y) {
        if let Ok(n) = v.parse::<i64>() {
            cfg.widget_y = Some(n);
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
    .map_err(|e| format!("save widget setting {key} failed: {e}"))?;
    session::session().mark_config_dirty();
    Ok(())
}

/// 删除 user_settings 中指定 key（数据迁移用）。不存在不报错。
pub fn delete_key(key: &str) -> Result<(), String> {
    let conn = db_conn()?;
    conn.execute("DELETE FROM user_settings WHERE key = ?1", params![key])
        .map_err(|e| format!("delete widget setting {key} failed: {e}"))?;
    Ok(())
}

/// 部分更新；payload 形如 `{ "refreshIntervalMin": 30 }`。
pub fn set_config(payload: &Value) -> Result<Value, String> {
    let obj = payload
        .as_object()
        .ok_or("set_config payload must be object")?;

    for (key, val) in obj.iter() {
        match key.as_str() {
            // enabled 必须走 enable / disable channel，避免绕过 widget 创建/销毁副作用
            "enabled" => {
                return Err(
                    "enabled must be set via tool:widget:enable / disable channels".into(),
                );
            }
            "style" => write_string(KEY_STYLE, val)?,
            "refreshIntervalMin" => write_i64(KEY_REFRESH_INTERVAL_MIN, val)?,
            "fullscreenBlacklist" => write_string_array(KEY_FULLSCREEN_BLACKLIST, val)?,
            "privacyMask" => write_bool(KEY_PRIVACY_MASK, val)?,
            "privacyMaskUntil" => write_optional_string(KEY_PRIVACY_MASK_UNTIL, val)?,
            "widgetY" => write_optional_i64(KEY_WIDGET_Y, val)?,
            other => {
                return Err(format!("unknown widget config key: {other}"));
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

fn write_optional_i64(key: &str, val: &Value) -> Result<(), String> {
    if val.is_null() {
        set_string(key, "")
    } else {
        let n = val
            .as_i64()
            .ok_or_else(|| format!("{key} must be integer or null"))?;
        set_string(key, &n.to_string())
    }
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

/// 将旧 wallpaper.* key 迁移到新 widget.* key。
/// 对于每个旧 key：若新 key 不存在且旧 key 有值，则复制到新 key 并删除旧 key。
pub fn migrate_legacy_keys() {
    let Ok(conn) = db_conn() else { return };
    let pairs: &[(&str, &str)] = &[
        (KEY_ENABLED, "wallpaper.enabled"),
        (KEY_STYLE, "wallpaper.style"),
        (KEY_REFRESH_INTERVAL_MIN, "wallpaper.refresh_interval_min"),
        (KEY_FULLSCREEN_BLACKLIST, "wallpaper.fullscreen_blacklist"),
        (KEY_PRIVACY_MASK, "wallpaper.privacy_mask"),
        (KEY_PRIVACY_MASK_UNTIL, "wallpaper.privacy_mask_until"),
        (KEY_WIDGET_Y, "wallpaper.widget_y"),
    ];
    for (new_key, old_key) in pairs {
        if read_string(&conn, new_key).is_some() {
            continue;
        }
        if let Some(val) = read_string(&conn, old_key) {
            let _ = set_string(new_key, &val);
            let _ = delete_key(old_key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_basics() {
        let cfg = WidgetConfig::default();
        assert_eq!(cfg.enabled, false);
        assert_eq!(cfg.style, "dashboard");
        assert_eq!(cfg.refresh_interval_min, 15);
        assert_eq!(cfg.privacy_mask, false);
        assert!(cfg.privacy_mask_until.is_none());
        assert!(cfg.widget_y.is_none());
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
        assert!(err.contains("unknown widget config key"));
    }

    #[test]
    fn set_config_rejects_enabled_key() {
        let err = set_config(&json!({ "enabled": false })).expect_err("must reject enabled");
        assert!(err.contains("enable / disable channels"));
    }

    #[test]
    fn set_config_requires_object() {
        let err = set_config(&json!("nope")).expect_err("must be object");
        assert!(err.contains("payload must be object"));
    }
}

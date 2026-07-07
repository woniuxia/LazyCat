use rusqlite::params;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use tauri_plugin_autostart::ManagerExt;

use super::helpers::{db_conn, get_base_dir, get_config_path, get_data_dir};

const ACTIONS: &[&str] = &[
    "get",
    "set",
    "get_all",
    "export",
    "import",
    "export_to_file",
    "import_from_file",
    "get_data_dir",
    "set_data_dir",
    "reset_data_dir",
    "enable_autostart",
    "disable_autostart",
    "is_autostart_enabled",
];

pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported settings action: {action}"));
    }
    match action {
        "get" => settings_get(payload),
        "set" => settings_set(payload),
        "get_all" => settings_get_all(),
        "export" => settings_export(),
        "import" => settings_import(payload),
        "export_to_file" => export_to_file(payload),
        "import_from_file" => import_from_file(payload),
        "get_data_dir" => action_get_data_dir(),
        "set_data_dir" => action_set_data_dir(payload),
        "reset_data_dir" => action_reset_data_dir(),
        _ => Err(format!("unsupported settings action: {action}")),
    }
}

pub fn execute_with_app(
    action: &str,
    payload: &Value,
    app: &tauri::AppHandle,
) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported settings action: {action}"));
    }
    match action {
        "enable_autostart" => enable_autostart(app),
        "disable_autostart" => disable_autostart(app),
        "is_autostart_enabled" => is_autostart_enabled(app),
        _ => execute(action, payload),
    }
}

fn settings_get(payload: &Value) -> Result<Value, String> {
    let key = payload["key"].as_str().unwrap_or_default();
    if key.is_empty() {
        return Err("settings key is empty".into());
    }
    let conn = db_conn()?;
    let result: Option<String> = conn
        .query_row(
            "SELECT value FROM user_settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .ok();
    match result {
        Some(v) => Ok(json!({ "key": key, "value": v })),
        None => Ok(Value::Null),
    }
}

fn settings_set(payload: &Value) -> Result<Value, String> {
    let key = payload["key"].as_str().unwrap_or_default();
    let value = payload["value"].as_str().unwrap_or_default();
    if key.is_empty() {
        return Err("settings key is empty".into());
    }
    let conn = db_conn()?;
    conn.execute(
        "INSERT INTO user_settings(key, value, updated_at) VALUES(?1, ?2, CURRENT_TIMESTAMP)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=CURRENT_TIMESTAMP",
        params![key, value],
    )
    .map_err(|e| format!("save setting failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn settings_get_all() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare("SELECT key, value FROM user_settings")
        .map_err(|e| format!("prepare query failed: {e}"))?;
    let mut map: HashMap<String, String> = HashMap::new();
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("query settings failed: {e}"))?;
    for r in rows {
        let (k, v) = r.map_err(|e| e.to_string())?;
        map.insert(k, v);
    }
    Ok(serde_json::to_value(map).map_err(|e| format!("serialize settings failed: {e}"))?)
}

fn settings_export() -> Result<Value, String> {
    // Collect all settings
    let conn = db_conn()?;
    let mut settings_map: HashMap<String, String> = HashMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT key, value FROM user_settings")
            .map_err(|e| format!("prepare settings query failed: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("query settings failed: {e}"))?;
        for r in rows {
            let (k, v) = r.map_err(|e| e.to_string())?;
            settings_map.insert(k, v);
        }
    }

    // Collect hosts profiles
    let mut hosts: Vec<Value> = Vec::new();
    {
        let mut stmt = conn
            .prepare("SELECT name, content, enabled FROM hosts_profiles ORDER BY updated_at DESC")
            .map_err(|e| format!("prepare hosts query failed: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(json!({
                    "name": row.get::<_, String>(0)?,
                    "content": row.get::<_, String>(1)?,
                    "enabled": row.get::<_, i64>(2)?,
                }))
            })
            .map_err(|e| format!("query hosts failed: {e}"))?;
        for r in rows {
            hosts.push(r.map_err(|e| e.to_string())?);
        }
    }

    let export_data = json!({
        "version": 2,
        "exportedAt": chrono::Utc::now().to_rfc3339(),
        "settings": settings_map,
        "hosts_profiles": hosts,
    });
    Ok(export_data)
}

fn settings_import(payload: &Value) -> Result<Value, String> {
    let data_str = payload["data"].as_str().unwrap_or_default();
    let mode = payload["mode"].as_str().unwrap_or("merge"); // "merge" or "overwrite"
    if data_str.is_empty() {
        return Err("import data is empty".into());
    }
    let data: Value =
        serde_json::from_str(data_str).map_err(|e| format!("parse import data failed: {e}"))?;

    let conn = db_conn()?;

    // Import settings
    if let Some(settings) = data["settings"].as_object() {
        if mode == "overwrite" {
            conn.execute("DELETE FROM user_settings", [])
                .map_err(|e| format!("clear settings failed: {e}"))?;
        }
        for (key, val) in settings {
            let value_str = match val {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            conn.execute(
                "INSERT INTO user_settings(key, value, updated_at) VALUES(?1, ?2, CURRENT_TIMESTAMP)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=CURRENT_TIMESTAMP",
                params![key, value_str],
            )
            .map_err(|e| format!("import setting '{key}' failed: {e}"))?;
        }
    }

    // Import hosts profiles
    if let Some(profiles) = data["hosts_profiles"].as_array() {
        if mode == "overwrite" {
            conn.execute("DELETE FROM hosts_profiles", [])
                .map_err(|e| format!("clear hosts_profiles failed: {e}"))?;
        }
        for profile in profiles {
            let name = profile["name"].as_str().unwrap_or_default();
            let content = profile["content"].as_str().unwrap_or_default();
            let enabled = profile["enabled"].as_i64().unwrap_or(0);
            if name.is_empty() {
                continue;
            }
            conn.execute(
                "INSERT INTO hosts_profiles(name, content, enabled, updated_at) VALUES(?1, ?2, ?3, CURRENT_TIMESTAMP)
                 ON CONFLICT(name) DO UPDATE SET content=excluded.content, enabled=excluded.enabled, updated_at=CURRENT_TIMESTAMP",
                params![name, content, enabled],
            )
            .map_err(|e| format!("import hosts profile '{name}' failed: {e}"))?;
        }
    }

    Ok(json!({ "ok": true }))
}

fn export_to_file(payload: &Value) -> Result<Value, String> {
    let path = payload["path"].as_str().ok_or("path is required")?;
    let data = settings_export()?;
    let json_str = serde_json::to_string_pretty(&data)
        .map_err(|e| format!("serialize export data failed: {e}"))?;
    fs::write(path, json_str).map_err(|e| format!("write file failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn import_from_file(payload: &Value) -> Result<Value, String> {
    let path = payload["path"].as_str().ok_or("path is required")?;
    let mode = payload["mode"].as_str().unwrap_or("merge");
    let content = fs::read_to_string(path).map_err(|e| format!("read file failed: {e}"))?;
    // Validate JSON
    serde_json::from_str::<Value>(&content).map_err(|e| format!("invalid JSON: {e}"))?;
    settings_import(&json!({ "data": content, "mode": mode }))
}

fn action_get_data_dir() -> Result<Value, String> {
    let data_dir = get_data_dir()?;
    let base_dir = get_base_dir()?;
    let config_path = get_config_path()?;
    let is_custom = data_dir != base_dir;
    Ok(json!({
        "dataDir": data_dir.to_string_lossy(),
        "baseDir": base_dir.to_string_lossy(),
        "configPath": config_path.to_string_lossy(),
        "isCustom": is_custom,
    }))
}

fn action_set_data_dir(payload: &Value) -> Result<Value, String> {
    let target = payload["path"].as_str().ok_or("path is required")?;
    let target_path = std::path::PathBuf::from(target);

    // 1. Create target dir, test write permission
    fs::create_dir_all(&target_path).map_err(|e| format!("create target dir failed: {e}"))?;
    let test_file = target_path.join(".lazycat_write_test");
    fs::write(&test_file, "test").map_err(|e| format!("target dir not writable: {e}"))?;
    let _ = fs::remove_file(&test_file);

    // 2. Check target doesn't already have lazycat.sqlite (avoid overwriting someone else's data)
    let target_db = target_path.join("lazycat.sqlite");
    if target_db.exists() {
        return Err(
            "target directory already contains lazycat.sqlite, choose an empty directory".into(),
        );
    }

    // 3. Copy current data to new directory
    let current_dir = get_data_dir()?;
    let current_db = current_dir.join("lazycat.sqlite");
    if current_db.exists() {
        fs::copy(&current_db, &target_db).map_err(|e| format!("copy database failed: {e}"))?;
    }

    // Copy hosts-backups directory
    let current_backups = current_dir.join("hosts-backups");
    let target_backups = target_path.join("hosts-backups");
    if current_backups.is_dir() {
        copy_dir_recursive(&current_backups, &target_backups)?;
    }

    // Copy attachments directory（富文本描述及未来文件附件的物理存储）
    let current_attachments = current_dir.join("attachments");
    let target_attachments = target_path.join("attachments");
    if current_attachments.is_dir() {
        copy_dir_recursive(&current_attachments, &target_attachments)?;
    }

    // Copy db-key（数据库工作台连接密码的本地加密密钥，缺失会导致已存密码无法解密）
    let current_db_key = current_dir.join("db-key");
    if current_db_key.is_file() {
        fs::copy(&current_db_key, target_path.join("db-key"))
            .map_err(|e| format!("copy db-key failed: {e}"))?;
    }

    // 4. Update config.json
    let config_path = get_config_path()?;
    let config = json!({ "data_dir": target });
    let config_str = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("serialize config failed: {e}"))?;
    fs::write(&config_path, config_str).map_err(|e| format!("write config.json failed: {e}"))?;

    Ok(json!({ "ok": true, "restartRequired": true }))
}

fn action_reset_data_dir() -> Result<Value, String> {
    let config_path = get_config_path()?;
    if config_path.exists() {
        let content = fs::read_to_string(&config_path).unwrap_or_default();
        if let Ok(mut obj) = serde_json::from_str::<serde_json::Map<String, Value>>(&content) {
            obj.remove("data_dir");
            let new_content = serde_json::to_string_pretty(&obj)
                .map_err(|e| format!("serialize config failed: {e}"))?;
            fs::write(&config_path, new_content)
                .map_err(|e| format!("write config.json failed: {e}"))?;
        } else {
            fs::write(&config_path, "{}").map_err(|e| format!("write config.json failed: {e}"))?;
        }
    }
    Ok(json!({ "ok": true, "restartRequired": true }))
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("create dir {:?} failed: {e}", dst))?;
    let entries = fs::read_dir(src).map_err(|e| format!("read dir {:?} failed: {e}", src))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read entry failed: {e}"))?;
        let file_type = entry
            .file_type()
            .map_err(|e| format!("get file type failed: {e}"))?;
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path)?;
        } else {
            fs::copy(entry.path(), &dst_path)
                .map_err(|e| format!("copy file {:?} failed: {e}", entry.path()))?;
        }
    }
    Ok(())
}

fn enable_autostart(app: &tauri::AppHandle) -> Result<Value, String> {
    app.autolaunch()
        .enable()
        .map_err(|e| format!("enable autostart failed: {e}"))?;

    // 同时保存到数据库
    let conn = db_conn()?;
    conn.execute(
        "INSERT INTO user_settings(key, value, updated_at) VALUES(?1, ?2, CURRENT_TIMESTAMP)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=CURRENT_TIMESTAMP",
        params!["autostart_enabled", "true"],
    )
    .map_err(|e| format!("save setting failed: {e}"))?;

    Ok(json!({ "success": true }))
}

fn disable_autostart(app: &tauri::AppHandle) -> Result<Value, String> {
    app.autolaunch()
        .disable()
        .map_err(|e| format!("disable autostart failed: {e}"))?;

    // 同时保存到数据库
    let conn = db_conn()?;
    conn.execute(
        "INSERT INTO user_settings(key, value, updated_at) VALUES(?1, ?2, CURRENT_TIMESTAMP)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=CURRENT_TIMESTAMP",
        params!["autostart_enabled", "false"],
    )
    .map_err(|e| format!("save setting failed: {e}"))?;

    Ok(json!({ "success": true }))
}

fn is_autostart_enabled(app: &tauri::AppHandle) -> Result<Value, String> {
    let enabled = app
        .autolaunch()
        .is_enabled()
        .map_err(|e| format!("check autostart status failed: {e}"))?;
    Ok(json!({ "enabled": enabled }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn settings_get_empty_key_should_fail() {
        let err = execute("get", &json!({ "key": "" })).expect_err("empty key");
        assert!(err.contains("settings key is empty"));
    }

    #[test]
    fn settings_unknown_action_should_fail() {
        let err = execute("unknown", &json!({})).expect_err("unknown action");
        assert!(err.contains("unsupported settings action"));
    }
}

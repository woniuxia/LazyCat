use rusqlite::params;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::helpers::db_conn;

const ACTIONS: &[&str] = &[
    "scan",
    "list",
    "add",
    "add_manual",
    "update",
    "remove",
    "reorder",
    "launch",
    "open_folder",
    "list_groups",
    "create_group",
    "rename_group",
    "delete_group",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported launcher action: {action}"));
    }
    match action {
        "scan" => scan_shortcuts(),
        "list" => list_entries(),
        "add" => add_entries(payload),
        "add_manual" => add_manual(payload),
        "update" => update_entry(payload),
        "remove" => remove_entry(payload),
        "reorder" => reorder_entries(payload),
        "launch" => launch_app(payload),
        "open_folder" => open_folder(payload),
        "list_groups" => list_groups(),
        "create_group" => create_group(payload),
        "rename_group" => rename_group(payload),
        "delete_group" => delete_group(payload),
        _ => Err(format!("launcher: unknown action '{action}'")),
    }
}

// ── Scan .lnk shortcuts from Start Menu & Desktop ──

fn scan_shortcuts() -> Result<Value, String> {
    let mut results: Vec<Value> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let dirs = collect_scan_dirs();
    for dir in &dirs {
        scan_dir_recursive(dir, &mut results, &mut seen);
    }

    Ok(json!({ "items": results }))
}

fn collect_scan_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let p = PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs");
        if p.is_dir() {
            dirs.push(p);
        }
    }
    if let Some(pd) = std::env::var_os("ProgramData") {
        let p = PathBuf::from(pd)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs");
        if p.is_dir() {
            dirs.push(p);
        }
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        let desktop = PathBuf::from(&profile).join("Desktop");
        if desktop.is_dir() {
            dirs.push(desktop);
        }
    }
    if let Some(public) = std::env::var_os("PUBLIC") {
        let desktop = PathBuf::from(public).join("Desktop");
        if desktop.is_dir() {
            dirs.push(desktop);
        }
    }
    dirs
}

fn scan_dir_recursive(
    dir: &Path,
    results: &mut Vec<Value>,
    seen: &mut std::collections::HashSet<String>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir_recursive(&path, results, seen);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("lnk") {
            continue;
        }
        if let Some(item) = parse_lnk(&path, seen) {
            results.push(item);
        }
    }
}

fn parse_lnk(lnk_path: &Path, seen: &mut std::collections::HashSet<String>) -> Option<Value> {
    // lnk crate can panic on malformed .lnk files (header.rs unwrap), catch it
    let lnk_path_buf = lnk_path.to_path_buf();
    let shell_link = std::panic::catch_unwind(|| lnk::ShellLink::open(&lnk_path_buf))
        .ok()?
        .ok()?;

    let target = shell_link
        .link_info()
        .as_ref()
        .and_then(|li| li.local_base_path().as_ref().map(|s| s.to_string()))
        .or_else(|| {
            shell_link.relative_path().as_ref().map(|rp| {
                let base = lnk_path.parent().unwrap_or(Path::new(""));
                base.join(rp).to_string_lossy().to_string()
            })
        })?;

    let target_lower = target.to_lowercase();

    // Filter: skip URLs, non-exe, uninstall entries, non-existent targets
    if !target_lower.ends_with(".exe") {
        return None;
    }
    if target_lower.contains("uninstall") || target_lower.contains("卸载") {
        return None;
    }
    if !Path::new(&target).exists() {
        return None;
    }

    // Deduplicate by lowercase exe path
    if !seen.insert(target_lower.clone()) {
        return None;
    }

    let name = lnk_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let arguments = shell_link
        .arguments()
        .as_deref()
        .unwrap_or_default()
        .to_string();

    Some(json!({
        "name": name,
        "exe_path": target,
        "arguments": arguments,
    }))
}

// ── Icon extraction ──

#[cfg(windows)]
fn extract_icon_base64(exe_path: &str) -> String {
    match windows_icons::get_icon_base64_by_path(exe_path) {
        Ok(b64) => b64,
        Err(_) => String::new(),
    }
}

#[cfg(not(windows))]
fn extract_icon_base64(_exe_path: &str) -> String {
    String::new()
}

// ── DB operations ──

fn list_entries() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, exe_path, arguments, icon_base64, group_name, sort_order,
                    launch_count, created_at, updated_at
             FROM launcher_entries ORDER BY launch_count DESC, sort_order ASC, id ASC",
        )
        .map_err(|e| format!("list query failed: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "exe_path": row.get::<_, String>(2)?,
                "arguments": row.get::<_, String>(3)?,
                "icon_base64": row.get::<_, String>(4)?,
                "group_name": row.get::<_, String>(5)?,
                "sort_order": row.get::<_, i64>(6)?,
                "launch_count": row.get::<_, i64>(7)?,
                "created_at": row.get::<_, String>(8)?,
                "updated_at": row.get::<_, String>(9)?,
            }))
        })
        .map_err(|e| format!("list map failed: {e}"))?;

    let items: Vec<Value> = rows.filter_map(|r| r.ok()).collect();
    Ok(json!({ "items": items }))
}

fn add_entries(payload: &Value) -> Result<Value, String> {
    let items = payload
        .get("items")
        .and_then(|v| v.as_array())
        .ok_or("missing 'items' array")?;

    let conn = db_conn()?;
    let mut count = 0i64;
    for item in items {
        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let exe_path = item.get("exe_path").and_then(|v| v.as_str()).unwrap_or("");
        let arguments = item.get("arguments").and_then(|v| v.as_str()).unwrap_or("");
        let group_name = item
            .get("group_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if exe_path.is_empty() {
            continue;
        }

        let icon = extract_icon_base64(exe_path);
        let result = conn.execute(
            "INSERT OR IGNORE INTO launcher_entries (name, exe_path, arguments, icon_base64, group_name)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![name, exe_path, arguments, icon, group_name],
        );
        if let Ok(n) = result {
            count += n as i64;
        }
    }
    Ok(json!({ "added": count }))
}

fn add_manual(payload: &Value) -> Result<Value, String> {
    let exe_path = payload
        .get("exe_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if exe_path.is_empty() {
        return Err("missing exe_path".into());
    }
    let p = Path::new(exe_path);
    if !p.exists() {
        return Err("file does not exist".into());
    }

    let is_dir = p.is_dir();
    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            if is_dir {
                p.file_name().and_then(|s| s.to_str()).unwrap_or("Unknown")
            } else {
                p.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown")
            }
        });
    let arguments = payload
        .get("arguments")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let group_name = payload
        .get("group_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let icon = if is_dir {
        String::new()
    } else {
        extract_icon_base64(exe_path)
    };

    let conn = db_conn()?;
    conn.execute(
        "INSERT OR IGNORE INTO launcher_entries (name, exe_path, arguments, icon_base64, group_name)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![name, exe_path, arguments, icon, group_name],
    )
    .map_err(|e| format!("add_manual failed: {e}"))?;

    Ok(json!({ "ok": true }))
}

fn update_entry(payload: &Value) -> Result<Value, String> {
    let id = payload
        .get("id")
        .and_then(|v| v.as_i64())
        .ok_or("missing id")?;
    let conn = db_conn()?;

    if let Some(name) = payload.get("name").and_then(|v| v.as_str()) {
        conn.execute(
            "UPDATE launcher_entries SET name = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![name, id],
        )
        .map_err(|e| format!("update name failed: {e}"))?;
    }
    if let Some(group) = payload.get("group_name").and_then(|v| v.as_str()) {
        conn.execute(
            "UPDATE launcher_entries SET group_name = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![group, id],
        ).map_err(|e| format!("update group failed: {e}"))?;
    }
    Ok(json!({ "ok": true }))
}

fn remove_entry(payload: &Value) -> Result<Value, String> {
    let id = payload
        .get("id")
        .and_then(|v| v.as_i64())
        .ok_or("missing id")?;
    let conn = db_conn()?;
    conn.execute("DELETE FROM launcher_entries WHERE id = ?1", params![id])
        .map_err(|e| format!("remove failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn reorder_entries(payload: &Value) -> Result<Value, String> {
    let orders = payload
        .get("orders")
        .and_then(|v| v.as_array())
        .ok_or("missing 'orders' array")?;

    let conn = db_conn()?;
    for item in orders {
        let id = item.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        let sort_order = item.get("sort_order").and_then(|v| v.as_i64()).unwrap_or(0);
        conn.execute(
            "UPDATE launcher_entries SET sort_order = ?1 WHERE id = ?2",
            params![sort_order, id],
        )
        .map_err(|e| format!("reorder failed: {e}"))?;
    }
    Ok(json!({ "ok": true }))
}

fn launch_app(payload: &Value) -> Result<Value, String> {
    let exe_path = payload
        .get("exe_path")
        .and_then(|v| v.as_str())
        .ok_or("missing exe_path")?;
    let arguments = payload
        .get("arguments")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let admin = payload
        .get("admin")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let p = Path::new(exe_path);
    if !p.exists() {
        return Err(format!("file not found: {exe_path}"));
    }

    // Increment launch_count
    let conn = db_conn()?;
    conn.execute(
        "UPDATE launcher_entries SET launch_count = launch_count + 1 WHERE exe_path = ?1",
        params![exe_path],
    )
    .ok();

    // Directories: open in default file manager
    if p.is_dir() {
        open::that(exe_path).map_err(|e| format!("open folder failed: {e}"))?;
        return Ok(json!({ "ok": true }));
    }

    if admin {
        launch_as_admin(exe_path, arguments)?;
    } else {
        let mut cmd = Command::new(exe_path);
        if !arguments.is_empty() {
            cmd.args(arguments.split_whitespace());
        }
        cmd.spawn().map_err(|e| format!("launch failed: {e}"))?;
    }
    Ok(json!({ "ok": true }))
}

#[cfg(windows)]
fn launch_as_admin(exe_path: &str, arguments: &str) -> Result<(), String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let verb: Vec<u16> = OsStr::new("runas").encode_wide().chain(Some(0)).collect();
    let file: Vec<u16> = OsStr::new(exe_path).encode_wide().chain(Some(0)).collect();
    let params: Vec<u16> = OsStr::new(arguments).encode_wide().chain(Some(0)).collect();

    let ret = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            params.as_ptr(),
            std::ptr::null(),
            windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL as i32,
        )
    };
    if (ret as isize) <= 32 {
        return Err("ShellExecuteW failed or user cancelled UAC".into());
    }
    Ok(())
}

#[cfg(not(windows))]
fn launch_as_admin(_exe_path: &str, _arguments: &str) -> Result<(), String> {
    Err("admin launch only supported on Windows".into())
}

fn open_folder(payload: &Value) -> Result<Value, String> {
    let exe_path = payload
        .get("exe_path")
        .and_then(|v| v.as_str())
        .ok_or("missing exe_path")?;
    let parent = Path::new(exe_path)
        .parent()
        .ok_or("cannot determine parent directory")?;
    open::that(parent).map_err(|e| format!("open folder failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

// ── Group management (stored in user_settings) ──

const GROUPS_SETTINGS_KEY: &str = "launcher_groups";

fn get_groups_from_settings() -> Vec<String> {
    let conn = match db_conn() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let mut stmt = match conn.prepare("SELECT value FROM user_settings WHERE key = ?1") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let value: Option<String> = stmt
        .query_row(params![GROUPS_SETTINGS_KEY], |row| row.get(0))
        .ok();
    match value {
        Some(v) => serde_json::from_str(&v).unwrap_or_default(),
        None => Vec::new(),
    }
}

fn save_groups_to_settings(groups: &[String]) -> Result<(), String> {
    let conn = db_conn()?;
    let value = serde_json::to_string(groups).map_err(|e| format!("serialize failed: {e}"))?;
    conn.execute(
        "INSERT OR REPLACE INTO user_settings (key, value) VALUES (?1, ?2)",
        params![GROUPS_SETTINGS_KEY, value],
    )
    .map_err(|e| format!("save groups failed: {e}"))?;
    Ok(())
}

fn list_groups() -> Result<Value, String> {
    let groups = get_groups_from_settings();
    Ok(json!({ "groups": groups }))
}

fn create_group(payload: &Value) -> Result<Value, String> {
    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("missing name")?
        .trim();
    if name.is_empty() {
        return Err("分组名称不能为空".into());
    }
    let mut groups = get_groups_from_settings();
    if groups.iter().any(|g| g.eq_ignore_ascii_case(name)) {
        return Err("分组名称已存在".into());
    }
    groups.push(name.to_string());
    save_groups_to_settings(&groups)?;
    Ok(json!({ "ok": true }))
}

fn rename_group(payload: &Value) -> Result<Value, String> {
    let old_name = payload
        .get("old_name")
        .and_then(|v| v.as_str())
        .ok_or("missing old_name")?;
    let new_name = payload
        .get("new_name")
        .and_then(|v| v.as_str())
        .ok_or("missing new_name")?
        .trim();
    if new_name.is_empty() {
        return Err("分组名称不能为空".into());
    }

    let mut groups = get_groups_from_settings();
    let idx = groups
        .iter()
        .position(|g| g == old_name)
        .ok_or("分组不存在")?;
    if groups
        .iter()
        .any(|g| g.eq_ignore_ascii_case(new_name) && g != old_name)
    {
        return Err("分组名称已存在".into());
    }
    groups[idx] = new_name.to_string();
    save_groups_to_settings(&groups)?;

    // Update entries with old group name
    let conn = db_conn()?;
    conn.execute(
        "UPDATE launcher_entries SET group_name = ?1, updated_at = CURRENT_TIMESTAMP WHERE group_name = ?2",
        params![new_name, old_name],
    )
    .map_err(|e| format!("update entries failed: {e}"))?;

    Ok(json!({ "ok": true }))
}

fn delete_group(payload: &Value) -> Result<Value, String> {
    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("missing name")?;

    let mut groups = get_groups_from_settings();
    groups.retain(|g| g != name);
    save_groups_to_settings(&groups)?;

    // Move entries to "未分组" (empty string)
    let conn = db_conn()?;
    conn.execute(
        "UPDATE launcher_entries SET group_name = '', updated_at = CURRENT_TIMESTAMP WHERE group_name = ?1",
        params![name],
    )
    .map_err(|e| format!("update entries failed: {e}"))?;

    Ok(json!({ "ok": true }))
}

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::helpers::db_conn;
use super::usage::{self, UsageKey, ACTION_LAUNCH, RESOURCE_LAUNCHER_ENTRY};

const ACTIONS: &[&str] = &[
    "scan",
    "list",
    "spotlight_list",
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
        "spotlight_list" => spotlight_list_entries(),
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
            "SELECT le.id, le.name, le.exe_path, le.arguments, le.icon_base64, le.group_name,
                    le.sort_order,
                    COALESCE((
                        SELECT SUM(u.use_count) FROM usage_daily u
                        WHERE u.resource_type = 'launcher-entry' AND u.scope_id = ''
                          AND u.resource_id = CAST(le.id AS TEXT) AND u.action = 'launch'
                    ), 0) AS usage_count,
                    le.created_at, le.updated_at
             FROM launcher_entries le
             ORDER BY usage_count DESC, le.sort_order ASC, le.id ASC",
        )
        .map_err(|e| format!("list query failed: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            let exe_path = row.get::<_, String>(2)?;
            let path_exists = Path::new(&exe_path).exists();
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "exe_path": exe_path,
                "arguments": row.get::<_, String>(3)?,
                "icon_base64": row.get::<_, String>(4)?,
                "group_name": row.get::<_, String>(5)?,
                "sort_order": row.get::<_, i64>(6)?,
                "launch_count": row.get::<_, i64>(7)?,
                "created_at": row.get::<_, String>(8)?,
                "updated_at": row.get::<_, String>(9)?,
                "path_exists": path_exists,
            }))
        })
        .map_err(|e| format!("list map failed: {e}"))?;

    let items: Vec<Value> = rows.filter_map(|r| r.ok()).collect();
    Ok(json!({ "items": items }))
}

fn spotlight_list_entries() -> Result<Value, String> {
    let conn = db_conn()?;
    spotlight_list_entries_with_conn(&conn)
}

fn spotlight_list_entries_with_conn(conn: &Connection) -> Result<Value, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, exe_path, arguments, group_name
             FROM launcher_entries
             ORDER BY sort_order ASC, id ASC",
        )
        .map_err(|error| format!("spotlight list query failed: {error}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "exe_path": row.get::<_, String>(2)?,
                "arguments": row.get::<_, String>(3)?,
                "group_name": row.get::<_, String>(4)?,
            }))
        })
        .map_err(|error| format!("spotlight list map failed: {error}"))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|error| format!("spotlight list read failed: {error}"))?);
    }
    Ok(json!({ "items": items }))
}

struct LauncherActionEntry {
    id: i64,
    name: String,
    exe_path: String,
    arguments: String,
}

fn load_action_entry_with_conn(
    conn: &Connection,
    id: i64,
) -> Result<Option<LauncherActionEntry>, String> {
    conn.query_row(
        "SELECT id, name, exe_path, arguments FROM launcher_entries WHERE id=?1",
        [id],
        |row| {
            Ok(LauncherActionEntry {
                id: row.get(0)?,
                name: row.get(1)?,
                exe_path: row.get(2)?,
                arguments: row.get(3)?,
            })
        },
    )
    .optional()
    .map_err(|error| format!("读取快捷启动目标失败: {error}"))
}

fn action_entry_availability(entry: &LauncherActionEntry) -> (bool, Option<String>) {
    if Path::new(&entry.exe_path).exists() {
        (true, None)
    } else {
        (false, Some(format!("路径不存在: {}", entry.exe_path)))
    }
}

pub(crate) fn list_action_targets_with_conn(
    conn: &Connection,
) -> Result<Vec<(String, String, bool, Option<String>)>, String> {
    usage::ensure_schema_and_migrate(conn)?;
    let mut stmt = conn
        .prepare(
            "SELECT le.id, le.name, le.exe_path, le.arguments
             FROM launcher_entries le
             ORDER BY COALESCE((
                 SELECT SUM(u.use_count) FROM usage_daily u
                 WHERE u.resource_type = 'launcher-entry' AND u.scope_id = ''
                   AND u.resource_id = CAST(le.id AS TEXT) AND u.action = 'launch'
             ), 0) DESC, le.sort_order ASC, le.id ASC",
        )
        .map_err(|error| format!("读取快捷启动目标失败: {error}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(LauncherActionEntry {
                id: row.get(0)?,
                name: row.get(1)?,
                exe_path: row.get(2)?,
                arguments: row.get(3)?,
            })
        })
        .map_err(|error| format!("读取快捷启动目标失败: {error}"))?;

    rows.map(|row| {
        let entry = row.map_err(|error| format!("读取快捷启动目标失败: {error}"))?;
        let (available, unavailable_reason) = action_entry_availability(&entry);
        Ok((
            entry.id.to_string(),
            entry.name,
            available,
            unavailable_reason,
        ))
    })
    .collect()
}

pub(crate) fn load_action_target_with_conn(
    conn: &Connection,
    id: i64,
) -> Result<Option<(String, bool, Option<String>)>, String> {
    load_action_entry_with_conn(conn, id).map(|entry| {
        entry.map(|entry| {
            let (available, unavailable_reason) = action_entry_availability(&entry);
            (entry.name, available, unavailable_reason)
        })
    })
}

pub(crate) fn launch_action_target(target_id: &str) -> Result<Option<String>, String> {
    let id = target_id
        .parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| format!("快捷启动目标 ID 无效: {target_id}"))?;
    let conn = db_conn()?;
    let entry = load_action_entry_with_conn(&conn, id)?
        .ok_or_else(|| format!("快捷启动目标不存在: {target_id}"))?;
    if let (_, Some(reason)) = action_entry_availability(&entry) {
        return Err(reason);
    }

    launch_path(&entry.exe_path, &entry.arguments, false)?;
    let warning = usage::record(
        &conn,
        UsageKey {
            resource_type: RESOURCE_LAUNCHER_ENTRY,
            scope_id: "",
            resource_id: &entry.id.to_string(),
        },
        ACTION_LAUNCH,
    )
    .err();
    Ok(Some(match warning {
        Some(warning) => format!("已启动 {}，但使用统计保存失败：{warning}", entry.name),
        None => format!("已启动 {}", entry.name),
    }))
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
    if let Some(exe_path) = payload.get("exe_path").and_then(|v| v.as_str()) {
        let exe_path = exe_path.trim();
        if exe_path.is_empty() {
            return Err("程序路径不能为空".into());
        }
        let icon = extract_icon_base64(exe_path);
        conn.execute(
            "UPDATE launcher_entries SET exe_path = ?1, icon_base64 = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?3",
            params![exe_path, icon, id],
        )
        .map_err(|e| format!("update path failed: {e}"))?;
    }
    if let Some(arguments) = payload.get("arguments").and_then(|v| v.as_str()) {
        conn.execute(
            "UPDATE launcher_entries SET arguments = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![arguments, id],
        )
        .map_err(|e| format!("update arguments failed: {e}"))?;
    }
    Ok(json!({ "ok": true }))
}

fn remove_entry(payload: &Value) -> Result<Value, String> {
    let id = payload
        .get("id")
        .and_then(|v| v.as_i64())
        .ok_or("missing id")?;
    let conn = db_conn()?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|error| format!("begin remove launcher transaction failed: {error}"))?;
    tx.execute("DELETE FROM launcher_entries WHERE id = ?1", params![id])
        .map_err(|e| format!("remove failed: {e}"))?;
    usage::delete_resource(
        &tx,
        UsageKey {
            resource_type: RESOURCE_LAUNCHER_ENTRY,
            scope_id: "",
            resource_id: &id.to_string(),
        },
    )?;
    tx.commit()
        .map_err(|error| format!("commit remove launcher transaction failed: {error}"))?;
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

    launch_path(exe_path, arguments, admin)?;

    let conn = db_conn()?;
    let entry_id = conn
        .query_row(
            "SELECT id FROM launcher_entries WHERE exe_path = ?1",
            params![exe_path],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("load launched entry failed: {error}"))?;
    let warning = entry_id.and_then(|entry_id| {
        usage::record(
            &conn,
            UsageKey {
                resource_type: RESOURCE_LAUNCHER_ENTRY,
                scope_id: "",
                resource_id: &entry_id.to_string(),
            },
            ACTION_LAUNCH,
        )
        .err()
    });
    Ok(json!({ "ok": true, "warning": warning }))
}

fn launch_path(exe_path: &str, arguments: &str, admin: bool) -> Result<(), String> {
    let path = Path::new(exe_path);
    if !path.exists() {
        return Err(format!("file not found: {exe_path}"));
    }
    if path.is_dir() {
        return open::that(exe_path).map_err(|error| format!("open folder failed: {error}"));
    }
    if admin {
        launch_as_admin(exe_path, arguments)
    } else {
        let mut command = Command::new(exe_path);
        apply_command_arguments(&mut command, arguments);
        command
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("launch failed: {error}"))
    }
}

#[cfg(windows)]
fn apply_command_arguments(command: &mut Command, arguments: &str) {
    use std::os::windows::process::CommandExt;
    if !arguments.trim().is_empty() {
        command.raw_arg(arguments);
    }
}

#[cfg(not(windows))]
fn apply_command_arguments(command: &mut Command, arguments: &str) {
    if !arguments.trim().is_empty() {
        command.args(arguments.split_whitespace());
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spotlight_list_excludes_icons_usage_and_filesystem_checks() {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "CREATE TABLE launcher_entries (
                 id INTEGER PRIMARY KEY,
                 name TEXT NOT NULL,
                 exe_path TEXT NOT NULL,
                 arguments TEXT NOT NULL DEFAULT '',
                 icon_base64 TEXT NOT NULL DEFAULT '',
                 group_name TEXT NOT NULL DEFAULT '',
                 sort_order INTEGER NOT NULL DEFAULT 0,
                 launch_count INTEGER NOT NULL DEFAULT 0
             );
             INSERT INTO launcher_entries(
                 id, name, exe_path, arguments, icon_base64, group_name, sort_order, launch_count
             ) VALUES(
                 7, 'IDE', 'C:\\missing\\ide.exe', '--reuse-window', 'large-icon', '开发', 2, 99
             );",
        )
        .expect("create launcher schema");

        let result = spotlight_list_entries_with_conn(&conn).unwrap();
        let item = &result["items"][0];

        assert_eq!(item["id"], 7);
        assert_eq!(item["name"], "IDE");
        assert_eq!(item["arguments"], "--reuse-window");
        assert_eq!(item["group_name"], "开发");
        assert!(item.get("icon_base64").is_none());
        assert!(item.get("launch_count").is_none());
        assert!(item.get("path_exists").is_none());
    }
}

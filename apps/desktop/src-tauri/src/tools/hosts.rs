use chrono::Local;
use rusqlite::params;
use serde_json::{json, Value};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::Command;

use super::helpers::{db_conn, get_data_dir};

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "save" => hosts_save(payload),
        "list" => hosts_list(),
        "delete" => hosts_delete(payload),
        "activate" => hosts_activate(payload),
        "reorder" => hosts_reorder(payload),
        "read_system" => hosts_read_system(),
        "admin_check" => hosts_admin_check(),
        "backup_list" => hosts_backup_list(),
        "backup_restore" => hosts_backup_restore(payload),
        _ => Err(format!("unsupported hosts action: {action}")),
    }
}

fn hosts_save(payload: &Value) -> Result<Value, String> {
    let name = payload["name"].as_str().unwrap_or_default();
    let content = payload["content"].as_str().unwrap_or_default();
    if name.is_empty() {
        return Err("hosts profile name is empty".into());
    }
    let conn = db_conn()?;
    let next_order: i64 = conn
        .query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM hosts_profiles", [], |r| r.get(0))
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO hosts_profiles(name, content, enabled, sort_order, updated_at) VALUES(?1, ?2, 0, ?3, CURRENT_TIMESTAMP)
        ON CONFLICT(name) DO UPDATE SET content=excluded.content, updated_at=CURRENT_TIMESTAMP",
        params![name, content, next_order],
    )
    .map_err(|e| format!("save hosts profile failed: {e}"))?;
    Ok(json!({"ok": true}))
}

fn hosts_list() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare("SELECT id, name, content, enabled, updated_at, sort_order FROM hosts_profiles ORDER BY enabled DESC, sort_order ASC, id ASC")
        .map_err(|e| format!("prepare query failed: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "content": row.get::<_, String>(2)?,
                "enabled": row.get::<_, i64>(3)? == 1,
                "updatedAt": row.get::<_, String>(4)?,
                "sortOrder": row.get::<_, i64>(5)?,
            }))
        })
        .map_err(|e| format!("query hosts failed: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(Value::Array(out))
}

fn hosts_delete(payload: &Value) -> Result<Value, String> {
    let name = payload["name"].as_str().unwrap_or_default();
    let conn = db_conn()?;
    conn.execute("DELETE FROM hosts_profiles WHERE name = ?1", params![name])
        .map_err(|e| format!("delete hosts profile failed: {e}"))?;
    Ok(json!({"ok": true}))
}

fn hosts_activate(payload: &Value) -> Result<Value, String> {
    let profile_name = payload["profileName"].as_str().unwrap_or_default();
    let mut content = payload["content"].as_str().unwrap_or_default().to_string();
    let conn = db_conn()?;
    if content.is_empty() {
        let mut stmt = conn
            .prepare("SELECT content FROM hosts_profiles WHERE name=?1 LIMIT 1")
            .map_err(|e| format!("prepare get profile failed: {e}"))?;
        let mut rows = stmt
            .query(params![profile_name])
            .map_err(|e| format!("query profile failed: {e}"))?;
        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            content = row.get::<_, String>(0).map_err(|e| e.to_string())?;
        }
    }
    if content.is_empty() {
        return Err("Hosts profile content is empty.".into());
    }
    let backup_dir = get_data_dir()?.join("hosts-backups");
    fs::create_dir_all(&backup_dir).map_err(|e| format!("create backup dir failed: {e}"))?;
    let original = fs::read_to_string(hosts_path()).map_err(|e| format!("read hosts failed: {e}"))?;
    let stamp = Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let backup_path = backup_dir.join(format!("{stamp}-{profile_name}.hosts.bak"));
    fs::write(&backup_path, original).map_err(|e| format!("write backup failed: {e}"))?;
    write_hosts_file(&content)?;
    conn.execute("UPDATE hosts_profiles SET enabled = 0", [])
        .map_err(|e| format!("disable previous profiles failed: {e}"))?;
    conn.execute(
        "UPDATE hosts_profiles SET enabled = 1, updated_at = CURRENT_TIMESTAMP WHERE name = ?1",
        params![profile_name],
    )
    .map_err(|e| format!("mark profile enabled failed: {e}"))?;
    Ok(json!({
      "backupPath": backup_path.to_string_lossy().to_string(),
      "digest": format!("{:x}", md5::compute(content.as_bytes()))
    }))
}

/// Accepts { "ids": [3, 1, 2] } — new display order of profile IDs.
fn hosts_reorder(payload: &Value) -> Result<Value, String> {
    let ids = payload["ids"]
        .as_array()
        .ok_or("ids must be an array")?;
    let conn = db_conn()?;
    for (idx, id_val) in ids.iter().enumerate() {
        let id = id_val.as_i64().ok_or("each id must be an integer")?;
        conn.execute(
            "UPDATE hosts_profiles SET sort_order = ?1 WHERE id = ?2",
            params![idx as i64, id],
        )
        .map_err(|e| format!("update sort_order failed: {e}"))?;
    }
    Ok(json!({"ok": true}))
}

fn hosts_path() -> PathBuf {
    PathBuf::from("C:\\Windows\\System32\\drivers\\etc\\hosts")
}

/// Try direct write first; on PermissionDenied, trigger UAC elevation via PowerShell.
fn write_hosts_file(content: &str) -> Result<(), String> {
    let path = hosts_path();
    match fs::write(&path, content.as_bytes()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            elevated_write_hosts(content)?;
            // Verify the elevated write actually succeeded
            let actual = fs::read_to_string(&path)
                .map_err(|e| format!("verify hosts write failed: {e}"))?;
            if actual.replace('\r', "") != content.replace('\r', "") {
                return Err("hosts 文件未被更新，UAC 提权可能被取消".into());
            }
            Ok(())
        }
        Err(e) => Err(format!("write hosts failed: {e}")),
    }
}

/// Write hosts via UAC-elevated PowerShell process.
/// 1. Save content to a temp file in .lazycat/
/// 2. Generate a .ps1 script that copies temp -> hosts
/// 3. Launch it elevated via `Start-Process -Verb RunAs`
fn elevated_write_hosts(content: &str) -> Result<(), String> {
    let data_dir = get_data_dir()?;
    let temp_path = data_dir.join("hosts-pending.tmp");
    let script_path = data_dir.join("hosts-elevate.ps1");
    let hosts = hosts_path();

    fs::write(&temp_path, content).map_err(|e| format!("write temp file failed: {e}"))?;

    // PS1 script: copy temp file to hosts location, then clean up temp
    let ps1 = format!(
        "Copy-Item -LiteralPath '{}' -Destination '{}' -Force\r\nRemove-Item -LiteralPath '{}' -Force\r\n",
        temp_path.to_string_lossy().replace('\'', "''"),
        hosts.to_string_lossy().replace('\'', "''"),
        temp_path.to_string_lossy().replace('\'', "''"),
    );
    fs::write(&script_path, &ps1).map_err(|e| format!("write elevate script failed: {e}"))?;

    // Outer PowerShell launches the script elevated via -Verb RunAs
    let launcher = format!(
        "Start-Process -FilePath 'powershell.exe' -ArgumentList '-NoProfile -ExecutionPolicy Bypass -File \"{}\"' -Verb RunAs -Wait",
        script_path.to_string_lossy()
    );

    let status = Command::new("powershell")
        .args(["-NoProfile", "-Command", &launcher])
        .status()
        .map_err(|e| format!("launch UAC elevation failed: {e}"))?;

    // Cleanup (script may have self-cleaned temp already)
    let _ = fs::remove_file(&script_path);
    let _ = fs::remove_file(&temp_path);

    if !status.success() {
        return Err("UAC 提权被取消或失败".into());
    }

    Ok(())
}

fn hosts_read_system() -> Result<Value, String> {
    let content = fs::read_to_string(hosts_path())
        .map_err(|e| format!("read system hosts failed: {e}"))?;
    Ok(json!({ "content": content }))
}

fn hosts_admin_check() -> Result<Value, String> {
    let path = hosts_path();
    let can_write = fs::OpenOptions::new().write(true).open(&path).is_ok();
    Ok(json!({ "canWrite": can_write }))
}

fn hosts_backup_list() -> Result<Value, String> {
    let backup_dir = get_data_dir()?.join("hosts-backups");
    if !backup_dir.exists() {
        return Ok(json!([]));
    }
    let mut entries: Vec<Value> = Vec::new();
    let read = fs::read_dir(&backup_dir)
        .map_err(|e| format!("read backup dir failed: {e}"))?;
    for entry in read {
        let entry = entry.map_err(|e| format!("read dir entry failed: {e}"))?;
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        if !name.ends_with(".bak") {
            continue;
        }
        let meta = fs::metadata(&path)
            .map_err(|e| format!("read file metadata failed: {e}"))?;
        let size = meta.len();
        let modified = meta.modified()
            .map(|t| {
                let dt: chrono::DateTime<Local> = t.into();
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            })
            .unwrap_or_default();
        entries.push(json!({
            "filename": name,
            "size": size,
            "modifiedAt": modified,
        }));
    }
    entries.sort_by(|a, b| {
        let ma = a["modifiedAt"].as_str().unwrap_or("");
        let mb = b["modifiedAt"].as_str().unwrap_or("");
        mb.cmp(ma)
    });
    Ok(Value::Array(entries))
}

fn hosts_backup_restore(payload: &Value) -> Result<Value, String> {
    let filename = payload["filename"].as_str().unwrap_or_default();
    if filename.is_empty() {
        return Err("backup filename is empty".into());
    }
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err("invalid backup filename".into());
    }
    let backup_dir = get_data_dir()?.join("hosts-backups");
    let backup_path = backup_dir.join(filename);
    if !backup_path.starts_with(&backup_dir) || !backup_path.exists() {
        return Err("backup file not found".into());
    }
    let backup_content = fs::read_to_string(&backup_path)
        .map_err(|e| format!("read backup file failed: {e}"))?;

    let current = fs::read_to_string(hosts_path())
        .map_err(|e| format!("read current hosts failed: {e}"))?;
    fs::create_dir_all(&backup_dir).map_err(|e| format!("create backup dir failed: {e}"))?;
    let stamp = Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let pre_restore_name = format!("{stamp}-pre-restore.hosts.bak");
    fs::write(backup_dir.join(&pre_restore_name), current)
        .map_err(|e| format!("write pre-restore backup failed: {e}"))?;

    write_hosts_file(&backup_content)?;
    Ok(json!({ "ok": true, "restoredFrom": filename }))
}

use chrono::Local;
use rusqlite::params;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

use super::helpers::{db_conn, get_data_dir};

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "save" => hosts_save(payload),
        "list" => hosts_list(),
        "delete" => hosts_delete(payload),
        "activate" => hosts_activate(payload),
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
    conn.execute(
        "INSERT INTO hosts_profiles(name, content, enabled, updated_at) VALUES(?1, ?2, 0, CURRENT_TIMESTAMP)
        ON CONFLICT(name) DO UPDATE SET content=excluded.content, updated_at=CURRENT_TIMESTAMP",
        params![name, content],
    )
    .map_err(|e| format!("save hosts profile failed: {e}"))?;
    Ok(json!({"ok": true}))
}

fn hosts_list() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare("SELECT id, name, content, enabled, updated_at FROM hosts_profiles ORDER BY updated_at DESC")
        .map_err(|e| format!("prepare query failed: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "content": row.get::<_, String>(2)?,
                "enabled": row.get::<_, i64>(3)? == 1,
                "updatedAt": row.get::<_, String>(4)?,
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
    let hosts_path = PathBuf::from("C:\\Windows\\System32\\drivers\\etc\\hosts");
    let backup_dir = get_data_dir()?.join("hosts-backups");
    fs::create_dir_all(&backup_dir).map_err(|e| format!("create backup dir failed: {e}"))?;
    let original = fs::read_to_string(&hosts_path).map_err(|e| format!("read hosts failed: {e}"))?;
    let stamp = Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let backup_path = backup_dir.join(format!("{stamp}-{profile_name}.hosts.bak"));
    fs::write(&backup_path, original).map_err(|e| format!("write backup failed: {e}"))?;
    fs::write(&hosts_path, content.as_bytes()).map_err(|e| format!("write hosts failed: {e}"))?;
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

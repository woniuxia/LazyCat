//! 附件持久化：供 pm_project / pm_item / todo 共用的内容寻址存储。
//!
//! 设计要点（见 docs/plans/2026-04-20-rich-description-tiptap-design.md §4~§6）：
//! - 物理文件按 blake3 前 16 字节 hex 作为文件名，扁平存放在 <data_dir>/attachments/
//! - DB `attachments` 表以 (owner_type, owner_id) 作为引用；同内容多 owner 只存一份物理文件
//! - owner_id 为 TEXT，兼容 "tmp-<uuid>" 暂存场景；提交时走 rebind 改写为真实 id
//! - 每次物理文件删除前都要再次按 hash 聚合计数，count=0 才能删

use std::fs;
use std::path::PathBuf;

use base64::{engine::general_purpose::STANDARD, Engine};
use rusqlite::{params, Connection};
use serde_json::{json, Value};

use super::helpers::{db_conn, get_attachments_dir, get_data_dir};

/// 单图上限：5 MB（前后端一致）
const MAX_SIZE_BYTES: i64 = 5 * 1024 * 1024;

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "save" => save(payload),
        "list" => list(payload),
        "remove" => remove(payload),
        "rebind" => rebind(payload),
        "cleanup_orphans" => cleanup_orphans(payload),
        "delete_by_owner" => delete_by_owner(payload),
        _ => Err(format!("unsupported attachments action: {action}")),
    }
}

// ── 公共辅助 ────────────────────────────────────────────

fn require_str<'a>(payload: &'a Value, key: &str) -> Result<&'a str, String> {
    payload
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("{key} is required"))
}

fn require_i64(payload: &Value, key: &str) -> Result<i64, String> {
    payload
        .get(key)
        .and_then(|v| v.as_i64())
        .ok_or_else(|| format!("{key} is required"))
}

fn validate_owner_type(t: &str) -> Result<(), String> {
    match t {
        "pm_project" | "pm_item" | "todo" => Ok(()),
        other => Err(format!("invalid owner_type: {other}")),
    }
}

/// 从 mime 或 fileName 推导扩展名；拒绝 image/svg+xml。
/// 返回小写、无前导点的扩展；未识别时返回 "bin"。
fn pick_ext(mime: &str, file_name: &str) -> Result<String, String> {
    let mime_lc = mime.trim().to_ascii_lowercase();
    if mime_lc == "image/svg+xml" || mime_lc == "image/svg" {
        return Err("svg not supported".into());
    }
    let from_mime = match mime_lc.as_str() {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        "image/bmp" => Some("bmp"),
        "image/avif" => Some("avif"),
        "image/heic" | "image/heif" => Some("heic"),
        "application/pdf" => Some("pdf"),
        _ => None,
    };
    if let Some(ext) = from_mime {
        return Ok(ext.to_string());
    }
    let lower = file_name.to_ascii_lowercase();
    if let Some(idx) = lower.rfind('.') {
        let ext = &lower[idx + 1..];
        if !ext.is_empty() && ext.len() <= 16 && ext.chars().all(|c| c.is_ascii_alphanumeric()) {
            // 二次防御 SVG
            if ext == "svg" {
                return Err("svg not supported".into());
            }
            return Ok(ext.to_string());
        }
    }
    Ok("bin".to_string())
}

fn abs_path_for(rel_path: &str) -> Result<PathBuf, String> {
    // rel_path 形如 "attachments/<hash>.<ext>"；解析时按 '/' 拆分，跨平台用 PathBuf::join
    let mut p = get_data_dir()?;
    for seg in rel_path.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(format!("invalid rel_path segment: {seg}"));
        }
        p.push(seg);
    }
    Ok(p)
}

fn row_to_attachment(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": row.get::<_, i64>(0)?,
        "ownerType": row.get::<_, String>(1)?,
        "ownerId": row.get::<_, String>(2)?,
        "relPath": row.get::<_, String>(3)?,
        "originalName": row.get::<_, String>(4)?,
        "mime": row.get::<_, String>(5)?,
        "size": row.get::<_, i64>(6)?,
        "hash": row.get::<_, String>(7)?,
        "kind": row.get::<_, String>(8)?,
        "createdAt": row.get::<_, String>(9)?,
    }))
}

// ── save ───────────────────────────────────────────────

fn save(payload: &Value) -> Result<Value, String> {
    let owner_type = require_str(payload, "ownerType")?;
    validate_owner_type(owner_type)?;
    let owner_id = require_str(payload, "ownerId")?;
    if owner_id.is_empty() {
        return Err("ownerId is required".into());
    }
    let file_name = payload
        .get("fileName")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let mime = payload.get("mime").and_then(|v| v.as_str()).unwrap_or("");
    let kind = payload
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("file");
    let bytes_b64 = require_str(payload, "bytesBase64")?;

    let bytes = STANDARD
        .decode(bytes_b64.as_bytes())
        .map_err(|e| format!("invalid base64: {e}"))?;
    let size = bytes.len() as i64;
    if size > MAX_SIZE_BYTES {
        return Err("single image exceeds 5 MB".into());
    }

    // blake3 前 16 字节 hex = 32 字符
    let hash_full = blake3::hash(&bytes);
    let hex = hash_full.to_hex();
    let hash = hex
        .as_str()
        .get(..32)
        .ok_or("hash length unexpected")?
        .to_string();

    let ext = pick_ext(mime, file_name)?;

    let conn = db_conn()?;

    // 命中已有物理文件则复用 rel_path（避免同内容不同扩展名二次落盘）
    let existing_rel: Option<String> = conn
        .query_row(
            "SELECT rel_path FROM attachments WHERE hash = ?1 LIMIT 1",
            params![&hash],
            |r| r.get(0),
        )
        .ok();

    let rel_path = existing_rel.unwrap_or_else(|| format!("attachments/{}.{}", hash, ext));
    let abs = abs_path_for(&rel_path)?;

    if !abs.exists() {
        // 保证父目录存在（当自定义数据目录首次写入时）
        let _ = get_attachments_dir()?;
        fs::write(&abs, &bytes).map_err(|e| format!("write attachment failed: {e}"))?;
    }

    conn.execute(
        "INSERT INTO attachments
            (owner_type, owner_id, rel_path, original_name, mime, size, hash, kind)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![owner_type, owner_id, &rel_path, file_name, mime, size, &hash, kind],
    )
    .map_err(|e| format!("insert attachment failed: {e}"))?;

    let id = conn.last_insert_rowid();
    Ok(json!({
        "id": id,
        "relPath": rel_path,
        "hash": hash,
        "size": size,
    }))
}

// ── list ───────────────────────────────────────────────

fn list(payload: &Value) -> Result<Value, String> {
    let owner_type = require_str(payload, "ownerType")?;
    validate_owner_type(owner_type)?;
    let owner_id = require_str(payload, "ownerId")?;
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, owner_type, owner_id, rel_path, original_name, mime, size, hash, kind, created_at
             FROM attachments
             WHERE owner_type = ?1 AND owner_id = ?2
             ORDER BY id",
        )
        .map_err(|e| format!("prepare list: {e}"))?;
    let rows = stmt
        .query_map(params![owner_type, owner_id], row_to_attachment)
        .map_err(|e| format!("query list: {e}"))?;
    let items: Vec<Value> = rows.filter_map(|r| r.ok()).collect();
    Ok(Value::Array(items))
}

// ── remove（单条） ─────────────────────────────────────

fn remove(payload: &Value) -> Result<Value, String> {
    let id = require_i64(payload, "id")?;
    let conn = db_conn()?;

    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT hash, rel_path FROM attachments WHERE id = ?1",
            params![id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .ok();
    let Some((hash, rel_path)) = row else {
        return Ok(json!({ "removedFile": false }));
    };

    conn.execute("DELETE FROM attachments WHERE id = ?1", params![id])
        .map_err(|e| format!("delete attachment row: {e}"))?;

    let still: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM attachments WHERE hash = ?1",
            params![&hash],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let mut removed_file = false;
    if still == 0 {
        if let Ok(abs) = abs_path_for(&rel_path) {
            match fs::remove_file(&abs) {
                Ok(()) => removed_file = true,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => eprintln!("attachments::remove delete file failed: {e}"),
            }
        }
    }
    Ok(json!({ "removedFile": removed_file }))
}

// ── rebind（tmp-<uuid> → realId） ──────────────────────

fn rebind(payload: &Value) -> Result<Value, String> {
    let owner_type = require_str(payload, "ownerType")?;
    validate_owner_type(owner_type)?;
    let from_owner_id = require_str(payload, "fromOwnerId")?;
    let to_owner_id = require_str(payload, "toOwnerId")?;
    if from_owner_id == to_owner_id {
        return Ok(json!({ "updated": 0 }));
    }
    let conn = db_conn()?;
    let updated = conn
        .execute(
            "UPDATE attachments SET owner_id = ?1
             WHERE owner_type = ?2 AND owner_id = ?3",
            params![to_owner_id, owner_type, from_owner_id],
        )
        .map_err(|e| format!("rebind failed: {e}"))?;
    Ok(json!({ "updated": updated }))
}

// ── cleanup_orphans（保留 keepIds） ────────────────────

fn cleanup_orphans(payload: &Value) -> Result<Value, String> {
    let owner_type = require_str(payload, "ownerType")?;
    validate_owner_type(owner_type)?;
    let owner_id = require_str(payload, "ownerId")?;
    let keep_ids: Vec<i64> = payload
        .get("keepIds")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_i64()).collect())
        .unwrap_or_default();

    let conn = db_conn()?;
    let (removed_count, removed_files) =
        cleanup_orphans_impl(&conn, owner_type, owner_id, &keep_ids)?;
    Ok(json!({
        "removedCount": removed_count,
        "removedFiles": removed_files,
    }))
}

// ── delete_by_owner（= cleanup_orphans with keepIds=[]） ──

fn delete_by_owner(payload: &Value) -> Result<Value, String> {
    let owner_type = require_str(payload, "ownerType")?;
    validate_owner_type(owner_type)?;
    let owner_id = require_str(payload, "ownerId")?;
    let conn = db_conn()?;
    let (removed_count, removed_files) =
        cleanup_orphans_impl(&conn, owner_type, owner_id, &[])?;
    Ok(json!({
        "removedCount": removed_count,
        "removedFiles": removed_files,
    }))
}

/// 供其他域（pm.rs / todo.rs / settings.rs 删除路径）事务内直接调用，
/// 避免再走一次 JSON payload → execute 的序列化。
/// 行为等同 cleanup_orphans(owner_type, owner_id, &[])。
pub fn delete_by_owner_internal(
    conn: &Connection,
    owner_type: &str,
    owner_id: &str,
) -> Result<(), String> {
    validate_owner_type(owner_type)?;
    let _ = cleanup_orphans_impl(conn, owner_type, owner_id, &[])?;
    Ok(())
}

// ── 核心实现：先收集 → 批量删 → 引用计数判断物理文件 ──

fn cleanup_orphans_impl(
    conn: &Connection,
    owner_type: &str,
    owner_id: &str,
    keep_ids: &[i64],
) -> Result<(i64, i64), String> {
    // 1. 找到待删 rows（id, hash, rel_path）
    let mut stmt = conn
        .prepare(
            "SELECT id, hash, rel_path FROM attachments
             WHERE owner_type = ?1 AND owner_id = ?2",
        )
        .map_err(|e| format!("prepare cleanup: {e}"))?;
    let all: Vec<(i64, String, String)> = stmt
        .query_map(params![owner_type, owner_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("query cleanup: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);

    let to_delete: Vec<(i64, String, String)> = all
        .into_iter()
        .filter(|(id, _, _)| !keep_ids.contains(id))
        .collect();
    if to_delete.is_empty() {
        return Ok((0, 0));
    }

    // 2. 批量 DELETE
    {
        let mut del_stmt = conn
            .prepare("DELETE FROM attachments WHERE id = ?1")
            .map_err(|e| format!("prepare delete: {e}"))?;
        for (id, _, _) in &to_delete {
            del_stmt
                .execute(params![id])
                .map_err(|e| format!("delete attachment row: {e}"))?;
        }
    }

    // 3. 对每个被删 hash 检查全局引用计数，=0 则删物理文件
    let mut removed_files: i64 = 0;
    let mut hashes_seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (_id, hash, rel_path) in &to_delete {
        if !hashes_seen.insert(hash.clone()) {
            continue; // 同一 hash 只检查一次
        }
        let still: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM attachments WHERE hash = ?1",
                params![hash],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if still == 0 {
            if let Ok(abs) = abs_path_for(rel_path) {
                match fs::remove_file(&abs) {
                    Ok(()) => removed_files += 1,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => eprintln!("attachments cleanup remove file failed: {e}"),
                }
            }
        }
    }

    Ok((to_delete.len() as i64, removed_files))
}

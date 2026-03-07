use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use openssl::hash::MessageDigest;
use openssl::pkcs5::pbkdf2_hmac;
use openssl::rand::rand_bytes;
use openssl::symm::{decrypt, encrypt, Cipher};
use rusqlite::{Connection, params};
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use zeroize::Zeroize;

use super::helpers::db_conn;

// --- Tag helper functions ---

fn get_entry_tags(conn: &Connection, entry_id: i64) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT tag FROM vault_entry_tags WHERE entry_id = ?1 ORDER BY tag")
        .map_err(|e| format!("prepare tags: {e}"))?;
    let rows = stmt
        .query_map(params![entry_id], |row| row.get::<_, String>(0))
        .map_err(|e| format!("query tags: {e}"))?;
    let mut tags = Vec::new();
    for row in rows {
        tags.push(row.map_err(|e| format!("tag row: {e}"))?);
    }
    Ok(tags)
}

fn set_entry_tags(conn: &Connection, entry_id: i64, tags: &[String]) -> Result<(), String> {
    // Delete existing tags
    conn.execute("DELETE FROM vault_entry_tags WHERE entry_id = ?1", params![entry_id])
        .map_err(|e| format!("delete old tags: {e}"))?;
    // Insert new tags
    for tag in tags {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            continue;
        }
        conn.execute(
            "INSERT INTO vault_entry_tags (entry_id, tag) VALUES (?1, ?2)",
            params![entry_id, trimmed],
        )
        .map_err(|e| format!("insert tag: {e}"))?;
    }
    Ok(())
}

fn clear_entry_tags(conn: &Connection, entry_id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM vault_entry_tags WHERE entry_id = ?1", params![entry_id])
        .map_err(|e| format!("clear tags: {e}"))?;
    Ok(())
}

const CANARY_PLAINTEXT: &[u8] = b"LAZYCAT_VAULT_OK";
const PBKDF2_ITERATIONS: usize = 600_000;
const SALT_LEN: usize = 32;
const KEY_LEN: usize = 32;
const IV_LEN: usize = 16;
const VAULT_LOCK_PROFILE_KEY: &str = "vault_lock_profile";
const DEFAULT_LOCK_PROFILE: &str = "balanced";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VaultLockState {
    Unlocked,
    Locked,
}

impl VaultLockState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Unlocked => "unlocked",
            Self::Locked => "locked",
        }
    }
}

struct VaultSession {
    key: Option<[u8; 32]>,
    last_activity: Instant,
    hard_lock_after_secs: u64,
}

static VAULT_SESSION: Mutex<Option<VaultSession>> = Mutex::new(None);

fn normalize_lock_profile(value: &str) -> &'static str {
    match value {
        "strict" => "strict",
        "convenient" => "convenient",
        _ => DEFAULT_LOCK_PROFILE,
    }
}

fn resolve_hard_lock_after_secs(lock_profile: &str) -> u64 {
    match normalize_lock_profile(lock_profile) {
        "strict" => 600,
        "convenient" => 3600,
        _ => 1800,
    }
}

fn load_lock_profile(conn: &Connection) -> String {
    conn.query_row(
        "SELECT value FROM user_settings WHERE key = ?1",
        params![VAULT_LOCK_PROFILE_KEY],
        |row| row.get::<_, String>(0),
    )
    .map(|value| normalize_lock_profile(value.trim()).to_string())
    .unwrap_or_else(|_| DEFAULT_LOCK_PROFILE.to_string())
}

fn load_hard_lock_after_secs(conn: &Connection) -> u64 {
    resolve_hard_lock_after_secs(&load_lock_profile(conn))
}

fn clear_session_key(session: &mut VaultSession) {
    if let Some(key) = session.key.as_mut() {
        key.zeroize();
    }
    session.key = None;
}

fn hard_lock_session(guard: &mut Option<VaultSession>) {
    if let Some(session) = guard.as_mut() {
        clear_session_key(session);
    }
    *guard = None;
}

fn session_expired(session: &VaultSession) -> bool {
    session.last_activity.elapsed().as_secs() > session.hard_lock_after_secs
}

fn ensure_session_alive(guard: &mut Option<VaultSession>) -> Result<(), String> {
    match guard.as_ref() {
        None => Err("vault_locked".to_string()),
        Some(session) if session_expired(session) => {
            hard_lock_session(guard);
            Err("vault_locked_timeout".to_string())
        }
        Some(_) => Ok(()),
    }
}

fn current_lock_state() -> VaultLockState {
    VAULT_SESSION
        .lock()
        .map(|mut guard| match ensure_session_alive(&mut guard) {
            Ok(()) => VaultLockState::Unlocked,
            Err(_) => VaultLockState::Locked,
        })
        .unwrap_or(VaultLockState::Locked)
}

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; KEY_LEN], String> {
    let mut key = [0u8; KEY_LEN];
    pbkdf2_hmac(
        password.as_bytes(),
        salt,
        PBKDF2_ITERATIONS,
        MessageDigest::sha256(),
        &mut key,
    )
    .map_err(|e| format!("PBKDF2 failed: {e}"))?;
    Ok(key)
}

fn random_bytes(len: usize) -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; len];
    rand_bytes(&mut buf).map_err(|e| format!("random generation failed: {e}"))?;
    Ok(buf)
}

fn aes256_encrypt(key: &[u8; KEY_LEN], iv: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    encrypt(Cipher::aes_256_cbc(), key, Some(iv), plaintext)
        .map_err(|e| format!("AES encrypt failed: {e}"))
}

fn aes256_decrypt(key: &[u8; KEY_LEN], iv: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    decrypt(Cipher::aes_256_cbc(), key, Some(iv), ciphertext)
        .map_err(|e| format!("AES decrypt failed: {e}"))
}

fn get_session_key() -> Result<[u8; KEY_LEN], String> {
    let mut guard = VAULT_SESSION.lock().map_err(|e| format!("session lock: {e}"))?;
    ensure_session_alive(&mut guard)?;

    match guard.as_mut() {
        None => Err("vault_locked".to_string()),
        Some(session) => {
            session.last_activity = Instant::now();
            session.key.ok_or_else(|| "vault_locked".to_string())
        }
    }
}

fn cmd_status(_payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let setup: bool = conn
        .query_row(
            "SELECT count(*) > 0 FROM vault_canary WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);

    let lock_state = current_lock_state();

    Ok(json!({
        "setup": setup,
        "unlocked": lock_state == VaultLockState::Unlocked,
        "lockState": lock_state.as_str(),
    }))
}

fn cmd_setup(payload: &Value) -> Result<Value, String> {
    let password = payload["masterPassword"]
        .as_str()
        .ok_or("masterPassword required")?;
    if password.is_empty() {
        return Err("password cannot be empty".to_string());
    }

    let conn = db_conn()?;
    let already: bool = conn
        .query_row(
            "SELECT count(*) > 0 FROM vault_canary WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if already {
        return Err("vault already initialized".to_string());
    }

    let salt = random_bytes(SALT_LEN)?;
    let key = derive_key(password, &salt)?;
    let iv = random_bytes(IV_LEN)?;
    let encrypted = aes256_encrypt(&key, &iv, CANARY_PLAINTEXT)?;

    conn.execute(
        "INSERT INTO vault_canary (id, salt, iv, encrypted, iterations) VALUES (1, ?1, ?2, ?3, ?4)",
        params![
            BASE64.encode(&salt),
            BASE64.encode(&iv),
            BASE64.encode(&encrypted),
            PBKDF2_ITERATIONS as i64,
        ],
    )
    .map_err(|e| format!("save canary failed: {e}"))?;

    let hard_lock_after_secs = load_hard_lock_after_secs(&conn);

    // 初始化后自动解锁
    let mut guard = VAULT_SESSION.lock().map_err(|e| format!("session lock: {e}"))?;
    *guard = Some(VaultSession {
        key: Some(key),
        last_activity: Instant::now(),
        hard_lock_after_secs,
    });

    Ok(json!({ "ok": true }))
}

fn cmd_unlock(payload: &Value) -> Result<Value, String> {
    let password = payload["masterPassword"]
        .as_str()
        .ok_or("masterPassword required")?;

    let conn = db_conn()?;
    let (salt_b64, iv_b64, encrypted_b64): (String, String, String) = conn
        .query_row(
            "SELECT salt, iv, encrypted FROM vault_canary WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| "vault not initialized".to_string())?;

    let salt = BASE64
        .decode(&salt_b64)
        .map_err(|e| format!("invalid salt: {e}"))?;
    let iv = BASE64
        .decode(&iv_b64)
        .map_err(|e| format!("invalid iv: {e}"))?;
    let encrypted = BASE64
        .decode(&encrypted_b64)
        .map_err(|e| format!("invalid encrypted data: {e}"))?;

    let key = derive_key(password, &salt)?;
    let decrypted = aes256_decrypt(&key, &iv, &encrypted).map_err(|_| "wrong_password".to_string())?;

    if decrypted != CANARY_PLAINTEXT {
        return Err("wrong_password".to_string());
    }

    let hard_lock_after_secs = load_hard_lock_after_secs(&conn);

    let mut guard = VAULT_SESSION.lock().map_err(|e| format!("session lock: {e}"))?;
    *guard = Some(VaultSession {
        key: Some(key),
        last_activity: Instant::now(),
        hard_lock_after_secs,
    });

    Ok(json!({ "unlocked": true, "lockState": VaultLockState::Unlocked.as_str() }))
}

fn cmd_lock(_payload: &Value) -> Result<Value, String> {
    let mut guard = VAULT_SESSION.lock().map_err(|e| format!("session lock: {e}"))?;
    hard_lock_session(&mut guard);
    Ok(json!({ "ok": true, "lockState": VaultLockState::Locked.as_str() }))
}

fn cmd_touch(_payload: &Value) -> Result<Value, String> {
    let hard_lock_after_secs = db_conn()
        .ok()
        .map(|conn| load_hard_lock_after_secs(&conn));
    let mut guard = VAULT_SESSION.lock().map_err(|e| format!("session lock: {e}"))?;
    ensure_session_alive(&mut guard)?;

    match guard.as_mut() {
        Some(session) => {
            if let Some(hard_lock_after_secs) = hard_lock_after_secs {
                session.hard_lock_after_secs = hard_lock_after_secs;
            }
            session.last_activity = Instant::now();
            Ok(json!({ "ok": true, "lockState": VaultLockState::Unlocked.as_str() }))
        }
        None => Err("vault_locked".to_string()),
    }
}

fn cmd_change_password(payload: &Value) -> Result<Value, String> {
    let current_password = payload["currentPassword"]
        .as_str()
        .ok_or("currentPassword required")?;
    let new_password = payload["newPassword"]
        .as_str()
        .ok_or("newPassword required")?;
    if new_password.is_empty() {
        return Err("new password cannot be empty".to_string());
    }

    // Verify current password
    let conn = db_conn()?;
    let (salt_b64, iv_b64, encrypted_b64): (String, String, String) = conn
        .query_row(
            "SELECT salt, iv, encrypted FROM vault_canary WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| "vault not initialized".to_string())?;

    let old_salt = BASE64.decode(&salt_b64).map_err(|e| format!("invalid salt: {e}"))?;
    let old_iv = BASE64.decode(&iv_b64).map_err(|e| format!("invalid iv: {e}"))?;
    let old_encrypted = BASE64.decode(&encrypted_b64).map_err(|e| format!("invalid data: {e}"))?;
    let old_key = derive_key(current_password, &old_salt)?;

    let decrypted = aes256_decrypt(&old_key, &old_iv, &old_encrypted)
        .map_err(|_| "wrong_password".to_string())?;
    if decrypted != CANARY_PLAINTEXT {
        return Err("wrong_password".to_string());
    }

    // Generate new key material
    let new_salt = random_bytes(SALT_LEN)?;
    let new_key = derive_key(new_password, &new_salt)?;
    let new_canary_iv = random_bytes(IV_LEN)?;
    let new_canary_encrypted = aes256_encrypt(&new_key, &new_canary_iv, CANARY_PLAINTEXT)?;

    // Re-encrypt all entries in a transaction
    let entries: Vec<(i64, String, String)> = {
        let mut stmt = conn
            .prepare("SELECT id, iv, encrypted_blob FROM vault_entries")
            .map_err(|e| format!("query entries: {e}"))?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(|e| format!("iterate entries: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("collect entries: {e}"))?
    };

    let tx = conn.unchecked_transaction().map_err(|e| format!("begin tx: {e}"))?;

    let re_encrypted_count = entries.len();
    for (id, entry_iv_b64, entry_blob_b64) in &entries {
        let entry_iv = BASE64.decode(entry_iv_b64).map_err(|e| format!("entry iv: {e}"))?;
        let entry_blob = BASE64.decode(entry_blob_b64).map_err(|e| format!("entry blob: {e}"))?;

        // Decrypt with old key
        let plain = aes256_decrypt(&old_key, &entry_iv, &entry_blob)
            .map_err(|e| format!("decrypt entry {id}: {e}"))?;

        // Re-encrypt with new key + new IV
        let new_iv = random_bytes(IV_LEN)?;
        let new_blob = aes256_encrypt(&new_key, &new_iv, &plain)?;

        tx.execute(
            "UPDATE vault_entries SET iv = ?1, encrypted_blob = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?3",
            params![BASE64.encode(&new_iv), BASE64.encode(&new_blob), id],
        )
        .map_err(|e| format!("update entry {id}: {e}"))?;
    }

    // Update canary
    tx.execute(
        "UPDATE vault_canary SET salt = ?1, iv = ?2, encrypted = ?3 WHERE id = 1",
        params![
            BASE64.encode(&new_salt),
            BASE64.encode(&new_canary_iv),
            BASE64.encode(&new_canary_encrypted),
        ],
    )
    .map_err(|e| format!("update canary: {e}"))?;

    tx.commit().map_err(|e| format!("commit: {e}"))?;

    let hard_lock_after_secs = load_hard_lock_after_secs(&conn);

    // 改密后刷新当前会话，继续保持已解锁状态
    let mut guard = VAULT_SESSION.lock().map_err(|e| format!("session lock: {e}"))?;
    *guard = Some(VaultSession {
        key: Some(new_key),
        last_activity: Instant::now(),
        hard_lock_after_secs,
    });

    Ok(json!({ "ok": true, "reEncrypted": re_encrypted_count }))
}

fn make_summary(category: &str, fields: &Value) -> String {
    match category {
        "app" => fields["url"].as_str().unwrap_or("").to_string(),
        "server" => {
            let addr = fields["address"].as_str().unwrap_or("");
            let stype = fields["serverType"].as_str().unwrap_or("");
            format!("{addr} {stype}").trim().to_string()
        }
        "database" => {
            let db_type = fields["dbType"].as_str().unwrap_or("");
            let addr = fields["address"].as_str().unwrap_or("");
            let port = fields["port"].as_u64().unwrap_or(0);
            if port > 0 {
                format!("{db_type} {addr}:{port}").trim().to_string()
            } else {
                format!("{db_type} {addr}").trim().to_string()
            }
        }
        _ => String::new(),
    }
}

fn cmd_list(payload: &Value) -> Result<Value, String> {
    let key = get_session_key()?;
    let conn = db_conn()?;
    let category = payload["category"].as_str().unwrap_or("");
    let keyword = payload["keyword"].as_str().unwrap_or("");
    let tag_filter = payload["tag"].as_str().unwrap_or("");

    let mut sql = String::from(
        "SELECT id, category, title, environment, iv, encrypted_blob, created_at, updated_at FROM vault_entries WHERE 1=1",
    );
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if !category.is_empty() {
        sql.push_str(" AND category = ?");
        param_values.push(Box::new(category.to_string()));
    }
    if !keyword.is_empty() {
        sql.push_str(" AND title LIKE ?");
        param_values.push(Box::new(format!("%{keyword}%")));
    }
    // Tag filter via join
    let tag_sql: String;
    if !tag_filter.is_empty() {
        tag_sql = format!(
            "{} AND id IN (SELECT entry_id FROM vault_entry_tags WHERE tag = ?)",
            sql
        );
        sql = tag_sql;
        param_values.push(Box::new(tag_filter.to_string()));
    }
    sql.push_str(" ORDER BY updated_at DESC");

    let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql).map_err(|e| format!("prepare: {e}"))?;
    let rows = stmt
        .query_map(params_refs.as_slice(), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
            ))
        })
        .map_err(|e| format!("query: {e}"))?;

    let mut entries: Vec<Value> = Vec::new();
    for row in rows {
        let (id, cat, title, environment, iv_b64, blob_b64, created_at, updated_at) =
            row.map_err(|e| format!("row: {e}"))?;

        let (account, summary) = match (BASE64.decode(&iv_b64), BASE64.decode(&blob_b64)) {
            (Ok(iv), Ok(blob)) => match aes256_decrypt(&key, &iv, &blob) {
                Ok(plain) => {
                    let fields: Value = serde_json::from_slice(&plain).unwrap_or(json!({}));
                    let acct = fields["account"].as_str().unwrap_or("").to_string();
                    let summ = make_summary(&cat, &fields);
                    (acct, summ)
                }
                Err(_) => (String::new(), String::new()),
            },
            _ => (String::new(), String::new()),
        };

        // Get tags for this entry
        let tags = get_entry_tags(&conn, id).unwrap_or_default();

        entries.push(json!({
            "id": id,
            "category": cat,
            "title": title,
            "environment": environment,
            "account": account,
            "summary": summary,
            "tags": tags,
            "createdAt": created_at,
            "updatedAt": updated_at,
        }));
    }
    Ok(json!(entries))
}

fn cmd_get(payload: &Value) -> Result<Value, String> {
    let key = get_session_key()?;
    let id = payload["id"].as_i64().ok_or("id required")?;

    let conn = db_conn()?;
    let (category, title, environment, iv_b64, blob_b64, created_at, updated_at): (
        String, String, String, String, String, String, String,
    ) = conn
        .query_row(
            "SELECT category, title, environment, iv, encrypted_blob, created_at, updated_at FROM vault_entries WHERE id = ?1",
            params![id],
            |row| Ok((
                row.get(0)?, row.get(1)?, row.get(2)?,
                row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?,
            )),
        )
        .map_err(|_| "entry not found".to_string())?;

    let iv = BASE64.decode(&iv_b64).map_err(|e| format!("iv: {e}"))?;
    let blob = BASE64.decode(&blob_b64).map_err(|e| format!("blob: {e}"))?;
    let plain = aes256_decrypt(&key, &iv, &blob)?;
    let fields: Value =
        serde_json::from_slice(&plain).map_err(|e| format!("parse fields: {e}"))?;

    // Get tags
    let tags = get_entry_tags(&conn, id).unwrap_or_default();

    Ok(json!({
        "id": id,
        "category": category,
        "title": title,
        "environment": environment,
        "fields": fields,
        "tags": tags,
        "createdAt": created_at,
        "updatedAt": updated_at,
    }))
}

fn cmd_create(payload: &Value) -> Result<Value, String> {
    let key = get_session_key()?;
    let category = payload["category"]
        .as_str()
        .ok_or("category required")?;
    if !["app", "server", "database"].contains(&category) {
        return Err("invalid category".to_string());
    }
    let title = payload["title"].as_str().unwrap_or("");
    let environment = payload["environment"].as_str().unwrap_or("");

    // Parse tags from payload
    let tags: Vec<String> = payload["tags"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    // Build the encrypted fields JSON
    let fields = build_fields(category, payload);
    let plain = serde_json::to_vec(&fields).map_err(|e| format!("serialize: {e}"))?;

    let iv = random_bytes(IV_LEN)?;
    let encrypted = aes256_encrypt(&key, &iv, &plain)?;

    let conn = db_conn()?;
    conn.execute(
        "INSERT INTO vault_entries (category, title, environment, iv, encrypted_blob) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            category,
            title,
            environment,
            BASE64.encode(&iv),
            BASE64.encode(&encrypted),
        ],
    )
    .map_err(|e| format!("insert: {e}"))?;

    let id = conn.last_insert_rowid();

    // Save tags
    if !tags.is_empty() {
        set_entry_tags(&conn, id, &tags)?;
    }

    Ok(json!({ "id": id }))
}

fn cmd_update(payload: &Value) -> Result<Value, String> {
    let key = get_session_key()?;
    let id = payload["id"].as_i64().ok_or("id required")?;

    let conn = db_conn()?;
    // Verify exists and get category
    let category: String = conn
        .query_row(
            "SELECT category FROM vault_entries WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|_| "entry not found".to_string())?;

    let actual_category = payload["category"].as_str().unwrap_or(&category);
    if !["app", "server", "database"].contains(&actual_category) {
        return Err("invalid category".to_string());
    }
    let title = payload["title"].as_str().unwrap_or("");
    let environment = payload["environment"].as_str().unwrap_or("");

    // Parse tags from payload
    let tags: Vec<String> = payload["tags"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let fields = build_fields(actual_category, payload);
    let plain = serde_json::to_vec(&fields).map_err(|e| format!("serialize: {e}"))?;

    let iv = random_bytes(IV_LEN)?;
    let encrypted = aes256_encrypt(&key, &iv, &plain)?;

    conn.execute(
        "UPDATE vault_entries SET category = ?1, title = ?2, environment = ?3, iv = ?4, encrypted_blob = ?5, updated_at = CURRENT_TIMESTAMP WHERE id = ?6",
        params![
            actual_category,
            title,
            environment,
            BASE64.encode(&iv),
            BASE64.encode(&encrypted),
            id,
        ],
    )
    .map_err(|e| format!("update: {e}"))?;

    // Update tags
    set_entry_tags(&conn, id, &tags)?;

    Ok(json!({ "ok": true }))
}

fn cmd_delete(payload: &Value) -> Result<Value, String> {
    let _key = get_session_key()?;
    let id = payload["id"].as_i64().ok_or("id required")?;

    let conn = db_conn()?;
    // Tags will be auto-deleted by CASCADE, but we can explicitly clear first
    clear_entry_tags(&conn, id)?;
    conn.execute("DELETE FROM vault_entries WHERE id = ?1", params![id])
        .map_err(|e| format!("delete: {e}"))?;

    Ok(json!({ "ok": true }))
}

fn cmd_tag_stats(_payload: &Value) -> Result<Value, String> {
    let _ = get_session_key()?;
    let conn = db_conn()?;

    let mut stmt = conn
        .prepare(
            "SELECT tag, COUNT(entry_id) as count FROM vault_entry_tags GROUP BY tag ORDER BY count DESC, tag ASC"
        )
        .map_err(|e| format!("prepare tag stats: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(json!({
                "tag": row.get::<_, String>(0)?,
                "count": row.get::<_, i64>(1)?,
            }))
        })
        .map_err(|e| format!("query tag stats: {e}"))?;

    let mut stats: Vec<Value> = Vec::new();
    for row in rows {
        stats.push(row.map_err(|e| format!("tag stat row: {e}"))?);
    }

    Ok(json!(stats))
}

fn cmd_rename_tag(payload: &Value) -> Result<Value, String> {
    let _key = get_session_key()?;
    let old_tag = payload["oldTag"]
        .as_str()
        .ok_or("oldTag required")?
        .trim();
    let new_tag = payload["newTag"]
        .as_str()
        .ok_or("newTag required")?
        .trim();

    if old_tag.is_empty() || new_tag.is_empty() {
        return Err("tag cannot be empty".to_string());
    }
    if old_tag == new_tag {
        return Ok(json!({ "updated": 0 }));
    }

    let conn = db_conn()?;

    // Check if new_tag already exists for some entries
    let conflict_count: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT entry_id) FROM vault_entry_tags WHERE tag = ?1 AND entry_id IN (SELECT entry_id FROM vault_entry_tags WHERE tag = ?2)",
            params![new_tag, old_tag],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if conflict_count > 0 {
        return Err("部分凭据已存在该标签，无法重命名".to_string());
    }

    // Update all entries with old_tag to new_tag
    let updated = conn
        .execute(
            "UPDATE vault_entry_tags SET tag = ?1 WHERE tag = ?2",
            params![new_tag, old_tag],
        )
        .map_err(|e| format!("rename tag: {e}"))?;

    Ok(json!({ "updated": updated }))
}

fn cmd_delete_tag(payload: &Value) -> Result<Value, String> {
    let _key = get_session_key()?;
    let tag = payload["tag"]
        .as_str()
        .ok_or("tag required")?
        .trim();

    if tag.is_empty() {
        return Err("tag cannot be empty".to_string());
    }

    let conn = db_conn()?;
    let deleted = conn
        .execute("DELETE FROM vault_entry_tags WHERE tag = ?1", params![tag])
        .map_err(|e| format!("delete tag: {e}"))?;

    Ok(json!({ "deleted": deleted }))
}

fn cmd_open_url(payload: &Value) -> Result<Value, String> {
    let url = payload["url"].as_str().ok_or("url required")?;
    if url.is_empty() {
        return Err("empty url".to_string());
    }
    // Validate URL structure to prevent injection
    let parsed = url::Url::parse(url).map_err(|_| "无效的 URL 格式".to_string())?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err("仅支持 http/https 链接".to_string()),
    }

    open::that(url).map_err(|e| format!("打开链接失败: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn build_fields(category: &str, payload: &Value) -> Value {
    match category {
        "app" => json!({
            "url": payload["url"].as_str().unwrap_or(""),
            "account": payload["account"].as_str().unwrap_or(""),
            "password": payload["password"].as_str().unwrap_or(""),
            "notes": payload["notes"].as_str().unwrap_or(""),
        }),
        "server" => json!({
            "address": payload["address"].as_str().unwrap_or(""),
            "serverType": payload["serverType"].as_str().unwrap_or("Linux"),
            "account": payload["account"].as_str().unwrap_or(""),
            "password": payload["password"].as_str().unwrap_or(""),
            "notes": payload["notes"].as_str().unwrap_or(""),
        }),
        "database" => json!({
            "dbType": payload["dbType"].as_str().unwrap_or("MySQL"),
            "address": payload["address"].as_str().unwrap_or(""),
            "port": payload["port"].as_u64().unwrap_or(0),
            "account": payload["account"].as_str().unwrap_or(""),
            "password": payload["password"].as_str().unwrap_or(""),
            "schema": payload["schema"].as_str().unwrap_or(""),
            "dbName": payload["dbName"].as_str().unwrap_or(""),
            "notes": payload["notes"].as_str().unwrap_or(""),
        }),
        _ => json!({}),
    }
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "status" => cmd_status(payload),
        "setup" => cmd_setup(payload),
        "unlock" => cmd_unlock(payload),
        "touch" => cmd_touch(payload),
        "lock" => cmd_lock(payload),
        "change_password" => cmd_change_password(payload),
        "list" => cmd_list(payload),
        "get" => cmd_get(payload),
        "create" => cmd_create(payload),
        "update" => cmd_update(payload),
        "delete" => cmd_delete(payload),
        "open_url" => cmd_open_url(payload),
        "tag_stats" => cmd_tag_stats(payload),
        "rename_tag" => cmd_rename_tag(payload),
        "delete_tag" => cmd_delete_tag(payload),
        _ => Err(format!("vault: unsupported action '{action}'")),
    }
}

pub fn force_lock() {
    if let Ok(mut guard) = VAULT_SESSION.lock() {
        hard_lock_session(&mut guard);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_derive_key_consistency() {
        let salt = vec![0u8; 32];
        let k1 = derive_key("test", &salt).unwrap();
        let k2 = derive_key("test", &salt).unwrap();
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_derive_key_different_passwords() {
        let salt = vec![0u8; 32];
        let k1 = derive_key("password1", &salt).unwrap();
        let k2 = derive_key("password2", &salt).unwrap();
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = derive_key("testpass", &[0u8; 32]).unwrap();
        let iv = vec![0u8; IV_LEN];
        let plaintext = b"hello world";
        let encrypted = aes256_encrypt(&key, &iv, plaintext).unwrap();
        let decrypted = aes256_decrypt(&key, &iv, &encrypted).unwrap();
        assert_eq!(&decrypted, plaintext);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = derive_key("correct", &[0u8; 32]).unwrap();
        let key2 = derive_key("wrong", &[0u8; 32]).unwrap();
        let iv = vec![0u8; IV_LEN];
        let encrypted = aes256_encrypt(&key1, &iv, b"secret").unwrap();
        assert!(aes256_decrypt(&key2, &iv, &encrypted).is_err());
    }

    #[test]
    fn test_build_fields_app() {
        let p = json!({ "url": "https://x.com", "account": "admin", "password": "123", "notes": "n" });
        let f = build_fields("app", &p);
        assert_eq!(f["url"], "https://x.com");
        assert_eq!(f["account"], "admin");
    }

    #[test]
    fn test_build_fields_server() {
        let p = json!({ "address": "10.0.0.1", "serverType": "Windows", "account": "root", "password": "p" });
        let f = build_fields("server", &p);
        assert_eq!(f["serverType"], "Windows");
    }

    #[test]
    fn test_build_fields_database() {
        let p = json!({ "dbType": "PostgreSQL", "address": "localhost", "port": 5432, "account": "pg", "password": "p", "dbName": "mydb" });
        let f = build_fields("database", &p);
        assert_eq!(f["port"], 5432);
        assert_eq!(f["dbType"], "PostgreSQL");
    }

    #[test]
    fn test_normalize_lock_profile_defaults_to_balanced() {
        assert_eq!(normalize_lock_profile("strict"), "strict");
        assert_eq!(normalize_lock_profile("convenient"), "convenient");
        assert_eq!(normalize_lock_profile("unexpected"), DEFAULT_LOCK_PROFILE);
    }

    #[test]
    fn test_resolve_hard_lock_after_secs_matches_profiles() {
        assert_eq!(resolve_hard_lock_after_secs("strict"), 600);
        assert_eq!(resolve_hard_lock_after_secs("balanced"), 1800);
        assert_eq!(resolve_hard_lock_after_secs("convenient"), 3600);
    }

    #[test]
    fn test_hard_lock_session_clears_key_and_guard() {
        let mut guard = Some(VaultSession {
            key: Some([7u8; KEY_LEN]),
            last_activity: Instant::now(),
            hard_lock_after_secs: 1800,
        });

        hard_lock_session(&mut guard);

        assert!(guard.is_none());
    }

    #[test]
    fn test_expired_session_hard_locks_guard() {
        let mut guard = Some(VaultSession {
            key: Some([1u8; KEY_LEN]),
            last_activity: Instant::now()
                .checked_sub(Duration::from_secs(16))
                .expect("checked_sub"),
            hard_lock_after_secs: 15,
        });

        let err = ensure_session_alive(&mut guard).expect_err("session should expire");

        assert_eq!(err, "vault_locked_timeout");
        assert!(guard.is_none());
    }
}

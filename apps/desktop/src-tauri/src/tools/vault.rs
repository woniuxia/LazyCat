use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use openssl::hash::MessageDigest;
use openssl::pkcs5::pbkdf2_hmac;
use openssl::rand::rand_bytes;
use openssl::symm::{decrypt, encrypt, Cipher};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::Emitter;
use zeroize::Zeroize;

use super::helpers::db_conn;
use super::vault_lock::{expired_reason, load_config, LockReason, VaultLockConfig};
use super::widget::guards::{try_system_input_snapshot, SystemInputSnapshot};

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

fn get_entry_tags_map(
    conn: &Connection,
    entry_ids: &[i64],
) -> Result<HashMap<i64, Vec<String>>, String> {
    let mut tags_by_entry = HashMap::with_capacity(entry_ids.len());
    if entry_ids.is_empty() {
        return Ok(tags_by_entry);
    }

    let placeholders = vec!["?"; entry_ids.len()].join(", ");
    let sql = format!(
        "SELECT entry_id, tag FROM vault_entry_tags WHERE entry_id IN ({placeholders}) ORDER BY entry_id, tag"
    );
    let params_refs: Vec<&dyn rusqlite::types::ToSql> = entry_ids
        .iter()
        .map(|entry_id| entry_id as &dyn rusqlite::types::ToSql)
        .collect();
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("prepare batch tags: {e}"))?;
    let rows = stmt
        .query_map(params_refs.as_slice(), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("query batch tags: {e}"))?;

    for row in rows {
        let (entry_id, tag) = row.map_err(|e| format!("batch tag row: {e}"))?;
        tags_by_entry.entry(entry_id).or_default().push(tag);
    }

    Ok(tags_by_entry)
}

fn set_entry_tags(conn: &Connection, entry_id: i64, tags: &[String]) -> Result<(), String> {
    // Delete existing tags
    conn.execute(
        "DELETE FROM vault_entry_tags WHERE entry_id = ?1",
        params![entry_id],
    )
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
    conn.execute(
        "DELETE FROM vault_entry_tags WHERE entry_id = ?1",
        params![entry_id],
    )
    .map_err(|e| format!("clear tags: {e}"))?;
    Ok(())
}

const CANARY_PLAINTEXT: &[u8] = b"LAZYCAT_VAULT_OK";
const PBKDF2_ITERATIONS: usize = 600_000;
const SALT_LEN: usize = 32;
const KEY_LEN: usize = 32;
const IV_LEN: usize = 16;
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
}

static VAULT_SESSION: Mutex<Option<VaultSession>> = Mutex::new(None);

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

fn ensure_session_alive(
    guard: &mut Option<VaultSession>,
    config: VaultLockConfig,
    current: Option<SystemInputSnapshot>,
    previous: Option<SystemInputSnapshot>,
) -> Result<(), String> {
    let Some(session) = guard.as_ref() else {
        return Err("vault_locked".to_string());
    };
    if expired_reason(
        config,
        session.last_activity.elapsed().as_secs(),
        current,
        previous,
    )
    .is_some()
    {
        hard_lock_session(guard);
        return Err("vault_locked_timeout".to_string());
    }
    Ok(())
}

fn current_lock_state(
    config: VaultLockConfig,
    current: Option<SystemInputSnapshot>,
) -> VaultLockState {
    VAULT_SESSION
        .lock()
        .map(|mut guard| match ensure_session_alive(&mut guard, config, current, None) {
            Ok(()) => VaultLockState::Unlocked,
            Err(_) => VaultLockState::Locked,
        })
        .unwrap_or(VaultLockState::Locked)
}

fn check_session_for_monitor(
    config: VaultLockConfig,
    current: Option<SystemInputSnapshot>,
    previous: Option<SystemInputSnapshot>,
) -> Option<LockReason> {
    let mut guard = VAULT_SESSION.lock().ok()?;
    let session = guard.as_ref()?;
    let reason = expired_reason(
        config,
        session.last_activity.elapsed().as_secs(),
        current,
        previous,
    );
    if reason.is_some() {
        hard_lock_session(&mut guard);
    }
    reason
}

fn monitor_once(
    previous: Option<SystemInputSnapshot>,
) -> Result<(Option<SystemInputSnapshot>, Option<LockReason>), String> {
    let unlocked = VAULT_SESSION
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(false);
    if !unlocked {
        return Ok((None, None));
    }

    let conn = db_conn()?;
    let config = load_config(&conn)?;
    let current = try_system_input_snapshot();
    let reason = check_session_for_monitor(config, current, previous);
    Ok((if reason.is_some() { None } else { current }, reason))
}

pub fn start_auto_lock_monitor(app: tauri::AppHandle) {
    static RUNNING: AtomicBool = AtomicBool::new(false);
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    std::thread::spawn(move || {
        let mut previous = None;
        loop {
            std::thread::sleep(Duration::from_secs(30));
            match monitor_once(previous) {
                Ok((_, Some(reason))) => {
                    previous = None;
                    let _ = app.emit(
                        crate::events::EVENT_VAULT_LOCKED,
                        json!({ "reason": reason }),
                    );
                }
                Ok((next, None)) => previous = next,
                Err(error) => eprintln!("vault auto-lock monitor failed: {error}"),
            }
        }
    });
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

fn aes256_encrypt(
    key: &[u8; KEY_LEN],
    iv: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, String> {
    encrypt(Cipher::aes_256_cbc(), key, Some(iv), plaintext)
        .map_err(|e| format!("AES encrypt failed: {e}"))
}

fn aes256_decrypt(
    key: &[u8; KEY_LEN],
    iv: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, String> {
    decrypt(Cipher::aes_256_cbc(), key, Some(iv), ciphertext)
        .map_err(|e| format!("AES decrypt failed: {e}"))
}

fn get_session_key() -> Result<[u8; KEY_LEN], String> {
    let conn = db_conn()?;
    let config = load_config(&conn)?;
    let current = try_system_input_snapshot();
    let mut guard = VAULT_SESSION
        .lock()
        .map_err(|e| format!("session lock: {e}"))?;
    ensure_session_alive(&mut guard, config, current, None)?;

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

    let config = load_config(&conn)?;
    let lock_state = current_lock_state(config, try_system_input_snapshot());

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

    // 初始化后自动解锁
    let mut guard = VAULT_SESSION
        .lock()
        .map_err(|e| format!("session lock: {e}"))?;
    *guard = Some(VaultSession {
        key: Some(key),
        last_activity: Instant::now(),
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
    let decrypted =
        aes256_decrypt(&key, &iv, &encrypted).map_err(|_| "wrong_password".to_string())?;

    if decrypted != CANARY_PLAINTEXT {
        return Err("wrong_password".to_string());
    }

    // 先回填迁移、再建立会话：关闭并发 IPC 在回填中途经 list 读到混合状态的理论窗口
    backfill_plain_fields(&conn, &key);

    let mut guard = VAULT_SESSION
        .lock()
        .map_err(|e| format!("session lock: {e}"))?;
    *guard = Some(VaultSession {
        key: Some(key),
        last_activity: Instant::now(),
    });

    Ok(json!({ "unlocked": true, "lockState": VaultLockState::Unlocked.as_str() }))
}

fn cmd_lock(_payload: &Value) -> Result<Value, String> {
    let mut guard = VAULT_SESSION
        .lock()
        .map_err(|e| format!("session lock: {e}"))?;
    hard_lock_session(&mut guard);
    Ok(json!({ "ok": true, "lockState": VaultLockState::Locked.as_str() }))
}

fn cmd_touch(_payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let config = load_config(&conn)?;
    let current = try_system_input_snapshot();
    let mut guard = VAULT_SESSION
        .lock()
        .map_err(|e| format!("session lock: {e}"))?;
    ensure_session_alive(&mut guard, config, current, None)?;

    match guard.as_mut() {
        Some(session) => {
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

    let old_salt = BASE64
        .decode(&salt_b64)
        .map_err(|e| format!("invalid salt: {e}"))?;
    let old_iv = BASE64
        .decode(&iv_b64)
        .map_err(|e| format!("invalid iv: {e}"))?;
    let old_encrypted = BASE64
        .decode(&encrypted_b64)
        .map_err(|e| format!("invalid data: {e}"))?;
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

    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("begin tx: {e}"))?;

    let re_encrypted_count = entries.len();
    for (id, entry_iv_b64, entry_blob_b64) in &entries {
        let entry_iv = BASE64
            .decode(entry_iv_b64)
            .map_err(|e| format!("entry iv: {e}"))?;
        let entry_blob = BASE64
            .decode(entry_blob_b64)
            .map_err(|e| format!("entry blob: {e}"))?;

        // Decrypt with old key
        let plain = aes256_decrypt(&old_key, &entry_iv, &entry_blob)
            .map_err(|e| format!("decrypt entry {id}: {e}"))?;

        let new_iv = random_bytes(IV_LEN)?;
        match serde_json::from_slice::<Value>(&plain) {
            Ok(fields) if blob_is_legacy(&fields) => {
                // 旧格式：顺手完成拆分迁移（触达路径 2）
                let (secret_fields, plain_fields) = split_fields(&fields);
                let secret_bytes =
                    serde_json::to_vec(&secret_fields).map_err(|e| format!("serialize: {e}"))?;
                let plain_text = serde_json::to_string(&plain_fields)
                    .map_err(|e| format!("serialize plain: {e}"))?;
                let new_blob = aes256_encrypt(&new_key, &new_iv, &secret_bytes)?;
                tx.execute(
                    "UPDATE vault_entries SET iv = ?1, encrypted_blob = ?2, plain_fields = ?3, updated_at = CURRENT_TIMESTAMP WHERE id = ?4",
                    params![BASE64.encode(&new_iv), BASE64.encode(&new_blob), plain_text, id],
                )
                .map_err(|e| format!("update entry {id}: {e}"))?;
            }
            _ => {
                // 新格式（blob 仅含密码）：按现状整体重加密
                let new_blob = aes256_encrypt(&new_key, &new_iv, &plain)?;
                tx.execute(
                    "UPDATE vault_entries SET iv = ?1, encrypted_blob = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?3",
                    params![BASE64.encode(&new_iv), BASE64.encode(&new_blob), id],
                )
                .map_err(|e| format!("update entry {id}: {e}"))?;
            }
        }
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

    // 改密后刷新当前会话，继续保持已解锁状态
    let mut guard = VAULT_SESSION
        .lock()
        .map_err(|e| format!("session lock: {e}"))?;
    *guard = Some(VaultSession {
        key: Some(new_key),
        last_activity: Instant::now(),
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
        "SELECT id, category, title, environment, iv, encrypted_blob, plain_fields, created_at, updated_at FROM vault_entries WHERE 1=1",
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
    sql.push_str(" ORDER BY (view_count + copy_count) DESC, updated_at DESC");

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

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
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
            ))
        })
        .map_err(|e| format!("query: {e}"))?;

    let mut entries: Vec<Value> = Vec::new();
    let mut entry_ids: Vec<i64> = Vec::new();
    for row in rows {
        let (id, cat, title, environment, iv_b64, blob_b64, plain_text, created_at, updated_at) =
            row.map_err(|e| format!("row: {e}"))?;

        // 快路径：明文列直接取账号与摘要，无需解密；未迁移行退回解密路径
        let from_plain = plain_text
            .as_deref()
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .map(|pf| {
                (
                    pf["account"].as_str().unwrap_or("").to_string(),
                    make_summary(&cat, &pf),
                )
            });
        let (account, summary) = match from_plain {
            Some(v) => v,
            None => match (BASE64.decode(&iv_b64), BASE64.decode(&blob_b64)) {
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
            },
        };

        entry_ids.push(id);
        entries.push(json!({
            "id": id,
            "category": cat,
            "title": title,
            "environment": environment,
            "account": account,
            "summary": summary,
            "tags": [],
            "createdAt": created_at,
            "updatedAt": updated_at,
        }));
    }

    let tags_by_entry = get_entry_tags_map(&conn, &entry_ids)?;
    for entry in &mut entries {
        let Some(id) = entry["id"].as_i64() else {
            continue;
        };
        let tags = tags_by_entry.get(&id).cloned().unwrap_or_default();
        if let Some(obj) = entry.as_object_mut() {
            obj.insert("tags".to_string(), json!(tags));
        }
    }
    Ok(json!(entries))
}

fn cmd_get(payload: &Value) -> Result<Value, String> {
    let key = get_session_key()?;
    let id = payload["id"].as_i64().ok_or("id required")?;

    let conn = db_conn()?;
    let (category, title, environment, iv_b64, blob_b64, plain_text, created_at, updated_at): (
        String, String, String, String, String, Option<String>, String, String,
    ) = conn
        .query_row(
            "SELECT category, title, environment, iv, encrypted_blob, plain_fields, created_at, updated_at FROM vault_entries WHERE id = ?1",
            params![id],
            |row| Ok((
                row.get(0)?, row.get(1)?, row.get(2)?,
                row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?,
            )),
        )
        .map_err(|_| "entry not found".to_string())?;

    let iv = BASE64.decode(&iv_b64).map_err(|e| format!("iv: {e}"))?;
    let blob = BASE64.decode(&blob_b64).map_err(|e| format!("blob: {e}"))?;
    let plain = aes256_decrypt(&key, &iv, &blob)?;
    let blob_fields: Value =
        serde_json::from_slice(&plain).map_err(|e| format!("parse fields: {e}"))?;
    let fields = merge_fields(plain_text.as_deref(), &blob_fields);

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
    let category = payload["category"].as_str().ok_or("category required")?;
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

    // Build the fields JSON, split into encrypted (password) and plaintext parts
    let fields = build_fields(category, payload);
    let (secret_fields, plain_fields) = split_fields(&fields);
    let secret_bytes = serde_json::to_vec(&secret_fields).map_err(|e| format!("serialize: {e}"))?;
    let plain_text =
        serde_json::to_string(&plain_fields).map_err(|e| format!("serialize plain: {e}"))?;

    let iv = random_bytes(IV_LEN)?;
    let encrypted = aes256_encrypt(&key, &iv, &secret_bytes)?;

    let conn = db_conn()?;
    conn.execute(
        "INSERT INTO vault_entries (category, title, environment, iv, encrypted_blob, plain_fields) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            category,
            title,
            environment,
            BASE64.encode(&iv),
            BASE64.encode(&encrypted),
            plain_text,
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
    let (secret_fields, plain_fields) = split_fields(&fields);
    let secret_bytes = serde_json::to_vec(&secret_fields).map_err(|e| format!("serialize: {e}"))?;
    let plain_text =
        serde_json::to_string(&plain_fields).map_err(|e| format!("serialize plain: {e}"))?;

    let iv = random_bytes(IV_LEN)?;
    let encrypted = aes256_encrypt(&key, &iv, &secret_bytes)?;

    conn.execute(
        "UPDATE vault_entries SET category = ?1, title = ?2, environment = ?3, iv = ?4, encrypted_blob = ?5, plain_fields = ?6, updated_at = CURRENT_TIMESTAMP WHERE id = ?7",
        params![
            actual_category,
            title,
            environment,
            BASE64.encode(&iv),
            BASE64.encode(&encrypted),
            plain_text,
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
    let old_tag = payload["oldTag"].as_str().ok_or("oldTag required")?.trim();
    let new_tag = payload["newTag"].as_str().ok_or("newTag required")?.trim();

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
    let tag = payload["tag"].as_str().ok_or("tag required")?.trim();

    if tag.is_empty() {
        return Err("tag cannot be empty".to_string());
    }

    let conn = db_conn()?;
    let deleted = conn
        .execute("DELETE FROM vault_entry_tags WHERE tag = ?1", params![tag])
        .map_err(|e| format!("delete tag: {e}"))?;

    Ok(json!({ "deleted": deleted }))
}

fn cmd_record_usage(payload: &Value) -> Result<Value, String> {
    // 免会话：仅递增明文计数列，与 meta_list 免会话口径一致（锁定态复制账号也计数）
    let id = payload["id"].as_i64().ok_or("id required")?;
    let usage_type = payload["type"].as_str().ok_or("type required")?;

    let column = match usage_type {
        "view" => "view_count",
        "copy" => "copy_count",
        _ => return Err("type must be 'view' or 'copy'".to_string()),
    };

    let conn = db_conn()?;
    let sql = format!("UPDATE vault_entries SET {column} = {column} + 1 WHERE id = ?1");
    conn.execute(&sql, params![id])
        .map_err(|e| format!("record_usage: {e}"))?;

    Ok(json!({ "success": true }))
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

// --- Spotlight 即时解锁支持 ---
//
// meta_list: 返回非加密元数据，不要求活跃会话；用于 Spotlight 在锁定状态下也能检索条目
// reveal_one: 用传入主密码单条解密，不修改 VAULT_SESSION；不延长全局会话
// 防爆破：每条目每分钟最多 5 次失败尝试

const REVEAL_ATTEMPT_WINDOW: Duration = Duration::from_secs(60);
const REVEAL_ATTEMPT_MAX: usize = 5;

static REVEAL_ATTEMPTS: Mutex<Option<HashMap<i64, Vec<Instant>>>> = Mutex::new(None);

fn reveal_throttle_check(entry_id: i64) -> Result<(), String> {
    let mut guard = REVEAL_ATTEMPTS
        .lock()
        .map_err(|e| format!("reveal throttle lock: {e}"))?;
    let map = guard.get_or_insert_with(HashMap::new);
    let now = Instant::now();
    let attempts = map.entry(entry_id).or_insert_with(Vec::new);
    attempts.retain(|ts| now.duration_since(*ts) < REVEAL_ATTEMPT_WINDOW);
    if attempts.len() >= REVEAL_ATTEMPT_MAX {
        return Err("too_many_attempts".to_string());
    }
    Ok(())
}

fn reveal_throttle_record_failure(entry_id: i64) {
    if let Ok(mut guard) = REVEAL_ATTEMPTS.lock() {
        let map = guard.get_or_insert_with(HashMap::new);
        let attempts = map.entry(entry_id).or_insert_with(Vec::new);
        attempts.push(Instant::now());
    }
}

fn reveal_throttle_clear(entry_id: i64) {
    if let Ok(mut guard) = REVEAL_ATTEMPTS.lock() {
        if let Some(map) = guard.as_mut() {
            map.remove(&entry_id);
        }
    }
}

fn cmd_meta_list(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let category = payload["category"].as_str().unwrap_or("");
    let keyword = payload["keyword"].as_str().unwrap_or("");

    let mut sql = String::from(
        "SELECT id, category, title, environment, view_count, copy_count, plain_fields, created_at, updated_at \
         FROM vault_entries WHERE 1=1",
    );
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if !category.is_empty() {
        sql.push_str(" AND category = ?");
        param_values.push(Box::new(category.to_string()));
    }
    if !keyword.is_empty() {
        sql.push_str(" AND (title LIKE ? OR IFNULL(plain_fields, '') LIKE ?)");
        param_values.push(Box::new(format!("%{keyword}%")));
        param_values.push(Box::new(format!("%{keyword}%")));
    }
    sql.push_str(" ORDER BY (view_count + copy_count) DESC, updated_at DESC");

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql).map_err(|e| format!("prepare: {e}"))?;
    let rows = stmt
        .query_map(params_refs.as_slice(), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
            ))
        })
        .map_err(|e| format!("query: {e}"))?;

    let mut entries: Vec<Value> = Vec::new();
    let mut entry_ids: Vec<i64> = Vec::new();
    for row in rows {
        let (
            id,
            cat,
            title,
            environment,
            view_count,
            copy_count,
            plain_text,
            created_at,
            updated_at,
        ) = row.map_err(|e| format!("row: {e}"))?;

        // 解析失败或未迁移行统一返回 null，前端按现状行为退化
        let plain_fields = plain_text
            .as_deref()
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .unwrap_or(Value::Null);

        entry_ids.push(id);
        entries.push(json!({
            "id": id,
            "category": cat,
            "title": title,
            "environment": environment,
            "viewCount": view_count,
            "copyCount": copy_count,
            "plainFields": plain_fields,
            "tags": [],
            "createdAt": created_at,
            "updatedAt": updated_at,
        }));
    }

    let tags_by_entry = get_entry_tags_map(&conn, &entry_ids)?;
    for entry in &mut entries {
        let Some(id) = entry["id"].as_i64() else {
            continue;
        };
        let tags = tags_by_entry.get(&id).cloned().unwrap_or_default();
        if let Some(obj) = entry.as_object_mut() {
            obj.insert("tags".to_string(), json!(tags));
        }
    }
    Ok(json!(entries))
}

fn cmd_reveal_one(payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id required")?;
    let password = payload["masterPassword"]
        .as_str()
        .ok_or("masterPassword required")?;

    reveal_throttle_check(id)?;

    let conn = db_conn()?;

    // Verify master password against canary first
    let (canary_salt_b64, canary_iv_b64, canary_blob_b64): (String, String, String) = conn
        .query_row(
            "SELECT salt, iv, encrypted FROM vault_canary WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| "vault not initialized".to_string())?;

    let canary_salt = BASE64
        .decode(&canary_salt_b64)
        .map_err(|e| format!("invalid salt: {e}"))?;
    let canary_iv = BASE64
        .decode(&canary_iv_b64)
        .map_err(|e| format!("invalid iv: {e}"))?;
    let canary_blob = BASE64
        .decode(&canary_blob_b64)
        .map_err(|e| format!("invalid encrypted data: {e}"))?;

    let mut key = derive_key(password, &canary_salt)?;
    let canary_check = aes256_decrypt(&key, &canary_iv, &canary_blob);
    let valid = match canary_check {
        Ok(plain) => plain == CANARY_PLAINTEXT,
        Err(_) => false,
    };
    if !valid {
        key.zeroize();
        reveal_throttle_record_failure(id);
        return Err("bad_master_password".to_string());
    }

    // Decrypt the requested entry
    let row = conn
        .query_row(
            "SELECT category, title, environment, iv, encrypted_blob, plain_fields, created_at, updated_at \
             FROM vault_entries WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            },
        );
    let (category, title, environment, iv_b64, blob_b64, plain_text, created_at, updated_at) =
        match row {
            Ok(r) => r,
            Err(_) => {
                key.zeroize();
                return Err("entry not found".to_string());
            }
        };

    let iv = BASE64.decode(&iv_b64).map_err(|e| format!("iv: {e}"))?;
    let blob = BASE64.decode(&blob_b64).map_err(|e| format!("blob: {e}"))?;
    let plain_result = aes256_decrypt(&key, &iv, &blob);
    key.zeroize();
    let plain = plain_result.map_err(|e| format!("decrypt entry: {e}"))?;
    let blob_fields: Value =
        serde_json::from_slice(&plain).map_err(|e| format!("parse fields: {e}"))?;
    let fields = merge_fields(plain_text.as_deref(), &blob_fields);

    let tags = get_entry_tags(&conn, id).unwrap_or_default();

    reveal_throttle_clear(id);

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

// --- 仅密码加密的存储模型 ---
//
// encrypted_blob 解密后仅含 {"password": ...}（新格式），其余字段明文存于 plain_fields 列。
// 旧格式（迁移前/降级期间旧版写回）的 blob 含全部字段，由 blob_is_legacy 判定，
// merge_fields 对旧格式直接以 blob 为准，避免陈旧明文键污染。

const SECRET_FIELD_KEY: &str = "password";

/// 完整字段 JSON -> (加密部分, 明文部分)：加密部分固定只含 password，其余键归明文部分。
fn split_fields(fields: &Value) -> (Value, Value) {
    let mut secret = serde_json::Map::new();
    let mut plain = serde_json::Map::new();
    if let Some(obj) = fields.as_object() {
        for (k, v) in obj {
            if k == SECRET_FIELD_KEY {
                secret.insert(k.clone(), v.clone());
            } else {
                plain.insert(k.clone(), v.clone());
            }
        }
    }
    (Value::Object(secret), Value::Object(plain))
}

/// blob 是否旧格式（含 password 以外的键）。
fn blob_is_legacy(blob_fields: &Value) -> bool {
    blob_fields
        .as_object()
        .map(|obj| obj.keys().any(|k| k != SECRET_FIELD_KEY))
        .unwrap_or(false)
}

/// 明文列 + 解密后的 blob -> 完整字段 JSON。
/// 旧格式以 blob 为准（忽略 plain_fields）；
/// 新格式以 plain_fields 为底（NULL/解析失败视为 {}），加上 blob 中的 password。
fn merge_fields(plain_fields_text: Option<&str>, blob_fields: &Value) -> Value {
    if blob_is_legacy(blob_fields) {
        return blob_fields.clone();
    }
    let mut merged = plain_fields_text
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    if let Some(obj) = blob_fields.as_object() {
        for (k, v) in obj {
            merged.insert(k.clone(), v.clone());
        }
    }
    Value::Object(merged)
}

/// 存量迁移：扫描全部条目，旧格式行（blob 含非密码键）拆分为「仅密码加密 + 明文列」。
/// 判定条件同时覆盖首次迁移与降级期间旧版编辑产生的陈旧状态；新格式行跳过，天然幂等。
/// 单行失败仅记录日志跳过（下次解锁重试），整体不阻断解锁；不触碰 updated_at 以免扰动排序。
fn backfill_plain_fields(conn: &Connection, key: &[u8; KEY_LEN]) {
    let rows: Vec<(i64, String, String)> = {
        let Ok(mut stmt) = conn.prepare("SELECT id, iv, encrypted_blob FROM vault_entries") else {
            return;
        };
        let Ok(mapped) = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        else {
            return;
        };
        mapped.filter_map(|r| r.ok()).collect()
    };

    for (id, iv_b64, blob_b64) in rows {
        let result = (|| -> Result<(), String> {
            let iv = BASE64.decode(&iv_b64).map_err(|e| format!("iv: {e}"))?;
            let blob = BASE64.decode(&blob_b64).map_err(|e| format!("blob: {e}"))?;
            let plain = aes256_decrypt(key, &iv, &blob)?;
            let fields: Value =
                serde_json::from_slice(&plain).map_err(|e| format!("parse fields: {e}"))?;
            if !blob_is_legacy(&fields) {
                return Ok(());
            }
            let (secret_fields, plain_fields) = split_fields(&fields);
            let secret_bytes =
                serde_json::to_vec(&secret_fields).map_err(|e| format!("serialize: {e}"))?;
            let plain_text =
                serde_json::to_string(&plain_fields).map_err(|e| format!("serialize plain: {e}"))?;
            let new_iv = random_bytes(IV_LEN)?;
            let new_blob = aes256_encrypt(key, &new_iv, &secret_bytes)?;
            conn.execute(
                "UPDATE vault_entries SET iv = ?1, encrypted_blob = ?2, plain_fields = ?3 WHERE id = ?4",
                params![BASE64.encode(&new_iv), BASE64.encode(&new_blob), plain_text, id],
            )
            .map_err(|e| format!("update: {e}"))?;
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("[vault] plain_fields backfill skipped entry {id}: {e}");
        }
    }
}

const ACTIONS: &[&str] = &[
    "status",
    "setup",
    "unlock",
    "touch",
    "lock",
    "change_password",
    "list",
    "meta_list",
    "get",
    "reveal_one",
    "create",
    "update",
    "delete",
    "open_url",
    "tag_stats",
    "rename_tag",
    "delete_tag",
    "record_usage",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported vault action: {action}"));
    }
    match action {
        "status" => cmd_status(payload),
        "setup" => cmd_setup(payload),
        "unlock" => cmd_unlock(payload),
        "touch" => cmd_touch(payload),
        "lock" => cmd_lock(payload),
        "change_password" => cmd_change_password(payload),
        "list" => cmd_list(payload),
        "meta_list" => cmd_meta_list(payload),
        "get" => cmd_get(payload),
        "reveal_one" => cmd_reveal_one(payload),
        "create" => cmd_create(payload),
        "update" => cmd_update(payload),
        "delete" => cmd_delete(payload),
        "open_url" => cmd_open_url(payload),
        "tag_stats" => cmd_tag_stats(payload),
        "rename_tag" => cmd_rename_tag(payload),
        "delete_tag" => cmd_delete_tag(payload),
        "record_usage" => cmd_record_usage(payload),
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
    use crate::tools::vault_lock::{LockReason, VaultLockConfig};
    use crate::tools::widget::guards::SystemInputSnapshot;
    use serde_json::json;
    use std::time::Duration;

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
        let p =
            json!({ "url": "https://x.com", "account": "admin", "password": "123", "notes": "n" });
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
    fn test_hard_lock_session_clears_key_and_guard() {
        let mut guard = Some(VaultSession {
            key: Some([7u8; KEY_LEN]),
            last_activity: Instant::now(),
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
        });
        let config = VaultLockConfig {
            activity_enabled: true,
            activity_after_secs: 15,
            system_idle_enabled: false,
            system_idle_after_secs: 900,
        };

        let err = ensure_session_alive(&mut guard, config, None, None)
            .expect_err("session should expire");

        assert_eq!(err, "vault_locked_timeout");
        assert!(guard.is_none());
    }

    #[test]
    fn test_system_idle_expiry_clears_session_key() {
        let mut guard = Some(VaultSession {
            key: Some([7u8; KEY_LEN]),
            last_activity: Instant::now(),
        });
        let config = VaultLockConfig {
            activity_enabled: false,
            activity_after_secs: 1_800,
            system_idle_enabled: true,
            system_idle_after_secs: 900,
        };
        let current = SystemInputSnapshot {
            last_input_tick_ms: 10,
            idle_secs: 900,
        };

        let error = ensure_session_alive(&mut guard, config, Some(current), None)
            .expect_err("system idle must lock");

        assert_eq!(error, "vault_locked_timeout");
        assert!(guard.is_none());
    }

    #[test]
    fn test_monitor_locks_once_after_input_resets() {
        *VAULT_SESSION.lock().expect("session lock") = Some(VaultSession {
            key: Some([9u8; KEY_LEN]),
            last_activity: Instant::now(),
        });
        let config = VaultLockConfig {
            activity_enabled: false,
            activity_after_secs: 1_800,
            system_idle_enabled: true,
            system_idle_after_secs: 900,
        };
        let previous = SystemInputSnapshot {
            last_input_tick_ms: 1_000,
            idle_secs: 870,
        };
        let current = SystemInputSnapshot {
            last_input_tick_ms: 901_000,
            idle_secs: 1,
        };

        assert_eq!(
            check_session_for_monitor(config, Some(current), Some(previous)),
            Some(LockReason::SystemIdle)
        );
        assert!(VAULT_SESSION.lock().expect("session lock").is_none());
        assert_eq!(
            check_session_for_monitor(config, Some(current), None),
            None
        );
    }

    #[test]
    fn test_reveal_throttle_blocks_after_max_failures() {
        // 用一个不太可能与真实条目冲突的 id
        let entry_id: i64 = -424242;
        reveal_throttle_clear(entry_id);

        for _ in 0..REVEAL_ATTEMPT_MAX {
            assert!(reveal_throttle_check(entry_id).is_ok());
            reveal_throttle_record_failure(entry_id);
        }

        // 第 N+1 次应被拒
        let err = reveal_throttle_check(entry_id).expect_err("should throttle");
        assert_eq!(err, "too_many_attempts");

        reveal_throttle_clear(entry_id);
    }

    #[test]
    fn test_reveal_throttle_clears_on_success() {
        let entry_id: i64 = -424243;
        reveal_throttle_clear(entry_id);

        reveal_throttle_record_failure(entry_id);
        reveal_throttle_record_failure(entry_id);
        reveal_throttle_clear(entry_id);

        // 清理后又能尝试
        assert!(reveal_throttle_check(entry_id).is_ok());
    }

    #[test]
    fn test_split_fields_app() {
        let fields = build_fields(
            "app",
            &json!({ "url": "https://x.com", "account": "admin", "password": "123", "notes": "n" }),
        );
        let (secret, plain) = split_fields(&fields);
        assert_eq!(secret, json!({ "password": "123" }));
        assert_eq!(plain["url"], "https://x.com");
        assert_eq!(plain["account"], "admin");
        assert_eq!(plain["notes"], "n");
        assert!(plain.get("password").is_none());
    }

    #[test]
    fn test_split_fields_server() {
        let fields = build_fields(
            "server",
            &json!({ "address": "10.0.0.1", "serverType": "Windows", "account": "root", "password": "p" }),
        );
        let (secret, plain) = split_fields(&fields);
        assert_eq!(secret, json!({ "password": "p" }));
        assert_eq!(plain["address"], "10.0.0.1");
        assert_eq!(plain["serverType"], "Windows");
        assert!(plain.get("password").is_none());
    }

    #[test]
    fn test_split_fields_database() {
        let fields = build_fields(
            "database",
            &json!({ "dbType": "PostgreSQL", "address": "localhost", "port": 5432, "account": "pg", "password": "p", "dbName": "mydb" }),
        );
        let (secret, plain) = split_fields(&fields);
        assert_eq!(secret, json!({ "password": "p" }));
        assert_eq!(plain["port"], 5432);
        assert_eq!(plain["dbName"], "mydb");
        assert!(plain.get("password").is_none());
    }

    #[test]
    fn test_split_fields_empty_password() {
        let fields = json!({ "account": "a", "password": "" });
        let (secret, plain) = split_fields(&fields);
        assert_eq!(secret, json!({ "password": "" }));
        assert_eq!(plain, json!({ "account": "a" }));
    }

    #[test]
    fn test_merge_fields_new_format() {
        let plain_text = r#"{"url":"https://x.com","account":"admin","notes":"n"}"#;
        let blob = json!({ "password": "123" });
        let merged = merge_fields(Some(plain_text), &blob);
        assert_eq!(merged["url"], "https://x.com");
        assert_eq!(merged["account"], "admin");
        assert_eq!(merged["notes"], "n");
        assert_eq!(merged["password"], "123");
    }

    #[test]
    fn test_merge_fields_legacy_format() {
        let blob = json!({ "url": "https://x.com", "account": "admin", "password": "123", "notes": "n" });
        let merged = merge_fields(None, &blob);
        assert_eq!(merged, blob);
    }

    #[test]
    fn test_merge_fields_stale_plain() {
        // 降级期旧版编辑：blob 为完整字段（可能已变更分类），plain_fields 残留陈旧键
        let stale_plain = r#"{"dbType":"MySQL","address":"old-host","account":"old"}"#;
        let blob = json!({ "url": "https://new.com", "account": "new", "password": "p", "notes": "" });
        let merged = merge_fields(Some(stale_plain), &blob);
        assert_eq!(merged, blob);
        assert!(merged.get("dbType").is_none());
        assert!(merged.get("address").is_none());
    }

    #[test]
    fn test_merge_fields_invalid_plain_text() {
        let blob = json!({ "password": "123" });
        let merged = merge_fields(Some("not-json{{"), &blob);
        assert_eq!(merged, json!({ "password": "123" }));
    }

    #[test]
    fn test_blob_is_legacy() {
        assert!(blob_is_legacy(&json!({ "password": "p", "account": "a" })));
        assert!(!blob_is_legacy(&json!({ "password": "p" })));
        assert!(!blob_is_legacy(&json!({})));
    }
}

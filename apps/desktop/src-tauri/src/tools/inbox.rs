use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use blake3::Hasher;
use chrono::{DateTime, Duration as ChronoDuration, NaiveDateTime, Utc};
use image::{imageops::FilterType, DynamicImage, ImageFormat, RgbaImage};
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Command;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use walkdir::WalkDir;

use super::helpers::{db_conn, get_data_dir};

const INLINE_TEXT_LIMIT: usize = 256 * 1024;
const EXTERNAL_TEXT_LIMIT: usize = 8 * 1024 * 1024;
const METADATA_EXCERPT_LIMIT: usize = 2048;
const IMAGE_KEEP_LIMIT_BYTES: usize = 10 * 1024 * 1024;
const IMAGE_KEEP_LIMIT_PIXELS: u64 = 12_000_000;
const THUMBNAIL_LONG_EDGE: u32 = 320;
const DUPLICATE_WINDOW_SECS: i64 = 30;
const DEFAULT_RETENTION_DAYS: i64 = 14;
const HISTORY_ITEM_LIMIT: i64 = 10_000;
const HISTORY_ASSET_LIMIT_BYTES: i64 = 1024 * 1024 * 1024;
const DEFAULT_PAUSE_MINUTES: i64 = 5;

const KEY_CAPTURE_CONSENT_ACK: &str = "inbox_capture_consent_ack";
const KEY_CAPTURE_ENABLED: &str = "inbox_capture_enabled";
const KEY_CAPTURE_WHEN_HIDDEN: &str = "inbox_capture_when_hidden";
const KEY_CAPTURE_PAUSED_UNTIL: &str = "inbox_capture_paused_until";
const KEY_HISTORY_RETENTION_DAYS: &str = "inbox_history_retention_days";

static HTML_TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").expect("html regex"));
static HTML_WS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+").expect("html whitespace regex"));
static RTF_CONTROL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\[a-zA-Z]+\d* ?").expect("rtf control regex"));
static RTF_HEX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\'[0-9a-fA-F]{2}").expect("rtf hex regex"));

#[derive(Debug, Clone)]
struct SuppressedClipboard {
    content_hash: String,
    expires_at: Instant,
}

static SUPPRESSED_CLIPBOARD: LazyLock<Arc<Mutex<Vec<SuppressedClipboard>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));

#[derive(Debug, Clone)]
struct InboxListQuery {
    bucket: Option<String>,
    item_type: Option<String>,
    starred_only: bool,
    external_only: bool,
    summary_only: bool,
    keyword: String,
    limit: i64,
    offset: i64,
}

#[derive(Debug, Clone)]
struct CaptureGateSettings {
    consent_ack: bool,
    capture_enabled: bool,
    capture_when_hidden: bool,
    history_retention_days: i64,
    paused_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
struct CapturedFileRef {
    file_path: String,
    file_name: String,
    file_size: Option<i64>,
    modified_at: Option<String>,
}

#[derive(Debug, Clone)]
struct CaptureCandidate {
    item_type: &'static str,
    storage_kind: &'static str,
    title: String,
    preview: String,
    search_text: String,
    payload_text: Option<String>,
    external_bytes: Option<Vec<u8>>,
    thumb_bytes: Option<Vec<u8>>,
    byte_size: i64,
    content_hash: String,
    meta_json: Value,
    file_refs: Vec<CapturedFileRef>,
}

type SqlParam = Box<dyn rusqlite::ToSql>;

const ACTIONS: &[&str] = &[
    "list",
    "get",
    "search",
    "promote",
    "update_meta",
    "archive",
    "delete",
    "cleanup",
    "capture_status",
    "capture_pause",
    "open_path",
    "copy_image",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported inbox action: {action}"));
    }
    match action {
        "list" => action_list(payload),
        "get" => action_get(payload),
        "search" => action_search(payload),
        "promote" => action_promote(payload),
        "update_meta" => action_update_meta(payload),
        "archive" => action_archive(payload),
        "delete" => action_delete(payload),
        "cleanup" => action_cleanup(),
        "capture_status" => action_capture_status(),
        "capture_pause" => action_capture_pause(payload),
        "open_path" => action_open_path(payload),
        "copy_image" => action_copy_image(payload),
        _ => Err(format!("unsupported inbox action: {action}")),
    }
}

pub fn suppress_clipboard_capture(content: &str) -> Result<(), String> {
    let raw_hash = hash_bytes(content.as_bytes());
    let normalized_hash = hash_bytes(normalize_text_for_hash(content).as_bytes());
    let mut hashes = vec![raw_hash];
    if normalized_hash != hashes[0] {
        hashes.push(normalized_hash);
    }
    suppress_clipboard_hashes(hashes)
}

fn suppress_clipboard_hash(content_hash: &str) -> Result<(), String> {
    suppress_clipboard_hashes(std::iter::once(content_hash.to_string()))
}

fn suppress_clipboard_hashes<I>(content_hashes: I) -> Result<(), String>
where
    I: IntoIterator<Item = String>,
{
    let now = Instant::now();
    let expires_at = now + Duration::from_secs(10);
    let mut list = SUPPRESSED_CLIPBOARD
        .lock()
        .map_err(|e| format!("suppress clipboard lock failed: {e}"))?;
    list.retain(|item| item.expires_at > now);

    let mut seen = HashSet::new();
    for content_hash in content_hashes {
        if content_hash.is_empty() || !seen.insert(content_hash.clone()) {
            continue;
        }
        if list.len() >= 100 {
            list.remove(0);
        }
        list.push(SuppressedClipboard {
            content_hash,
            expires_at,
        });
    }
    Ok(())
}

pub fn process_clipboard_change(window_visible: bool) -> Result<bool, String> {
    let conn = db_conn()?;
    let gate = load_capture_gate_settings(&conn)?;
    if !gate.consent_ack || !gate.capture_enabled {
        return Ok(false);
    }
    if !gate.capture_when_hidden && !window_visible {
        return Ok(false);
    }
    if gate
        .paused_until
        .as_ref()
        .is_some_and(|paused_until| *paused_until > Utc::now())
    {
        return Ok(false);
    }

    #[cfg(windows)]
    {
        let Some(candidate) = read_clipboard_candidate()? else {
            return Ok(false);
        };
        if should_suppress_capture_hash(&candidate.content_hash) {
            return Ok(false);
        }
        upsert_capture_candidate(&conn, candidate)?;
        cleanup_history_if_needed(&conn, gate.history_retention_days)?;
        return Ok(true);
    }

    #[cfg(not(windows))]
    {
        let _ = window_visible;
        Ok(false)
    }
}

fn action_list(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let query = parse_list_query(payload, false)?;
    list_items(&conn, &query)
}

fn action_search(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let query = parse_list_query(payload, true)?;
    list_items(&conn, &query)
}

fn action_get(payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;
    let item = conn
        .query_row(
            "SELECT id, bucket, item_type, storage_kind, title, preview, search_text, payload_ref,
                    byte_size, content_hash, captured_at, last_seen_at, seen_count, note, starred, meta_json
             FROM inbox_items
             WHERE id = ?1",
            params![id],
            row_to_detail,
        )
        .optional()
        .map_err(|e| format!("query inbox detail failed: {e}"))?
        .ok_or("收纳箱条目不存在")?;
    Ok(item)
}

fn action_promote(payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;
    conn.execute(
        "UPDATE inbox_items
         SET bucket = 'inbox'
         WHERE id = ?1 AND bucket = 'history'",
        params![id],
    )
    .map_err(|e| format!("promote inbox item failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn action_update_meta(payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let mut sets = Vec::new();
    let mut params_list: Vec<SqlParam> = Vec::new();
    if let Some(title) = payload["title"].as_str() {
        sets.push("title = ?");
        params_list.push(Box::new(title.trim().to_string()));
    }
    if let Some(note) = payload["note"].as_str() {
        sets.push("note = ?");
        params_list.push(Box::new(note.to_string()));
    }
    if payload.get("starred").is_some() {
        sets.push("starred = ?");
        params_list.push(Box::new(bool_to_i64(
            payload["starred"].as_bool().unwrap_or(false),
        )));
    }
    if sets.is_empty() {
        return Err("未提供需要更新的字段".to_string());
    }
    let mut sql = String::from("UPDATE inbox_items SET ");
    sql.push_str(&sets.join(", "));
    sql.push_str(" WHERE id = ?");
    params_list.push(Box::new(id));
    let conn = db_conn()?;
    let params_refs: Vec<&dyn rusqlite::ToSql> =
        params_list.iter().map(|item| item.as_ref()).collect();
    conn.execute(&sql, params_refs.as_slice())
        .map_err(|e| format!("update inbox meta failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn action_archive(payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let archived = payload["archived"].as_bool().unwrap_or(true);
    let next_bucket = if archived { "archived" } else { "inbox" };
    let conn = db_conn()?;
    conn.execute(
        "UPDATE inbox_items SET bucket = ?1 WHERE id = ?2",
        params![next_bucket, id],
    )
    .map_err(|e| format!("archive inbox item failed: {e}"))?;
    Ok(json!({ "ok": true, "bucket": next_bucket }))
}

fn action_delete(payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let mut conn = db_conn()?;
    delete_inbox_item(&mut conn, id)?;
    Ok(json!({ "ok": true }))
}

fn action_cleanup() -> Result<Value, String> {
    let conn = db_conn()?;
    let settings = load_capture_gate_settings(&conn)?;
    cleanup_history_if_needed(&conn, settings.history_retention_days)?;
    Ok(json!({ "ok": true }))
}

fn action_capture_status() -> Result<Value, String> {
    let conn = db_conn()?;
    build_capture_status(&conn)
}

fn action_capture_pause(payload: &Value) -> Result<Value, String> {
    let minutes = payload["minutes"].as_i64().unwrap_or(DEFAULT_PAUSE_MINUTES);
    let conn = db_conn()?;
    if minutes <= 0 {
        save_setting(&conn, KEY_CAPTURE_PAUSED_UNTIL, "")?;
    } else {
        let paused_until = (Utc::now() + ChronoDuration::minutes(minutes))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        save_setting(&conn, KEY_CAPTURE_PAUSED_UNTIL, &paused_until)?;
    }
    build_capture_status(&conn)
}

fn action_open_path(payload: &Value) -> Result<Value, String> {
    let path = payload["path"].as_str().ok_or("path is required")?;
    let reveal = payload["reveal"].as_bool().unwrap_or(false);
    let raw_path = PathBuf::from(path);
    if !raw_path.exists() {
        return Err("目标路径不存在".to_string());
    }

    #[cfg(windows)]
    if reveal && raw_path.is_file() {
        let explorer_arg = format!("/select,{}", raw_path.to_string_lossy());
        Command::new("explorer.exe")
            .arg(explorer_arg)
            .spawn()
            .map_err(|e| format!("open path failed: {e}"))?;
        return Ok(json!({ "ok": true }));
    }

    let target = if reveal && raw_path.is_file() {
        raw_path.parent().map(Path::to_path_buf).unwrap_or(raw_path)
    } else {
        raw_path
    };
    open::that(&target).map_err(|e| format!("open path failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn action_copy_image(payload: &Value) -> Result<Value, String> {
    let path = payload["path"].as_str().ok_or("path is required")?;
    let image_path = PathBuf::from(path);
    if !image_path.exists() {
        return Err("图像文件不存在".to_string());
    }
    if !image_path.is_file() {
        return Err("图像路径必须是文件".to_string());
    }
    copy_image_file_to_clipboard(&image_path)?;
    Ok(json!({ "ok": true }))
}

fn parse_list_query(payload: &Value, require_keyword: bool) -> Result<InboxListQuery, String> {
    let keyword = payload["keyword"].as_str().unwrap_or("").trim().to_string();
    if require_keyword && keyword.is_empty() {
        return Err("keyword is required".to_string());
    }
    let bucket = match payload["bucket"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some("all") | None => None,
        Some(value @ ("history" | "inbox" | "archived")) => Some(value.to_string()),
        Some(_) => return Err("invalid bucket".to_string()),
    };
    let item_type = match payload["itemType"]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value @ ("text" | "html" | "rtf" | "image" | "file" | "unknown")) => {
            Some(value.to_string())
        }
        Some(_) => return Err("invalid itemType".to_string()),
        None => None,
    };
    let limit = payload["limit"].as_i64().unwrap_or(50).clamp(1, 100);
    let offset = payload["offset"].as_i64().unwrap_or(0).max(0);
    Ok(InboxListQuery {
        bucket,
        item_type,
        starred_only: payload["starredOnly"].as_bool().unwrap_or(false),
        external_only: payload["externalOnly"].as_bool().unwrap_or(false),
        summary_only: payload["summaryOnly"].as_bool().unwrap_or(false),
        keyword,
        limit,
        offset,
    })
}

fn list_items(conn: &Connection, query: &InboxListQuery) -> Result<Value, String> {
    let (where_sql, params_list) = build_item_where(conn, query, true, true);
    let total = query_total_count(conn, &where_sql, &params_list)?;
    let facets = query_facets(conn, query)?;

    let mut sql = String::from(
        "SELECT id, bucket, item_type, storage_kind, title, preview, byte_size,
                captured_at, last_seen_at, seen_count, starred, note, meta_json
         FROM inbox_items",
    );
    if !where_sql.is_empty() {
        sql.push(' ');
        sql.push_str(&where_sql);
    }
    sql.push_str(" ORDER BY last_seen_at DESC, id DESC LIMIT ? OFFSET ?");

    let mut paged_params = params_list;
    paged_params.push(Box::new(query.limit));
    paged_params.push(Box::new(query.offset));
    let params_refs: Vec<&dyn rusqlite::ToSql> =
        paged_params.iter().map(|value| value.as_ref()).collect();

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("prepare inbox list failed: {e}"))?;
    let rows = stmt
        .query_map(params_refs.as_slice(), row_to_summary)
        .map_err(|e| format!("query inbox list failed: {e}"))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| e.to_string())?);
    }

    Ok(json!({
        "items": items,
        "total": total,
        "hasMore": query.offset + query.limit < total,
        "nextOffset": query.offset + query.limit,
        "facets": facets,
    }))
}

fn build_capture_status(conn: &Connection) -> Result<Value, String> {
    let settings = load_capture_gate_settings(conn)?;
    let paused = settings
        .paused_until
        .as_ref()
        .is_some_and(|paused_until| *paused_until > Utc::now());
    Ok(json!({
        "monitorRunning": true,
        "consentAck": settings.consent_ack,
        "captureEnabled": settings.capture_enabled,
        "captureWhenHidden": settings.capture_when_hidden,
        "historyRetentionDays": settings.history_retention_days,
        "paused": paused,
        "pausedUntil": settings.paused_until.map(|value| value.to_rfc3339()),
    }))
}

fn load_capture_gate_settings(conn: &Connection) -> Result<CaptureGateSettings, String> {
    let consent_ack = read_setting_bool(conn, KEY_CAPTURE_CONSENT_ACK, false)?;
    let capture_enabled = read_setting_bool(conn, KEY_CAPTURE_ENABLED, false)?;
    let capture_when_hidden = read_setting_bool(conn, KEY_CAPTURE_WHEN_HIDDEN, true)?;
    let history_retention_days =
        read_setting_i64(conn, KEY_HISTORY_RETENTION_DAYS, DEFAULT_RETENTION_DAYS)?;
    let paused_until = read_setting_string(conn, KEY_CAPTURE_PAUSED_UNTIL)?
        .and_then(|value| parse_datetime(&value));
    Ok(CaptureGateSettings {
        consent_ack,
        capture_enabled,
        capture_when_hidden,
        history_retention_days,
        paused_until,
    })
}

fn read_setting_string(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT value FROM user_settings WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| format!("read inbox setting '{key}' failed: {e}"))
}

fn read_setting_bool(conn: &Connection, key: &str, default_value: bool) -> Result<bool, String> {
    Ok(read_setting_string(conn, key)?
        .map(|value| value == "true")
        .unwrap_or(default_value))
}

fn read_setting_i64(conn: &Connection, key: &str, default_value: i64) -> Result<i64, String> {
    Ok(read_setting_string(conn, key)?
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(default_value))
}

fn save_setting(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO user_settings(key, value, updated_at) VALUES(?1, ?2, CURRENT_TIMESTAMP)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=CURRENT_TIMESTAMP",
        params![key, value],
    )
    .map_err(|e| format!("save inbox setting failed: {e}"))?;
    Ok(())
}

fn build_item_where(
    conn: &Connection,
    query: &InboxListQuery,
    include_bucket: bool,
    include_item_type: bool,
) -> (String, Vec<SqlParam>) {
    let mut conditions = Vec::new();
    let mut params_list: Vec<SqlParam> = Vec::new();

    if include_bucket {
        if let Some(bucket) = query.bucket.as_deref() {
            conditions.push("bucket = ?".to_string());
            params_list.push(Box::new(bucket.to_string()));
        }
    }

    if include_item_type {
        if let Some(item_type) = query.item_type.as_deref() {
            conditions.push("item_type = ?".to_string());
            params_list.push(Box::new(item_type.to_string()));
        }
    }

    if query.starred_only {
        conditions.push("starred = 1".to_string());
    }
    if query.external_only {
        conditions.push("storage_kind = 'external'".to_string());
    }
    if query.summary_only {
        conditions.push("storage_kind = 'metadata_only'".to_string());
    }
    if !query.keyword.is_empty() {
        if inbox_has_fts(conn) {
            conditions
                .push("id IN (SELECT rowid FROM inbox_fts WHERE inbox_fts MATCH ?)".to_string());
            params_list.push(Box::new(build_fts_keyword(&query.keyword)));
        } else {
            conditions.push(
                "(title LIKE ? OR preview LIKE ? OR note LIKE ? OR search_text LIKE ?)".to_string(),
            );
            let keyword = format!("%{}%", query.keyword);
            for _ in 0..4 {
                params_list.push(Box::new(keyword.clone()));
            }
        }
    }

    if conditions.is_empty() {
        (String::new(), params_list)
    } else {
        (format!("WHERE {}", conditions.join(" AND ")), params_list)
    }
}

fn build_fts_keyword(keyword: &str) -> String {
    keyword
        .split_whitespace()
        .filter(|part| !part.trim().is_empty())
        .map(|part| format!("\"{}\"", part.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ")
}

fn query_total_count(
    conn: &Connection,
    where_sql: &str,
    params_list: &[SqlParam],
) -> Result<i64, String> {
    let mut sql = String::from("SELECT COUNT(*) FROM inbox_items");
    if !where_sql.is_empty() {
        sql.push(' ');
        sql.push_str(where_sql);
    }
    let params_refs: Vec<&dyn rusqlite::ToSql> =
        params_list.iter().map(|value| value.as_ref()).collect();
    conn.query_row(&sql, params_refs.as_slice(), |row| row.get::<_, i64>(0))
        .map_err(|e| format!("count inbox items failed: {e}"))
}

fn query_facets(conn: &Connection, query: &InboxListQuery) -> Result<Value, String> {
    let (base_where, base_params) = build_item_where(conn, query, false, false);
    let base_param_refs: Vec<&dyn rusqlite::ToSql> =
        base_params.iter().map(|value| value.as_ref()).collect();

    let mut bucket_stmt = String::from("SELECT bucket, COUNT(*) FROM inbox_items");
    if !base_where.is_empty() {
        bucket_stmt.push(' ');
        bucket_stmt.push_str(&base_where);
    }
    bucket_stmt.push_str(" GROUP BY bucket");

    let mut stmt = conn
        .prepare(&bucket_stmt)
        .map_err(|e| format!("prepare inbox bucket facets failed: {e}"))?;
    let rows = stmt
        .query_map(base_param_refs.as_slice(), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|e| format!("query inbox bucket facets failed: {e}"))?;

    let mut bucket_counts = json!({
        "history": 0,
        "inbox": 0,
        "archived": 0,
    });
    for row in rows {
        let (bucket, count) = row.map_err(|e| e.to_string())?;
        bucket_counts[&bucket] = json!(count);
    }

    let mut type_stmt = String::from("SELECT item_type, COUNT(*) FROM inbox_items");
    if !base_where.is_empty() {
        type_stmt.push(' ');
        type_stmt.push_str(&base_where);
    }
    type_stmt.push_str(" GROUP BY item_type");
    let mut type_counts = Map::new();
    let mut stmt = conn
        .prepare(&type_stmt)
        .map_err(|e| format!("prepare inbox type facets failed: {e}"))?;
    let rows = stmt
        .query_map(base_param_refs.as_slice(), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|e| format!("query inbox type facets failed: {e}"))?;
    for row in rows {
        let (item_type, count) = row.map_err(|e| e.to_string())?;
        type_counts.insert(item_type, json!(count));
    }

    let starred = query_simple_count(conn, &base_where, &base_params, "starred = 1")?;
    let external =
        query_simple_count(conn, &base_where, &base_params, "storage_kind = 'external'")?;
    let summary_only = query_simple_count(
        conn,
        &base_where,
        &base_params,
        "storage_kind = 'metadata_only'",
    )?;

    Ok(json!({
        "buckets": bucket_counts,
        "types": type_counts,
        "starred": starred,
        "external": external,
        "summaryOnly": summary_only,
    }))
}

fn query_simple_count(
    conn: &Connection,
    base_where: &str,
    params_list: &[SqlParam],
    extra_condition: &str,
) -> Result<i64, String> {
    let mut sql = String::from("SELECT COUNT(*) FROM inbox_items");
    if base_where.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(extra_condition);
    } else {
        sql.push(' ');
        sql.push_str(base_where);
        sql.push_str(" AND ");
        sql.push_str(extra_condition);
    }
    let params_refs: Vec<&dyn rusqlite::ToSql> =
        params_list.iter().map(|value| value.as_ref()).collect();
    conn.query_row(&sql, params_refs.as_slice(), |row| row.get::<_, i64>(0))
        .map_err(|e| format!("query inbox facet count failed: {e}"))
}

fn row_to_summary(row: &Row<'_>) -> rusqlite::Result<Value> {
    let meta_json = row.get::<_, Option<String>>(12)?;
    let note = row.get::<_, String>(11)?;
    Ok(json!({
        "id": row.get::<_, i64>(0)?,
        "bucket": row.get::<_, String>(1)?,
        "itemType": row.get::<_, String>(2)?,
        "storageKind": row.get::<_, String>(3)?,
        "title": row.get::<_, String>(4)?,
        "preview": row.get::<_, String>(5)?,
        "byteSize": row.get::<_, i64>(6)?,
        "capturedAt": row.get::<_, String>(7)?,
        "lastSeenAt": row.get::<_, String>(8)?,
        "seenCount": row.get::<_, i64>(9)?,
        "starred": row.get::<_, i64>(10)? > 0,
        "hasNote": !note.trim().is_empty(),
        "metaJson": parse_meta_json(meta_json),
    }))
}

fn row_to_detail(row: &Row<'_>) -> rusqlite::Result<Value> {
    let id = row.get::<_, i64>(0)?;
    let storage_kind = row.get::<_, String>(3)?;
    let item_type = row.get::<_, String>(2)?;
    let payload_ref = row.get::<_, Option<String>>(7)?;
    let note = row.get::<_, String>(13)?;
    let meta_json = row.get::<_, Option<String>>(15)?;
    let mut meta_value = parse_meta_json(meta_json);
    let file_refs = load_file_refs(id).unwrap_or_else(|_| Vec::new());
    let (payload_text, payload_data_url, open_path, can_open_path) = resolve_detail_payload(
        &item_type,
        &storage_kind,
        payload_ref.as_deref(),
        &mut meta_value,
    );

    Ok(json!({
        "id": id,
        "bucket": row.get::<_, String>(1)?,
        "itemType": item_type,
        "storageKind": storage_kind,
        "title": row.get::<_, String>(4)?,
        "preview": row.get::<_, String>(5)?,
        "searchText": row.get::<_, String>(6)?,
        "payloadText": payload_text,
        "payloadDataUrl": payload_data_url,
        "byteSize": row.get::<_, i64>(8)?,
        "contentHash": row.get::<_, String>(9)?,
        "capturedAt": row.get::<_, String>(10)?,
        "lastSeenAt": row.get::<_, String>(11)?,
        "seenCount": row.get::<_, i64>(12)?,
        "note": note,
        "starred": row.get::<_, i64>(14)? > 0,
        "metaJson": meta_value,
        "fileRefs": file_refs,
        "openPath": open_path,
        "canOpenPath": can_open_path,
        "hasNote": !note.trim().is_empty(),
    }))
}

fn load_file_refs(item_id: i64) -> Result<Vec<Value>, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, file_path, file_name, file_size, modified_at
             FROM inbox_file_refs
             WHERE inbox_item_id = ?1
             ORDER BY id ASC",
        )
        .map_err(|e| format!("prepare inbox file refs failed: {e}"))?;
    let rows = stmt
        .query_map(params![item_id], |row| {
            let file_path = row.get::<_, String>(1)?;
            let path = PathBuf::from(&file_path);
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "filePath": file_path,
                "fileName": row.get::<_, String>(2)?,
                "fileSize": row.get::<_, Option<i64>>(3)?,
                "modifiedAt": row.get::<_, Option<String>>(4)?,
                "exists": path.exists(),
                "isDirectory": path.is_dir(),
            }))
        })
        .map_err(|e| format!("query inbox file refs failed: {e}"))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| e.to_string())?);
    }
    Ok(items)
}

fn resolve_detail_payload(
    item_type: &str,
    storage_kind: &str,
    payload_ref: Option<&str>,
    meta_json: &mut Value,
) -> (Option<String>, Option<String>, Option<String>, bool) {
    let mut open_path = payload_ref.map(|value| value.to_string());
    let mut can_open_path = false;
    match item_type {
        "image" => {
            if let Some(thumb_path) = meta_json["thumbPath"].as_str() {
                open_path = Some(resolve_asset_path(thumb_path).to_string_lossy().to_string());
                can_open_path = true;
            }
            let original_data_url = payload_ref.and_then(read_asset_data_url).or_else(|| {
                meta_json["thumbPath"]
                    .as_str()
                    .and_then(read_asset_data_url)
            });
            if let Some(original_path) = payload_ref {
                open_path = Some(
                    resolve_asset_path(original_path)
                        .to_string_lossy()
                        .to_string(),
                );
                can_open_path = true;
            }
            (None, original_data_url, open_path, can_open_path)
        }
        "file" => (None, None, open_path, false),
        _ => {
            if storage_kind == "external" {
                if let Some(path) = payload_ref {
                    let full_path = resolve_asset_path(path);
                    let payload_text = fs::read_to_string(&full_path).ok();
                    (
                        payload_text,
                        None,
                        Some(full_path.to_string_lossy().to_string()),
                        true,
                    )
                } else {
                    (None, None, open_path, false)
                }
            } else {
                (
                    payload_ref.map(|value| value.to_string()),
                    None,
                    None,
                    false,
                )
            }
        }
    }
}

fn read_asset_data_url(relative_path: &str) -> Option<String> {
    let full_path = resolve_asset_path(relative_path);
    let bytes = fs::read(full_path).ok()?;
    Some(format!("data:image/png;base64,{}", BASE64.encode(bytes)))
}

fn parse_meta_json(meta_json: Option<String>) -> Value {
    meta_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .unwrap_or(Value::Null)
}

fn upsert_capture_candidate(conn: &Connection, candidate: CaptureCandidate) -> Result<(), String> {
    if let Some(existing_id) =
        find_duplicate_item(conn, candidate.item_type, &candidate.content_hash)?
    {
        conn.execute(
            "UPDATE inbox_items
             SET last_seen_at = CURRENT_TIMESTAMP,
                 seen_count = seen_count + 1
             WHERE id = ?1",
            params![existing_id],
        )
        .map_err(|e| format!("update duplicate inbox item failed: {e}"))?;
        return Ok(());
    }

    let (payload_ref, meta_json) = persist_candidate_assets(conn, &candidate)?;
    conn.execute(
        "INSERT INTO inbox_items(
            bucket, item_type, storage_kind, title, preview, search_text, payload_ref, byte_size,
            content_hash, note, starred, meta_json
         ) VALUES(
            'history', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, '', 0, ?9
         )",
        params![
            candidate.item_type,
            candidate.storage_kind,
            candidate.title,
            candidate.preview,
            candidate.search_text,
            payload_ref,
            candidate.byte_size,
            candidate.content_hash,
            meta_json,
        ],
    )
    .map_err(|e| format!("insert inbox item failed: {e}"))?;

    let item_id = conn.last_insert_rowid();
    for file_ref in candidate.file_refs {
        conn.execute(
            "INSERT INTO inbox_file_refs(inbox_item_id, file_path, file_name, file_size, modified_at)
             VALUES(?1, ?2, ?3, ?4, ?5)",
            params![
                item_id,
                file_ref.file_path,
                file_ref.file_name,
                file_ref.file_size,
                file_ref.modified_at,
            ],
        )
        .map_err(|e| format!("insert inbox file ref failed: {e}"))?;
    }
    Ok(())
}

fn find_duplicate_item(
    conn: &Connection,
    item_type: &str,
    content_hash: &str,
) -> Result<Option<i64>, String> {
    let row = conn
        .query_row(
            "SELECT id, last_seen_at
             FROM inbox_items
             WHERE item_type = ?1 AND content_hash = ?2
             ORDER BY last_seen_at DESC, id DESC
             LIMIT 1",
            params![item_type, content_hash],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|e| format!("query duplicate inbox item failed: {e}"))?;
    let Some((id, last_seen_at)) = row else {
        return Ok(None);
    };
    let Some(last_seen_at) = parse_datetime(&last_seen_at) else {
        return Ok(None);
    };
    if Utc::now() - last_seen_at <= ChronoDuration::seconds(DUPLICATE_WINDOW_SECS) {
        Ok(Some(id))
    } else {
        Ok(None)
    }
}

fn persist_candidate_assets(
    conn: &Connection,
    candidate: &CaptureCandidate,
) -> Result<(Option<String>, String), String> {
    let mut meta_map = candidate.meta_json.as_object().cloned().unwrap_or_default();
    let payload_ref = if let Some(bytes) = candidate.external_bytes.as_ref() {
        Some(persist_asset_ref(conn, &candidate.content_hash, bytes)?)
    } else {
        candidate.payload_text.clone()
    };

    if let Some(thumb_bytes) = candidate.thumb_bytes.as_ref() {
        let thumb_hash = format!("{}-thumb", candidate.content_hash);
        let thumb_path = persist_asset_ref(conn, &thumb_hash, thumb_bytes)?;
        meta_map.insert("thumbPath".to_string(), json!(thumb_path));
    }

    let meta_json = if meta_map.is_empty() {
        Value::Null.to_string()
    } else {
        serde_json::to_string(&Value::Object(meta_map))
            .map_err(|e| format!("serialize inbox meta failed: {e}"))?
    };
    Ok((payload_ref, meta_json))
}

fn persist_asset_ref(conn: &Connection, asset_key: &str, bytes: &[u8]) -> Result<String, String> {
    if let Some((file_path, ref_count)) = conn
        .query_row(
            "SELECT file_path, ref_count FROM inbox_asset_refs WHERE content_hash = ?1",
            params![asset_key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(|e| format!("query inbox asset ref failed: {e}"))?
    {
        let full_path = resolve_asset_path(&file_path);
        if !full_path.exists() {
            write_asset_file(&full_path, bytes)?;
        }
        conn.execute(
            "UPDATE inbox_asset_refs SET ref_count = ?2 WHERE content_hash = ?1",
            params![asset_key, ref_count + 1],
        )
        .map_err(|e| format!("update inbox asset ref count failed: {e}"))?;
        return Ok(file_path);
    }

    let relative_path = build_asset_relative_path(asset_key);
    let full_path = resolve_asset_path(&relative_path);
    write_asset_file(&full_path, bytes)?;
    conn.execute(
        "INSERT INTO inbox_asset_refs(content_hash, file_path, ref_count, byte_size)
         VALUES(?1, ?2, 1, ?3)",
        params![asset_key, relative_path, bytes.len() as i64],
    )
    .map_err(|e| format!("insert inbox asset ref failed: {e}"))?;
    Ok(relative_path)
}

fn write_asset_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create inbox asset parent failed: {e}"))?;
    }
    fs::write(path, bytes).map_err(|e| format!("write inbox asset failed: {e}"))?;
    Ok(())
}

fn build_asset_relative_path(asset_key: &str) -> String {
    let prefix = asset_key.chars().take(2).collect::<String>();
    format!("inbox-assets/{prefix}/{asset_key}")
}

fn resolve_asset_path(relative_path: &str) -> PathBuf {
    get_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(relative_path)
}

fn delete_inbox_item(conn: &mut Connection, id: i64) -> Result<(), String> {
    let tx = conn
        .transaction()
        .map_err(|e| format!("start inbox delete transaction failed: {e}"))?;

    let row = tx
        .query_row(
            "SELECT payload_ref, meta_json FROM inbox_items WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("query inbox item before delete failed: {e}"))?
        .ok_or("收纳箱条目不存在")?;

    let payload_ref = row.0;
    let meta_json = parse_meta_json(row.1);

    tx.execute("DELETE FROM inbox_items WHERE id = ?1", params![id])
        .map_err(|e| format!("delete inbox item failed: {e}"))?;

    if let Some(path) = payload_ref.as_deref() {
        decrement_asset_ref(&tx, path)?;
    }
    if let Some(path) = meta_json["thumbPath"].as_str() {
        decrement_asset_ref(&tx, path)?;
    }

    tx.commit()
        .map_err(|e| format!("commit inbox delete transaction failed: {e}"))?;
    Ok(())
}

fn decrement_asset_ref(conn: &Connection, relative_path: &str) -> Result<(), String> {
    let Some((content_hash, ref_count)) = conn
        .query_row(
            "SELECT content_hash, ref_count FROM inbox_asset_refs WHERE file_path = ?1",
            params![relative_path],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(|e| format!("query inbox asset by path failed: {e}"))?
    else {
        return Ok(());
    };

    if ref_count > 1 {
        conn.execute(
            "UPDATE inbox_asset_refs SET ref_count = ?2 WHERE content_hash = ?1",
            params![content_hash, ref_count - 1],
        )
        .map_err(|e| format!("decrement inbox asset ref failed: {e}"))?;
        return Ok(());
    }

    conn.execute(
        "DELETE FROM inbox_asset_refs WHERE content_hash = ?1",
        params![content_hash],
    )
    .map_err(|e| format!("delete inbox asset ref failed: {e}"))?;

    let full_path = resolve_asset_path(relative_path);
    let _ = fs::remove_file(full_path);
    Ok(())
}

fn cleanup_history_if_needed(conn: &Connection, retention_days: i64) -> Result<(), String> {
    cleanup_expired_history(retention_days)?;
    cleanup_history_count_limit()?;
    cleanup_history_asset_limit()?;
    cleanup_orphan_asset_files(conn)?;
    Ok(())
}

fn cleanup_expired_history(retention_days: i64) -> Result<(), String> {
    let conn = db_conn()?;
    let cutoff = (Utc::now() - ChronoDuration::days(retention_days.max(1)))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let mut stmt = conn
        .prepare(
            "SELECT id FROM inbox_items
             WHERE bucket = 'history' AND last_seen_at < ?1
             ORDER BY last_seen_at ASC, id ASC",
        )
        .map_err(|e| format!("prepare expired inbox history failed: {e}"))?;
    let ids = stmt
        .query_map(params![cutoff], |row| row.get::<_, i64>(0))
        .map_err(|e| format!("query expired inbox history failed: {e}"))?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    let mut conn = db_conn()?;
    for id in ids {
        let _ = delete_inbox_item(&mut conn, id);
    }
    Ok(())
}

fn cleanup_history_count_limit() -> Result<(), String> {
    let conn = db_conn()?;
    let current_count = conn
        .query_row(
            "SELECT COUNT(*) FROM inbox_items WHERE bucket = 'history'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| format!("count inbox history failed: {e}"))?;
    if current_count <= HISTORY_ITEM_LIMIT {
        return Ok(());
    }

    let remove_count = current_count - HISTORY_ITEM_LIMIT;
    let mut stmt = conn
        .prepare(
            "SELECT id FROM inbox_items
             WHERE bucket = 'history'
             ORDER BY last_seen_at ASC, id ASC
             LIMIT ?1",
        )
        .map_err(|e| format!("prepare inbox history trim failed: {e}"))?;
    let ids = stmt
        .query_map(params![remove_count], |row| row.get::<_, i64>(0))
        .map_err(|e| format!("query inbox history trim failed: {e}"))?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    let mut conn = db_conn()?;
    for id in ids {
        let _ = delete_inbox_item(&mut conn, id);
    }
    Ok(())
}

fn cleanup_history_asset_limit() -> Result<(), String> {
    let conn = db_conn()?;
    let current_bytes = conn
        .query_row(
            "SELECT COALESCE(SUM(byte_size), 0) FROM inbox_asset_refs",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| format!("sum inbox asset refs failed: {e}"))?;
    if current_bytes <= HISTORY_ASSET_LIMIT_BYTES {
        return Ok(());
    }

    let mut bytes = current_bytes;
    let mut stmt = conn
        .prepare(
            "SELECT id FROM inbox_items
             WHERE bucket = 'history'
             ORDER BY last_seen_at ASC, id ASC",
        )
        .map_err(|e| format!("prepare inbox asset trim failed: {e}"))?;
    let ids = stmt
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(|e| format!("query inbox asset trim failed: {e}"))?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    let mut conn = db_conn()?;
    for id in ids {
        if bytes <= HISTORY_ASSET_LIMIT_BYTES {
            break;
        }
        delete_inbox_item(&mut conn, id)?;
        bytes = conn
            .query_row(
                "SELECT COALESCE(SUM(byte_size), 0) FROM inbox_asset_refs",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| format!("sum inbox asset refs after trim failed: {e}"))?;
    }
    Ok(())
}

fn cleanup_orphan_asset_files(conn: &Connection) -> Result<(), String> {
    let asset_root = get_data_dir()?.join("inbox-assets");
    if !asset_root.exists() {
        return Ok(());
    }

    let mut known_paths = HashSet::new();
    let mut stmt = conn
        .prepare("SELECT file_path FROM inbox_asset_refs")
        .map_err(|e| format!("prepare inbox asset path scan failed: {e}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("query inbox asset path scan failed: {e}"))?;
    for row in rows {
        known_paths.insert(resolve_asset_path(&row.map_err(|e| e.to_string())?));
    }

    for entry in WalkDir::new(&asset_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        if !known_paths.contains(entry.path()) {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(())
}

fn inbox_has_fts(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT count(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'inbox_fts'",
        [],
        |row| row.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

fn should_suppress_capture_hash(content_hash: &str) -> bool {
    let Ok(mut list) = SUPPRESSED_CLIPBOARD.try_lock() else {
        return false;
    };
    let now = Instant::now();
    list.retain(|item| item.expires_at > now);
    if let Some(index) = list
        .iter()
        .position(|item| item.content_hash == content_hash)
    {
        list.remove(index);
        return true;
    }
    false
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    hasher.finalize().to_hex().to_string()
}

#[cfg(windows)]
fn copy_image_file_to_clipboard(path: &Path) -> Result<(), String> {
    use std::{mem, ptr};
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::Graphics::Gdi::{BITMAPINFOHEADER, BI_RGB};
    use windows_sys::Win32::System::DataExchange::{EmptyClipboard, SetClipboardData};
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };

    const CF_DIB: u32 = 8;

    let file_bytes = fs::read(path).map_err(|e| format!("read image file failed: {e}"))?;
    let image = image::load_from_memory(&file_bytes)
        .map_err(|e| format!("decode image file failed: {e}"))?;
    let mut png_bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
        .map_err(|e| format!("encode clipboard image failed: {e}"))?;
    suppress_clipboard_hash(&hash_bytes(&png_bytes))?;

    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return Err("图像尺寸无效".to_string());
    }

    let mut dib_pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in (0..height).rev() {
        for x in 0..width {
            let [r, g, b, a] = rgba.get_pixel(x, y).0;
            dib_pixels.extend_from_slice(&[b, g, r, a]);
        }
    }

    let header = BITMAPINFOHEADER {
        biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width as i32,
        biHeight: height as i32,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        biSizeImage: dib_pixels.len() as u32,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    };

    let header_size = mem::size_of::<BITMAPINFOHEADER>();
    let total_size = header_size + dib_pixels.len();

    unsafe {
        let handle = GlobalAlloc(GMEM_MOVEABLE, total_size);
        if handle.is_null() {
            return Err("allocate clipboard image failed".to_string());
        }

        let buffer = GlobalLock(handle) as *mut u8;
        if buffer.is_null() {
            GlobalFree(handle);
            return Err("lock clipboard image failed".to_string());
        }

        ptr::copy_nonoverlapping(
            (&header as *const BITMAPINFOHEADER).cast::<u8>(),
            buffer,
            header_size,
        );
        ptr::copy_nonoverlapping(
            dib_pixels.as_ptr(),
            buffer.add(header_size),
            dib_pixels.len(),
        );
        GlobalUnlock(handle);

        let _guard = crate::clipboard::ClipboardGuard::open()?;
        if EmptyClipboard() == 0 {
            GlobalFree(handle);
            return Err("clear clipboard failed".to_string());
        }
        if SetClipboardData(CF_DIB, handle).is_null() {
            GlobalFree(handle);
            return Err("set clipboard image failed".to_string());
        }
    }

    Ok(())
}

#[cfg(not(windows))]
fn copy_image_file_to_clipboard(_path: &Path) -> Result<(), String> {
    Err("当前平台暂不支持复制图像到系统剪贴板".to_string())
}

fn parse_datetime(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
        .or_else(|| {
            NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|value| DateTime::<Utc>::from_naive_utc_and_offset(value, Utc))
        })
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn strip_html(raw: &str) -> String {
    let without_tags = HTML_TAG_RE.replace_all(raw, " ");
    HTML_WS_RE.replace_all(without_tags.trim(), " ").to_string()
}

fn parse_cf_html_offset(raw: &str, key: &str) -> Option<usize> {
    raw.lines().find_map(|line| {
        line.trim()
            .strip_prefix(key)
            .and_then(|value| value.trim().parse::<usize>().ok())
    })
}

fn extract_cf_html_slice(
    raw: &str,
    bytes: &[u8],
    start_key: &str,
    end_key: &str,
) -> Option<String> {
    let start = parse_cf_html_offset(raw, start_key)?;
    let end = parse_cf_html_offset(raw, end_key)?;
    if start >= end || end > bytes.len() {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes[start..end]).to_string())
}

fn extract_cf_html_content(bytes: &[u8]) -> String {
    let raw = String::from_utf8_lossy(bytes);
    if let Some(fragment) =
        extract_cf_html_slice(raw.as_ref(), bytes, "StartFragment:", "EndFragment:")
    {
        return fragment.trim_matches('\0').to_string();
    }
    if let Some(html) = extract_cf_html_slice(raw.as_ref(), bytes, "StartHTML:", "EndHTML:") {
        return html.trim_matches('\0').to_string();
    }

    let raw = raw.trim_end_matches('\0');
    if let (Some(start), Some(end)) = (
        raw.find("<!--StartFragment-->"),
        raw.find("<!--EndFragment-->"),
    ) {
        let fragment_start = start + "<!--StartFragment-->".len();
        if fragment_start < end {
            return raw[fragment_start..end].trim().to_string();
        }
    }
    if let Some(index) = raw.to_ascii_lowercase().find("<html") {
        return raw[index..].to_string();
    }
    if let Some(index) = raw.find('<') {
        return raw[index..].to_string();
    }
    raw.to_string()
}

fn strip_rtf(raw: &str) -> String {
    let with_breaks = raw.replace("\\par", "\n");
    let without_hex = RTF_HEX_RE.replace_all(&with_breaks, "");
    let without_controls = RTF_CONTROL_RE.replace_all(without_hex.as_ref(), " ");
    let without_braces = without_controls.replace(['{', '}'], " ");
    HTML_WS_RE
        .replace_all(without_braces.trim(), " ")
        .to_string()
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    value.chars().take(max_chars).collect::<String>() + "…"
}

fn single_line_preview(value: &str, max_chars: usize) -> String {
    let compact = value
        .replace('\r', " ")
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    truncate_chars(compact.trim(), max_chars)
}

fn auto_title(value: &str, fallback: &str) -> String {
    let first_line = value
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or(fallback);
    truncate_chars(first_line, 48)
}

fn metadata_excerpt(value: &str) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= METADATA_EXCERPT_LIMIT * 2 {
        return value.to_string();
    }
    let head = chars[..METADATA_EXCERPT_LIMIT].iter().collect::<String>();
    let tail = chars[chars.len().saturating_sub(METADATA_EXCERPT_LIMIT)..]
        .iter()
        .collect::<String>();
    format!("{head}\n...\n{tail}")
}

fn normalize_text_for_hash(value: &str) -> String {
    value.replace("\r\n", "\n").trim().to_string()
}

fn format_byte_size(byte_size: i64) -> String {
    if byte_size >= 1024 * 1024 {
        format!("{:.1} MB", byte_size as f64 / (1024.0 * 1024.0))
    } else if byte_size >= 1024 {
        format!("{:.1} KB", byte_size as f64 / 1024.0)
    } else {
        format!("{byte_size} B")
    }
}

#[cfg(windows)]
fn read_clipboard_candidate() -> Result<Option<CaptureCandidate>, String> {
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
        BI_RGB, DIB_RGB_COLORS, HBITMAP,
    };
    use windows_sys::Win32::System::DataExchange::{
        EnumClipboardFormats, GetClipboardData, GetClipboardFormatNameW,
        IsClipboardFormatAvailable, RegisterClipboardFormatW,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use windows_sys::Win32::UI::Shell::DragQueryFileW;

    const CF_BITMAP: u32 = 2;
    const CF_UNICODETEXT: u32 = 13;
    const CF_HDROP: u32 = 15;

    fn read_bytes_handle(handle: HANDLE) -> Option<Vec<u8>> {
        unsafe {
            let size = GlobalSize(handle);
            if size == 0 {
                return None;
            }
            let ptr = GlobalLock(handle) as *const u8;
            if ptr.is_null() {
                return None;
            }
            let bytes = std::slice::from_raw_parts(ptr, size).to_vec();
            GlobalUnlock(handle);
            Some(bytes)
        }
    }

    fn build_text_candidate(
        item_type: &'static str,
        raw: String,
        search_text: String,
    ) -> CaptureCandidate {
        let bytes = raw.as_bytes();
        let byte_size = bytes.len() as i64;
        let normalized = (item_type == "text").then(|| normalize_text_for_hash(&raw));
        let hash_source = normalized.as_deref().map(str::as_bytes).unwrap_or(bytes);
        let content_hash = hash_bytes(hash_source);
        let title = auto_title(
            &search_text,
            match item_type {
                "html" => "HTML 片段",
                "rtf" => "RTF 文本",
                _ => "文本片段",
            },
        );
        let preview = single_line_preview(&search_text, 120);
        if bytes.len() <= INLINE_TEXT_LIMIT {
            return CaptureCandidate {
                item_type,
                storage_kind: "inline",
                title,
                preview,
                search_text,
                payload_text: Some(raw.clone()),
                external_bytes: None,
                thumb_bytes: None,
                byte_size,
                content_hash,
                meta_json: Value::Null,
                file_refs: Vec::new(),
            };
        }
        if bytes.len() <= EXTERNAL_TEXT_LIMIT {
            return CaptureCandidate {
                item_type,
                storage_kind: "external",
                title,
                preview,
                search_text,
                payload_text: None,
                external_bytes: Some(raw.clone().into_bytes()),
                thumb_bytes: None,
                byte_size,
                content_hash,
                meta_json: Value::Null,
                file_refs: Vec::new(),
            };
        }
        let excerpt = metadata_excerpt(&raw);
        CaptureCandidate {
            item_type,
            storage_kind: "metadata_only",
            title,
            preview,
            search_text: metadata_excerpt(&search_text),
            payload_text: Some(excerpt),
            external_bytes: None,
            thumb_bytes: None,
            byte_size,
            content_hash,
            meta_json: json!({ "excerpt": true }),
            file_refs: Vec::new(),
        }
    }

    fn read_files() -> Result<Option<CaptureCandidate>, String> {
        unsafe {
            if IsClipboardFormatAvailable(CF_HDROP) == 0 {
                return Ok(None);
            }
            let handle = GetClipboardData(CF_HDROP);
            if handle.is_null() {
                return Ok(None);
            }
            let count = DragQueryFileW(handle as _, u32::MAX, std::ptr::null_mut(), 0);
            if count == 0 {
                return Ok(None);
            }

            let mut refs = Vec::new();
            let mut search_parts = Vec::new();
            let mut total_size = 0i64;
            for index in 0..count {
                let len = DragQueryFileW(handle as _, index, std::ptr::null_mut(), 0);
                if len == 0 {
                    continue;
                }
                let mut buf = vec![0u16; len as usize + 1];
                DragQueryFileW(handle as _, index, buf.as_mut_ptr(), len + 1);
                let path = String::from_utf16_lossy(&buf[..len as usize]);
                let path_buf = PathBuf::from(&path);
                let metadata = fs::metadata(&path_buf).ok();
                let modified_at = metadata
                    .as_ref()
                    .and_then(|meta| meta.modified().ok())
                    .map(|time| DateTime::<Utc>::from(time).to_rfc3339());
                let file_name = path_buf
                    .file_name()
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone());
                total_size += metadata.as_ref().map(|meta| meta.len() as i64).unwrap_or(0);
                search_parts.push(path.clone());
                refs.push(CapturedFileRef {
                    file_path: path,
                    file_name,
                    file_size: metadata.as_ref().map(|meta| meta.len() as i64),
                    modified_at,
                });
            }
            if refs.is_empty() {
                return Ok(None);
            }
            let title = if refs.len() == 1 {
                refs[0].file_name.clone()
            } else {
                format!("{} 个文件引用", refs.len())
            };
            let preview = refs
                .iter()
                .take(3)
                .map(|item| item.file_name.as_str())
                .collect::<Vec<_>>()
                .join(" · ");
            let hash_source = refs
                .iter()
                .map(|item| item.file_path.to_lowercase())
                .collect::<Vec<_>>()
                .join("\n");
            Ok(Some(CaptureCandidate {
                item_type: "file",
                storage_kind: "metadata_only",
                title,
                preview,
                search_text: search_parts.join("\n"),
                payload_text: None,
                external_bytes: None,
                thumb_bytes: None,
                byte_size: total_size,
                content_hash: hash_bytes(hash_source.as_bytes()),
                meta_json: json!({ "count": refs.len() }),
                file_refs: refs,
            }))
        }
    }

    fn read_image() -> Result<Option<CaptureCandidate>, String> {
        unsafe {
            if IsClipboardFormatAvailable(CF_BITMAP) == 0 {
                return Ok(None);
            }
            let handle = GetClipboardData(CF_BITMAP);
            if handle.is_null() {
                return Ok(None);
            }
            let hbitmap = handle as HBITMAP;
            let mut bitmap = std::mem::zeroed::<BITMAP>();
            if GetObjectW(
                hbitmap as _,
                std::mem::size_of::<BITMAP>() as i32,
                &mut bitmap as *mut _ as *mut c_void,
            ) == 0
            {
                return Ok(None);
            }
            let width = bitmap.bmWidth as u32;
            let height = bitmap.bmHeight.unsigned_abs() as u32;
            if width == 0 || height == 0 {
                return Ok(None);
            }

            let mut info = std::mem::zeroed::<BITMAPINFO>();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = width as i32;
            info.bmiHeader.biHeight = -(height as i32);
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;

            let mut pixels = vec![0u8; (width * height * 4) as usize];
            let hdc = CreateCompatibleDC(std::ptr::null_mut());
            let rows = GetDIBits(
                hdc,
                hbitmap,
                0,
                height,
                pixels.as_mut_ptr() as *mut c_void,
                &mut info,
                DIB_RGB_COLORS,
            );
            DeleteDC(hdc);
            if rows == 0 {
                return Ok(None);
            }

            for chunk in pixels.chunks_exact_mut(4) {
                chunk.swap(0, 2);
                if chunk[3] == 0 {
                    chunk[3] = 255;
                }
            }

            let image = RgbaImage::from_raw(width, height, pixels)
                .map(DynamicImage::ImageRgba8)
                .ok_or("build clipboard image failed")?;
            let mut original_png = Vec::new();
            image
                .write_to(&mut Cursor::new(&mut original_png), ImageFormat::Png)
                .map_err(|e| format!("encode clipboard image failed: {e}"))?;

            let thumbnail = image.resize(
                THUMBNAIL_LONG_EDGE,
                THUMBNAIL_LONG_EDGE,
                FilterType::Lanczos3,
            );
            let mut thumb_png = Vec::new();
            thumbnail
                .write_to(&mut Cursor::new(&mut thumb_png), ImageFormat::Png)
                .map_err(|e| format!("encode clipboard thumbnail failed: {e}"))?;

            let keep_original = original_png.len() <= IMAGE_KEEP_LIMIT_BYTES
                && (width as u64) * (height as u64) <= IMAGE_KEEP_LIMIT_PIXELS;
            let preview = format!(
                "{}×{} · {}",
                width,
                height,
                format_byte_size(original_png.len() as i64)
            );

            Ok(Some(CaptureCandidate {
                item_type: "image",
                storage_kind: if keep_original {
                    "external"
                } else {
                    "metadata_only"
                },
                title: format!("图片 {}×{}", width, height),
                preview,
                search_text: format!("image {}x{}", width, height),
                payload_text: None,
                external_bytes: if keep_original {
                    Some(original_png.clone())
                } else {
                    None
                },
                thumb_bytes: Some(thumb_png),
                byte_size: original_png.len() as i64,
                content_hash: hash_bytes(&original_png),
                meta_json: json!({
                    "width": width,
                    "height": height,
                    "keptOriginal": keep_original,
                }),
                file_refs: Vec::new(),
            }))
        }
    }

    fn read_registered_text(format_name: &str) -> Result<Option<Vec<u8>>, String> {
        let wide = format_name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>();
        unsafe {
            let format_id = RegisterClipboardFormatW(wide.as_ptr());
            if format_id == 0 || IsClipboardFormatAvailable(format_id) == 0 {
                return Ok(None);
            }
            let handle = GetClipboardData(format_id);
            if handle.is_null() {
                return Ok(None);
            }
            Ok(read_bytes_handle(handle))
        }
    }

    fn read_text() -> Result<Option<CaptureCandidate>, String> {
        let Some(text) = crate::clipboard::read_unicode_text_from_open_clipboard()? else {
            return Ok(None);
        };
        if text.trim().is_empty() {
            return Ok(None);
        }
        Ok(Some(build_text_candidate(
            "text",
            text.clone(),
            text.trim().to_string(),
        )))
    }

    fn read_html() -> Result<Option<CaptureCandidate>, String> {
        let Some(bytes) = read_registered_text("HTML Format")? else {
            return Ok(None);
        };
        let raw = String::from_utf8_lossy(&bytes)
            .trim_end_matches('\0')
            .to_string();
        if raw.trim().is_empty() {
            return Ok(None);
        }
        let html_body = extract_cf_html_content(&bytes);
        Ok(Some(build_text_candidate(
            "html",
            raw.clone(),
            strip_html(&html_body),
        )))
    }

    fn read_rtf() -> Result<Option<CaptureCandidate>, String> {
        let Some(bytes) = read_registered_text("Rich Text Format")? else {
            return Ok(None);
        };
        let raw = String::from_utf8_lossy(&bytes)
            .trim_end_matches('\0')
            .to_string();
        if raw.trim().is_empty() {
            return Ok(None);
        }
        Ok(Some(build_text_candidate(
            "rtf",
            raw.clone(),
            strip_rtf(&raw),
        )))
    }

    fn read_unknown() -> Result<Option<CaptureCandidate>, String> {
        unsafe {
            let mut format = 0u32;
            let mut names = Vec::new();
            loop {
                format = EnumClipboardFormats(format);
                if format == 0 {
                    break;
                }
                let label = match format as u32 {
                    x if x == CF_UNICODETEXT as u32 => "CF_UNICODETEXT".to_string(),
                    x if x == CF_HDROP as u32 => "CF_HDROP".to_string(),
                    x if x == CF_BITMAP as u32 => "CF_BITMAP".to_string(),
                    other => {
                        let mut buf = vec![0u16; 256];
                        let len =
                            GetClipboardFormatNameW(other, buf.as_mut_ptr(), buf.len() as i32);
                        if len > 0 {
                            String::from_utf16_lossy(&buf[..len as usize])
                        } else {
                            format!("format:{other}")
                        }
                    }
                };
                names.push(label);
            }
            if names.is_empty() {
                return Ok(None);
            }
            let preview = names.join(" · ");
            Ok(Some(CaptureCandidate {
                item_type: "unknown",
                storage_kind: "metadata_only",
                title: "未知剪贴板格式".to_string(),
                preview: single_line_preview(&preview, 120),
                search_text: preview.clone(),
                payload_text: None,
                external_bytes: None,
                thumb_bytes: None,
                byte_size: 0,
                content_hash: hash_bytes(preview.as_bytes()),
                meta_json: json!({ "formats": names }),
                file_refs: Vec::new(),
            }))
        }
    }

    let _guard = crate::clipboard::ClipboardGuard::open()?;
    if let Some(candidate) = read_files()? {
        return Ok(Some(candidate));
    }
    if let Some(candidate) = read_image()? {
        return Ok(Some(candidate));
    }
    if let Some(candidate) = read_html()? {
        return Ok(Some(candidate));
    }
    if let Some(candidate) = read_rtf()? {
        return Ok(Some(candidate));
    }
    if let Some(candidate) = read_text()? {
        return Ok(Some(candidate));
    }
    read_unknown()
}

#[cfg(not(windows))]
fn read_clipboard_candidate() -> Result<Option<CaptureCandidate>, String> {
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{
        extract_cf_html_content, hash_bytes, normalize_text_for_hash, should_suppress_capture_hash,
        strip_html, suppress_clipboard_capture, SUPPRESSED_CLIPBOARD,
    };
    use serde_json::json;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn build_cf_html(fragment: &str) -> Vec<u8> {
        let body =
            format!("<html><body><!--StartFragment-->{fragment}<!--EndFragment--></body></html>");
        let header = format!(
            "Version:1.0\r\nStartHTML:{:08}\r\nEndHTML:{:08}\r\nStartFragment:{:08}\r\nEndFragment:{:08}\r\n",
            0, 0, 0, 0
        );
        let start_html = header.len();
        let start_fragment = start_html
            + body
                .find("<!--StartFragment-->")
                .expect("start fragment marker")
            + "<!--StartFragment-->".len();
        let end_fragment = start_html
            + body
                .find("<!--EndFragment-->")
                .expect("end fragment marker");
        let end_html = start_html + body.len();
        let header = format!(
            "Version:1.0\r\nStartHTML:{start_html:08}\r\nEndHTML:{end_html:08}\r\nStartFragment:{start_fragment:08}\r\nEndFragment:{end_fragment:08}\r\n"
        );
        format!("{header}{body}").into_bytes()
    }

    #[test]
    fn extract_cf_html_prefers_fragment_offsets() {
        let bytes = build_cf_html("<p>收纳正文</p>");
        let content = extract_cf_html_content(&bytes);
        assert_eq!(content, "<p>收纳正文</p>");
        assert_eq!(strip_html(&content), "收纳正文");
    }

    #[test]
    fn extract_cf_html_falls_back_to_html_start() {
        let raw = "Version:1.0\r\nStartHTML:bad\r\n<html><body><p>收纳正文</p></body></html>";
        assert_eq!(
            extract_cf_html_content(raw.as_bytes()),
            "<html><body><p>收纳正文</p></body></html>"
        );
    }

    #[test]
    fn suppress_clipboard_capture_matches_normalized_text_once() {
        let mut list = SUPPRESSED_CLIPBOARD
            .lock()
            .expect("suppressed clipboard lock");
        list.clear();
        drop(list);

        suppress_clipboard_capture("密码123\r\n").expect("suppress clipboard text");

        let normalized_hash = hash_bytes(normalize_text_for_hash("密码123\n").as_bytes());
        assert!(should_suppress_capture_hash(&normalized_hash));
        assert!(!should_suppress_capture_hash(&normalized_hash));
    }

    #[test]
    fn copy_image_path_validation_returns_clear_errors() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("lazycat-inbox-copy-image-{unique}"));
        fs::create_dir_all(&temp_dir).expect("create temp dir");

        let missing_path = temp_dir.join("missing.png");
        let missing_err = super::action_copy_image(&json!({
            "path": missing_path.to_string_lossy().to_string(),
        }))
        .expect_err("missing path should fail");
        assert_eq!(missing_err, "图像文件不存在");

        let directory_err = super::action_copy_image(&json!({
            "path": temp_dir.to_string_lossy().to_string(),
        }))
        .expect_err("directory path should fail");
        assert_eq!(directory_err, "图像路径必须是文件");

        fs::remove_dir_all(temp_dir).expect("cleanup temp dir");
    }
}

use rusqlite::{params, Connection};
use serde_json::{json, Value};

use super::helpers::db_conn;

const INIT_KEY: &str = "snippet_workspace_v2_initialized";

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "v2_init" => v2_init(payload),
        "v2_list" | "list" => v2_list(payload),
        "v2_get" | "get" => v2_get(payload),
        "v2_create" | "create" => v2_create(payload),
        "v2_update" | "update" => v2_update(payload),
        "v2_delete" | "delete" => v2_delete(payload),
        "v2_search" | "search" => v2_search(payload),
        "v2_mark_used" => v2_mark_used(payload),
        "v2_tag_stats" | "tags" => v2_tag_stats(),
        "v2_folder_list" | "folder_list" => v2_folder_list(),
        "v2_folder_create" | "folder_create" => v2_folder_create(payload),
        "v2_folder_update" | "folder_update" => v2_folder_update(payload),
        "v2_folder_delete" | "folder_delete" => v2_folder_delete(payload),
        "toggle_favorite" => toggle_favorite(payload),
        "language_stats" => language_stats(),
        "batch_update" => batch_update(payload),
        "batch_delete" => batch_delete(payload),
        _ => Err(format!("unsupported snippets action: {action}")),
    }
}

fn parse_ids(payload: &Value) -> Result<Vec<i64>, String> {
    let ids = payload["ids"]
        .as_array()
        .ok_or("ids is required and must be an array")?;
    let mut out = Vec::new();
    for v in ids {
        if let Some(id) = v.as_i64() {
            if id > 0 && !out.contains(&id) {
                out.push(id);
            }
        }
    }
    if out.is_empty() {
        return Err("ids is empty".to_string());
    }
    Ok(out)
}

fn parse_tags(payload: &Value, key: &str) -> Vec<String> {
    payload[key]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
        })
        .unwrap_or_default()
}

fn parse_fragments(payload: &Value) -> Vec<(String, String, String)> {
    let mut frags = payload["fragments"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|f| {
                    (
                        f["label"].as_str().unwrap_or("main").to_string(),
                        f["language"].as_str().unwrap_or("plaintext").to_string(),
                        f["code"].as_str().unwrap_or_default().to_string(),
                    )
                })
                .collect::<Vec<(String, String, String)>>()
        })
        .unwrap_or_default();

    if frags.is_empty() {
        frags.push((
            "main".to_string(),
            "plaintext".to_string(),
            String::new(),
        ));
    }

    frags
}

fn parse_sort(payload: &Value) -> &'static str {
    match payload["sort_by"].as_str().unwrap_or("last_used") {
        "updated_at" => "se.updated_at DESC",
        "created_at" => "se.created_at DESC",
        "title" => "se.title COLLATE NOCASE ASC",
        _ => "se.last_used_at DESC, se.use_count DESC, se.updated_at DESC",
    }
}

fn has_fts(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT count(*) > 0 FROM sqlite_master WHERE type='table' AND name='snippet_fts'",
        [],
        |r| r.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

fn set_initialized(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "INSERT INTO user_settings (key, value, updated_at) VALUES (?1, '1', CURRENT_TIMESTAMP)
         ON CONFLICT(key) DO UPDATE SET value='1', updated_at=CURRENT_TIMESTAMP",
        params![INIT_KEY],
    )
    .map_err(|e| format!("set init flag failed: {e}"))?;
    Ok(())
}

fn is_initialized(conn: &Connection) -> Result<bool, String> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM user_settings WHERE key = ?1",
            params![INIT_KEY],
            |r| r.get(0),
        )
        .ok();
    Ok(value.as_deref() == Some("1"))
}

fn reset_v2_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS snippet_entry_tags;
         DROP TABLE IF EXISTS snippet_fragments_v2;
         DROP TABLE IF EXISTS snippet_entries;
         DROP TABLE IF EXISTS snippet_folders_v2;
         DROP TABLE IF EXISTS snippet_fts;

         CREATE TABLE IF NOT EXISTS snippet_folders_v2 (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            parent_id INTEGER DEFAULT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (parent_id) REFERENCES snippet_folders_v2(id) ON DELETE CASCADE
         );

         CREATE TABLE IF NOT EXISTS snippet_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            folder_id INTEGER DEFAULT NULL,
            is_favorite INTEGER NOT NULL DEFAULT 0,
            primary_language TEXT NOT NULL DEFAULT 'plaintext',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_used_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            use_count INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (folder_id) REFERENCES snippet_folders_v2(id) ON DELETE SET NULL
         );

         CREATE INDEX IF NOT EXISTS idx_entries_last_used_at ON snippet_entries(last_used_at DESC);
         CREATE INDEX IF NOT EXISTS idx_entries_updated_at ON snippet_entries(updated_at DESC);

         CREATE TABLE IF NOT EXISTS snippet_fragments_v2 (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entry_id INTEGER NOT NULL,
            label TEXT NOT NULL DEFAULT 'main',
            language TEXT NOT NULL DEFAULT 'plaintext',
            code TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (entry_id) REFERENCES snippet_entries(id) ON DELETE CASCADE
         );

         CREATE INDEX IF NOT EXISTS idx_fragments_v2_entry_sort ON snippet_fragments_v2(entry_id, sort_order);

         CREATE TABLE IF NOT EXISTS snippet_entry_tags (
            entry_id INTEGER NOT NULL,
            tag TEXT NOT NULL,
            PRIMARY KEY (entry_id, tag),
            FOREIGN KEY (entry_id) REFERENCES snippet_entries(id) ON DELETE CASCADE
         );

         CREATE INDEX IF NOT EXISTS idx_entry_tags_tag ON snippet_entry_tags(tag);",
    )
    .map_err(|e| format!("reset v2 schema failed: {e}"))?;
    let _ = conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS snippet_fts USING fts5(
            entry_id UNINDEXED,
            title,
            description,
            tags_text,
            code_text
         );",
    );

    Ok(())
}

fn rebuild_fts_for_entry(conn: &Connection, entry_id: i64) -> Result<(), String> {
    if !has_fts(conn) {
        return Ok(());
    }

    conn.execute("DELETE FROM snippet_fts WHERE entry_id = ?1", params![entry_id])
        .map_err(|e| format!("delete fts row failed: {e}"))?;

    let row = conn
        .query_row(
            "SELECT
                se.title,
                se.description,
                COALESCE((
                    SELECT GROUP_CONCAT(tag, ' ')
                    FROM snippet_entry_tags et
                    WHERE et.entry_id = se.id
                ), ''),
                COALESCE((
                    SELECT GROUP_CONCAT(code, ' ')
                    FROM snippet_fragments_v2 sf
                    WHERE sf.entry_id = se.id
                ), '')
             FROM snippet_entries se
             WHERE se.id = ?1",
            params![entry_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )
        .ok();

    if let Some((title, description, tags_text, code_text)) = row {
        conn.execute(
            "INSERT INTO snippet_fts(entry_id, title, description, tags_text, code_text) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![entry_id, title, description, tags_text, code_text],
        )
        .map_err(|e| format!("insert fts row failed: {e}"))?;
    }

    Ok(())
}

fn collect_tags(conn: &Connection, entry_id: i64) -> Result<Value, String> {
    let mut stmt = conn
        .prepare("SELECT tag FROM snippet_entry_tags WHERE entry_id = ?1 ORDER BY tag ASC")
        .map_err(|e| format!("prepare tags failed: {e}"))?;
    let rows = stmt
        .query_map(params![entry_id], |r| r.get::<_, String>(0))
        .map_err(|e| format!("query tags failed: {e}"))?;

    let mut tags = Vec::new();
    for row in rows {
        tags.push(Value::String(row.map_err(|e| e.to_string())?));
    }
    Ok(Value::Array(tags))
}

fn collect_fragments(conn: &Connection, entry_id: i64) -> Result<Value, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, label, language, code, sort_order
             FROM snippet_fragments_v2
             WHERE entry_id = ?1
             ORDER BY sort_order ASC",
        )
        .map_err(|e| format!("prepare fragments failed: {e}"))?;

    let rows = stmt
        .query_map(params![entry_id], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "label": r.get::<_, String>(1)?,
                "language": r.get::<_, String>(2)?,
                "code": r.get::<_, String>(3)?,
                "sortOrder": r.get::<_, i64>(4)?,
            }))
        })
        .map_err(|e| format!("query fragments failed: {e}"))?;

    let mut frags = Vec::new();
    for row in rows {
        frags.push(row.map_err(|e| e.to_string())?);
    }
    Ok(Value::Array(frags))
}

fn entry_summary_row_to_json(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": row.get::<_, i64>(0)?,
        "title": row.get::<_, String>(1)?,
        "description": row.get::<_, String>(2)?,
        "folderId": row.get::<_, Option<i64>>(3)?,
        "isFavorite": row.get::<_, i64>(4)? == 1,
        "primaryLanguage": row.get::<_, String>(5)?,
        "createdAt": row.get::<_, String>(6)?,
        "updatedAt": row.get::<_, String>(7)?,
        "lastUsedAt": row.get::<_, String>(8)?,
        "useCount": row.get::<_, i64>(9)?,
        "fragmentCount": row.get::<_, i64>(10)?,
        "tagCsv": row.get::<_, Option<String>>(11)?.unwrap_or_default(),
    }))
}

fn row_with_tags(mut row: Value) -> Value {
    let tags = row["tagCsv"]
        .as_str()
        .unwrap_or_default()
        .split('\u{1f}')
        .filter(|s| !s.is_empty())
        .map(|s| Value::String(s.to_string()))
        .collect::<Vec<Value>>();
    row["tags"] = Value::Array(tags);
    row.as_object_mut().map(|obj| obj.remove("tagCsv"));
    row
}

fn v2_init(payload: &Value) -> Result<Value, String> {
    let confirm = payload["confirm"].as_bool().unwrap_or(false);
    let conn = db_conn()?;

    if is_initialized(&conn)? {
        return Ok(json!({ "initialized": true, "requiresConfirm": false }));
    }

    if !confirm {
        return Ok(json!({
            "initialized": false,
            "requiresConfirm": true,
            "message": "首次进入将清空旧代码片段数据并重建工作区。"
        }));
    }

    reset_v2_schema(&conn)?;
    set_initialized(&conn)?;
    Ok(json!({ "initialized": true, "requiresConfirm": false }))
}

fn v2_list(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let folder_id = payload["folder_id"].as_i64();
    let tag = payload["tag"].as_str().unwrap_or_default();
    let favorite_only = payload["favorite_only"].as_bool().unwrap_or(false);
    let untagged_only = payload["untagged_only"].as_bool().unwrap_or(false);
    let recent_days = payload["recent_days"].as_i64().unwrap_or(0);
    let order_clause = parse_sort(payload);

    let mut conditions = Vec::new();
    let mut params_boxed: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(fid) = folder_id {
        conditions.push("se.folder_id = ?".to_string());
        params_boxed.push(Box::new(fid));
    }
    if !tag.is_empty() {
        conditions.push("EXISTS (SELECT 1 FROM snippet_entry_tags et2 WHERE et2.entry_id = se.id AND et2.tag = ?)".to_string());
        params_boxed.push(Box::new(tag.to_string()));
    }
    if favorite_only {
        conditions.push("se.is_favorite = 1".to_string());
    }
    if untagged_only {
        conditions.push("NOT EXISTS (SELECT 1 FROM snippet_entry_tags et3 WHERE et3.entry_id = se.id)".to_string());
    }
    if recent_days > 0 {
        conditions.push("se.last_used_at >= datetime('now', ?)".to_string());
        params_boxed.push(Box::new(format!("-{recent_days} days")));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT
            se.id,
            se.title,
            se.description,
            se.folder_id,
            se.is_favorite,
            se.primary_language,
            se.created_at,
            se.updated_at,
            se.last_used_at,
            se.use_count,
            (SELECT COUNT(*) FROM snippet_fragments_v2 sf WHERE sf.entry_id = se.id) AS fragment_count,
            (
                SELECT GROUP_CONCAT(tag, CHAR(31))
                FROM (
                    SELECT tag FROM snippet_entry_tags et
                    WHERE et.entry_id = se.id
                    ORDER BY tag ASC
                )
            ) AS tag_csv
         FROM snippet_entries se
         {}
         ORDER BY {}",
        where_clause, order_clause
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| format!("prepare v2_list failed: {e}"))?;
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params_boxed.iter().map(|p| p.as_ref()).collect();
    let rows = stmt
        .query_map(params_ref.as_slice(), entry_summary_row_to_json)
        .map_err(|e| format!("query v2_list failed: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row_with_tags(row.map_err(|e| e.to_string())?));
    }

    Ok(Value::Array(out))
}

fn v2_get(payload: &Value) -> Result<Value, String> {
    let entry_id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;

    let mut snippet = conn
        .query_row(
            "SELECT id, title, description, folder_id, is_favorite, primary_language, created_at, updated_at, last_used_at, use_count
             FROM snippet_entries WHERE id = ?1",
            params![entry_id],
            |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "title": r.get::<_, String>(1)?,
                    "description": r.get::<_, String>(2)?,
                    "folderId": r.get::<_, Option<i64>>(3)?,
                    "isFavorite": r.get::<_, i64>(4)? == 1,
                    "primaryLanguage": r.get::<_, String>(5)?,
                    "createdAt": r.get::<_, String>(6)?,
                    "updatedAt": r.get::<_, String>(7)?,
                    "lastUsedAt": r.get::<_, String>(8)?,
                    "useCount": r.get::<_, i64>(9)?,
                }))
            },
        )
        .map_err(|e| format!("v2_get not found: {e}"))?;

    snippet["tags"] = collect_tags(&conn, entry_id)?;
    snippet["fragments"] = collect_fragments(&conn, entry_id)?;
    Ok(snippet)
}

fn v2_create(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let title = payload["title"].as_str().unwrap_or_default();
    let description = payload["description"].as_str().unwrap_or_default();
    let folder_id = payload["folderId"].as_i64();
    let is_favorite = payload["isFavorite"].as_bool().unwrap_or(false);
    let fragments = parse_fragments(payload);
    let tags = parse_tags(payload, "tags");

    let primary_language = fragments
        .first()
        .map(|(_, language, _)| language.clone())
        .unwrap_or_else(|| "plaintext".to_string());

    conn.execute(
        "INSERT INTO snippet_entries (title, description, folder_id, is_favorite, primary_language, last_used_at)
         VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)",
        params![
            title,
            description,
            folder_id,
            if is_favorite { 1 } else { 0 },
            primary_language
        ],
    )
    .map_err(|e| format!("v2_create entry failed: {e}"))?;

    let entry_id = conn.last_insert_rowid();

    for (idx, (label, language, code)) in fragments.iter().enumerate() {
        conn.execute(
            "INSERT INTO snippet_fragments_v2 (entry_id, label, language, code, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![entry_id, label, language, code, idx as i64],
        )
        .map_err(|e| format!("v2_create fragments failed: {e}"))?;
    }

    for tag in &tags {
        conn.execute(
            "INSERT OR IGNORE INTO snippet_entry_tags (entry_id, tag) VALUES (?1, ?2)",
            params![entry_id, tag],
        )
        .map_err(|e| format!("v2_create tags failed: {e}"))?;
    }

    rebuild_fts_for_entry(&conn, entry_id)?;
    v2_get(&json!({ "id": entry_id }))
}

fn v2_update(payload: &Value) -> Result<Value, String> {
    let entry_id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;

    if let Some(title) = payload["title"].as_str() {
        conn.execute(
            "UPDATE snippet_entries SET title = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![title.trim(), entry_id],
        )
        .map_err(|e| format!("v2_update title failed: {e}"))?;
    }

    if payload.get("description").is_some() {
        conn.execute(
            "UPDATE snippet_entries SET description = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![payload["description"].as_str().unwrap_or_default(), entry_id],
        )
        .map_err(|e| format!("v2_update description failed: {e}"))?;
    }

    if payload.get("folderId").is_some() {
        conn.execute(
            "UPDATE snippet_entries SET folder_id = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![payload["folderId"].as_i64(), entry_id],
        )
        .map_err(|e| format!("v2_update folder failed: {e}"))?;
    }

    if payload.get("isFavorite").is_some() {
        let flag = payload["isFavorite"].as_bool().unwrap_or(false);
        conn.execute(
            "UPDATE snippet_entries SET is_favorite = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![if flag { 1 } else { 0 }, entry_id],
        )
        .map_err(|e| format!("v2_update favorite failed: {e}"))?;
    }

    if payload.get("fragments").is_some() {
        let frags = parse_fragments(payload);
        let primary_language = frags
            .first()
            .map(|(_, language, _)| language.clone())
            .unwrap_or_else(|| "plaintext".to_string());

        conn.execute(
            "DELETE FROM snippet_fragments_v2 WHERE entry_id = ?1",
            params![entry_id],
        )
        .map_err(|e| format!("v2_update clear fragments failed: {e}"))?;

        for (idx, (label, language, code)) in frags.iter().enumerate() {
            conn.execute(
                "INSERT INTO snippet_fragments_v2 (entry_id, label, language, code, sort_order)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![entry_id, label, language, code, idx as i64],
            )
            .map_err(|e| format!("v2_update insert fragment failed: {e}"))?;
        }

        conn.execute(
            "UPDATE snippet_entries SET primary_language = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![primary_language, entry_id],
        )
        .map_err(|e| format!("v2_update primary language failed: {e}"))?;
    }

    if payload.get("tags").is_some() {
        let tags = parse_tags(payload, "tags");
        conn.execute(
            "DELETE FROM snippet_entry_tags WHERE entry_id = ?1",
            params![entry_id],
        )
        .map_err(|e| format!("v2_update clear tags failed: {e}"))?;

        for tag in &tags {
            conn.execute(
                "INSERT OR IGNORE INTO snippet_entry_tags (entry_id, tag) VALUES (?1, ?2)",
                params![entry_id, tag],
            )
            .map_err(|e| format!("v2_update insert tag failed: {e}"))?;
        }
    }

    rebuild_fts_for_entry(&conn, entry_id)?;
    v2_get(&json!({ "id": entry_id }))
}

fn v2_delete(payload: &Value) -> Result<Value, String> {
    let entry_id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;
    conn.execute("DELETE FROM snippet_entries WHERE id = ?1", params![entry_id])
        .map_err(|e| format!("v2_delete failed: {e}"))?;
    conn.execute("DELETE FROM snippet_fts WHERE entry_id = ?1", params![entry_id])
        .ok();
    Ok(json!({ "ok": true }))
}

fn v2_mark_used(payload: &Value) -> Result<Value, String> {
    let entry_id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;
    conn.execute(
        "UPDATE snippet_entries
         SET last_used_at = CURRENT_TIMESTAMP,
             use_count = use_count + 1,
             updated_at = CURRENT_TIMESTAMP
         WHERE id = ?1",
        params![entry_id],
    )
    .map_err(|e| format!("v2_mark_used failed: {e}"))?;

    Ok(json!({ "ok": true }))
}

fn v2_search(payload: &Value) -> Result<Value, String> {
    let keyword = payload["keyword"].as_str().unwrap_or_default().trim();
    if keyword.is_empty() {
        return v2_list(payload);
    }

    let conn = db_conn()?;
    let folder_id = payload["folder_id"].as_i64();
    let tag = payload["tag"].as_str().unwrap_or_default();
    let favorite_only = payload["favorite_only"].as_bool().unwrap_or(false);
    let untagged_only = payload["untagged_only"].as_bool().unwrap_or(false);
    let recent_days = payload["recent_days"].as_i64().unwrap_or(0);
    let order_clause = parse_sort(payload);

    let mut conditions = Vec::new();
    let mut params_boxed: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if has_fts(&conn) {
        conditions.push("se.id IN (SELECT entry_id FROM snippet_fts WHERE snippet_fts MATCH ?)".to_string());
        params_boxed.push(Box::new(keyword.to_string()));
    } else {
        let like = format!("%{keyword}%");
        conditions.push(
            "(
                se.title LIKE ? OR
                se.description LIKE ? OR
                EXISTS (SELECT 1 FROM snippet_entry_tags etf WHERE etf.entry_id = se.id AND etf.tag LIKE ?) OR
                EXISTS (SELECT 1 FROM snippet_fragments_v2 sff WHERE sff.entry_id = se.id AND sff.code LIKE ?)
             )"
                .to_string(),
        );
        params_boxed.push(Box::new(like.clone()));
        params_boxed.push(Box::new(like.clone()));
        params_boxed.push(Box::new(like.clone()));
        params_boxed.push(Box::new(like));
    }

    if let Some(fid) = folder_id {
        conditions.push("se.folder_id = ?".to_string());
        params_boxed.push(Box::new(fid));
    }
    if !tag.is_empty() {
        conditions.push("EXISTS (SELECT 1 FROM snippet_entry_tags et2 WHERE et2.entry_id = se.id AND et2.tag = ?)".to_string());
        params_boxed.push(Box::new(tag.to_string()));
    }
    if favorite_only {
        conditions.push("se.is_favorite = 1".to_string());
    }
    if untagged_only {
        conditions.push("NOT EXISTS (SELECT 1 FROM snippet_entry_tags et3 WHERE et3.entry_id = se.id)".to_string());
    }
    if recent_days > 0 {
        conditions.push("se.last_used_at >= datetime('now', ?)".to_string());
        params_boxed.push(Box::new(format!("-{recent_days} days")));
    }

    let where_clause = format!("WHERE {}", conditions.join(" AND "));

    let sql = format!(
        "SELECT
            se.id,
            se.title,
            se.description,
            se.folder_id,
            se.is_favorite,
            se.primary_language,
            se.created_at,
            se.updated_at,
            se.last_used_at,
            se.use_count,
            (SELECT COUNT(*) FROM snippet_fragments_v2 sf WHERE sf.entry_id = se.id) AS fragment_count,
            (
                SELECT GROUP_CONCAT(tag, CHAR(31))
                FROM (
                    SELECT tag FROM snippet_entry_tags et
                    WHERE et.entry_id = se.id
                    ORDER BY tag ASC
                )
            ) AS tag_csv
         FROM snippet_entries se
         {}
         ORDER BY {}",
        where_clause, order_clause
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("prepare v2_search failed: {e}"))?;
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params_boxed.iter().map(|p| p.as_ref()).collect();
    let rows = stmt
        .query_map(params_ref.as_slice(), entry_summary_row_to_json)
        .map_err(|e| format!("query v2_search failed: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row_with_tags(row.map_err(|e| e.to_string())?));
    }

    Ok(Value::Array(out))
}

fn v2_tag_stats() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT tag, COUNT(*) AS cnt
             FROM snippet_entry_tags
             GROUP BY tag
             ORDER BY cnt DESC, tag ASC",
        )
        .map_err(|e| format!("prepare v2_tag_stats failed: {e}"))?;

    let rows = stmt
        .query_map([], |r| {
            Ok(json!({
                "tag": r.get::<_, String>(0)?,
                "count": r.get::<_, i64>(1)?,
            }))
        })
        .map_err(|e| format!("query v2_tag_stats failed: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(Value::Array(out))
}

fn v2_folder_list() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT
                f.id,
                f.name,
                f.parent_id,
                f.sort_order,
                f.created_at,
                COUNT(se.id) AS snippet_count
             FROM snippet_folders_v2 f
             LEFT JOIN snippet_entries se ON se.folder_id = f.id
             GROUP BY f.id, f.name, f.parent_id, f.sort_order, f.created_at
             ORDER BY f.sort_order ASC, f.id ASC",
        )
        .map_err(|e| format!("prepare v2_folder_list failed: {e}"))?;

    let rows = stmt
        .query_map([], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "name": r.get::<_, String>(1)?,
                "parentId": r.get::<_, Option<i64>>(2)?,
                "sortOrder": r.get::<_, i64>(3)?,
                "createdAt": r.get::<_, String>(4)?,
                "snippetCount": r.get::<_, i64>(5)?,
            }))
        })
        .map_err(|e| format!("query v2_folder_list failed: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }

    Ok(Value::Array(out))
}

fn v2_folder_create(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let name = payload["name"].as_str().unwrap_or("新建文件夹").trim();
    let parent_id = payload["parentId"].as_i64();
    let next_order: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM snippet_folders_v2",
            [],
            |r| r.get(0),
        )
        .unwrap_or(1);

    conn.execute(
        "INSERT INTO snippet_folders_v2 (name, parent_id, sort_order) VALUES (?1, ?2, ?3)",
        params![if name.is_empty() { "新建文件夹" } else { name }, parent_id, next_order],
    )
    .map_err(|e| format!("v2_folder_create failed: {e}"))?;

    Ok(json!({ "ok": true, "id": conn.last_insert_rowid() }))
}

fn v2_folder_update(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let folder_id = payload["id"].as_i64().ok_or("id is required")?;

    if let Some(name) = payload["name"].as_str() {
        conn.execute(
            "UPDATE snippet_folders_v2 SET name = ?1 WHERE id = ?2",
            params![if name.trim().is_empty() { "未命名文件夹" } else { name.trim() }, folder_id],
        )
        .map_err(|e| format!("v2_folder_update name failed: {e}"))?;
    }

    if payload.get("parentId").is_some() {
        conn.execute(
            "UPDATE snippet_folders_v2 SET parent_id = ?1 WHERE id = ?2",
            params![payload["parentId"].as_i64(), folder_id],
        )
        .map_err(|e| format!("v2_folder_update parent failed: {e}"))?;
    }

    Ok(json!({ "ok": true }))
}

fn v2_folder_delete(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let folder_id = payload["id"].as_i64().ok_or("id is required")?;

    conn.execute(
        "UPDATE snippet_entries SET folder_id = NULL WHERE folder_id = ?1",
        params![folder_id],
    )
    .map_err(|e| format!("v2_folder_delete unlink failed: {e}"))?;

    conn.execute(
        "DELETE FROM snippet_folders_v2 WHERE id = ?1",
        params![folder_id],
    )
    .map_err(|e| format!("v2_folder_delete failed: {e}"))?;

    Ok(json!({ "ok": true }))
}

fn toggle_favorite(payload: &Value) -> Result<Value, String> {
    let entry_id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;
    conn.execute(
        "UPDATE snippet_entries
         SET is_favorite = 1 - is_favorite, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?1",
        params![entry_id],
    )
    .map_err(|e| format!("toggle_favorite failed: {e}"))?;
    let favorite: i64 = conn
        .query_row(
            "SELECT is_favorite FROM snippet_entries WHERE id = ?1",
            params![entry_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("read favorite failed: {e}"))?;
    Ok(json!({ "id": entry_id, "isFavorite": favorite == 1 }))
}

fn language_stats() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT language, COUNT(*) AS cnt
             FROM snippet_fragments_v2
             GROUP BY language
             ORDER BY cnt DESC, language ASC",
        )
        .map_err(|e| format!("prepare language_stats failed: {e}"))?;

    let rows = stmt
        .query_map([], |r| {
            Ok(json!({
                "language": r.get::<_, String>(0)?,
                "count": r.get::<_, i64>(1)?,
            }))
        })
        .map_err(|e| format!("query language_stats failed: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(Value::Array(out))
}

fn batch_update(payload: &Value) -> Result<Value, String> {
    let ids = parse_ids(payload)?;
    let set_favorite = payload.get("setFavorite").and_then(|v| v.as_bool());
    let has_folder = payload.get("folderId").is_some();
    let folder_id = payload["folderId"].as_i64();
    let add_tags = parse_tags(payload, "addTags");
    let remove_tags = parse_tags(payload, "removeTags");

    if set_favorite.is_none() && !has_folder && add_tags.is_empty() && remove_tags.is_empty() {
        return Err("batch_update requires at least one operation".to_string());
    }

    let mut conn = db_conn()?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("batch_update begin tx failed: {e}"))?;

    for id in &ids {
        if let Some(flag) = set_favorite {
            tx.execute(
                "UPDATE snippet_entries
                 SET is_favorite = ?1, updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?2",
                params![if flag { 1 } else { 0 }, id],
            )
            .map_err(|e| format!("batch_update favorite failed: {e}"))?;
        }
        if has_folder {
            tx.execute(
                "UPDATE snippet_entries
                 SET folder_id = ?1, updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?2",
                params![folder_id, id],
            )
            .map_err(|e| format!("batch_update folder failed: {e}"))?;
        }
        for tag in &add_tags {
            tx.execute(
                "INSERT OR IGNORE INTO snippet_entry_tags (entry_id, tag) VALUES (?1, ?2)",
                params![id, tag],
            )
            .map_err(|e| format!("batch_update add tag failed: {e}"))?;
        }
        for tag in &remove_tags {
            tx.execute(
                "DELETE FROM snippet_entry_tags WHERE entry_id = ?1 AND tag = ?2",
                params![id, tag],
            )
            .map_err(|e| format!("batch_update remove tag failed: {e}"))?;
        }
        tx.execute(
            "UPDATE snippet_entries SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
            params![id],
        )
        .map_err(|e| format!("batch_update touch failed: {e}"))?;
    }

    tx.commit()
        .map_err(|e| format!("batch_update commit failed: {e}"))?;

    let conn = db_conn()?;
    for id in &ids {
        rebuild_fts_for_entry(&conn, *id)?;
    }

    Ok(json!({ "ok": true, "affected": ids.len() }))
}

fn batch_delete(payload: &Value) -> Result<Value, String> {
    let ids = parse_ids(payload)?;
    let mut conn = db_conn()?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("batch_delete begin tx failed: {e}"))?;
    let mut affected = 0;
    for id in &ids {
        affected += tx
            .execute("DELETE FROM snippet_entries WHERE id = ?1", params![id])
            .map_err(|e| format!("batch_delete delete failed: {e}"))?;
        tx.execute("DELETE FROM snippet_fts WHERE entry_id = ?1", params![id])
            .ok();
    }
    tx.commit()
        .map_err(|e| format!("batch_delete commit failed: {e}"))?;
    Ok(json!({ "ok": true, "affected": affected }))
}

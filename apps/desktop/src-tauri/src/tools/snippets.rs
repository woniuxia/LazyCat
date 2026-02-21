use rusqlite::params;
use serde_json::{json, Value};

use super::helpers::db_conn;

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "list" => snippet_list(payload),
        "get" => snippet_get(payload),
        "create" => snippet_create(payload),
        "update" => snippet_update(payload),
        "delete" => snippet_delete(payload),
        "toggle_favorite" => snippet_toggle_favorite(payload),
        "folder_list" => folder_list(),
        "folder_create" => folder_create(payload),
        "folder_update" => folder_update(payload),
        "folder_delete" => folder_delete(payload),
        "tags" => tag_list(),
        "language_stats" => language_stats(),
        "search" => snippet_search(payload),
        _ => Err(format!("unsupported snippets action: {action}")),
    }
}

/// List snippets with optional filters: folder_id, tag, language, keyword, is_favorite
/// Supports sort_by: "updated_at" (default), "created_at", "title"
fn snippet_list(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    let folder_id = payload["folder_id"].as_i64();
    let tag = payload["tag"].as_str().unwrap_or_default();
    let language = payload["language"].as_str().unwrap_or_default();
    let is_favorite = payload["is_favorite"].as_bool();
    let sort_by = payload["sort_by"].as_str().unwrap_or("updated_at");

    let order_clause = match sort_by {
        "created_at" => "s.created_at DESC",
        "title" => "s.title ASC",
        _ => "s.updated_at DESC",
    };

    let mut conditions = Vec::new();
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(fid) = folder_id {
        conditions.push("s.folder_id = ?".to_string());
        param_values.push(Box::new(fid));
    }
    if !tag.is_empty() {
        conditions.push(
            "s.id IN (SELECT snippet_id FROM snippet_tags WHERE tag = ?)".to_string(),
        );
        param_values.push(Box::new(tag.to_string()));
    }
    if !language.is_empty() {
        conditions.push(
            "s.id IN (SELECT snippet_id FROM snippet_fragments WHERE language = ?)".to_string(),
        );
        param_values.push(Box::new(language.to_string()));
    }
    if let Some(fav) = is_favorite {
        conditions.push("s.is_favorite = ?".to_string());
        param_values.push(Box::new(fav as i64));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT s.id, s.title, s.description, s.folder_id, s.is_favorite, s.created_at, s.updated_at
         FROM snippets s {} ORDER BY {}",
        where_clause, order_clause
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| format!("prepare snippet list: {e}"))?;
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
    let rows = stmt
        .query_map(params_ref.as_slice(), |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "title": row.get::<_, String>(1)?,
                "description": row.get::<_, String>(2)?,
                "folderId": row.get::<_, Option<i64>>(3)?,
                "isFavorite": row.get::<_, i64>(4)? == 1,
                "createdAt": row.get::<_, String>(5)?,
                "updatedAt": row.get::<_, String>(6)?,
            }))
        })
        .map_err(|e| format!("query snippets: {e}"))?;

    let mut out = Vec::new();
    for r in rows {
        let mut snippet = r.map_err(|e| e.to_string())?;
        let sid = snippet["id"].as_i64().unwrap();
        // Attach tags
        snippet["tags"] = get_snippet_tags(&conn, sid)?;
        // Attach first fragment language for display
        let lang: String = conn
            .query_row(
                "SELECT language FROM snippet_fragments WHERE snippet_id = ?1 ORDER BY sort_order ASC LIMIT 1",
                params![sid],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "plaintext".to_string());
        snippet["language"] = json!(lang);
        out.push(snippet);
    }
    Ok(Value::Array(out))
}

/// Get a single snippet with all fragments and tags
fn snippet_get(payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;

    let mut snippet = conn
        .query_row(
            "SELECT id, title, description, folder_id, is_favorite, created_at, updated_at FROM snippets WHERE id = ?1",
            params![id],
            |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "title": row.get::<_, String>(1)?,
                    "description": row.get::<_, String>(2)?,
                    "folderId": row.get::<_, Option<i64>>(3)?,
                    "isFavorite": row.get::<_, i64>(4)? == 1,
                    "createdAt": row.get::<_, String>(5)?,
                    "updatedAt": row.get::<_, String>(6)?,
                }))
            },
        )
        .map_err(|e| format!("snippet not found: {e}"))?;

    snippet["tags"] = get_snippet_tags(&conn, id)?;
    snippet["fragments"] = get_snippet_fragments(&conn, id)?;
    Ok(snippet)
}

fn get_snippet_tags(conn: &rusqlite::Connection, snippet_id: i64) -> Result<Value, String> {
    let mut stmt = conn
        .prepare("SELECT tag FROM snippet_tags WHERE snippet_id = ?1 ORDER BY tag")
        .map_err(|e| format!("prepare tags: {e}"))?;
    let rows = stmt
        .query_map(params![snippet_id], |row| row.get::<_, String>(0))
        .map_err(|e| format!("query tags: {e}"))?;
    let mut tags = Vec::new();
    for r in rows {
        tags.push(Value::String(r.map_err(|e| e.to_string())?));
    }
    Ok(Value::Array(tags))
}

fn get_snippet_fragments(conn: &rusqlite::Connection, snippet_id: i64) -> Result<Value, String> {
    let mut stmt = conn
        .prepare("SELECT id, label, language, code, sort_order FROM snippet_fragments WHERE snippet_id = ?1 ORDER BY sort_order ASC")
        .map_err(|e| format!("prepare fragments: {e}"))?;
    let rows = stmt
        .query_map(params![snippet_id], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "label": row.get::<_, String>(1)?,
                "language": row.get::<_, String>(2)?,
                "code": row.get::<_, String>(3)?,
                "sortOrder": row.get::<_, i64>(4)?,
            }))
        })
        .map_err(|e| format!("query fragments: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(Value::Array(out))
}

/// Create a snippet with fragments and tags
fn snippet_create(payload: &Value) -> Result<Value, String> {
    let title = payload["title"].as_str().unwrap_or("未命名片段");
    let description = payload["description"].as_str().unwrap_or_default();
    let folder_id = payload["folderId"].as_i64();
    let conn = db_conn()?;

    conn.execute(
        "INSERT INTO snippets (title, description, folder_id) VALUES (?1, ?2, ?3)",
        params![title, description, folder_id],
    )
    .map_err(|e| format!("create snippet: {e}"))?;
    let snippet_id = conn.last_insert_rowid();

    // Insert fragments
    let fragments = payload["fragments"].as_array();
    if let Some(frags) = fragments {
        for (i, f) in frags.iter().enumerate() {
            let label = f["label"].as_str().unwrap_or("main");
            let language = f["language"].as_str().unwrap_or("plaintext");
            let code = f["code"].as_str().unwrap_or_default();
            conn.execute(
                "INSERT INTO snippet_fragments (snippet_id, label, language, code, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![snippet_id, label, language, code, i as i64],
            )
            .map_err(|e| format!("create fragment: {e}"))?;
        }
    } else {
        // Default: one empty fragment
        conn.execute(
            "INSERT INTO snippet_fragments (snippet_id, label, language, code, sort_order) VALUES (?1, 'main', 'plaintext', '', 0)",
            params![snippet_id],
        )
        .map_err(|e| format!("create default fragment: {e}"))?;
    }

    // Insert tags
    if let Some(tags) = payload["tags"].as_array() {
        for t in tags {
            if let Some(tag) = t.as_str() {
                if !tag.is_empty() {
                    conn.execute(
                        "INSERT OR IGNORE INTO snippet_tags (snippet_id, tag) VALUES (?1, ?2)",
                        params![snippet_id, tag],
                    )
                    .map_err(|e| format!("create tag: {e}"))?;
                }
            }
        }
    }

    snippet_get(&json!({"id": snippet_id}))
}

/// Update snippet: full replace of fragments and tags
fn snippet_update(payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;

    // Update main fields if provided
    if let Some(title) = payload["title"].as_str() {
        conn.execute(
            "UPDATE snippets SET title = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![title, id],
        )
        .map_err(|e| format!("update title: {e}"))?;
    }
    if payload.get("description").is_some() {
        let desc = payload["description"].as_str().unwrap_or_default();
        conn.execute(
            "UPDATE snippets SET description = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![desc, id],
        )
        .map_err(|e| format!("update description: {e}"))?;
    }
    if payload.get("folderId").is_some() {
        let folder_id = payload["folderId"].as_i64();
        conn.execute(
            "UPDATE snippets SET folder_id = ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![folder_id, id],
        )
        .map_err(|e| format!("update folder: {e}"))?;
    }

    // Replace fragments if provided
    if let Some(frags) = payload["fragments"].as_array() {
        conn.execute("DELETE FROM snippet_fragments WHERE snippet_id = ?1", params![id])
            .map_err(|e| format!("clear fragments: {e}"))?;
        for (i, f) in frags.iter().enumerate() {
            let label = f["label"].as_str().unwrap_or("main");
            let language = f["language"].as_str().unwrap_or("plaintext");
            let code = f["code"].as_str().unwrap_or_default();
            conn.execute(
                "INSERT INTO snippet_fragments (snippet_id, label, language, code, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, label, language, code, i as i64],
            )
            .map_err(|e| format!("insert fragment: {e}"))?;
        }
    }

    // Replace tags if provided
    if let Some(tags) = payload["tags"].as_array() {
        conn.execute("DELETE FROM snippet_tags WHERE snippet_id = ?1", params![id])
            .map_err(|e| format!("clear tags: {e}"))?;
        for t in tags {
            if let Some(tag) = t.as_str() {
                if !tag.is_empty() {
                    conn.execute(
                        "INSERT OR IGNORE INTO snippet_tags (snippet_id, tag) VALUES (?1, ?2)",
                        params![id, tag],
                    )
                    .map_err(|e| format!("insert tag: {e}"))?;
                }
            }
        }
    }

    // Touch updated_at
    conn.execute(
        "UPDATE snippets SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
        params![id],
    )
    .map_err(|e| format!("touch updated_at: {e}"))?;

    snippet_get(&json!({"id": id}))
}

fn snippet_delete(payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;
    conn.execute("DELETE FROM snippets WHERE id = ?1", params![id])
        .map_err(|e| format!("delete snippet: {e}"))?;
    Ok(json!({"ok": true}))
}

fn snippet_toggle_favorite(payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;
    conn.execute(
        "UPDATE snippets SET is_favorite = 1 - is_favorite, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
        params![id],
    )
    .map_err(|e| format!("toggle favorite: {e}"))?;
    let fav: i64 = conn
        .query_row("SELECT is_favorite FROM snippets WHERE id = ?1", params![id], |r| r.get(0))
        .map_err(|e| format!("read favorite: {e}"))?;
    Ok(json!({"id": id, "isFavorite": fav == 1}))
}

/// List all folders as flat list (frontend builds tree)
fn folder_list() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare("SELECT id, name, parent_id, sort_order, created_at FROM snippet_folders ORDER BY sort_order ASC, id ASC")
        .map_err(|e| format!("prepare folder list: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "parentId": row.get::<_, Option<i64>>(2)?,
                "sortOrder": row.get::<_, i64>(3)?,
                "createdAt": row.get::<_, String>(4)?,
            }))
        })
        .map_err(|e| format!("query folders: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        let mut folder = r.map_err(|e| e.to_string())?;
        let fid = folder["id"].as_i64().unwrap();
        // Count snippets in this folder
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM snippets WHERE folder_id = ?1", params![fid], |r| r.get(0))
            .unwrap_or(0);
        folder["snippetCount"] = json!(count);
        out.push(folder);
    }
    Ok(Value::Array(out))
}

fn folder_create(payload: &Value) -> Result<Value, String> {
    let name = payload["name"].as_str().unwrap_or("新建文件夹");
    let parent_id = payload["parentId"].as_i64();
    let conn = db_conn()?;
    let next_order: i64 = conn
        .query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM snippet_folders", [], |r| r.get(0))
        .unwrap_or(0);
    conn.execute(
        "INSERT INTO snippet_folders (name, parent_id, sort_order) VALUES (?1, ?2, ?3)",
        params![name, parent_id, next_order],
    )
    .map_err(|e| format!("create folder: {e}"))?;
    let id = conn.last_insert_rowid();
    Ok(json!({"id": id, "name": name, "parentId": parent_id, "sortOrder": next_order}))
}

fn folder_update(payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;
    if let Some(name) = payload["name"].as_str() {
        conn.execute("UPDATE snippet_folders SET name = ?1 WHERE id = ?2", params![name, id])
            .map_err(|e| format!("rename folder: {e}"))?;
    }
    if payload.get("parentId").is_some() {
        let parent_id = payload["parentId"].as_i64();
        conn.execute("UPDATE snippet_folders SET parent_id = ?1 WHERE id = ?2", params![parent_id, id])
            .map_err(|e| format!("move folder: {e}"))?;
    }
    Ok(json!({"ok": true}))
}

fn folder_delete(payload: &Value) -> Result<Value, String> {
    let id = payload["id"].as_i64().ok_or("id is required")?;
    let conn = db_conn()?;
    // Set snippets in this folder to no folder
    conn.execute("UPDATE snippets SET folder_id = NULL WHERE folder_id = ?1", params![id])
        .map_err(|e| format!("unlink snippets: {e}"))?;
    // Also unlink snippets in child folders
    conn.execute("UPDATE snippets SET folder_id = NULL WHERE folder_id IN (SELECT id FROM snippet_folders WHERE parent_id = ?1)", params![id])
        .map_err(|e| format!("unlink child snippets: {e}"))?;
    conn.execute("DELETE FROM snippet_folders WHERE id = ?1", params![id])
        .map_err(|e| format!("delete folder: {e}"))?;
    Ok(json!({"ok": true}))
}

/// List all tags with usage count
fn tag_list() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare("SELECT tag, COUNT(*) as cnt FROM snippet_tags GROUP BY tag ORDER BY cnt DESC, tag ASC")
        .map_err(|e| format!("prepare tag list: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(json!({
                "tag": row.get::<_, String>(0)?,
                "count": row.get::<_, i64>(1)?,
            }))
        })
        .map_err(|e| format!("query tags: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(Value::Array(out))
}

fn snippet_search(payload: &Value) -> Result<Value, String> {
    let keyword = payload["keyword"].as_str().unwrap_or_default();
    if keyword.is_empty() {
        return snippet_list(&json!({}));
    }
    let like = format!("%{}%", keyword);
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT s.id, s.title, s.description, s.folder_id, s.is_favorite, s.created_at, s.updated_at
             FROM snippets s
             LEFT JOIN snippet_fragments sf ON sf.snippet_id = s.id
             LEFT JOIN snippet_tags st ON st.snippet_id = s.id
             WHERE s.title LIKE ?1
                OR s.description LIKE ?1
                OR sf.code LIKE ?1
                OR st.tag LIKE ?1
             ORDER BY s.updated_at DESC"
        )
        .map_err(|e| format!("prepare search: {e}"))?;
    let rows = stmt
        .query_map(params![like], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "title": row.get::<_, String>(1)?,
                "description": row.get::<_, String>(2)?,
                "folderId": row.get::<_, Option<i64>>(3)?,
                "isFavorite": row.get::<_, i64>(4)? == 1,
                "createdAt": row.get::<_, String>(5)?,
                "updatedAt": row.get::<_, String>(6)?,
            }))
        })
        .map_err(|e| format!("search snippets: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        let mut snippet = r.map_err(|e| e.to_string())?;
        let sid = snippet["id"].as_i64().unwrap();
        snippet["tags"] = get_snippet_tags(&conn, sid)?;
        let lang: String = conn
            .query_row(
                "SELECT language FROM snippet_fragments WHERE snippet_id = ?1 ORDER BY sort_order ASC LIMIT 1",
                params![sid],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "plaintext".to_string());
        snippet["language"] = json!(lang);
        out.push(snippet);
    }
    Ok(Value::Array(out))
}

/// Count fragments per language, sorted by count descending
fn language_stats() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare("SELECT language, COUNT(*) as cnt FROM snippet_fragments GROUP BY language ORDER BY cnt DESC, language ASC")
        .map_err(|e| format!("prepare language stats: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(json!({
                "language": row.get::<_, String>(0)?,
                "count": row.get::<_, i64>(1)?,
            }))
        })
        .map_err(|e| format!("query language stats: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(Value::Array(out))
}

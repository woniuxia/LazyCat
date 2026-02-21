use rusqlite::{Connection, params};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

/// Fixed base directory: ~/.lazycat (always exists, never changes)
pub fn get_base_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("home dir not found".to_string())?;
    let p = home.join(".lazycat");
    fs::create_dir_all(&p).map_err(|e| format!("create base dir failed: {e}"))?;
    Ok(p)
}

/// Fixed config pointer file: ~/.lazycat/config.json
pub fn get_config_path() -> Result<PathBuf, String> {
    Ok(get_base_dir()?.join("config.json"))
}

/// Actual data directory: reads config.json for custom path, falls back to base dir
pub fn get_data_dir() -> Result<PathBuf, String> {
    let base = get_base_dir()?;
    let config_path = base.join("config.json");
    if config_path.exists() {
        if let Ok(content) = fs::read_to_string(&config_path) {
            if let Ok(obj) = serde_json::from_str::<Value>(&content) {
                if let Some(custom) = obj["data_dir"].as_str() {
                    let custom_path = PathBuf::from(custom);
                    // Verify the custom path is accessible
                    if custom_path.is_dir() {
                        return Ok(custom_path);
                    }
                    // Custom path not reachable, silently fall back to base
                }
            }
        }
    }
    Ok(base)
}

fn get_schema_version(conn: &Connection) -> i64 {
    // Check if schema_version table exists
    let exists: bool = conn
        .query_row(
            "SELECT count(*) > 0 FROM sqlite_master WHERE type='table' AND name='schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if !exists {
        return 0;
    }
    conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_version", [], |row| {
        row.get(0)
    })
    .unwrap_or(0)
}

fn set_schema_version(conn: &Connection, version: i64) -> Result<(), String> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);")
        .map_err(|e| format!("create schema_version table failed: {e}"))?;
    conn.execute("DELETE FROM schema_version", [])
        .map_err(|e| format!("clear schema_version failed: {e}"))?;
    conn.execute("INSERT INTO schema_version (version) VALUES (?1)", params![version])
        .map_err(|e| format!("set schema_version failed: {e}"))?;
    Ok(())
}

fn run_migrations(conn: &Connection) -> Result<(), String> {
    let current = get_schema_version(conn);

    // Migration 1: hosts_profiles table (already exists via CREATE IF NOT EXISTS)
    if current < 1 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS hosts_profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                content TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );"
        )
        .map_err(|e| format!("migration 1 failed: {e}"))?;
        set_schema_version(conn, 1)?;
    }

    // Migration 2: user_settings table
    if current < 2 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS user_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );"
        )
        .map_err(|e| format!("migration 2 failed: {e}"))?;
        set_schema_version(conn, 2)?;
    }

    // Migration 3: hosts_profiles add sort_order column
    if current < 3 {
        conn.execute_batch(
            "ALTER TABLE hosts_profiles ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;"
        )
        .map_err(|e| format!("migration 3 failed: {e}"))?;
        // Initialize sort_order based on existing id order
        conn.execute_batch(
            "UPDATE hosts_profiles SET sort_order = id;"
        )
        .map_err(|e| format!("migration 3 init sort_order failed: {e}"))?;
        set_schema_version(conn, 3)?;
    }

    // Migration 4: snippet_folders table
    if current < 4 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS snippet_folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                parent_id INTEGER DEFAULT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (parent_id) REFERENCES snippet_folders(id) ON DELETE CASCADE
            );"
        )
        .map_err(|e| format!("migration 4 failed: {e}"))?;
        set_schema_version(conn, 4)?;
    }

    // Migration 5: snippets table
    if current < 5 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS snippets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                folder_id INTEGER DEFAULT NULL,
                is_favorite INTEGER NOT NULL DEFAULT 0,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (folder_id) REFERENCES snippet_folders(id) ON DELETE SET NULL
            );"
        )
        .map_err(|e| format!("migration 5 failed: {e}"))?;
        set_schema_version(conn, 5)?;
    }

    // Migration 6: snippet_fragments table (multi-tab code)
    if current < 6 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS snippet_fragments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                snippet_id INTEGER NOT NULL,
                label TEXT NOT NULL DEFAULT 'main',
                language TEXT NOT NULL DEFAULT 'plaintext',
                code TEXT NOT NULL DEFAULT '',
                sort_order INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (snippet_id) REFERENCES snippets(id) ON DELETE CASCADE
            );"
        )
        .map_err(|e| format!("migration 6 failed: {e}"))?;
        set_schema_version(conn, 6)?;
    }

    // Migration 7: snippet_tags table
    if current < 7 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS snippet_tags (
                snippet_id INTEGER NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (snippet_id, tag),
                FOREIGN KEY (snippet_id) REFERENCES snippets(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_snippet_tags_tag ON snippet_tags(tag);"
        )
        .map_err(|e| format!("migration 7 failed: {e}"))?;
        set_schema_version(conn, 7)?;
    }

    Ok(())
}

pub fn db_conn() -> Result<Connection, String> {
    let db_path = get_data_dir()?.join("lazycat.sqlite");
    let conn = Connection::open(db_path).map_err(|e| format!("open db failed: {e}"))?;
    run_migrations(&conn)?;
    Ok(conn)
}

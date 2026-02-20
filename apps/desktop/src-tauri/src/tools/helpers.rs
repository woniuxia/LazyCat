use rusqlite::{Connection};
use std::fs;
use std::path::PathBuf;

pub fn get_data_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("home dir not found".to_string())?;
    let p = home.join(".lazycat");
    fs::create_dir_all(&p).map_err(|e| format!("create data dir failed: {e}"))?;
    Ok(p)
}

pub fn db_conn() -> Result<Connection, String> {
    let db_path = get_data_dir()?.join("lazycat.sqlite");
    let conn = Connection::open(db_path).map_err(|e| format!("open db failed: {e}"))?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS hosts_profiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            content TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
    ",
    )
    .map_err(|e| format!("init db failed: {e}"))?;
    Ok(conn)
}

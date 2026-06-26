use rusqlite::{params, Connection};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

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
    static DATA_DIR_CACHE: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    let cache = DATA_DIR_CACHE.get_or_init(|| Mutex::new(None));

    if let Some(cached) = cache
        .lock()
        .map_err(|e| format!("data dir cache lock failed: {e}"))?
        .clone()
    {
        return Ok(cached);
    }

    let base = get_base_dir()?;
    let config_path = base.join("config.json");
    let resolved = if config_path.exists() {
        if let Ok(content) = fs::read_to_string(&config_path) {
            if let Ok(obj) = serde_json::from_str::<Value>(&content) {
                if let Some(custom) = obj["data_dir"].as_str() {
                    let custom_path = PathBuf::from(custom);
                    if custom_path.is_dir() {
                        custom_path
                    } else {
                        base.clone()
                    }
                } else {
                    base.clone()
                }
            } else {
                base.clone()
            }
        } else {
            base.clone()
        }
    } else {
        base.clone()
    };

    let mut guard = cache
        .lock()
        .map_err(|e| format!("data dir cache lock failed: {e}"))?;
    if let Some(cached) = guard.clone() {
        return Ok(cached);
    }
    *guard = Some(resolved.clone());
    Ok(resolved)
}

fn initialize_connection(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|e| format!("initialize connection failed: {e}"))
}

fn ensure_process_schema(conn: &Connection) -> Result<(), String> {
    static SCHEMA_READY: AtomicBool = AtomicBool::new(false);
    static SCHEMA_INIT_LOCK: Mutex<()> = Mutex::new(());

    if SCHEMA_READY.load(Ordering::Acquire) {
        return Ok(());
    }

    let _guard = SCHEMA_INIT_LOCK
        .lock()
        .map_err(|e| format!("schema init lock failed: {e}"))?;

    if SCHEMA_READY.load(Ordering::Acquire) {
        return Ok(());
    }

    ensure_schema(conn)?;
    SCHEMA_READY.store(true, Ordering::Release);
    Ok(())
}

fn ensure_schema(conn: &Connection) -> Result<(), String> {
    // 前置修补：为 schema 26 旧数据库补齐 completed_at 列。
    // ALTER TABLE ADD COLUMN 无 IF NOT EXISTS 语法，表不存在或列已存在均会报错，直接忽略。
    let _ = conn.execute_batch("ALTER TABLE todo_items ADD COLUMN completed_at TEXT DEFAULT NULL;");
    let _ =
        conn.execute_batch("ALTER TABLE data_dictionaries ADD COLUMN sort_field_path TEXT DEFAULT NULL;");
    let _ =
        conn.execute_batch("ALTER TABLE data_dictionaries ADD COLUMN sort_direction TEXT NOT NULL DEFAULT 'asc';");
    let _ =
        conn.execute_batch("ALTER TABLE data_dictionaries ADD COLUMN nav_order INTEGER NOT NULL DEFAULT 0;");

    // Phase 2: Add project_id to todo_items
    let _ =
        conn.execute_batch("ALTER TABLE todo_items ADD COLUMN project_id INTEGER DEFAULT NULL;");

    // PM-SiYuan integration: project-level default location override
    let _ = conn
        .execute_batch("ALTER TABLE pm_projects ADD COLUMN siyuan_notebook_id TEXT DEFAULT NULL;");
    let _ = conn.execute_batch(
        "ALTER TABLE pm_projects ADD COLUMN siyuan_notebook_name TEXT DEFAULT NULL;",
    );
    let _ = conn.execute_batch(
        "ALTER TABLE pm_projects ADD COLUMN siyuan_parent_doc_id TEXT DEFAULT NULL;",
    );
    let _ = conn.execute_batch(
        "ALTER TABLE pm_projects ADD COLUMN siyuan_parent_doc_title TEXT DEFAULT NULL;",
    );
    let _ = conn
        .execute_batch("ALTER TABLE pm_projects ADD COLUMN siyuan_parent_hpath TEXT DEFAULT NULL;");
    let _ = conn
        .execute_batch("ALTER TABLE pm_projects ADD COLUMN siyuan_parent_path TEXT DEFAULT NULL;");

    // Status flow timestamps
    let _ = conn.execute_batch("ALTER TABLE pm_items ADD COLUMN started_at TEXT DEFAULT NULL;");
    let _ = conn.execute_batch("ALTER TABLE pm_items ADD COLUMN testing_at TEXT DEFAULT NULL;");

    // PM-SiYuan integration: item primary page cache
    let _ = conn.execute_batch("ALTER TABLE pm_items ADD COLUMN siyuan_doc_id TEXT DEFAULT NULL;");
    let _ =
        conn.execute_batch("ALTER TABLE pm_items ADD COLUMN siyuan_doc_title TEXT DEFAULT NULL;");
    let _ =
        conn.execute_batch("ALTER TABLE pm_items ADD COLUMN siyuan_doc_hpath TEXT DEFAULT NULL;");
    let _ =
        conn.execute_batch("ALTER TABLE pm_items ADD COLUMN siyuan_doc_path TEXT DEFAULT NULL;");
    let _ =
        conn.execute_batch("ALTER TABLE pm_items ADD COLUMN siyuan_notebook_id TEXT DEFAULT NULL;");
    let _ = conn
        .execute_batch("ALTER TABLE pm_items ADD COLUMN siyuan_notebook_name TEXT DEFAULT NULL;");
    let _ = conn.execute_batch("ALTER TABLE pm_items ADD COLUMN link_url TEXT DEFAULT NULL;");
    let _ = conn.execute_batch("ALTER TABLE pm_items ADD COLUMN ref_code TEXT DEFAULT NULL;");

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS hosts_profiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            content TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 0,
            sort_order INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_hosts_profiles_enabled_sort
            ON hosts_profiles(enabled DESC, sort_order ASC, id ASC);

        CREATE TABLE IF NOT EXISTS user_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS data_dictionaries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            record_count INTEGER NOT NULL DEFAULT 0,
            sort_field_path TEXT DEFAULT NULL,
            sort_direction TEXT NOT NULL DEFAULT 'asc',
            nav_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_data_dictionaries_nav_order
            ON data_dictionaries(nav_order ASC, updated_at DESC, id DESC);
        CREATE INDEX IF NOT EXISTS idx_data_dictionaries_updated_at
            ON data_dictionaries(updated_at DESC);

        CREATE TABLE IF NOT EXISTS data_dictionary_fields (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            dictionary_id INTEGER NOT NULL,
            field_path TEXT NOT NULL,
            display_name TEXT NOT NULL DEFAULT '',
            meaning TEXT NOT NULL DEFAULT '',
            searchable INTEGER NOT NULL DEFAULT 1,
            visible INTEGER NOT NULL DEFAULT 1,
            sort_order INTEGER NOT NULL DEFAULT 0,
            type_hint TEXT NOT NULL DEFAULT 'unknown',
            sample_value TEXT NOT NULL DEFAULT '',
            present_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(dictionary_id, field_path),
            FOREIGN KEY (dictionary_id) REFERENCES data_dictionaries(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_data_dictionary_fields_dictionary_sort
            ON data_dictionary_fields(dictionary_id, sort_order, field_path);

        CREATE TABLE IF NOT EXISTS data_dictionary_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            dictionary_id INTEGER NOT NULL,
            row_index INTEGER NOT NULL,
            raw_json TEXT NOT NULL,
            search_text TEXT NOT NULL,
            normalized_search_text TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (dictionary_id) REFERENCES data_dictionaries(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_data_dictionary_records_dictionary
            ON data_dictionary_records(dictionary_id, row_index);

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
        CREATE INDEX IF NOT EXISTS idx_fragments_v2_entry_sort
            ON snippet_fragments_v2(entry_id, sort_order);

        CREATE TABLE IF NOT EXISTS snippet_entry_tags (
            entry_id INTEGER NOT NULL,
            tag TEXT NOT NULL,
            PRIMARY KEY (entry_id, tag),
            FOREIGN KEY (entry_id) REFERENCES snippet_entries(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_entry_tags_tag ON snippet_entry_tags(tag);

        CREATE TABLE IF NOT EXISTS vault_canary (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            salt TEXT NOT NULL,
            iv TEXT NOT NULL,
            encrypted TEXT NOT NULL,
            iterations INTEGER NOT NULL DEFAULT 600000,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS vault_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            category TEXT NOT NULL CHECK (category IN ('app', 'server', 'database')),
            title TEXT NOT NULL DEFAULT '',
            environment TEXT NOT NULL DEFAULT '',
            iv TEXT NOT NULL,
            encrypted_blob TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            view_count INTEGER NOT NULL DEFAULT 0,
            copy_count INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_vault_category ON vault_entries(category);

        CREATE TABLE IF NOT EXISTS vault_entry_tags (
            entry_id INTEGER NOT NULL,
            tag TEXT NOT NULL,
            PRIMARY KEY (entry_id, tag),
            FOREIGN KEY (entry_id) REFERENCES vault_entries(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_vault_entry_tags_tag ON vault_entry_tags(tag);

        CREATE TABLE IF NOT EXISTS launcher_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            exe_path TEXT NOT NULL,
            arguments TEXT NOT NULL DEFAULT '',
            icon_base64 TEXT NOT NULL DEFAULT '',
            group_name TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0,
            launch_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_launcher_exe_path ON launcher_entries(exe_path);

        CREATE TABLE IF NOT EXISTS todo_types (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            color TEXT NOT NULL DEFAULT '#409eff',
            builtin INTEGER NOT NULL DEFAULT 0,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_todo_types_builtin_sort
            ON todo_types(builtin DESC, sort_order ASC, id ASC);

        CREATE TABLE IF NOT EXISTS todo_assignees (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS todo_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            type_id INTEGER DEFAULT NULL,
            priority TEXT NOT NULL DEFAULT 'P2' CHECK (priority IN ('P0', 'P1', 'P2', 'P3')),
            description TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'in_progress', 'completed')),
            event_at TEXT DEFAULT NULL,
            pinned INTEGER NOT NULL DEFAULT 0,
            kind TEXT NOT NULL DEFAULT 'one_off' CHECK (kind IN ('one_off', 'recurring')),
            parent_id INTEGER DEFAULT NULL,
            series_id INTEGER DEFAULT NULL,
            remind_at TEXT DEFAULT NULL,
            snooze_until TEXT DEFAULT NULL,
            last_notified_at TEXT DEFAULT NULL,
            completed_at TEXT DEFAULT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (type_id) REFERENCES todo_types(id) ON DELETE SET NULL,
            FOREIGN KEY (parent_id) REFERENCES todo_items(id) ON DELETE SET NULL
        );
        CREATE INDEX IF NOT EXISTS idx_todo_items_status ON todo_items(status);
        CREATE INDEX IF NOT EXISTS idx_todo_items_event_at ON todo_items(event_at);
        CREATE INDEX IF NOT EXISTS idx_todo_items_kind ON todo_items(kind);
        CREATE INDEX IF NOT EXISTS idx_todo_items_series_id ON todo_items(series_id);
        CREATE INDEX IF NOT EXISTS idx_todo_items_parent_id ON todo_items(parent_id);
        CREATE INDEX IF NOT EXISTS idx_todo_items_completed_at ON todo_items(completed_at);
        CREATE INDEX IF NOT EXISTS idx_todo_items_project_id ON todo_items(project_id);

        CREATE TABLE IF NOT EXISTS todo_series_rules (
            series_id INTEGER PRIMARY KEY,
            rule_mode TEXT NOT NULL,
            rule_json TEXT,
            cron_expression TEXT,
            timezone TEXT,
            start_at TEXT,
            end_mode TEXT NOT NULL DEFAULT 'never',
            end_value TEXT,
            occurrence_index INTEGER NOT NULL DEFAULT 1,
            active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (series_id) REFERENCES todo_items(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS todo_item_assignees (
            item_id INTEGER NOT NULL,
            assignee_id INTEGER NOT NULL,
            PRIMARY KEY (item_id, assignee_id),
            FOREIGN KEY (item_id) REFERENCES todo_items(id) ON DELETE CASCADE,
            FOREIGN KEY (assignee_id) REFERENCES todo_assignees(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_todo_item_assignees_assignee
            ON todo_item_assignees(assignee_id);

        CREATE TABLE IF NOT EXISTS todo_item_reminders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            item_id INTEGER NOT NULL,
            reminder_preset TEXT NOT NULL,
            offset_minutes INTEGER NOT NULL,
            remind_at TEXT NOT NULL,
            snooze_until TEXT DEFAULT NULL,
            last_notified_at TEXT DEFAULT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (item_id) REFERENCES todo_items(id) ON DELETE CASCADE,
            UNIQUE(item_id, reminder_preset)
        );
        CREATE INDEX IF NOT EXISTS idx_todo_item_reminders_item
            ON todo_item_reminders(item_id, offset_minutes, id);
        CREATE INDEX IF NOT EXISTS idx_todo_item_reminders_fire
            ON todo_item_reminders(remind_at, snooze_until, last_notified_at, id);

        CREATE TABLE IF NOT EXISTS todo_item_links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            item_id INTEGER NOT NULL,
            url TEXT NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (item_id) REFERENCES todo_items(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_todo_item_links_item
            ON todo_item_links(item_id, sort_order);

        CREATE TABLE IF NOT EXISTS todo_reminder_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id INTEGER NOT NULL,
            task_reminder_id INTEGER DEFAULT NULL,
            title TEXT NOT NULL,
            body TEXT NOT NULL DEFAULT '',
            fire_at TEXT NOT NULL,
            is_read INTEGER NOT NULL DEFAULT 0,
            reminder_preset TEXT DEFAULT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (task_id) REFERENCES todo_items(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_todo_reminder_events_unread
            ON todo_reminder_events(is_read, fire_at DESC, id DESC);

        CREATE TABLE IF NOT EXISTS inbox_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            bucket TEXT NOT NULL CHECK(bucket IN ('history', 'inbox', 'archived')),
            item_type TEXT NOT NULL,
            storage_kind TEXT NOT NULL CHECK(storage_kind IN ('inline', 'external', 'metadata_only')),
            title TEXT NOT NULL DEFAULT '',
            preview TEXT NOT NULL DEFAULT '',
            search_text TEXT NOT NULL DEFAULT '',
            payload_ref TEXT,
            byte_size INTEGER NOT NULL DEFAULT 0,
            content_hash TEXT NOT NULL,
            captured_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            seen_count INTEGER NOT NULL DEFAULT 1,
            note TEXT NOT NULL DEFAULT '',
            starred INTEGER NOT NULL DEFAULT 0,
            meta_json TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_inbox_items_bucket_last_seen
            ON inbox_items(bucket, last_seen_at DESC, id DESC);
        CREATE INDEX IF NOT EXISTS idx_inbox_items_hash_type
            ON inbox_items(content_hash, item_type);
        CREATE INDEX IF NOT EXISTS idx_inbox_items_type
            ON inbox_items(item_type);

        CREATE TABLE IF NOT EXISTS inbox_file_refs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            inbox_item_id INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            file_name TEXT NOT NULL,
            file_size INTEGER,
            modified_at TEXT,
            FOREIGN KEY (inbox_item_id) REFERENCES inbox_items(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_inbox_file_refs_item
            ON inbox_file_refs(inbox_item_id);

        CREATE TABLE IF NOT EXISTS inbox_asset_refs (
            content_hash TEXT PRIMARY KEY,
            file_path TEXT NOT NULL,
            ref_count INTEGER NOT NULL DEFAULT 1,
            byte_size INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS pm_projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            color TEXT NOT NULL DEFAULT '#409eff',
            status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
            siyuan_notebook_id TEXT DEFAULT NULL,
            siyuan_notebook_name TEXT DEFAULT NULL,
            siyuan_parent_doc_id TEXT DEFAULT NULL,
            siyuan_parent_doc_title TEXT DEFAULT NULL,
            siyuan_parent_hpath TEXT DEFAULT NULL,
            siyuan_parent_path TEXT DEFAULT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_pm_projects_status_sort
            ON pm_projects(status, sort_order ASC, id DESC);

        CREATE TABLE IF NOT EXISTS pm_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id INTEGER NOT NULL,
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            link_url TEXT DEFAULT NULL,
            ref_code TEXT DEFAULT NULL,
            item_type TEXT NOT NULL DEFAULT 'task' CHECK (item_type IN ('task', 'bug', 'feature', 'improvement')),
            priority TEXT NOT NULL DEFAULT 'P2' CHECK (priority IN ('P0', 'P1', 'P2', 'P3')),
            status TEXT NOT NULL DEFAULT 'todo' CHECK (status IN ('todo', 'in_progress', 'testing', 'done')),
            start_at TEXT DEFAULT NULL,
            end_at TEXT DEFAULT NULL,
            pinned INTEGER NOT NULL DEFAULT 0,
            sort_order INTEGER NOT NULL DEFAULT 0,
            siyuan_doc_id TEXT DEFAULT NULL,
            siyuan_doc_title TEXT DEFAULT NULL,
            siyuan_doc_hpath TEXT DEFAULT NULL,
            siyuan_doc_path TEXT DEFAULT NULL,
            siyuan_notebook_id TEXT DEFAULT NULL,
            siyuan_notebook_name TEXT DEFAULT NULL,
            started_at TEXT DEFAULT NULL,
            testing_at TEXT DEFAULT NULL,
            completed_at TEXT DEFAULT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (project_id) REFERENCES pm_projects(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_pm_items_project_status
            ON pm_items(project_id, status);
        CREATE INDEX IF NOT EXISTS idx_pm_items_completed_at
            ON pm_items(completed_at);
        CREATE INDEX IF NOT EXISTS idx_pm_items_end_at
            ON pm_items(end_at);
        CREATE INDEX IF NOT EXISTS idx_pm_items_status
            ON pm_items(status);
        CREATE INDEX IF NOT EXISTS idx_pm_items_updated_at
            ON pm_items(updated_at);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_pm_items_ref_code
            ON pm_items(ref_code) WHERE ref_code IS NOT NULL;

        CREATE TABLE IF NOT EXISTS pm_item_tags (
            item_id INTEGER NOT NULL,
            tag TEXT NOT NULL,
            PRIMARY KEY (item_id, tag),
            FOREIGN KEY (item_id) REFERENCES pm_items(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_pm_item_tags_tag ON pm_item_tags(tag);",
    )
    .map_err(|e| format!("initialize schema failed: {e}"))?;

    let _ = conn.execute_batch(
        "ALTER TABLE vault_entries ADD COLUMN view_count INTEGER NOT NULL DEFAULT 0;",
    );
    let _ = conn.execute_batch(
        "ALTER TABLE vault_entries ADD COLUMN copy_count INTEGER NOT NULL DEFAULT 0;",
    );
    let _ =
        conn.execute_batch("ALTER TABLE vault_entries ADD COLUMN plain_fields TEXT DEFAULT NULL;");

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS pm_item_siyuan_links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            item_id INTEGER NOT NULL,
            doc_id TEXT NOT NULL,
            doc_title TEXT NOT NULL,
            doc_hpath TEXT NOT NULL,
            doc_path TEXT DEFAULT NULL,
            notebook_id TEXT NOT NULL,
            notebook_name TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (item_id) REFERENCES pm_items(id) ON DELETE CASCADE,
            UNIQUE(item_id, doc_id)
        );
        CREATE INDEX IF NOT EXISTS idx_pm_item_siyuan_links_item
            ON pm_item_siyuan_links(item_id, sort_order ASC, id ASC);",
    )
    .map_err(|e| format!("initialize pm_item_siyuan_links failed: {e}"))?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS pm_item_todo_links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            pm_item_id INTEGER NOT NULL,
            todo_item_id INTEGER NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (pm_item_id) REFERENCES pm_items(id) ON DELETE CASCADE,
            FOREIGN KEY (todo_item_id) REFERENCES todo_items(id) ON DELETE CASCADE
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_pm_item_todo_links_todo_item_id
            ON pm_item_todo_links(todo_item_id);
        CREATE INDEX IF NOT EXISTS idx_pm_item_todo_links_pm_item_id
            ON pm_item_todo_links(pm_item_id);",
    )
    .map_err(|e| format!("initialize pm_item_todo_links failed: {e}"))?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS attachments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            owner_type TEXT NOT NULL,
            owner_id   TEXT NOT NULL,
            rel_path   TEXT NOT NULL,
            original_name TEXT NOT NULL DEFAULT '',
            mime       TEXT NOT NULL DEFAULT '',
            size       INTEGER NOT NULL DEFAULT 0,
            hash       TEXT NOT NULL DEFAULT '',
            kind       TEXT NOT NULL DEFAULT 'file',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_attachments_owner ON attachments(owner_type, owner_id);
        CREATE INDEX IF NOT EXISTS idx_attachments_hash  ON attachments(hash);",
    )
    .map_err(|e| format!("initialize attachments failed: {e}"))?;

    let fts_result = conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS snippet_fts USING fts5(
            entry_id UNINDEXED,
            title,
            description,
            tags_text,
            code_text
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS data_dictionary_fts USING fts5(
            record_id UNINDEXED,
            dictionary_id UNINDEXED,
            search_text,
            tokenize='unicode61 remove_diacritics 2'
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS inbox_fts USING fts5(
            title,
            preview,
            note,
            search_text,
            content='inbox_items',
            content_rowid='id',
            tokenize='unicode61 remove_diacritics 2'
        );

        CREATE TRIGGER IF NOT EXISTS inbox_fts_insert AFTER INSERT ON inbox_items BEGIN
            INSERT INTO inbox_fts(rowid, title, preview, note, search_text)
            VALUES (new.id, new.title, new.preview, new.note, new.search_text);
        END;

        CREATE TRIGGER IF NOT EXISTS inbox_fts_update AFTER UPDATE ON inbox_items BEGIN
            INSERT INTO inbox_fts(inbox_fts, rowid, title, preview, note, search_text)
            VALUES('delete', old.id, old.title, old.preview, old.note, old.search_text);
            INSERT INTO inbox_fts(rowid, title, preview, note, search_text)
            VALUES(new.id, new.title, new.preview, new.note, new.search_text);
        END;

        CREATE TRIGGER IF NOT EXISTS inbox_fts_delete AFTER DELETE ON inbox_items BEGIN
            INSERT INTO inbox_fts(inbox_fts, rowid, title, preview, note, search_text)
            VALUES('delete', old.id, old.title, old.preview, old.note, old.search_text);
        END;",
    );
    if fts_result.is_err() {
        let _ = conn.execute_batch(
            "DROP TRIGGER IF EXISTS inbox_fts_insert;
             DROP TRIGGER IF EXISTS inbox_fts_update;
             DROP TRIGGER IF EXISTS inbox_fts_delete;
             DROP TABLE IF EXISTS inbox_fts;
             DROP TABLE IF EXISTS data_dictionary_fts;",
        );
    }

    for (name, color, sort_order) in [
        ("待报事项", "#409eff", 10_i64),
        ("工作任务", "#67c23a", 20_i64),
        ("会议安排", "#e6a23c", 30_i64),
        ("个人事项", "#f56c6c", 40_i64),
    ] {
        conn.execute(
            "INSERT OR IGNORE INTO todo_types (name, color, builtin, sort_order) VALUES (?1, ?2, 1, ?3)",
            params![name, color, sort_order],
        )
        .map_err(|e| format!("seed todo types failed: {e}"))?;
    }

    Ok(())
}

pub fn db_conn() -> Result<Connection, String> {
    let db_path = get_data_dir()?.join("lazycat.sqlite");
    let conn = Connection::open(db_path).map_err(|e| format!("open db failed: {e}"))?;
    initialize_connection(&conn)?;
    ensure_process_schema(&conn)?;
    Ok(conn)
}

/// 附件物理目录：<data_dir>/attachments
/// 首次访问会自动创建目录；跨平台用 fs 层，目录名始终是 "attachments"。
pub fn get_attachments_dir() -> Result<PathBuf, String> {
    let dir = get_data_dir()?.join("attachments");
    fs::create_dir_all(&dir).map_err(|e| format!("create attachments dir failed: {e}"))?;
    Ok(dir)
}

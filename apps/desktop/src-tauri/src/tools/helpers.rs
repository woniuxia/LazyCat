use chrono::{DateTime, Duration, Local, NaiveDateTime, Utc};
use chrono_tz::Tz;
use cron::Schedule;
use rusqlite::{Connection, params};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

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

fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool, String> {
    let sql = format!("PRAGMA table_info({table})");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("prepare table info failed: {e}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| format!("query table info failed: {e}"))?;
    for row in rows {
        if row.map_err(|e| e.to_string())? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn parse_utc_datetime(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|| {
            NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
        })
}

fn compute_next_occurrence(
    cron_expression: &str,
    timezone: &str,
    after_utc: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let schedule = Schedule::from_str(cron_expression).ok()?;

    if timezone.eq_ignore_ascii_case("utc") {
        return schedule.after(&after_utc).next();
    }
    if timezone.eq_ignore_ascii_case("local") {
        let local_after = after_utc.with_timezone(&Local);
        let next = schedule.after(&local_after).next();
        return next.map(|dt| dt.with_timezone(&Utc));
    }
    match timezone.parse::<Tz>() {
        Ok(tz) => {
            let tz_after = after_utc.with_timezone(&tz);
            schedule
                .after(&tz_after)
                .next()
                .map(|dt| dt.with_timezone(&Utc))
        }
        Err(_) => {
            let local_after = after_utc.with_timezone(&Local);
            let next = schedule.after(&local_after).next();
            next.map(|dt| dt.with_timezone(&Utc))
        }
    }
}

fn compute_next_occurrence_with_start(
    cron_expression: &str,
    timezone: &str,
    start_at: Option<&str>,
    after_utc: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let Some(start_at_dt) = start_at.and_then(parse_utc_datetime) else {
        return compute_next_occurrence(cron_expression, timezone, after_utc);
    };
    let search_after = if after_utc <= start_at_dt {
        start_at_dt - Duration::seconds(1)
    } else {
        after_utc
    };
    let next = compute_next_occurrence(cron_expression, timezone, search_after)?;
    (next >= start_at_dt).then_some(next)
}

fn build_simple_weekly_cron_expression(rule: &Value) -> Option<String> {
    let time = rule.get("time").and_then(Value::as_str)?.trim();
    let parts = time.split(':').collect::<Vec<&str>>();
    if parts.len() != 2 {
        return None;
    }
    let hour = parts[0].parse::<i64>().ok()?;
    let minute = parts[1].parse::<i64>().ok()?;
    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) || minute % 5 != 0 {
        return None;
    }

    // 前端 weekday 语义为 1=周一 ... 7=周日；这里输出 Mon..Sun，避免 cron 数值周字段歧义。
    let mut weekdays = rule
        .get("weekdays")
        .and_then(Value::as_array)
        .map(|arr| {
            let mut out = arr
                .iter()
                .filter_map(Value::as_i64)
                .filter(|v| (1..=7).contains(v))
                .collect::<Vec<i64>>();
            out.sort_unstable();
            out.dedup();
            out
        })
        .unwrap_or_else(|| vec![1]);
    if weekdays.is_empty() {
        weekdays = vec![1];
    }

    let dow = if weekdays == vec![1, 2, 3, 4, 5] {
        "Mon-Fri".to_string()
    } else {
        let items = weekdays
            .iter()
            .filter_map(|weekday| match weekday {
                1 => Some("Mon"),
                2 => Some("Tue"),
                3 => Some("Wed"),
                4 => Some("Thu"),
                5 => Some("Fri"),
                6 => Some("Sat"),
                7 => Some("Sun"),
                _ => None,
            })
            .collect::<Vec<&str>>();
        if items.is_empty() {
            "Mon".to_string()
        } else {
            items.join(",")
        }
    };

    Some(format!("0 {minute} {hour} * * {dow}"))
}

fn detect_reminder_offset_minutes(event_at: &str, remind_at: &str) -> Option<i64> {
    const ALLOWED_OFFSETS_MINUTES: [i64; 7] = [0, 5, 10, 30, 60, 24 * 60, 48 * 60];

    let event_dt = parse_utc_datetime(event_at)?;
    let remind_dt = parse_utc_datetime(remind_at)?;
    let diff_seconds = event_dt.signed_duration_since(remind_dt).num_seconds();

    ALLOWED_OFFSETS_MINUTES
        .iter()
        .copied()
        .find(|offset| diff_seconds == offset * 60)
}

fn reminder_preset_from_offset(offset_minutes: i64) -> Option<&'static str> {
    match offset_minutes {
        0 => Some("0m"),
        5 => Some("5m"),
        10 => Some("10m"),
        30 => Some("30m"),
        60 => Some("1h"),
        1440 => Some("1d"),
        2880 => Some("2d"),
        _ => None,
    }
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

    // Migration 8: snippets workspace v2 schema
    if current < 8 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS snippet_folders_v2 (
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
            CREATE INDEX IF NOT EXISTS idx_entry_tags_tag ON snippet_entry_tags(tag);"
        )
        .map_err(|e| format!("migration 8 failed: {e}"))?;
        let _ = conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS snippet_fts USING fts5(
                entry_id UNINDEXED,
                title,
                description,
                tags_text,
                code_text
            );"
        );
        set_schema_version(conn, 8)?;
    }

    // Migration 9: vault tables (password manager)
    if current < 9 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS vault_canary (
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
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS idx_vault_category ON vault_entries(category);"
        )
        .map_err(|e| format!("migration 9 failed: {e}"))?;
        set_schema_version(conn, 9)?;
    }

    // Migration 10: launcher_entries (quick launcher)
    if current < 10 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS launcher_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                exe_path TEXT NOT NULL,
                arguments TEXT NOT NULL DEFAULT '',
                icon_base64 TEXT NOT NULL DEFAULT '',
                group_name TEXT NOT NULL DEFAULT '',
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_launcher_exe_path ON launcher_entries(exe_path);"
        )
        .map_err(|e| format!("migration 10 failed: {e}"))?;
        set_schema_version(conn, 10)?;
    }

    // Migration 11: add launch_count column to launcher_entries
    if current < 11 {
        conn.execute_batch(
            "ALTER TABLE launcher_entries ADD COLUMN launch_count INTEGER NOT NULL DEFAULT 0;"
        )
        .map_err(|e| format!("migration 11 failed: {e}"))?;
        set_schema_version(conn, 11)?;
    }

    // Migration 12: vault_entry_tags table (password manager tags)
    if current < 12 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS vault_entry_tags (
                entry_id INTEGER NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (entry_id, tag),
                FOREIGN KEY (entry_id) REFERENCES vault_entries(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_vault_entry_tags_tag ON vault_entry_tags(tag);"
        )
        .map_err(|e| format!("migration 12 failed: {e}"))?;
        set_schema_version(conn, 12)?;
    }

    // Migration 13: todo & reminder tables
    if current < 13 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS todo_types (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                color TEXT NOT NULL DEFAULT '#409eff',
                builtin INTEGER NOT NULL DEFAULT 0,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS idx_todo_types_builtin_sort ON todo_types(builtin DESC, sort_order ASC, id ASC);

            CREATE TABLE IF NOT EXISTS todo_assignees (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS todo_templates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                type_id INTEGER DEFAULT NULL,
                priority TEXT NOT NULL DEFAULT 'P2' CHECK (priority IN ('P0', 'P1', 'P2', 'P3')),
                description TEXT NOT NULL DEFAULT '',
                rule_mode TEXT NOT NULL DEFAULT 'simple' CHECK (rule_mode IN ('simple', 'cron')),
                rule_json TEXT NOT NULL DEFAULT '{}',
                cron_expression TEXT NOT NULL,
                timezone TEXT NOT NULL DEFAULT 'local',
                end_mode TEXT NOT NULL DEFAULT 'never' CHECK (end_mode IN ('never', 'until_date', 'after_count')),
                end_value TEXT DEFAULT NULL,
                next_occurrence_at TEXT DEFAULT NULL,
                generated_count INTEGER NOT NULL DEFAULT 0,
                active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (type_id) REFERENCES todo_types(id) ON DELETE SET NULL
            );
            CREATE INDEX IF NOT EXISTS idx_todo_templates_active_next ON todo_templates(active, next_occurrence_at);

            CREATE TABLE IF NOT EXISTS todo_template_assignees (
                template_id INTEGER NOT NULL,
                assignee_id INTEGER NOT NULL,
                PRIMARY KEY (template_id, assignee_id),
                FOREIGN KEY (template_id) REFERENCES todo_templates(id) ON DELETE CASCADE,
                FOREIGN KEY (assignee_id) REFERENCES todo_assignees(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS todo_tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                type_id INTEGER DEFAULT NULL,
                priority TEXT NOT NULL DEFAULT 'P2' CHECK (priority IN ('P0', 'P1', 'P2', 'P3')),
                description TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'in_progress', 'completed', 'canceled')),
                due_at TEXT DEFAULT NULL,
                remind_at TEXT DEFAULT NULL,
                snooze_until TEXT DEFAULT NULL,
                last_notified_at TEXT DEFAULT NULL,
                source_template_id INTEGER DEFAULT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (type_id) REFERENCES todo_types(id) ON DELETE SET NULL,
                FOREIGN KEY (source_template_id) REFERENCES todo_templates(id) ON DELETE SET NULL
            );
            CREATE INDEX IF NOT EXISTS idx_todo_tasks_status_priority ON todo_tasks(status, priority);
            CREATE INDEX IF NOT EXISTS idx_todo_tasks_due ON todo_tasks(due_at);
            CREATE INDEX IF NOT EXISTS idx_todo_tasks_remind ON todo_tasks(remind_at, snooze_until, last_notified_at);

            CREATE TABLE IF NOT EXISTS todo_task_assignees (
                task_id INTEGER NOT NULL,
                assignee_id INTEGER NOT NULL,
                PRIMARY KEY (task_id, assignee_id),
                FOREIGN KEY (task_id) REFERENCES todo_tasks(id) ON DELETE CASCADE,
                FOREIGN KEY (assignee_id) REFERENCES todo_assignees(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_todo_task_assignees_assignee ON todo_task_assignees(assignee_id);

            CREATE TABLE IF NOT EXISTS todo_reminder_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                title TEXT NOT NULL,
                body TEXT NOT NULL DEFAULT '',
                fire_at TEXT NOT NULL,
                is_read INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (task_id) REFERENCES todo_tasks(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_todo_reminder_events_unread ON todo_reminder_events(is_read, fire_at DESC, id DESC);"
        )
        .map_err(|e| format!("migration 13 failed: {e}"))?;

        conn.execute(
            "INSERT OR IGNORE INTO todo_types (name, color, builtin, sort_order) VALUES (?1, ?2, 1, ?3)",
            params!["待报事项", "#409eff", 10],
        )
        .map_err(|e| format!("migration 13 seed type failed: {e}"))?;
        conn.execute(
            "INSERT OR IGNORE INTO todo_types (name, color, builtin, sort_order) VALUES (?1, ?2, 1, ?3)",
            params!["工作任务", "#67c23a", 20],
        )
        .map_err(|e| format!("migration 13 seed type failed: {e}"))?;
        conn.execute(
            "INSERT OR IGNORE INTO todo_types (name, color, builtin, sort_order) VALUES (?1, ?2, 1, ?3)",
            params!["会议安排", "#e6a23c", 30],
        )
        .map_err(|e| format!("migration 13 seed type failed: {e}"))?;
        conn.execute(
            "INSERT OR IGNORE INTO todo_types (name, color, builtin, sort_order) VALUES (?1, ?2, 1, ?3)",
            params!["个人事项", "#f56c6c", 40],
        )
        .map_err(|e| format!("migration 13 seed type failed: {e}"))?;

        set_schema_version(conn, 13)?;
    }

    // Migration 14: unify todo template into logical series model
    if current < 14 {
        if !has_column(conn, "todo_templates", "series_kind")? {
            conn.execute_batch(
                "ALTER TABLE todo_templates ADD COLUMN series_kind TEXT NOT NULL DEFAULT 'recurring';
                CREATE INDEX IF NOT EXISTS idx_todo_templates_kind_active_next ON todo_templates(series_kind, active, next_occurrence_at);",
            )
            .map_err(|e| format!("migration 14 alter todo_templates failed: {e}"))?;
        } else {
            conn.execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_todo_templates_kind_active_next ON todo_templates(series_kind, active, next_occurrence_at);",
            )
            .map_err(|e| format!("migration 14 create index failed: {e}"))?;
        }

        conn.execute(
            "UPDATE todo_templates
             SET series_kind='recurring'
             WHERE series_kind IS NULL OR TRIM(series_kind) = ''",
            [],
        )
        .map_err(|e| format!("migration 14 normalize recurring series failed: {e}"))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, title, type_id, priority, description, created_at, updated_at
                 FROM todo_tasks
                 WHERE source_template_id IS NULL
                 ORDER BY id ASC",
            )
            .map_err(|e| format!("migration 14 load orphan tasks failed: {e}"))?;
        let task_rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(|e| format!("migration 14 map orphan tasks failed: {e}"))?;

        let mut orphan_tasks = Vec::new();
        for row in task_rows {
            orphan_tasks.push(row.map_err(|e| e.to_string())?);
        }

        for (task_id, title, type_id, priority, description, created_at, updated_at) in orphan_tasks {
            conn.execute(
                "INSERT INTO todo_templates
                 (title, type_id, priority, description, rule_mode, rule_json, cron_expression,
                  timezone, end_mode, end_value, next_occurrence_at, generated_count, active,
                  created_at, updated_at, series_kind)
                 VALUES(?1, ?2, ?3, ?4, 'simple', '{}', '0 0 0 1 1 *', 'local', 'never', NULL, NULL, 0, 0, ?5, ?6, 'one_off')",
                params![title, type_id, priority, description, created_at, updated_at],
            )
            .map_err(|e| format!("migration 14 create one-off series failed: {e}"))?;
            let series_id = conn.last_insert_rowid();

            conn.execute(
                "INSERT OR IGNORE INTO todo_template_assignees(template_id, assignee_id)
                 SELECT ?1, assignee_id FROM todo_task_assignees WHERE task_id = ?2",
                params![series_id, task_id],
            )
            .map_err(|e| format!("migration 14 copy assignees failed: {e}"))?;

            conn.execute(
                "UPDATE todo_tasks SET source_template_id=?1 WHERE id=?2",
                params![series_id, task_id],
            )
            .map_err(|e| format!("migration 14 bind task series failed: {e}"))?;
        }

        set_schema_version(conn, 14)?;
    }

    // Migration 15: event time + reminder preset model
    if current < 15 {
        if !has_column(conn, "todo_tasks", "event_at")? {
            conn.execute_batch(
                "ALTER TABLE todo_tasks ADD COLUMN event_at TEXT DEFAULT NULL;
                CREATE INDEX IF NOT EXISTS idx_todo_tasks_event ON todo_tasks(event_at);",
            )
            .map_err(|e| format!("migration 15 alter todo_tasks failed: {e}"))?;
        } else {
            conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_todo_tasks_event ON todo_tasks(event_at);")
                .map_err(|e| format!("migration 15 create todo_tasks event index failed: {e}"))?;
        }

        if !has_column(conn, "todo_templates", "reminder_offset_minutes")? {
            conn.execute_batch(
                "ALTER TABLE todo_templates ADD COLUMN reminder_offset_minutes INTEGER DEFAULT NULL;",
            )
            .map_err(|e| format!("migration 15 alter todo_templates failed: {e}"))?;
        }

        conn.execute(
            "UPDATE todo_tasks
             SET event_at = COALESCE(event_at, due_at, remind_at)
             WHERE event_at IS NULL OR TRIM(event_at) = ''",
            [],
        )
        .map_err(|e| format!("migration 15 backfill event_at failed: {e}"))?;

        let mut stmt = conn
            .prepare("SELECT id, event_at, remind_at FROM todo_tasks ORDER BY id ASC")
            .map_err(|e| format!("migration 15 load todo_tasks failed: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|e| format!("migration 15 map todo_tasks failed: {e}"))?;

        let mut task_rows = Vec::new();
        for row in rows {
            task_rows.push(row.map_err(|e| e.to_string())?);
        }

        for (task_id, event_at, remind_at) in task_rows {
            let normalized_event_at = event_at
                .as_deref()
                .and_then(parse_utc_datetime)
                .map(|dt| dt.to_rfc3339())
                .or(event_at);

            let normalized_remind_at = match (
                normalized_event_at.as_deref(),
                remind_at.as_deref().and_then(parse_utc_datetime).map(|dt| dt.to_rfc3339()),
            ) {
                (Some(event_at), Some(remind_at))
                    if detect_reminder_offset_minutes(event_at, &remind_at).is_some() =>
                {
                    Some(remind_at)
                }
                _ => None,
            };

            conn.execute(
                "UPDATE todo_tasks
                 SET event_at=?1, remind_at=?2
                 WHERE id=?3",
                params![normalized_event_at, normalized_remind_at, task_id],
            )
            .map_err(|e| format!("migration 15 normalize todo_task failed: {e}"))?;
        }

        conn.execute(
            "UPDATE todo_templates SET reminder_offset_minutes=NULL",
            [],
        )
        .map_err(|e| format!("migration 15 reset template reminder offsets failed: {e}"))?;

        set_schema_version(conn, 15)?;
    }

    // Migration 16: collapse one-off todo templates back into standalone tasks
    if current < 16 {
        if has_column(conn, "todo_templates", "series_kind")? {
            conn.execute(
                "UPDATE todo_tasks
                 SET source_template_id=NULL
                 WHERE source_template_id IN (
                     SELECT id FROM todo_templates WHERE COALESCE(series_kind, 'recurring')='one_off'
                 )",
                [],
            )
            .map_err(|e| format!("migration 16 detach one-off tasks failed: {e}"))?;

            conn.execute(
                "DELETE FROM todo_template_assignees
                 WHERE template_id IN (
                     SELECT id FROM todo_templates WHERE COALESCE(series_kind, 'recurring')='one_off'
                 )",
                [],
            )
            .map_err(|e| format!("migration 16 delete one-off assignees failed: {e}"))?;

            conn.execute(
                "DELETE FROM todo_templates WHERE COALESCE(series_kind, 'recurring')='one_off'",
                [],
            )
            .map_err(|e| format!("migration 16 delete one-off templates failed: {e}"))?;
        }

        set_schema_version(conn, 16)?;
    }

    // Migration 17: recurring template start time
    if current < 17 {
        if !has_column(conn, "todo_templates", "start_at")? {
            conn.execute_batch("ALTER TABLE todo_templates ADD COLUMN start_at TEXT DEFAULT NULL;")
                .map_err(|e| format!("migration 17 alter todo_templates failed: {e}"))?;
        }

        let mut stmt = conn
            .prepare("SELECT id, start_at, next_occurrence_at, created_at FROM todo_templates ORDER BY id ASC")
            .map_err(|e| format!("migration 17 load todo_templates failed: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| format!("migration 17 map todo_templates failed: {e}"))?;

        let mut template_rows = Vec::new();
        for row in rows {
            template_rows.push(row.map_err(|e| e.to_string())?);
        }

        for (template_id, start_at, next_occurrence_at, created_at) in template_rows {
            let normalized_start_at = start_at
                .as_deref()
                .and_then(parse_utc_datetime)
                .map(|dt| dt.to_rfc3339())
                .or_else(|| {
                    next_occurrence_at
                        .as_deref()
                        .and_then(parse_utc_datetime)
                        .map(|dt| dt.to_rfc3339())
                })
                .or_else(|| parse_utc_datetime(&created_at).map(|dt| dt.to_rfc3339()));

            conn.execute(
                "UPDATE todo_templates SET start_at=?1 WHERE id=?2",
                params![normalized_start_at, template_id],
            )
            .map_err(|e| format!("migration 17 normalize template start time failed: {e}"))?;
        }

        set_schema_version(conn, 17)?;
    }

    // Migration 18: multi reminder tables
    if current < 18 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS todo_template_reminders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                template_id INTEGER NOT NULL,
                reminder_preset TEXT NOT NULL,
                offset_minutes INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (template_id) REFERENCES todo_templates(id) ON DELETE CASCADE,
                UNIQUE(template_id, reminder_preset)
            );
            CREATE INDEX IF NOT EXISTS idx_todo_template_reminders_template ON todo_template_reminders(template_id, offset_minutes, id);

            CREATE TABLE IF NOT EXISTS todo_task_reminders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                reminder_preset TEXT NOT NULL,
                offset_minutes INTEGER NOT NULL,
                remind_at TEXT NOT NULL,
                snooze_until TEXT DEFAULT NULL,
                last_notified_at TEXT DEFAULT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (task_id) REFERENCES todo_tasks(id) ON DELETE CASCADE,
                UNIQUE(task_id, reminder_preset)
            );
            CREATE INDEX IF NOT EXISTS idx_todo_task_reminders_task ON todo_task_reminders(task_id, offset_minutes, id);
            CREATE INDEX IF NOT EXISTS idx_todo_task_reminders_fire ON todo_task_reminders(remind_at, snooze_until, last_notified_at, id);",
        )
        .map_err(|e| format!("migration 18 create reminder tables failed: {e}"))?;

        if !has_column(conn, "todo_reminder_events", "task_reminder_id")? {
            conn.execute_batch(
                "ALTER TABLE todo_reminder_events ADD COLUMN task_reminder_id INTEGER DEFAULT NULL;",
            )
            .map_err(|e| format!("migration 18 alter todo_reminder_events task_reminder_id failed: {e}"))?;
        }

        if !has_column(conn, "todo_reminder_events", "reminder_preset")? {
            conn.execute_batch(
                "ALTER TABLE todo_reminder_events ADD COLUMN reminder_preset TEXT DEFAULT NULL;",
            )
            .map_err(|e| format!("migration 18 alter todo_reminder_events reminder_preset failed: {e}"))?;
        }

        let mut task_stmt = conn
            .prepare(
                "SELECT id, event_at, remind_at, snooze_until, last_notified_at
                 FROM todo_tasks ORDER BY id ASC",
            )
            .map_err(|e| format!("migration 18 load todo_tasks failed: {e}"))?;
        let task_rows = task_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .map_err(|e| format!("migration 18 map todo_tasks failed: {e}"))?;

        let mut normalized_task_rows = Vec::new();
        for row in task_rows {
            normalized_task_rows.push(row.map_err(|e| e.to_string())?);
        }

        for (task_id, event_at, remind_at, snooze_until, last_notified_at) in normalized_task_rows {
            let Some(offset_minutes) = event_at
                .as_deref()
                .zip(remind_at.as_deref())
                .and_then(|(event_at, remind_at)| detect_reminder_offset_minutes(event_at, remind_at))
            else {
                continue;
            };

            let Some(reminder_preset) = reminder_preset_from_offset(offset_minutes) else {
                continue;
            };

            conn.execute(
                "INSERT OR IGNORE INTO todo_task_reminders(
                    task_id, reminder_preset, offset_minutes, remind_at, snooze_until, last_notified_at
                 )
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
                params![task_id, reminder_preset, offset_minutes, remind_at, snooze_until, last_notified_at],
            )
            .map_err(|e| format!("migration 18 backfill todo_task_reminders failed: {e}"))?;
        }

        let mut template_stmt = conn
            .prepare("SELECT id, reminder_offset_minutes FROM todo_templates ORDER BY id ASC")
            .map_err(|e| format!("migration 18 load todo_templates failed: {e}"))?;
        let template_rows = template_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                ))
            })
            .map_err(|e| format!("migration 18 map todo_templates failed: {e}"))?;

        let mut normalized_template_rows = Vec::new();
        for row in template_rows {
            normalized_template_rows.push(row.map_err(|e| e.to_string())?);
        }

        for (template_id, offset_minutes) in normalized_template_rows {
            let Some(offset_minutes) = offset_minutes else {
                continue;
            };
            let Some(reminder_preset) = reminder_preset_from_offset(offset_minutes) else {
                continue;
            };

            conn.execute(
                "INSERT OR IGNORE INTO todo_template_reminders(template_id, reminder_preset, offset_minutes)
                 VALUES(?1, ?2, ?3)",
                params![template_id, reminder_preset, offset_minutes],
            )
            .map_err(|e| format!("migration 18 backfill todo_template_reminders failed: {e}"))?;
        }

        set_schema_version(conn, 18)?;
    }

    // Migration 19: task pin support
    if current < 19 {
        if !has_column(conn, "todo_tasks", "pinned")? {
            conn.execute_batch(
                "ALTER TABLE todo_tasks ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;",
            )
            .map_err(|e| format!("migration 19 alter todo_tasks pinned failed: {e}"))?;
        }

        set_schema_version(conn, 19)?;
    }

    // Migration 20: fix todo simple weekly weekday semantics (Mon..Sun)
    if current < 20 {
        let mut stmt = conn
            .prepare(
                "SELECT id, rule_mode, rule_json, cron_expression, timezone, start_at
                 FROM todo_templates
                 WHERE COALESCE(series_kind, 'recurring')='recurring'
                 ORDER BY id ASC",
            )
            .map_err(|e| format!("migration 20 load todo_templates failed: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            })
            .map_err(|e| format!("migration 20 map todo_templates failed: {e}"))?;

        let mut template_rows = Vec::new();
        for row in rows {
            template_rows.push(row.map_err(|e| e.to_string())?);
        }

        for (template_id, rule_mode, rule_json, cron_expression, timezone, start_at) in template_rows
        {
            if rule_mode.trim().to_lowercase() != "simple" {
                continue;
            }

            let rule = serde_json::from_str::<Value>(&rule_json).unwrap_or(Value::Null);
            let frequency = rule
                .get("frequency")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_lowercase();
            if frequency != "weekly" {
                continue;
            }

            let Some(next_expression) = build_simple_weekly_cron_expression(&rule) else {
                continue;
            };
            if next_expression == cron_expression {
                continue;
            }

            let last_event_at: Option<String> = conn
                .query_row(
                    "SELECT MAX(event_at) FROM todo_tasks WHERE source_template_id=?1",
                    params![template_id],
                    |row| row.get(0),
                )
                .unwrap_or(None);
            let after_utc = last_event_at
                .as_deref()
                .and_then(parse_utc_datetime)
                .map(|dt| dt + Duration::seconds(1))
                .unwrap_or_else(Utc::now);

            let next_occurrence_at = compute_next_occurrence_with_start(
                &next_expression,
                &timezone,
                start_at.as_deref(),
                after_utc,
            )
            .map(|dt| dt.to_rfc3339());

            conn.execute(
                "UPDATE todo_templates
                 SET cron_expression=?1, next_occurrence_at=?2, updated_at=CURRENT_TIMESTAMP
                 WHERE id=?3",
                params![next_expression, next_occurrence_at, template_id],
            )
            .map_err(|e| format!("migration 20 update todo_templates failed: {e}"))?;
        }

        set_schema_version(conn, 20)?;
    }

    Ok(())
}

pub fn db_conn() -> Result<Connection, String> {
    let db_path = get_data_dir()?.join("lazycat.sqlite");
    let conn = Connection::open(db_path).map_err(|e| format!("open db failed: {e}"))?;
    run_migrations(&conn)?;
    Ok(conn)
}

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};

use super::helpers::db_conn;

pub const RESOURCE_TOOL: &str = "tool";
pub const RESOURCE_LAUNCHER_ENTRY: &str = "launcher-entry";
pub const RESOURCE_BROWSER_PROFILE: &str = "browser-profile";
pub const RESOURCE_SNIPPET: &str = "snippet";
pub const RESOURCE_VAULT_ENTRY: &str = "vault-entry";
pub const RESOURCE_DATA_DICTIONARY_RECORD: &str = "data-dictionary-record";
pub const RESOURCE_TODO_ITEM: &str = "todo-item";
pub const RESOURCE_PM_ITEM: &str = "pm-item";
pub const RESOURCE_ACTION_COMBINATION: &str = "action-combination";

pub const ACTION_OPEN: &str = "open";
pub const ACTION_LAUNCH: &str = "launch";
pub const ACTION_VIEW: &str = "view";
pub const ACTION_REVEAL: &str = "reveal";
pub const ACTION_COPY: &str = "copy";
pub const ACTION_RUN: &str = "run";

const MIGRATION_NAME: &str = "usage_v1";
const DAY_MS: i64 = 86_400_000;
const LEGACY_DAY: i64 = 0;
const DEFAULT_WINDOW_DAYS: i64 = 30;
const RESOURCE_SUMMARY_QUERY_BATCH_SIZE: usize = 200;

const ACTIONS: &[&str] = &["summaries", "tool_summaries", "record_tool_open"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageKey<'a> {
    pub resource_type: &'a str,
    pub scope_id: &'a str,
    pub resource_id: &'a str,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub total_count: i64,
    pub window_count: i64,
    pub last_used_at: Option<i64>,
    pub action_counts: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResourceSummaryRequest {
    resource_type: String,
    scope_id: String,
    resource_id: String,
    actions: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ActionUsageSummary {
    total_count: i64,
    window_count: i64,
    last_used_at: Option<i64>,
}

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported usage action: {action}"));
    }
    match action {
        "summaries" => resource_summaries(payload),
        "tool_summaries" => tool_summaries(payload),
        "record_tool_open" => record_tool_open(payload),
        _ => Err(format!("unsupported usage action: {action}")),
    }
}

pub(crate) fn ensure_schema_and_migrate(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS usage_daily (
            resource_type TEXT NOT NULL,
            scope_id TEXT NOT NULL DEFAULT '',
            resource_id TEXT NOT NULL,
            action TEXT NOT NULL,
            day_utc INTEGER NOT NULL,
            use_count INTEGER NOT NULL CHECK(use_count > 0),
            last_used_at_ms INTEGER NOT NULL,
            PRIMARY KEY(resource_type, scope_id, resource_id, action, day_utc)
        );
        CREATE INDEX IF NOT EXISTS idx_usage_daily_resource
            ON usage_daily(resource_type, scope_id, resource_id);
        CREATE INDEX IF NOT EXISTS idx_usage_daily_window
            ON usage_daily(resource_type, action, day_utc, use_count DESC);
        CREATE TABLE IF NOT EXISTS usage_migrations (
            name TEXT PRIMARY KEY,
            applied_at_ms INTEGER NOT NULL
        );",
    )
    .map_err(|error| format!("create usage schema failed: {error}"))?;

    let migrated = conn
        .query_row(
            "SELECT 1 FROM usage_migrations WHERE name = ?1",
            [MIGRATION_NAME],
            |_| Ok(()),
        )
        .optional()
        .map_err(|error| format!("read usage migration state failed: {error}"))?
        .is_some();
    if migrated {
        return Ok(());
    }

    let tx = conn
        .unchecked_transaction()
        .map_err(|error| format!("begin usage migration failed: {error}"))?;
    migrate_tool_clicks(&tx)?;
    migrate_launcher(&tx)?;
    migrate_browser_profiles(&tx)?;
    migrate_snippets(&tx)?;
    migrate_vault(&tx)?;
    migrate_data_dictionary(&tx)?;
    tx.execute(
        "INSERT INTO usage_migrations(name, applied_at_ms) VALUES(?1, ?2)",
        params![MIGRATION_NAME, Utc::now().timestamp_millis()],
    )
    .map_err(|error| format!("finish usage migration failed: {error}"))?;
    tx.commit()
        .map_err(|error| format!("commit usage migration failed: {error}"))
}

pub(crate) fn record(
    conn: &Connection,
    key: UsageKey<'_>,
    action: &str,
) -> Result<UsageSummary, String> {
    record_at(conn, key.clone(), action, Utc::now().timestamp_millis(), 1)?;
    summary(conn, key, DEFAULT_WINDOW_DAYS, &[])
}

pub(crate) fn summary(
    conn: &Connection,
    key: UsageKey<'_>,
    window_days: i64,
    actions: &[&str],
) -> Result<UsageSummary, String> {
    let cutoff_day = current_day() - window_days.max(1) + 1;
    let mut stmt = conn
        .prepare(
            "SELECT action, day_utc, use_count, last_used_at_ms
             FROM usage_daily
             WHERE resource_type = ?1 AND scope_id = ?2 AND resource_id = ?3",
        )
        .map_err(|error| format!("prepare usage summary failed: {error}"))?;
    let rows = stmt
        .query_map(
            params![key.resource_type, key.scope_id, key.resource_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .map_err(|error| format!("query usage summary failed: {error}"))?;

    let mut result = UsageSummary::default();
    for row in rows {
        let (row_action, day, count, last_used_at) =
            row.map_err(|error| format!("read usage summary failed: {error}"))?;
        if !actions.is_empty() && !actions.contains(&row_action.as_str()) {
            continue;
        }
        result.total_count += count;
        if day != LEGACY_DAY && day >= cutoff_day {
            result.window_count += count;
        }
        if last_used_at > 0 {
            result.last_used_at = Some(
                result
                    .last_used_at
                    .map_or(last_used_at, |current| current.max(last_used_at)),
            );
        }
        *result.action_counts.entry(row_action).or_default() += count;
    }
    Ok(result)
}

pub(crate) fn summaries_for_type(
    conn: &Connection,
    resource_type: &str,
    window_days: i64,
) -> Result<HashMap<(String, String), UsageSummary>, String> {
    let cutoff_day = current_day() - window_days.max(1) + 1;
    let mut stmt = conn
        .prepare(
            "SELECT scope_id, resource_id, action, day_utc, use_count, last_used_at_ms
             FROM usage_daily WHERE resource_type = ?1",
        )
        .map_err(|error| format!("prepare usage summaries failed: {error}"))?;
    let rows = stmt
        .query_map([resource_type], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(|error| format!("query usage summaries failed: {error}"))?;

    let mut summaries = HashMap::<(String, String), UsageSummary>::new();
    for row in rows {
        let (scope_id, resource_id, action, day, count, last_used_at) =
            row.map_err(|error| format!("read usage summaries failed: {error}"))?;
        let entry = summaries.entry((scope_id, resource_id)).or_default();
        entry.total_count += count;
        if day != LEGACY_DAY && day >= cutoff_day {
            entry.window_count += count;
        }
        if last_used_at > 0 {
            entry.last_used_at = Some(
                entry
                    .last_used_at
                    .map_or(last_used_at, |current| current.max(last_used_at)),
            );
        }
        *entry.action_counts.entry(action).or_default() += count;
    }
    Ok(summaries)
}

pub(crate) fn delete_resource(conn: &Connection, key: UsageKey<'_>) -> Result<(), String> {
    conn.execute(
        "DELETE FROM usage_daily
         WHERE resource_type = ?1 AND scope_id = ?2 AND resource_id = ?3",
        params![key.resource_type, key.scope_id, key.resource_id],
    )
    .map_err(|error| format!("delete resource usage failed: {error}"))?;
    Ok(())
}

pub(crate) fn delete_scope(
    conn: &Connection,
    resource_type: &str,
    scope_id: &str,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM usage_daily WHERE resource_type = ?1 AND scope_id = ?2",
        params![resource_type, scope_id],
    )
    .map_err(|error| format!("delete scope usage failed: {error}"))?;
    Ok(())
}

pub(crate) fn format_timestamp_ms(value: Option<i64>) -> Option<String> {
    value.and_then(|timestamp| {
        Utc.timestamp_millis_opt(timestamp)
            .single()
            .map(|value| value.to_rfc3339())
    })
}

fn tool_summaries(payload: &Value) -> Result<Value, String> {
    let ids = payload["toolIds"].as_array().ok_or("toolIds is required")?;
    if ids.len() > 512 {
        return Err("toolIds exceeds 512 items".into());
    }
    let conn = db_conn()?;
    let all = summaries_for_type(&conn, RESOURCE_TOOL, DEFAULT_WINDOW_DAYS)?;
    let mut items = Vec::new();
    for id in ids {
        let Some(id) = id.as_str().map(str::trim).filter(|id| !id.is_empty()) else {
            continue;
        };
        let summary = all
            .get(&(String::new(), id.to_string()))
            .cloned()
            .unwrap_or_default();
        items.push(json!({ "resourceId": id, "summary": summary }));
    }
    Ok(json!({ "items": items }))
}

fn resource_summaries(payload: &Value) -> Result<Value, String> {
    let conn = db_conn()?;
    resource_summaries_with_conn(&conn, payload)
}

fn resource_summaries_with_conn(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let refs = payload["refs"].as_array().ok_or("refs is required")?;
    if refs.len() > 512 {
        return Err("refs exceeds 512 items".into());
    }
    let allowed_types = [
        RESOURCE_TOOL,
        RESOURCE_LAUNCHER_ENTRY,
        RESOURCE_BROWSER_PROFILE,
        RESOURCE_SNIPPET,
        RESOURCE_VAULT_ENTRY,
        RESOURCE_DATA_DICTIONARY_RECORD,
        RESOURCE_TODO_ITEM,
        RESOURCE_PM_ITEM,
        RESOURCE_ACTION_COMBINATION,
    ];
    let mut requests = Vec::with_capacity(refs.len());
    for item in refs {
        let resource_type = item["resourceType"]
            .as_str()
            .filter(|value| allowed_types.contains(value))
            .ok_or("invalid resourceType")?;
        let scope_id = item["scopeId"].as_str().unwrap_or_default();
        let resource_id = item["resourceId"]
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty() && value.len() <= 1024)
            .ok_or("invalid resourceId")?;
        let actions = item["actions"]
            .as_array()
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        requests.push(ResourceSummaryRequest {
            resource_type: resource_type.to_string(),
            scope_id: scope_id.to_string(),
            resource_id: resource_id.to_string(),
            actions: actions.into_iter().map(str::to_string).collect(),
        });
    }

    let summaries = batch_resource_summaries(conn, &requests, DEFAULT_WINDOW_DAYS)?;
    let mut items = Vec::with_capacity(requests.len());
    for request in requests {
        let key = (
            request.resource_type.clone(),
            request.scope_id.clone(),
            request.resource_id.clone(),
        );
        let mut result = UsageSummary::default();
        if let Some(action_summaries) = summaries.get(&key) {
            for (action, action_summary) in action_summaries {
                if !request.actions.is_empty() && !request.actions.contains(action) {
                    continue;
                }
                result.total_count += action_summary.total_count;
                result.window_count += action_summary.window_count;
                result.last_used_at = match (result.last_used_at, action_summary.last_used_at) {
                    (Some(current), Some(candidate)) => Some(current.max(candidate)),
                    (None, candidate) => candidate,
                    (current, None) => current,
                };
                result
                    .action_counts
                    .insert(action.clone(), action_summary.total_count);
            }
        }
        items.push(json!({
            "resourceType": request.resource_type,
            "scopeId": request.scope_id,
            "resourceId": request.resource_id,
            "actions": request.actions,
            "summary": result,
        }));
    }
    Ok(json!({ "items": items }))
}

fn batch_resource_summaries(
    conn: &Connection,
    requests: &[ResourceSummaryRequest],
    window_days: i64,
) -> Result<HashMap<(String, String, String), BTreeMap<String, ActionUsageSummary>>, String> {
    let mut seen = HashSet::new();
    let mut keys = Vec::with_capacity(requests.len());
    for request in requests {
        let key = (
            request.resource_type.clone(),
            request.scope_id.clone(),
            request.resource_id.clone(),
        );
        if seen.insert(key.clone()) {
            keys.push(key);
        }
    }

    let cutoff_day = current_day() - window_days.max(1) + 1;
    let mut summaries = HashMap::new();
    for chunk in keys.chunks(RESOURCE_SUMMARY_QUERY_BATCH_SIZE) {
        let requested_values = std::iter::repeat_n("(?, ?, ?)", chunk.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "WITH requested(resource_type, scope_id, resource_id) AS (VALUES {requested_values})
             SELECT u.resource_type, u.scope_id, u.resource_id, u.action,
                    SUM(u.use_count),
                    SUM(CASE WHEN u.day_utc != {LEGACY_DAY} AND u.day_utc >= {cutoff_day}
                             THEN u.use_count ELSE 0 END),
                    MAX(u.last_used_at_ms)
             FROM usage_daily u
             JOIN requested r
               ON r.resource_type = u.resource_type
              AND r.scope_id = u.scope_id
              AND r.resource_id = u.resource_id
             GROUP BY u.resource_type, u.scope_id, u.resource_id, u.action"
        );
        let query_params = chunk
            .iter()
            .flat_map(|(resource_type, scope_id, resource_id)| {
                [
                    resource_type.as_str(),
                    scope_id.as_str(),
                    resource_id.as_str(),
                ]
            })
            .collect::<Vec<_>>();
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|error| format!("prepare batched usage summaries failed: {error}"))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(query_params), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            })
            .map_err(|error| format!("query batched usage summaries failed: {error}"))?;
        for row in rows {
            let (resource_type, scope_id, resource_id, action, total, window, last_used_at) =
                row.map_err(|error| format!("read batched usage summaries failed: {error}"))?;
            summaries
                .entry((resource_type, scope_id, resource_id))
                .or_insert_with(BTreeMap::new)
                .insert(
                    action,
                    ActionUsageSummary {
                        total_count: total,
                        window_count: window,
                        last_used_at: (last_used_at > 0).then_some(last_used_at),
                    },
                );
        }
    }
    Ok(summaries)
}

fn record_tool_open(payload: &Value) -> Result<Value, String> {
    let id = payload["toolId"]
        .as_str()
        .map(str::trim)
        .filter(|id| !id.is_empty() && id.len() <= 128)
        .ok_or("toolId is required")?;
    let conn = db_conn()?;
    let summary = record(
        &conn,
        UsageKey {
            resource_type: RESOURCE_TOOL,
            scope_id: "",
            resource_id: id,
        },
        ACTION_OPEN,
    )?;
    Ok(json!({ "resourceId": id, "summary": summary }))
}

fn record_at(
    conn: &Connection,
    key: UsageKey<'_>,
    action: &str,
    timestamp_ms: i64,
    count: i64,
) -> Result<(), String> {
    if key.resource_type.is_empty()
        || key.resource_id.is_empty()
        || action.is_empty()
        || count <= 0
        || timestamp_ms <= 0
    {
        return Err("invalid usage record".into());
    }
    upsert_bucket(
        conn,
        key,
        action,
        timestamp_ms.div_euclid(DAY_MS),
        timestamp_ms,
        count,
    )
}

fn upsert_bucket(
    conn: &Connection,
    key: UsageKey<'_>,
    action: &str,
    day: i64,
    last_used_at_ms: i64,
    count: i64,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO usage_daily
         (resource_type, scope_id, resource_id, action, day_utc, use_count, last_used_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(resource_type, scope_id, resource_id, action, day_utc) DO UPDATE SET
           use_count = usage_daily.use_count + excluded.use_count,
           last_used_at_ms = MAX(usage_daily.last_used_at_ms, excluded.last_used_at_ms)",
        params![
            key.resource_type,
            key.scope_id,
            key.resource_id,
            action,
            day,
            count,
            last_used_at_ms
        ],
    )
    .map_err(|error| format!("record usage failed: {error}"))?;
    Ok(())
}

fn insert_legacy(
    conn: &Connection,
    key: UsageKey<'_>,
    action: &str,
    count: i64,
    last_used_at_ms: Option<i64>,
) -> Result<(), String> {
    if count <= 0 {
        return Ok(());
    }
    upsert_bucket(
        conn,
        key,
        action,
        LEGACY_DAY,
        last_used_at_ms.unwrap_or_default(),
        count,
    )
}

fn current_day() -> i64 {
    Utc::now().timestamp_millis().div_euclid(DAY_MS)
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [name],
        |row| row.get(0),
    )
    .map_err(|error| format!("check table {name} failed: {error}"))
}

fn parse_timestamp(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.timestamp_millis())
        .ok()
        .or_else(|| {
            NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|value| value.and_utc().timestamp_millis())
        })
}

fn migrate_tool_clicks(conn: &Connection) -> Result<(), String> {
    if !table_exists(conn, "user_settings")? {
        return Ok(());
    }
    let raw = conn
        .query_row(
            "SELECT value FROM user_settings WHERE key = 'tool_clicks'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("read legacy tool clicks failed: {error}"))?;
    let Some(raw) = raw else {
        return Ok(());
    };
    let clicks: HashMap<String, Vec<i64>> = match serde_json::from_str(&raw) {
        Ok(clicks) => clicks,
        Err(error) => {
            eprintln!("skip invalid legacy tool clicks during usage migration: {error}");
            return Ok(());
        }
    };
    for (tool_id, timestamps) in clicks {
        let mut buckets = BTreeMap::<i64, (i64, i64)>::new();
        for timestamp in timestamps.into_iter().filter(|value| *value > 0) {
            let bucket = buckets
                .entry(timestamp.div_euclid(DAY_MS))
                .or_insert((0, timestamp));
            bucket.0 += 1;
            bucket.1 = bucket.1.max(timestamp);
        }
        for (day, (count, last_used_at)) in buckets {
            upsert_bucket(
                conn,
                UsageKey {
                    resource_type: RESOURCE_TOOL,
                    scope_id: "",
                    resource_id: &tool_id,
                },
                ACTION_OPEN,
                day,
                last_used_at,
                count,
            )?;
        }
    }
    Ok(())
}

fn migrate_launcher(conn: &Connection) -> Result<(), String> {
    if !table_exists(conn, "launcher_entries")? {
        return Ok(());
    }
    let mut stmt = conn
        .prepare("SELECT id, launch_count FROM launcher_entries WHERE launch_count > 0")
        .map_err(|error| format!("prepare launcher usage migration failed: {error}"))?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .map_err(|error| format!("query launcher usage migration failed: {error}"))?;
    for row in rows {
        let (id, count) = row.map_err(|error| error.to_string())?;
        insert_legacy(
            conn,
            UsageKey {
                resource_type: RESOURCE_LAUNCHER_ENTRY,
                scope_id: "",
                resource_id: &id.to_string(),
            },
            ACTION_LAUNCH,
            count,
            None,
        )?;
    }
    Ok(())
}

fn migrate_browser_profiles(conn: &Connection) -> Result<(), String> {
    if !table_exists(conn, "user_settings")? {
        return Ok(());
    }
    let raw = conn
        .query_row(
            "SELECT value FROM user_settings WHERE key = 'browser_profiles_config_v1'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("read browser profile usage migration failed: {error}"))?;
    let Some(raw) = raw else {
        return Ok(());
    };
    let config: Value = match serde_json::from_str(&raw) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("skip invalid browser profile usage during migration: {error}");
            return Ok(());
        }
    };
    for browser in ["edge", "chrome"] {
        let Some(entries) = config.get(browser).and_then(Value::as_object) else {
            continue;
        };
        for (profile_dir, entry) in entries {
            let count = entry
                .get("launchCount")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            let last_used = entry
                .get("lastLaunchedAt")
                .and_then(Value::as_str)
                .and_then(parse_timestamp);
            let resource_id = serde_json::to_string(&(browser, profile_dir))
                .map_err(|error| format!("encode browser profile usage id failed: {error}"))?;
            insert_legacy(
                conn,
                UsageKey {
                    resource_type: RESOURCE_BROWSER_PROFILE,
                    scope_id: "",
                    resource_id: &resource_id,
                },
                ACTION_LAUNCH,
                count,
                last_used,
            )?;
        }
    }
    Ok(())
}

fn migrate_snippets(conn: &Connection) -> Result<(), String> {
    if !table_exists(conn, "snippet_entries")? {
        return Ok(());
    }
    let mut stmt = conn
        .prepare("SELECT id, use_count, last_used_at FROM snippet_entries WHERE use_count > 0")
        .map_err(|error| format!("prepare snippet usage migration failed: {error}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("query snippet usage migration failed: {error}"))?;
    for row in rows {
        let (id, count, last_used) = row.map_err(|error| error.to_string())?;
        insert_legacy(
            conn,
            UsageKey {
                resource_type: RESOURCE_SNIPPET,
                scope_id: "",
                resource_id: &id.to_string(),
            },
            ACTION_VIEW,
            count,
            parse_timestamp(&last_used),
        )?;
    }
    Ok(())
}

fn migrate_vault(conn: &Connection) -> Result<(), String> {
    if !table_exists(conn, "vault_entries")? {
        return Ok(());
    }
    let mut stmt = conn
        .prepare("SELECT id, view_count, copy_count FROM vault_entries")
        .map_err(|error| format!("prepare vault usage migration failed: {error}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|error| format!("query vault usage migration failed: {error}"))?;
    for row in rows {
        let (id, views, copies) = row.map_err(|error| error.to_string())?;
        let resource_id = id.to_string();
        let key = || UsageKey {
            resource_type: RESOURCE_VAULT_ENTRY,
            scope_id: "",
            resource_id: &resource_id,
        };
        insert_legacy(conn, key(), ACTION_REVEAL, views, None)?;
        insert_legacy(conn, key(), ACTION_COPY, copies, None)?;
    }
    Ok(())
}

fn migrate_data_dictionary(conn: &Connection) -> Result<(), String> {
    if !table_exists(conn, "data_dictionary_record_usage")? {
        return Ok(());
    }
    let mut stmt = conn
        .prepare(
            "SELECT dictionary_id, normalized_value, used_count, last_used_at
             FROM data_dictionary_record_usage WHERE used_count > 0",
        )
        .map_err(|error| format!("prepare data dictionary usage migration failed: {error}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|error| format!("query data dictionary usage migration failed: {error}"))?;
    for row in rows {
        let (dictionary_id, normalized_value, count, last_used) =
            row.map_err(|error| error.to_string())?;
        insert_legacy(
            conn,
            UsageKey {
                resource_type: RESOURCE_DATA_DICTIONARY_RECORD,
                scope_id: &dictionary_id.to_string(),
                resource_id: &normalized_value,
            },
            ACTION_VIEW,
            count,
            parse_timestamp(&last_used),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE user_settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE launcher_entries(id INTEGER PRIMARY KEY, launch_count INTEGER NOT NULL);
             CREATE TABLE snippet_entries(id INTEGER PRIMARY KEY, use_count INTEGER NOT NULL, last_used_at TEXT NOT NULL);
             CREATE TABLE vault_entries(id INTEGER PRIMARY KEY, view_count INTEGER NOT NULL, copy_count INTEGER NOT NULL);
             CREATE TABLE data_dictionary_record_usage(
               dictionary_id INTEGER NOT NULL,
               normalized_value TEXT NOT NULL,
               used_count INTEGER NOT NULL,
               last_used_at TEXT NOT NULL
             );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn migrates_legacy_sources_once_without_fabricating_window_counts() {
        let conn = connection();
        let now = Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO user_settings(key, value) VALUES('tool_clicks', ?1)",
            [json!({ "formatter": [now - 1000, now] }).to_string()],
        )
        .unwrap();
        conn.execute("INSERT INTO launcher_entries VALUES(7, 12)", [])
            .unwrap();
        conn.execute(
            "INSERT INTO snippet_entries VALUES(8, 3, '2026-07-01 10:00:00')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO vault_entries VALUES(9, 4, 5)", [])
            .unwrap();
        conn.execute(
            "INSERT INTO data_dictionary_record_usage VALUES(10, 'primary-value', 6, '2026-07-02 11:00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO user_settings(key, value) VALUES('browser_profiles_config_v1', ?1)",
            [json!({
                "edge": {
                    "Default": {
                        "launchCount": 11,
                        "lastLaunchedAt": "2026-07-03T12:00:00+08:00"
                    }
                }
            })
            .to_string()],
        )
        .unwrap();

        ensure_schema_and_migrate(&conn).unwrap();
        ensure_schema_and_migrate(&conn).unwrap();

        let tool = summary(
            &conn,
            UsageKey {
                resource_type: RESOURCE_TOOL,
                scope_id: "",
                resource_id: "formatter",
            },
            30,
            &[],
        )
        .unwrap();
        assert_eq!((tool.total_count, tool.window_count), (2, 2));

        let launcher = summary(
            &conn,
            UsageKey {
                resource_type: RESOURCE_LAUNCHER_ENTRY,
                scope_id: "",
                resource_id: "7",
            },
            30,
            &[],
        )
        .unwrap();
        assert_eq!((launcher.total_count, launcher.window_count), (12, 0));

        let snippet = summary(
            &conn,
            UsageKey {
                resource_type: RESOURCE_SNIPPET,
                scope_id: "",
                resource_id: "8",
            },
            30,
            &[],
        )
        .unwrap();
        assert_eq!((snippet.total_count, snippet.window_count), (3, 0));
        assert_eq!(snippet.action_counts.get(ACTION_VIEW), Some(&3));

        let vault = summary(
            &conn,
            UsageKey {
                resource_type: RESOURCE_VAULT_ENTRY,
                scope_id: "",
                resource_id: "9",
            },
            30,
            &[],
        )
        .unwrap();
        assert_eq!((vault.total_count, vault.window_count), (9, 0));
        assert_eq!(vault.action_counts.get(ACTION_REVEAL), Some(&4));
        assert_eq!(vault.action_counts.get(ACTION_COPY), Some(&5));

        let dictionary = summary(
            &conn,
            UsageKey {
                resource_type: RESOURCE_DATA_DICTIONARY_RECORD,
                scope_id: "10",
                resource_id: "primary-value",
            },
            30,
            &[],
        )
        .unwrap();
        assert_eq!((dictionary.total_count, dictionary.window_count), (6, 0));

        let browser_resource_id = json!(["edge", "Default"]).to_string();
        let browser = summary(
            &conn,
            UsageKey {
                resource_type: RESOURCE_BROWSER_PROFILE,
                scope_id: "",
                resource_id: &browser_resource_id,
            },
            30,
            &[],
        )
        .unwrap();
        assert_eq!((browser.total_count, browser.window_count), (11, 0));
        assert_eq!(
            browser.last_used_at,
            parse_timestamp("2026-07-03T12:00:00+08:00")
        );

        let migration_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM usage_migrations WHERE name = ?1",
                [MIGRATION_NAME],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(migration_count, 1);
    }

    #[test]
    fn skips_invalid_optional_json_sources_without_blocking_migration() {
        let conn = connection();
        conn.execute(
            "INSERT INTO user_settings(key, value) VALUES('tool_clicks', 'not-json')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO user_settings(key, value) VALUES('browser_profiles_config_v1', '{')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO launcher_entries VALUES(7, 2)", [])
            .unwrap();

        ensure_schema_and_migrate(&conn).unwrap();

        let launcher = summary(
            &conn,
            UsageKey {
                resource_type: RESOURCE_LAUNCHER_ENTRY,
                scope_id: "",
                resource_id: "7",
            },
            30,
            &[],
        )
        .unwrap();
        assert_eq!((launcher.total_count, launcher.window_count), (2, 0));
        assert!(conn
            .query_row(
                "SELECT 1 FROM usage_migrations WHERE name = ?1",
                [MIGRATION_NAME],
                |_| Ok(()),
            )
            .optional()
            .unwrap()
            .is_some());
    }

    #[test]
    fn records_actions_atomically_and_keeps_action_breakdown() {
        let conn = connection();
        ensure_schema_and_migrate(&conn).unwrap();
        let key = UsageKey {
            resource_type: RESOURCE_VAULT_ENTRY,
            scope_id: "",
            resource_id: "42",
        };
        record(&conn, key.clone(), ACTION_REVEAL).unwrap();
        record(&conn, key.clone(), ACTION_COPY).unwrap();
        record(&conn, key.clone(), ACTION_COPY).unwrap();

        let result = summary(&conn, key, 30, &[]).unwrap();
        assert_eq!(result.total_count, 3);
        assert_eq!(result.window_count, 3);
        assert_eq!(result.action_counts.get(ACTION_REVEAL), Some(&1));
        assert_eq!(result.action_counts.get(ACTION_COPY), Some(&2));
        assert!(result.last_used_at.is_some());
    }

    #[test]
    fn resource_summaries_batch_preserves_order_actions_and_empty_results() {
        let conn = connection();
        ensure_schema_and_migrate(&conn).unwrap();
        let now = Utc::now().timestamp_millis();
        let key = UsageKey {
            resource_type: RESOURCE_VAULT_ENTRY,
            scope_id: "",
            resource_id: "42",
        };
        record_at(&conn, key.clone(), ACTION_REVEAL, now - 1_000, 2).unwrap();
        record_at(&conn, key, ACTION_COPY, now, 3).unwrap();
        record_at(
            &conn,
            UsageKey {
                resource_type: RESOURCE_DATA_DICTIONARY_RECORD,
                scope_id: "7",
                resource_id: "primary-value",
            },
            ACTION_VIEW,
            now,
            4,
        )
        .unwrap();

        let result = resource_summaries_with_conn(
            &conn,
            &json!({
                "refs": [
                    {
                        "resourceType": RESOURCE_VAULT_ENTRY,
                        "resourceId": "missing",
                        "actions": [ACTION_COPY]
                    },
                    {
                        "resourceType": RESOURCE_VAULT_ENTRY,
                        "resourceId": "42",
                        "actions": [ACTION_COPY]
                    },
                    {
                        "resourceType": RESOURCE_VAULT_ENTRY,
                        "resourceId": "42",
                        "actions": []
                    },
                    {
                        "resourceType": RESOURCE_DATA_DICTIONARY_RECORD,
                        "scopeId": "7",
                        "resourceId": "primary-value",
                        "actions": [ACTION_VIEW]
                    }
                ]
            }),
        )
        .unwrap();
        let items = result["items"].as_array().unwrap();

        assert_eq!(items.len(), 4);
        assert_eq!(items[0]["resourceId"], "missing");
        assert_eq!(items[0]["summary"]["totalCount"], 0);
        assert_eq!(items[1]["summary"]["totalCount"], 3);
        assert_eq!(items[1]["summary"]["actionCounts"][ACTION_COPY], 3);
        assert!(items[1]["summary"]["actionCounts"]
            .get(ACTION_REVEAL)
            .is_none());
        assert_eq!(items[2]["summary"]["totalCount"], 5);
        assert_eq!(items[2]["summary"]["actionCounts"][ACTION_REVEAL], 2);
        assert_eq!(items[2]["summary"]["actionCounts"][ACTION_COPY], 3);
        assert_eq!(items[3]["scopeId"], "7");
        assert_eq!(items[3]["summary"]["totalCount"], 4);
    }

    #[test]
    fn resource_summaries_accepts_spotlight_domain_refs() {
        let conn = connection();
        ensure_schema_and_migrate(&conn).unwrap();
        let now = Utc::now().timestamp_millis();
        let cases = [
            (RESOURCE_TODO_ITEM, "11", ACTION_OPEN),
            (RESOURCE_PM_ITEM, "22", ACTION_OPEN),
            (RESOURCE_ACTION_COMBINATION, "33", ACTION_RUN),
        ];

        for &(resource_type, resource_id, action) in &cases {
            record_at(
                &conn,
                UsageKey {
                    resource_type,
                    scope_id: "",
                    resource_id,
                },
                action,
                now,
                1,
            )
            .unwrap();
        }

        let refs = cases
            .iter()
            .map(|(resource_type, resource_id, action)| {
                json!({
                    "resourceType": resource_type,
                    "resourceId": resource_id,
                    "actions": [action]
                })
            })
            .collect::<Vec<_>>();
        let result = resource_summaries_with_conn(&conn, &json!({ "refs": refs })).unwrap();
        let items = result["items"].as_array().unwrap();

        assert_eq!(items.len(), cases.len());
        for (item, (resource_type, resource_id, action)) in items.iter().zip(cases) {
            assert_eq!(item["resourceType"], resource_type);
            assert_eq!(item["resourceId"], resource_id);
            assert_eq!(item["summary"]["totalCount"], 1);
            assert_eq!(item["summary"]["actionCounts"][action], 1);
        }
    }

    #[test]
    fn resource_summaries_batch_handles_more_than_one_sqlite_parameter_batch() {
        let conn = connection();
        ensure_schema_and_migrate(&conn).unwrap();
        let last_id = RESOURCE_SUMMARY_QUERY_BATCH_SIZE;
        let refs = (0..=last_id)
            .map(|index| {
                json!({
                    "resourceType": RESOURCE_LAUNCHER_ENTRY,
                    "resourceId": index.to_string(),
                    "actions": [ACTION_LAUNCH]
                })
            })
            .collect::<Vec<_>>();
        record_at(
            &conn,
            UsageKey {
                resource_type: RESOURCE_LAUNCHER_ENTRY,
                scope_id: "",
                resource_id: &last_id.to_string(),
            },
            ACTION_LAUNCH,
            Utc::now().timestamp_millis(),
            6,
        )
        .unwrap();

        let result = resource_summaries_with_conn(&conn, &json!({ "refs": refs })).unwrap();
        let items = result["items"].as_array().unwrap();

        assert_eq!(items.len(), RESOURCE_SUMMARY_QUERY_BATCH_SIZE + 1);
        assert_eq!(items[last_id]["resourceId"], last_id.to_string());
        assert_eq!(items[last_id]["summary"]["totalCount"], 6);
    }

    #[test]
    fn resource_summaries_rejects_invalid_refs_before_querying() {
        let conn = connection();
        ensure_schema_and_migrate(&conn).unwrap();

        let invalid_type = resource_summaries_with_conn(
            &conn,
            &json!({
                "refs": [{ "resourceType": "unknown", "resourceId": "1" }]
            }),
        )
        .unwrap_err();
        let invalid_id = resource_summaries_with_conn(
            &conn,
            &json!({
                "refs": [{ "resourceType": RESOURCE_TOOL, "resourceId": "  " }]
            }),
        )
        .unwrap_err();

        assert_eq!(invalid_type, "invalid resourceType");
        assert_eq!(invalid_id, "invalid resourceId");
    }
}

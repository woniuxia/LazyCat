use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use serde_json::{json, Value};

use super::helpers::db_conn;

const ACTIONS: &[&str] = &[
    "item_list",
    "item_get",
    "item_create",
    "item_update",
    "progress_add",
    "continue_following",
    "confirm_completed",
    "confirm_canceled",
    "stop_following",
    "reopen",
    "progress_update",
    "progress_delete",
    "item_snooze",
    "item_delete",
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderDispatch {
    pub item_id: Option<i64>,
    pub due_count: usize,
    pub title: String,
    pub body: String,
    pub review_at: Option<String>,
    #[serde(skip)]
    pub(crate) dispatch_targets: Vec<ReminderTarget>,
}

#[derive(Clone, Debug)]
pub(crate) struct ReminderTarget {
    item_id: i64,
    review_at: String,
}

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported follow_up action: {action}"));
    }
    let mut conn = db_conn()?;
    execute_with_conn(&mut conn, action, payload)
        .map_err(|error| format!("follow_up.{action} failed: {error}"))
}

pub fn scheduler_tick() -> Result<Vec<ReminderDispatch>, String> {
    let mut conn = db_conn()?;
    collect_due_with_conn(&mut conn, Utc::now())
        .map_err(|error| format!("collect follow-up reminders failed: {error}"))
}

pub fn acknowledge_scheduler_dispatches(reminders: &[ReminderDispatch]) -> Result<(), String> {
    let mut conn = db_conn()?;
    acknowledge_dispatches_with_conn(&mut conn, reminders, Utc::now())
        .map_err(|error| format!("acknowledge follow-up reminders failed: {error}"))
}

fn execute_with_conn(
    conn: &mut Connection,
    action: &str,
    payload: &Value,
) -> Result<Value, String> {
    ensure_schema(conn)?;
    match action {
        "item_list" => item_list(conn, payload),
        "item_get" => item_get(conn, required_id(payload, "id")?),
        "item_create" => item_create(conn, payload),
        "item_update" => item_update(conn, payload),
        "progress_add" => progress_add(conn, payload),
        "continue_following" => transition(conn, payload, "continued"),
        "confirm_completed" => transition(conn, payload, "completed"),
        "confirm_canceled" => transition(conn, payload, "canceled"),
        "stop_following" => transition(conn, payload, "stopped_following"),
        "reopen" => transition(conn, payload, "reopened"),
        "progress_update" => progress_update(conn, payload),
        "progress_delete" => progress_delete(conn, payload),
        "item_snooze" => item_snooze(conn, payload),
        "item_delete" => item_delete(conn, payload),
        _ => Err(format!("unsupported follow_up action: {action}")),
    }
}

pub(crate) fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS follow_up_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL CHECK(length(trim(title)) > 0),
            description TEXT NOT NULL DEFAULT '',
            expected_outcome TEXT NOT NULL DEFAULT '',
            priority TEXT NOT NULL DEFAULT 'P2' CHECK(priority IN ('P0','P1','P2','P3')),
            attention_status TEXT NOT NULL DEFAULT 'active' CHECK(attention_status IN ('active','ended')),
            external_result TEXT NOT NULL DEFAULT 'unknown' CHECK(external_result IN ('unknown','completed','canceled')),
            ending_mode TEXT CHECK(ending_mode IN ('result_confirmed','stopped_following')),
            person_id INTEGER,
            person_name_snapshot TEXT NOT NULL CHECK(length(trim(person_name_snapshot)) > 0),
            review_at TEXT,
            expected_completion_at TEXT,
            snooze_until TEXT,
            last_notified_review_at TEXT,
            ended_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            CHECK((attention_status = 'active' AND review_at IS NOT NULL AND ending_mode IS NULL AND ended_at IS NULL)
                OR (attention_status = 'ended' AND review_at IS NULL AND ending_mode IS NOT NULL AND ended_at IS NOT NULL)),
            CHECK((ending_mode IS NULL AND external_result = 'unknown')
                OR (ending_mode = 'stopped_following' AND external_result = 'unknown')
                OR (ending_mode = 'result_confirmed' AND external_result IN ('completed','canceled'))),
            FOREIGN KEY(person_id) REFERENCES todo_assignees(id) ON DELETE SET NULL
        );
        CREATE INDEX IF NOT EXISTS idx_follow_up_items_review ON follow_up_items(attention_status, review_at, id);
        CREATE INDEX IF NOT EXISTS idx_follow_up_items_person ON follow_up_items(person_id, id);
        CREATE TABLE IF NOT EXISTS follow_up_progress (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            item_id INTEGER NOT NULL,
            kind TEXT NOT NULL CHECK(kind IN ('progress','continued','completed','canceled','stopped_following','reopened')),
            content TEXT NOT NULL CHECK(length(trim(content)) > 0),
            occurred_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(item_id) REFERENCES follow_up_items(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_follow_up_progress_item ON follow_up_progress(item_id, occurred_at DESC, id DESC);
        CREATE TABLE IF NOT EXISTS follow_up_links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            item_id INTEGER NOT NULL,
            url TEXT NOT NULL CHECK(length(trim(url)) > 0),
            title TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY(item_id) REFERENCES follow_up_items(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_follow_up_links_item ON follow_up_links(item_id, sort_order, id);
        "#,
    )
    .map_err(|error| format!("ensure follow-up schema failed: {error}"))
}

fn item_create(conn: &mut Connection, payload: &Value) -> Result<Value, String> {
    let title = required_text(payload, "title")?;
    let review_at = required_timestamp(payload, "reviewAt")?;
    let priority = priority(payload)?;
    let person_id = required_id(payload, "personId")?;
    let person_name = person_name(conn, person_id)?;
    let now = Utc::now().to_rfc3339();
    let tx = conn
        .transaction()
        .map_err(db_error("begin create transaction"))?;
    tx.execute(
        "INSERT INTO follow_up_items(title,description,expected_outcome,priority,person_id,person_name_snapshot,review_at,expected_completion_at,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)",
        params![title, optional_text(payload,"description"), optional_text(payload,"expectedOutcome"), priority, person_id, person_name, review_at, optional_timestamp(payload,"expectedCompletionAt")?, now],
    ).map_err(db_error("insert follow-up item"))?;
    let id = tx.last_insert_rowid();
    replace_links(&tx, id, payload.get("links"))?;
    tx.commit().map_err(db_error("commit create transaction"))?;
    item_get(conn, id)
}

fn item_update(conn: &mut Connection, payload: &Value) -> Result<Value, String> {
    let id = required_id(payload, "id")?;
    let current = load_item_state(conn, id)?;
    let title = required_text(payload, "title")?;
    let priority = priority(payload)?;
    let person_id = required_id(payload, "personId")?;
    let person_name = person_name(conn, person_id)?;
    let review_at = if current.0 == "active" {
        Some(required_timestamp(payload, "reviewAt")?)
    } else {
        None
    };
    let now = Utc::now().to_rfc3339();
    let tx = conn
        .transaction()
        .map_err(db_error("begin update transaction"))?;
    let changed = tx.execute(
        "UPDATE follow_up_items SET title=?1,description=?2,expected_outcome=?3,priority=?4,person_id=?5,person_name_snapshot=?6,snooze_until=CASE WHEN review_at IS NOT ?7 THEN NULL ELSE snooze_until END,last_notified_review_at=CASE WHEN review_at IS NOT ?7 THEN NULL ELSE last_notified_review_at END,review_at=?7,expected_completion_at=?8,updated_at=?9 WHERE id=?10",
        params![title, optional_text(payload,"description"), optional_text(payload,"expectedOutcome"), priority, person_id, person_name, review_at, optional_timestamp(payload,"expectedCompletionAt")?, now, id],
    ).map_err(db_error("update follow-up item"))?;
    ensure_changed(changed, id)?;
    replace_links(&tx, id, payload.get("links"))?;
    tx.commit().map_err(db_error("commit update transaction"))?;
    item_get(conn, id)
}

fn progress_add(conn: &mut Connection, payload: &Value) -> Result<Value, String> {
    let item_id = required_id(payload, "id")?;
    let content = required_text(payload, "content")?;
    ensure_item_exists(conn, item_id)?;
    let now = Utc::now().to_rfc3339();
    conn.execute("INSERT INTO follow_up_progress(item_id,kind,content,occurred_at,updated_at) VALUES(?1,'progress',?2,?3,?3)", params![item_id,content,now])
        .map_err(db_error("append progress"))?;
    item_get(conn, item_id)
}

fn transition(conn: &mut Connection, payload: &Value, kind: &str) -> Result<Value, String> {
    let item_id = required_id(payload, "id")?;
    let content = if kind == "reopened" {
        optional_text(payload, "content")
    } else {
        required_text(payload, "content")?
    };
    let review_at = if matches!(kind, "continued" | "reopened") {
        Some(required_timestamp(payload, "reviewAt")?)
    } else {
        None
    };
    let (status, _result, _ending_mode, _review_at) = load_item_state(conn, item_id)?;
    match kind {
        "reopened" if status != "ended" => {
            return Err("only ended follow-up items can be reopened".into())
        }
        "reopened" => {}
        _ if status != "active" => return Err("only active follow-up items can transition".into()),
        _ => {}
    }
    let now = Utc::now().to_rfc3339();
    let tx = conn
        .transaction()
        .map_err(db_error("begin lifecycle transaction"))?;
    let timeline_content = if kind == "reopened" && content.is_empty() {
        "重新关注"
    } else {
        &content
    };
    tx.execute("INSERT INTO follow_up_progress(item_id,kind,content,occurred_at,updated_at) VALUES(?1,?2,?3,?4,?4)", params![item_id,kind,timeline_content,now])
        .map_err(db_error("append lifecycle progress"))?;
    let changed = match kind {
        "continued" => tx.execute("UPDATE follow_up_items SET review_at=?1,snooze_until=NULL,last_notified_review_at=NULL,updated_at=?2 WHERE id=?3 AND attention_status='active'", params![review_at,now,item_id]),
        "completed" => tx.execute("UPDATE follow_up_items SET attention_status='ended',external_result='completed',ending_mode='result_confirmed',review_at=NULL,snooze_until=NULL,last_notified_review_at=NULL,ended_at=?1,updated_at=?1 WHERE id=?2 AND attention_status='active'", params![now,item_id]),
        "canceled" => tx.execute("UPDATE follow_up_items SET attention_status='ended',external_result='canceled',ending_mode='result_confirmed',review_at=NULL,snooze_until=NULL,last_notified_review_at=NULL,ended_at=?1,updated_at=?1 WHERE id=?2 AND attention_status='active'", params![now,item_id]),
        "stopped_following" => tx.execute("UPDATE follow_up_items SET attention_status='ended',external_result='unknown',ending_mode='stopped_following',review_at=NULL,snooze_until=NULL,last_notified_review_at=NULL,ended_at=?1,updated_at=?1 WHERE id=?2 AND attention_status='active'", params![now,item_id]),
        "reopened" => tx.execute("UPDATE follow_up_items SET attention_status='active',external_result='unknown',ending_mode=NULL,review_at=?1,snooze_until=NULL,last_notified_review_at=NULL,ended_at=NULL,updated_at=?2 WHERE id=?3 AND attention_status='ended'", params![review_at,now,item_id]),
        _ => unreachable!(),
    }.map_err(db_error("update lifecycle state"))?;
    ensure_changed(changed, item_id)?;
    tx.commit()
        .map_err(db_error("commit lifecycle transaction"))?;
    item_get(conn, item_id)
}

fn progress_update(conn: &mut Connection, payload: &Value) -> Result<Value, String> {
    let progress_id = required_id(payload, "progressId")?;
    let content = required_text(payload, "content")?;
    let now = Utc::now().to_rfc3339();
    let item_id: Option<i64> = conn
        .query_row(
            "SELECT item_id FROM follow_up_progress WHERE id=?1 AND kind='progress'",
            [progress_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error("find ordinary progress"))?;
    let item_id = item_id.ok_or_else(|| format!("ordinary progress {progress_id} not found"))?;
    conn.execute(
        "UPDATE follow_up_progress SET content=?1,updated_at=?2 WHERE id=?3 AND kind='progress'",
        params![content, now, progress_id],
    )
    .map_err(db_error("update ordinary progress"))?;
    item_get(conn, item_id)
}

fn progress_delete(conn: &mut Connection, payload: &Value) -> Result<Value, String> {
    let progress_id = required_id(payload, "progressId")?;
    let item_id: Option<i64> = conn
        .query_row(
            "SELECT item_id FROM follow_up_progress WHERE id=?1 AND kind='progress'",
            [progress_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error("find ordinary progress"))?;
    let item_id = item_id.ok_or_else(|| format!("ordinary progress {progress_id} not found"))?;
    conn.execute(
        "DELETE FROM follow_up_progress WHERE id=?1 AND kind='progress'",
        [progress_id],
    )
    .map_err(db_error("delete ordinary progress"))?;
    item_get(conn, item_id)
}

fn item_snooze(conn: &mut Connection, payload: &Value) -> Result<Value, String> {
    let id = required_id(payload, "id")?;
    let minutes = payload
        .get("minutes")
        .and_then(Value::as_i64)
        .ok_or("minutes is required")?;
    if !(1..=7 * 24 * 60).contains(&minutes) {
        return Err("minutes must be between 1 and 10080".into());
    }
    let until = (Utc::now() + chrono::Duration::minutes(minutes)).to_rfc3339();
    let changed = conn.execute("UPDATE follow_up_items SET snooze_until=?1,last_notified_review_at=NULL,updated_at=?2 WHERE id=?3 AND attention_status='active'", params![until,Utc::now().to_rfc3339(),id]).map_err(db_error("snooze follow-up reminder"))?;
    ensure_changed(changed, id)?;
    item_get(conn, id)
}

fn item_delete(conn: &mut Connection, payload: &Value) -> Result<Value, String> {
    let id = required_id(payload, "id")?;
    let changed = conn
        .execute("DELETE FROM follow_up_items WHERE id=?1", [id])
        .map_err(db_error("delete follow-up item"))?;
    ensure_changed(changed, id)?;
    Ok(json!({"ok":true,"id":id}))
}

fn item_list(conn: &Connection, payload: &Value) -> Result<Value, String> {
    let keyword = optional_text(payload, "keyword").to_lowercase();
    let person_id = payload.get("personId").and_then(Value::as_i64);
    let filter_priority = payload
        .get("priority")
        .and_then(Value::as_str)
        .unwrap_or("");
    let status = payload
        .get("attentionStatus")
        .and_then(Value::as_str)
        .unwrap_or("");
    let mut stmt = conn.prepare("SELECT i.id FROM follow_up_items i LEFT JOIN todo_assignees a ON a.id=i.person_id WHERE (?1='' OR lower(i.title||' '||i.description||' '||i.person_name_snapshot||' '||coalesce(a.name,'')) LIKE '%'||?1||'%' OR EXISTS(SELECT 1 FROM follow_up_progress p WHERE p.item_id=i.id AND lower(p.content) LIKE '%'||?1||'%')) AND (?2 IS NULL OR i.person_id=?2) AND (?3='' OR i.priority=?3) AND (?4='' OR i.attention_status=?4) ORDER BY CASE WHEN i.attention_status='active' THEN 0 ELSE 1 END, i.review_at ASC, i.ended_at DESC, i.id ASC").map_err(db_error("prepare follow-up list"))?;
    let ids = stmt
        .query_map(
            params![keyword, person_id, filter_priority, status],
            |row| row.get::<_, i64>(0),
        )
        .map_err(db_error("query follow-up list"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_error("read follow-up list"))?;
    let items = ids
        .into_iter()
        .map(|id| item_value(conn, id))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::Array(items))
}

fn item_get(conn: &Connection, id: i64) -> Result<Value, String> {
    item_value(conn, id)
}

fn item_value(conn: &Connection, id: i64) -> Result<Value, String> {
    let mut item: Value = conn.query_row("SELECT i.id,i.title,i.description,i.expected_outcome,i.priority,i.attention_status,i.external_result,i.ending_mode,i.person_id,coalesce(a.name,i.person_name_snapshot),i.person_name_snapshot,i.review_at,i.expected_completion_at,i.snooze_until,i.last_notified_review_at,i.ended_at,i.created_at,i.updated_at FROM follow_up_items i LEFT JOIN todo_assignees a ON a.id=i.person_id WHERE i.id=?1", [id], |row| Ok(json!({
        "id":row.get::<_,i64>(0)?,"title":row.get::<_,String>(1)?,"description":row.get::<_,String>(2)?,"expectedOutcome":row.get::<_,String>(3)?,"priority":row.get::<_,String>(4)?,"attentionStatus":row.get::<_,String>(5)?,"externalResult":row.get::<_,String>(6)?,"endingMode":row.get::<_,Option<String>>(7)?,"personId":row.get::<_,Option<i64>>(8)?,"personName":row.get::<_,String>(9)?,"personNameSnapshot":row.get::<_,String>(10)?,"reviewAt":row.get::<_,Option<String>>(11)?,"expectedCompletionAt":row.get::<_,Option<String>>(12)?,"snoozeUntil":row.get::<_,Option<String>>(13)?,"lastNotifiedReviewAt":row.get::<_,Option<String>>(14)?,"endedAt":row.get::<_,Option<String>>(15)?,"createdAt":row.get::<_,String>(16)?,"updatedAt":row.get::<_,String>(17)?
    }))).optional().map_err(db_error("load follow-up item"))?.ok_or_else(|| format!("follow-up item {id} not found"))?;
    let progress = load_children(conn,"SELECT id,kind,content,occurred_at,updated_at FROM follow_up_progress WHERE item_id=?1 ORDER BY occurred_at DESC,id DESC",id,true)?;
    let links = load_children(conn,"SELECT id,'link',title,url,'' FROM follow_up_links WHERE item_id=?1 ORDER BY sort_order,id",id,false)?;
    item["latestProgress"] = progress.first().cloned().unwrap_or(Value::Null);
    item["progress"] = Value::Array(progress);
    item["links"] = Value::Array(links);
    Ok(item)
}

fn load_children(
    conn: &Connection,
    sql: &str,
    id: i64,
    progress: bool,
) -> Result<Vec<Value>, String> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(db_error("prepare follow-up children"))?;
    let rows = stmt.query_map([id],|row| if progress { Ok(json!({"id":row.get::<_,i64>(0)?,"kind":row.get::<_,String>(1)?,"content":row.get::<_,String>(2)?,"occurredAt":row.get::<_,String>(3)?,"updatedAt":row.get::<_,String>(4)?})) } else { Ok(json!({"id":row.get::<_,i64>(0)?,"title":row.get::<_,String>(2)?,"url":row.get::<_,String>(3)?})) }).map_err(db_error("query follow-up children"))?.collect::<Result<Vec<_>,_>>().map_err(db_error("read follow-up children"))?;
    Ok(rows)
}

fn replace_links(tx: &Transaction<'_>, item_id: i64, links: Option<&Value>) -> Result<(), String> {
    tx.execute("DELETE FROM follow_up_links WHERE item_id=?1", [item_id])
        .map_err(db_error("clear follow-up links"))?;
    let Some(values) = links.and_then(Value::as_array) else {
        return Ok(());
    };
    for (index, link) in values.iter().enumerate() {
        let url = required_text(link, "url")?;
        tx.execute(
            "INSERT INTO follow_up_links(item_id,url,title,sort_order) VALUES(?1,?2,?3,?4)",
            params![item_id, url, optional_text(link, "title"), index as i64],
        )
        .map_err(db_error("insert follow-up link"))?;
    }
    Ok(())
}

fn collect_due_with_conn(
    conn: &mut Connection,
    now: DateTime<Utc>,
) -> Result<Vec<ReminderDispatch>, String> {
    ensure_schema(conn)?;
    let now_text = now.to_rfc3339();
    let due = {
        let mut stmt=conn.prepare("SELECT id,title,review_at FROM follow_up_items WHERE attention_status='active' AND review_at<=?1 AND (snooze_until IS NULL OR snooze_until<=?1) AND (last_notified_review_at IS NULL OR last_notified_review_at<>review_at) ORDER BY review_at,id").map_err(db_error("prepare due follow-ups"))?;
        let rows = stmt
            .query_map([&now_text], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(db_error("query due follow-ups"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_error("read due follow-ups"))?;
        rows
    };
    if due.is_empty() {
        return Ok(vec![]);
    }
    if due.len() == 1 {
        let (id, title, review_at) = due.into_iter().next().unwrap();
        return Ok(vec![ReminderDispatch {
            item_id: Some(id),
            due_count: 1,
            title: "关注事项待复查".into(),
            body: title,
            review_at: Some(review_at.clone()),
            dispatch_targets: vec![ReminderTarget {
                item_id: id,
                review_at,
            }],
        }]);
    }
    let dispatch_targets = due
        .iter()
        .map(|(item_id, _, review_at)| ReminderTarget {
            item_id: *item_id,
            review_at: review_at.clone(),
        })
        .collect();
    Ok(vec![ReminderDispatch {
        item_id: None,
        due_count: due.len(),
        title: "关注事项待复查".into(),
        body: format!("有 {} 项关注事项需要复查", due.len()),
        review_at: None,
        dispatch_targets,
    }])
}

fn acknowledge_dispatches_with_conn(
    conn: &mut Connection,
    reminders: &[ReminderDispatch],
    now: DateTime<Utc>,
) -> Result<(), String> {
    let now_text = now.to_rfc3339();
    let tx = conn
        .transaction()
        .map_err(db_error("begin reminder acknowledgement"))?;
    for target in reminders
        .iter()
        .flat_map(|reminder| &reminder.dispatch_targets)
    {
        tx.execute(
            "UPDATE follow_up_items SET last_notified_review_at=review_at,updated_at=?1 WHERE id=?2 AND attention_status='active' AND review_at=?3 AND (snooze_until IS NULL OR snooze_until<=?1) AND (last_notified_review_at IS NULL OR last_notified_review_at<>review_at)",
            params![now_text, target.item_id, target.review_at],
        )
        .map_err(db_error("acknowledge due follow-up"))?;
    }
    tx.commit()
        .map_err(db_error("commit reminder acknowledgement"))
}

fn person_name(conn: &Connection, id: i64) -> Result<String, String> {
    conn.query_row("SELECT name FROM todo_assignees WHERE id=?1", [id], |row| {
        row.get(0)
    })
    .optional()
    .map_err(db_error("load person"))?
    .ok_or_else(|| format!("person {id} not found"))
}
fn ensure_item_exists(conn: &Connection, id: i64) -> Result<(), String> {
    let found = conn
        .query_row("SELECT 1 FROM follow_up_items WHERE id=?1", [id], |row| {
            row.get::<_, i64>(0)
        })
        .optional()
        .map_err(db_error("find follow-up item"))?;
    if found.is_none() {
        Err(format!("follow-up item {id} not found"))
    } else {
        Ok(())
    }
}
fn load_item_state(
    conn: &Connection,
    id: i64,
) -> Result<(String, String, Option<String>, Option<String>), String> {
    conn.query_row("SELECT attention_status,external_result,ending_mode,review_at FROM follow_up_items WHERE id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional().map_err(db_error("load follow-up state"))?.ok_or_else(||format!("follow-up item {id} not found"))
}
fn required_id(payload: &Value, key: &str) -> Result<i64, String> {
    payload
        .get(key)
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| format!("{key} is required"))
}
fn required_text(payload: &Value, key: &str) -> Result<String, String> {
    let value = optional_text(payload, key);
    if value.is_empty() {
        Err(format!("{key} is required"))
    } else {
        Ok(value)
    }
}
fn optional_text(payload: &Value, key: &str) -> String {
    payload
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}
fn required_timestamp(payload: &Value, key: &str) -> Result<String, String> {
    optional_timestamp(payload, key)?.ok_or_else(|| format!("{key} is required"))
}
fn optional_timestamp(payload: &Value, key: &str) -> Result<Option<String>, String> {
    let value = optional_text(payload, key);
    if value.is_empty() {
        return Ok(None);
    };
    DateTime::parse_from_rfc3339(&value)
        .map(|date| Some(date.with_timezone(&Utc).to_rfc3339()))
        .map_err(|_| format!("{key} must be an RFC3339 timestamp"))
}
fn priority(payload: &Value) -> Result<String, String> {
    let value = payload
        .get("priority")
        .and_then(Value::as_str)
        .unwrap_or("P2");
    if matches!(value, "P0" | "P1" | "P2" | "P3") {
        Ok(value.into())
    } else {
        Err("priority must be P0, P1, P2 or P3".into())
    }
}
fn ensure_changed(changed: usize, id: i64) -> Result<(), String> {
    if changed == 1 {
        Ok(())
    } else {
        Err(format!("follow-up item {id} not found or stale"))
    }
}
fn db_error(context: &'static str) -> impl FnOnce(rusqlite::Error) -> String {
    move |error| format!("{context}: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE todo_assignees(id INTEGER PRIMARY KEY,name TEXT NOT NULL); INSERT INTO todo_assignees(id,name) VALUES(1,'张三'),(2,'李四');").unwrap();
        ensure_schema(&conn).unwrap();
        conn
    }
    fn create(conn: &mut Connection, review_at: &str) -> Value {
        execute_with_conn(conn,"item_create",&json!({"title":"确认接口进度","personId":1,"reviewAt":review_at,"priority":"P1","links":[{"url":"https://example.test/ticket","title":"工单"}]})).unwrap()
    }

    #[test]
    fn creation_enforces_active_invariant_and_preserves_person_snapshot() {
        let mut conn = test_conn();
        for payload in [
            json!({"personId":1,"reviewAt":"2026-08-18T10:00:00+08:00"}),
            json!({"title":"x","reviewAt":"2026-08-18T10:00:00+08:00"}),
            json!({"title":"x","personId":1}),
        ] {
            assert!(execute_with_conn(&mut conn, "item_create", &payload).is_err());
        }
        let item = create(&mut conn, "2026-08-17T10:00:00+08:00");
        assert_eq!(item["attentionStatus"], "active");
        assert_eq!(item["personName"], "张三");
        assert_eq!(item["links"].as_array().unwrap().len(), 1);
        conn.execute("DELETE FROM todo_assignees WHERE id=1", [])
            .unwrap();
        let loaded = execute_with_conn(&mut conn, "item_get", &json!({"id":item["id"]})).unwrap();
        assert_eq!(loaded["personId"], Value::Null);
        assert_eq!(loaded["personName"], "张三");
    }

    #[test]
    fn lifecycle_actions_are_atomic_and_keep_attention_separate_from_result() {
        let mut conn = test_conn();
        let item = create(&mut conn, "2026-08-18T10:00:00+08:00");
        let id = item["id"].as_i64().unwrap();
        let continued = execute_with_conn(
            &mut conn,
            "continue_following",
            &json!({"id":id,"content":"仍在联调","reviewAt":"2026-08-21T10:00:00+08:00"}),
        )
        .unwrap();
        assert_eq!(continued["reviewAt"], "2026-08-21T02:00:00+00:00");
        let ended = execute_with_conn(
            &mut conn,
            "stop_following",
            &json!({"id":id,"content":"不再需要跟踪"}),
        )
        .unwrap();
        assert_eq!(ended["attentionStatus"], "ended");
        assert_eq!(ended["externalResult"], "unknown");
        assert_eq!(ended["endingMode"], "stopped_following");
        let reopened = execute_with_conn(
            &mut conn,
            "reopen",
            &json!({"id":id,"reviewAt":"2026-08-25T10:00:00+08:00"}),
        )
        .unwrap();
        assert_eq!(reopened["attentionStatus"], "active");
        assert_eq!(reopened["externalResult"], "unknown");
        let completed = execute_with_conn(
            &mut conn,
            "confirm_completed",
            &json!({"id":id,"content":"已验收"}),
        )
        .unwrap();
        assert_eq!(completed["externalResult"], "completed");
        assert_eq!(completed["reviewAt"], Value::Null);
    }

    #[test]
    fn lifecycle_rolls_back_timeline_when_state_write_fails() {
        let mut conn = test_conn();
        let id = create(&mut conn, "2026-08-18T10:00:00+08:00")["id"]
            .as_i64()
            .unwrap();
        conn.execute_batch("CREATE TRIGGER fail_follow_up_state BEFORE UPDATE ON follow_up_items BEGIN SELECT RAISE(ABORT,'forced state failure'); END;").unwrap();
        assert!(execute_with_conn(
            &mut conn,
            "continue_following",
            &json!({"id":id,"content":"推进中","reviewAt":"2026-08-22T10:00:00+08:00"})
        )
        .is_err());
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM follow_up_progress WHERE item_id=?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn scheduler_acknowledges_each_review_cycle_once_and_aggregates_batches() {
        let mut conn = test_conn();
        create(&mut conn, "2026-08-18T01:00:00Z");
        let now = DateTime::parse_from_rfc3339("2026-08-18T02:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let first = collect_due_with_conn(&mut conn, now).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].item_id, Some(1));
        assert_eq!(collect_due_with_conn(&mut conn, now).unwrap().len(), 1);
        acknowledge_dispatches_with_conn(&mut conn, &first, now).unwrap();
        assert!(collect_due_with_conn(&mut conn, now).unwrap().is_empty());
        create(&mut conn, "2026-08-18T01:30:00Z");
        create(&mut conn, "2026-08-18T01:45:00Z");
        let aggregate = collect_due_with_conn(&mut conn, now).unwrap();
        assert_eq!(aggregate[0].item_id, None);
        assert_eq!(aggregate[0].due_count, 2);
        assert_eq!(aggregate[0].dispatch_targets.len(), 2);
        acknowledge_dispatches_with_conn(&mut conn, &aggregate, now).unwrap();
        assert!(collect_due_with_conn(&mut conn, now).unwrap().is_empty());
    }

    #[test]
    fn scheduler_acknowledgement_failure_and_snooze_remain_retryable() {
        let mut conn = test_conn();
        create(&mut conn, "2026-08-18T01:00:00Z");
        let now = DateTime::parse_from_rfc3339("2026-08-18T02:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let reminders = collect_due_with_conn(&mut conn, now).unwrap();

        conn.execute_batch("CREATE TRIGGER fail_follow_up_ack BEFORE UPDATE OF last_notified_review_at ON follow_up_items BEGIN SELECT RAISE(ABORT,'forced ack failure'); END;").unwrap();
        assert!(acknowledge_dispatches_with_conn(&mut conn, &reminders, now).is_err());
        assert_eq!(collect_due_with_conn(&mut conn, now).unwrap().len(), 1);
        conn.execute_batch("DROP TRIGGER fail_follow_up_ack")
            .unwrap();

        execute_with_conn(&mut conn, "item_snooze", &json!({"id":1,"minutes":60})).unwrap();
        acknowledge_dispatches_with_conn(&mut conn, &reminders, now).unwrap();
        let (last_notified, snooze_until): (Option<String>, String) = conn
            .query_row(
                "SELECT last_notified_review_at,snooze_until FROM follow_up_items WHERE id=1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(last_notified.is_none());
        assert!(collect_due_with_conn(&mut conn, now).unwrap().is_empty());
        let after_snooze = DateTime::parse_from_rfc3339(&snooze_until)
            .unwrap()
            .with_timezone(&Utc)
            + chrono::Duration::minutes(1);
        assert_eq!(
            collect_due_with_conn(&mut conn, after_snooze)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn snooze_keeps_review_time_but_rearms_notification_cycle() {
        let mut conn = test_conn();
        let id = create(&mut conn, "2026-08-18T01:00:00Z")["id"]
            .as_i64()
            .unwrap();
        conn.execute(
            "UPDATE follow_up_items SET last_notified_review_at=review_at WHERE id=?1",
            [id],
        )
        .unwrap();
        execute_with_conn(&mut conn, "item_snooze", &json!({"id":id,"minutes":60})).unwrap();
        let state:(String,Option<String>,Option<String>)=conn.query_row("SELECT review_at,snooze_until,last_notified_review_at FROM follow_up_items WHERE id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
        assert_eq!(state.0, "2026-08-18T01:00:00+00:00");
        assert!(state.1.is_some());
        assert!(state.2.is_none());
    }

    #[test]
    fn core_edit_resets_reminder_state_only_when_review_time_changes() {
        let mut conn = test_conn();
        let id = create(&mut conn, "2026-08-18T01:00:00Z")["id"]
            .as_i64()
            .unwrap();
        conn.execute("UPDATE follow_up_items SET snooze_until='2026-08-18T03:00:00+00:00',last_notified_review_at=review_at WHERE id=?1",[id]).unwrap();
        let base = json!({"id":id,"title":"修改描述","description":"新上下文","priority":"P1","personId":1,"reviewAt":"2026-08-18T01:00:00Z","links":[]});
        execute_with_conn(&mut conn, "item_update", &base).unwrap();
        let kept: (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT snooze_until,last_notified_review_at FROM follow_up_items WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(kept.0.is_some() && kept.1.is_some());
        let mut changed = base;
        changed["reviewAt"] = json!("2026-08-19T01:00:00Z");
        execute_with_conn(&mut conn, "item_update", &changed).unwrap();
        let reset: (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT snooze_until,last_notified_review_at FROM follow_up_items WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(reset, (None, None));
    }

    #[test]
    fn list_searches_progress_and_applies_person_priority_and_status_filters() {
        let mut conn = test_conn();
        let first = create(&mut conn, "2026-08-18T01:00:00Z");
        let first_id = first["id"].as_i64().unwrap();
        execute_with_conn(
            &mut conn,
            "progress_add",
            &json!({"id":first_id,"content":"等待灰度验证"}),
        )
        .unwrap();
        execute_with_conn(
            &mut conn,
            "item_create",
            &json!({"title":"另一个事项","personId":2,"reviewAt":"2026-08-19T01:00:00Z","priority":"P3"}),
        )
        .unwrap();
        let found = execute_with_conn(
            &mut conn,
            "item_list",
            &json!({"keyword":"灰度","personId":1,"priority":"P1","attentionStatus":"active"}),
        )
        .unwrap();
        assert_eq!(found.as_array().unwrap().len(), 1);
        assert_eq!(found[0]["id"], first_id);
    }

    #[test]
    fn rejects_stale_id_and_invalid_lifecycle_transitions() {
        let mut conn = test_conn();
        assert!(execute_with_conn(&mut conn, "item_get", &json!({"id":999})).is_err());
        let id = create(&mut conn, "2026-08-18T01:00:00Z")["id"]
            .as_i64()
            .unwrap();
        assert!(execute_with_conn(
            &mut conn,
            "reopen",
            &json!({"id":id,"reviewAt":"2026-08-19T01:00:00Z"})
        )
        .is_err());
        execute_with_conn(
            &mut conn,
            "confirm_completed",
            &json!({"id":id,"content":"已完成"}),
        )
        .unwrap();
        assert!(execute_with_conn(
            &mut conn,
            "continue_following",
            &json!({"id":id,"content":"继续","reviewAt":"2026-08-20T01:00:00Z"})
        )
        .is_err());
    }

    #[test]
    fn ordinary_progress_is_editable_but_lifecycle_history_is_protected_and_delete_cascades() {
        let mut conn = test_conn();
        let id = create(&mut conn, "2026-08-18T01:00:00Z")["id"]
            .as_i64()
            .unwrap();
        let item = execute_with_conn(
            &mut conn,
            "progress_add",
            &json!({"id":id,"content":"等待反馈"}),
        )
        .unwrap();
        let progress_id = item["progress"][0]["id"].as_i64().unwrap();
        execute_with_conn(
            &mut conn,
            "progress_update",
            &json!({"progressId":progress_id,"content":"已收到初步反馈"}),
        )
        .unwrap();
        execute_with_conn(
            &mut conn,
            "progress_delete",
            &json!({"progressId":progress_id}),
        )
        .unwrap();
        let ended = execute_with_conn(
            &mut conn,
            "confirm_canceled",
            &json!({"id":id,"content":"需求取消"}),
        )
        .unwrap();
        let transition_id = ended["progress"][0]["id"].as_i64().unwrap();
        assert!(execute_with_conn(
            &mut conn,
            "progress_delete",
            &json!({"progressId":transition_id})
        )
        .is_err());
        execute_with_conn(&mut conn, "item_delete", &json!({"id":id})).unwrap();
        let children: i64 = conn
            .query_row("SELECT COUNT(*) FROM follow_up_progress", [], |r| r.get(0))
            .unwrap();
        assert_eq!(children, 0);
    }
}

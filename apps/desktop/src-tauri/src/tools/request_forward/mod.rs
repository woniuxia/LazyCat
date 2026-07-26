use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension};
use serde_json::{json, Value};

use super::helpers::db_conn;
use model::{
    encode_request_forward_error, encode_request_forward_error_with_code, RequestForwardErrorCode,
    RuleWriteInput,
};
use runtime::LifecycleRepository;

pub use runtime::RestoreResult;

mod http;
mod model;
mod observability;
mod preflight;
mod repository;
mod runtime;
mod tcp;
mod udp;
mod validation;

const ACTIONS: &[&str] = &[
    "preflight",
    "list",
    "get",
    "create",
    "update",
    "delete",
    "start",
    "stop",
    "start_all",
    "stop_all",
    "auto_start_update",
    "status",
    "log_list",
    "log_clear",
    "stats_get",
    "stats_reset",
];

pub fn encode_preflight_task_error(message: &str) -> String {
    encode_request_forward_error_with_code(
        message,
        runtime::RuntimeState::Stopped.as_str(),
        RequestForwardErrorCode::Unknown,
    )
}

struct DatabaseLifecycleRepository;

impl LifecycleRepository for DatabaseLifecycleRepository {
    fn auto_start_rules(&self) -> Result<Vec<model::ForwardRule>, String> {
        let conn = db_conn()?;
        Ok(repository::list_with_conn(&conn)?
            .into_iter()
            .filter(|rule| rule.auto_start)
            .collect())
    }
}

pub fn initialize_manager() -> Result<(), String> {
    db_conn()?;
    // 工具分发当前是同步全局入口；进程级唯一 runtime 避免每个 action 都耦合 AppHandle。
    let _ = runtime::global_manager();
    Ok(())
}

pub(crate) fn list_action_targets_with_conn(
    conn: &Connection,
) -> Result<Vec<(String, String)>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name FROM request_forward_rules ORDER BY id ASC")
        .map_err(|error| format!("查询请求转发动作目标失败: {error}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?.to_string(), row.get(1)?))
        })
        .map_err(|error| format!("查询请求转发动作目标失败: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("读取请求转发动作目标失败: {error}"))
}

pub(crate) fn load_action_target_with_conn(
    conn: &Connection,
    id: i64,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT name FROM request_forward_rules WHERE id = ?1",
        [id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| format!("读取请求转发动作目标失败: {error}"))
}

fn start_action_target_with_conn_and_manager(
    conn: &Connection,
    manager: &runtime::RuntimeManager,
    target_id: &str,
) -> Result<bool, String> {
    let id = target_id
        .parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| format!("请求转发动作目标 ID 无效: {target_id}"))?;
    match manager.status(id).state {
        runtime::RuntimeState::Running => {
            repository::get_with_conn(conn, id)?;
            Ok(false)
        }
        runtime::RuntimeState::Stopped | runtime::RuntimeState::Failed => {
            manager.start_loaded(id, || repository::get_with_conn(conn, id))?;
            Ok(true)
        }
        state => Err(format!(
            "请求转发规则当前状态不允许启动: {}",
            state.as_str()
        )),
    }
}

pub(crate) fn start_action_target(target_id: &str) -> Result<bool, String> {
    let conn = db_conn()?;
    start_action_target_with_conn_and_manager(&conn, runtime::global_manager(), target_id)
}

pub fn restore_auto_start_rules() -> Result<Vec<RestoreResult>, String> {
    runtime::global_manager().restore_auto_start_rules(&DatabaseLifecycleRepository)
}

pub fn on_app_exit() {
    for result in runtime::global_manager()
        .on_app_exit()
        .into_iter()
        .filter(|result| !result.ok)
    {
        eprintln!(
            "request-forward shutdown failed for rule {}: {}",
            result.rule_id,
            result.error.as_deref().unwrap_or("未知错误")
        );
    }
}

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

#[cfg(test)]
mod action_contract_tests {
    use serde_json::json;

    use super::{
        parse_auto_start_enabled, parse_batch_rule_ids, resolve_batch_rule_ids, supported_actions,
    };

    #[test]
    fn supports_explicit_auto_start_update_action() {
        assert!(supported_actions().contains(&"auto_start_update"));
    }

    #[test]
    fn auto_start_update_requires_an_explicit_boolean() {
        assert!(parse_auto_start_enabled(&json!({ "enabled": true })).unwrap());
        assert!(!parse_auto_start_enabled(&json!({ "enabled": false })).unwrap());
        assert!(parse_auto_start_enabled(&json!({})).is_err());
        assert!(parse_auto_start_enabled(&json!({ "enabled": 1 })).is_err());
    }

    #[test]
    fn batch_ids_are_optional_positive_unique_integers() {
        assert_eq!(parse_batch_rule_ids(&json!({})).unwrap(), None);
        assert_eq!(
            parse_batch_rule_ids(&json!({ "ids": [3, 3, 0, -1, "2", 5] })).unwrap(),
            Some(vec![3, 5])
        );
        assert_eq!(
            parse_batch_rule_ids(&json!({ "ids": [] })).unwrap(),
            Some(vec![])
        );
        assert!(parse_batch_rule_ids(&json!({ "ids": "all" })).is_err());
    }

    #[test]
    fn explicit_batch_ids_bypass_the_legacy_all_rules_scope() {
        assert_eq!(
            resolve_batch_rule_ids(&json!({ "ids": [3, 1, 3] }), || {
                panic!("explicit scope must not load all rule ids")
            })
            .unwrap(),
            vec![3, 1]
        );
        assert_eq!(
            resolve_batch_rule_ids(&json!({}), || Ok(vec![8, 9])).unwrap(),
            vec![8, 9]
        );
    }
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    execute_inner(action, payload).map_err(|message| {
        let state = payload
            .get("id")
            .and_then(Value::as_i64)
            .filter(|id| *id > 0)
            .map(|id| runtime::global_manager().status(id).state)
            .unwrap_or(runtime::RuntimeState::Stopped);
        encode_request_forward_error(&message, state.as_str())
    })
}

fn execute_inner(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err("unsupported request_forward action".into());
    }
    match action {
        "preflight" => {
            let input = parse_rule_input(payload)?;
            Ok(json!(preflight::preflight(input)?))
        }
        "list" => {
            let conn = db_conn()?;
            Ok(json!({ "items": repository::list_with_conn(&conn)? }))
        }
        "get" => {
            let conn = db_conn()?;
            Ok(json!({ "item": repository::get_with_conn(&conn, parse_rule_id(payload)?)? }))
        }
        "create" => {
            let mut conn = db_conn()?;
            let input = parse_rule_input(payload)?;
            Ok(json!({ "item": repository::create_with_conn(&mut conn, input)? }))
        }
        "update" => {
            let conn = db_conn()?;
            let id = parse_rule_id(payload)?;
            let input = parse_update_rule_input(payload)?;
            let item = runtime::global_manager()
                .with_rule_mutation(id, || repository::update_with_conn(&conn, id, input))?;
            Ok(json!({ "item": item }))
        }
        "delete" => {
            let conn = db_conn()?;
            let id = parse_rule_id(payload)?;
            let manager = runtime::global_manager();
            manager.with_rule_mutation(id, || {
                repository::delete_with_conn(&conn, id)?;
                manager.clear_rule_state(id);
                Ok(())
            })?;
            Ok(json!({ "ok": true }))
        }
        "start" => {
            let conn = db_conn()?;
            let id = parse_rule_id(payload)?;
            let status = runtime::global_manager()
                .start_loaded(id, || repository::get_with_conn(&conn, id))?;
            Ok(json!({ "item": status }))
        }
        "stop" => {
            let conn = db_conn()?;
            let id = parse_rule_id(payload)?;
            let status = runtime::global_manager()
                .stop_loaded(id, || repository::get_with_conn(&conn, id))?;
            Ok(json!({ "item": status }))
        }
        "start_all" => {
            let conn = db_conn()?;
            let rule_ids = resolve_batch_rule_ids(payload, || {
                Ok(repository::list_with_conn(&conn)?
                    .into_iter()
                    .map(|rule| rule.id)
                    .collect::<Vec<_>>())
            })?;
            let results = runtime::global_manager()
                .start_all_loaded(&rule_ids, |id| repository::get_with_conn(&conn, id));
            Ok(json!({ "results": results }))
        }
        "stop_all" => {
            let conn = db_conn()?;
            let rule_ids = resolve_batch_rule_ids(payload, || {
                Ok(repository::list_with_conn(&conn)?
                    .into_iter()
                    .map(|rule| rule.id)
                    .collect::<Vec<_>>())
            })?;
            let results = runtime::global_manager()
                .stop_all_loaded(&rule_ids, |id| repository::get_with_conn(&conn, id));
            Ok(json!({ "results": results }))
        }
        "auto_start_update" => {
            let conn = db_conn()?;
            let id = parse_rule_id(payload)?;
            let enabled = parse_auto_start_enabled(payload)?;
            let item = runtime::global_manager().with_auto_start_mutation(id, || {
                repository::set_auto_start_with_conn(&conn, id, enabled)?;
                repository::get_with_conn(&conn, id)
            })?;
            Ok(json!({ "item": item }))
        }
        "status" => {
            let conn = db_conn()?;
            if payload.get("id").is_some() {
                let rule = repository::get_with_conn(&conn, parse_rule_id(payload)?)?;
                Ok(json!({ "item": runtime::global_manager().status(rule.id) }))
            } else {
                let rules = repository::list_with_conn(&conn)?;
                let items =
                    runtime::global_manager().statuses(rules.into_iter().map(|rule| rule.id));
                Ok(json!({ "items": items }))
            }
        }
        "log_list" => {
            let conn = db_conn()?;
            let page = repository::list_logs_with_conn(&conn, &parse_log_query(payload)?)?;
            Ok(json!({ "items": page.items, "total": page.total, "latestId": page.latest_id }))
        }
        "log_clear" => {
            let conn = db_conn()?;
            repository::clear_logs_with_conn(&conn, parse_rule_id(payload)?)?;
            Ok(json!({ "ok": true }))
        }
        "stats_get" => {
            let id = parse_rule_id(payload)?;
            Ok(json!({ "item": runtime::global_manager().stats(id)? }))
        }
        "stats_reset" => {
            let id = parse_rule_id(payload)?;
            Ok(json!({ "item": runtime::global_manager().reset_stats(id)? }))
        }
        _ => Err("request_forward action not implemented".into()),
    }
}

fn parse_rule_id(payload: &Value) -> Result<i64, String> {
    payload
        .get("id")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| "转发规则 ID 无效".into())
}

fn parse_auto_start_enabled(payload: &Value) -> Result<bool, String> {
    payload
        .get("enabled")
        .and_then(Value::as_bool)
        .ok_or_else(|| "自动恢复开关参数无效".to_string())
}

fn parse_batch_rule_ids(payload: &Value) -> Result<Option<Vec<i64>>, String> {
    let Some(value) = payload.get("ids") else {
        return Ok(None);
    };
    let values = value
        .as_array()
        .ok_or_else(|| "批量规则 ID 必须是数组".to_string())?;
    let mut seen = HashSet::new();
    let ids = values
        .iter()
        .filter_map(Value::as_i64)
        .filter(|id| *id > 0 && seen.insert(*id))
        .collect();
    Ok(Some(ids))
}

fn resolve_batch_rule_ids(
    payload: &Value,
    load_all_rule_ids: impl FnOnce() -> Result<Vec<i64>, String>,
) -> Result<Vec<i64>, String> {
    match parse_batch_rule_ids(payload)? {
        Some(ids) => Ok(ids),
        None => load_all_rule_ids(),
    }
}

fn parse_rule_input(payload: &Value) -> Result<RuleWriteInput, String> {
    serde_json::from_value(payload.clone()).map_err(|e| format!("转发规则参数无效: {e}"))
}

fn parse_update_rule_input(payload: &Value) -> Result<RuleWriteInput, String> {
    let mut rule_payload = payload
        .as_object()
        .cloned()
        .ok_or_else(|| "转发规则参数无效".to_string())?;
    rule_payload.remove("id");
    parse_rule_input(&Value::Object(rule_payload))
}

fn parse_log_query(payload: &Value) -> Result<repository::LogQuery, String> {
    let rule_id = parse_rule_id(payload)?;
    let keyword = match payload.get("keyword") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(_) => return Err("转发日志关键词无效".into()),
    };
    let outcome = match payload.get("mode") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if value == "success" => Some(repository::LogOutcome::Success),
        Some(Value::String(value)) if value == "error" => Some(repository::LogOutcome::Error),
        Some(_) => return Err("转发日志结果模式无效".into()),
    };
    let method = parse_optional_log_text(payload, "method", "转发日志 Method 无效")?
        .map(|value| value.to_ascii_uppercase());
    let status_code = match payload.get("statusCode") {
        None | Some(Value::Null) => None,
        Some(value) => value
            .as_u64()
            .filter(|value| (100..=599).contains(value))
            .and_then(|value| u16::try_from(value).ok())
            .ok_or_else(|| "转发日志状态码无效".to_string())
            .map(Some)?,
    };
    let started_at = parse_optional_log_time(payload, "startedAt", "转发日志开始时间无效")?;
    let ended_at = parse_optional_log_time(payload, "endedAt", "转发日志结束时间无效")?;
    if let (Some(started_at), Some(ended_at)) = (&started_at, &ended_at) {
        if started_at > ended_at {
            return Err("转发日志时间范围无效".into());
        }
    }
    let offset = match payload.get("offset") {
        None | Some(Value::Null) => 0,
        Some(value) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| "转发日志分页偏移无效".to_string())?,
    };
    let limit = match payload.get("limit") {
        None | Some(Value::Null) => 100,
        Some(value) => value
            .as_u64()
            .filter(|value| (1..=1000).contains(value))
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| "转发日志分页大小无效".to_string())?,
    };
    Ok(repository::LogQuery {
        rule_id,
        keyword,
        outcome,
        method,
        status_code,
        started_at,
        ended_at,
        offset,
        limit,
    })
}

fn parse_optional_log_text(
    payload: &Value,
    key: &str,
    message: &str,
) -> Result<Option<String>, String> {
    match payload.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => {
            let value = value.trim();
            Ok((!value.is_empty()).then(|| value.to_string()))
        }
        Some(_) => Err(message.into()),
    }
}

fn parse_optional_log_time(
    payload: &Value,
    key: &str,
    message: &str,
) -> Result<Option<String>, String> {
    let Some(value) = parse_optional_log_text(payload, key, message)? else {
        return Ok(None);
    };
    if let Ok(value) = chrono::DateTime::parse_from_rfc3339(&value) {
        return Ok(Some(
            value
                .with_timezone(&chrono::Utc)
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string(),
        ));
    }
    let normalized = value.replace('T', " ");
    let parsed = chrono::NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%d %H:%M:%S%.f")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%d %H:%M"))
        .map_err(|_| message.to_string())?;
    Ok(Some(parsed.format("%Y-%m-%d %H:%M:%S%.3f").to_string()))
}

#[cfg(test)]
mod tests {
    use super::model::{
        classify_request_forward_error, encode_request_forward_error, ForwardProtocol,
        RequestForwardErrorCode, RuleWriteInput,
    };
    use super::runtime::{RuleRunner, RunningHandle, RuntimeManager, RuntimeState, RuntimeStatus};
    use super::{repository, validation};
    use crate::tools::helpers::ensure_request_forward_schema_for_test;
    use rusqlite::{params, Connection};
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    #[derive(Default)]
    struct ActionTargetRunner {
        fail_next: AtomicBool,
        starts: AtomicUsize,
    }

    impl RuleRunner for ActionTargetRunner {
        fn start(&self, _rule: &super::model::ForwardRule) -> Result<RunningHandle, String> {
            let attempt = self.starts.fetch_add(1, Ordering::SeqCst) + 1;
            if self.fail_next.swap(false, Ordering::SeqCst) {
                return Err("expected start failure".into());
            }
            Ok(RunningHandle(attempt as u64))
        }

        fn stop(&self, _handle: RunningHandle) -> Result<(), String> {
            Ok(())
        }
    }

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .expect("enable foreign keys");
        ensure_request_forward_schema_for_test(&conn).expect("create request forward schema");
        conn
    }

    fn http_input() -> RuleWriteInput {
        RuleWriteInput {
            name: "本地 API".into(),
            protocol: ForwardProtocol::Http,
            bind_host: "127.0.0.1".into(),
            listen_port: 8080,
            target_url: Some("https://example.com/api/".into()),
            target_host: None,
            target_port: None,
            capture_http_headers: true,
            capture_http_body: false,
        }
    }

    fn tcp_input() -> RuleWriteInput {
        RuleWriteInput {
            name: "数据库端口".into(),
            protocol: ForwardProtocol::Tcp,
            bind_host: "127.0.0.1".into(),
            listen_port: 5433,
            target_url: None,
            target_host: Some("192.168.1.10".into()),
            target_port: Some(5432),
            capture_http_headers: false,
            capture_http_body: false,
        }
    }

    #[test]
    fn runtime_errors_have_stable_codes_with_windows_and_case_coverage() {
        let cases = [
            (
                "HTTP 监听绑定失败: OS error 10048: Only one usage of each socket address is normally permitted",
                RequestForwardErrorCode::ListenerInUse,
            ),
            (
                "udp 监听绑定失败: ADDRESS ALREADY IN USE",
                RequestForwardErrorCode::ListenerInUse,
            ),
            (
                "解析目标地址 api.invalid:443 失败: No such host is known",
                RequestForwardErrorCode::DnsFailed,
            ),
            (
                "解析目标地址 tls-proxy.invalid:443 失败: No such host is known",
                RequestForwardErrorCode::DnsFailed,
            ),
            (
                "解析目标地址 certificate-api.invalid:443 失败: No such host is known",
                RequestForwardErrorCode::DnsFailed,
            ),
            (
                "解析下游 tls-proxy.invalid:443 失败: No such host is known",
                RequestForwardErrorCode::DnsFailed,
            ),
            (
                "连接下游 10.0.0.8:443 失败: connection refused",
                RequestForwardErrorCode::TargetUnreachable,
            ),
            (
                "连接下游 tls-proxy.invalid:443 失败: connection refused",
                RequestForwardErrorCode::TargetUnreachable,
            ),
            (
                "client error (Connect): invalid peer certificate: certificate not valid for name",
                RequestForwardErrorCode::TlsFailed,
            ),
            (
                "TLS handshake failed: invalid dnsname for certificate",
                RequestForwardErrorCode::TlsFailed,
            ),
            (
                "目标地址与监听地址相同，不能直接转发到自身",
                RequestForwardErrorCode::SelfForward,
            ),
            ("HTTP 目标 URL 格式不正确", RequestForwardErrorCode::InvalidConfig),
            (
                "HTTP 目标 URL 不能包含 query 或 fragment",
                RequestForwardErrorCode::InvalidConfig,
            ),
            (
                "已保存规则不能修改协议，请新建规则",
                RequestForwardErrorCode::InvalidConfig,
            ),
            (
                "已启动的转发规则不能修改或删除",
                RequestForwardErrorCode::LifecycleConflict,
            ),
            ("创建转发规则失败: 磁盘只读", RequestForwardErrorCode::PersistenceFailed),
            (
                "failed to persist request forward rule: disk is read-only",
                RequestForwardErrorCode::PersistenceFailed,
            ),
            (
                "连接下游 10.0.0.8:443 失败: OS error 10048",
                RequestForwardErrorCode::TargetUnreachable,
            ),
        ];

        for (message, expected) in cases {
            assert_eq!(
                classify_request_forward_error(message),
                expected,
                "{message}"
            );
        }
        assert_eq!(
            classify_request_forward_error("connect button label failed to render"),
            RequestForwardErrorCode::Unknown,
            "an arbitrary mention of connect must not look like a listener conflict"
        );
    }

    #[test]
    fn structured_error_envelope_keeps_original_message_and_actual_state() {
        let original = "opaque runner failure: 保留原文";
        let encoded = encode_request_forward_error(original, RuntimeState::Running.as_str());
        let decoded: serde_json::Value =
            serde_json::from_str(&encoded).expect("valid envelope JSON");

        assert_eq!(decoded["marker"], "lazycat.request_forward.error");
        assert_eq!(decoded["version"], 1);
        assert_eq!(decoded["code"], "unknown");
        assert_eq!(decoded["message"], original);
        assert_eq!(decoded["state"], "running");
    }

    #[test]
    fn preflight_task_error_is_an_unknown_stopped_envelope() {
        let original = "配置预检任务异常结束: task 17 panicked";
        let encoded = super::encode_preflight_task_error(original);
        let decoded: serde_json::Value =
            serde_json::from_str(&encoded).expect("valid preflight task envelope");

        assert_eq!(decoded["marker"], "lazycat.request_forward.error");
        assert_eq!(decoded["version"], 1);
        assert_eq!(decoded["code"], "unknown");
        assert_eq!(decoded["message"], original);
        assert_eq!(decoded["state"], "stopped");
    }

    #[test]
    fn schema_creates_rules_stats_logs_and_cascade() {
        let conn = test_conn();
        for name in [
            "request_forward_rules",
            "request_forward_stats",
            "request_forward_logs",
        ] {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [name],
                    |row| row.get(0),
                )
                .expect("query table");
            assert_eq!(exists, 1, "{name} should exist");
        }
        let index_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                ["idx_request_forward_logs_rule_created"],
                |row| row.get(0),
            )
            .expect("query index");
        assert_eq!(index_exists, 1);

        conn.execute(
            "INSERT INTO request_forward_rules
             (name, protocol, bind_host, listen_port, capture_http_headers, capture_http_body, auto_start, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            params!["级联规则", "tcp", "127.0.0.1", 9010, 1, 0, 0],
        )
        .expect("insert rule");
        let rule_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO request_forward_stats(rule_id, updated_at) VALUES (?1, CURRENT_TIMESTAMP)",
            [rule_id],
        )
        .expect("insert stats");
        conn.execute(
            "INSERT INTO request_forward_logs(rule_id, protocol, target_addr, created_at)
             VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)",
            params![rule_id, "tcp", "192.168.1.10:5432"],
        )
        .expect("insert log");

        conn.execute("DELETE FROM request_forward_rules WHERE id = ?1", [rule_id])
            .expect("delete rule");
        let stat_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM request_forward_stats WHERE rule_id = ?1",
                [rule_id],
                |row| row.get(0),
            )
            .expect("query stats");
        let log_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM request_forward_logs WHERE rule_id = ?1",
                [rule_id],
                |row| row.get(0),
            )
            .expect("query logs");
        assert_eq!(stat_count, 0);
        assert_eq!(log_count, 0);
    }

    #[test]
    fn validation_rejects_protocol_field_mismatch_and_self_forward() {
        let mut invalid_bind = http_input();
        invalid_bind.bind_host = "localhost".into();
        assert!(validation::validate_rule_input(invalid_bind).is_err());

        let mut invalid_listener_port = http_input();
        invalid_listener_port.listen_port = 0;
        assert!(validation::validate_rule_input(invalid_listener_port).is_err());

        let mut invalid_http_scheme = http_input();
        invalid_http_scheme.target_url = Some("ftp://example.com".into());
        assert!(validation::validate_rule_input(invalid_http_scheme).is_err());

        let mut invalid_http_query = http_input();
        invalid_http_query.target_url = Some("https://example.com/api?q=1".into());
        assert!(validation::validate_rule_input(invalid_http_query).is_err());

        let mut invalid_http_fragment = http_input();
        invalid_http_fragment.target_url = Some("https://example.com/api#part".into());
        assert!(validation::validate_rule_input(invalid_http_fragment).is_err());

        let mut http_with_target_host = http_input();
        http_with_target_host.target_host = Some("192.168.1.10".into());
        assert!(validation::validate_rule_input(http_with_target_host).is_err());

        let mut http_with_target_port = http_input();
        http_with_target_port.target_port = Some(443);
        assert!(validation::validate_rule_input(http_with_target_port).is_err());

        let mut tcp_with_url = tcp_input();
        tcp_with_url.target_url = Some("https://example.com".into());
        assert!(validation::validate_rule_input(tcp_with_url).is_err());

        let mut tcp_without_host = tcp_input();
        tcp_without_host.target_host = None;
        assert!(validation::validate_rule_input(tcp_without_host).is_err());

        let mut udp_without_port = tcp_input();
        udp_without_port.protocol = ForwardProtocol::Udp;
        udp_without_port.target_port = None;
        assert!(validation::validate_rule_input(udp_without_port).is_err());

        let mut invalid_target_port = tcp_input();
        invalid_target_port.target_port = Some(0);
        assert!(validation::validate_rule_input(invalid_target_port).is_err());

        let mut self_forward = tcp_input();
        self_forward.target_host = Some("127.0.0.1".into());
        self_forward.target_port = Some(5433);
        assert!(validation::validate_rule_input(self_forward).is_err());

        let normalized = validation::validate_rule_input(http_input()).expect("valid HTTP input");
        assert_eq!(
            normalized.target_url.as_deref(),
            Some("https://example.com/api")
        );
    }

    #[test]
    fn validation_rejects_http_self_forward_for_ipv4_and_equivalent_ipv6() {
        let mut default_port_self_forward = http_input();
        default_port_self_forward.listen_port = 80;
        default_port_self_forward.target_url = Some("http://127.0.0.1/api".into());
        assert!(validation::validate_rule_input(default_port_self_forward).is_err());

        let mut ipv4_self_forward = http_input();
        ipv4_self_forward.target_url = Some("http://127.0.0.1:8080/api".into());
        assert!(validation::validate_rule_input(ipv4_self_forward).is_err());

        let mut ipv6_self_forward = http_input();
        ipv6_self_forward.bind_host = "0:0:0:0:0:0:0:1".into();
        ipv6_self_forward.listen_port = 8443;
        ipv6_self_forward.target_url = Some("https://[::1]:8443/api".into());
        assert!(validation::validate_rule_input(ipv6_self_forward).is_err());
    }

    #[test]
    fn validation_rejects_http_explicit_port_zero() {
        let mut invalid_http_port = http_input();
        invalid_http_port.target_url = Some("http://example.com:0/api".into());
        assert!(validation::validate_rule_input(invalid_http_port).is_err());
    }

    #[test]
    fn repository_crud_creates_stats_and_reports_missing_rules() {
        let mut conn = test_conn();
        let created = repository::create_with_conn(&mut conn, http_input()).expect("create rule");
        assert!(!created.auto_start);
        assert_eq!(
            created.target_url.as_deref(),
            Some("https://example.com/api")
        );
        let stats_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM request_forward_stats WHERE rule_id = ?1 AND event_count = 0 AND upload_bytes = 0 AND download_bytes = 0 AND error_count = 0",
                [created.id],
                |row| row.get(0),
            )
            .expect("query zeroed stats");
        assert_eq!(stats_count, 1);

        let listed = repository::list_with_conn(&conn).expect("list rules");
        assert_eq!(listed, vec![created.clone()]);
        assert_eq!(
            repository::get_with_conn(&conn, created.id).unwrap(),
            created
        );

        let mut update = http_input();
        update.name = "已更新规则".into();
        let updated = repository::update_with_conn(&conn, created.id, update).expect("update rule");
        assert_eq!(updated.name, "已更新规则");
        assert_eq!(updated.protocol, ForwardProtocol::Http);
        assert_eq!(
            updated.target_url.as_deref(),
            Some("https://example.com/api")
        );

        repository::set_auto_start_with_conn(&conn, created.id, true).expect("enable auto-start");
        assert!(
            repository::get_with_conn(&conn, created.id)
                .expect("read enabled auto-start")
                .auto_start
        );
        repository::set_auto_start_with_conn(&conn, created.id, false).expect("disable auto-start");
        assert!(
            !repository::get_with_conn(&conn, created.id)
                .expect("read disabled auto-start")
                .auto_start
        );

        repository::delete_with_conn(&conn, created.id).expect("delete rule");
        assert!(repository::list_with_conn(&conn).unwrap().is_empty());
        assert!(repository::get_with_conn(&conn, created.id)
            .expect_err("deleted rule missing")
            .contains("不存在"));
        assert!(
            repository::update_with_conn(&conn, created.id, http_input())
                .expect_err("missing update")
                .contains("不存在")
        );
        assert!(repository::delete_with_conn(&conn, created.id)
            .expect_err("missing delete")
            .contains("不存在"));
        assert!(
            repository::set_auto_start_with_conn(&conn, created.id, true)
                .expect_err("missing auto-start update")
                .contains("不存在")
        );
    }

    #[test]
    fn request_forward_running_is_already_satisfied_without_auto_start_mutation() {
        let mut conn = test_conn();
        let rule = repository::create_with_conn(&mut conn, http_input()).unwrap();
        assert!(!rule.auto_start);

        let runner = Arc::new(ActionTargetRunner::default());
        let manager = RuntimeManager::new(runner.clone());
        assert!(super::start_action_target_with_conn_and_manager(
            &conn,
            &manager,
            &rule.id.to_string(),
        )
        .unwrap());
        assert_eq!(manager.status(rule.id).state, RuntimeState::Running);
        assert!(!super::start_action_target_with_conn_and_manager(
            &conn,
            &manager,
            &rule.id.to_string(),
        )
        .unwrap());
        assert_eq!(runner.starts.load(Ordering::SeqCst), 1);
        assert!(
            !repository::get_with_conn(&conn, rule.id)
                .unwrap()
                .auto_start
        );
        manager
            .stop_loaded(rule.id, || repository::get_with_conn(&conn, rule.id))
            .unwrap();

        let retry_runner = Arc::new(ActionTargetRunner::default());
        retry_runner.fail_next.store(true, Ordering::SeqCst);
        let retry_manager = RuntimeManager::new(retry_runner.clone());
        assert!(retry_manager
            .start_loaded(rule.id, || repository::get_with_conn(&conn, rule.id))
            .is_err());
        assert_eq!(retry_manager.status(rule.id).state, RuntimeState::Failed);
        assert!(super::start_action_target_with_conn_and_manager(
            &conn,
            &retry_manager,
            &rule.id.to_string(),
        )
        .unwrap());
        assert_eq!(retry_manager.status(rule.id).state, RuntimeState::Running);
        assert_eq!(retry_runner.starts.load(Ordering::SeqCst), 2);
        assert!(
            !repository::get_with_conn(&conn, rule.id)
                .unwrap()
                .auto_start
        );
        retry_manager
            .stop_loaded(rule.id, || repository::get_with_conn(&conn, rule.id))
            .unwrap();
    }

    #[test]
    fn repository_rejects_blank_names_and_persists_trimmed_names() {
        let mut conn = test_conn();
        let mut blank_create = http_input();
        blank_create.name = " \t ".into();
        assert!(repository::create_with_conn(&mut conn, blank_create)
            .expect_err("blank create name must fail")
            .contains("名称"));

        let mut trimmed_create = http_input();
        trimmed_create.name = "  本地 API  ".into();
        let created = repository::create_with_conn(&mut conn, trimmed_create)
            .expect("trimmed create succeeds");
        assert_eq!(created.name, "本地 API");

        let mut blank_update = http_input();
        blank_update.name = "\r\n".into();
        assert!(
            repository::update_with_conn(&conn, created.id, blank_update)
                .expect_err("blank update name must fail")
                .contains("名称")
        );
        assert_eq!(
            repository::get_with_conn(&conn, created.id)
                .expect("read unchanged rule")
                .name,
            "本地 API"
        );

        let mut trimmed_update = http_input();
        trimmed_update.name = "  已更新名称  ".into();
        let updated = repository::update_with_conn(&conn, created.id, trimmed_update)
            .expect("trimmed update succeeds");
        assert_eq!(updated.name, "已更新名称");
    }

    #[test]
    fn repository_rejects_protocol_changes_but_allows_same_protocol_updates() {
        let mut conn = test_conn();
        let created = repository::create_with_conn(&mut conn, http_input()).expect("create HTTP");

        for protocol in [ForwardProtocol::Tcp, ForwardProtocol::Udp] {
            let mut switched = tcp_input();
            switched.protocol = protocol;
            let error = repository::update_with_conn(&conn, created.id, switched)
                .expect_err("persisted protocol is immutable");
            assert!(error.contains("协议"));
            assert_eq!(
                repository::get_with_conn(&conn, created.id)
                    .expect("read unchanged rule")
                    .protocol,
                ForwardProtocol::Http
            );
        }

        let mut same_protocol = http_input();
        same_protocol.name = "同协议更新".into();
        let updated = repository::update_with_conn(&conn, created.id, same_protocol)
            .expect("same protocol update succeeds");
        assert_eq!(updated.protocol, ForwardProtocol::Http);
        assert_eq!(updated.name, "同协议更新");
    }

    #[test]
    fn action_rejects_unknown_action() {
        let err = super::execute("unknown", &json!({})).expect_err("unknown action should fail");
        assert!(err.contains("unsupported request_forward action"));
    }

    #[test]
    fn preflight_action_contract_serializes_camel_case_checks() {
        let listener =
            std::net::UdpSocket::bind("127.0.0.1:0").expect("reserve temporary UDP listener port");
        let listen_port = listener
            .local_addr()
            .expect("read temporary UDP listener port")
            .port();
        drop(listener);

        assert!(super::supported_actions().contains(&"preflight"));
        let value = super::execute(
            "preflight",
            &json!({
                "name": "UDP 预检",
                "protocol": "udp",
                "bindHost": "127.0.0.1",
                "listenPort": listen_port,
                "targetUrl": null,
                "targetHost": "127.0.0.1",
                "targetPort": 9,
                "captureHttpHeaders": false,
                "captureHttpBody": false
            }),
        )
        .expect("execute UDP preflight action");

        assert_eq!(value["ready"], true);
        assert!(value["checks"].as_array().is_some_and(|checks| {
            checks.iter().any(|check| {
                check["kind"] == "listener"
                    && check["state"] == "passed"
                    && check["message"].is_string()
            })
        }));
        assert!(value.get("suggestedListenPort").is_some());
        assert!(value.get("suggested_listen_port").is_none());
    }

    #[test]
    fn preflight_action_rejects_invalid_payload() {
        let error = super::execute("preflight", &json!({ "protocol": "tcp" }))
            .expect_err("invalid preflight payload must remain an action error");

        assert!(error.contains("转发规则参数无效"));
    }

    #[test]
    fn log_and_status_actions_serialize_camel_case_output() {
        let query = super::parse_log_query(&json!({
            "id": 7,
            "keyword": "timeout",
            "mode": "error",
            "method": " post ",
            "statusCode": 502,
            "startedAt": "2026-07-15T08:30",
            "endedAt": "2026-07-15T09:30:00.250",
            "offset": 20,
            "limit": 50
        }))
        .expect("parse log query");
        assert_eq!(query.rule_id, 7);
        assert_eq!(query.keyword.as_deref(), Some("timeout"));
        assert_eq!(query.outcome, Some(repository::LogOutcome::Error));
        assert_eq!(query.method.as_deref(), Some("POST"));
        assert_eq!(query.status_code, Some(502));
        assert_eq!(query.started_at.as_deref(), Some("2026-07-15 08:30:00.000"));
        assert_eq!(query.ended_at.as_deref(), Some("2026-07-15 09:30:00.250"));
        assert_eq!(query.offset, 20);
        assert_eq!(query.limit, 50);

        let value = serde_json::to_value(RuntimeStatus {
            rule_id: 7,
            state: RuntimeState::Running,
            last_error: None,
            last_observability_error: Some("database is read-only".into()),
        })
        .expect("serialize status");
        assert_eq!(value["ruleId"], 7);
        assert_eq!(value["lastObservabilityError"], "database is read-only");
        assert!(value.get("last_observability_error").is_none());

        let log_value = serde_json::to_value(repository::ForwardLog {
            id: 1,
            rule_id: 7,
            protocol: ForwardProtocol::Http,
            client_addr: Some("127.0.0.1:12345".into()),
            target_addr: "example.com:80".into(),
            method: Some("GET".into()),
            path: Some("/health".into()),
            status_code: Some(200),
            duration_ms: Some(12),
            upload_bytes: 3,
            download_bytes: 4,
            request_headers: None,
            response_headers: None,
            request_body_preview: None,
            response_body_preview: None,
            request_body_truncated: false,
            response_body_truncated: false,
            error: None,
            created_at: "2026-07-15 00:00:00".into(),
        })
        .expect("serialize log");
        assert_eq!(log_value["ruleId"], 7);
        assert_eq!(log_value["clientAddr"], "127.0.0.1:12345");
        assert_eq!(log_value["statusCode"], 200);
        assert!(log_value.get("client_addr").is_none());
    }

    #[test]
    fn log_query_rejects_invalid_mode_and_pagination() {
        assert!(super::parse_log_query(&json!({ "id": 1, "mode": "all" })).is_err());
        assert!(super::parse_log_query(&json!({ "id": 1, "limit": 0 })).is_err());
        assert!(super::parse_log_query(&json!({ "id": 1, "offset": -1 })).is_err());
        assert!(super::parse_log_query(&json!({ "id": 1, "method": 1 })).is_err());
        assert!(super::parse_log_query(&json!({ "id": 1, "statusCode": 99 })).is_err());
        assert!(super::parse_log_query(&json!({ "id": 1, "statusCode": 600 })).is_err());
        assert!(super::parse_log_query(&json!({ "id": 1, "startedAt": "today" })).is_err());
        assert!(super::parse_log_query(&json!({
            "id": 1,
            "startedAt": "2026-07-15T10:00",
            "endedAt": "2026-07-15T09:00"
        }))
        .is_err());
    }

    #[test]
    fn log_query_normalizes_rfc3339_times_to_utc() {
        let query = super::parse_log_query(&json!({
            "id": 1,
            "startedAt": "2026-07-15T16:00:00+08:00",
            "endedAt": "2026-07-15T08:00:00Z"
        }))
        .expect("parse equivalent RFC3339 boundaries");

        assert_eq!(query.started_at.as_deref(), Some("2026-07-15 08:00:00.000"));
        assert_eq!(query.ended_at.as_deref(), Some("2026-07-15 08:00:00.000"));
    }
}

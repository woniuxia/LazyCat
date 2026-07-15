use serde_json::{json, Value};

use super::helpers::db_conn;
use model::RuleWriteInput;
use runtime::{AutoStartPersistence, LifecycleRepository};

pub use runtime::RestoreResult;

mod http;
mod model;
mod observability;
mod repository;
mod runtime;
mod tcp;
mod udp;
mod validation;

const ACTIONS: &[&str] = &[
    "list",
    "get",
    "create",
    "update",
    "delete",
    "start",
    "stop",
    "start_all",
    "stop_all",
    "status",
    "log_list",
    "log_clear",
    "stats_get",
    "stats_reset",
];

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

pub fn restore_auto_start_rules() -> Result<Vec<RestoreResult>, String> {
    runtime::global_manager().restore_auto_start_rules(&DatabaseLifecycleRepository)
}

pub fn on_app_exit() {
    runtime::global_manager().on_app_exit();
}

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err("unsupported request_forward action".into());
    }
    match action {
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
            let persistence = DatabaseAutoStartPersistence { conn: &conn };
            let status = runtime::global_manager()
                .start_loaded(id, &persistence, || repository::get_with_conn(&conn, id))
                .map_err(|error| runtime_action_error(id, error))?;
            Ok(json!({ "item": status }))
        }
        "stop" => {
            let conn = db_conn()?;
            let id = parse_rule_id(payload)?;
            let persistence = DatabaseAutoStartPersistence { conn: &conn };
            let status = runtime::global_manager()
                .stop_loaded(id, &persistence, || repository::get_with_conn(&conn, id))
                .map_err(|error| runtime_action_error(id, error))?;
            Ok(json!({ "item": status }))
        }
        "start_all" => {
            let conn = db_conn()?;
            let rule_ids = repository::list_with_conn(&conn)?
                .into_iter()
                .map(|rule| rule.id)
                .collect::<Vec<_>>();
            let persistence = DatabaseAutoStartPersistence { conn: &conn };
            let results =
                runtime::global_manager().start_all_loaded(&rule_ids, &persistence, |id| {
                    repository::get_with_conn(&conn, id)
                });
            Ok(json!({ "results": results }))
        }
        "stop_all" => {
            let conn = db_conn()?;
            let rule_ids = repository::list_with_conn(&conn)?
                .into_iter()
                .map(|rule| rule.id)
                .collect::<Vec<_>>();
            let persistence = DatabaseAutoStartPersistence { conn: &conn };
            let results =
                runtime::global_manager().stop_all_loaded(&rule_ids, &persistence, |id| {
                    repository::get_with_conn(&conn, id)
                });
            Ok(json!({ "results": results }))
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
            Ok(json!({ "items": page.items, "total": page.total }))
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

struct DatabaseAutoStartPersistence<'a> {
    conn: &'a rusqlite::Connection,
}

impl AutoStartPersistence for DatabaseAutoStartPersistence<'_> {
    fn set_auto_start(&self, rule_id: i64, value: bool) -> Result<(), String> {
        repository::set_auto_start_with_conn(self.conn, rule_id, value)
    }
}

fn runtime_action_error(rule_id: i64, error: String) -> String {
    let status = runtime::global_manager().status(rule_id);
    format!("{error}; 当前运行状态: {}", status.state.as_str())
}

fn parse_rule_id(payload: &Value) -> Result<i64, String> {
    payload
        .get("id")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| "转发规则 ID 无效".into())
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
        offset,
        limit,
    })
}

#[cfg(test)]
mod tests {
    use super::model::{ForwardProtocol, RuleWriteInput};
    use super::runtime::{RuntimeState, RuntimeStatus};
    use super::{repository, validation};
    use crate::tools::helpers::ensure_request_forward_schema_for_test;
    use rusqlite::{params, Connection};
    use serde_json::json;

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

        let mut update = tcp_input();
        update.name = "已更新规则".into();
        let updated = repository::update_with_conn(&conn, created.id, update).expect("update rule");
        assert_eq!(updated.name, "已更新规则");
        assert_eq!(updated.protocol, ForwardProtocol::Tcp);
        assert_eq!(updated.target_host.as_deref(), Some("192.168.1.10"));

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
    }

    #[test]
    fn action_rejects_unknown_action() {
        let err = super::execute("unknown", &json!({})).expect_err("unknown action should fail");
        assert!(err.contains("unsupported request_forward action"));
    }

    #[test]
    fn log_and_status_actions_serialize_camel_case_output() {
        let query = super::parse_log_query(&json!({
            "id": 7,
            "keyword": "timeout",
            "mode": "error",
            "offset": 20,
            "limit": 50
        }))
        .expect("parse log query");
        assert_eq!(query.rule_id, 7);
        assert_eq!(query.keyword.as_deref(), Some("timeout"));
        assert_eq!(query.outcome, Some(repository::LogOutcome::Error));
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
    }
}

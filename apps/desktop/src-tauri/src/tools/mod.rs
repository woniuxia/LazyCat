pub mod capture;
pub mod convert;
pub mod cron;
pub mod crypto;
pub mod data_dictionary;
pub mod dns;
pub mod encode;
pub mod env;
pub mod file;
pub mod format;
pub mod gen;
pub mod helpers;
pub mod hosts;
pub mod hotkey;
pub mod image;
pub mod inbox;
pub mod jwt;
pub mod launcher;
pub mod manuals;
pub mod maven;
pub mod mybatis;
pub mod network;
pub mod nginx;
pub mod pdf;
pub mod pm;
pub mod pm_calendar;
pub mod pm_matrix;
pub mod pm_siyuan;
pub mod pm_today;
pub mod pm_weekly;
pub mod pm_todo_link;
pub mod port;
pub mod regex;
pub mod schema;
pub mod settings;
pub mod snippets;
pub mod text;
pub mod time;
pub mod todo;
pub mod vault;
pub mod attachments;
pub mod system;
pub mod widget;

use serde_json::Value;

pub fn execute_tool(domain: &str, action: &str, payload: &Value) -> Result<Value, String> {
    let result = dispatch_tool(domain, action, payload);
    // 数据变更类 action 成功后通知挂件：5s 静默后立刷一次
    if result.is_ok() && pm_or_todo_data_changed(domain, action) {
        widget::pulse::notify_data_changed(domain_static(domain));
    }
    result
}

fn dispatch_tool(domain: &str, action: &str, payload: &Value) -> Result<Value, String> {
    match domain {
        "encode" => encode::execute(action, payload),
        "convert" => convert::execute(action, payload),
        "text" => text::execute(action, payload),
        "time" => time::execute(action, payload),
        "gen" => gen::execute(action, payload),
        "regex" => regex::execute(action, payload),
        "cron" => cron::execute(action, payload),
        "crypto" => crypto::execute(action, payload),
        "data_dictionary" => data_dictionary::execute(action, payload),
        "format" => format::execute(action, payload),
        "network" => network::execute(action, payload),
        "dns" => dns::execute(action, payload),
        "env" => env::execute(action, payload),
        "port" => port::execute(action, payload),
        "file" => file::execute(action, payload),
        "image" => image::execute(action, payload),
        "hosts" => hosts::execute(action, payload),
        "manuals" => manuals::execute(action, payload),
        "settings" => settings::execute(action, payload),
        "hotkey" => hotkey::execute(action, payload),
        "jwt" => jwt::execute(action, payload),
        "schema" => schema::execute(action, payload),
        "mybatis" => mybatis::execute(action, payload),
        "nginx" => nginx::execute(action, payload),
        "snippets" => snippets::execute(action, payload),
        "pdf" => pdf::execute(action, payload),
        "vault" => vault::execute(action, payload),
        "launcher" => launcher::execute(action, payload),
        "todo" => todo::execute(action, payload),
        "pm" => pm::execute(action, payload),
        "maven" => maven::execute(action, payload),
        "inbox" => inbox::execute(action, payload),
        "attachments" => attachments::execute(action, payload),
        "system" => system::execute(action, payload),
        "widget" => widget::execute(action, payload),
        _ => Err(format!("unsupported command: {domain}.{action}")),
    }
}

/// 判定是否要通知挂件刷新；只对真正改写 PM / Todo 数据的 action 触发。
///
/// 故意排除纯查询（item_list / item_counts / siyuan_*）与跨域副作用（todo_link
/// 由 PM 域统一覆盖），避免无意义的合成请求。
fn pm_or_todo_data_changed(domain: &str, action: &str) -> bool {
    match domain {
        "pm" => matches!(
            action,
            "item_create"
                | "item_update"
                | "item_change_status"
                | "item_reorder"
                | "item_toggle_pin"
                | "item_batch_update"
                | "item_delete"
                | "item_move_project"
                | "item_todo_create"
                | "item_todo_link"
                | "item_todo_unlink"
                | "project_create"
                | "project_update"
                | "project_archive"
                | "project_restore"
                | "project_delete"
        ),
        "todo" => matches!(
            action,
            "item_create"
                | "item_update"
                | "item_change_status"
                | "item_reorder"
                | "item_toggle_pin"
                | "item_batch_update"
                | "item_delete"
                | "item_complete"
                | "item_undo_complete"
        ),
        _ => false,
    }
}

/// 把 domain &str 升级为 'static str（仅 pm / todo 两个值，匹配 events::notify 签名）。
fn domain_static(domain: &str) -> &'static str {
    match domain {
        "pm" => "pm",
        "todo" => "todo",
        _ => "other",
    }
}

pub fn execute_tool_with_app(
    domain: &str,
    action: &str,
    payload: &Value,
    app: &tauri::AppHandle,
) -> Result<Value, String> {
    match domain {
        "settings" => settings::execute_with_app(action, payload, app),
        "widget" => widget::execute_with_app(action, payload, app),
        _ => execute_tool(domain, action, payload),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unsupported_domain_should_fail() {
        let err = execute_tool("nope", "x", &json!({})).expect_err("must fail");
        assert!(err.contains("unsupported command"));
    }

    #[test]
    fn known_domain_should_dispatch() {
        let out = execute_tool("gen", "uuid", &json!({})).expect("dispatch");
        assert!(out.as_str().map(|s| s.len() == 36).unwrap_or(false));
    }
}

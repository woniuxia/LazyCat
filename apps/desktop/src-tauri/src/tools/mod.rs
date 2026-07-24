pub mod api_mock;
pub mod browser_profiles;
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
pub mod pomodoro;
pub mod port;
pub mod regex;
pub mod release_package;
pub mod release_package_archive;
pub mod release_package_deploy;
pub mod release_package_remote;
pub mod release_package_runtime;
pub mod request_forward;
pub mod schema;
pub mod sql_entity;
pub mod settings;
pub mod snippets;
pub mod text;
pub mod time;
pub mod todo;
pub mod vault;
mod vault_lock;
pub mod attachments;
pub mod access_path_diagnostics;
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
        "api_mock" => api_mock::execute(action, payload),
        "browser_profiles" => browser_profiles::execute(action, payload),
        "encode" => encode::execute(action, payload),
        "convert" => convert::execute(action, payload),
        "text" => text::execute(action, payload),
        "time" => time::execute(action, payload),
        "gen" => gen::execute(action, payload),
        "regex" => regex::execute(action, payload),
        "release_package" => release_package::execute(action, payload),
        "request_forward" => request_forward::execute(action, payload),
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
        "sql_entity" => sql_entity::execute(action, payload),
        "mybatis" => mybatis::execute(action, payload),
        "nginx" => nginx::execute(action, payload),
        "snippets" => snippets::execute(action, payload),
        "pdf" => pdf::execute(action, payload),
        "vault" => vault::execute(action, payload),
        "launcher" => launcher::execute(action, payload),
        "todo" => todo::execute(action, payload),
        "pm" => pm::execute(action, payload),
        "pomodoro" => pomodoro::execute(action, payload),
        "maven" => maven::execute(action, payload),
        "inbox" => inbox::execute(action, payload),
        "attachments" => attachments::execute(action, payload),
        "system" => system::execute(action, payload),
        "widget" => widget::execute(action, payload),
        _ => Err(format!("unsupported command: {domain}.{action}")),
    }
}

#[cfg(test)]
pub fn supported_actions(domain: &str) -> Option<&'static [&'static str]> {
    match domain {
        "api_mock" => Some(api_mock::supported_actions()),
        "browser_profiles" => Some(browser_profiles::supported_actions()),
        "encode" => Some(encode::supported_actions()),
        "convert" => Some(convert::supported_actions()),
        "text" => Some(text::supported_actions()),
        "time" => Some(time::supported_actions()),
        "gen" => Some(gen::supported_actions()),
        "regex" => Some(regex::supported_actions()),
        "release_package" => Some(release_package::supported_actions()),
        "request_forward" => Some(request_forward::supported_actions()),
        "cron" => Some(cron::supported_actions()),
        "crypto" => Some(crypto::supported_actions()),
        "data_dictionary" => Some(data_dictionary::supported_actions()),
        "format" => Some(format::supported_actions()),
        "network" => Some(network::supported_actions()),
        "dns" => Some(dns::supported_actions()),
        "env" => Some(env::supported_actions()),
        "port" => Some(port::supported_actions()),
        "file" => Some(file::supported_actions()),
        "image" => Some(image::supported_actions()),
        "hosts" => Some(hosts::supported_actions()),
        "manuals" => Some(manuals::supported_actions()),
        "settings" => Some(settings::supported_actions()),
        "hotkey" => Some(hotkey::supported_actions()),
        "jwt" => Some(jwt::supported_actions()),
        "schema" => Some(schema::supported_actions()),
        "sql_entity" => Some(sql_entity::supported_actions()),
        "mybatis" => Some(mybatis::supported_actions()),
        "nginx" => Some(nginx::supported_actions()),
        "snippets" => Some(snippets::supported_actions()),
        "pdf" => Some(pdf::supported_actions()),
        "vault" => Some(vault::supported_actions()),
        "launcher" => Some(launcher::supported_actions()),
        "todo" => Some(todo::supported_actions()),
        "pm" => Some(pm::supported_actions()),
        "pomodoro" => Some(pomodoro::supported_actions()),
        "maven" => Some(maven::supported_actions()),
        "inbox" => Some(inbox::supported_actions()),
        "attachments" => Some(attachments::supported_actions()),
        "system" => Some(system::supported_actions()),
        "widget" => Some(widget::supported_actions()),
        _ => None,
    }
}

/// 判定是否要通知挂件刷新；只对真正改写 PM / Todo 数据的 action 触发。
///
/// 故意排除纯查询（item_list / item_counts / siyuan_*）与跨域副作用（todo_link
/// 由 PM 域统一覆盖），避免无意义的合成请求。
const PM_WIDGET_REFRESH_ACTIONS: &[&str] = &[
    "item_create",
    "item_update",
    "item_change_status",
    "item_reorder",
    "item_toggle_pin",
    "item_batch_update",
    "item_delete",
    "item_move_project",
    "item_todo_create",
    "item_todo_link",
    "item_todo_unlink",
    "project_create",
    "project_update",
    "project_archive",
    "project_restore",
    "project_delete",
];

const TODO_WIDGET_REFRESH_ACTIONS: &[&str] = &[
    "item_create",
    "item_update",
    "item_change_status",
    "item_toggle_pin",
    "item_delete",
];

fn pm_or_todo_data_changed(domain: &str, action: &str) -> bool {
    match domain {
        "pm" => PM_WIDGET_REFRESH_ACTIONS.contains(&action),
        "todo" => TODO_WIDGET_REFRESH_ACTIONS.contains(&action),
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
        "release_package" => release_package::execute_with_app(action, payload, app),
        "settings" => settings::execute_with_app(action, payload, app),
        "widget" => widget::execute_with_app(action, payload, app),
        _ => execute_tool(domain, action, payload),
    }
}

#[cfg(test)]
mod contract_tests;

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

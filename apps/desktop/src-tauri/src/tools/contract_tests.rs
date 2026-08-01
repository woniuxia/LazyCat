use super::{supported_actions, PM_WIDGET_REFRESH_ACTIONS, TODO_WIDGET_REFRESH_ACTIONS};
use crate::events;
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;

const SYNC_GUIDE: &str =
    "新增 action 需同步：模块 supported_actions、bridge/tauri.ts CHANNEL_MAP、（写操作）mod.rs 挂件白名单";

const DOMAINS: &[&str] = &[
    "action_center",
    "api_mock",
    "browser_profiles",
    "encode",
    "convert",
    "text",
    "time",
    "gen",
    "regex",
    "release_package",
    "cron",
    "crypto",
    "data_dictionary",
    "format",
    "network",
    "request_forward",
    "dns",
    "env",
    "port",
    "file",
    "file_lock",
    "image",
    "hosts",
    "manuals",
    "settings",
    "hotkey",
    "jwt",
    "schema",
    "mybatis",
    "nginx",
    "snippets",
    "pdf",
    "vault",
    "launcher",
    "todo",
    "usage",
    "pm",
    "pomodoro",
    "maven",
    "inbox",
    "attachments",
    "system",
    "widget",
];

const EXEMPT: &[(&str, &str)] = &[
    ("snippets", "list"), // snippets legacy alias for v2_list; no CHANNEL_MAP entry.
    ("snippets", "get"),  // snippets legacy alias for v2_get; no CHANNEL_MAP entry.
    ("snippets", "create"), // snippets legacy alias for v2_create; no CHANNEL_MAP entry.
    ("snippets", "update"), // snippets legacy alias for v2_update; no CHANNEL_MAP entry.
    ("snippets", "delete"), // snippets legacy alias for v2_delete; no CHANNEL_MAP entry.
    ("snippets", "search"), // snippets legacy alias for v2_search; no CHANNEL_MAP entry.
    ("snippets", "tags"), // snippets legacy alias for v2_tag_stats; no CHANNEL_MAP entry.
    ("snippets", "folder_list"), // snippets legacy alias for v2_folder_list; no CHANNEL_MAP entry.
    ("snippets", "folder_create"), // snippets legacy alias for v2_folder_create; no CHANNEL_MAP entry.
    ("snippets", "folder_update"), // snippets legacy alias for v2_folder_update; no CHANNEL_MAP entry.
    ("snippets", "folder_delete"), // snippets legacy alias for v2_folder_delete; no CHANNEL_MAP entry.
    ("snippets", "toggle_favorite"), // snippets backend maintenance action; no CHANNEL_MAP entry.
    ("snippets", "language_stats"), // snippets backend stats action; no CHANNEL_MAP entry.
    ("snippets", "batch_update"),  // snippets backend batch action; no CHANNEL_MAP entry.
    ("snippets", "batch_delete"),  // snippets backend batch action; no CHANNEL_MAP entry.
];

fn parse_channel_map() -> Vec<(String, String)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/bridge/tauri.ts");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read CHANNEL_MAP from {:?} failed: {err}", path));
    parse_channel_map_source(&source)
}

fn parse_channel_map_source(source: &str) -> Vec<(String, String)> {
    let entry_re = Regex::new(
        r#""tool:[^"]+"\s*:\s*\{\s*domain:\s*"([a-z_]+)"\s*,\s*action:\s*"([a-z0-9_]+)"\s*,?\s*\}\s*,?"#,
    )
    .expect("valid CHANNEL_MAP regex");

    entry_re
        .captures_iter(source)
        .map(|caps| (caps[1].to_string(), caps[2].to_string()))
        .collect()
}

fn parse_frontend_events() -> HashSet<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/bridge/events.ts");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read APP_EVENTS from {:?} failed: {err}", path));
    let entry_re =
        Regex::new(r#"^\s*[A-Z0-9_]+:\s*"([^"]+)",\s*$"#).expect("valid APP_EVENTS regex");

    source
        .lines()
        .filter_map(|line| entry_re.captures(line).map(|caps| caps[1].to_string()))
        .collect()
}

#[test]
fn channel_map_actions_are_supported_by_backend() {
    let channel_actions = parse_channel_map();
    assert!(
        channel_actions.len() >= 300,
        "CHANNEL_MAP 解析条目数过少：{}。{SYNC_GUIDE}",
        channel_actions.len()
    );

    for (domain, action) in &channel_actions {
        let actions = supported_actions(domain).unwrap_or_else(|| {
            panic!("CHANNEL_MAP domain 未注册：{domain}.{action}。{SYNC_GUIDE}")
        });
        assert!(
            actions.contains(&action.as_str()),
            "CHANNEL_MAP action 未被后端支持：{domain}.{action}。{SYNC_GUIDE}"
        );
    }
}

#[test]
fn backend_supported_actions_are_exposed_or_explicitly_exempted() {
    let channel_actions: HashSet<(String, String)> = parse_channel_map().into_iter().collect();
    assert!(
        channel_actions.len() >= 300,
        "CHANNEL_MAP 解析条目数过少：{}。{SYNC_GUIDE}",
        channel_actions.len()
    );

    for domain in DOMAINS {
        let actions = supported_actions(domain)
            .unwrap_or_else(|| panic!("supported_actions domain 未注册：{domain}。{SYNC_GUIDE}"));
        for action in actions {
            let in_channel = channel_actions.contains(&(domain.to_string(), action.to_string()));
            let exempted = EXEMPT.contains(&(*domain, *action));
            assert!(
                in_channel || exempted,
                "后端 action 未暴露到 CHANNEL_MAP 且未豁免：{domain}.{action}。{SYNC_GUIDE}"
            );
        }
    }
}

#[test]
fn rust_events_are_declared_on_frontend() {
    let frontend_events = parse_frontend_events();
    assert!(
        frontend_events.len() >= 10,
        "APP_EVENTS 解析条目数过少：{}。事件名需同步 src/bridge/events.ts 与 src-tauri/src/events.rs",
        frontend_events.len()
    );

    for event in events::ALL {
        assert!(
            frontend_events.contains(*event),
            "Rust 事件未在前端 APP_EVENTS 声明：{event}。事件名需同步 src/bridge/events.ts 与 src-tauri/src/events.rs"
        );
    }
}

#[test]
fn widget_refresh_whitelists_are_supported_actions() {
    let pm_actions = supported_actions("pm").expect("pm supported actions");
    for action in PM_WIDGET_REFRESH_ACTIONS {
        assert!(
            pm_actions.contains(action),
            "PM 挂件刷新白名单 action 未被 pm 支持：{action}。{SYNC_GUIDE}"
        );
    }

    let todo_actions = supported_actions("todo").expect("todo supported actions");
    for action in TODO_WIDGET_REFRESH_ACTIONS {
        assert!(
            todo_actions.contains(action),
            "Todo 挂件刷新白名单 action 未被 todo 支持：{action}。{SYNC_GUIDE}"
        );
    }
}

#[test]
fn channel_map_parser_accepts_multiline_entries() {
    let source = r#"
        const CHANNEL_MAP = {
          "tool:action-center:combination-definition-list": {
            domain: "action_center",
            action: "combination_definition_list",
          },
        };
    "#;

    assert_eq!(
        parse_channel_map_source(source),
        vec![(
            "action_center".to_string(),
            "combination_definition_list".to_string(),
        )]
    );
}

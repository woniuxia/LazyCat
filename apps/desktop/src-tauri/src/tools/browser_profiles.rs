use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use rusqlite::params;
use serde_json::json;
use serde_json::{Map, Value};

const CONFIG_KEY: &str = "browser_profiles_config_v1";
const BROWSER_EDGE: &str = "edge";

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserProfilesConfig {
    #[serde(default)]
    edge_path: Option<String>,
    #[serde(default)]
    edge: BTreeMap<String, BrowserProfileConfigEntry>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserProfileConfigEntry {
    #[serde(default)]
    alias: Option<String>,
    #[serde(default)]
    hidden: Option<bool>,
    #[serde(default, rename = "launchCount")]
    launch_count: Option<i64>,
    #[serde(default, rename = "lastLaunchedAt")]
    last_launched_at: Option<String>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscoveredProfile {
    profile_dir: String,
    edge_display_name: String,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct BrowserProfileItem {
    browser: String,
    profile_dir: String,
    edge_display_name: String,
    alias: String,
    hidden: bool,
    launch_count: i64,
    last_launched_at: Option<String>,
}

const ACTIONS: &[&str] = &[
    "list",
    "save_alias",
    "set_hidden",
    "set_edge_path",
    "launch",
];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported browser_profiles action: {action}"));
    }
    match action {
        "list" => list_profiles(),
        "save_alias" => save_alias(payload),
        "set_hidden" => set_hidden(payload),
        "set_edge_path" => set_edge_path(payload),
        "launch" => launch_profile(payload),
        _ => Err(format!("unsupported browser_profiles action: {action}")),
    }
}

fn parse_local_state_profile_names(
    content: &str,
) -> Result<(BTreeMap<String, String>, Vec<String>), String> {
    let value = match serde_json::from_str::<Value>(content) {
        Ok(value) => value,
        Err(err) => {
            return Ok((
                BTreeMap::new(),
                vec![format!("Local State 解析失败: {err}")],
            ));
        }
    };

    let mut names = BTreeMap::new();
    let Some(info_cache) = value
        .get("profile")
        .and_then(|profile| profile.get("info_cache"))
        .and_then(Value::as_object)
    else {
        return Ok((names, Vec::new()));
    };

    for (profile_dir, profile_info) in info_cache {
        let Some(name) = profile_info.get("name").and_then(Value::as_str) else {
            continue;
        };
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            names.insert(profile_dir.clone(), trimmed.to_string());
        }
    }

    Ok((names, Vec::new()))
}

fn is_edge_profile_dir_name(name: &str) -> bool {
    if name == "Default" {
        return true;
    }

    let Some(number) = name.strip_prefix("Profile ") else {
        return false;
    };

    !number.is_empty() && number.chars().all(|ch| ch.is_ascii_digit())
}

fn collect_profile_dirs_from_names(names: &[&str]) -> Vec<String> {
    let mut dirs = names
        .iter()
        .copied()
        .filter(|name| is_edge_profile_dir_name(name))
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    dirs.sort_by(compare_profile_dir_name);
    dirs
}

#[allow(dead_code)]
fn parse_config_json(raw: Option<&str>, warnings: &mut Vec<String>) -> BrowserProfilesConfig {
    let Some(raw) = raw else {
        return BrowserProfilesConfig::default();
    };

    match serde_json::from_str::<BrowserProfilesConfig>(raw) {
        Ok(config) => config,
        Err(err) => {
            warnings.push(format!("{CONFIG_KEY} 配置解析失败，已按空配置处理: {err}"));
            BrowserProfilesConfig::default()
        }
    }
}

fn merge_profiles(
    discovered: Vec<DiscoveredProfile>,
    config: &BrowserProfilesConfig,
) -> Vec<BrowserProfileItem> {
    discovered
        .into_iter()
        .map(|profile| {
            let entry = config.edge.get(&profile.profile_dir);
            BrowserProfileItem {
                browser: BROWSER_EDGE.to_string(),
                alias: entry
                    .and_then(|entry| entry.alias.clone())
                    .unwrap_or_default(),
                hidden: entry.and_then(|entry| entry.hidden).unwrap_or(false),
                launch_count: entry
                    .and_then(|entry| entry.launch_count)
                    .unwrap_or_default(),
                last_launched_at: entry.and_then(|entry| entry.last_launched_at.clone()),
                edge_display_name: profile.edge_display_name,
                profile_dir: profile.profile_dir,
            }
        })
        .collect()
}

fn sort_profiles(items: &mut [BrowserProfileItem]) {
    items.sort_by(|left, right| {
        left.hidden
            .cmp(&right.hidden)
            .then_with(|| right.launch_count.cmp(&left.launch_count))
            .then_with(|| compare_optional_desc(&left.last_launched_at, &right.last_launched_at))
            .then_with(|| display_name_for_sort(left).cmp(&display_name_for_sort(right)))
            .then_with(|| left.profile_dir.cmp(&right.profile_dir))
    });
}

fn build_edge_profile_arg(profile_dir: &str) -> String {
    format!("--profile-directory={profile_dir}")
}

fn validate_edge_exe_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err("Edge 可执行文件不存在".into());
    }
    if !path.is_file() {
        return Err("Edge 路径不是文件".into());
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if !file_name.eq_ignore_ascii_case("msedge.exe") {
        return Err("必须选择 msedge.exe".into());
    }

    Ok(())
}

fn candidate_edge_paths(config_edge_path: Option<&str>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = config_edge_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        push_unique_path(&mut paths, PathBuf::from(path));
    }

    if let Ok(program_files_x86) = std::env::var("ProgramFiles(x86)") {
        push_unique_path(
            &mut paths,
            PathBuf::from(program_files_x86)
                .join("Microsoft")
                .join("Edge")
                .join("Application")
                .join("msedge.exe"),
        );
    }
    if let Ok(program_files) = std::env::var("ProgramFiles") {
        push_unique_path(
            &mut paths,
            PathBuf::from(program_files)
                .join("Microsoft")
                .join("Edge")
                .join("Application")
                .join("msedge.exe"),
        );
    }
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        push_unique_path(
            &mut paths,
            PathBuf::from(local_app_data)
                .join("Microsoft")
                .join("Edge")
                .join("Application")
                .join("msedge.exe"),
        );
    }

    paths
}

fn find_edge_path(config_edge_path: Option<&str>) -> (Option<PathBuf>, Vec<PathBuf>) {
    let paths = candidate_edge_paths(config_edge_path);
    let found = paths
        .iter()
        .find(|path| validate_edge_exe_path(path).is_ok())
        .cloned();
    (found, paths)
}

fn edge_user_data_dir() -> PathBuf {
    std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Microsoft")
        .join("Edge")
        .join("User Data")
}

fn scan_edge_profiles(user_data_dir: &Path) -> (Vec<DiscoveredProfile>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut edge_names = BTreeMap::new();
    let local_state_path = user_data_dir.join("Local State");

    if user_data_dir.is_dir() {
        match fs::read_to_string(&local_state_path) {
            Ok(content) => match parse_local_state_profile_names(&content) {
                Ok((names, local_warnings)) => {
                    edge_names = names;
                    warnings.extend(local_warnings);
                }
                Err(err) => warnings.push(format!("Local State 解析失败: {err}")),
            },
            Err(err) if local_state_path.exists() => {
                warnings.push(format!("读取 Local State 失败: {err}"));
            }
            Err(_) => {
                warnings.push(format!(
                    "未找到 Edge Local State: {}",
                    local_state_path.to_string_lossy()
                ));
            }
        }
    } else {
        warnings.push(format!(
            "未找到 Edge User Data: {}",
            user_data_dir.to_string_lossy()
        ));
        return (Vec::new(), warnings);
    }

    let mut names = edge_names.keys().cloned().collect::<BTreeSet<_>>();
    match fs::read_dir(user_data_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if !file_type.is_dir() {
                    continue;
                }
                if let Some(name) = entry.file_name().to_str() {
                    names.insert(name.to_string());
                }
            }
        }
        Err(err) => warnings.push(format!("读取 Edge User Data 失败: {err}")),
    }

    let name_refs = names.iter().map(String::as_str).collect::<Vec<_>>();
    let profiles = collect_profile_dirs_from_names(&name_refs)
        .into_iter()
        .map(|profile_dir| DiscoveredProfile {
            edge_display_name: edge_names.get(&profile_dir).cloned().unwrap_or_default(),
            profile_dir,
        })
        .collect();

    (profiles, warnings)
}

fn load_config_from_settings(
    conn: &rusqlite::Connection,
    warnings: &mut Vec<String>,
) -> BrowserProfilesConfig {
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM user_settings WHERE key = ?1",
            params![CONFIG_KEY],
            |row| row.get(0),
        )
        .ok();
    parse_config_json(raw.as_deref(), warnings)
}

fn save_config_to_settings(
    conn: &rusqlite::Connection,
    config: &BrowserProfilesConfig,
) -> Result<(), String> {
    let value =
        serde_json::to_string(config).map_err(|err| format!("serialize config failed: {err}"))?;
    conn.execute(
        "INSERT INTO user_settings(key, value, updated_at) VALUES(?1, ?2, CURRENT_TIMESTAMP)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=CURRENT_TIMESTAMP",
        params![CONFIG_KEY, value],
    )
    .map_err(|err| format!("save browser profiles config failed: {err}"))?;
    Ok(())
}

fn mutate_config<F>(f: F) -> Result<BrowserProfilesConfig, String>
where
    F: FnOnce(&mut BrowserProfilesConfig) -> Result<(), String>,
{
    let conn = super::helpers::db_conn()?;
    let mut warnings = Vec::new();
    let mut config = load_config_from_settings(&conn, &mut warnings);
    f(&mut config)?;
    save_config_to_settings(&conn, &config)?;
    Ok(config)
}

fn list_profiles() -> Result<Value, String> {
    let mut warnings = Vec::new();
    let config = {
        let conn = super::helpers::db_conn()?;
        load_config_from_settings(&conn, &mut warnings)
    };
    let (edge_path, probed_paths) = find_edge_path(config.edge_path.as_deref());
    let user_data_dir = edge_user_data_dir();
    let (discovered, scan_warnings) = scan_edge_profiles(&user_data_dir);
    warnings.extend(scan_warnings);

    let mut profiles = merge_profiles(discovered, &config);
    sort_profiles(&mut profiles);

    Ok(json!({
        "edgeFound": edge_path.is_some(),
        "edgePath": edge_path.map(|path| path.to_string_lossy().to_string()),
        "userDataDir": user_data_dir.to_string_lossy().to_string(),
        "probedEdgePaths": probed_paths
            .into_iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>(),
        "warnings": warnings,
        "profiles": profiles,
    }))
}

fn save_alias(payload: &Value) -> Result<Value, String> {
    require_edge_browser(payload)?;
    let profile_dir = require_profile_dir(payload)?;
    ensure_profile_exists(&profile_dir)?;
    let alias = payload["alias"].as_str().unwrap_or_default();
    mutate_config(|config| {
        save_alias_in_config(config, &profile_dir, alias);
        Ok(())
    })?;
    Ok(json!({ "ok": true }))
}

fn set_hidden(payload: &Value) -> Result<Value, String> {
    require_edge_browser(payload)?;
    let profile_dir = require_profile_dir(payload)?;
    ensure_profile_exists(&profile_dir)?;
    let hidden = payload["hidden"]
        .as_bool()
        .ok_or_else(|| "hidden is required".to_string())?;
    mutate_config(|config| {
        let entry = config.edge.entry(profile_dir.clone()).or_default();
        entry.hidden = Some(hidden);
        Ok(())
    })?;
    Ok(json!({ "ok": true }))
}

fn set_edge_path(payload: &Value) -> Result<Value, String> {
    let edge_path = payload["edgePath"]
        .as_str()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| "edgePath is required".to_string())?;
    validate_edge_exe_path(Path::new(edge_path))?;
    mutate_config(|config| {
        config.edge_path = Some(edge_path.to_string());
        Ok(())
    })?;
    Ok(json!({ "ok": true }))
}

fn launch_profile(payload: &Value) -> Result<Value, String> {
    require_edge_browser(payload)?;
    let profile_dir = require_profile_dir(payload)?;
    let mut warnings = Vec::new();
    let config = {
        let conn = super::helpers::db_conn()?;
        load_config_from_settings(&conn, &mut warnings)
    };
    let (edge_path, _) = find_edge_path(config.edge_path.as_deref());
    let edge_path = edge_path.ok_or_else(|| "未找到 msedge.exe".to_string())?;
    ensure_profile_exists(&profile_dir)?;

    Command::new(&edge_path)
        .arg(build_edge_profile_arg(&profile_dir))
        .spawn()
        .map_err(|err| format!("launch failed: {err}"))?;

    let now = chrono::Local::now().to_rfc3339();
    let stats_result = mutate_config(|config| {
        update_launch_stats_in_config(config, &profile_dir, &now);
        Ok(())
    });
    let launch_count = match stats_result {
        Ok(config) => config
            .edge
            .get(&profile_dir)
            .and_then(|entry| entry.launch_count)
            .unwrap_or_default(),
        Err(err) => {
            warnings.push(format!("启动成功，但使用统计保存失败：{err}"));
            config
                .edge
                .get(&profile_dir)
                .and_then(|entry| entry.launch_count)
                .unwrap_or_default()
                + 1
        }
    };

    Ok(json!({
        "ok": true,
        "launchCount": launch_count,
        "lastLaunchedAt": now,
        "warnings": warnings,
    }))
}

fn require_edge_browser(payload: &Value) -> Result<(), String> {
    match payload["browser"].as_str() {
        Some(BROWSER_EDGE) => Ok(()),
        _ => Err("首版只支持 Edge".into()),
    }
}

fn require_profile_dir(payload: &Value) -> Result<String, String> {
    payload["profileDir"]
        .as_str()
        .map(str::trim)
        .filter(|profile_dir| !profile_dir.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "profileDir is required".to_string())
}

fn ensure_profile_exists(profile_dir: &str) -> Result<(), String> {
    let user_data_dir = edge_user_data_dir();
    let (profiles, _) = scan_edge_profiles(&user_data_dir);
    if profiles
        .iter()
        .any(|profile| profile.profile_dir == profile_dir)
    {
        Ok(())
    } else {
        Err(format!("Profile 已不存在: {profile_dir}"))
    }
}

fn save_alias_in_config(config: &mut BrowserProfilesConfig, profile_dir: &str, alias: &str) {
    let entry = config.edge.entry(profile_dir.to_string()).or_default();
    entry.alias = Some(alias.trim().to_string());
}

fn update_launch_stats_in_config(config: &mut BrowserProfilesConfig, profile_dir: &str, now: &str) {
    let entry = config.edge.entry(profile_dir.to_string()).or_default();
    entry.launch_count = Some(entry.launch_count.unwrap_or_default() + 1);
    entry.last_launched_at = Some(now.to_string());
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn compare_optional_desc(left: &Option<String>, right: &Option<String>) -> Ordering {
    match (left.as_deref(), right.as_deref()) {
        (Some(left), Some(right)) => right.cmp(left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn display_name_for_sort(item: &BrowserProfileItem) -> String {
    for candidate in [&item.alias, &item.edge_display_name, &item.profile_dir] {
        let trimmed = candidate.trim();
        if !trimmed.is_empty() {
            return trimmed.to_lowercase();
        }
    }
    String::new()
}

fn compare_profile_dir_name(left: &String, right: &String) -> Ordering {
    profile_dir_sort_key(left)
        .cmp(&profile_dir_sort_key(right))
        .then_with(|| left.cmp(right))
}

fn profile_dir_sort_key(name: &str) -> (u8, u64) {
    if name == "Default" {
        return (0, 0);
    }

    let number = name
        .strip_prefix("Profile ")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(u64::MAX);
    (1, number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item(
        profile_dir: &str,
        alias: &str,
        edge_display_name: &str,
        hidden: bool,
        launch_count: i64,
        last_launched_at: Option<&str>,
    ) -> BrowserProfileItem {
        BrowserProfileItem {
            browser: "edge".into(),
            profile_dir: profile_dir.into(),
            edge_display_name: edge_display_name.into(),
            alias: alias.into(),
            hidden,
            launch_count,
            last_launched_at: last_launched_at.map(str::to_string),
        }
    }

    #[test]
    fn parses_profile_names_from_local_state_info_cache() {
        let content = r#"{
          "profile": {
            "info_cache": {
              "Default": { "name": "个人" },
              "Profile 2": { "name": "测试账号" },
              "Guest Profile": { "name": "访客" }
            }
          }
        }"#;

        let (names, warnings) = parse_local_state_profile_names(content).expect("parse");

        assert!(warnings.is_empty());
        assert_eq!(names.get("Default").map(String::as_str), Some("个人"));
        assert_eq!(names.get("Profile 2").map(String::as_str), Some("测试账号"));
        assert_eq!(names.get("Guest Profile").map(String::as_str), Some("访客"));
    }

    #[test]
    fn invalid_local_state_returns_warning_and_empty_names() {
        let (names, warnings) = parse_local_state_profile_names("{not json").expect("soft parse");
        assert!(names.is_empty());
        assert!(warnings.iter().any(|w| w.contains("Local State")));
    }

    #[test]
    fn filters_default_and_profile_number_directories_only() {
        let names = [
            "Default",
            "Profile 1",
            "Profile 22",
            "Guest Profile",
            "System Profile",
            "Profile abc",
        ];
        assert_eq!(
            collect_profile_dirs_from_names(&names),
            vec!["Default", "Profile 1", "Profile 22"]
        );
    }

    #[test]
    fn merges_discovered_profiles_with_user_config_without_showing_deleted_profiles() {
        let discovered = vec![
            DiscoveredProfile {
                profile_dir: "Default".into(),
                edge_display_name: "个人".into(),
            },
            DiscoveredProfile {
                profile_dir: "Profile 2".into(),
                edge_display_name: "测试账号".into(),
            },
        ];
        let mut config = BrowserProfilesConfig::default();
        config.edge.insert(
            "Default".into(),
            BrowserProfileConfigEntry {
                alias: Some("管理员".into()),
                hidden: Some(false),
                launch_count: Some(12),
                last_launched_at: Some("2026-07-02T10:30:00+08:00".into()),
                extra: Default::default(),
            },
        );
        config.edge.insert(
            "Deleted".into(),
            BrowserProfileConfigEntry {
                alias: Some("旧账号".into()),
                hidden: Some(false),
                launch_count: Some(99),
                last_launched_at: None,
                extra: Default::default(),
            },
        );

        let merged = merge_profiles(discovered, &config);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].profile_dir, "Default");
        assert_eq!(merged[0].alias, "管理员");
        assert_eq!(merged[0].launch_count, 12);
        assert!(!merged.iter().any(|p| p.profile_dir == "Deleted"));
    }

    #[test]
    fn sorts_by_hidden_launch_count_last_launched_display_name_and_dir() {
        let mut items = vec![
            item(
                "Profile 3",
                "",
                "Beta",
                false,
                2,
                Some("2026-07-02T09:00:00+08:00"),
            ),
            item(
                "Profile 2",
                "管理员",
                "Zeta",
                false,
                3,
                Some("2026-07-01T09:00:00+08:00"),
            ),
            item(
                "Default",
                "",
                "Alpha",
                false,
                3,
                Some("2026-07-02T09:00:00+08:00"),
            ),
            item(
                "Profile 4",
                "",
                "Hidden",
                true,
                99,
                Some("2026-07-03T09:00:00+08:00"),
            ),
        ];

        sort_profiles(&mut items);

        assert_eq!(
            items
                .iter()
                .map(|p| p.profile_dir.as_str())
                .collect::<Vec<_>>(),
            vec!["Default", "Profile 2", "Profile 3", "Profile 4"]
        );
    }

    #[test]
    fn builds_profile_directory_as_single_command_argument() {
        assert_eq!(
            build_edge_profile_arg("Profile 2"),
            "--profile-directory=Profile 2"
        );
    }

    #[test]
    fn rejects_non_edge_browser_payload() {
        let err =
            require_edge_browser(&json!({ "browser": "chrome" })).expect_err("chrome unsupported");
        assert!(err.contains("只支持 Edge") || err.contains("edge"));
    }

    #[test]
    fn trims_alias_before_writing_config_entry() {
        let mut config = BrowserProfilesConfig::default();
        save_alias_in_config(&mut config, "Profile 2", "  普通用户  ");
        assert_eq!(
            config
                .edge
                .get("Profile 2")
                .and_then(|e| e.alias.as_deref()),
            Some("普通用户")
        );
    }

    #[test]
    fn empty_alias_clears_alias_but_preserves_entry_stats() {
        let mut config = BrowserProfilesConfig::default();
        config.edge.insert(
            "Profile 2".into(),
            BrowserProfileConfigEntry {
                alias: Some("旧名".into()),
                launch_count: Some(7),
                ..Default::default()
            },
        );

        save_alias_in_config(&mut config, "Profile 2", " ");

        let entry = config.edge.get("Profile 2").expect("entry");
        assert_eq!(entry.alias.as_deref(), Some(""));
        assert_eq!(entry.launch_count, Some(7));
    }

    #[test]
    fn launch_stats_increment_preserves_alias_and_hidden() {
        let mut config = BrowserProfilesConfig::default();
        config.edge.insert(
            "Profile 2".into(),
            BrowserProfileConfigEntry {
                alias: Some("普通用户".into()),
                hidden: Some(true),
                launch_count: Some(8),
                last_launched_at: Some("2026-07-01T09:00:00+08:00".into()),
                extra: Default::default(),
            },
        );

        update_launch_stats_in_config(&mut config, "Profile 2", "2026-07-02T10:30:00+08:00");

        let entry = config.edge.get("Profile 2").expect("entry");
        assert_eq!(entry.alias.as_deref(), Some("普通用户"));
        assert_eq!(entry.hidden, Some(true));
        assert_eq!(entry.launch_count, Some(9));
        assert_eq!(
            entry.last_launched_at.as_deref(),
            Some("2026-07-02T10:30:00+08:00")
        );
    }
}

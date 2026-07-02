use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{Map, Value};

#[allow(dead_code)]
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

#[allow(dead_code)]
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
        assert_eq!(
            names.get("Profile 2").map(String::as_str),
            Some("测试账号")
        );
        assert_eq!(
            names.get("Guest Profile").map(String::as_str),
            Some("访客")
        );
    }

    #[test]
    fn invalid_local_state_returns_warning_and_empty_names() {
        let (names, warnings) =
            parse_local_state_profile_names("{not json").expect("soft parse");
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
}

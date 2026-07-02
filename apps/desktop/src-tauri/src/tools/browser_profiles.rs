#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscoveredProfile {
    profile_dir: String,
    edge_display_name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct BrowserProfilesConfig {
    edge: std::collections::BTreeMap<String, BrowserProfileConfigEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct BrowserProfileConfigEntry {
    alias: Option<String>,
    hidden: Option<bool>,
    launch_count: Option<i64>,
    last_launched_at: Option<String>,
    extra: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BrowserProfileItem {
    browser: String,
    profile_dir: String,
    edge_display_name: String,
    alias: String,
    hidden: bool,
    launch_count: i64,
    last_launched_at: Option<String>,
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

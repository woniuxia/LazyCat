use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use rusqlite::params;
use serde_json::json;
use serde_json::{Map, Value};

use super::usage::{self, UsageKey, ACTION_LAUNCH, RESOURCE_BROWSER_PROFILE};

const CONFIG_KEY: &str = "browser_profiles_config_v1";
const BROWSER_EDGE: &str = "edge";
const BROWSER_CHROME: &str = "chrome";

static CONFIG_MUTATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserProfilesConfig {
    #[serde(default)]
    edge_path: Option<String>,
    #[serde(default)]
    chrome_path: Option<String>,
    #[serde(default)]
    edge: BTreeMap<String, BrowserProfileConfigEntry>,
    #[serde(default)]
    chrome: BTreeMap<String, BrowserProfileConfigEntry>,
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

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
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
    "set_chrome_path",
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
        "set_chrome_path" => set_chrome_path(payload),
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
    merge_profiles_for_browser(BROWSER_EDGE, discovered, &config.edge)
}

fn merge_profiles_for_browser(
    browser: &str,
    discovered: Vec<DiscoveredProfile>,
    entries: &BTreeMap<String, BrowserProfileConfigEntry>,
) -> Vec<BrowserProfileItem> {
    discovered
        .into_iter()
        .map(|profile| {
            let entry = entries.get(&profile.profile_dir);
            BrowserProfileItem {
                browser: browser.to_string(),
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
    validate_browser_exe_path(path, "msedge.exe", "Edge")
}

fn validate_chrome_exe_path(path: &Path) -> Result<(), String> {
    validate_browser_exe_path(path, "chrome.exe", "Chrome")
}

fn validate_browser_exe_path(path: &Path, executable: &str, label: &str) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("{label} 可执行文件不存在"));
    }
    if !path.is_file() {
        return Err(format!("{label} 路径不是文件"));
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if !file_name.eq_ignore_ascii_case(executable) {
        return Err(format!("必须选择 {executable}"));
    }

    Ok(())
}

fn candidate_chrome_paths(config_chrome_path: Option<&str>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = config_chrome_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        push_unique_path(&mut paths, PathBuf::from(path));
    }
    for env_name in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Ok(base) = std::env::var(env_name) {
            push_unique_path(
                &mut paths,
                PathBuf::from(base)
                    .join("Google")
                    .join("Chrome")
                    .join("Application")
                    .join("chrome.exe"),
            );
        }
    }
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        push_unique_path(
            &mut paths,
            PathBuf::from(local_app_data)
                .join("Google")
                .join("Chrome")
                .join("Application")
                .join("chrome.exe"),
        );
    }
    paths
}

fn find_chrome_path(config_chrome_path: Option<&str>) -> (Option<PathBuf>, Vec<PathBuf>) {
    let paths = candidate_chrome_paths(config_chrome_path);
    let found = paths
        .iter()
        .find(|path| validate_chrome_exe_path(path).is_ok())
        .cloned();
    (found, paths)
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

fn chrome_user_data_dir() -> PathBuf {
    std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("Google")
        .join("Chrome")
        .join("User Data")
}

fn scan_edge_profiles(user_data_dir: &Path) -> (Vec<DiscoveredProfile>, Vec<String>) {
    scan_chromium_profiles(user_data_dir, "Edge")
}

fn scan_chrome_profiles(user_data_dir: &Path) -> (Vec<DiscoveredProfile>, Vec<String>) {
    scan_chromium_profiles(user_data_dir, "Chrome")
}

fn scan_chromium_profiles(
    user_data_dir: &Path,
    browser_label: &str,
) -> (Vec<DiscoveredProfile>, Vec<String>) {
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
                    "未找到 {browser_label} Local State: {}",
                    local_state_path.to_string_lossy()
                ));
            }
        }
    } else {
        warnings.push(format!(
            "未找到 {browser_label} User Data: {}",
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
        Err(err) => warnings.push(format!("读取 {browser_label} User Data 失败: {err}")),
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

fn with_config_mutation_lock<T>(mutation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let lock = CONFIG_MUTATION_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock
        .lock()
        .map_err(|_| "浏览器身份配置写入锁已损坏".to_string())?;
    mutation()
}

fn mutate_config<F>(f: F) -> Result<BrowserProfilesConfig, String>
where
    F: FnOnce(&mut BrowserProfilesConfig) -> Result<(), String>,
{
    with_config_mutation_lock(|| {
        let conn = super::helpers::db_conn()?;
        let mut warnings = Vec::new();
        let mut config = load_config_from_settings(&conn, &mut warnings);
        f(&mut config)?;
        save_config_to_settings(&conn, &config)?;
        Ok(config)
    })
}

fn list_profiles_with_conn(conn: &rusqlite::Connection) -> Result<Value, String> {
    let mut warnings = Vec::new();
    let config = load_config_from_settings(conn, &mut warnings);
    let (edge_path, probed_paths) = find_edge_path(config.edge_path.as_deref());
    let (chrome_path, probed_chrome_paths) = find_chrome_path(config.chrome_path.as_deref());
    let edge_user_data_dir = edge_user_data_dir();
    let chrome_user_data_dir = chrome_user_data_dir();
    let (edge_discovered, edge_scan_warnings) = if edge_user_data_dir.is_dir() {
        scan_edge_profiles(&edge_user_data_dir)
    } else {
        (Vec::new(), Vec::new())
    };
    let (chrome_discovered, chrome_scan_warnings) = if chrome_user_data_dir.is_dir() {
        scan_chrome_profiles(&chrome_user_data_dir)
    } else {
        (Vec::new(), Vec::new())
    };
    warnings.extend(edge_scan_warnings);
    warnings.extend(chrome_scan_warnings);

    let mut profiles = merge_profiles(edge_discovered, &config);
    profiles.extend(merge_profiles_for_browser(
        BROWSER_CHROME,
        chrome_discovered,
        &config.chrome,
    ));
    apply_usage_stats(conn, &mut profiles)?;
    sort_profiles(&mut profiles);

    Ok(json!({
        "edgeFound": edge_path.is_some(),
        "edgePath": edge_path.map(|path| path.to_string_lossy().to_string()),
        "userDataDir": edge_user_data_dir.to_string_lossy().to_string(),
        "probedEdgePaths": probed_paths
            .into_iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>(),
        "chromeFound": chrome_path.is_some(),
        "chromePath": chrome_path.map(|path| path.to_string_lossy().to_string()),
        "chromeUserDataDir": chrome_user_data_dir.to_string_lossy().to_string(),
        "probedChromePaths": probed_chrome_paths
            .into_iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>(),
        "warnings": warnings,
        "profiles": profiles,
    }))
}

fn list_profiles() -> Result<Value, String> {
    let conn = super::helpers::db_conn()?;
    list_profiles_with_conn(&conn)
}

pub(crate) fn encode_action_target(browser: &str, profile_dir: &str) -> Result<String, String> {
    if !matches!(browser, BROWSER_EDGE | BROWSER_CHROME) {
        return Err("浏览器身份目标仅支持 Edge 和 Chrome".into());
    }
    if profile_dir.trim().is_empty() {
        return Err("浏览器身份 Profile 目录不能为空".into());
    }
    serde_json::to_string(&(browser, profile_dir))
        .map_err(|error| format!("编码浏览器身份目标失败: {error}"))
}

pub(crate) fn decode_action_target(target_id: &str) -> Result<(String, String), String> {
    let (browser, profile_dir): (String, String) = serde_json::from_str(target_id)
        .map_err(|error| format!("浏览器身份目标 ID 无效: {error}"))?;
    if !matches!(browser.as_str(), BROWSER_EDGE | BROWSER_CHROME) {
        return Err("浏览器身份目标仅支持 Edge 和 Chrome".into());
    }
    if profile_dir.trim().is_empty() {
        return Err("浏览器身份 Profile 目录不能为空".into());
    }
    Ok((browser, profile_dir))
}

fn build_action_targets(
    profiles: Vec<BrowserProfileItem>,
    edge_available: bool,
    chrome_available: bool,
) -> Result<Vec<(String, String, bool, Option<String>)>, String> {
    profiles
        .into_iter()
        .map(|profile| {
            let available = match profile.browser.as_str() {
                BROWSER_EDGE => edge_available,
                BROWSER_CHROME => chrome_available,
                _ => false,
            };
            let unavailable_reason = (!available).then(|| {
                format!(
                    "未找到 {}，无法启动该浏览器身份",
                    browser_executable_name(&profile.browser)
                )
            });
            let browser_label = if profile.browser == BROWSER_CHROME {
                "Chrome"
            } else {
                "Edge"
            };
            Ok((
                encode_action_target(&profile.browser, &profile.profile_dir)?,
                format!("{browser_label} · {}", profile_display_name(&profile)),
                available,
                unavailable_reason,
            ))
        })
        .collect()
}

pub(crate) fn list_action_targets_with_conn(
    conn: &rusqlite::Connection,
) -> Result<Vec<(String, String, bool, Option<String>)>, String> {
    let payload = list_profiles_with_conn(conn)?;
    let edge_available = payload
        .get("edgeFound")
        .and_then(Value::as_bool)
        .ok_or_else(|| "浏览器身份列表缺少 edgeFound".to_string())?;
    let chrome_available = payload
        .get("chromeFound")
        .and_then(Value::as_bool)
        .ok_or_else(|| "浏览器身份列表缺少 chromeFound".to_string())?;
    let profiles = serde_json::from_value(
        payload
            .get("profiles")
            .cloned()
            .ok_or_else(|| "浏览器身份列表缺少 profiles".to_string())?,
    )
    .map_err(|error| format!("读取浏览器身份动作目标失败: {error}"))?;
    build_action_targets(profiles, edge_available, chrome_available)
}

fn save_alias(payload: &Value) -> Result<Value, String> {
    let browser = require_supported_browser(payload)?;
    let profile_dir = require_profile_dir(payload)?;
    ensure_browser_profile_exists(browser, &profile_dir)?;
    let alias = payload["alias"].as_str().unwrap_or_default();
    mutate_config(|config| {
        save_alias_for_browser(config, browser, &profile_dir, alias);
        Ok(())
    })?;
    Ok(json!({ "ok": true }))
}

fn set_hidden(payload: &Value) -> Result<Value, String> {
    let browser = require_supported_browser(payload)?;
    let profile_dir = require_profile_dir(payload)?;
    ensure_browser_profile_exists(browser, &profile_dir)?;
    let hidden = payload["hidden"]
        .as_bool()
        .ok_or_else(|| "hidden is required".to_string())?;
    mutate_config(|config| {
        let entry = config_entries_mut(config, browser)
            .entry(profile_dir.clone())
            .or_default();
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

fn set_chrome_path(payload: &Value) -> Result<Value, String> {
    let chrome_path = payload["chromePath"]
        .as_str()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| "chromePath is required".to_string())?;
    validate_chrome_exe_path(Path::new(chrome_path))?;
    mutate_config(|config| {
        config.chrome_path = Some(chrome_path.to_string());
        Ok(())
    })?;
    Ok(json!({ "ok": true }))
}

struct LaunchProfileResult {
    launch_count: i64,
    last_launched_at: String,
    warnings: Vec<String>,
}

fn launch_profile_inner(browser: &str, profile_dir: &str) -> Result<LaunchProfileResult, String> {
    let mut warnings = Vec::new();
    let config = {
        let conn = super::helpers::db_conn()?;
        load_config_from_settings(&conn, &mut warnings)
    };
    let executable = match browser {
        BROWSER_EDGE => find_edge_path(config.edge_path.as_deref()).0,
        BROWSER_CHROME => find_chrome_path(config.chrome_path.as_deref()).0,
        _ => None,
    }
    .ok_or_else(|| format!("未找到 {}", browser_executable_name(browser)))?;
    ensure_browser_profile_exists(browser, profile_dir)?;

    Command::new(&executable)
        .arg(build_edge_profile_arg(profile_dir))
        .spawn()
        .map_err(|err| format!("launch failed: {err}"))?;

    let now = chrono::Utc::now().to_rfc3339();
    let resource_id = encode_action_target(browser, profile_dir)?;
    let usage_result = {
        let conn = super::helpers::db_conn()?;
        usage::record(
            &conn,
            UsageKey {
                resource_type: RESOURCE_BROWSER_PROFILE,
                scope_id: "",
                resource_id: &resource_id,
            },
            ACTION_LAUNCH,
        )
    };
    let launch_count = match usage_result {
        Ok(summary) => summary.total_count,
        Err(err) => {
            warnings.push(format!("启动成功，但使用统计保存失败：{err}"));
            config_entries(&config, browser)
                .get(profile_dir)
                .and_then(|entry| entry.launch_count)
                .unwrap_or_default()
                + 1
        }
    };

    Ok(LaunchProfileResult {
        launch_count,
        last_launched_at: now,
        warnings,
    })
}

fn launch_profile(payload: &Value) -> Result<Value, String> {
    let browser = require_supported_browser(payload)?;
    let profile_dir = require_profile_dir(payload)?;
    let result = launch_profile_inner(browser, &profile_dir)?;
    Ok(json!({
        "ok": true,
        "launchCount": result.launch_count,
        "lastLaunchedAt": result.last_launched_at,
        "warnings": result.warnings,
    }))
}

fn launch_action_target_with(
    target_id: &str,
    launch: impl FnOnce(&str, &str) -> Result<Vec<String>, String>,
) -> Result<Option<String>, String> {
    let (browser, profile_dir) = decode_action_target(target_id)?;
    let warnings = launch(&browser, &profile_dir)?;
    if warnings.is_empty() {
        Ok(None)
    } else {
        Ok(Some(warnings.join("\n")))
    }
}

pub(crate) fn launch_action_target(target_id: &str) -> Result<Option<String>, String> {
    launch_action_target_with(target_id, |browser, profile_dir| {
        Ok(launch_profile_inner(browser, profile_dir)?.warnings)
    })
}

fn require_supported_browser(payload: &Value) -> Result<&str, String> {
    match payload["browser"].as_str() {
        Some(browser @ (BROWSER_EDGE | BROWSER_CHROME)) => Ok(browser),
        _ => Err("仅支持 Edge 和 Chrome".into()),
    }
}

fn browser_executable_name(browser: &str) -> &'static str {
    match browser {
        BROWSER_CHROME => "chrome.exe",
        _ => "msedge.exe",
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

fn ensure_browser_profile_exists(browser: &str, profile_dir: &str) -> Result<(), String> {
    let (profiles, _) = match browser {
        BROWSER_CHROME => scan_chrome_profiles(&chrome_user_data_dir()),
        _ => scan_edge_profiles(&edge_user_data_dir()),
    };
    if profiles
        .iter()
        .any(|profile| profile.profile_dir == profile_dir)
    {
        Ok(())
    } else {
        Err(format!("Profile 已不存在: {profile_dir}"))
    }
}

#[cfg(test)]
fn save_alias_in_config(config: &mut BrowserProfilesConfig, profile_dir: &str, alias: &str) {
    save_alias_for_browser(config, BROWSER_EDGE, profile_dir, alias);
}

fn save_alias_for_browser(
    config: &mut BrowserProfilesConfig,
    browser: &str,
    profile_dir: &str,
    alias: &str,
) {
    let entry = config_entries_mut(config, browser)
        .entry(profile_dir.to_string())
        .or_default();
    entry.alias = Some(alias.trim().to_string());
}

#[cfg(test)]
fn update_launch_stats_in_config(config: &mut BrowserProfilesConfig, profile_dir: &str, now: &str) {
    update_launch_stats_for_browser(config, BROWSER_EDGE, profile_dir, now);
}

#[cfg(test)]
fn update_launch_stats_for_browser(
    config: &mut BrowserProfilesConfig,
    browser: &str,
    profile_dir: &str,
    now: &str,
) {
    let entry = config_entries_mut(config, browser)
        .entry(profile_dir.to_string())
        .or_default();
    entry.launch_count = Some(entry.launch_count.unwrap_or_default() + 1);
    entry.last_launched_at = Some(now.to_string());
}

fn apply_usage_stats(
    conn: &rusqlite::Connection,
    profiles: &mut [BrowserProfileItem],
) -> Result<(), String> {
    let summaries = usage::summaries_for_type(conn, RESOURCE_BROWSER_PROFILE, 30)?;
    for profile in profiles {
        let resource_id = encode_action_target(&profile.browser, &profile.profile_dir)?;
        let summary = summaries
            .get(&(String::new(), resource_id))
            .cloned()
            .unwrap_or_default();
        profile.launch_count = summary.total_count;
        profile.last_launched_at = usage::format_timestamp_ms(summary.last_used_at);
    }
    Ok(())
}

fn config_entries<'a>(
    config: &'a BrowserProfilesConfig,
    browser: &str,
) -> &'a BTreeMap<String, BrowserProfileConfigEntry> {
    if browser == BROWSER_CHROME {
        &config.chrome
    } else {
        &config.edge
    }
}

fn config_entries_mut<'a>(
    config: &'a mut BrowserProfilesConfig,
    browser: &str,
) -> &'a mut BTreeMap<String, BrowserProfileConfigEntry> {
    if browser == BROWSER_CHROME {
        &mut config.chrome
    } else {
        &mut config.edge
    }
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
    profile_display_name(item).to_lowercase()
}

fn profile_display_name(item: &BrowserProfileItem) -> &str {
    for candidate in [&item.alias, &item.edge_display_name, &item.profile_dir] {
        let trimmed = candidate.trim();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }
    ""
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
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::sync::{mpsc, Arc};
    use std::thread;
    use std::time::Duration;

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
    fn unavailable_browser_profiles_remain_action_targets_with_a_reason() {
        let targets = build_action_targets(
            vec![item("Profile 1", "工作", "", false, 0, None)],
            false,
            true,
        )
        .unwrap();

        assert_eq!(targets.len(), 1);
        assert_eq!(
            decode_action_target(&targets[0].0).unwrap(),
            ("edge".into(), "Profile 1".into())
        );
        assert_eq!(targets[0].1, "Edge · 工作");
        assert!(!targets[0].2);
        assert!(targets[0].3.as_deref().unwrap().contains("msedge.exe"));
    }

    #[test]
    fn config_mutation_lock_allows_only_one_concurrent_critical_section() {
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        let (first_entered_tx, first_entered_rx) = mpsc::channel();
        let (release_first_tx, release_first_rx) = mpsc::channel();

        let first_active = active.clone();
        let first_max_active = max_active.clone();
        let first = thread::spawn(move || {
            with_config_mutation_lock(|| {
                let current = first_active.fetch_add(1, AtomicOrdering::SeqCst) + 1;
                first_max_active.fetch_max(current, AtomicOrdering::SeqCst);
                first_entered_tx.send(()).expect("signal first entry");
                release_first_rx.recv().expect("release first entry");
                first_active.fetch_sub(1, AtomicOrdering::SeqCst);
                Ok(())
            })
        });
        first_entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("first mutation enters critical section");

        let second_active = active.clone();
        let second_max_active = max_active.clone();
        let (second_attempted_tx, second_attempted_rx) = mpsc::channel();
        let (second_entered_tx, second_entered_rx) = mpsc::channel();
        let second = thread::spawn(move || {
            second_attempted_tx
                .send(())
                .expect("signal second mutation attempt");
            with_config_mutation_lock(|| {
                let current = second_active.fetch_add(1, AtomicOrdering::SeqCst) + 1;
                second_max_active.fetch_max(current, AtomicOrdering::SeqCst);
                second_entered_tx.send(()).expect("signal second entry");
                second_active.fetch_sub(1, AtomicOrdering::SeqCst);
                Ok(())
            })
        });
        second_attempted_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("second mutation attempts critical section");
        assert!(matches!(
            second_entered_rx.recv_timeout(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));

        release_first_tx.send(()).expect("release first mutation");
        first
            .join()
            .expect("join first mutation")
            .expect("first mutation succeeds");
        second_entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("second mutation enters after first exits");
        second
            .join()
            .expect("join second mutation")
            .expect("second mutation succeeds");

        assert_eq!(max_active.load(AtomicOrdering::SeqCst), 1);
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
    fn accepts_edge_and_chrome_but_rejects_unknown_browser() {
        assert_eq!(
            require_supported_browser(&json!({ "browser": "edge" })).unwrap(),
            BROWSER_EDGE
        );
        assert_eq!(
            require_supported_browser(&json!({ "browser": "chrome" })).unwrap(),
            BROWSER_CHROME
        );
        let err = require_supported_browser(&json!({ "browser": "firefox" }))
            .expect_err("firefox unsupported");
        assert!(err.contains("Edge") && err.contains("Chrome"));
    }

    #[test]
    fn chrome_alias_is_isolated_from_edge_config() {
        let mut config = BrowserProfilesConfig::default();
        save_alias_for_browser(&mut config, BROWSER_CHROME, "Profile 2", "Chrome 工作");
        assert!(config.edge.is_empty());
        assert_eq!(
            config
                .chrome
                .get("Profile 2")
                .and_then(|entry| entry.alias.as_deref()),
            Some("Chrome 工作")
        );
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

    #[test]
    fn browser_action_target_decodes_and_always_invokes_launch_core() {
        let target_id = encode_action_target(BROWSER_EDGE, "Profile 1").unwrap();
        let mut launches = Vec::new();

        let first = launch_action_target_with(&target_id, |browser, profile_dir| {
            launches.push((browser.to_string(), profile_dir.to_string()));
            Ok(Vec::new())
        })
        .unwrap();
        let second = launch_action_target_with(&target_id, |browser, profile_dir| {
            launches.push((browser.to_string(), profile_dir.to_string()));
            Ok(vec!["启动成功，但使用统计保存失败：磁盘只读".into()])
        })
        .unwrap();

        assert_eq!(
            launches,
            vec![
                (BROWSER_EDGE.to_string(), "Profile 1".to_string()),
                (BROWSER_EDGE.to_string(), "Profile 1".to_string()),
            ]
        );
        assert_eq!(first, None);
        assert_eq!(
            second.as_deref(),
            Some("启动成功，但使用统计保存失败：磁盘只读")
        );
    }
}

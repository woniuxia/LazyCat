use crate::tools::access_path_diagnostics::model::{AccessProtocol, NormalizedAccessPathTarget};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyProfileKind {
    Environment,
    WindowsUser,
    WinHttp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyProfileAvailability {
    Available,
    Unavailable,
    ReadError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyRoute {
    Direct,
    Proxy,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyCapabilityKind {
    Pac,
    Wpad,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyCapabilityState {
    NotConfigured,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyCapability {
    pub kind: ProxyCapabilityKind,
    pub state: ProxyCapabilityState,
    pub configured: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sanitized_location: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SanitizedProxyEndpoint {
    pub url: String,
    pub scheme: String,
    pub host: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default)]
    pub credentials_redacted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyDecision {
    pub route: ProxyRoute,
    pub configuration_source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<SanitizedProxyEndpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_rule: Option<String>,
    #[serde(default)]
    pub uncertainties: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyProfileSnapshot {
    pub kind: ProxyProfileKind,
    pub availability: ProxyProfileAvailability,
    #[serde(default)]
    pub configuration_sources: Vec<String>,
    pub decision: ProxyDecision,
    #[serde(default)]
    pub capabilities: Vec<ProxyCapability>,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyInspection {
    pub recommended_profile: ProxyProfileKind,
    pub recommendation_reason: String,
    pub profiles: Vec<ProxyProfileSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProxySetting {
    scope: String,
    source: String,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProfileConfiguration {
    availability: ProxyProfileAvailability,
    enabled: bool,
    proxy_settings: Vec<ProxySetting>,
    bypass_rules: Vec<String>,
    configuration_sources: Vec<String>,
    pac_url: Option<String>,
    wpad_enabled: bool,
    errors: Vec<String>,
}

impl ProfileConfiguration {
    #[cfg(not(windows))]
    fn unavailable(error: impl Into<String>) -> Self {
        Self {
            availability: ProxyProfileAvailability::Unavailable,
            enabled: false,
            proxy_settings: Vec::new(),
            bypass_rules: Vec::new(),
            configuration_sources: Vec::new(),
            pac_url: None,
            wpad_enabled: false,
            errors: vec![error.into()],
        }
    }
}

/// Reads all supported client proxy profiles without modifying system state.
///
/// `recommended_profile` is a diagnostic default, not a claim that every
/// application uses the same proxy stack. Callers should let users select a
/// concrete profile when reproducing a specific client's access path.
pub fn inspect_proxy_profiles(target: &NormalizedAccessPathTarget) -> ProxyInspection {
    let environment = inspect_configuration(
        target,
        ProxyProfileKind::Environment,
        read_environment_configuration(),
    );
    let windows_user = inspect_configuration(
        target,
        ProxyProfileKind::WindowsUser,
        read_wininet_configuration(),
    );
    let winhttp = inspect_configuration(
        target,
        ProxyProfileKind::WinHttp,
        read_winhttp_configuration(),
    );

    let (recommended_profile, recommendation_reason) = if environment.decision.route
        != ProxyRoute::Direct
        || !environment.configuration_sources.is_empty()
    {
        (
            ProxyProfileKind::Environment,
            "检测到环境变量代理配置；默认按环境变量兼容客户端画像展示".to_string(),
        )
    } else if windows_user.availability == ProxyProfileAvailability::Available {
        (
            ProxyProfileKind::WindowsUser,
            "Windows 桌面应用默认建议检查当前用户 WinINET 画像".to_string(),
        )
    } else {
        (
            ProxyProfileKind::WinHttp,
            "当前用户 WinINET 画像不可用，默认展示 WinHTTP 画像".to_string(),
        )
    };

    ProxyInspection {
        recommended_profile,
        recommendation_reason,
        profiles: vec![environment, windows_user, winhttp],
    }
}

/// Reads and evaluates one explicitly selected client profile.
pub fn inspect_proxy_profile(
    target: &NormalizedAccessPathTarget,
    kind: ProxyProfileKind,
) -> ProxyProfileSnapshot {
    let configuration = match kind {
        ProxyProfileKind::Environment => read_environment_configuration(),
        ProxyProfileKind::WindowsUser => read_wininet_configuration(),
        ProxyProfileKind::WinHttp => read_winhttp_configuration(),
    };
    inspect_configuration(target, kind, configuration)
}

fn inspect_configuration(
    target: &NormalizedAccessPathTarget,
    kind: ProxyProfileKind,
    configuration: ProfileConfiguration,
) -> ProxyProfileSnapshot {
    let capabilities = vec![
        ProxyCapability {
            kind: ProxyCapabilityKind::Pac,
            state: if configuration.pac_url.is_some() {
                ProxyCapabilityState::Unsupported
            } else {
                ProxyCapabilityState::NotConfigured
            },
            configured: configuration.pac_url.is_some(),
            sanitized_location: configuration.pac_url.as_deref().map(sanitize_url_location),
            detail: if configuration.pac_url.is_some() {
                "已检测到 PAC，但当前版本不会获取或执行 PAC".to_string()
            } else {
                "未检测到 PAC 配置".to_string()
            },
        },
        ProxyCapability {
            kind: ProxyCapabilityKind::Wpad,
            state: if configuration.wpad_enabled {
                ProxyCapabilityState::Unsupported
            } else {
                ProxyCapabilityState::NotConfigured
            },
            configured: configuration.wpad_enabled,
            sanitized_location: None,
            detail: if configuration.wpad_enabled {
                "已检测到 WPAD 自动发现，但当前版本不会发起自动发现请求".to_string()
            } else {
                "未检测到 WPAD 自动发现配置".to_string()
            },
        },
    ];

    let decision = decide_route(target, kind, &configuration);
    ProxyProfileSnapshot {
        kind,
        availability: configuration.availability,
        configuration_sources: configuration.configuration_sources,
        decision,
        capabilities,
        errors: configuration.errors,
    }
}

fn decide_route(
    target: &NormalizedAccessPathTarget,
    kind: ProxyProfileKind,
    configuration: &ProfileConfiguration,
) -> ProxyDecision {
    let default_source = profile_label(kind).to_string();
    if configuration.availability != ProxyProfileAvailability::Available {
        return ProxyDecision {
            route: ProxyRoute::Unresolved,
            configuration_source: default_source,
            proxy: None,
            matched_rule: None,
            uncertainties: vec!["无法读取该客户端画像，不能推断代理路由".to_string()],
        };
    }

    if configuration.pac_url.is_some() || configuration.wpad_enabled {
        return ProxyDecision {
            route: ProxyRoute::Unresolved,
            configuration_source: default_source,
            proxy: None,
            matched_rule: None,
            uncertainties: vec![
                "PAC/WPAD 决策能力尚未实现，静态代理配置不能代表最终路由".to_string()
            ],
        };
    }

    if let Some(rule) =
        find_matching_bypass_rule(&target.hostname, target.port, &configuration.bypass_rules)
    {
        return ProxyDecision {
            route: ProxyRoute::Direct,
            configuration_source: default_source,
            proxy: None,
            matched_rule: Some(rule),
            uncertainties: Vec::new(),
        };
    }

    if !configuration.enabled {
        return ProxyDecision {
            route: ProxyRoute::Direct,
            configuration_source: default_source,
            proxy: None,
            matched_rule: None,
            uncertainties: Vec::new(),
        };
    }

    let Some(setting) = select_proxy_setting(&configuration.proxy_settings, target.protocol) else {
        return ProxyDecision {
            route: ProxyRoute::Unresolved,
            configuration_source: default_source,
            proxy: None,
            matched_rule: None,
            uncertainties: vec!["代理已启用，但没有适用于当前协议的代理地址".to_string()],
        };
    };

    match sanitize_proxy_endpoint(&setting.value) {
        Ok(proxy) => {
            let mut uncertainties = Vec::new();
            if !matches!(proxy.scheme.as_str(), "http" | "https") {
                uncertainties.push(format!(
                    "代理协议 {} 的后续连接探测能力需要单独确认",
                    proxy.scheme
                ));
            }
            ProxyDecision {
                route: ProxyRoute::Proxy,
                configuration_source: setting.source.clone(),
                proxy: Some(proxy),
                matched_rule: None,
                uncertainties,
            }
        }
        Err(error) => ProxyDecision {
            route: ProxyRoute::Unresolved,
            configuration_source: setting.source.clone(),
            proxy: None,
            matched_rule: None,
            uncertainties: vec![format!("代理地址格式无效：{error}")],
        },
    }
}

fn profile_label(kind: ProxyProfileKind) -> &'static str {
    match kind {
        ProxyProfileKind::Environment => "environment",
        ProxyProfileKind::WindowsUser => "windows_user",
        ProxyProfileKind::WinHttp => "winhttp",
    }
}

fn read_environment_configuration() -> ProfileConfiguration {
    let entries = [
        "http_proxy",
        "HTTP_PROXY",
        "https_proxy",
        "HTTPS_PROXY",
        "all_proxy",
        "ALL_PROXY",
        "no_proxy",
        "NO_PROXY",
    ]
    .into_iter()
    .filter_map(|name| {
        std::env::var_os(name).map(|value| (name.to_string(), value.to_string_lossy().into_owned()))
    })
    .collect::<Vec<_>>();
    environment_configuration_from_entries(&entries)
}

fn environment_configuration_from_entries(entries: &[(String, String)]) -> ProfileConfiguration {
    let mut proxy_settings = Vec::new();
    let mut bypass_rules = Vec::new();
    let mut configuration_sources = Vec::new();
    let mut errors = Vec::new();

    for (scope, names) in [
        ("http", ["http_proxy", "HTTP_PROXY"]),
        ("https", ["https_proxy", "HTTPS_PROXY"]),
        ("all", ["all_proxy", "ALL_PROXY"]),
    ] {
        let values = names
            .iter()
            .filter_map(|name| {
                entries
                    .iter()
                    .find(|(entry_name, value)| entry_name == name && !value.trim().is_empty())
                    .map(|(entry_name, value)| (entry_name.clone(), value.clone()))
            })
            .collect::<Vec<_>>();
        if let Some((name, value)) = values.first() {
            proxy_settings.push(ProxySetting {
                scope: scope.to_string(),
                source: name.clone(),
                value: value.clone(),
            });
            configuration_sources.push(name.clone());
        }
        if values.len() > 1 && values.iter().any(|(_, value)| value != &values[0].1) {
            errors.push(format!(
                "{} 与 {} 同时存在且值不同，按小写变量优先",
                names[0], names[1]
            ));
        }
    }

    let no_proxy_values = ["no_proxy", "NO_PROXY"]
        .iter()
        .filter_map(|name| {
            entries
                .iter()
                .find(|(entry_name, value)| entry_name == name && !value.trim().is_empty())
                .map(|(entry_name, value)| (entry_name.clone(), value.clone()))
        })
        .collect::<Vec<_>>();
    if let Some((name, value)) = no_proxy_values.first() {
        bypass_rules = split_bypass_rules(value);
        configuration_sources.push(name.clone());
    }
    if no_proxy_values.len() > 1
        && no_proxy_values
            .iter()
            .any(|(_, value)| value != &no_proxy_values[0].1)
    {
        errors.push("no_proxy 与 NO_PROXY 同时存在且值不同，按小写变量优先".to_string());
    }

    ProfileConfiguration {
        availability: ProxyProfileAvailability::Available,
        enabled: !proxy_settings.is_empty(),
        proxy_settings,
        bypass_rules,
        configuration_sources,
        pac_url: None,
        wpad_enabled: false,
        errors,
    }
}

fn parse_static_proxy_settings(value: &str, source_prefix: &str) -> Vec<ProxySetting> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if !trimmed.contains('=') {
        return vec![ProxySetting {
            scope: "all".to_string(),
            source: format!("{source_prefix}:all"),
            value: trimmed.to_string(),
        }];
    }

    trimmed
        .split(';')
        .filter_map(|entry| {
            let (scope, endpoint) = entry.split_once('=')?;
            let scope = scope.trim().to_ascii_lowercase();
            let endpoint = endpoint.trim();
            if endpoint.is_empty() || !matches!(scope.as_str(), "http" | "https" | "all") {
                return None;
            }
            Some(ProxySetting {
                source: format!("{source_prefix}:{scope}"),
                scope,
                value: endpoint.to_string(),
            })
        })
        .collect()
}

fn select_proxy_setting(
    settings: &[ProxySetting],
    protocol: AccessProtocol,
) -> Option<&ProxySetting> {
    let protocol_key = match protocol {
        AccessProtocol::Http => "http",
        AccessProtocol::Https => "https",
    };
    settings
        .iter()
        .find(|setting| setting.scope.eq_ignore_ascii_case(protocol_key))
        .or_else(|| {
            settings
                .iter()
                .find(|setting| setting.scope.eq_ignore_ascii_case("all"))
        })
}

fn split_bypass_rules(value: &str) -> Vec<String> {
    value
        .split([',', ';'])
        .map(str::trim)
        .filter(|rule| !rule.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn find_matching_bypass_rule(host: &str, port: u16, rules: &[String]) -> Option<String> {
    rules
        .iter()
        .find(|rule| bypass_rule_matches(host, port, rule))
        .cloned()
}

pub fn bypass_rule_matches(host: &str, port: u16, rule: &str) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let rule = rule.trim();
    if rule.is_empty() {
        return false;
    }
    if rule == "*" {
        return true;
    }
    if rule.eq_ignore_ascii_case("<local>") {
        return !host.contains('.') && host.parse::<IpAddr>().is_err();
    }

    if let Some((network, prefix)) = parse_cidr(rule) {
        return host
            .parse::<IpAddr>()
            .is_ok_and(|address| ip_in_network(address, network, prefix));
    }

    let (rule_host, rule_port) = split_rule_host_port(rule);
    if rule_port.is_some_and(|expected| expected != port) {
        return false;
    }
    let rule_host = rule_host.trim_end_matches('.').to_ascii_lowercase();

    if let (Ok(target_ip), Ok(rule_ip)) = (host.parse::<IpAddr>(), rule_host.parse::<IpAddr>()) {
        return target_ip == rule_ip;
    }
    if let Some(suffix) = rule_host.strip_prefix("*.") {
        return host.ends_with(&format!(".{suffix}"));
    }
    if let Some(suffix) = rule_host.strip_prefix('.') {
        return host == suffix || host.ends_with(&format!(".{suffix}"));
    }
    if rule_host.contains('*') {
        return wildcard_matches(&host, &rule_host);
    }
    host == rule_host || host.ends_with(&format!(".{rule_host}"))
}

fn split_rule_host_port(rule: &str) -> (&str, Option<u16>) {
    if let Some(rest) = rule.strip_prefix('[') {
        if let Some(close) = rest.find(']') {
            let host = &rest[..close];
            let suffix = &rest[close + 1..];
            let port = suffix
                .strip_prefix(':')
                .and_then(|value| value.parse::<u16>().ok());
            return (host, port);
        }
    }
    if rule.matches(':').count() == 1 {
        if let Some((host, port)) = rule.rsplit_once(':') {
            if let Ok(port) = port.parse::<u16>() {
                return (host, Some(port));
            }
        }
    }
    (rule, None)
}

fn parse_cidr(rule: &str) -> Option<(IpAddr, u8)> {
    let (network, prefix) = rule.split_once('/')?;
    let network = network.parse::<IpAddr>().ok()?;
    let prefix = prefix.parse::<u8>().ok()?;
    let valid = match network {
        IpAddr::V4(_) => prefix <= 32,
        IpAddr::V6(_) => prefix <= 128,
    };
    valid.then_some((network, prefix))
}

fn ip_in_network(address: IpAddr, network: IpAddr, prefix: u8) -> bool {
    match (address, network) {
        (IpAddr::V4(address), IpAddr::V4(network)) => {
            prefix == 0
                || (u32::from(address) >> (32 - prefix)) == (u32::from(network) >> (32 - prefix))
        }
        (IpAddr::V6(address), IpAddr::V6(network)) => {
            prefix == 0
                || (u128::from(address) >> (128 - prefix))
                    == (u128::from(network) >> (128 - prefix))
        }
        _ => false,
    }
}

fn wildcard_matches(value: &str, pattern: &str) -> bool {
    let parts = pattern.split('*').collect::<Vec<_>>();
    let mut remaining = value;
    let mut first = true;
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            first = false;
            continue;
        }
        if first && !remaining.starts_with(part) {
            return false;
        }
        let Some(position) = remaining.find(part) else {
            return false;
        };
        remaining = &remaining[position + part.len()..];
        if index == parts.len() - 1 && !pattern.ends_with('*') && !remaining.is_empty() {
            return false;
        }
        first = false;
    }
    true
}

fn sanitize_proxy_endpoint(value: &str) -> Result<SanitizedProxyEndpoint, String> {
    let value = value.trim();
    let candidate = if value.contains("://") {
        value.to_string()
    } else {
        format!("http://{value}")
    };
    let parsed = Url::parse(&candidate).map_err(|error| error.to_string())?;
    let host = parsed
        .host_str()
        .filter(|host| !host.is_empty())
        .ok_or_else(|| "缺少代理主机".to_string())?
        .to_string();
    let scheme = parsed.scheme().to_ascii_lowercase();
    let credentials_redacted = !parsed.username().is_empty() || parsed.password().is_some();
    let host_display = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.clone()
    };
    let port = parsed.port();
    let url = match port {
        Some(port) => format!("{scheme}://{host_display}:{port}"),
        None => format!("{scheme}://{host_display}"),
    };
    Ok(SanitizedProxyEndpoint {
        url,
        scheme,
        host,
        port,
        credentials_redacted,
    })
}

fn sanitize_url_location(value: &str) -> String {
    let Ok(mut parsed) = Url::parse(value) else {
        return "<configured-but-invalid>".to_string();
    };
    let had_credentials = !parsed.username().is_empty() || parsed.password().is_some();
    let _ = parsed.set_username("");
    let _ = parsed.set_password(None);
    parsed.set_query(None);
    parsed.set_fragment(None);
    let mut sanitized = parsed.to_string();
    if had_credentials {
        sanitized.push_str("#credentials-redacted");
    }
    sanitized
}

#[cfg(windows)]
fn read_wininet_configuration() -> ProfileConfiguration {
    use std::ffi::c_void;
    use std::slice;
    use windows_sys::Win32::Foundation::GlobalFree;

    #[repr(C)]
    struct WinHttpCurrentUserIeProxyConfig {
        auto_detect: i32,
        auto_config_url: *mut u16,
        proxy: *mut u16,
        proxy_bypass: *mut u16,
    }

    #[link(name = "winhttp")]
    extern "system" {
        fn WinHttpGetIEProxyConfigForCurrentUser(
            proxy_config: *mut WinHttpCurrentUserIeProxyConfig,
        ) -> i32;
    }

    unsafe fn take_wide_string(pointer: *mut u16) -> Option<String> {
        if pointer.is_null() {
            return None;
        }
        let mut length = 0usize;
        while *pointer.add(length) != 0 {
            length += 1;
        }
        let value = String::from_utf16_lossy(slice::from_raw_parts(pointer, length));
        GlobalFree(pointer.cast::<c_void>());
        Some(value)
    }

    let mut config = WinHttpCurrentUserIeProxyConfig {
        auto_detect: 0,
        auto_config_url: std::ptr::null_mut(),
        proxy: std::ptr::null_mut(),
        proxy_bypass: std::ptr::null_mut(),
    };
    if unsafe { WinHttpGetIEProxyConfigForCurrentUser(&mut config) } == 0 {
        return ProfileConfiguration {
            availability: ProxyProfileAvailability::ReadError,
            enabled: false,
            proxy_settings: Vec::new(),
            bypass_rules: Vec::new(),
            configuration_sources: vec!["WinHttpGetIEProxyConfigForCurrentUser".to_string()],
            pac_url: None,
            wpad_enabled: false,
            errors: vec![format!(
                "WinHttpGetIEProxyConfigForCurrentUser failed: {}",
                std::io::Error::last_os_error()
            )],
        };
    }

    let proxy_server =
        unsafe { take_wide_string(config.proxy) }.filter(|value| !value.trim().is_empty());
    let proxy_override =
        unsafe { take_wide_string(config.proxy_bypass) }.filter(|value| !value.trim().is_empty());
    let pac_url = unsafe { take_wide_string(config.auto_config_url) }
        .filter(|value| !value.trim().is_empty());
    let wpad_enabled = config.auto_detect != 0;

    ProfileConfiguration {
        availability: ProxyProfileAvailability::Available,
        enabled: proxy_server.is_some(),
        proxy_settings: proxy_server
            .as_deref()
            .map(|value| parse_static_proxy_settings(value, "wininet"))
            .unwrap_or_default(),
        bypass_rules: proxy_override
            .as_deref()
            .map(split_bypass_rules)
            .unwrap_or_default(),
        configuration_sources: vec!["WinHttpGetIEProxyConfigForCurrentUser".to_string()],
        pac_url,
        wpad_enabled,
        errors: Vec::new(),
    }
}

#[cfg(not(windows))]
fn read_wininet_configuration() -> ProfileConfiguration {
    ProfileConfiguration::unavailable("WinINET 画像仅在 Windows 上可读")
}

#[cfg(windows)]
fn read_winhttp_configuration() -> ProfileConfiguration {
    use std::ffi::c_void;
    use std::slice;
    use windows_sys::Win32::Foundation::GlobalFree;

    const WINHTTP_ACCESS_TYPE_NO_PROXY: u32 = 1;
    const WINHTTP_ACCESS_TYPE_NAMED_PROXY: u32 = 3;

    #[repr(C)]
    struct WinHttpProxyInfo {
        access_type: u32,
        proxy: *mut u16,
        proxy_bypass: *mut u16,
    }

    #[link(name = "winhttp")]
    extern "system" {
        fn WinHttpGetDefaultProxyConfiguration(proxy_info: *mut WinHttpProxyInfo) -> i32;
    }

    unsafe fn take_wide_string(pointer: *mut u16) -> Option<String> {
        if pointer.is_null() {
            return None;
        }
        let mut length = 0usize;
        while *pointer.add(length) != 0 {
            length += 1;
        }
        let value = String::from_utf16_lossy(slice::from_raw_parts(pointer, length));
        GlobalFree(pointer.cast::<c_void>());
        Some(value)
    }

    let mut info = WinHttpProxyInfo {
        access_type: 0,
        proxy: std::ptr::null_mut(),
        proxy_bypass: std::ptr::null_mut(),
    };
    let succeeded = unsafe { WinHttpGetDefaultProxyConfiguration(&mut info) } != 0;
    if !succeeded {
        return ProfileConfiguration {
            availability: ProxyProfileAvailability::ReadError,
            enabled: false,
            proxy_settings: Vec::new(),
            bypass_rules: Vec::new(),
            configuration_sources: vec!["WinHttpGetDefaultProxyConfiguration".to_string()],
            pac_url: None,
            wpad_enabled: false,
            errors: vec![format!(
                "WinHttpGetDefaultProxyConfiguration failed: {}",
                std::io::Error::last_os_error()
            )],
        };
    }

    let proxy = unsafe { take_wide_string(info.proxy) };
    let bypass = unsafe { take_wide_string(info.proxy_bypass) };
    let mut errors = Vec::new();
    if !matches!(
        info.access_type,
        WINHTTP_ACCESS_TYPE_NO_PROXY | WINHTTP_ACCESS_TYPE_NAMED_PROXY
    ) {
        errors.push(format!("未知 WinHTTP access type: {}", info.access_type));
    }

    ProfileConfiguration {
        availability: ProxyProfileAvailability::Available,
        enabled: info.access_type == WINHTTP_ACCESS_TYPE_NAMED_PROXY,
        proxy_settings: proxy
            .as_deref()
            .map(|value| parse_static_proxy_settings(value, "winhttp"))
            .unwrap_or_default(),
        bypass_rules: bypass
            .as_deref()
            .map(split_bypass_rules)
            .unwrap_or_default(),
        configuration_sources: vec!["WinHttpGetDefaultProxyConfiguration".to_string()],
        pac_url: None,
        wpad_enabled: false,
        errors,
    }
}

#[cfg(not(windows))]
fn read_winhttp_configuration() -> ProfileConfiguration {
    ProfileConfiguration::unavailable("WinHTTP 画像仅在 Windows 上可读")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::access_path_diagnostics::model::AccessPathTargetKind;

    fn target(hostname: &str, port: u16, protocol: AccessProtocol) -> NormalizedAccessPathTarget {
        NormalizedAccessPathTarget {
            raw_input: hostname.to_string(),
            protocol,
            hostname: hostname.to_string(),
            target_kind: hostname
                .parse::<IpAddr>()
                .map(|address| match address {
                    IpAddr::V4(_) => AccessPathTargetKind::Ipv4,
                    IpAddr::V6(_) => AccessPathTargetKind::Ipv6,
                })
                .unwrap_or(AccessPathTargetKind::Hostname),
            port,
            path: "/".to_string(),
            url: format!("https://{hostname}:{port}/"),
            sni: Some(hostname.to_string()),
            verify_hostname: Some(hostname.to_string()),
            http_host: hostname.to_string(),
            connection_ip: None,
        }
    }

    #[test]
    fn no_proxy_matches_domains_suffixes_ports_and_local_names() {
        assert!(bypass_rule_matches("api.example.test", 443, "example.test"));
        assert!(bypass_rule_matches("example.test", 443, ".example.test"));
        assert!(bypass_rule_matches(
            "api.example.test",
            443,
            "*.example.test"
        ));
        assert!(!bypass_rule_matches("example.test", 443, "*.example.test"));
        assert!(bypass_rule_matches(
            "api.example.test",
            8443,
            "api.example.test:8443"
        ));
        assert!(!bypass_rule_matches(
            "api.example.test",
            443,
            "api.example.test:8443"
        ));
        assert!(bypass_rule_matches("intranet", 80, "<local>"));
    }

    #[test]
    fn no_proxy_matches_ipv4_and_ipv6_cidr() {
        assert!(bypass_rule_matches("10.23.4.5", 443, "10.0.0.0/8"));
        assert!(!bypass_rule_matches("11.23.4.5", 443, "10.0.0.0/8"));
        assert!(bypass_rule_matches("2001:db8::42", 443, "2001:db8::/32"));
        assert!(bypass_rule_matches(
            "2001:db8::42",
            8443,
            "[2001:db8::42]:8443"
        ));
        assert!(!bypass_rule_matches(
            "2001:db8::42",
            443,
            "[2001:db8::42]:8443"
        ));
    }

    #[test]
    fn environment_uses_lowercase_first_and_reports_conflicts() {
        let configuration = environment_configuration_from_entries(&[
            ("https_proxy".to_string(), "lower.test:8443".to_string()),
            ("HTTPS_PROXY".to_string(), "upper.test:9443".to_string()),
            ("NO_PROXY".to_string(), ".internal.test".to_string()),
        ]);
        let snapshot = inspect_configuration(
            &target("public.test", 443, AccessProtocol::Https),
            ProxyProfileKind::Environment,
            configuration,
        );

        assert_eq!(snapshot.decision.route, ProxyRoute::Proxy);
        assert_eq!(snapshot.decision.proxy.unwrap().host, "lower.test");
        assert!(snapshot.errors.iter().any(|error| error.contains("值不同")));
    }

    #[test]
    fn no_proxy_rule_changes_environment_decision_to_direct() {
        let configuration = environment_configuration_from_entries(&[
            ("HTTPS_PROXY".to_string(), "proxy.test:8080".to_string()),
            (
                "NO_PROXY".to_string(),
                "10.0.0.0/8,.internal.test".to_string(),
            ),
        ]);
        let snapshot = inspect_configuration(
            &target("api.internal.test", 443, AccessProtocol::Https),
            ProxyProfileKind::Environment,
            configuration,
        );

        assert_eq!(snapshot.decision.route, ProxyRoute::Direct);
        assert_eq!(
            snapshot.decision.matched_rule.as_deref(),
            Some(".internal.test")
        );
    }

    #[test]
    fn proxy_credentials_and_pac_query_are_redacted() {
        let endpoint = sanitize_proxy_endpoint("http://alice:secret@proxy.test:8080/path?token=x")
            .expect("parse proxy");
        assert_eq!(endpoint.url, "http://proxy.test:8080");
        assert!(endpoint.credentials_redacted);
        assert!(!endpoint.url.contains("alice"));
        assert!(!endpoint.url.contains("secret"));

        let pac = sanitize_url_location("https://alice:secret@config.test/proxy.pac?token=x");
        assert!(!pac.contains("alice"));
        assert!(!pac.contains("secret"));
        assert!(!pac.contains("token"));
    }

    #[test]
    fn pac_configuration_is_unresolved_instead_of_static_success() {
        let configuration = ProfileConfiguration {
            availability: ProxyProfileAvailability::Available,
            enabled: true,
            proxy_settings: parse_static_proxy_settings("proxy.test:8080", "wininet"),
            bypass_rules: Vec::new(),
            configuration_sources: vec!["test".to_string()],
            pac_url: Some("https://config.test/proxy.pac".to_string()),
            wpad_enabled: false,
            errors: Vec::new(),
        };
        let snapshot = inspect_configuration(
            &target("example.test", 443, AccessProtocol::Https),
            ProxyProfileKind::WindowsUser,
            configuration,
        );

        assert_eq!(snapshot.decision.route, ProxyRoute::Unresolved);
        assert_eq!(
            snapshot.capabilities[0].state,
            ProxyCapabilityState::Unsupported
        );
    }

    #[test]
    fn static_protocol_mapping_selects_https_proxy() {
        let configuration = ProfileConfiguration {
            availability: ProxyProfileAvailability::Available,
            enabled: true,
            proxy_settings: parse_static_proxy_settings(
                "http=plain.test:8080;https=secure.test:8443",
                "wininet",
            ),
            bypass_rules: Vec::new(),
            configuration_sources: Vec::new(),
            pac_url: None,
            wpad_enabled: false,
            errors: Vec::new(),
        };
        let snapshot = inspect_configuration(
            &target("example.test", 443, AccessProtocol::Https),
            ProxyProfileKind::WindowsUser,
            configuration,
        );

        assert_eq!(snapshot.decision.proxy.unwrap().host, "secure.test");
    }

    #[cfg(windows)]
    #[test]
    fn windows_inspection_reads_all_client_profiles_without_mutation() {
        let inspection =
            inspect_proxy_profiles(&target("example.test", 443, AccessProtocol::Https));
        let kinds = inspection
            .profiles
            .iter()
            .map(|profile| profile.kind)
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [
                ProxyProfileKind::Environment,
                ProxyProfileKind::WindowsUser,
                ProxyProfileKind::WinHttp,
            ]
        );
    }
}

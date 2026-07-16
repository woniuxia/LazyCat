use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::str::FromStr;

use url::{Host, Url};

use super::model::{ForwardProtocol, RuleWriteInput, ValidatedRuleWriteInput};

pub(crate) fn validate_start_target(rule: &super::model::ForwardRule) -> Result<(), String> {
    let bind_host = parse_bind_host(&rule.bind_host)?;
    let (target_host, target_port) = target_endpoint(rule)?;
    let target_addrs = resolve_target_addrs(&target_host, target_port)?;

    if target_port != rule.listen_port {
        return Ok(());
    }

    let local_addresses = if bind_host.is_unspecified() {
        Some(local_interface_addresses()?)
    } else {
        None
    };
    let targets_listener = target_addrs.iter().any(|target| {
        let target_ip = target.ip();
        if bind_host.is_unspecified() {
            same_address_family(bind_host, target_ip)
                && (target_ip.is_loopback()
                    || local_addresses
                        .as_ref()
                        .is_some_and(|addresses| addresses.contains(&target_ip)))
        } else {
            target_ip == bind_host
        }
    });
    if targets_listener {
        return Err("目标地址与监听地址相同，不能直接转发到自身".into());
    }

    Ok(())
}

fn target_endpoint(rule: &super::model::ForwardRule) -> Result<(String, u16), String> {
    match rule.protocol {
        ForwardProtocol::Http => {
            let target_url = rule
                .target_url
                .as_deref()
                .ok_or_else(|| "HTTP 规则必须配置目标 URL".to_string())?;
            let parsed = Url::parse(target_url)
                .map_err(|error| format!("HTTP 目标 URL 格式不正确: {error}"))?;
            if parsed.scheme() == "https" {
                return Err("当前版本暂不支持 HTTPS 下游".into());
            }
            let host = parsed
                .host_str()
                .ok_or_else(|| "HTTP 目标 URL 必须包含主机名".to_string())?;
            let port = parsed
                .port_or_known_default()
                .ok_or_else(|| "HTTP 目标 URL 缺少有效端口".to_string())?;
            Ok((host.to_string(), port))
        }
        ForwardProtocol::Tcp | ForwardProtocol::Udp => {
            let host = rule
                .target_host
                .as_deref()
                .map(str::trim)
                .filter(|host| !host.is_empty())
                .ok_or_else(|| "TCP/UDP 规则必须配置目标主机".to_string())?;
            let port = rule
                .target_port
                .ok_or_else(|| "TCP/UDP 规则必须配置目标端口".to_string())?;
            Ok((host.to_string(), port))
        }
    }
}

fn resolve_target_addrs(host: &str, port: u16) -> Result<Vec<SocketAddr>, String> {
    let addresses = (host, port)
        .to_socket_addrs()
        .map_err(|error| format!("解析目标地址 {host}:{port} 失败: {error}"))?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err(format!("解析目标地址 {host}:{port} 失败: 未返回任何地址"));
    }
    Ok(addresses)
}

fn local_interface_addresses() -> Result<HashSet<IpAddr>, String> {
    local_ip_address::list_afinet_netifas()
        .map(|interfaces| interfaces.into_iter().map(|(_, address)| address).collect())
        .map_err(|error| format!("枚举本机网络接口地址失败: {error}"))
}

fn same_address_family(left: IpAddr, right: IpAddr) -> bool {
    matches!((left, right), (IpAddr::V4(_), IpAddr::V4(_)) | (IpAddr::V6(_), IpAddr::V6(_)))
}

pub(crate) fn validate_rule_input(
    mut input: RuleWriteInput,
) -> Result<ValidatedRuleWriteInput, String> {
    input.name = input.name.trim().to_string();
    if input.name.is_empty() {
        return Err("规则名称不能为空".into());
    }
    let bind_host = parse_bind_host(&input.bind_host)?;
    validate_port(input.listen_port, "监听端口")?;

    match input.protocol {
        ForwardProtocol::Http => validate_http_input(input, bind_host),
        ForwardProtocol::Tcp | ForwardProtocol::Udp => validate_socket_input(input, bind_host),
    }
}

fn parse_bind_host(value: &str) -> Result<IpAddr, String> {
    let normalized = value.trim();
    IpAddr::from_str(normalized).map_err(|_| "监听地址必须是 IPv4 或 IPv6 字面量".to_string())
}

fn validate_port(port: u16, field_name: &str) -> Result<(), String> {
    if port == 0 {
        return Err(format!("{field_name}必须在 1 到 65535 之间"));
    }
    Ok(())
}

fn validate_http_input(
    input: RuleWriteInput,
    bind_host: IpAddr,
) -> Result<ValidatedRuleWriteInput, String> {
    if input.target_host.is_some() || input.target_port.is_some() {
        return Err("HTTP 规则只能配置目标 URL，不能配置目标主机或端口".into());
    }

    let target_url = input
        .target_url
        .as_deref()
        .ok_or_else(|| "HTTP 规则必须配置目标 URL".to_string())?;
    let target_url = normalize_http_target_url(target_url, bind_host, input.listen_port)?;

    Ok(ValidatedRuleWriteInput {
        name: input.name,
        protocol: input.protocol,
        bind_host: bind_host.to_string(),
        listen_port: input.listen_port,
        target_url: Some(target_url),
        target_host: None,
        target_port: None,
        capture_http_headers: input.capture_http_headers,
        capture_http_body: input.capture_http_body,
    })
}

fn normalize_http_target_url(
    value: &str,
    bind_host: IpAddr,
    listen_port: u16,
) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("HTTP 规则必须配置目标 URL".into());
    }

    let parsed = Url::parse(value).map_err(|_| "HTTP 目标 URL 格式不正确".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("HTTP 目标 URL 仅支持 http 或 https".into());
    }
    if parsed.host().is_none() {
        return Err("HTTP 目标 URL 必须包含主机名".into());
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err("HTTP 目标 URL 不能包含 query 或 fragment".into());
    }
    if let Some(port) = parsed.port() {
        validate_port(port, "HTTP 目标端口")?;
    }
    if url_ip_host(&parsed) == Some(bind_host)
        && parsed.port_or_known_default() == Some(listen_port)
    {
        return Err("目标地址与监听地址相同，不能直接转发到自身".into());
    }

    Ok(parsed.as_str().trim_end_matches('/').to_string())
}

fn url_ip_host(url: &Url) -> Option<IpAddr> {
    match url.host()? {
        Host::Ipv4(host) => Some(IpAddr::V4(host)),
        Host::Ipv6(host) => Some(IpAddr::V6(host)),
        Host::Domain(_) => None,
    }
}

fn validate_socket_input(
    input: RuleWriteInput,
    bind_host: IpAddr,
) -> Result<ValidatedRuleWriteInput, String> {
    if input.target_url.is_some() {
        return Err("TCP/UDP 规则不能配置目标 URL".into());
    }

    let target_host = input
        .target_host
        .as_deref()
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .ok_or_else(|| "TCP/UDP 规则必须配置目标主机".to_string())?
        .to_string();
    let target_port = input
        .target_port
        .ok_or_else(|| "TCP/UDP 规则必须配置目标端口".to_string())?;
    validate_port(target_port, "目标端口")?;

    if target_host.parse::<IpAddr>().ok() == Some(bind_host) && target_port == input.listen_port {
        return Err("目标地址与监听地址相同，不能直接转发到自身".into());
    }

    Ok(ValidatedRuleWriteInput {
        name: input.name,
        protocol: input.protocol,
        bind_host: bind_host.to_string(),
        listen_port: input.listen_port,
        target_url: None,
        target_host: Some(target_host),
        target_port: Some(target_port),
        capture_http_headers: input.capture_http_headers,
        capture_http_body: input.capture_http_body,
    })
}

#[cfg(test)]
mod tests {
    use super::validate_start_target;
    use crate::tools::request_forward::model::{ForwardProtocol, ForwardRule};

    fn socket_rule(protocol: ForwardProtocol, bind_host: &str, target_host: &str) -> ForwardRule {
        ForwardRule {
            id: 1,
            name: "自转发检查".into(),
            protocol,
            bind_host: bind_host.into(),
            listen_port: 18_080,
            target_url: None,
            target_host: Some(target_host.into()),
            target_port: Some(18_080),
            capture_http_headers: false,
            capture_http_body: false,
            auto_start: false,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn wildcard_ipv4_rejects_loopback_target_on_same_port() {
        let rule = socket_rule(ForwardProtocol::Tcp, "0.0.0.0", "127.0.0.1");

        let error = validate_start_target(&rule).expect_err("wildcard listener covers loopback");

        assert!(error.contains("不能直接转发到自身"));
    }

    #[test]
    fn wildcard_ipv6_rejects_loopback_target_on_same_port() {
        let rule = socket_rule(ForwardProtocol::Udp, "::", "::1");

        let error = validate_start_target(&rule).expect_err("wildcard listener covers loopback");

        assert!(error.contains("不能直接转发到自身"));
    }

    #[test]
    fn specific_listener_rejects_hostname_resolving_to_itself() {
        let rule = ForwardRule {
            protocol: ForwardProtocol::Http,
            bind_host: "127.0.0.1".into(),
            target_url: Some("http://localhost:18080/api".into()),
            target_host: None,
            target_port: None,
            ..socket_rule(ForwardProtocol::Http, "127.0.0.1", "unused")
        };

        let error = validate_start_target(&rule).expect_err("localhost resolves to listener");

        assert!(error.contains("不能直接转发到自身"));
    }

    #[test]
    fn same_target_address_on_different_port_is_allowed() {
        let mut rule = socket_rule(ForwardProtocol::Tcp, "0.0.0.0", "127.0.0.1");
        rule.target_port = Some(18_081);

        validate_start_target(&rule).expect("different port cannot loop into this listener");
    }

    #[test]
    fn wildcard_listener_allows_remote_target_on_same_port() {
        let rule = socket_rule(ForwardProtocol::Tcp, "0.0.0.0", "192.0.2.1");

        validate_start_target(&rule).expect("wildcard must not reject every remote address");
    }
}

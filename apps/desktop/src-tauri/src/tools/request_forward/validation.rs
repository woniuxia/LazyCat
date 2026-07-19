use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::str::FromStr;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use url::{Host, Url};

use super::model::{ForwardProtocol, RuleWriteInput, ValidatedRuleWriteInput};

const TARGET_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) fn validate_start_target(rule: &super::model::ForwardRule) -> Result<(), String> {
    validate_start_target_with_resolver(rule, &|host, port| {
        resolve_target_addrs_bounded(host.to_string(), port)
    })
}

fn validate_start_target_with_resolver(
    rule: &super::model::ForwardRule,
    resolver: &impl Fn(&str, u16) -> Result<Vec<SocketAddr>, String>,
) -> Result<(), String> {
    let (target_host, target_port) = target_endpoint(rule)?;
    if target_port != rule.listen_port {
        return Ok(());
    }

    let bind_host = parse_bind_host(&rule.bind_host)?;
    let target_addrs = resolver(&target_host, target_port)?;
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

pub(crate) fn resolve_target_addrs_bounded(
    host: String,
    port: u16,
) -> Result<Vec<SocketAddr>, String> {
    resolve_target_addrs_bounded_with(
        host,
        port,
        TARGET_RESOLUTION_TIMEOUT,
        |host, port| resolve_target_addrs(&host, port),
    )
}

fn resolve_target_addrs_bounded_with(
    host: String,
    port: u16,
    timeout: Duration,
    resolver: impl FnOnce(String, u16) -> Result<Vec<SocketAddr>, String> + Send + 'static,
) -> Result<Vec<SocketAddr>, String> {
    let display_host = host.clone();
    let (result_tx, result_rx) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("request-forward-dns".into())
        .spawn(move || {
            let _ = result_tx.send(resolver(host, port));
        })
        .map_err(|error| format!("无法启动目标地址解析线程: {error}"))?;

    match result_rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            // getaddrinfo cannot be cancelled portably. Dropping the JoinHandle bounds the
            // manager/rule-lock wait; the detached resolver may remain until the OS call returns.
            Err(format!(
                "解析目标地址 {display_host}:{port} 超时（{} ms）",
                timeout.as_millis()
            ))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(format!("解析目标地址 {display_host}:{port} 失败: 解析线程异常退出"))
        }
    }
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
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use super::{
        resolve_target_addrs_bounded_with, validate_start_target,
        validate_start_target_with_resolver,
    };
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

    #[test]
    fn different_ports_skip_resolution_for_http_tcp_and_udp() {
        let calls = AtomicUsize::new(0);
        let resolver = |_: &str, _: u16| -> Result<Vec<SocketAddr>, String> {
            calls.fetch_add(1, Ordering::SeqCst);
            Err("resolver must not be called".into())
        };
        let http_rule = ForwardRule {
            protocol: ForwardProtocol::Http,
            target_url: Some("https://different-port.invalid:18081/api".into()),
            target_host: None,
            target_port: None,
            ..socket_rule(ForwardProtocol::Http, "127.0.0.1", "unused")
        };
        let mut tcp_rule = socket_rule(ForwardProtocol::Tcp, "127.0.0.1", "different-port.invalid");
        tcp_rule.target_port = Some(18_081);
        let mut udp_rule = socket_rule(ForwardProtocol::Udp, "127.0.0.1", "different-port.invalid");
        udp_rule.target_port = Some(18_081);

        for rule in [&http_rule, &tcp_rule, &udp_rule] {
            validate_start_target_with_resolver(rule, &resolver)
                .expect("different port does not require DNS self-forward checking");
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn same_port_resolution_times_out_within_the_bound() {
        let (release_tx, release_rx) = mpsc::channel();
        let started = Instant::now();

        let error = resolve_target_addrs_bounded_with(
            "slow.invalid".into(),
            18_080,
            Duration::from_millis(30),
            move |_, _| {
                let _ = release_rx.recv();
                Ok(vec!["127.0.0.1:18080".parse().expect("test address")])
            },
        )
        .expect_err("same-port resolution must time out");
        let elapsed = started.elapsed();
        release_tx.send(()).expect("release detached resolver");

        assert!(error.contains("解析目标地址 slow.invalid:18080 超时"));
        assert!(elapsed < Duration::from_millis(500), "elapsed: {elapsed:?}");
    }
}

use std::net::IpAddr;
use std::str::FromStr;

use url::Url;

use super::model::{ForwardProtocol, RuleWriteInput, ValidatedRuleWriteInput};

pub(crate) fn validate_rule_input(
    input: RuleWriteInput,
) -> Result<ValidatedRuleWriteInput, String> {
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
    let target_url = normalize_http_target_url(target_url)?;

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

fn normalize_http_target_url(value: &str) -> Result<String, String> {
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

    Ok(parsed.as_str().trim_end_matches('/').to_string())
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

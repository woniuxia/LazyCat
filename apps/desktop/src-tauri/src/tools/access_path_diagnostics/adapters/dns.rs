use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::time::{Duration, Instant};

use hickory_resolver::config::{
    ConnectionConfig, NameServerConfig, ResolveHosts, ResolverConfig, ResolverOpts,
};
use hickory_resolver::net::runtime::TokioRuntimeProvider;
use hickory_resolver::net::{DnsError as HickoryDnsError, NetError};
use hickory_resolver::proto::op::ResponseCode;
use hickory_resolver::proto::rr::{RData, RecordType};
use hickory_resolver::proto::ProtoError;
use hickory_resolver::system_conf::read_system_conf;
use hickory_resolver::TokioResolver;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use super::super::model::{AccessPathTargetKind, NormalizedAccessPathTarget, StepOutcome};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsQueryOutcome {
    Success,
    NoRecords,
    Failed,
    Cancelled,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsErrorCode {
    NoRecords,
    Nxdomain,
    Servfail,
    Refused,
    Timeout,
    TransportError,
    MalformedResponse,
    InvalidServer,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsError {
    pub code: DnsErrorCode,
    pub message: String,
    pub raw_error: String,
    pub retriable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negative_ttl: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsRecordType {
    A,
    Aaaa,
    Cname,
    Ptr,
}

impl DnsRecordType {
    fn hickory_type(self) -> RecordType {
        match self {
            Self::A => RecordType::A,
            Self::Aaaa => RecordType::AAAA,
            Self::Cname => RecordType::CNAME,
            Self::Ptr => RecordType::PTR,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsRecord {
    pub name: String,
    pub value: String,
    pub ttl: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsQueryResult {
    pub record_type: DnsRecordType,
    pub outcome: DnsQueryOutcome,
    pub elapsed_ms: u64,
    #[serde(default)]
    pub records: Vec<DnsRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<DnsError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemResolverInfo {
    #[serde(default)]
    pub name_servers: Vec<String>,
    #[serde(default)]
    pub search_suffixes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<DnsError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemEffectiveResolution {
    pub query_name: String,
    pub outcome: DnsQueryOutcome,
    pub elapsed_ms: u64,
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<DnsError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BypassDnsResult {
    pub requested_server: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub queries: Vec<DnsQueryResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<DnsError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsCapabilityId {
    WindowsDnsCache,
    Nrpt,
    Doh,
    CandidateDomainExpansion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsCapabilityStatus {
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsCapability {
    pub id: DnsCapabilityId,
    pub status: DnsCapabilityStatus,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsDiagnosticResult {
    pub outcome_hint: StepOutcome,
    pub system_resolver: SystemResolverInfo,
    pub system: SystemEffectiveResolution,
    #[serde(default)]
    pub bypass: Vec<BypassDnsResult>,
    pub capabilities: Vec<DnsCapability>,
}

impl DnsDiagnosticResult {
    pub fn derive_outcome_hint(&self) -> StepOutcome {
        derive_outcome_hint(&self.system, &self.bypass)
    }
}

pub async fn diagnose_dns(
    target: &NormalizedAccessPathTarget,
    dns_servers: &[String],
    query_timeout: Duration,
    cancellation: CancellationToken,
) -> DnsDiagnosticResult {
    let timeout = query_timeout.max(Duration::from_millis(1));
    let system_resolver = read_system_resolver_info();
    let system = resolve_system_effective(target, timeout, cancellation.clone()).await;

    let mut tasks = tokio::task::JoinSet::new();
    for (index, server) in dns_servers.iter().enumerate() {
        let server = server.clone();
        let target = target.clone();
        let cancellation = cancellation.clone();
        tasks.spawn(async move {
            (
                index,
                query_bypass_server(&target, server, timeout, cancellation).await,
            )
        });
    }

    let mut indexed_results = Vec::with_capacity(dns_servers.len());
    while let Some(result) = tasks.join_next().await {
        if let Ok(result) = result {
            indexed_results.push(result);
        }
    }
    indexed_results.sort_by_key(|(index, _)| *index);
    let bypass = indexed_results
        .into_iter()
        .map(|(_, result)| result)
        .collect::<Vec<_>>();
    let outcome_hint = derive_outcome_hint(&system, &bypass);

    DnsDiagnosticResult {
        outcome_hint,
        system_resolver,
        system,
        bypass,
        capabilities: unsupported_capabilities(),
    }
}

fn read_system_resolver_info() -> SystemResolverInfo {
    match read_system_conf() {
        Ok((config, _)) => {
            let mut seen_servers = HashSet::new();
            let name_servers = config
                .name_servers()
                .iter()
                .flat_map(|server| {
                    server.connections.iter().map(move |connection| {
                        SocketAddr::new(server.ip, connection.port).to_string()
                    })
                })
                .filter(|server| seen_servers.insert(server.clone()))
                .collect();
            let mut search_suffixes = config
                .search()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            if let Some(domain) = config.domain() {
                let domain = domain.to_string();
                if !search_suffixes.contains(&domain) {
                    search_suffixes.insert(0, domain);
                }
            }
            SystemResolverInfo {
                name_servers,
                search_suffixes,
                error: None,
            }
        }
        Err(error) => SystemResolverInfo {
            name_servers: Vec::new(),
            search_suffixes: Vec::new(),
            error: Some(new_error(
                DnsErrorCode::TransportError,
                "无法读取系统 DNS 配置",
                error.to_string(),
                false,
            )),
        },
    }
}

async fn resolve_system_effective(
    target: &NormalizedAccessPathTarget,
    timeout: Duration,
    cancellation: CancellationToken,
) -> SystemEffectiveResolution {
    if target.target_kind != AccessPathTargetKind::Hostname {
        return SystemEffectiveResolution {
            query_name: target.hostname.clone(),
            outcome: DnsQueryOutcome::NotApplicable,
            elapsed_ms: 0,
            addresses: vec![target.hostname.clone()],
            error: None,
        };
    }

    let started = Instant::now();
    let lookup = tokio::net::lookup_host((target.hostname.as_str(), target.port));
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => SystemEffectiveResolution {
            query_name: target.hostname.clone(),
            outcome: DnsQueryOutcome::Cancelled,
            elapsed_ms: elapsed_ms(started),
            addresses: Vec::new(),
            error: Some(cancelled_error()),
        },
        result = tokio::time::timeout(timeout, lookup) => {
            match result {
                Err(_) => SystemEffectiveResolution {
                    query_name: target.hostname.clone(),
                    outcome: DnsQueryOutcome::Failed,
                    elapsed_ms: elapsed_ms(started),
                    addresses: Vec::new(),
                    error: Some(timeout_error("系统有效解析超时")),
                },
                Ok(Err(error)) => {
                    let classified = classify_system_error(&error);
                    SystemEffectiveResolution {
                        query_name: target.hostname.clone(),
                        outcome: outcome_for_error(classified.code),
                        elapsed_ms: elapsed_ms(started),
                        addresses: Vec::new(),
                        error: Some(classified),
                    }
                }
                Ok(Ok(addresses)) => {
                    let mut seen = HashSet::new();
                    let addresses = addresses
                        .map(|address| address.ip().to_string())
                        .filter(|address| seen.insert(address.clone()))
                        .collect::<Vec<_>>();
                    if addresses.is_empty() {
                        let error = no_records_error("系统有效解析未返回地址");
                        SystemEffectiveResolution {
                            query_name: target.hostname.clone(),
                            outcome: DnsQueryOutcome::NoRecords,
                            elapsed_ms: elapsed_ms(started),
                            addresses,
                            error: Some(error),
                        }
                    } else {
                        SystemEffectiveResolution {
                            query_name: target.hostname.clone(),
                            outcome: DnsQueryOutcome::Success,
                            elapsed_ms: elapsed_ms(started),
                            addresses,
                            error: None,
                        }
                    }
                }
            }
        }
    }
}

async fn query_bypass_server(
    target: &NormalizedAccessPathTarget,
    requested_server: String,
    timeout: Duration,
    cancellation: CancellationToken,
) -> BypassDnsResult {
    let endpoint = match parse_dns_server(&requested_server) {
        Ok(endpoint) => endpoint,
        Err(error) => {
            return BypassDnsResult {
                requested_server,
                endpoint: None,
                queries: Vec::new(),
                error: Some(error),
            };
        }
    };

    if cancellation.is_cancelled() {
        return BypassDnsResult {
            requested_server,
            endpoint: Some(endpoint.to_string()),
            queries: Vec::new(),
            error: Some(cancelled_error()),
        };
    }

    let resolver = match resolver_for_endpoint(endpoint, timeout) {
        Ok(resolver) => resolver,
        Err(error) => {
            return BypassDnsResult {
                requested_server,
                endpoint: Some(endpoint.to_string()),
                queries: Vec::new(),
                error: Some(error),
            };
        }
    };
    let queries = if target.target_kind == AccessPathTargetKind::Hostname {
        let name = absolute_name(&target.hostname);
        let (a, aaaa, cname) = tokio::join!(
            query_record(
                &resolver,
                &name,
                DnsRecordType::A,
                timeout,
                cancellation.clone()
            ),
            query_record(
                &resolver,
                &name,
                DnsRecordType::Aaaa,
                timeout,
                cancellation.clone()
            ),
            query_record(
                &resolver,
                &name,
                DnsRecordType::Cname,
                timeout,
                cancellation.clone()
            ),
        );
        vec![a, aaaa, cname]
    } else {
        let ip = IpAddr::from_str(&target.hostname).expect("normalized IP target must be valid");
        vec![
            query_record(
                &resolver,
                &ptr_name(ip),
                DnsRecordType::Ptr,
                timeout,
                cancellation,
            )
            .await,
        ]
    };

    BypassDnsResult {
        requested_server,
        endpoint: Some(endpoint.to_string()),
        queries,
        error: None,
    }
}

fn resolver_for_endpoint(
    endpoint: SocketAddr,
    timeout: Duration,
) -> Result<TokioResolver, DnsError> {
    let mut udp = ConnectionConfig::udp();
    udp.port = endpoint.port();
    let mut tcp = ConnectionConfig::tcp();
    tcp.port = endpoint.port();
    let name_server = NameServerConfig::new(endpoint.ip(), true, vec![udp, tcp]);
    let config = ResolverConfig::from_parts(None, Vec::new(), vec![name_server]);
    let mut options = ResolverOpts::default();
    options.timeout = timeout;
    options.attempts = 1;
    options.use_hosts_file = ResolveHosts::Never;
    options.try_tcp_on_error = true;
    options.preserve_intermediates = true;
    TokioResolver::builder_with_config(config, TokioRuntimeProvider::default())
        .with_options(options)
        .build()
        .map_err(|error| classify_resolve_error(&error))
}

async fn query_record(
    resolver: &TokioResolver,
    name: &str,
    record_type: DnsRecordType,
    timeout: Duration,
    cancellation: CancellationToken,
) -> DnsQueryResult {
    let started = Instant::now();
    let lookup = resolver.lookup(name, record_type.hickory_type());
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => DnsQueryResult {
            record_type,
            outcome: DnsQueryOutcome::Cancelled,
            elapsed_ms: elapsed_ms(started),
            records: Vec::new(),
            error: Some(cancelled_error()),
        },
        result = tokio::time::timeout(timeout, lookup) => {
            match result {
                Err(_) => DnsQueryResult {
                    record_type,
                    outcome: DnsQueryOutcome::Failed,
                    elapsed_ms: elapsed_ms(started),
                    records: Vec::new(),
                    error: Some(timeout_error("旁路 DNS 查询超时")),
                },
                Ok(Err(error)) => {
                    let classified = classify_resolve_error(&error);
                    DnsQueryResult {
                        record_type,
                        outcome: outcome_for_error(classified.code),
                        elapsed_ms: elapsed_ms(started),
                        records: Vec::new(),
                        error: Some(classified),
                    }
                }
                Ok(Ok(lookup)) => {
                    let records = lookup.answers().iter().filter_map(|record| {
                        let value = match (record_type, &record.data) {
                            (DnsRecordType::A, RData::A(value)) => value.0.to_string(),
                            (DnsRecordType::Aaaa, RData::AAAA(value)) => value.0.to_string(),
                            (DnsRecordType::Cname, RData::CNAME(value)) => value.0.to_string(),
                            (DnsRecordType::Ptr, RData::PTR(value)) => value.0.to_string(),
                            _ => return None,
                        };
                        Some(DnsRecord {
                            name: record.name.to_string(),
                            value,
                            ttl: record.ttl,
                        })
                    }).collect::<Vec<_>>();
                    if records.is_empty() {
                        DnsQueryResult {
                            record_type,
                            outcome: DnsQueryOutcome::NoRecords,
                            elapsed_ms: elapsed_ms(started),
                            records,
                            error: Some(no_records_error("DNS 响应未包含请求类型的记录")),
                        }
                    } else {
                        DnsQueryResult {
                            record_type,
                            outcome: DnsQueryOutcome::Success,
                            elapsed_ms: elapsed_ms(started),
                            records,
                            error: None,
                        }
                    }
                }
            }
        }
    }
}

fn parse_dns_server(value: &str) -> Result<SocketAddr, DnsError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(invalid_server_error(value, "DNS 服务器不能为空"));
    }
    if let Ok(ip) = IpAddr::from_str(value) {
        return Ok(SocketAddr::new(ip, 53));
    }
    if value.starts_with('[') && value.ends_with(']') {
        if let Ok(ip) = IpAddr::from_str(&value[1..value.len() - 1]) {
            return Ok(SocketAddr::new(ip, 53));
        }
    }
    SocketAddr::from_str(value)
        .map_err(|error| invalid_server_error(value, format!("无效的 DNS 服务器地址: {error}")))
}

fn classify_resolve_error(error: &NetError) -> DnsError {
    let raw_error = error.to_string();
    match error {
        NetError::Dns(HickoryDnsError::NoRecordsFound(no_records)) => {
            classify_response_code(no_records.response_code, no_records.negative_ttl, raw_error)
        }
        NetError::Dns(HickoryDnsError::ResponseCode(response_code)) => {
            classify_response_code(*response_code, None, raw_error)
        }
        NetError::Timeout => timeout_error_with_raw("DNS 查询超时", raw_error),
        NetError::Io(error) if error.kind() == std::io::ErrorKind::TimedOut => {
            timeout_error_with_raw("DNS 传输超时", raw_error)
        }
        NetError::Proto(
            ProtoError::CharacterDataTooLong { .. }
            | ProtoError::Decode(_)
            | ProtoError::FormError { .. }
            | ProtoError::MaxBufferSizeExceeded(_)
            | ProtoError::NotAResponse,
        ) => new_error(
            DnsErrorCode::MalformedResponse,
            "DNS 响应格式错误",
            raw_error,
            false,
        ),
        NetError::Proto(_) => new_error(
            DnsErrorCode::TransportError,
            "DNS 协议传输失败",
            raw_error,
            true,
        ),
        _ => new_error(
            DnsErrorCode::TransportError,
            "DNS 查询传输失败",
            raw_error,
            true,
        ),
    }
}

fn classify_response_code(
    response_code: ResponseCode,
    negative_ttl: Option<u32>,
    raw_error: String,
) -> DnsError {
    let (code, message, retriable) = match response_code {
        ResponseCode::NoError => (DnsErrorCode::NoRecords, "DNS 名称存在但无此类型记录", false),
        ResponseCode::NXDomain => (DnsErrorCode::Nxdomain, "DNS 名称不存在", false),
        ResponseCode::ServFail => (DnsErrorCode::Servfail, "DNS 服务器处理查询失败", true),
        ResponseCode::Refused => (DnsErrorCode::Refused, "DNS 服务器拒绝查询", false),
        ResponseCode::FormErr => (
            DnsErrorCode::MalformedResponse,
            "DNS 服务器报告请求格式错误",
            false,
        ),
        _ => (
            DnsErrorCode::TransportError,
            "DNS 服务器返回未支持的响应状态",
            false,
        ),
    };
    DnsError {
        code,
        message: message.to_string(),
        raw_error,
        retriable,
        response_code: Some(format!("{response_code:?}")),
        negative_ttl,
    }
}

fn classify_system_error(error: &std::io::Error) -> DnsError {
    let raw_error = error.to_string();
    let code = match error.raw_os_error() {
        Some(11001) => DnsErrorCode::Nxdomain,
        Some(11002) => DnsErrorCode::Servfail,
        Some(11004) => DnsErrorCode::NoRecords,
        Some(10060) => DnsErrorCode::Timeout,
        _ if error.kind() == std::io::ErrorKind::TimedOut => DnsErrorCode::Timeout,
        _ => DnsErrorCode::TransportError,
    };
    let (message, retriable) = match code {
        DnsErrorCode::Nxdomain => ("系统有效解析未找到该名称", false),
        DnsErrorCode::Servfail => ("系统解析器暂时无法完成查询", true),
        DnsErrorCode::NoRecords => ("系统有效解析未返回地址", false),
        DnsErrorCode::Timeout => ("系统有效解析超时", true),
        _ => ("系统有效解析失败", true),
    };
    new_error(code, message, raw_error, retriable)
}

fn derive_outcome_hint(
    system: &SystemEffectiveResolution,
    bypass: &[BypassDnsResult],
) -> StepOutcome {
    match system.outcome {
        DnsQueryOutcome::Failed | DnsQueryOutcome::NoRecords => return StepOutcome::Failed,
        DnsQueryOutcome::Cancelled => return StepOutcome::Unverified,
        DnsQueryOutcome::Success | DnsQueryOutcome::NotApplicable => {}
    }

    let has_bypass_problem = bypass.iter().any(bypass_has_problem);
    if has_bypass_problem {
        StepOutcome::Warning
    } else if system.outcome == DnsQueryOutcome::NotApplicable && bypass.is_empty() {
        StepOutcome::Unverified
    } else {
        StepOutcome::Success
    }
}

fn bypass_has_problem(server: &BypassDnsResult) -> bool {
    if server.error.is_some()
        || server.queries.iter().any(|query| {
            matches!(
                query.outcome,
                DnsQueryOutcome::Failed | DnsQueryOutcome::Cancelled
            )
        })
    {
        return true;
    }
    let address_queries = server
        .queries
        .iter()
        .filter(|query| matches!(query.record_type, DnsRecordType::A | DnsRecordType::Aaaa))
        .collect::<Vec<_>>();
    if !address_queries.is_empty()
        && !address_queries
            .iter()
            .any(|query| query.outcome == DnsQueryOutcome::Success)
    {
        return true;
    }
    server.queries.iter().any(|query| {
        query.record_type == DnsRecordType::Ptr && query.outcome == DnsQueryOutcome::NoRecords
    })
}

fn unsupported_capabilities() -> Vec<DnsCapability> {
    [
        (
            DnsCapabilityId::WindowsDnsCache,
            "首期不读取 Windows DNS 缓存，系统有效解析仍可能使用该缓存",
        ),
        (
            DnsCapabilityId::Nrpt,
            "首期不读取 NRPT 规则，系统有效解析与旁路查询可能因此不同",
        ),
        (
            DnsCapabilityId::Doh,
            "首期不检测 Windows DoH 配置，旁路查询仅使用明文 DNS",
        ),
        (
            DnsCapabilityId::CandidateDomainExpansion,
            "首期不扩展 DNS 搜索后缀或候选域名，仅诊断用户输入的规范化主机名",
        ),
    ]
    .into_iter()
    .map(|(id, reason)| DnsCapability {
        id,
        status: DnsCapabilityStatus::Unsupported,
        reason: reason.to_string(),
    })
    .collect()
}

fn outcome_for_error(code: DnsErrorCode) -> DnsQueryOutcome {
    match code {
        DnsErrorCode::NoRecords => DnsQueryOutcome::NoRecords,
        DnsErrorCode::Cancelled => DnsQueryOutcome::Cancelled,
        _ => DnsQueryOutcome::Failed,
    }
}

fn absolute_name(hostname: &str) -> String {
    format!("{}.", hostname.trim_end_matches('.'))
}

fn ptr_name(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(address) => {
            let octets = address.octets();
            format!(
                "{}.{}.{}.{}.in-addr.arpa.",
                octets[3], octets[2], octets[1], octets[0]
            )
        }
        IpAddr::V6(address) => {
            let nibbles = address
                .octets()
                .into_iter()
                .flat_map(|byte| [byte >> 4, byte & 0x0f])
                .rev()
                .map(|nibble| format!("{nibble:x}"))
                .collect::<Vec<_>>()
                .join(".");
            format!("{nibbles}.ip6.arpa.")
        }
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn new_error(
    code: DnsErrorCode,
    message: impl Into<String>,
    raw_error: impl Into<String>,
    retriable: bool,
) -> DnsError {
    DnsError {
        code,
        message: message.into(),
        raw_error: raw_error.into(),
        retriable,
        response_code: None,
        negative_ttl: None,
    }
}

fn invalid_server_error(value: &str, message: impl Into<String>) -> DnsError {
    new_error(
        DnsErrorCode::InvalidServer,
        message,
        format!("invalid DNS server: {value}"),
        false,
    )
}

fn timeout_error(message: &str) -> DnsError {
    timeout_error_with_raw(message, "operation timed out".to_string())
}

fn timeout_error_with_raw(message: &str, raw_error: String) -> DnsError {
    new_error(DnsErrorCode::Timeout, message, raw_error, true)
}

fn no_records_error(message: &str) -> DnsError {
    new_error(
        DnsErrorCode::NoRecords,
        message,
        "no records returned",
        false,
    )
}

fn cancelled_error() -> DnsError {
    new_error(
        DnsErrorCode::Cancelled,
        "DNS 查询已取消",
        "operation cancelled",
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::access_path_diagnostics::model::{AccessPathTargetKind, AccessProtocol};
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, UdpSocket};
    use tokio::task::JoinHandle;

    fn target(hostname: &str, target_kind: AccessPathTargetKind) -> NormalizedAccessPathTarget {
        NormalizedAccessPathTarget {
            raw_input: hostname.to_string(),
            protocol: AccessProtocol::Https,
            hostname: hostname.to_string(),
            target_kind,
            port: 443,
            path: "/".to_string(),
            url: format!("https://{hostname}/"),
            sni: None,
            verify_hostname: Some(hostname.to_string()),
            http_host: hostname.to_string(),
            connection_ip: None,
        }
    }

    fn dns_response(query: &[u8], response_code: u8) -> Vec<u8> {
        let mut response = query.to_vec();
        if response.len() < 12 {
            return response;
        }
        response[2] |= 0x80;
        response[3] = 0x80 | (response_code & 0x0f);
        response[6..8].copy_from_slice(&0u16.to_be_bytes());
        response[8..10].copy_from_slice(&0u16.to_be_bytes());
        response[10..12].copy_from_slice(&0u16.to_be_bytes());
        let mut offset = 12usize;
        while offset < response.len() && response[offset] != 0 {
            offset = offset.saturating_add(1 + response[offset] as usize);
        }
        if offset < response.len() {
            offset = (offset + 5).min(response.len());
            response.truncate(offset);
        }
        response
    }

    async fn spawn_dns_fixture(response_code: u8) -> (SocketAddr, JoinHandle<()>) {
        let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let address = udp.local_addr().unwrap();
        let tcp = TcpListener::bind(address).await.unwrap();
        let task = tokio::spawn(async move {
            loop {
                let mut udp_query = [0u8; 2048];
                tokio::select! {
                    received = udp.recv_from(&mut udp_query) => {
                        let Ok((size, peer)) = received else { break; };
                        let response = dns_response(&udp_query[..size], response_code);
                        if udp.send_to(&response, peer).await.is_err() { break; }
                    }
                    accepted = tcp.accept() => {
                        let Ok((mut stream, _)) = accepted else { break; };
                        let mut length = [0u8; 2];
                        if stream.read_exact(&mut length).await.is_err() { continue; }
                        let mut query = vec![0u8; u16::from_be_bytes(length) as usize];
                        if stream.read_exact(&mut query).await.is_err() { continue; }
                        let response = dns_response(&query, response_code);
                        let response_length = (response.len() as u16).to_be_bytes();
                        if stream.write_all(&response_length).await.is_err() { continue; }
                        let _ = stream.write_all(&response).await;
                    }
                }
            }
        });
        (address, task)
    }
    #[test]
    fn parses_ipv4_ipv6_and_explicit_ports_without_splitting_ipv6() {
        assert_eq!(
            parse_dns_server("10.0.0.53").unwrap(),
            "10.0.0.53:53".parse().unwrap()
        );
        assert_eq!(
            parse_dns_server("10.0.0.53:5353").unwrap(),
            "10.0.0.53:5353".parse().unwrap()
        );
        assert_eq!(
            parse_dns_server("2001:db8::53").unwrap(),
            "[2001:db8::53]:53".parse().unwrap()
        );
        assert_eq!(
            parse_dns_server("[2001:db8::53]:5353").unwrap(),
            "[2001:db8::53]:5353".parse().unwrap()
        );
        assert_eq!(
            parse_dns_server("[2001:db8::53]").unwrap(),
            "[2001:db8::53]:53".parse().unwrap()
        );
    }

    #[test]
    fn response_codes_have_distinct_error_categories() {
        let cases = [
            (ResponseCode::NoError, DnsErrorCode::NoRecords),
            (ResponseCode::NXDomain, DnsErrorCode::Nxdomain),
            (ResponseCode::ServFail, DnsErrorCode::Servfail),
            (ResponseCode::Refused, DnsErrorCode::Refused),
            (ResponseCode::FormErr, DnsErrorCode::MalformedResponse),
        ];
        for (response_code, expected) in cases {
            let error = classify_response_code(response_code, Some(30), "raw".to_string());
            assert_eq!(error.code, expected);
            assert_eq!(error.negative_ttl, Some(30));
            assert_eq!(error.raw_error, "raw");
        }
    }

    #[test]
    fn optional_aaaa_and_cname_no_records_do_not_warn_when_a_succeeds() {
        let query = |record_type, outcome| DnsQueryResult {
            record_type,
            outcome,
            elapsed_ms: 1,
            records: Vec::new(),
            error: None,
        };
        let bypass = BypassDnsResult {
            requested_server: "192.0.2.53".into(),
            endpoint: Some("192.0.2.53:53".into()),
            queries: vec![
                query(DnsRecordType::A, DnsQueryOutcome::Success),
                query(DnsRecordType::Aaaa, DnsQueryOutcome::NoRecords),
                query(DnsRecordType::Cname, DnsQueryOutcome::NoRecords),
            ],
            error: None,
        };
        assert!(!bypass_has_problem(&bypass));
    }

    #[test]
    fn builds_ipv4_and_ipv6_ptr_names() {
        assert_eq!(
            ptr_name("8.8.4.4".parse().unwrap()),
            "4.4.8.8.in-addr.arpa."
        );
        assert_eq!(
            ptr_name("2001:db8::1".parse().unwrap()),
            "1.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.8.b.d.0.1.0.0.2.ip6.arpa."
        );
    }

    #[tokio::test]
    async fn cancelled_bypass_query_finishes_without_network_access() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let result = diagnose_dns(
            &target("example.test", AccessPathTargetKind::Hostname),
            &["192.0.2.53".to_string()],
            Duration::from_secs(30),
            cancellation,
        )
        .await;
        assert_eq!(result.system.outcome, DnsQueryOutcome::Cancelled);
        assert_eq!(
            result.bypass[0].error.as_ref().unwrap().code,
            DnsErrorCode::Cancelled
        );
        assert_eq!(result.outcome_hint, StepOutcome::Unverified);
    }

    #[tokio::test]
    async fn invalid_server_is_structured_and_does_not_hide_valid_entries() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let result = diagnose_dns(
            &target("203.0.113.10", AccessPathTargetKind::Ipv4),
            &["not-a-server".to_string(), "192.0.2.53".to_string()],
            Duration::from_secs(30),
            cancellation,
        )
        .await;
        assert_eq!(result.bypass.len(), 2);
        assert_eq!(
            result.bypass[0].error.as_ref().unwrap().code,
            DnsErrorCode::InvalidServer
        );
        assert_eq!(
            result.bypass[1].error.as_ref().unwrap().code,
            DnsErrorCode::Cancelled
        );
        assert_eq!(result.outcome_hint, StepOutcome::Warning);
    }

    #[tokio::test]
    async fn local_dns_fixture_preserves_negative_response_categories() {
        let cases = [
            (0u8, DnsErrorCode::NoRecords),
            (2u8, DnsErrorCode::Servfail),
            (3u8, DnsErrorCode::Nxdomain),
            (5u8, DnsErrorCode::Refused),
        ];

        for (response_code, expected_code) in cases {
            let (server, task) = spawn_dns_fixture(response_code).await;
            let result = query_bypass_server(
                &target("fixture.test", AccessPathTargetKind::Hostname),
                server.to_string(),
                Duration::from_millis(500),
                CancellationToken::new(),
            )
            .await;

            assert_eq!(result.error, None);
            let expected_outcome = if expected_code == DnsErrorCode::NoRecords {
                DnsQueryOutcome::NoRecords
            } else {
                DnsQueryOutcome::Failed
            };
            assert!(result.queries.iter().all(|query| {
                query.outcome == expected_outcome
                    && query.error.as_ref().map(|error| error.code) == Some(expected_code)
            }));
            task.abort();
        }
    }

    #[tokio::test]
    async fn local_dns_fixture_timeout_is_not_reported_as_no_records() {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server = socket.local_addr().unwrap();
        let result = query_bypass_server(
            &target("timeout.test", AccessPathTargetKind::Hostname),
            server.to_string(),
            Duration::from_millis(80),
            CancellationToken::new(),
        )
        .await;

        assert_eq!(result.error, None);
        assert_eq!(result.queries.len(), 3);
        assert!(result.queries.iter().all(|query| {
            query.outcome == DnsQueryOutcome::Failed
                && query.error.as_ref().map(|error| error.code) == Some(DnsErrorCode::Timeout)
        }));
    }

    #[test]
    fn system_transport_errors_keep_transport_category() {
        let error =
            classify_system_error(&std::io::Error::from(std::io::ErrorKind::ConnectionReset));
        assert_eq!(error.code, DnsErrorCode::TransportError);
        assert!(error.retriable);
    }

    #[test]
    fn unsupported_capabilities_are_explicit() {
        let capabilities = unsupported_capabilities();
        assert_eq!(capabilities.len(), 4);
        assert!(capabilities
            .iter()
            .all(|item| item.status == DnsCapabilityStatus::Unsupported));
        let wire = serde_json::to_value(capabilities).unwrap();
        assert_eq!(wire[0]["id"], "windows_dns_cache");
        assert_eq!(wire[1]["id"], "nrpt");
        assert_eq!(wire[2]["id"], "doh");
        assert_eq!(wire[3]["id"], "candidate_domain_expansion");
    }
}

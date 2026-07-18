use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::pin::Pin;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, WebviewWindow};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::events::EVENT_ACCESS_PATH_DIAGNOSIS_SNAPSHOT;

use super::adapters::{
    dns::diagnose_dns,
    hosts::diagnose_system_hosts,
    proxy::{
        inspect_proxy_profile, inspect_proxy_profiles, ProxyProfileAvailability, ProxyProfileKind,
        ProxyProfileSnapshot, ProxyRoute, SanitizedProxyEndpoint,
    },
};
use super::model::{
    AccessPathTargetKind, AccessProtocol, Conclusion, ConclusionSeverity, DiagnosisReport,
    DiagnosticError, DiagnosticStep, Evidence, NormalizedAccessPathTarget, Recommendation, StepId,
    StepLifecycle, StepOutcome, REPORT_SCHEMA_VERSION,
};
use super::probes::http::{
    connect_http_proxy_tunnel, connect_http_proxy_tunnel_cancellable, probe_http_with_connector,
    ConnectedHttpTransport, ConnectorHttpProbeRequest, HttpProbeError, HttpProbeLimits,
    HttpProbeResult, ProxyConnectEvidence,
};
use super::probes::tcp::{probe_tcp, select_probe_addresses, TcpProbeStatus};
use super::probes::tls::{
    connect_tls_address, connect_tls_stream, probe_tls_addresses, TlsProbeConfig, TlsProbeResult,
    TlsVerificationState,
};
use url::Url;

const DEFAULT_OVERALL_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_STEP_TIMEOUT_MS: u64 = 5_000;
const MIN_OVERALL_TIMEOUT_MS: u64 = 20;
const MAX_OVERALL_TIMEOUT_MS: u64 = 300_000;
const MIN_STEP_TIMEOUT_MS: u64 = 10;
const MAX_STEP_TIMEOUT_MS: u64 = 120_000;
const MAX_ACTIVE_RUNS: usize = 16;
const MAX_RETAINED_TERMINAL_RUNS: usize = 32;

const STEP_ORDER: [StepId; 6] = [
    StepId::Proxy,
    StepId::Hosts,
    StepId::Dns,
    StepId::Tcp,
    StepId::Tls,
    StepId::Http,
];

fn default_overall_timeout_ms() -> u64 {
    DEFAULT_OVERALL_TIMEOUT_MS
}

fn default_step_timeout_ms() -> u64 {
    DEFAULT_STEP_TIMEOUT_MS
}

fn default_proxy_profile() -> DiagnosisProxyProfile {
    DiagnosisProxyProfile::Auto
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosisStartRequest {
    pub input: NormalizedAccessPathTarget,
    #[serde(default = "default_overall_timeout_ms")]
    pub overall_timeout_ms: u64,
    #[serde(default = "default_step_timeout_ms")]
    pub step_timeout_ms: u64,
    #[serde(default)]
    pub dns_servers: Vec<String>,
    #[serde(default = "default_proxy_profile")]
    pub proxy_profile: DiagnosisProxyProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisProxyProfile {
    Auto,
    Environment,
    WindowsUser,
    WinHttp,
    Direct,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosisStartResponse {
    pub run_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisRunStatus {
    Running,
    Completed,
    Cancelled,
    TimedOut,
    Failed,
}

impl DiagnosisRunStatus {
    fn is_terminal(self) -> bool {
        self != Self::Running
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosisRunSnapshot {
    pub run_id: String,
    pub sequence: u64,
    pub status: DiagnosisRunStatus,
    pub report: DiagnosisReport,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosisSnapshotEvent {
    pub run_id: String,
    pub sequence: u64,
    pub snapshot: DiagnosisRunSnapshot,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosisCancelResponse {
    pub run_id: String,
    pub cancelled: bool,
    pub snapshot: DiagnosisRunSnapshot,
}

pub(crate) enum StepExecutionResult {
    Completed {
        outcome: StepOutcome,
        evidence: Vec<Evidence>,
        error: Option<DiagnosticError>,
    },
    Blocked {
        error: DiagnosticError,
    },
    Skipped {
        error: DiagnosticError,
    },
}

type StepFuture = Pin<Box<dyn Future<Output = StepExecutionResult> + Send + 'static>>;

pub(crate) trait StepExecutor: Send + Sync {
    fn execute(
        &self,
        step_id: StepId,
        request: DiagnosisStartRequest,
        cancellation: CancellationToken,
    ) -> StepFuture;
}

struct EnvironmentStepExecutor;

impl StepExecutor for EnvironmentStepExecutor {
    fn execute(
        &self,
        step_id: StepId,
        request: DiagnosisStartRequest,
        cancellation: CancellationToken,
    ) -> StepFuture {
        match step_id {
            StepId::Proxy => execute_proxy_step(request, cancellation),
            StepId::Hosts => execute_hosts_step(request.input, cancellation),
            StepId::Dns => execute_dns_step(request, cancellation),
            StepId::Tcp => execute_tcp_step(request, cancellation),
            StepId::Tls => execute_tls_step(request, cancellation),
            StepId::Http => execute_http_step(request, cancellation),
        }
    }
}

fn execute_proxy_step(
    request: DiagnosisStartRequest,
    cancellation: CancellationToken,
) -> StepFuture {
    Box::pin(async move {
        if request.proxy_profile == DiagnosisProxyProfile::Direct {
            return completed_with_evidence(
                StepId::Proxy,
                "proxy_manual_direct",
                StepOutcome::Success,
                &serde_json::json!({
                    "profile": "direct",
                    "route": "direct",
                    "configurationSource": "user_override",
                }),
            );
        }

        let selected_profile = request.proxy_profile;
        let target = request.input;
        let task = tokio::task::spawn_blocking(move || match selected_profile {
            DiagnosisProxyProfile::Auto => Ok(inspect_proxy_profiles(&target)),
            DiagnosisProxyProfile::Environment => Err(inspect_proxy_profile(
                &target,
                ProxyProfileKind::Environment,
            )),
            DiagnosisProxyProfile::WindowsUser => Err(inspect_proxy_profile(
                &target,
                ProxyProfileKind::WindowsUser,
            )),
            DiagnosisProxyProfile::WinHttp => {
                Err(inspect_proxy_profile(&target, ProxyProfileKind::WinHttp))
            }
            DiagnosisProxyProfile::Direct => unreachable!(),
        });

        tokio::select! {
            biased;
            _ = cancellation.cancelled() => StepExecutionResult::Blocked {
                error: cancelled_step_error(),
            },
            result = task => match result {
                Err(error) => failed_step(
                    "proxy_task_failed",
                    format!("代理画像读取任务失败: {error}"),
                    true,
                ),
                Ok(Ok(inspection)) => {
                    let outcome = inspection
                        .profiles
                        .iter()
                        .find(|profile| profile.kind == inspection.recommended_profile)
                        .map(proxy_profile_outcome)
                        .unwrap_or(StepOutcome::Unverified);
                    completed_with_evidence(
                        StepId::Proxy,
                        "proxy_inspection",
                        outcome,
                        &inspection,
                    )
                }
                Ok(Err(profile)) => completed_with_evidence(
                    StepId::Proxy,
                    "proxy_profile",
                    proxy_profile_outcome(&profile),
                    &profile,
                ),
            }
        }
    })
}

fn proxy_profile_outcome(profile: &ProxyProfileSnapshot) -> StepOutcome {
    if profile.availability != ProxyProfileAvailability::Available
        || profile.decision.route == ProxyRoute::Unresolved
    {
        return StepOutcome::Unverified;
    }
    if !profile.errors.is_empty()
        || !profile.decision.uncertainties.is_empty()
        || profile
            .capabilities
            .iter()
            .any(|capability| capability.configured)
    {
        StepOutcome::Warning
    } else {
        StepOutcome::Success
    }
}

fn execute_hosts_step(
    input: NormalizedAccessPathTarget,
    cancellation: CancellationToken,
) -> StepFuture {
    Box::pin(async move {
        if input.target_kind != AccessPathTargetKind::Hostname {
            return StepExecutionResult::Skipped {
                error: DiagnosticError {
                    code: "hosts_not_applicable".into(),
                    message: "IP 目标不进行 Hosts 域名匹配".into(),
                    details: None,
                    retriable: false,
                },
            };
        }
        let hostname = input.hostname;
        let task = tokio::task::spawn_blocking(move || diagnose_system_hosts(&hostname));
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => StepExecutionResult::Blocked {
                error: cancelled_step_error(),
            },
            result = task => match result {
                Err(error) => failed_step("hosts_task_failed", format!("Hosts 诊断任务失败: {error}"), true),
                Ok(Err(error)) => {
                    let details = match serde_json::to_value(&error) {
                        Ok(details) => Some(details),
                        Err(serialize_error) => return failed_step(
                            "hosts_error_serialize_failed",
                            format!("序列化 Hosts 错误失败: {serialize_error}"),
                            false,
                        ),
                    };
                    StepExecutionResult::Completed {
                        outcome: StepOutcome::Failed,
                        evidence: Vec::new(),
                        error: Some(DiagnosticError {
                            code: "hosts_read_failed".into(),
                            message: error.message,
                            details,
                            retriable: true,
                        }),
                    }
                }
                Ok(Ok(result)) => {
                    let outcome = if result.duplicate_mapping
                        || result.multiple_addresses
                        || result.mixed_address_families
                        || !result.commented_entries.is_empty()
                        || !result.issues.is_empty()
                    {
                        StepOutcome::Warning
                    } else {
                        StepOutcome::Success
                    };
                    completed_with_evidence(StepId::Hosts, "hosts_diagnostic", outcome, &result)
                }
            }
        }
    })
}

fn execute_dns_step(request: DiagnosisStartRequest, cancellation: CancellationToken) -> StepFuture {
    Box::pin(async move {
        if request.input.target_kind != AccessPathTargetKind::Hostname
            && request.dns_servers.is_empty()
        {
            return StepExecutionResult::Skipped {
                error: DiagnosticError {
                    code: "dns_not_applicable".into(),
                    message: "IP 目标未指定旁路 DNS，不需要执行 DNS 查询".into(),
                    details: None,
                    retriable: false,
                },
            };
        }
        let result = diagnose_dns(
            &request.input,
            &request.dns_servers,
            Duration::from_millis(request.step_timeout_ms),
            cancellation,
        )
        .await;
        let outcome = result.derive_outcome_hint();
        let error = (outcome == StepOutcome::Failed).then(|| DiagnosticError {
            code: "dns_resolution_failed".into(),
            message: "DNS 解析失败，详见结构化证据".into(),
            details: None,
            retriable: true,
        });
        completed_with_evidence_and_error(StepId::Dns, "dns_diagnostic", outcome, &result, error)
    })
}

#[derive(Debug, Clone)]
enum SelectedTransportRoute {
    Direct {
        configuration_source: String,
    },
    Proxy {
        endpoint: SanitizedProxyEndpoint,
        configuration_source: String,
    },
}

fn execute_tcp_step(request: DiagnosisStartRequest, cancellation: CancellationToken) -> StepFuture {
    Box::pin(async move {
        let route = match resolve_selected_route(&request, cancellation.clone()).await {
            Ok(route) => route,
            Err(error) => return StepExecutionResult::Blocked { error },
        };

        let (target, route_label, destination_role, configuration_source) = match route {
            SelectedTransportRoute::Direct {
                configuration_source,
            } => (
                request.input.clone(),
                "direct",
                "target",
                configuration_source,
            ),
            SelectedTransportRoute::Proxy {
                endpoint,
                configuration_source,
            } => {
                let Some(target) = proxy_probe_target(&request.input, &endpoint) else {
                    return blocked_step(
                        "proxy_port_missing",
                        format!("代理 {} 未提供端口，且协议没有已知默认端口", endpoint.url),
                        false,
                        Some(serde_json::json!({
                            "proxy": endpoint,
                            "configurationSource": configuration_source,
                        })),
                    );
                };
                (target, "proxy", "proxy", configuration_source)
            }
        };

        let candidates = match resolve_probe_candidates(&target, cancellation.clone()).await {
            Ok(candidates) => candidates,
            Err(error) => return StepExecutionResult::Blocked { error },
        };
        let result = match probe_tcp(
            &target,
            &candidates,
            Duration::from_millis(request.step_timeout_ms),
            cancellation,
        )
        .await
        {
            Ok(result) => result,
            Err(error) if error.code.starts_with("invalid_") => {
                return blocked_step(&error.code, error.message, false, None);
            }
            Err(error) => return failed_step(&error.code, error.message, true),
        };
        if result.attempts.is_empty() {
            let code = if route_label == "proxy" {
                "proxy_dns_failed"
            } else {
                "tcp_no_candidate_addresses"
            };
            return blocked_step(
                code,
                format!("{} 没有可用于 TCP 探测的地址", target.hostname),
                true,
                Some(serde_json::json!({
                    "route": route_label,
                    "destinationRole": destination_role,
                    "destinationHost": target.hostname,
                })),
            );
        }

        let connected = result
            .attempts
            .iter()
            .filter(|attempt| attempt.status == TcpProbeStatus::Connected)
            .count();
        let outcome = if connected == result.attempts.len() {
            StepOutcome::Success
        } else if connected > 0 {
            StepOutcome::Warning
        } else {
            StepOutcome::Failed
        };
        let error = (outcome == StepOutcome::Failed).then(|| DiagnosticError {
            code: "tcp_all_attempts_failed".into(),
            message: "所有 TCP 候选地址均连接失败，详见结构化证据".into(),
            details: None,
            retriable: true,
        });
        completed_with_evidence_and_error(
            StepId::Tcp,
            "tcp_probe",
            outcome,
            &serde_json::json!({
                "route": route_label,
                "destinationRole": destination_role,
                "destinationHost": target.hostname,
                "configurationSource": configuration_source,
                "result": result,
            }),
            error,
        )
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TlsTransportEvidence {
    route: String,
    destination_role: String,
    destination_host: String,
    target_host: String,
    connection_ip_role: String,
    configuration_source: String,
    proxy_endpoint: Option<String>,
    target_authority: Option<String>,
    proxy_connect: Vec<ProxyConnectEvidence>,
    proxy_connect_errors: Vec<HttpProbeError>,
    results: Vec<TlsProbeResult>,
}

fn execute_tls_step(request: DiagnosisStartRequest, cancellation: CancellationToken) -> StepFuture {
    Box::pin(async move {
        if request.input.protocol != AccessProtocol::Https {
            return StepExecutionResult::Skipped {
                error: DiagnosticError {
                    code: "tls_not_applicable".into(),
                    message: "HTTP 目标不执行 TLS 探测".into(),
                    details: None,
                    retriable: false,
                },
            };
        }
        let route = match resolve_selected_route(&request, cancellation.clone()).await {
            Ok(route) => route,
            Err(error) => return StepExecutionResult::Blocked { error },
        };
        let (sni, verify_hostname) = initial_tls_names(&request.input);

        match route {
            SelectedTransportRoute::Direct {
                configuration_source,
            } => {
                let candidates =
                    match resolve_probe_candidates(&request.input, cancellation.clone()).await {
                        Ok(candidates) => candidates,
                        Err(error) => return StepExecutionResult::Blocked { error },
                    };
                let addresses = match select_probe_addresses(&request.input, &candidates) {
                    Ok(addresses) => addresses,
                    Err(error) => {
                        return blocked_step(&error.code, error.message, false, None);
                    }
                };
                if addresses.is_empty() {
                    return blocked_step(
                        "tls_blocked_by_dns",
                        "没有可用于 TLS 探测的目标地址".into(),
                        true,
                        Some(serde_json::json!({ "hostname": request.input.hostname })),
                    );
                }
                let port = request.input.port;
                let attempt_timeout =
                    bounded_attempt_timeout(request.step_timeout_ms, addresses.len());
                let task = tokio::task::spawn_blocking(move || {
                    probe_tls_addresses(
                        &addresses,
                        port,
                        sni.as_deref(),
                        verify_hostname.as_deref(),
                        attempt_timeout,
                    )
                });
                let results = tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => {
                        return StepExecutionResult::Blocked { error: cancelled_step_error() };
                    }
                    result = task => match result {
                        Ok(results) => results,
                        Err(error) => {
                            return failed_step(
                                "tls_probe_task_failed",
                                format!("TLS 探测任务异常结束: {error}"),
                                true,
                            );
                        }
                    }
                };
                tls_step_result(TlsTransportEvidence {
                    route: "direct".into(),
                    destination_role: "target".into(),
                    destination_host: request.input.hostname.clone(),
                    target_host: request.input.hostname,
                    connection_ip_role: "target".into(),
                    configuration_source,
                    proxy_endpoint: None,
                    target_authority: None,
                    proxy_connect: Vec::new(),
                    proxy_connect_errors: Vec::new(),
                    results,
                })
            }
            SelectedTransportRoute::Proxy {
                endpoint,
                configuration_source,
            } => {
                if endpoint.scheme != "http" {
                    return blocked_step(
                        "proxy_scheme_unsupported",
                        format!(
                            "当前版本仅支持 HTTP 代理 CONNECT，不支持 {} 代理传输",
                            endpoint.scheme
                        ),
                        false,
                        Some(serde_json::json!({ "proxy": endpoint })),
                    );
                }
                let Some(proxy_target) = proxy_probe_target(&request.input, &endpoint) else {
                    return blocked_step(
                        "proxy_port_missing",
                        "HTTP 代理没有可用端口".into(),
                        false,
                        Some(serde_json::json!({ "proxy": endpoint })),
                    );
                };
                let candidates =
                    match resolve_probe_candidates(&proxy_target, cancellation.clone()).await {
                        Ok(candidates) => candidates,
                        Err(error) => return StepExecutionResult::Blocked { error },
                    };
                let proxy_addresses = match select_probe_addresses(&proxy_target, &candidates) {
                    Ok(addresses) => addresses,
                    Err(error) => {
                        return blocked_step(&error.code, error.message, false, None);
                    }
                };
                if proxy_addresses.is_empty() {
                    return blocked_step(
                        "proxy_dns_failed",
                        "没有可用于代理 CONNECT 的代理地址".into(),
                        true,
                        Some(serde_json::json!({ "proxy": endpoint })),
                    );
                }

                let authority_host = request
                    .input
                    .connection_ip
                    .as_deref()
                    .unwrap_or(&request.input.hostname);
                let target_authority = format_authority(authority_host, request.input.port);
                let proxy_port = proxy_target.port;
                let target_port = request.input.port;
                let attempt_timeout =
                    bounded_attempt_timeout(request.step_timeout_ms, proxy_addresses.len());
                let target_authority_for_task = target_authority.clone();
                let task = tokio::task::spawn_blocking(move || {
                    let mut proxy_connect = Vec::new();
                    let mut proxy_connect_errors = Vec::new();
                    let mut results = Vec::new();
                    for proxy_ip in proxy_addresses {
                        let proxy_address = SocketAddr::new(proxy_ip, proxy_port);
                        let mut limits = HttpProbeLimits::default();
                        limits.timeout = attempt_timeout;
                        match connect_http_proxy_tunnel(
                            proxy_address,
                            &target_authority_for_task,
                            &limits,
                        ) {
                            Ok(tunnel) => {
                                proxy_connect.push(tunnel.evidence);
                                let config = TlsProbeConfig {
                                    connection_ip: proxy_ip,
                                    sni: sni.clone(),
                                    verify_hostname: verify_hostname.clone(),
                                    timeout: attempt_timeout,
                                };
                                match connect_tls_stream(tunnel.stream, target_port, config) {
                                    Ok(session) => results.push(session.result),
                                    Err(result) => results.push(result),
                                }
                            }
                            Err(error) => proxy_connect_errors.push(error),
                        }
                    }
                    (proxy_connect, proxy_connect_errors, results)
                });
                let (proxy_connect, proxy_connect_errors, results) = tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => {
                        return StepExecutionResult::Blocked { error: cancelled_step_error() };
                    }
                    result = task => match result {
                        Ok(result) => result,
                        Err(error) => {
                            return failed_step(
                                "tls_probe_task_failed",
                                format!("代理 TLS 探测任务异常结束: {error}"),
                                true,
                            );
                        }
                    }
                };
                if results.is_empty() {
                    let details = serde_json::json!({
                        "proxy": endpoint,
                        "targetAuthority": target_authority,
                        "connectErrors": proxy_connect_errors,
                    });
                    return blocked_step(
                        "tls_blocked_by_proxy_connect",
                        "所有代理 CONNECT 尝试均失败，未执行 TLS 握手".into(),
                        true,
                        Some(details),
                    );
                }
                tls_step_result(TlsTransportEvidence {
                    route: "proxy".into(),
                    destination_role: "proxy".into(),
                    destination_host: endpoint.host,
                    target_host: request.input.hostname,
                    connection_ip_role: "proxy_peer".into(),
                    configuration_source,
                    proxy_endpoint: Some(endpoint.url),
                    target_authority: Some(target_authority),
                    proxy_connect,
                    proxy_connect_errors,
                    results,
                })
            }
        }
    })
}

fn tls_step_result(evidence: TlsTransportEvidence) -> StepExecutionResult {
    let handshake_succeeded = evidence
        .results
        .iter()
        .filter(|result| result.handshake_succeeded)
        .count();
    let valid = evidence
        .results
        .iter()
        .filter(|result| {
            result.handshake_succeeded
                && result.hostname_verification.state != TlsVerificationState::Failed
                && result.trust_verification.openssl_state != TlsVerificationState::Failed
        })
        .count();
    let invalid = handshake_succeeded.saturating_sub(valid);
    let outcome =
        if valid == 0 {
            StepOutcome::Failed
        } else if invalid > 0 || handshake_succeeded < evidence.results.len() {
            StepOutcome::Warning
        } else if evidence.results.iter().any(|result| {
            matches!(
                result.trust_verification.windows_state,
                TlsVerificationState::Unverified | TlsVerificationState::Unsupported
            )
        }) {
            StepOutcome::Unverified
        } else if evidence.results.iter().any(|result| {
            result.trust_verification.revocation_state != TlsVerificationState::Verified
        }) {
            StepOutcome::Warning
        } else {
            StepOutcome::Success
        };
    let outcome =
        tls_outcome_with_proxy_connect_errors(outcome, evidence.proxy_connect_errors.len());
    let error = (outcome == StepOutcome::Failed).then(|| DiagnosticError {
        code: if handshake_succeeded > 0 {
            "tls_certificate_invalid".into()
        } else {
            "tls_handshake_failed".into()
        },
        message: if handshake_succeeded > 0 {
            "TLS 握手成功，但证书主机名或信任校验失败".into()
        } else {
            "所有候选地址的 TLS 握手均失败".into()
        },
        details: None,
        retriable: true,
    });
    completed_with_evidence_and_error(StepId::Tls, "tls_probe", outcome, &evidence, error)
}

fn tls_outcome_with_proxy_connect_errors(
    outcome: StepOutcome,
    proxy_connect_error_count: usize,
) -> StepOutcome {
    if proxy_connect_error_count > 0 && outcome == StepOutcome::Success {
        StepOutcome::Warning
    } else {
        outcome
    }
}

#[derive(Debug)]
struct HttpBlockingProbe {
    result: Result<HttpProbeResult, HttpProbeError>,
    proxy_connect: Vec<ProxyConnectEvidence>,
    proxy_connect_errors: Vec<HttpProbeError>,
    tls_results: Vec<TlsProbeResult>,
    initial_proxy_connect_failed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HttpTransportEvidence {
    route: String,
    connection_ip_role: String,
    configuration_source: String,
    proxy_endpoint: Option<String>,
    proxy_connect: Vec<ProxyConnectEvidence>,
    proxy_connect_errors: Vec<HttpProbeError>,
    tls_results: Vec<TlsProbeResult>,
    result: Option<HttpProbeResult>,
    probe_error: Option<HttpProbeError>,
}

fn execute_http_step(
    request: DiagnosisStartRequest,
    cancellation: CancellationToken,
) -> StepFuture {
    Box::pin(async move {
        let route = match resolve_selected_route(&request, cancellation.clone()).await {
            Ok(route) => route,
            Err(error) => return StepExecutionResult::Blocked { error },
        };
        let url = match Url::parse(&request.input.url) {
            Ok(url) => url,
            Err(error) => {
                return blocked_step(
                    "invalid_normalized_url",
                    format!("归一化 URL 无法解析: {error}"),
                    false,
                    None,
                );
            }
        };
        let mut limits = HttpProbeLimits::default();
        limits.timeout = Duration::from_millis(request.step_timeout_ms);
        let http_host = Some(request.input.http_host.clone());
        let initial_connection_ip = match request.input.connection_ip.as_deref() {
            Some(value) => match value.parse::<IpAddr>() {
                Ok(value) => Some(value),
                Err(error) => {
                    return blocked_step(
                        "invalid_connection_ip",
                        format!("连接 IP 无效: {error}"),
                        false,
                        None,
                    );
                }
            },
            None => None,
        };
        let (initial_sni, initial_verify_hostname) = initial_tls_names(&request.input);

        let (configuration_source, proxy_endpoint, connection_ip_role, task) = match route {
            SelectedTransportRoute::Direct {
                configuration_source,
            } => {
                let blocking_cancellation = cancellation.clone();
                let task = tokio::task::spawn_blocking(move || {
                    probe_http_direct_blocking(
                        url,
                        http_host,
                        initial_connection_ip,
                        initial_sni,
                        initial_verify_hostname,
                        limits,
                        blocking_cancellation,
                    )
                });
                (configuration_source, None, "target".to_string(), task)
            }
            SelectedTransportRoute::Proxy {
                endpoint,
                configuration_source,
            } => {
                if endpoint.scheme != "http" {
                    return blocked_step(
                        "proxy_scheme_unsupported",
                        format!(
                            "当前版本仅支持 HTTP 代理，不支持 {} 代理传输",
                            endpoint.scheme
                        ),
                        false,
                        Some(serde_json::json!({ "proxy": endpoint })),
                    );
                }
                if forward_proxy_connection_override_is_unsupported(&url, initial_connection_ip) {
                    return blocked_step(
                        "http_forward_proxy_connection_ip_unsupported",
                        "当前版本不能在明文 HTTP 代理重定向中同时保持 connection IP 与 Host 分离"
                            .into(),
                        false,
                        Some(serde_json::json!({
                            "connectionIp": initial_connection_ip,
                            "httpHost": request.input.http_host,
                            "targetHost": request.input.hostname,
                        })),
                    );
                }
                let Some(proxy_target) = proxy_probe_target(&request.input, &endpoint) else {
                    return blocked_step(
                        "proxy_port_missing",
                        "HTTP 代理没有可用端口".into(),
                        false,
                        Some(serde_json::json!({ "proxy": endpoint })),
                    );
                };
                let candidates =
                    match resolve_probe_candidates(&proxy_target, cancellation.clone()).await {
                        Ok(candidates) => candidates,
                        Err(error) => return StepExecutionResult::Blocked { error },
                    };
                let proxy_addresses = match select_probe_addresses(&proxy_target, &candidates) {
                    Ok(addresses) => addresses
                        .into_iter()
                        .map(|ip| SocketAddr::new(ip, proxy_target.port))
                        .collect::<Vec<_>>(),
                    Err(error) => {
                        return blocked_step(&error.code, error.message, false, None);
                    }
                };
                if proxy_addresses.is_empty() {
                    return blocked_step(
                        "proxy_dns_failed",
                        "没有可用于 HTTP 代理探测的代理地址".into(),
                        true,
                        Some(serde_json::json!({ "proxy": endpoint })),
                    );
                }
                let proxy_label = endpoint.url.clone();
                let blocking_cancellation = cancellation.clone();
                let task = tokio::task::spawn_blocking(move || {
                    probe_http_proxy_blocking(
                        url,
                        http_host,
                        initial_connection_ip,
                        initial_sni,
                        initial_verify_hostname,
                        proxy_addresses,
                        proxy_label,
                        limits,
                        blocking_cancellation,
                    )
                });
                (
                    configuration_source,
                    Some(endpoint.url),
                    "proxy_peer".to_string(),
                    task,
                )
            }
        };
        let blocking = tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                return StepExecutionResult::Blocked { error: cancelled_step_error() };
            }
            result = task => match result {
                Ok(result) => result,
                Err(error) => {
                    return failed_step(
                        "http_probe_task_failed",
                        format!("HTTP 探测任务异常结束: {error}"),
                        true,
                    );
                }
            }
        };
        http_step_result(
            configuration_source,
            proxy_endpoint,
            connection_ip_role,
            blocking,
        )
    })
}

fn probe_http_direct_blocking(
    url: Url,
    http_host: Option<String>,
    initial_connection_ip: Option<IpAddr>,
    initial_sni: Option<String>,
    initial_verify_hostname: Option<String>,
    limits: HttpProbeLimits,
    cancellation: CancellationToken,
) -> HttpBlockingProbe {
    let mut tls_results = Vec::new();
    let request = ConnectorHttpProbeRequest {
        url,
        http_host,
        limits,
        cancellation: cancellation.clone(),
    };
    let result = probe_http_with_connector(request, |url, remaining, first_exchange| {
        let connector_started = Instant::now();
        ensure_http_probe_active(&cancellation)?;
        let addresses = resolve_http_addresses(
            url,
            first_exchange.then_some(initial_connection_ip).flatten(),
        )?;
        ensure_http_probe_active(&cancellation)?;
        if url.scheme() == "https" {
            let sni = if first_exchange {
                initial_sni.clone()
            } else {
                default_tls_name(url)
            };
            let verify_hostname = if first_exchange {
                initial_verify_hostname.clone()
            } else {
                url.host_str().map(str::to_owned)
            };
            let mut last_error = None;
            let address_count = addresses.len();
            for (index, address) in addresses.into_iter().enumerate() {
                let attempt_started = Instant::now();
                let attempt_timeout = divide_timeout(
                    remaining_http_stage(connector_started, remaining, &cancellation)?,
                    address_count - index,
                );
                match connect_tls_address(
                    address,
                    sni.as_deref(),
                    verify_hostname.as_deref(),
                    attempt_timeout,
                ) {
                    Ok(session) => {
                        ensure_http_probe_active(&cancellation)?;
                        if !tls_result_allows_http(&session.result) {
                            last_error = Some(http_error_from_tls(&session.result));
                            tls_results.push(session.result);
                            continue;
                        }
                        let io_timeout =
                            remaining_http_stage(attempt_started, attempt_timeout, &cancellation)?;
                        configure_http_socket_timeout(session.stream.get_ref(), io_timeout)?;
                        let connection_ip = session.result.connection_ip.to_string();
                        tls_results.push(session.result);
                        return Ok(ConnectedHttpTransport {
                            stream: Box::new(session.stream),
                            connection_ip: Some(connection_ip),
                            absolute_form: false,
                            via_proxy: false,
                            proxy: None,
                        });
                    }
                    Err(result) => {
                        ensure_http_probe_active(&cancellation)?;
                        last_error = Some(http_error_from_tls(&result));
                        tls_results.push(result);
                    }
                }
            }
            Err(last_error.unwrap_or_else(|| {
                HttpProbeError::new("tls_transport_failed", "没有可用于 HTTPS 的候选地址")
            }))
        } else {
            connect_plain_http_addresses(
                &addresses,
                remaining_http_stage(connector_started, remaining, &cancellation)?,
                false,
                None,
                &cancellation,
            )
        }
    });
    HttpBlockingProbe {
        result,
        proxy_connect: Vec::new(),
        proxy_connect_errors: Vec::new(),
        tls_results,
        initial_proxy_connect_failed: false,
    }
}

#[allow(clippy::too_many_arguments)]
fn probe_http_proxy_blocking(
    url: Url,
    http_host: Option<String>,
    initial_connection_ip: Option<IpAddr>,
    initial_sni: Option<String>,
    initial_verify_hostname: Option<String>,
    proxy_addresses: Vec<SocketAddr>,
    proxy_label: String,
    limits: HttpProbeLimits,
    cancellation: CancellationToken,
) -> HttpBlockingProbe {
    let mut proxy_connect = Vec::new();
    let mut proxy_connect_errors = Vec::new();
    let mut tls_results = Vec::new();
    let mut initial_proxy_connect_failed = false;
    let request = ConnectorHttpProbeRequest {
        url,
        http_host,
        limits,
        cancellation: cancellation.clone(),
    };
    let result = probe_http_with_connector(request, |url, remaining, first_exchange| {
        let connector_started = Instant::now();
        ensure_http_probe_active(&cancellation)?;
        if url.scheme() == "http" {
            return connect_plain_http_addresses(
                &proxy_addresses,
                remaining_http_stage(connector_started, remaining, &cancellation)?,
                true,
                Some(proxy_label.clone()),
                &cancellation,
            );
        }

        let authority_host = if first_exchange {
            initial_connection_ip
                .map(|ip| ip.to_string())
                .or_else(|| url.host_str().map(str::to_owned))
        } else {
            url.host_str().map(str::to_owned)
        }
        .ok_or_else(|| HttpProbeError::new("origin_dns_failed", "HTTPS URL 缺少目标主机"))?;
        let port = url
            .port_or_known_default()
            .ok_or_else(|| HttpProbeError::new("invalid_target_port", "HTTPS URL 缺少目标端口"))?;
        let authority = format_authority(&authority_host, port);
        let sni = if first_exchange {
            initial_sni.clone()
        } else {
            default_tls_name(url)
        };
        let verify_hostname = if first_exchange {
            initial_verify_hostname.clone()
        } else {
            url.host_str().map(str::to_owned)
        };
        let mut last_error = None;
        let mut connected_tunnel = false;
        let proxy_count = proxy_addresses.len();
        for (index, proxy_address) in proxy_addresses.iter().enumerate() {
            let attempt_started = Instant::now();
            let attempt_timeout = divide_timeout(
                remaining_http_stage(connector_started, remaining, &cancellation)?,
                proxy_count - index,
            );
            let mut connect_limits = HttpProbeLimits::default();
            connect_limits.timeout = attempt_timeout;
            match connect_http_proxy_tunnel_cancellable(
                *proxy_address,
                &authority,
                &connect_limits,
                &cancellation,
            ) {
                Ok(tunnel) => {
                    ensure_http_probe_active(&cancellation)?;
                    connected_tunnel = true;
                    proxy_connect.push(tunnel.evidence);
                    let tls_timeout =
                        remaining_http_stage(attempt_started, attempt_timeout, &cancellation)?;
                    let config = TlsProbeConfig {
                        connection_ip: proxy_address.ip(),
                        sni: sni.clone(),
                        verify_hostname: verify_hostname.clone(),
                        timeout: tls_timeout,
                    };
                    match connect_tls_stream(tunnel.stream, port, config) {
                        Ok(session) => {
                            ensure_http_probe_active(&cancellation)?;
                            if !tls_result_allows_http(&session.result) {
                                last_error = Some(http_error_from_tls(&session.result));
                                tls_results.push(session.result);
                                continue;
                            }
                            let io_timeout = remaining_http_stage(
                                attempt_started,
                                attempt_timeout,
                                &cancellation,
                            )?;
                            configure_http_socket_timeout(session.stream.get_ref(), io_timeout)?;
                            let connection_ip = session.result.connection_ip.to_string();
                            tls_results.push(session.result);
                            return Ok(ConnectedHttpTransport {
                                stream: Box::new(session.stream),
                                connection_ip: Some(connection_ip),
                                absolute_form: false,
                                via_proxy: true,
                                proxy: Some(proxy_label.clone()),
                            });
                        }
                        Err(result) => {
                            ensure_http_probe_active(&cancellation)?;
                            last_error = Some(http_error_from_tls(&result));
                            tls_results.push(result);
                        }
                    }
                }
                Err(error) => {
                    ensure_http_probe_active(&cancellation)?;
                    last_error = Some(error.clone());
                    proxy_connect_errors.push(error);
                }
            }
        }
        if first_exchange && !connected_tunnel {
            initial_proxy_connect_failed = true;
        }
        Err(last_error.unwrap_or_else(|| {
            HttpProbeError::new("proxy_connect_failed", "没有可用于 CONNECT 的代理地址")
        }))
    });
    HttpBlockingProbe {
        result,
        proxy_connect,
        proxy_connect_errors,
        tls_results,
        initial_proxy_connect_failed,
    }
}

fn resolve_http_addresses(
    url: &Url,
    connection_ip: Option<IpAddr>,
) -> Result<Vec<SocketAddr>, HttpProbeError> {
    let port = url
        .port_or_known_default()
        .ok_or_else(|| HttpProbeError::new("invalid_target_port", "HTTP URL 缺少目标端口"))?;
    if let Some(ip) = connection_ip {
        return Ok(vec![SocketAddr::new(ip, port)]);
    }
    let host = url
        .host_str()
        .ok_or_else(|| HttpProbeError::new("origin_dns_failed", "HTTP URL 缺少目标主机"))?;
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, port)]);
    }
    let mut seen = HashSet::new();
    let addresses = (host, port)
        .to_socket_addrs()
        .map_err(|error| {
            let mut result = HttpProbeError::new(
                "origin_dns_failed",
                format!("解析 HTTP 目标 {host} 失败: {error}"),
            );
            result.raw_error = Some(error.to_string());
            result
        })?
        .filter(|address| seen.insert(*address))
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err(HttpProbeError::new(
            "origin_dns_failed",
            format!("HTTP 目标 {host} 没有可用地址"),
        ));
    }
    Ok(addresses)
}

fn connect_plain_http_addresses(
    addresses: &[SocketAddr],
    timeout: Duration,
    via_proxy: bool,
    proxy: Option<String>,
    cancellation: &CancellationToken,
) -> Result<ConnectedHttpTransport, HttpProbeError> {
    let started = Instant::now();
    let mut last_error = None;
    for (index, address) in addresses.iter().enumerate() {
        let attempt_started = Instant::now();
        let attempt_timeout = divide_timeout(
            remaining_http_stage(started, timeout, cancellation)?,
            addresses.len() - index,
        );
        match TcpStream::connect_timeout(address, attempt_timeout) {
            Ok(stream) => {
                ensure_http_probe_active(cancellation)?;
                let io_timeout =
                    remaining_http_stage(attempt_started, attempt_timeout, cancellation)?;
                if let Err(error) = configure_http_socket_timeout(&stream, io_timeout) {
                    last_error = Some(error);
                    continue;
                }
                return Ok(ConnectedHttpTransport {
                    stream: Box::new(stream),
                    connection_ip: Some(address.ip().to_string()),
                    absolute_form: via_proxy,
                    via_proxy,
                    proxy,
                });
            }
            Err(error) => {
                last_error = Some(HttpProbeError::from_io(
                    if via_proxy {
                        "proxy_tcp_failed"
                    } else {
                        "origin_tcp_failed"
                    },
                    if via_proxy {
                        "连接 HTTP 代理失败"
                    } else {
                        "连接 HTTP 目标失败"
                    },
                    error,
                ));
            }
        }
    }
    Err(last_error.unwrap_or_else(|| {
        HttpProbeError::new(
            if via_proxy {
                "proxy_tcp_failed"
            } else {
                "origin_tcp_failed"
            },
            "没有可用的 HTTP 连接地址",
        )
    }))
}

fn configure_http_socket_timeout(
    stream: &TcpStream,
    timeout: Duration,
) -> Result<(), HttpProbeError> {
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|_| stream.set_write_timeout(Some(timeout)))
        .map_err(|error| {
            HttpProbeError::from_io(
                "http_socket_configuration_failed",
                "设置 HTTP socket 超时失败",
                error,
            )
        })
}

fn http_error_from_tls(result: &TlsProbeResult) -> HttpProbeError {
    let mut error = HttpProbeError::new(
        "tls_transport_failed",
        result
            .error
            .as_ref()
            .map(|error| error.message.clone())
            .unwrap_or_else(|| "TLS 证书校验失败，未发送 HTTP 请求".into()),
    );
    error.raw_error = serde_json::to_string(result).ok();
    error
}

fn tls_result_allows_http(result: &TlsProbeResult) -> bool {
    result.handshake_succeeded
        && result.hostname_verification.state == TlsVerificationState::Verified
        && result.trust_verification.openssl_state == TlsVerificationState::Verified
}

fn http_step_result(
    configuration_source: String,
    proxy_endpoint: Option<String>,
    connection_ip_role: String,
    blocking: HttpBlockingProbe,
) -> StepExecutionResult {
    match blocking.result {
        Ok(result) => {
            let warning = !(200..300).contains(&result.final_status)
                || result.exchanges.iter().any(|exchange| {
                    exchange.head_fallback
                        || exchange.response_body_truncated
                        || exchange.redirect_cross_host
                        || exchange.redirect_cross_scheme
                });
            completed_with_evidence(
                StepId::Http,
                "http_probe",
                if warning {
                    StepOutcome::Warning
                } else {
                    StepOutcome::Success
                },
                &HttpTransportEvidence {
                    route: if proxy_endpoint.is_some() {
                        "proxy".into()
                    } else {
                        "direct".into()
                    },
                    connection_ip_role,
                    configuration_source,
                    proxy_endpoint,
                    proxy_connect: blocking.proxy_connect,
                    proxy_connect_errors: blocking.proxy_connect_errors,
                    tls_results: blocking.tls_results,
                    result: Some(result),
                    probe_error: None,
                },
            )
        }
        Err(error) => {
            let evidence = HttpTransportEvidence {
                route: if proxy_endpoint.is_some() {
                    "proxy".into()
                } else {
                    "direct".into()
                },
                connection_ip_role,
                configuration_source,
                proxy_endpoint,
                proxy_connect: blocking.proxy_connect,
                proxy_connect_errors: blocking.proxy_connect_errors,
                tls_results: blocking.tls_results,
                result: None,
                probe_error: Some(error.clone()),
            };
            if error.exchanges.is_empty() && blocking.initial_proxy_connect_failed {
                return blocked_step(
                    "http_blocked_by_proxy_connect",
                    error.message,
                    true,
                    serde_json::to_value(evidence).ok(),
                );
            }
            if error.exchanges.is_empty() && is_pre_http_blocker(&error.code) {
                return blocked_step(
                    match error.code.as_str() {
                        "origin_dns_failed" => "http_blocked_by_dns",
                        "origin_tcp_failed" | "proxy_tcp_failed" => "http_blocked_by_tcp",
                        "tls_transport_failed" => "http_blocked_by_tls",
                        _ => "http_blocked_by_proxy_connect",
                    },
                    error.message,
                    true,
                    serde_json::to_value(evidence).ok(),
                );
            }
            completed_with_evidence_and_error(
                StepId::Http,
                "http_probe",
                StepOutcome::Failed,
                &evidence,
                Some(DiagnosticError {
                    code: error.code,
                    message: error.message,
                    details: None,
                    retriable: true,
                }),
            )
        }
    }
}

fn is_pre_http_blocker(code: &str) -> bool {
    matches!(
        code,
        "origin_dns_failed"
            | "origin_tcp_failed"
            | "proxy_tcp_failed"
            | "proxy_connect_failed"
            | "proxy_authentication_required"
            | "proxy_connect_rejected"
            | "tls_transport_failed"
    )
}

fn default_tls_name(url: &Url) -> Option<String> {
    url.host_str()
        .filter(|host| host.parse::<IpAddr>().is_err())
        .map(str::to_owned)
}

fn forward_proxy_connection_override_is_unsupported(
    url: &Url,
    initial_connection_ip: Option<IpAddr>,
) -> bool {
    url.scheme() == "http" && initial_connection_ip.is_some()
}

fn initial_tls_names(target: &NormalizedAccessPathTarget) -> (Option<String>, Option<String>) {
    (
        target.sni.clone(),
        Some(
            target
                .verify_hostname
                .clone()
                .unwrap_or_else(|| target.hostname.clone()),
        ),
    )
}

fn divide_timeout(timeout: Duration, attempts: usize) -> Duration {
    let millis = timeout.as_millis() / attempts.max(1) as u128;
    Duration::from_millis(millis.max(1).min(u128::from(u64::MAX)) as u64)
}

fn ensure_http_probe_active(cancellation: &CancellationToken) -> Result<(), HttpProbeError> {
    if cancellation.is_cancelled() {
        Err(HttpProbeError::new(
            "diagnosis_cancelled",
            "HTTP 探测已取消，未继续发送请求",
        ))
    } else {
        Ok(())
    }
}

fn remaining_http_stage(
    started: Instant,
    timeout: Duration,
    cancellation: &CancellationToken,
) -> Result<Duration, HttpProbeError> {
    ensure_http_probe_active(cancellation)?;
    timeout
        .checked_sub(started.elapsed())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| HttpProbeError::new("http_timeout", "HTTP 探测超过整体耗时限制"))
}

fn bounded_attempt_timeout(step_timeout_ms: u64, attempts: usize) -> Duration {
    Duration::from_millis((step_timeout_ms / attempts.max(1) as u64).max(1))
}

fn format_authority(host: &str, port: u16) -> String {
    match host.parse::<IpAddr>() {
        Ok(ip) => SocketAddr::new(ip, port).to_string(),
        Err(_) => format!("{host}:{port}"),
    }
}

async fn resolve_selected_route(
    request: &DiagnosisStartRequest,
    cancellation: CancellationToken,
) -> Result<SelectedTransportRoute, DiagnosticError> {
    if request.proxy_profile == DiagnosisProxyProfile::Direct {
        return Ok(SelectedTransportRoute::Direct {
            configuration_source: "user_override".into(),
        });
    }

    let selected_profile = request.proxy_profile;
    let target = request.input.clone();
    let task = tokio::task::spawn_blocking(move || match selected_profile {
        DiagnosisProxyProfile::Auto => {
            let inspection = inspect_proxy_profiles(&target);
            inspection
                .profiles
                .into_iter()
                .find(|profile| profile.kind == inspection.recommended_profile)
                .ok_or_else(|| "自动代理画像没有推荐结果".to_string())
        }
        DiagnosisProxyProfile::Environment => Ok(inspect_proxy_profile(
            &target,
            ProxyProfileKind::Environment,
        )),
        DiagnosisProxyProfile::WindowsUser => Ok(inspect_proxy_profile(
            &target,
            ProxyProfileKind::WindowsUser,
        )),
        DiagnosisProxyProfile::WinHttp => {
            Ok(inspect_proxy_profile(&target, ProxyProfileKind::WinHttp))
        }
        DiagnosisProxyProfile::Direct => unreachable!(),
    });
    let profile = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Err(cancelled_step_error()),
        result = task => match result {
            Err(error) => {
                return Err(DiagnosticError {
                    code: "proxy_route_task_failed".into(),
                    message: format!("代理路由计算任务失败: {error}"),
                    details: None,
                    retriable: true,
                });
            }
            Ok(Err(error)) => {
                return Err(DiagnosticError {
                    code: "proxy_route_unresolved".into(),
                    message: error,
                    details: None,
                    retriable: true,
                });
            }
            Ok(Ok(profile)) => profile,
        }
    };
    selected_transport_route(profile)
}

fn selected_transport_route(
    profile: ProxyProfileSnapshot,
) -> Result<SelectedTransportRoute, DiagnosticError> {
    if profile.availability != ProxyProfileAvailability::Available
        || profile.decision.route == ProxyRoute::Unresolved
    {
        let details = serde_json::to_value(&profile).ok();
        return Err(DiagnosticError {
            code: "proxy_route_unresolved".into(),
            message: "所选客户端画像无法确定当前目标应走代理还是直连".into(),
            details,
            retriable: true,
        });
    }

    match profile.decision.route {
        ProxyRoute::Direct => Ok(SelectedTransportRoute::Direct {
            configuration_source: profile.decision.configuration_source,
        }),
        ProxyRoute::Proxy => {
            let details = serde_json::to_value(&profile).ok();
            let Some(endpoint) = profile.decision.proxy else {
                return Err(DiagnosticError {
                    code: "proxy_route_unresolved".into(),
                    message: "代理画像选择了代理路由，但没有有效代理端点".into(),
                    details,
                    retriable: true,
                });
            };
            Ok(SelectedTransportRoute::Proxy {
                endpoint,
                configuration_source: profile.decision.configuration_source,
            })
        }
        ProxyRoute::Unresolved => unreachable!(),
    }
}

fn proxy_probe_target(
    original: &NormalizedAccessPathTarget,
    endpoint: &SanitizedProxyEndpoint,
) -> Option<NormalizedAccessPathTarget> {
    let port = endpoint.port.or_else(|| match endpoint.scheme.as_str() {
        "http" => Some(80),
        "https" => Some(443),
        "socks" | "socks4" | "socks5" => Some(1080),
        _ => None,
    })?;
    let target_kind = match endpoint.host.parse::<IpAddr>() {
        Ok(IpAddr::V4(_)) => AccessPathTargetKind::Ipv4,
        Ok(IpAddr::V6(_)) => AccessPathTargetKind::Ipv6,
        Err(_) => AccessPathTargetKind::Hostname,
    };
    let mut target = original.clone();
    target.raw_input = endpoint.url.clone();
    target.hostname = endpoint.host.clone();
    target.target_kind = target_kind;
    target.port = port;
    target.path.clear();
    target.url = endpoint.url.clone();
    target.sni = None;
    target.http_host = endpoint.host.clone();
    target.connection_ip = None;
    Some(target)
}

async fn resolve_probe_candidates(
    target: &NormalizedAccessPathTarget,
    cancellation: CancellationToken,
) -> Result<Vec<IpAddr>, DiagnosticError> {
    if target.connection_ip.is_some() || target.target_kind != AccessPathTargetKind::Hostname {
        return Ok(Vec::new());
    }
    let lookup = tokio::net::lookup_host((target.hostname.as_str(), target.port));
    let addresses = tokio::select! {
        biased;
        _ = cancellation.cancelled() => return Err(cancelled_step_error()),
        result = lookup => result.map_err(|error| DiagnosticError {
            code: "tcp_address_resolution_failed".into(),
            message: format!("解析 {} 的 TCP 候选地址失败: {error}", target.hostname),
            details: Some(serde_json::json!({ "rawError": error.to_string() })),
            retriable: true,
        })?,
    };
    let mut seen = HashSet::new();
    Ok(addresses
        .map(|address| address.ip())
        .filter(|address| seen.insert(*address))
        .collect())
}

fn completed_with_evidence(
    step_id: StepId,
    kind: &str,
    outcome: StepOutcome,
    value: &impl Serialize,
) -> StepExecutionResult {
    completed_with_evidence_and_error(step_id, kind, outcome, value, None)
}

fn completed_with_evidence_and_error(
    step_id: StepId,
    kind: &str,
    outcome: StepOutcome,
    value: &impl Serialize,
    error: Option<DiagnosticError>,
) -> StepExecutionResult {
    match serde_json::to_value(value) {
        Ok(value) => StepExecutionResult::Completed {
            outcome,
            evidence: vec![Evidence {
                id: Uuid::new_v4().to_string(),
                step_id,
                kind: kind.into(),
                value,
                observed_at: Some(now_iso()),
            }],
            error,
        },
        Err(serialize_error) => failed_step(
            "evidence_serialize_failed",
            format!("序列化诊断证据失败: {serialize_error}"),
            false,
        ),
    }
}

fn failed_step(code: &str, message: String, retriable: bool) -> StepExecutionResult {
    StepExecutionResult::Completed {
        outcome: StepOutcome::Failed,
        evidence: Vec::new(),
        error: Some(DiagnosticError {
            code: code.into(),
            message,
            details: None,
            retriable,
        }),
    }
}

fn blocked_step(
    code: &str,
    message: String,
    retriable: bool,
    details: Option<serde_json::Value>,
) -> StepExecutionResult {
    StepExecutionResult::Blocked {
        error: DiagnosticError {
            code: code.into(),
            message,
            details,
            retriable,
        },
    }
}

fn refresh_report_findings(report: &mut DiagnosisReport) {
    report.conclusions.clear();
    report.recommendations.clear();

    let mut has_finding = false;
    for index in 0..report.steps.len() {
        let step = report.steps[index].clone();
        let finding = match (step.lifecycle, step.outcome) {
            (StepLifecycle::Blocked, _) | (StepLifecycle::Cancelled, _) => true,
            (
                StepLifecycle::Completed,
                Some(StepOutcome::Failed | StepOutcome::Warning | StepOutcome::Unverified),
            ) => true,
            _ => false,
        };
        if !finding {
            continue;
        }
        has_finding = true;
        let mut evidence_ids = step.evidence_ids.clone();
        if evidence_ids.is_empty() {
            let evidence_id = Uuid::new_v4().to_string();
            report.evidence.push(Evidence {
                id: evidence_id.clone(),
                step_id: step.id,
                kind: "step_terminal_state".into(),
                value: serde_json::json!({
                    "lifecycle": step.lifecycle,
                    "outcome": step.outcome,
                    "error": step.error,
                }),
                observed_at: Some(now_iso()),
            });
            evidence_ids.push(evidence_id);
            report.steps[index].evidence_ids = evidence_ids.clone();
        }
        let key = step_key(step.id);
        let severity = match step.lifecycle {
            StepLifecycle::Cancelled => ConclusionSeverity::Info,
            _ if step.outcome == Some(StepOutcome::Warning)
                || step.outcome == Some(StepOutcome::Unverified) =>
            {
                ConclusionSeverity::Warning
            }
            _ => ConclusionSeverity::Error,
        };
        let conclusion_id = format!("conclusion-{key}");
        let recommendation_id = format!("recommendation-{key}");
        report.conclusions.push(Conclusion {
            id: conclusion_id,
            severity,
            message: finding_message(&step),
            evidence_ids: evidence_ids.clone(),
            recommendation_ids: if severity == ConclusionSeverity::Info {
                Vec::new()
            } else {
                vec![recommendation_id.clone()]
            },
        });
        if severity != ConclusionSeverity::Info {
            let (title, action) = recommendation_for_step(step.id);
            report.recommendations.push(Recommendation {
                id: recommendation_id,
                title: title.into(),
                action: action.into(),
                evidence_ids,
            });
        }
    }

    if !has_finding
        && report.steps.iter().all(|step| {
            matches!(
                step.lifecycle,
                StepLifecycle::Completed | StepLifecycle::Skipped
            )
        })
    {
        let mut evidence_ids = report
            .evidence
            .iter()
            .map(|evidence| evidence.id.clone())
            .collect::<Vec<_>>();
        if evidence_ids.is_empty() {
            let evidence_id = Uuid::new_v4().to_string();
            report.evidence.push(Evidence {
                id: evidence_id.clone(),
                step_id: StepId::Proxy,
                kind: "diagnosis_summary".into(),
                value: serde_json::json!({ "status": "completed" }),
                observed_at: Some(now_iso()),
            });
            evidence_ids.push(evidence_id);
        }
        report.conclusions.push(Conclusion {
            id: "diagnosis-complete".into(),
            severity: ConclusionSeverity::Info,
            message: "访问链路诊断已完成，未发现步骤失败".into(),
            evidence_ids,
            recommendation_ids: Vec::new(),
        });
    }
}

fn finding_message(step: &DiagnosticStep) -> String {
    if let Some(error) = &step.error {
        return format!("{}：{}", step_label(step.id), error.message);
    }
    match (step.lifecycle, step.outcome) {
        (StepLifecycle::Blocked, _) => {
            format!("{} 被前置条件阻断，后续结论不完整", step_label(step.id))
        }
        (StepLifecycle::Cancelled, _) => format!("{} 尚未完成，诊断已取消", step_label(step.id)),
        (StepLifecycle::Completed, Some(StepOutcome::Warning)) => {
            format!("{} 存在差异或风险项，需要结合证据确认影响", step_label(step.id))
        }
        (StepLifecycle::Completed, Some(StepOutcome::Unverified)) => {
            format!("{} 缺少足够证据，当前无法验证", step_label(step.id))
        }
        (StepLifecycle::Completed, Some(StepOutcome::Failed)) => {
            format!("{} 检查失败", step_label(step.id))
        }
        _ => format!("{} 未形成完整的成功证据", step_label(step.id)),
    }
}

fn step_key(step_id: StepId) -> &'static str {
    match step_id {
        StepId::Proxy => "proxy",
        StepId::Hosts => "hosts",
        StepId::Dns => "dns",
        StepId::Tcp => "tcp",
        StepId::Tls => "tls",
        StepId::Http => "http",
    }
}

fn step_label(step_id: StepId) -> &'static str {
    match step_id {
        StepId::Proxy => "代理",
        StepId::Hosts => "Hosts",
        StepId::Dns => "DNS",
        StepId::Tcp => "TCP",
        StepId::Tls => "TLS",
        StepId::Http => "HTTP",
    }
}

fn recommendation_for_step(step_id: StepId) -> (&'static str, &'static str) {
    match step_id {
        StepId::Proxy => (
            "检查代理配置",
            "核对选定客户端画像、代理端点和 NO_PROXY 规则。",
        ),
        StepId::Hosts => ("检查 Hosts 文件", "确认目标域名映射、重复记录和格式错误。"),
        StepId::Dns => ("检查 DNS", "对比系统有效解析与指定 DNS 的结构化错误。"),
        StepId::Tcp => ("检查 TCP 连通性", "确认目标地址、端口、防火墙和路由。"),
        StepId::Tls => ("检查 TLS 证书", "核对 SNI、证书校验名、证书链和信任状态。"),
        StepId::Http => (
            "检查 HTTP 响应",
            "根据状态码、重定向和代理 CONNECT 证据继续排查。",
        ),
    }
}
fn cancelled_step_error() -> DiagnosticError {
    DiagnosticError {
        code: "diagnosis_cancelled".into(),
        message: "诊断已取消".into(),
        details: None,
        retriable: true,
    }
}

trait EventSink: Send + Sync {
    fn emit(&self, event: &DiagnosisSnapshotEvent) -> Result<(), String>;
}

struct WindowEventSink {
    window: WebviewWindow,
}

impl EventSink for WindowEventSink {
    fn emit(&self, event: &DiagnosisSnapshotEvent) -> Result<(), String> {
        self.window
            .emit(EVENT_ACCESS_PATH_DIAGNOSIS_SNAPSHOT, event)
            .map_err(|error| format!("emit diagnosis snapshot failed: {error}"))
    }
}

struct RunState {
    status: DiagnosisRunStatus,
    sequence: u64,
    report: DiagnosisReport,
}

struct RunRecord {
    run_id: String,
    owner_label: String,
    created_order: u64,
    request: DiagnosisStartRequest,
    state: Mutex<RunState>,
    cancellation: CancellationToken,
    sink: Arc<dyn EventSink>,
}

impl RunRecord {
    fn new(
        owner_label: String,
        created_order: u64,
        request: DiagnosisStartRequest,
        sink: Arc<dyn EventSink>,
    ) -> Self {
        let run_id = Uuid::new_v4().to_string();
        let steps = STEP_ORDER
            .iter()
            .copied()
            .map(|id| DiagnosticStep {
                id,
                lifecycle: StepLifecycle::Pending,
                outcome: None,
                evidence_ids: Vec::new(),
                error: None,
                started_at: None,
                finished_at: None,
            })
            .collect();
        let report = DiagnosisReport {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: Uuid::new_v4().to_string(),
            run_id: Some(run_id.clone()),
            input: request.input.clone(),
            steps,
            evidence: Vec::new(),
            conclusions: Vec::new(),
            recommendations: Vec::new(),
            started_at: now_iso(),
            finished_at: None,
        };
        Self {
            run_id,
            owner_label,
            created_order,
            request,
            state: Mutex::new(RunState {
                status: DiagnosisRunStatus::Running,
                sequence: 0,
                report,
            }),
            cancellation: CancellationToken::new(),
            sink,
        }
    }

    fn snapshot(&self) -> Result<DiagnosisRunSnapshot, String> {
        let state = self
            .state
            .lock()
            .map_err(|error| format!("diagnosis run state lock failed: {error}"))?;
        Ok(self.snapshot_from_state(&state))
    }

    fn snapshot_from_state(&self, state: &RunState) -> DiagnosisRunSnapshot {
        DiagnosisRunSnapshot {
            run_id: self.run_id.clone(),
            sequence: state.sequence,
            status: state.status,
            report: state.report.clone(),
        }
    }

    fn publish_initial(&self) -> Result<(), String> {
        self.mutate(|_| true).map(|_| ())
    }

    fn mutate(
        &self,
        update: impl FnOnce(&mut RunState) -> bool,
    ) -> Result<Option<DiagnosisRunSnapshot>, String> {
        let event = {
            let mut state = self
                .state
                .lock()
                .map_err(|error| format!("diagnosis run state lock failed: {error}"))?;
            if !update(&mut state) {
                return Ok(None);
            }
            state.sequence = state.sequence.saturating_add(1);
            let snapshot = self.snapshot_from_state(&state);
            DiagnosisSnapshotEvent {
                run_id: self.run_id.clone(),
                sequence: state.sequence,
                snapshot,
            }
        };
        if let Err(error) = self.sink.emit(&event) {
            eprintln!("[access-path-diagnostics] {error}");
        }
        Ok(Some(event.snapshot))
    }

    fn mark_step_running(&self, step_id: StepId) -> Result<bool, String> {
        self.mutate(|state| {
            if state.status != DiagnosisRunStatus::Running {
                return false;
            }
            let Some(step) = state
                .report
                .steps
                .iter_mut()
                .find(|step| step.id == step_id)
            else {
                return false;
            };
            if step.lifecycle != StepLifecycle::Pending {
                return false;
            }
            step.lifecycle = StepLifecycle::Running;
            step.started_at = Some(now_iso());
            true
        })
        .map(|snapshot| snapshot.is_some())
    }

    fn finish_step(&self, step_id: StepId, result: StepExecutionResult) -> Result<bool, String> {
        self.mutate(|state| {
            if state.status != DiagnosisRunStatus::Running {
                return false;
            }
            let Some(step_index) = state
                .report
                .steps
                .iter()
                .position(|step| step.id == step_id)
            else {
                return false;
            };
            if state.report.steps[step_index].lifecycle != StepLifecycle::Running {
                return false;
            }
            let step = &mut state.report.steps[step_index];
            step.finished_at = Some(now_iso());
            match result {
                StepExecutionResult::Completed {
                    outcome,
                    evidence,
                    error,
                } => {
                    step.lifecycle = StepLifecycle::Completed;
                    step.outcome = Some(outcome);
                    step.evidence_ids = evidence.iter().map(|item| item.id.clone()).collect();
                    step.error = error;
                    state.report.evidence.extend(evidence);
                }
                StepExecutionResult::Blocked { error } => {
                    step.lifecycle = StepLifecycle::Blocked;
                    step.error = Some(error);
                }
                StepExecutionResult::Skipped { error } => {
                    step.lifecycle = StepLifecycle::Skipped;
                    step.error = Some(error);
                }
            }
            true
        })
        .map(|snapshot| snapshot.is_some())
    }

    fn request_cancel(&self) -> Result<DiagnosisCancelResponse, String> {
        self.cancellation.cancel();
        let changed = self
            .mutate(|state| {
                if state.status.is_terminal() {
                    return false;
                }
                state.status = DiagnosisRunStatus::Cancelled;
                state.report.finished_at = Some(now_iso());
                for step in &mut state.report.steps {
                    if matches!(
                        step.lifecycle,
                        StepLifecycle::Pending | StepLifecycle::Running
                    ) {
                        step.lifecycle = StepLifecycle::Cancelled;
                        step.outcome = None;
                        step.finished_at = Some(now_iso());
                        step.error = Some(DiagnosticError {
                            code: "diagnosis_cancelled".into(),
                            message: "诊断已取消".into(),
                            details: None,
                            retriable: true,
                        });
                    }
                }
                refresh_report_findings(&mut state.report);
                true
            })?
            .is_some();
        Ok(DiagnosisCancelResponse {
            run_id: self.run_id.clone(),
            cancelled: changed,
            snapshot: self.snapshot()?,
        })
    }

    fn finalize_completed(&self) -> Result<(), String> {
        self.mutate(|state| {
            if state.status != DiagnosisRunStatus::Running {
                return false;
            }
            state.status = DiagnosisRunStatus::Completed;
            state.report.finished_at = Some(now_iso());
            refresh_report_findings(&mut state.report);
            true
        })
        .map(|_| ())
    }

    fn finalize_timed_out(&self) -> Result<(), String> {
        self.cancellation.cancel();
        self.mutate(|state| {
            if state.status != DiagnosisRunStatus::Running {
                return false;
            }
            state.status = DiagnosisRunStatus::TimedOut;
            state.report.finished_at = Some(now_iso());
            for step in &mut state.report.steps {
                match step.lifecycle {
                    StepLifecycle::Running => {
                        step.lifecycle = StepLifecycle::Completed;
                        step.outcome = Some(StepOutcome::Failed);
                        step.finished_at = Some(now_iso());
                        step.error = Some(DiagnosticError {
                            code: "diagnosis_timeout".into(),
                            message: "诊断超过整体超时限制".into(),
                            details: None,
                            retriable: true,
                        });
                    }
                    StepLifecycle::Pending => {
                        step.lifecycle = StepLifecycle::Blocked;
                        step.error = Some(DiagnosticError {
                            code: "diagnosis_timeout".into(),
                            message: "诊断整体超时，步骤未执行".into(),
                            details: None,
                            retriable: true,
                        });
                    }
                    _ => {}
                }
            }
            refresh_report_findings(&mut state.report);
            true
        })
        .map(|_| ())
    }

    fn finalize_failed(&self, message: String) -> Result<(), String> {
        self.cancellation.cancel();
        self.mutate(|state| {
            if state.status != DiagnosisRunStatus::Running {
                return false;
            }
            state.status = DiagnosisRunStatus::Failed;
            state.report.finished_at = Some(now_iso());
            for step in &mut state.report.steps {
                if matches!(
                    step.lifecycle,
                    StepLifecycle::Pending | StepLifecycle::Running
                ) {
                    step.lifecycle = StepLifecycle::Blocked;
                    step.finished_at = Some(now_iso());
                    step.error = Some(DiagnosticError {
                        code: "runtime_failed".into(),
                        message: message.clone(),
                        details: None,
                        retriable: true,
                    });
                }
            }
            refresh_report_findings(&mut state.report);
            true
        })
        .map(|_| ())
    }
}

enum ExecutionExit {
    Completed,
    Cancelled,
}

async fn execute_steps(
    run: Arc<RunRecord>,
    executor: Arc<dyn StepExecutor>,
) -> Result<ExecutionExit, String> {
    for step_id in STEP_ORDER {
        if run.cancellation.is_cancelled() {
            return Ok(ExecutionExit::Cancelled);
        }
        if !run.mark_step_running(step_id)? {
            return Ok(ExecutionExit::Cancelled);
        }

        let step_cancellation = run.cancellation.child_token();
        let execution = executor.execute(step_id, run.request.clone(), step_cancellation.clone());
        let result = tokio::select! {
            biased;
            _ = run.cancellation.cancelled() => {
                step_cancellation.cancel();
                return Ok(ExecutionExit::Cancelled);
            }
            result = tokio::time::timeout(
                Duration::from_millis(run.request.step_timeout_ms),
                execution,
            ) => {
                match result {
                    Ok(result) => result,
                    Err(_) => {
                        step_cancellation.cancel();
                        StepExecutionResult::Completed {
                            outcome: StepOutcome::Failed,
                            evidence: Vec::new(),
                            error: Some(DiagnosticError {
                                code: "step_timeout".into(),
                                message: format!("{step_id:?} 步骤超时"),
                                details: None,
                                retriable: true,
                            }),
                        }
                    }
                }
            }
        };
        if !run.finish_step(step_id, result)? {
            return Ok(ExecutionExit::Cancelled);
        }
    }
    Ok(ExecutionExit::Completed)
}

async fn execute_run(run: Arc<RunRecord>, executor: Arc<dyn StepExecutor>) {
    let result = tokio::time::timeout(
        Duration::from_millis(run.request.overall_timeout_ms),
        execute_steps(Arc::clone(&run), executor),
    )
    .await;
    let finalized = match result {
        Err(_) => run.finalize_timed_out(),
        Ok(Ok(ExecutionExit::Completed)) => run.finalize_completed(),
        Ok(Ok(ExecutionExit::Cancelled)) => Ok(()),
        Ok(Err(error)) => run.finalize_failed(error),
    };
    if let Err(error) = finalized {
        eprintln!("[access-path-diagnostics] finalize run failed: {error}");
    }
}

struct RegistryState {
    next_order: u64,
    runs: HashMap<String, Arc<RunRecord>>,
}

pub(crate) struct DiagnosisRuntime {
    registry: Mutex<RegistryState>,
    executor: Arc<dyn StepExecutor>,
}

impl DiagnosisRuntime {
    fn new(executor: Arc<dyn StepExecutor>) -> Self {
        Self {
            registry: Mutex::new(RegistryState {
                next_order: 1,
                runs: HashMap::new(),
            }),
            executor,
        }
    }

    fn start(
        &self,
        owner_label: String,
        request: DiagnosisStartRequest,
        sink: Arc<dyn EventSink>,
    ) -> Result<DiagnosisStartResponse, String> {
        validate_request(&request)?;
        let run = {
            let mut registry = self
                .registry
                .lock()
                .map_err(|error| format!("diagnosis registry lock failed: {error}"))?;
            prune_terminal_runs(&mut registry)?;
            let mut active_count = 0;
            for run in registry.runs.values() {
                if !run.snapshot()?.status.is_terminal() {
                    active_count += 1;
                }
            }
            if active_count >= MAX_ACTIVE_RUNS {
                return Err(format!(
                    "too many active diagnosis runs (limit {MAX_ACTIVE_RUNS})"
                ));
            }
            let created_order = registry.next_order;
            registry.next_order = registry.next_order.saturating_add(1);
            let run = Arc::new(RunRecord::new(owner_label, created_order, request, sink));
            registry.runs.insert(run.run_id.clone(), Arc::clone(&run));
            run
        };
        run.publish_initial()?;
        let task_run = Arc::clone(&run);
        let executor = Arc::clone(&self.executor);
        tokio::spawn(async move {
            execute_run(task_run, executor).await;
        });
        Ok(DiagnosisStartResponse {
            run_id: run.run_id.clone(),
        })
    }

    fn get(&self, owner_label: &str, run_id: &str) -> Result<DiagnosisRunSnapshot, String> {
        let run = self.find_owned(owner_label, run_id)?;
        run.snapshot()
    }

    fn cancel(&self, owner_label: &str, run_id: &str) -> Result<DiagnosisCancelResponse, String> {
        let run = self.find_owned(owner_label, run_id)?;
        run.request_cancel()
    }

    fn find_owned(&self, owner_label: &str, run_id: &str) -> Result<Arc<RunRecord>, String> {
        let registry = self
            .registry
            .lock()
            .map_err(|error| format!("diagnosis registry lock failed: {error}"))?;
        let run = registry
            .runs
            .get(run_id)
            .filter(|run| run.owner_label == owner_label)
            .cloned()
            .ok_or_else(|| "diagnosis run not found".to_string())?;
        Ok(run)
    }

    fn remove_owner(&self, owner_label: &str) {
        let runs = match self.registry.lock() {
            Ok(mut registry) => {
                let ids = registry
                    .runs
                    .iter()
                    .filter(|(_, run)| run.owner_label == owner_label)
                    .map(|(run_id, _)| run_id.clone())
                    .collect::<Vec<_>>();
                ids.into_iter()
                    .filter_map(|run_id| registry.runs.remove(&run_id))
                    .collect::<Vec<_>>()
            }
            Err(error) => {
                eprintln!("[access-path-diagnostics] registry cleanup failed: {error}");
                return;
            }
        };
        for run in runs {
            let _ = run.request_cancel();
        }
    }

    fn cancel_all(&self) {
        let runs = match self.registry.lock() {
            Ok(mut registry) => registry
                .runs
                .drain()
                .map(|(_, run)| run)
                .collect::<Vec<_>>(),
            Err(error) => {
                eprintln!("[access-path-diagnostics] registry cleanup failed: {error}");
                return;
            }
        };
        for run in runs {
            let _ = run.request_cancel();
        }
    }
}

fn validate_request(request: &DiagnosisStartRequest) -> Result<(), String> {
    if !(MIN_OVERALL_TIMEOUT_MS..=MAX_OVERALL_TIMEOUT_MS).contains(&request.overall_timeout_ms) {
        return Err(format!(
            "overallTimeoutMs must be {MIN_OVERALL_TIMEOUT_MS}..={MAX_OVERALL_TIMEOUT_MS}"
        ));
    }
    if !(MIN_STEP_TIMEOUT_MS..=MAX_STEP_TIMEOUT_MS).contains(&request.step_timeout_ms) {
        return Err(format!(
            "stepTimeoutMs must be {MIN_STEP_TIMEOUT_MS}..={MAX_STEP_TIMEOUT_MS}"
        ));
    }
    if request.dns_servers.len() > 8 {
        return Err("dnsServers supports at most 8 entries".into());
    }
    Ok(())
}

fn prune_terminal_runs(registry: &mut RegistryState) -> Result<(), String> {
    let mut terminal_runs = Vec::new();
    for (run_id, run) in &registry.runs {
        if run.snapshot()?.status.is_terminal() {
            terminal_runs.push((run.created_order, run_id.clone()));
        }
    }
    if terminal_runs.len() <= MAX_RETAINED_TERMINAL_RUNS {
        return Ok(());
    }
    terminal_runs.sort_by_key(|(order, _)| *order);
    let remove_count = terminal_runs.len() - MAX_RETAINED_TERMINAL_RUNS;
    for (_, run_id) in terminal_runs.into_iter().take(remove_count) {
        registry.runs.remove(&run_id);
    }
    Ok(())
}

static RUNTIME: LazyLock<DiagnosisRuntime> =
    LazyLock::new(|| DiagnosisRuntime::new(Arc::new(EnvironmentStepExecutor)));

#[tauri::command]
pub async fn diagnosis_start(
    window: WebviewWindow,
    request: DiagnosisStartRequest,
) -> Result<DiagnosisStartResponse, String> {
    RUNTIME.start(
        window.label().to_string(),
        request,
        Arc::new(WindowEventSink { window }),
    )
}

#[tauri::command]
pub async fn diagnosis_get(
    window: WebviewWindow,
    run_id: String,
) -> Result<DiagnosisRunSnapshot, String> {
    RUNTIME.get(window.label(), &run_id)
}

#[tauri::command]
pub async fn diagnosis_cancel(
    window: WebviewWindow,
    run_id: String,
) -> Result<DiagnosisCancelResponse, String> {
    RUNTIME.cancel(window.label(), &run_id)
}

pub fn on_window_closed(owner_label: &str) {
    RUNTIME.remove_owner(owner_label);
}

pub fn on_app_exit() {
    RUNTIME.cancel_all();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::access_path_diagnostics::model::{AccessPathTargetKind, AccessProtocol};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[derive(Default)]
    struct RecordingSink {
        events: Mutex<Vec<DiagnosisSnapshotEvent>>,
    }

    impl EventSink for RecordingSink {
        fn emit(&self, event: &DiagnosisSnapshotEvent) -> Result<(), String> {
            self.events.lock().unwrap().push(event.clone());
            Ok(())
        }
    }

    struct DelaySuccessExecutor {
        delay: Duration,
    }

    impl StepExecutor for DelaySuccessExecutor {
        fn execute(
            &self,
            _step_id: StepId,
            _request: DiagnosisStartRequest,
            cancellation: CancellationToken,
        ) -> StepFuture {
            let delay = self.delay;
            Box::pin(async move {
                tokio::select! {
                    _ = cancellation.cancelled() => StepExecutionResult::Blocked {
                        error: DiagnosticError {
                            code: "cancelled".into(),
                            message: "cancelled".into(),
                            details: None,
                            retriable: true,
                        }
                    },
                    _ = tokio::time::sleep(delay) => StepExecutionResult::Completed {
                        outcome: StepOutcome::Success,
                        evidence: Vec::new(),
                        error: None,
                    }
                }
            })
        }
    }

    struct FirstFastExecutor;

    impl StepExecutor for FirstFastExecutor {
        fn execute(
            &self,
            step_id: StepId,
            _request: DiagnosisStartRequest,
            cancellation: CancellationToken,
        ) -> StepFuture {
            let delay = if step_id == StepId::Proxy {
                Duration::from_millis(2)
            } else {
                Duration::from_millis(200)
            };
            Box::pin(async move {
                tokio::select! {
                    _ = cancellation.cancelled() => StepExecutionResult::Blocked {
                        error: DiagnosticError {
                            code: "cancelled".into(),
                            message: "cancelled".into(),
                            details: None,
                            retriable: true,
                        }
                    },
                    _ = tokio::time::sleep(delay) => StepExecutionResult::Completed {
                        outcome: StepOutcome::Success,
                        evidence: Vec::new(),
                        error: None,
                    }
                }
            })
        }
    }

    fn target() -> NormalizedAccessPathTarget {
        NormalizedAccessPathTarget {
            raw_input: "example.test".into(),
            protocol: AccessProtocol::Https,
            hostname: "example.test".into(),
            target_kind: AccessPathTargetKind::Hostname,
            port: 443,
            path: "/".into(),
            url: "https://example.test/".into(),
            sni: Some("example.test".into()),
            verify_hostname: Some("example.test".into()),
            http_host: "example.test".into(),
            connection_ip: None,
        }
    }

    fn request(overall_timeout_ms: u64, step_timeout_ms: u64) -> DiagnosisStartRequest {
        DiagnosisStartRequest {
            input: target(),
            overall_timeout_ms,
            step_timeout_ms,
            dns_servers: Vec::new(),
            proxy_profile: DiagnosisProxyProfile::Auto,
        }
    }

    async fn wait_terminal(
        runtime: &DiagnosisRuntime,
        owner: &str,
        run_id: &str,
    ) -> DiagnosisRunSnapshot {
        for _ in 0..400 {
            let snapshot = runtime.get(owner, run_id).expect("get run");
            if snapshot.status.is_terminal() {
                return snapshot;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("diagnosis run did not finish");
    }

    #[tokio::test]
    async fn sequences_are_monotonic_and_get_recovers_final_snapshot() {
        let runtime = DiagnosisRuntime::new(Arc::new(DelaySuccessExecutor {
            delay: Duration::from_millis(1),
        }));
        let sink = Arc::new(RecordingSink::default());
        let response = runtime
            .start("main".into(), request(1_000, 100), sink.clone())
            .expect("start run");
        let snapshot = wait_terminal(&runtime, "main", &response.run_id).await;
        assert_eq!(snapshot.status, DiagnosisRunStatus::Completed);
        assert!(snapshot
            .report
            .steps
            .iter()
            .all(|step| step.outcome == Some(StepOutcome::Success)));

        let events = sink.events.lock().unwrap();
        assert!(events.len() >= 14);
        assert!(events
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence));
        assert_eq!(events.last().unwrap().snapshot.sequence, snapshot.sequence);
        let wire = serde_json::to_value(events.last().unwrap()).expect("serialize event");
        assert_eq!(wire["runId"], response.run_id);
        assert_eq!(wire["snapshot"]["status"], "completed");
        assert_eq!(wire["snapshot"]["sequence"], snapshot.sequence);
    }

    #[tokio::test]
    async fn manual_direct_proxy_profile_produces_explicit_evidence() {
        let mut request = request(1_000, 100);
        request.proxy_profile = DiagnosisProxyProfile::Direct;
        let result = execute_proxy_step(request, CancellationToken::new()).await;
        match result {
            StepExecutionResult::Completed {
                outcome,
                evidence,
                error,
            } => {
                assert_eq!(outcome, StepOutcome::Success);
                assert!(error.is_none());
                assert_eq!(evidence.len(), 1);
                assert_eq!(evidence[0].kind, "proxy_manual_direct");
                assert_eq!(evidence[0].value["configurationSource"], "user_override");
            }
            _ => panic!("direct proxy profile must complete with evidence"),
        }
    }

    #[tokio::test]
    async fn direct_tcp_step_probes_target_and_labels_evidence() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let accept = tokio::spawn(async move { listener.accept().await });
        let mut request = request(1_000, 500);
        request.proxy_profile = DiagnosisProxyProfile::Direct;
        request.input.hostname = "127.0.0.1".into();
        request.input.target_kind = AccessPathTargetKind::Ipv4;
        request.input.port = address.port();
        request.input.connection_ip = Some("127.0.0.1".into());

        let result = execute_tcp_step(request, CancellationToken::new()).await;
        match result {
            StepExecutionResult::Completed {
                outcome,
                evidence,
                error,
            } => {
                assert_eq!(outcome, StepOutcome::Success);
                assert!(error.is_none());
                assert_eq!(evidence.len(), 1);
                assert_eq!(evidence[0].kind, "tcp_probe");
                assert_eq!(evidence[0].value["route"], "direct");
                assert_eq!(evidence[0].value["destinationRole"], "target");
                assert_eq!(
                    evidence[0].value["result"]["attempts"][0]["status"],
                    "connected"
                );
            }
            _ => panic!("direct TCP probe must complete with evidence"),
        }
        accept.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn direct_http_step_records_real_response_without_credentials() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0u8; 4096];
            let size = stream.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..size]);
            assert!(request.starts_with("HEAD /health HTTP/1.1\r\n"));
            assert!(!request.to_ascii_lowercase().contains("authorization:"));
            assert!(!request.to_ascii_lowercase().contains("cookie:"));
            stream
                .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
        });
        let mut request = request(1_000, 500);
        request.proxy_profile = DiagnosisProxyProfile::Direct;
        request.input.protocol = AccessProtocol::Http;
        request.input.hostname = "127.0.0.1".into();
        request.input.target_kind = AccessPathTargetKind::Ipv4;
        request.input.port = address.port();
        request.input.path = "/health".into();
        request.input.url = format!("http://127.0.0.1:{}/health", address.port());
        request.input.sni = None;
        request.input.http_host = format!("127.0.0.1:{}", address.port());
        request.input.connection_ip = Some("127.0.0.1".into());

        let result = execute_http_step(request, CancellationToken::new()).await;
        match result {
            StepExecutionResult::Completed {
                outcome,
                evidence,
                error,
            } => {
                assert_eq!(outcome, StepOutcome::Success);
                assert!(error.is_none());
                assert_eq!(evidence[0].kind, "http_probe");
                assert_eq!(evidence[0].value["route"], "direct");
                assert_eq!(evidence[0].value["result"]["finalStatus"], 204);
            }
            _ => panic!("direct HTTP probe must complete with response evidence"),
        }
        server.await.unwrap();
    }

    #[test]
    fn cancelled_blocking_http_probe_does_not_open_or_write_a_request() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let mut limits = HttpProbeLimits::default();
        limits.timeout = Duration::from_millis(100);

        let probe = probe_http_direct_blocking(
            Url::parse(&format!("http://127.0.0.1:{}/", address.port())).unwrap(),
            Some(format!("127.0.0.1:{}", address.port())),
            Some(address.ip()),
            None,
            None,
            limits,
            cancellation,
        );

        assert_eq!(probe.result.unwrap_err().code, "diagnosis_cancelled");
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[tokio::test]
    async fn http_target_skips_tls_step() {
        let mut request = request(1_000, 100);
        request.input.protocol = AccessProtocol::Http;
        let result = execute_tls_step(request, CancellationToken::new()).await;
        match result {
            StepExecutionResult::Skipped { error } => {
                assert_eq!(error.code, "tls_not_applicable");
            }
            _ => panic!("HTTP target must skip TLS"),
        }
    }

    #[test]
    fn initial_tls_names_keep_sni_and_hostname_verification_independent() {
        let mut target = target();
        target.sni = Some("routing.example.test".into());
        target.verify_hostname = Some("certificate.example.test".into());

        let (sni, verify_hostname) = initial_tls_names(&target);

        assert_eq!(sni.as_deref(), Some("routing.example.test"));
        assert_eq!(verify_hostname.as_deref(), Some("certificate.example.test"));
    }

    #[test]
    fn initial_tls_names_fall_back_to_url_hostname_for_legacy_payloads() {
        let mut target = target();
        target.sni = Some("routing.example.test".into());
        target.verify_hostname = None;

        let (sni, verify_hostname) = initial_tls_names(&target);

        assert_eq!(sni.as_deref(), Some("routing.example.test"));
        assert_eq!(verify_hostname.as_deref(), Some("example.test"));
    }

    #[test]
    fn forward_proxy_connection_override_is_explicitly_unsupported_only_for_http() {
        let connection_ip = Some("192.0.2.10".parse().unwrap());

        assert!(forward_proxy_connection_override_is_unsupported(
            &Url::parse("http://example.test/health").unwrap(),
            connection_ip
        ));
        assert!(!forward_proxy_connection_override_is_unsupported(
            &Url::parse("https://example.test/health").unwrap(),
            connection_ip
        ));
        assert!(!forward_proxy_connection_override_is_unsupported(
            &Url::parse("http://example.test/health").unwrap(),
            None
        ));
    }

    #[test]
    fn partial_proxy_connect_errors_only_downgrade_successful_tls_outcome() {
        assert_eq!(
            tls_outcome_with_proxy_connect_errors(StepOutcome::Success, 1),
            StepOutcome::Warning
        );
        assert_eq!(
            tls_outcome_with_proxy_connect_errors(StepOutcome::Failed, 1),
            StepOutcome::Failed
        );
        assert_eq!(
            tls_outcome_with_proxy_connect_errors(StepOutcome::Unverified, 1),
            StepOutcome::Unverified
        );
    }

    #[test]
    fn proxy_tls_evidence_distinguishes_proxy_destination_from_target_host() {
        let evidence = TlsTransportEvidence {
            route: "proxy".into(),
            destination_role: "proxy".into(),
            destination_host: "proxy.example.test".into(),
            target_host: "origin.example.test".into(),
            connection_ip_role: "proxy_peer".into(),
            configuration_source: "environment".into(),
            proxy_endpoint: Some("http://proxy.example.test".into()),
            target_authority: Some("origin.example.test:443".into()),
            proxy_connect: Vec::new(),
            proxy_connect_errors: Vec::new(),
            results: Vec::new(),
        };
        let value = serde_json::to_value(evidence).unwrap();

        assert_eq!(value["destinationHost"], "proxy.example.test");
        assert_eq!(value["targetHost"], "origin.example.test");
    }

    #[test]
    fn initial_tls_failure_blocks_http_without_synthetic_status() {
        let result = http_step_result(
            "user_override".into(),
            None,
            "target".into(),
            HttpBlockingProbe {
                result: Err(HttpProbeError::new(
                    "tls_transport_failed",
                    "certificate mismatch",
                )),
                proxy_connect: Vec::new(),
                proxy_connect_errors: Vec::new(),
                tls_results: Vec::new(),
                initial_proxy_connect_failed: false,
            },
        );
        match result {
            StepExecutionResult::Blocked { error } => {
                assert_eq!(error.code, "http_blocked_by_tls");
                assert!(error.details.is_some());
            }
            _ => panic!("TLS failure before request must block HTTP"),
        }
    }

    #[test]
    fn arbitrary_initial_connect_failure_blocks_http_before_any_request() {
        let result = http_step_result(
            "environment".into(),
            Some("http://proxy.example.test".into()),
            "proxy_peer".into(),
            HttpBlockingProbe {
                result: Err(HttpProbeError::new(
                    "response_headers_too_large",
                    "CONNECT 响应头超过限制",
                )),
                proxy_connect: Vec::new(),
                proxy_connect_errors: vec![HttpProbeError::new(
                    "response_headers_too_large",
                    "CONNECT 响应头超过限制",
                )],
                tls_results: Vec::new(),
                initial_proxy_connect_failed: true,
            },
        );

        match result {
            StepExecutionResult::Blocked { error } => {
                assert_eq!(error.code, "http_blocked_by_proxy_connect");
            }
            _ => panic!("initial CONNECT failure must block HTTP"),
        }
    }

    #[test]
    fn proxy_probe_target_uses_proxy_endpoint_instead_of_connection_override() {
        let endpoint = SanitizedProxyEndpoint {
            url: "http://proxy.example.test".into(),
            scheme: "http".into(),
            host: "proxy.example.test".into(),
            port: None,
            credentials_redacted: false,
        };
        let mut original = target();
        original.connection_ip = Some("192.0.2.10".into());
        let derived = proxy_probe_target(&original, &endpoint).expect("derive proxy target");
        assert_eq!(derived.hostname, "proxy.example.test");
        assert_eq!(derived.port, 80);
        assert_eq!(derived.target_kind, AccessPathTargetKind::Hostname);
        assert!(derived.connection_ip.is_none());
        assert!(derived.sni.is_none());
    }

    #[tokio::test]
    async fn ip_target_without_bypass_server_skips_dns_instead_of_marking_unverified() {
        let mut request = request(1_000, 100);
        request.input.hostname = "192.0.2.10".into();
        request.input.target_kind = AccessPathTargetKind::Ipv4;
        let result = execute_dns_step(request, CancellationToken::new()).await;
        match result {
            StepExecutionResult::Skipped { error } => {
                assert_eq!(error.code, "dns_not_applicable");
            }
            _ => panic!("IP target without bypass DNS must be skipped"),
        }
    }

    #[test]
    fn start_request_defaults_to_auto_proxy_and_no_bypass_dns() {
        let parsed: DiagnosisStartRequest = serde_json::from_value(serde_json::json!({
            "input": target()
        }))
        .expect("deserialize start request");
        assert_eq!(parsed.proxy_profile, DiagnosisProxyProfile::Auto);
        assert!(parsed.dns_servers.is_empty());
        assert_eq!(parsed.overall_timeout_ms, DEFAULT_OVERALL_TIMEOUT_MS);
        assert_eq!(parsed.step_timeout_ms, DEFAULT_STEP_TIMEOUT_MS);
    }

    #[tokio::test]
    async fn repeated_cancel_is_idempotent_and_preserves_completed_steps() {
        let runtime = DiagnosisRuntime::new(Arc::new(FirstFastExecutor));
        let sink = Arc::new(RecordingSink::default());
        let response = runtime
            .start("main".into(), request(2_000, 500), sink)
            .expect("start run");
        loop {
            let snapshot = runtime.get("main", &response.run_id).unwrap();
            if snapshot.report.steps[0].lifecycle == StepLifecycle::Completed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }

        let first = runtime
            .cancel("main", &response.run_id)
            .expect("cancel run");
        let repeated = runtime
            .cancel("main", &response.run_id)
            .expect("repeat cancel");
        assert!(first.cancelled);
        assert!(!repeated.cancelled);
        assert_eq!(repeated.snapshot.status, DiagnosisRunStatus::Cancelled);
        assert_eq!(
            repeated.snapshot.report.steps[0].lifecycle,
            StepLifecycle::Completed
        );
        assert!(repeated.snapshot.report.steps[1..]
            .iter()
            .all(|step| step.lifecycle == StepLifecycle::Cancelled));
        assert!(!repeated.snapshot.report.conclusions.is_empty());
        assert!(repeated
            .snapshot
            .report
            .conclusions
            .iter()
            .all(|item| !item.evidence_ids.is_empty()));
    }

    #[tokio::test]
    async fn step_timeout_is_a_failed_step_without_fake_success() {
        let runtime = DiagnosisRuntime::new(Arc::new(DelaySuccessExecutor {
            delay: Duration::from_millis(50),
        }));
        let response = runtime
            .start(
                "main".into(),
                request(1_000, 10),
                Arc::new(RecordingSink::default()),
            )
            .expect("start run");
        let snapshot = wait_terminal(&runtime, "main", &response.run_id).await;
        assert_eq!(snapshot.status, DiagnosisRunStatus::Completed);
        assert!(snapshot.report.steps.iter().all(|step| {
            step.lifecycle == StepLifecycle::Completed
                && step.outcome == Some(StepOutcome::Failed)
                && step.error.as_ref().map(|error| error.code.as_str()) == Some("step_timeout")
        }));
        assert!(!snapshot.report.conclusions.is_empty());
        assert!(snapshot
            .report
            .conclusions
            .iter()
            .all(|item| item.message.contains("步骤超时")));
        assert!(snapshot
            .report
            .recommendations
            .iter()
            .all(|item| !item.evidence_ids.is_empty()));
    }

    #[tokio::test]
    async fn overall_timeout_blocks_pending_steps_and_discards_late_results() {
        let runtime = DiagnosisRuntime::new(Arc::new(DelaySuccessExecutor {
            delay: Duration::from_millis(100),
        }));
        let sink = Arc::new(RecordingSink::default());
        let response = runtime
            .start("main".into(), request(20, 500), sink.clone())
            .expect("start run");
        let snapshot = wait_terminal(&runtime, "main", &response.run_id).await;
        assert_eq!(snapshot.status, DiagnosisRunStatus::TimedOut);
        assert_eq!(snapshot.report.steps[0].outcome, Some(StepOutcome::Failed));
        assert!(snapshot.report.steps[1..]
            .iter()
            .all(|step| step.lifecycle == StepLifecycle::Blocked));
        let final_sequence = snapshot.sequence;
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert_eq!(
            runtime.get("main", &response.run_id).unwrap().sequence,
            final_sequence
        );
    }

    #[tokio::test]
    async fn concurrent_runs_are_isolated_and_owner_cleanup_removes_only_its_runs() {
        let runtime = DiagnosisRuntime::new(Arc::new(DelaySuccessExecutor {
            delay: Duration::from_millis(10),
        }));
        let first = runtime
            .start(
                "main".into(),
                request(1_000, 200),
                Arc::new(RecordingSink::default()),
            )
            .unwrap();
        let second = runtime
            .start(
                "other".into(),
                request(1_000, 200),
                Arc::new(RecordingSink::default()),
            )
            .unwrap();

        assert!(runtime.get("main", &second.run_id).is_err());
        runtime.remove_owner("main");
        assert!(runtime.get("main", &first.run_id).is_err());
        let second_snapshot = wait_terminal(&runtime, "other", &second.run_id).await;
        assert_eq!(second_snapshot.status, DiagnosisRunStatus::Completed);
        runtime.cancel_all();
        assert!(runtime.get("other", &second.run_id).is_err());
    }

    #[tokio::test]
    async fn app_exit_cleanup_cancels_and_removes_active_runs() {
        let runtime = DiagnosisRuntime::new(Arc::new(DelaySuccessExecutor {
            delay: Duration::from_millis(100),
        }));
        let response = runtime
            .start(
                "main".into(),
                request(1_000, 500),
                Arc::new(RecordingSink::default()),
            )
            .unwrap();

        runtime.cancel_all();
        assert!(runtime.get("main", &response.run_id).is_err());
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert!(runtime.get("main", &response.run_id).is_err());
    }
    #[tokio::test]
    async fn old_terminal_runs_are_pruned_without_removing_recent_snapshots() {
        let runtime = DiagnosisRuntime::new(Arc::new(DelaySuccessExecutor {
            delay: Duration::from_millis(1),
        }));
        let mut run_ids = Vec::new();
        for _ in 0..(MAX_RETAINED_TERMINAL_RUNS + 2) {
            let response = runtime
                .start(
                    "main".into(),
                    request(1_000, 100),
                    Arc::new(RecordingSink::default()),
                )
                .unwrap();
            wait_terminal(&runtime, "main", &response.run_id).await;
            run_ids.push(response.run_id);
        }

        assert!(runtime.get("main", &run_ids[0]).is_err());
        assert!(runtime.get("main", run_ids.last().unwrap()).is_ok());
    }
}

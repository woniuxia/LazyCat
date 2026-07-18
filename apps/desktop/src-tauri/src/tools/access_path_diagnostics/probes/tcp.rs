use std::collections::HashSet;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use super::super::model::{AccessPathTargetKind, NormalizedAccessPathTarget};

pub const MAX_TCP_PROBE_CONCURRENCY: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TcpAddressFamily {
    Ipv4,
    Ipv6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TcpProbeStatus {
    Connected,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TcpProbeErrorKind {
    Refused,
    Timeout,
    NetworkUnreachable,
    HostUnreachable,
    AddressUnavailable,
    TransportError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpProbeError {
    pub kind: TcpProbeErrorKind,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_os_error: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpProbeAttempt {
    pub ip: IpAddr,
    pub address_family: TcpAddressFamily,
    pub port: u16,
    pub socket_address: String,
    pub duration_ms: u64,
    pub status: TcpProbeStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<TcpProbeError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpProbeResult {
    pub port: u16,
    pub attempts: Vec<TcpProbeAttempt>,
    pub cancelled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpProbeInputError {
    pub code: String,
    pub message: String,
}

/// Selects the actual TCP destinations. A connection IP is an explicit override
/// and must not be combined with DNS-derived candidates.
pub fn select_probe_addresses(
    target: &NormalizedAccessPathTarget,
    candidate_ips: &[IpAddr],
) -> Result<Vec<IpAddr>, TcpProbeInputError> {
    if let Some(connection_ip) = target.connection_ip.as_deref() {
        return connection_ip
            .parse::<IpAddr>()
            .map(|ip| vec![ip])
            .map_err(|error| TcpProbeInputError {
                code: "invalid_connection_ip".into(),
                message: format!("连接 IP 无效: {error}"),
            });
    }

    let mut seen = HashSet::new();
    let mut addresses = candidate_ips
        .iter()
        .copied()
        .filter(|ip| seen.insert(*ip))
        .collect::<Vec<_>>();

    if addresses.is_empty()
        && matches!(
            target.target_kind,
            AccessPathTargetKind::Ipv4 | AccessPathTargetKind::Ipv6
        )
    {
        let ip = target
            .hostname
            .parse::<IpAddr>()
            .map_err(|error| TcpProbeInputError {
                code: "invalid_target_ip".into(),
                message: format!("目标 IP 无效: {error}"),
            })?;
        addresses.push(ip);
    }

    Ok(addresses)
}

/// Probes every selected address with a bounded amount of concurrency. Attempts
/// are returned in candidate order even when connections finish out of order.
pub async fn probe_tcp(
    target: &NormalizedAccessPathTarget,
    candidate_ips: &[IpAddr],
    per_address_timeout: Duration,
    cancellation: CancellationToken,
) -> Result<TcpProbeResult, TcpProbeInputError> {
    let addresses = select_probe_addresses(target, candidate_ips)?;
    let semaphore = Arc::new(Semaphore::new(MAX_TCP_PROBE_CONCURRENCY));
    let mut tasks = JoinSet::new();

    for (index, ip) in addresses.iter().copied().enumerate() {
        let semaphore = semaphore.clone();
        let cancellation = cancellation.clone();
        let port = target.port;
        tasks.spawn(async move {
            let attempt = probe_one(ip, port, per_address_timeout, cancellation, semaphore).await;
            (index, attempt)
        });
    }

    let mut attempts = Vec::with_capacity(addresses.len());
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(attempt) => attempts.push(attempt),
            Err(error) => {
                return Err(TcpProbeInputError {
                    code: "tcp_probe_task_failed".into(),
                    message: format!("TCP 探测任务异常结束: {error}"),
                });
            }
        }
    }
    attempts.sort_by_key(|(index, _)| *index);

    Ok(TcpProbeResult {
        port: target.port,
        attempts: attempts.into_iter().map(|(_, attempt)| attempt).collect(),
        cancelled: cancellation.is_cancelled(),
    })
}

async fn probe_one(
    ip: IpAddr,
    port: u16,
    per_address_timeout: Duration,
    cancellation: CancellationToken,
    semaphore: Arc<Semaphore>,
) -> TcpProbeAttempt {
    let socket_address = SocketAddr::new(ip, port);
    let display_address = format_socket_address(ip, port);
    let started = Instant::now();

    let permit = tokio::select! {
        biased;
        _ = cancellation.cancelled() => {
            return cancelled_attempt(ip, port, display_address, started.elapsed());
        }
        permit = semaphore.acquire_owned() => permit,
    };
    let Ok(_permit) = permit else {
        return failed_attempt(
            ip,
            port,
            display_address,
            started.elapsed(),
            TcpProbeError {
                kind: TcpProbeErrorKind::TransportError,
                message: "TCP 探测并发控制器已关闭".into(),
                raw_os_error: None,
            },
        );
    };

    let result = tokio::select! {
        biased;
        _ = cancellation.cancelled() => {
            return cancelled_attempt(ip, port, display_address, started.elapsed());
        }
        result = tokio::time::timeout(per_address_timeout, TcpStream::connect(socket_address)) => result,
    };

    match result {
        Ok(Ok(_stream)) => TcpProbeAttempt {
            ip,
            address_family: address_family(ip),
            port,
            socket_address: display_address,
            duration_ms: duration_ms(started.elapsed()),
            status: TcpProbeStatus::Connected,
            error: None,
        },
        Ok(Err(error)) => {
            let classified = classify_io_error(&error);
            failed_attempt(ip, port, display_address, started.elapsed(), classified)
        }
        Err(_) => failed_attempt(
            ip,
            port,
            display_address,
            started.elapsed(),
            TcpProbeError {
                kind: TcpProbeErrorKind::Timeout,
                message: format!("TCP 连接在 {} ms 内未完成", per_address_timeout.as_millis()),
                raw_os_error: None,
            },
        ),
    }
}

pub fn format_socket_address(ip: IpAddr, port: u16) -> String {
    SocketAddr::new(ip, port).to_string()
}

fn address_family(ip: IpAddr) -> TcpAddressFamily {
    match ip {
        IpAddr::V4(_) => TcpAddressFamily::Ipv4,
        IpAddr::V6(_) => TcpAddressFamily::Ipv6,
    }
}

fn failed_attempt(
    ip: IpAddr,
    port: u16,
    socket_address: String,
    elapsed: Duration,
    error: TcpProbeError,
) -> TcpProbeAttempt {
    TcpProbeAttempt {
        ip,
        address_family: address_family(ip),
        port,
        socket_address,
        duration_ms: duration_ms(elapsed),
        status: TcpProbeStatus::Failed,
        error: Some(error),
    }
}

fn cancelled_attempt(
    ip: IpAddr,
    port: u16,
    socket_address: String,
    elapsed: Duration,
) -> TcpProbeAttempt {
    TcpProbeAttempt {
        ip,
        address_family: address_family(ip),
        port,
        socket_address,
        duration_ms: duration_ms(elapsed),
        status: TcpProbeStatus::Cancelled,
        error: None,
    }
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn classify_io_error(error: &io::Error) -> TcpProbeError {
    let raw_os_error = error.raw_os_error();
    let kind = match error.kind() {
        io::ErrorKind::ConnectionRefused => TcpProbeErrorKind::Refused,
        io::ErrorKind::TimedOut => TcpProbeErrorKind::Timeout,
        io::ErrorKind::NetworkUnreachable => TcpProbeErrorKind::NetworkUnreachable,
        io::ErrorKind::HostUnreachable => TcpProbeErrorKind::HostUnreachable,
        io::ErrorKind::AddrNotAvailable => TcpProbeErrorKind::AddressUnavailable,
        _ => classify_raw_os_error(raw_os_error),
    };
    TcpProbeError {
        kind,
        message: error.to_string(),
        raw_os_error,
    }
}

fn classify_raw_os_error(raw_os_error: Option<i32>) -> TcpProbeErrorKind {
    match raw_os_error {
        // Winsock values followed by their common Unix equivalents.
        Some(10061 | 111) => TcpProbeErrorKind::Refused,
        Some(10060 | 110) => TcpProbeErrorKind::Timeout,
        Some(10051 | 101) => TcpProbeErrorKind::NetworkUnreachable,
        Some(10065 | 113) => TcpProbeErrorKind::HostUnreachable,
        Some(10049 | 99) => TcpProbeErrorKind::AddressUnavailable,
        _ => TcpProbeErrorKind::TransportError,
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use tokio::net::TcpListener;

    use super::*;
    use crate::tools::access_path_diagnostics::model::AccessProtocol;

    fn target(hostname: &str, kind: AccessPathTargetKind, port: u16) -> NormalizedAccessPathTarget {
        NormalizedAccessPathTarget {
            raw_input: hostname.into(),
            protocol: AccessProtocol::Https,
            hostname: hostname.into(),
            target_kind: kind,
            port,
            path: "/".into(),
            url: format!("https://{hostname}:{port}/"),
            sni: None,
            verify_hostname: Some(hostname.into()),
            http_host: hostname.into(),
            connection_ip: None,
        }
    }

    #[test]
    fn connection_ip_overrides_and_deduplicates_candidates() {
        let mut input = target("example.test", AccessPathTargetKind::Hostname, 443);
        input.connection_ip = Some("192.0.2.10".into());
        let candidates = [
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
        ];

        assert_eq!(
            select_probe_addresses(&input, &candidates).unwrap(),
            vec!["192.0.2.10".parse::<IpAddr>().unwrap()]
        );

        input.connection_ip = None;
        assert_eq!(
            select_probe_addresses(&input, &candidates).unwrap(),
            vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]
        );
    }

    #[test]
    fn formats_ipv6_socket_address_with_brackets() {
        assert_eq!(
            format_socket_address(IpAddr::V6(Ipv6Addr::LOCALHOST), 8443),
            "[::1]:8443"
        );
    }

    #[test]
    fn classifies_required_io_error_kinds() {
        let cases = [
            (io::ErrorKind::ConnectionRefused, TcpProbeErrorKind::Refused),
            (io::ErrorKind::TimedOut, TcpProbeErrorKind::Timeout),
            (
                io::ErrorKind::NetworkUnreachable,
                TcpProbeErrorKind::NetworkUnreachable,
            ),
            (
                io::ErrorKind::HostUnreachable,
                TcpProbeErrorKind::HostUnreachable,
            ),
            (
                io::ErrorKind::AddrNotAvailable,
                TcpProbeErrorKind::AddressUnavailable,
            ),
        ];
        for (source, expected) in cases {
            assert_eq!(classify_io_error(&io::Error::from(source)).kind, expected);
        }
        assert_eq!(
            classify_io_error(&io::Error::from_raw_os_error(10061)).kind,
            TcpProbeErrorKind::Refused
        );
    }

    #[tokio::test]
    async fn probes_local_listener_and_preserves_success_details() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let accept = tokio::spawn(async move { listener.accept().await });
        let input = target("127.0.0.1", AccessPathTargetKind::Ipv4, address.port());

        let result = probe_tcp(
            &input,
            &[address.ip()],
            Duration::from_secs(1),
            CancellationToken::new(),
        )
        .await
        .unwrap();

        assert_eq!(result.attempts.len(), 1);
        assert_eq!(result.attempts[0].status, TcpProbeStatus::Connected);
        assert_eq!(result.attempts[0].address_family, TcpAddressFamily::Ipv4);
        assert_eq!(result.attempts[0].socket_address, address.to_string());
        assert!(result.attempts[0].error.is_none());
        accept.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn pre_cancelled_probe_keeps_cancelled_attempts() {
        let input = target("example.test", AccessPathTargetKind::Hostname, 443);
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let result = probe_tcp(
            &input,
            &["192.0.2.1".parse().unwrap(), "2001:db8::1".parse().unwrap()],
            Duration::from_secs(5),
            cancellation,
        )
        .await
        .unwrap();

        assert!(result.cancelled);
        assert_eq!(result.attempts.len(), 2);
        assert!(result
            .attempts
            .iter()
            .all(|attempt| attempt.status == TcpProbeStatus::Cancelled));
    }
}

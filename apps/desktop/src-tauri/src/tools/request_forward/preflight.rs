use std::io;
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::Arc;
use std::time::{Duration, Instant};

use hyper_rustls::ConfigBuilderExt;
use rustls::pki_types::ServerName;
use rustls::ClientConfig;
use serde::Serialize;
use tokio_rustls::TlsConnector;
use url::Url;

use super::model::{ForwardProtocol, RuleWriteInput, ValidatedRuleWriteInput};
use super::validation::{resolve_target_addrs_bounded, validate_rule_input};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const PORT_SUGGESTION_SCAN_LIMIT: u16 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CheckKind {
    Listener,
    Dns,
    Connect,
    Tls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CheckState {
    Passed,
    Failed,
    Warning,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreflightCheck {
    pub(crate) kind: CheckKind,
    pub(crate) state: CheckState,
    pub(crate) message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreflightResult {
    pub(crate) checks: Vec<PreflightCheck>,
    pub(crate) suggested_listen_port: Option<u16>,
    pub(crate) ready: bool,
}

struct TargetEndpoint {
    host: String,
    port: u16,
    https: bool,
}

pub(crate) fn preflight(input: RuleWriteInput) -> Result<PreflightResult, String> {
    preflight_with_tls_loader(input, || {
        ClientConfig::builder()
            .with_native_roots()
            .map(|builder| Arc::new(builder.with_no_client_auth()))
            .map_err(|error| format!("无法加载系统 TLS 根证书: {error}"))
    })
}

#[cfg(test)]
fn preflight_with_tls_config(
    input: RuleWriteInput,
    config: Arc<ClientConfig>,
) -> Result<PreflightResult, String> {
    preflight_with_tls_loader(input, || Ok(config))
}

fn preflight_with_tls_loader(
    input: RuleWriteInput,
    tls_config_loader: impl FnOnce() -> Result<Arc<ClientConfig>, String>,
) -> Result<PreflightResult, String> {
    // 参数错误属于 action 错误；预检不能把无效配置伪装成普通检查结果。
    let input = validate_rule_input(input)?;
    let target = target_endpoint(&input)?;
    let mut checks = Vec::new();

    let listener_address = socket_address(&input.bind_host, input.listen_port)?;
    let suggested_listen_port = match bind_temporarily(input.protocol, listener_address) {
        Ok(()) => {
            checks.push(PreflightCheck {
                kind: CheckKind::Listener,
                state: CheckState::Passed,
                message: format!(
                    "检测时可绑定监听地址 {listener_address}；实际启动时的绑定结果为最终依据"
                ),
            });
            None
        }
        Err(error) => {
            let suggestion =
                suggest_listen_port(input.protocol, listener_address.ip(), input.listen_port);
            let suggestion_message = suggestion.map_or_else(String::new, |port| {
                format!("；可尝试端口 {port}，该建议仅表示检测时可用，不保证实际启动时仍可绑定")
            });
            checks.push(PreflightCheck {
                kind: CheckKind::Listener,
                state: CheckState::Failed,
                message: format!(
                    "检测时无法绑定监听地址 {listener_address}: {error}{suggestion_message}；实际启动时的绑定结果为最终依据"
                ),
            });
            suggestion
        }
    };

    let addresses = match resolve_target_addrs_bounded(target.host.clone(), target.port) {
        Ok(addresses) => {
            checks.push(PreflightCheck {
                kind: CheckKind::Dns,
                state: CheckState::Passed,
                message: format!(
                    "检测时已解析目标 {}:{}，得到 {} 个地址；实际启动时会重新解析，启动结果为最终依据",
                    target.host,
                    target.port,
                    addresses.len()
                ),
            });
            addresses
        }
        Err(error) => {
            checks.push(PreflightCheck {
                kind: CheckKind::Dns,
                state: CheckState::Failed,
                message: format!("{error}；实际启动时会重新解析，启动结果为最终依据"),
            });
            return Ok(finish(checks, suggested_listen_port));
        }
    };

    if input.protocol == ForwardProtocol::Udp {
        checks.push(PreflightCheck {
            kind: CheckKind::Connect,
            state: CheckState::Warning,
            message: format!(
                "检测时已解析 UDP 目标 {}:{}；UDP 是无连接协议，无法证明目标服务响应，实际转发结果为最终依据",
                target.host, target.port
            ),
        });
        return Ok(finish(checks, suggested_listen_port));
    }

    let connect_deadline = Instant::now() + CONNECT_TIMEOUT;
    let (stream, connected_address) = match connect_any(&addresses, connect_deadline) {
        Ok(connected) => connected,
        Err(error) => {
            checks.push(PreflightCheck {
                kind: CheckKind::Connect,
                state: CheckState::Failed,
                message: format!(
                    "检测时无法连接目标 {}:{}: {error}；实际启动连接结果为最终依据",
                    target.host, target.port
                ),
            });
            return Ok(finish(checks, suggested_listen_port));
        }
    };
    checks.push(PreflightCheck {
        kind: CheckKind::Connect,
        state: CheckState::Passed,
        message: format!("检测时可连接目标 {connected_address}；实际启动连接结果为最终依据"),
    });

    if target.https {
        let tls_result = tls_config_loader()
            .and_then(|config| tls_handshake(stream, &target.host, connect_deadline, config));
        match tls_result {
            Ok(()) => checks.push(PreflightCheck {
                kind: CheckKind::Tls,
                state: CheckState::Passed,
                message: format!(
                    "检测时 TLS 握手、主机名和证书链校验通过；未发送 HTTP 业务请求，实际启动连接结果为最终依据"
                ),
            }),
            Err(error) => checks.push(PreflightCheck {
                kind: CheckKind::Tls,
                state: CheckState::Failed,
                message: format!(
                    "检测时 TLS 握手或严格证书校验失败: {error}；实际启动连接结果为最终依据"
                ),
            }),
        }
    }

    Ok(finish(checks, suggested_listen_port))
}

fn socket_address(host: &str, port: u16) -> Result<SocketAddr, String> {
    let ip = host
        .parse::<IpAddr>()
        .map_err(|_| "监听地址必须是 IPv4 或 IPv6 字面量".to_string())?;
    Ok(SocketAddr::new(ip, port))
}

fn bind_temporarily(protocol: ForwardProtocol, address: SocketAddr) -> io::Result<()> {
    match protocol {
        ForwardProtocol::Http | ForwardProtocol::Tcp => {
            drop(TcpListener::bind(address)?);
        }
        ForwardProtocol::Udp => {
            drop(UdpSocket::bind(address)?);
        }
    }
    Ok(())
}

fn suggest_listen_port(protocol: ForwardProtocol, host: IpAddr, original_port: u16) -> Option<u16> {
    (1..=PORT_SUGGESTION_SCAN_LIMIT).find_map(|offset| {
        let candidate = original_port.checked_add(offset).unwrap_or(offset);
        (candidate != original_port
            && bind_temporarily(protocol, SocketAddr::new(host, candidate)).is_ok())
        .then_some(candidate)
    })
}

fn target_endpoint(input: &ValidatedRuleWriteInput) -> Result<TargetEndpoint, String> {
    match input.protocol {
        ForwardProtocol::Http => {
            let target_url = input
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
            Ok(TargetEndpoint {
                host: host.to_string(),
                port,
                https: parsed.scheme() == "https",
            })
        }
        ForwardProtocol::Tcp | ForwardProtocol::Udp => Ok(TargetEndpoint {
            host: input
                .target_host
                .clone()
                .ok_or_else(|| "TCP/UDP 规则必须配置目标主机".to_string())?,
            port: input
                .target_port
                .ok_or_else(|| "TCP/UDP 规则必须配置目标端口".to_string())?,
            https: false,
        }),
    }
}

fn connect_any(
    addresses: &[SocketAddr],
    deadline: Instant,
) -> Result<(TcpStream, SocketAddr), String> {
    let mut useful_error = None;
    for (index, address) in addresses.iter().enumerate() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            useful_error.get_or_insert_with(|| "连接总超时".to_string());
            break;
        }
        let attempts_left = u32::try_from(addresses.len() - index).unwrap_or(u32::MAX);
        let attempt_timeout = remaining / attempts_left;
        if attempt_timeout.is_zero() {
            useful_error.get_or_insert_with(|| "连接总超时".to_string());
            break;
        }
        match TcpStream::connect_timeout(address, attempt_timeout) {
            Ok(stream) => return Ok((stream, *address)),
            Err(error) => {
                let detail = format!("{address}: {error}");
                if useful_error.is_none() || error.kind() != io::ErrorKind::TimedOut {
                    useful_error = Some(detail);
                }
            }
        }
    }
    Err(useful_error.unwrap_or_else(|| "未解析到可尝试的目标地址".into()))
}

fn tls_handshake(
    stream: TcpStream,
    host: &str,
    deadline: Instant,
    config: Arc<ClientConfig>,
) -> Result<(), String> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err("TLS 握手总超时".into());
    }
    let server_name = ServerName::try_from(host.to_string())
        .map_err(|error| format!("TLS 主机名无效: {error}"))?;
    stream
        .set_nonblocking(true)
        .map_err(|error| format!("配置 TLS socket 失败: {error}"))?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|error| format!("创建 TLS 预检 runtime 失败: {error}"))?;
    runtime.block_on(async move {
        let stream = tokio::net::TcpStream::from_std(stream)
            .map_err(|error| format!("接管 TLS socket 失败: {error}"))?;
        let connector = TlsConnector::from(config);
        match tokio::time::timeout(remaining, connector.connect(server_name, stream)).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(error)) => Err(format!("{error}")),
            Err(_) => Err(format!("TLS 握手超时（{} ms）", remaining.as_millis())),
        }
    })
}

fn finish(checks: Vec<PreflightCheck>, suggested_listen_port: Option<u16>) -> PreflightResult {
    let ready = checks.iter().all(|check| check.state != CheckState::Failed);
    PreflightResult {
        checks,
        suggested_listen_port,
        ready,
    }
}

#[cfg(test)]
mod tests {
    use std::net::{TcpListener, TcpStream, UdpSocket};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};

    use rustls::{ClientConfig, RootCertStore};

    use super::{preflight, preflight_with_tls_config, CheckKind, CheckState};
    use crate::tools::request_forward::http::integration_tests::accept_tls_once;
    use crate::tools::request_forward::model::{ForwardProtocol, RuleWriteInput};

    const TEST_TIMEOUT: Duration = Duration::from_secs(2);

    fn free_tcp_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .expect("bind temporary TCP port")
            .local_addr()
            .expect("read temporary TCP port")
            .port()
    }

    fn free_udp_port() -> u16 {
        UdpSocket::bind("127.0.0.1:0")
            .expect("bind temporary UDP port")
            .local_addr()
            .expect("read temporary UDP port")
            .port()
    }

    fn socket_input(
        protocol: ForwardProtocol,
        listen_port: u16,
        target_host: &str,
        target_port: u16,
    ) -> RuleWriteInput {
        RuleWriteInput {
            name: "预检规则".into(),
            protocol,
            bind_host: "127.0.0.1".into(),
            listen_port,
            target_url: None,
            target_host: Some(target_host.into()),
            target_port: Some(target_port),
            capture_http_headers: false,
            capture_http_body: false,
        }
    }

    fn https_input(listen_port: u16, target_url: String) -> RuleWriteInput {
        RuleWriteInput {
            name: "HTTPS 预检规则".into(),
            protocol: ForwardProtocol::Http,
            bind_host: "127.0.0.1".into(),
            listen_port,
            target_url: Some(target_url),
            target_host: None,
            target_port: None,
            capture_http_headers: false,
            capture_http_body: false,
        }
    }

    fn check_state(result: &super::PreflightResult, kind: CheckKind) -> CheckState {
        result
            .checks
            .iter()
            .find(|check| check.kind == kind)
            .unwrap_or_else(|| panic!("missing {kind:?} check"))
            .state
    }

    fn tcp_target() -> (std::net::SocketAddr, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind TCP target");
        let address = listener.local_addr().expect("read TCP target address");
        listener
            .set_nonblocking(true)
            .expect("make TCP target nonblocking");
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + TEST_TIMEOUT;
            loop {
                match listener.accept() {
                    Ok(_) => return,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "TCP target accept timed out");
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("TCP target accept failed: {error}"),
                }
            }
        });
        (address, worker)
    }

    #[test]
    fn free_tcp_and_udp_listeners_pass() {
        let (tcp_target, tcp_worker) = tcp_target();
        let tcp = preflight(socket_input(
            ForwardProtocol::Tcp,
            free_tcp_port(),
            "127.0.0.1",
            tcp_target.port(),
        ))
        .expect("preflight free TCP listener");
        tcp_worker.join().expect("join TCP target");

        assert_eq!(check_state(&tcp, CheckKind::Listener), CheckState::Passed);
        assert!(tcp.ready);

        let udp = preflight(socket_input(
            ForwardProtocol::Udp,
            free_udp_port(),
            "127.0.0.1",
            9,
        ))
        .expect("preflight free UDP listener");
        assert_eq!(check_state(&udp, CheckKind::Listener), CheckState::Passed);
        assert!(udp.ready);
    }

    #[test]
    fn occupied_listener_fails_and_suggests_bindable_different_port() {
        let occupied = TcpListener::bind("127.0.0.1:0").expect("occupy listener");
        let occupied_port = occupied.local_addr().expect("read occupied port").port();
        let (target, worker) = tcp_target();

        let result = preflight(socket_input(
            ForwardProtocol::Tcp,
            occupied_port,
            "127.0.0.1",
            target.port(),
        ))
        .expect("preflight occupied listener");
        worker.join().expect("join TCP target");

        assert_eq!(
            check_state(&result, CheckKind::Listener),
            CheckState::Failed
        );
        assert!(!result.ready);
        let suggested = result
            .suggested_listen_port
            .expect("occupied listener has suggestion");
        assert_ne!(suggested, occupied_port);
        TcpListener::bind(("127.0.0.1", suggested)).expect("suggested TCP port is bindable");
    }

    #[test]
    fn dns_failure_is_explicit() {
        let result = preflight(socket_input(
            ForwardProtocol::Tcp,
            free_tcp_port(),
            "invalid target host ^",
            8080,
        ))
        .expect("DNS failure is a check result");

        assert_eq!(check_state(&result, CheckKind::Dns), CheckState::Failed);
        assert!(!result.ready);
    }

    #[test]
    fn tcp_target_success_and_failure_are_reported() {
        let (target, worker) = tcp_target();
        let success = preflight(socket_input(
            ForwardProtocol::Tcp,
            free_tcp_port(),
            "127.0.0.1",
            target.port(),
        ))
        .expect("preflight reachable TCP target");
        worker.join().expect("join TCP target");
        assert_eq!(
            check_state(&success, CheckKind::Connect),
            CheckState::Passed
        );
        assert!(success.ready);

        let unavailable = TcpListener::bind("127.0.0.1:0").expect("reserve unavailable port");
        let unavailable_port = unavailable
            .local_addr()
            .expect("read unavailable port")
            .port();
        drop(unavailable);
        let failure = preflight(socket_input(
            ForwardProtocol::Tcp,
            free_tcp_port(),
            "127.0.0.1",
            unavailable_port,
        ))
        .expect("preflight unreachable TCP target");
        assert_eq!(
            check_state(&failure, CheckKind::Connect),
            CheckState::Failed
        );
        assert!(!failure.ready);
    }

    #[test]
    fn udp_connect_is_warning_because_it_cannot_prove_a_response() {
        let result = preflight(socket_input(
            ForwardProtocol::Udp,
            free_udp_port(),
            "127.0.0.1",
            9,
        ))
        .expect("preflight UDP target");

        assert_eq!(check_state(&result, CheckKind::Dns), CheckState::Passed);
        assert_eq!(
            check_state(&result, CheckKind::Connect),
            CheckState::Warning
        );
        assert!(result
            .checks
            .iter()
            .find(|check| check.kind == CheckKind::Connect)
            .expect("UDP connect check")
            .message
            .contains("无法证明目标服务响应"));
        assert!(result.ready);
    }

    #[test]
    fn https_trusted_fixture_passes_without_sending_http() {
        let fixture = accept_tls_once(true, |_| {});
        let result = preflight_with_tls_config(
            https_input(free_tcp_port(), format!("https://{}", fixture.address)),
            Arc::new(fixture.client_config.clone()),
        )
        .expect("preflight trusted HTTPS target");

        assert_eq!(check_state(&result, CheckKind::Connect), CheckState::Passed);
        assert_eq!(check_state(&result, CheckKind::Tls), CheckState::Passed);
        assert!(result.ready);
        fixture.finish().expect("finish trusted TLS fixture");
    }

    #[test]
    fn https_hostname_mismatch_and_untrusted_ca_fail() {
        let hostname_fixture =
            accept_tls_once(false, |_| panic!("hostname mismatch must not complete TLS"));
        let hostname_result = preflight_with_tls_config(
            https_input(
                free_tcp_port(),
                format!("https://{}", hostname_fixture.address),
            ),
            Arc::new(hostname_fixture.client_config.clone()),
        )
        .expect("preflight hostname mismatch");
        assert_eq!(
            check_state(&hostname_result, CheckKind::Tls),
            CheckState::Failed
        );
        assert!(!hostname_result.ready);
        assert!(hostname_fixture.finish().is_err());

        let untrusted_fixture = accept_tls_once(true, |_| {
            panic!("untrusted certificate must not complete TLS")
        });
        let untrusted_config = ClientConfig::builder()
            .with_root_certificates(RootCertStore::empty())
            .with_no_client_auth();
        let untrusted_result = preflight_with_tls_config(
            https_input(
                free_tcp_port(),
                format!("https://{}", untrusted_fixture.address),
            ),
            Arc::new(untrusted_config),
        )
        .expect("preflight untrusted CA");
        assert_eq!(
            check_state(&untrusted_result, CheckKind::Tls),
            CheckState::Failed
        );
        assert!(!untrusted_result.ready);
        assert!(untrusted_fixture.finish().is_err());
    }

    #[test]
    fn invalid_configuration_remains_an_action_error() {
        let mut invalid = socket_input(ForwardProtocol::Tcp, free_tcp_port(), "127.0.0.1", 9);
        invalid.name = "  ".into();

        let error = preflight(invalid).expect_err("invalid input must not become a check");

        assert!(error.contains("名称"));
    }

    #[test]
    fn listener_check_does_not_keep_the_port() {
        let (target, worker) = tcp_target();
        let port = free_tcp_port();
        let result = preflight(socket_input(
            ForwardProtocol::Tcp,
            port,
            "127.0.0.1",
            target.port(),
        ))
        .expect("preflight TCP listener");
        worker.join().expect("join TCP target");
        assert!(result.ready);

        let rebound = TcpListener::bind(("127.0.0.1", port)).expect("preflight releases listener");
        rebound
            .set_nonblocking(true)
            .expect("configure rebound listener");
        assert!(TcpStream::connect_timeout(
            &rebound.local_addr().expect("read rebound listener"),
            TEST_TIMEOUT,
        )
        .is_ok());
    }
}

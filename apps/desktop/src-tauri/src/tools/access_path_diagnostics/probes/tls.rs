use std::collections::HashSet;
use std::io;
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use openssl::asn1::Asn1Time;
use openssl::hash::MessageDigest;
use openssl::ssl::{HandshakeError, Ssl, SslContextBuilder, SslMethod, SslStream, SslVerifyMode};
use openssl::stack::Stack;
use openssl::x509::store::X509StoreBuilder;
use openssl::x509::{X509NameRef, X509StoreContext, X509VerifyResult, X509};
use serde::{Deserialize, Serialize};

const MIN_SOCKET_TIMEOUT: Duration = Duration::from_millis(1);

#[derive(Debug, Clone)]
pub struct TlsProbeConfig {
    pub connection_ip: IpAddr,
    pub sni: Option<String>,
    pub verify_hostname: Option<String>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TlsProbeStage {
    TcpConnect,
    TlsSetup,
    TlsHandshake,
    CertificateInspection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TlsProbeErrorKind {
    Timeout,
    ConnectionRefused,
    NetworkUnreachable,
    Io,
    Tls,
    Certificate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsProbeError {
    pub stage: TlsProbeStage,
    pub kind: TlsProbeErrorKind,
    pub message: String,
    pub os_code: Option<i32>,
    #[serde(default)]
    pub openssl_errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TlsVerificationState {
    Verified,
    Failed,
    Unverified,
    Unsupported,
    NotRequested,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsHostnameVerification {
    pub hostname: Option<String>,
    pub state: TlsVerificationState,
    pub method: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsTrustVerification {
    pub openssl_state: TlsVerificationState,
    pub openssl_error: Option<String>,
    pub openssl_error_depth: Option<u32>,
    pub windows_state: TlsVerificationState,
    pub windows_capability: String,
    pub revocation_state: TlsVerificationState,
    pub revocation_capability: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsSubjectAlternativeName {
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsCertificate {
    pub subject: String,
    pub issuer: String,
    pub subject_alt_names: Vec<TlsSubjectAlternativeName>,
    pub not_before: String,
    pub not_after: String,
    pub days_until_expiry: Option<i32>,
    pub serial_number: String,
    pub signature_algorithm: String,
    pub public_key_algorithm: Option<String>,
    pub public_key_bits: Option<u32>,
    pub sha256_fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsProbeResult {
    pub connection_ip: IpAddr,
    pub port: u16,
    pub address_family: String,
    pub sni: Option<String>,
    pub verify_hostname: Option<String>,
    pub connected: bool,
    pub handshake_succeeded: bool,
    pub connect_elapsed_ms: Option<u64>,
    pub handshake_elapsed_ms: Option<u64>,
    pub tls_version: Option<String>,
    pub cipher: Option<String>,
    pub cipher_secret_bits: Option<i32>,
    #[serde(default)]
    pub certificate_chain: Vec<TlsCertificate>,
    pub hostname_verification: TlsHostnameVerification,
    pub trust_verification: TlsTrustVerification,
    pub error: Option<TlsProbeError>,
}

pub struct TlsProbeSession {
    pub stream: SslStream<TcpStream>,
    pub result: TlsProbeResult,
}

pub fn probe_tls_addresses(
    addresses: &[IpAddr],
    port: u16,
    sni: Option<&str>,
    verify_hostname: Option<&str>,
    timeout: Duration,
) -> Vec<TlsProbeResult> {
    addresses
        .iter()
        .copied()
        .map(|connection_ip| {
            probe_tls_address(
                SocketAddr::new(connection_ip, port),
                sni,
                verify_hostname,
                timeout,
            )
        })
        .collect()
}

pub fn probe_tls_address(
    address: SocketAddr,
    sni: Option<&str>,
    verify_hostname: Option<&str>,
    timeout: Duration,
) -> TlsProbeResult {
    match connect_tls_address(address, sni, verify_hostname, timeout) {
        Ok(session) => session.result,
        Err(result) => result,
    }
}

pub fn connect_tls_address(
    address: SocketAddr,
    sni: Option<&str>,
    verify_hostname: Option<&str>,
    timeout: Duration,
) -> Result<TlsProbeSession, TlsProbeResult> {
    let started = Instant::now();
    let config = TlsProbeConfig {
        connection_ip: address.ip(),
        sni: normalized_optional_name(sni),
        verify_hostname: normalized_optional_name(verify_hostname),
        timeout,
    };
    let mut result = empty_result(&config, address.port());

    let stream = match TcpStream::connect_timeout(&address, socket_timeout(timeout)) {
        Ok(stream) => stream,
        Err(error) => {
            result.connect_elapsed_ms = Some(elapsed_ms(started));
            result.error = Some(io_probe_error(TlsProbeStage::TcpConnect, error));
            return Err(result);
        }
    };
    result.connected = true;
    result.connect_elapsed_ms = Some(elapsed_ms(started));

    let remaining = timeout.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        result.error = Some(timeout_error(TlsProbeStage::TlsHandshake));
        return Err(result);
    }

    connect_tls_stream_with_result(stream, config, result, remaining)
}

pub fn connect_tls_stream(
    stream: TcpStream,
    port: u16,
    config: TlsProbeConfig,
) -> Result<TlsProbeSession, TlsProbeResult> {
    let mut result = empty_result(&config, port);
    result.connected = true;
    connect_tls_stream_with_result(stream, config.clone(), result, config.timeout)
}

fn connect_tls_stream_with_result(
    stream: TcpStream,
    config: TlsProbeConfig,
    mut result: TlsProbeResult,
    handshake_timeout: Duration,
) -> Result<TlsProbeSession, TlsProbeResult> {
    if let Err(error) = set_stream_timeouts(&stream, handshake_timeout) {
        result.error = Some(io_probe_error(TlsProbeStage::TlsSetup, error));
        return Err(result);
    }

    let mut context = match SslContextBuilder::new(SslMethod::tls()) {
        Ok(context) => context,
        Err(error) => {
            result.error = Some(openssl_probe_error(TlsProbeStage::TlsSetup, error));
            return Err(result);
        }
    };
    // The first handshake observes the peer even when trust or hostname validation fails.
    // Both validations are reported independently after the handshake.
    context.set_verify(SslVerifyMode::NONE);
    let mut ssl = match Ssl::new(&context.build()) {
        Ok(ssl) => ssl,
        Err(error) => {
            result.error = Some(openssl_probe_error(TlsProbeStage::TlsSetup, error));
            return Err(result);
        }
    };
    let effective_sni = config
        .sni
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.parse::<IpAddr>().is_err());
    result.sni = effective_sni.map(str::to_string);
    if let Some(sni) = effective_sni {
        if let Err(error) = ssl.set_hostname(sni) {
            result.error = Some(openssl_probe_error(TlsProbeStage::TlsSetup, error));
            return Err(result);
        }
    }

    let handshake_started = Instant::now();
    let stream = match ssl.connect(stream) {
        Ok(stream) => stream,
        Err(error) => {
            result.handshake_elapsed_ms = Some(elapsed_ms(handshake_started));
            result.error = Some(handshake_probe_error(error));
            return Err(result);
        }
    };

    result.handshake_succeeded = true;
    result.handshake_elapsed_ms = Some(elapsed_ms(handshake_started));
    result.tls_version = Some(stream.ssl().version_str().to_string());
    if let Some(cipher) = stream.ssl().current_cipher() {
        result.cipher = Some(cipher.name().to_string());
        result.cipher_secret_bits = Some(cipher.bits().secret);
    }

    let peer_certificate = stream.ssl().peer_certificate();
    if let Some(peer) = peer_certificate.as_ref() {
        result.certificate_chain = inspect_certificate_chain(&stream, peer);
        result.hostname_verification =
            verify_certificate_hostname(peer, config.verify_hostname.as_deref());
        result.trust_verification = verify_certificate_trust(&stream, peer);
    } else {
        result.hostname_verification = unverified_hostname(
            config.verify_hostname.clone(),
            "服务器未返回叶证书，无法校验主机名",
        );
        result.trust_verification.openssl_state = TlsVerificationState::Unverified;
        result.trust_verification.openssl_error = Some("服务器未返回证书链".into());
        result.error = Some(TlsProbeError {
            stage: TlsProbeStage::CertificateInspection,
            kind: TlsProbeErrorKind::Certificate,
            message: "TLS 握手成功，但服务器未返回叶证书".into(),
            os_code: None,
            openssl_errors: Vec::new(),
        });
    }

    Ok(TlsProbeSession { stream, result })
}

fn empty_result(config: &TlsProbeConfig, port: u16) -> TlsProbeResult {
    TlsProbeResult {
        connection_ip: config.connection_ip,
        port,
        address_family: match config.connection_ip {
            IpAddr::V4(_) => "ipv4",
            IpAddr::V6(_) => "ipv6",
        }
        .into(),
        sni: config.sni.clone(),
        verify_hostname: config.verify_hostname.clone(),
        connected: false,
        handshake_succeeded: false,
        connect_elapsed_ms: None,
        handshake_elapsed_ms: None,
        tls_version: None,
        cipher: None,
        cipher_secret_bits: None,
        certificate_chain: Vec::new(),
        hostname_verification: TlsHostnameVerification {
            hostname: config.verify_hostname.clone(),
            state: if config.verify_hostname.is_some() {
                TlsVerificationState::Unverified
            } else {
                TlsVerificationState::NotRequested
            },
            method: "openssl_certificate_identity".into(),
            error: None,
        },
        trust_verification: unsupported_windows_trust(),
        error: None,
    }
}

fn unsupported_windows_trust() -> TlsTrustVerification {
    TlsTrustVerification {
        openssl_state: TlsVerificationState::Unverified,
        openssl_error: None,
        openssl_error_depth: None,
        windows_state: TlsVerificationState::Unsupported,
        windows_capability:
            "当前探测器未调用 Windows 证书链 API，OpenSSL 结果不代表 Windows 系统信任".into(),
        revocation_state: TlsVerificationState::Unverified,
        revocation_capability: "未发起 OCSP/CRL/AIA 在线请求，吊销状态无法离线确认".into(),
    }
}

fn inspect_certificate_chain(stream: &SslStream<TcpStream>, peer: &X509) -> Vec<TlsCertificate> {
    let mut certificates = Vec::new();
    let mut fingerprints = HashSet::new();
    push_certificate(peer, &mut certificates, &mut fingerprints);
    if let Some(chain) = stream.ssl().peer_cert_chain() {
        for certificate in chain {
            push_certificate(certificate, &mut certificates, &mut fingerprints);
        }
    }
    certificates
}

fn push_certificate(
    certificate: &openssl::x509::X509Ref,
    output: &mut Vec<TlsCertificate>,
    fingerprints: &mut HashSet<Vec<u8>>,
) {
    let fingerprint = certificate
        .digest(MessageDigest::sha256())
        .map(|value| value.to_vec())
        .unwrap_or_default();
    if !fingerprints.insert(fingerprint.clone()) {
        return;
    }
    output.push(inspect_certificate(certificate, fingerprint));
}

fn inspect_certificate(
    certificate: &openssl::x509::X509Ref,
    fingerprint: Vec<u8>,
) -> TlsCertificate {
    let now = Asn1Time::days_from_now(0).ok();
    let days_until_expiry = now
        .as_ref()
        .and_then(|now| now.diff(certificate.not_after()).ok())
        .map(|diff| diff.days);
    let public_key = certificate.public_key().ok();

    TlsCertificate {
        subject: format_x509_name(certificate.subject_name()),
        issuer: format_x509_name(certificate.issuer_name()),
        subject_alt_names: certificate
            .subject_alt_names()
            .map(|names| {
                names
                    .iter()
                    .filter_map(|name| {
                        if let Some(value) = name.dnsname() {
                            return Some(TlsSubjectAlternativeName {
                                kind: "dns".into(),
                                value: value.into(),
                            });
                        }
                        if let Some(value) = name.ipaddress() {
                            return Some(TlsSubjectAlternativeName {
                                kind: "ip".into(),
                                value: format_ip_bytes(value)?,
                            });
                        }
                        if let Some(value) = name.uri() {
                            return Some(TlsSubjectAlternativeName {
                                kind: "uri".into(),
                                value: value.into(),
                            });
                        }
                        if let Some(value) = name.email() {
                            return Some(TlsSubjectAlternativeName {
                                kind: "email".into(),
                                value: value.into(),
                            });
                        }
                        None
                    })
                    .collect()
            })
            .unwrap_or_default(),
        not_before: certificate.not_before().to_string(),
        not_after: certificate.not_after().to_string(),
        days_until_expiry,
        serial_number: certificate
            .serial_number()
            .to_bn()
            .and_then(|number| number.to_hex_str())
            .map(|value| value.to_string())
            .unwrap_or_else(|_| "unavailable".into()),
        signature_algorithm: certificate
            .signature_algorithm()
            .object()
            .nid()
            .long_name()
            .unwrap_or("unknown")
            .into(),
        public_key_algorithm: public_key
            .as_ref()
            .map(|key| format!("{:?}", key.id()).to_lowercase()),
        public_key_bits: public_key.as_ref().map(|key| key.bits()),
        sha256_fingerprint: (!fingerprint.is_empty()).then(|| hex_bytes(&fingerprint, ":")),
    }
}

fn verify_certificate_hostname(peer: &X509, hostname: Option<&str>) -> TlsHostnameVerification {
    let Some(hostname) = hostname else {
        return TlsHostnameVerification {
            hostname: None,
            state: TlsVerificationState::NotRequested,
            method: "openssl_certificate_identity".into(),
            error: None,
        };
    };

    let matched = if let Ok(expected_ip) = hostname.parse::<IpAddr>() {
        peer.subject_alt_names()
            .map(|names| {
                names
                    .iter()
                    .any(|name| name.ipaddress().and_then(parse_ip_bytes) == Some(expected_ip))
            })
            .unwrap_or(false)
    } else {
        let san_dns: Vec<String> = peer
            .subject_alt_names()
            .map(|names| {
                names
                    .iter()
                    .filter_map(|name| name.dnsname().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if san_dns.is_empty() {
            peer.subject_name()
                .entries_by_nid(openssl::nid::Nid::COMMONNAME)
                .filter_map(|entry| entry.data().as_utf8().ok())
                .any(|name| dns_identity_matches(name.as_ref(), hostname))
        } else {
            san_dns
                .iter()
                .any(|name| dns_identity_matches(name, hostname))
        }
    };

    TlsHostnameVerification {
        hostname: Some(hostname.into()),
        state: if matched {
            TlsVerificationState::Verified
        } else {
            TlsVerificationState::Failed
        },
        method: "rfc6125_san_with_cn_fallback".into(),
        error: (!matched).then(|| format!("证书标识不匹配 {hostname}")),
    }
}

fn verify_certificate_trust(stream: &SslStream<TcpStream>, peer: &X509) -> TlsTrustVerification {
    let mut result = unsupported_windows_trust();
    let mut store = match X509StoreBuilder::new() {
        Ok(store) => store,
        Err(error) => {
            result.openssl_error = Some(error.to_string());
            return result;
        }
    };
    if let Err(error) = store.set_default_paths() {
        result.openssl_error = Some(format!("无法加载 OpenSSL 默认信任路径: {error}"));
        return result;
    }
    let store = store.build();
    let mut chain = match Stack::new() {
        Ok(chain) => chain,
        Err(error) => {
            result.openssl_error = Some(error.to_string());
            return result;
        }
    };
    if let Some(peer_chain) = stream.ssl().peer_cert_chain() {
        let peer_der = peer.to_der().ok();
        for certificate in peer_chain {
            if peer_der
                .as_ref()
                .is_some_and(|der| certificate.to_der().ok().as_ref() == Some(der))
            {
                continue;
            }
            if let Err(error) = chain.push(certificate.to_owned()) {
                result.openssl_error = Some(error.to_string());
                return result;
            }
        }
    }
    let mut context = match X509StoreContext::new() {
        Ok(context) => context,
        Err(error) => {
            result.openssl_error = Some(error.to_string());
            return result;
        }
    };
    match context.init(&store, peer, &chain, |context| {
        let verified = context.verify_cert()?;
        Ok((verified, context.error(), context.error_depth()))
    }) {
        Ok((true, _, _)) => result.openssl_state = TlsVerificationState::Verified,
        Ok((false, error, depth)) => {
            result.openssl_state = TlsVerificationState::Failed;
            result.openssl_error = Some(verify_error_message(error));
            result.openssl_error_depth = Some(depth);
        }
        Err(error) => {
            result.openssl_state = TlsVerificationState::Unverified;
            result.openssl_error = Some(error.to_string());
        }
    }
    result
}

fn format_x509_name(name: &X509NameRef) -> String {
    name.entries()
        .map(|entry| {
            let key = entry.object().nid().short_name().unwrap_or("OID");
            let value = entry
                .data()
                .as_utf8()
                .map(|value| value.to_string())
                .unwrap_or_else(|_| hex_bytes(entry.data().as_slice(), ""));
            format!("{key}={value}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn dns_identity_matches(pattern: &str, hostname: &str) -> bool {
    let pattern = pattern.trim_end_matches('.').to_ascii_lowercase();
    let hostname = hostname.trim_end_matches('.').to_ascii_lowercase();
    if pattern == hostname {
        return true;
    }
    let Some(suffix) = pattern.strip_prefix("*.") else {
        return false;
    };
    if suffix.is_empty() || suffix.contains('*') || suffix.split('.').count() < 2 {
        return false;
    }
    let Some(prefix) = hostname.strip_suffix(suffix) else {
        return false;
    };
    prefix.ends_with('.') && !prefix[..prefix.len() - 1].contains('.') && prefix.len() > 1
}

fn normalized_optional_name(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn set_stream_timeouts(stream: &TcpStream, timeout: Duration) -> io::Result<()> {
    let timeout = Some(socket_timeout(timeout));
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)
}

fn socket_timeout(timeout: Duration) -> Duration {
    timeout.max(MIN_SOCKET_TIMEOUT)
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

fn io_probe_error(stage: TlsProbeStage, error: io::Error) -> TlsProbeError {
    let kind = match error.kind() {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => TlsProbeErrorKind::Timeout,
        io::ErrorKind::ConnectionRefused => TlsProbeErrorKind::ConnectionRefused,
        io::ErrorKind::NetworkUnreachable | io::ErrorKind::HostUnreachable => {
            TlsProbeErrorKind::NetworkUnreachable
        }
        _ => TlsProbeErrorKind::Io,
    };
    TlsProbeError {
        stage,
        kind,
        message: error.to_string(),
        os_code: error.raw_os_error(),
        openssl_errors: Vec::new(),
    }
}

fn timeout_error(stage: TlsProbeStage) -> TlsProbeError {
    TlsProbeError {
        stage,
        kind: TlsProbeErrorKind::Timeout,
        message: "TLS 探测超时".into(),
        os_code: None,
        openssl_errors: Vec::new(),
    }
}

fn openssl_probe_error(stage: TlsProbeStage, error: openssl::error::ErrorStack) -> TlsProbeError {
    TlsProbeError {
        stage,
        kind: TlsProbeErrorKind::Tls,
        message: error.to_string(),
        os_code: None,
        openssl_errors: error.errors().iter().map(ToString::to_string).collect(),
    }
}

fn handshake_probe_error(error: HandshakeError<TcpStream>) -> TlsProbeError {
    match error {
        HandshakeError::SetupFailure(error) => openssl_probe_error(TlsProbeStage::TlsSetup, error),
        HandshakeError::Failure(stream) | HandshakeError::WouldBlock(stream) => {
            let io_error = stream.error().io_error();
            let kind = io_error
                .filter(|error| {
                    matches!(
                        error.kind(),
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                    )
                })
                .map(|_| TlsProbeErrorKind::Timeout)
                .unwrap_or(TlsProbeErrorKind::Tls);
            TlsProbeError {
                stage: TlsProbeStage::TlsHandshake,
                kind,
                message: stream.error().to_string(),
                os_code: io_error.and_then(io::Error::raw_os_error),
                openssl_errors: stream
                    .error()
                    .ssl_error()
                    .map(|errors| errors.errors().iter().map(ToString::to_string).collect())
                    .unwrap_or_default(),
            }
        }
    }
}

fn unverified_hostname(hostname: Option<String>, message: &str) -> TlsHostnameVerification {
    TlsHostnameVerification {
        hostname,
        state: TlsVerificationState::Unverified,
        method: "openssl_certificate_identity".into(),
        error: Some(message.into()),
    }
}

fn verify_error_message(error: X509VerifyResult) -> String {
    format!("{} (code {})", error.error_string(), error.as_raw())
}

fn parse_ip_bytes(value: &[u8]) -> Option<IpAddr> {
    match value.len() {
        4 => Some(IpAddr::from(<[u8; 4]>::try_from(value).ok()?)),
        16 => Some(IpAddr::from(<[u8; 16]>::try_from(value).ok()?)),
        _ => None,
    }
}

fn format_ip_bytes(value: &[u8]) -> Option<String> {
    parse_ip_bytes(value).map(|value| value.to_string())
}

fn hex_bytes(value: &[u8], separator: &str) -> String {
    value
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(separator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    use openssl::asn1::Asn1Integer;
    use openssl::bn::BigNum;
    use openssl::pkey::PKey;
    use openssl::rsa::Rsa;
    use openssl::ssl::{NameType, SslAcceptor};
    use openssl::x509::extension::{BasicConstraints, SubjectAlternativeName};
    use openssl::x509::{X509NameBuilder, X509};

    fn fixture_certificate(dns_name: &str) -> (PKey<openssl::pkey::Private>, X509) {
        let key = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
        let mut name = X509NameBuilder::new().unwrap();
        name.append_entry_by_text("CN", dns_name).unwrap();
        let name = name.build();
        let mut certificate = X509::builder().unwrap();
        certificate.set_version(2).unwrap();
        let serial = Asn1Integer::from_bn(&BigNum::from_u32(1).unwrap()).unwrap();
        certificate.set_serial_number(&serial).unwrap();
        certificate.set_subject_name(&name).unwrap();
        certificate.set_issuer_name(&name).unwrap();
        certificate.set_pubkey(&key).unwrap();
        certificate
            .set_not_before(&Asn1Time::days_from_now(0).unwrap())
            .unwrap();
        certificate
            .set_not_after(&Asn1Time::days_from_now(30).unwrap())
            .unwrap();
        certificate
            .append_extension(BasicConstraints::new().critical().ca().build().unwrap())
            .unwrap();
        let san = SubjectAlternativeName::new()
            .dns(dns_name)
            .build(&certificate.x509v3_context(None, None))
            .unwrap();
        certificate.append_extension(san).unwrap();
        certificate.sign(&key, MessageDigest::sha256()).unwrap();
        (key, certificate.build())
    }

    fn start_tls_fixture(dns_name: &str) -> (SocketAddr, mpsc::Receiver<Option<String>>) {
        let (key, certificate) = fixture_certificate(dns_name);
        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        acceptor.set_private_key(&key).unwrap();
        acceptor.set_certificate(&certificate).unwrap();
        acceptor.check_private_key().unwrap();
        let acceptor = acceptor.build();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let stream = acceptor.accept(stream).unwrap();
            let _ = sender.send(
                stream
                    .ssl()
                    .servername(NameType::HOST_NAME)
                    .map(str::to_string),
            );
        });
        (address, receiver)
    }

    #[test]
    fn established_stream_keeps_sni_and_verify_hostname_independent() {
        let (address, received_sni) = start_tls_fixture("certificate.test");
        let stream = TcpStream::connect(address).unwrap();
        let session = connect_tls_stream(
            stream,
            address.port(),
            TlsProbeConfig {
                connection_ip: address.ip(),
                sni: Some("routing.test".into()),
                verify_hostname: Some("certificate.test".into()),
                timeout: Duration::from_secs(2),
            },
        )
        .map_err(|result| result.error)
        .unwrap();

        assert!(session.result.handshake_succeeded);
        assert_eq!(session.result.connection_ip, address.ip());
        assert_eq!(
            session.result.hostname_verification.state,
            TlsVerificationState::Verified
        );
        assert_eq!(
            received_sni
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .as_deref(),
            Some("routing.test")
        );
        assert!(!session.result.certificate_chain.is_empty());
        assert_eq!(
            session.result.trust_verification.windows_state,
            TlsVerificationState::Unsupported
        );
        assert_eq!(
            session.result.trust_verification.revocation_state,
            TlsVerificationState::Unverified
        );
    }

    #[test]
    fn reports_hostname_mismatch_without_hiding_successful_handshake() {
        let (address, _) = start_tls_fixture("certificate.test");
        let result = probe_tls_address(
            address,
            Some("certificate.test"),
            Some("wrong.test"),
            Duration::from_secs(2),
        );
        assert!(result.handshake_succeeded);
        assert_eq!(
            result.hostname_verification.state,
            TlsVerificationState::Failed
        );
        assert_eq!(
            result.trust_verification.openssl_state,
            TlsVerificationState::Failed
        );
        assert!(result.error.is_none());
    }

    #[test]
    fn ip_sni_is_treated_as_no_sni() {
        let (address, received_sni) = start_tls_fixture("certificate.test");
        let result = probe_tls_address(
            address,
            Some("127.0.0.1"),
            Some("certificate.test"),
            Duration::from_secs(2),
        );
        assert!(result.handshake_succeeded);
        assert!(result.sni.is_none());
        assert_eq!(
            received_sni.recv_timeout(Duration::from_secs(1)).unwrap(),
            None
        );
    }

    #[test]
    fn reports_handshake_timeout_from_silent_peer() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(500));
        });
        let started = Instant::now();
        let result = probe_tls_address(address, None, None, Duration::from_millis(50));
        assert!(!result.handshake_succeeded);
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(
            result.error.as_ref().unwrap().stage,
            TlsProbeStage::TlsHandshake
        );
        assert_eq!(
            result.error.as_ref().unwrap().kind,
            TlsProbeErrorKind::Timeout
        );
    }

    #[test]
    fn wildcard_matches_exactly_one_label() {
        assert!(dns_identity_matches("*.example.test", "api.example.test"));
        assert!(!dns_identity_matches("*.example.test", "a.b.example.test"));
        assert!(!dns_identity_matches("*.test", "api.test"));
        assert!(!dns_identity_matches("f*.example.test", "foo.example.test"));
    }
}

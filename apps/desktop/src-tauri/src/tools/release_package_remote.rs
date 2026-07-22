use std::collections::HashMap;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD_NO_PAD;
use base64::Engine;
use serde::Serialize;
use ssh2::{HostKeyType, Session};
use uuid::Uuid;

const SSH_TIMEOUT: Duration = Duration::from_secs(10);
const PROBE_TTL: Duration = Duration::from_secs(300);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostTrust {
    Trusted,
    Unknown,
    Changed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteEndpoint {
    pub host: String,
    pub port: u16,
    pub username: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeSnapshot {
    pub endpoint: RemoteEndpoint,
    pub key_type: String,
    pub fingerprint_sha256: String,
}

struct ExpiringProbe {
    snapshot: ProbeSnapshot,
    expires_at: Instant,
}

static PROBES: OnceLock<Mutex<HashMap<String, ExpiringProbe>>> = OnceLock::new();

pub fn fingerprint_sha256(key: &[u8]) -> String {
    let digest = openssl::sha::sha256(key);
    format!("SHA256:{}", STANDARD_NO_PAD.encode(digest))
}

pub fn classify_trust(previous: Option<&str>, current: &str) -> HostTrust {
    match previous {
        None => HostTrust::Unknown,
        Some(value) if value == current => HostTrust::Trusted,
        Some(_) => HostTrust::Changed,
    }
}

pub fn validate_remote_dir(path: &str) -> Result<String, String> {
    validate_remote_path(path, false)
}

pub fn validate_remote_file(path: &str) -> Result<String, String> {
    validate_remote_path(path, true)
}

fn validate_remote_path(path: &str, file: bool) -> Result<String, String> {
    let value = path.trim();
    if value.is_empty() {
        return Err("远程路径不能为空".into());
    }
    if !value.starts_with('/') || value == "/" {
        return Err("远程路径必须是绝对 Linux 路径且不能是根目录".into());
    }
    if value.contains('\0') {
        return Err("远程路径不能包含 NUL".into());
    }
    if value
        .split('/')
        .any(|segment| segment == "." || segment == "..")
    {
        return Err("远程路径不能包含 . 或 .. 片段".into());
    }
    if file && value.ends_with('/') {
        return Err("远程文件路径必须包含文件名".into());
    }
    Ok(value.to_string())
}

pub fn create_session() -> Result<Session, String> {
    Session::new().map_err(|error| format!("创建 SSH 会话失败：{error}"))
}

fn host_key_type_name(kind: HostKeyType) -> String {
    match kind {
        HostKeyType::Unknown => "unknown",
        HostKeyType::Rsa => "rsa",
        HostKeyType::Dss => "dss",
        HostKeyType::Ecdsa256 => "ecdsa256",
        HostKeyType::Ecdsa384 => "ecdsa384",
        HostKeyType::Ecdsa521 => "ecdsa521",
        HostKeyType::Ed25519 => "ed25519",
    }
    .to_string()
}

fn connect_tcp(endpoint: &RemoteEndpoint) -> Result<TcpStream, String> {
    let addresses = (endpoint.host.as_str(), endpoint.port)
        .to_socket_addrs()
        .map_err(|error| format!("解析 SSH 服务器地址失败：{error}"))?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err("SSH 服务器地址未解析到可用 IP".into());
    }
    let mut last_error = None;
    for address in addresses {
        match TcpStream::connect_timeout(&address, SSH_TIMEOUT) {
            Ok(stream) => return Ok(stream),
            Err(error) => last_error = Some(error),
        }
    }
    Err(format!(
        "连接 SSH 服务器失败：{}",
        last_error.expect("resolved addresses are not empty")
    ))
}

pub fn probe_host(endpoint: &RemoteEndpoint) -> Result<ProbeSnapshot, String> {
    let stream = connect_tcp(endpoint)?;
    stream
        .set_read_timeout(Some(SSH_TIMEOUT))
        .map_err(|error| format!("设置 SSH 读超时失败：{error}"))?;
    stream
        .set_write_timeout(Some(SSH_TIMEOUT))
        .map_err(|error| format!("设置 SSH 写超时失败：{error}"))?;

    let mut session = create_session()?;
    session.set_tcp_stream(stream);
    session
        .handshake()
        .map_err(|error| format!("SSH 握手失败：{error}"))?;
    let (key, key_type) = session
        .host_key()
        .ok_or_else(|| "SSH 服务器未返回主机公钥".to_string())?;
    Ok(ProbeSnapshot {
        endpoint: endpoint.clone(),
        key_type: host_key_type_name(key_type),
        fingerprint_sha256: fingerprint_sha256(key),
    })
}

pub fn store_probe(snapshot: ProbeSnapshot) -> Result<String, String> {
    let token = Uuid::new_v4().to_string();
    let mut probes = PROBES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "SSH 探测令牌存储不可用".to_string())?;
    let now = Instant::now();
    probes.retain(|_, probe| probe.expires_at > now);
    probes.insert(
        token.clone(),
        ExpiringProbe {
            snapshot,
            expires_at: now + PROBE_TTL,
        },
    );
    Ok(token)
}

pub fn consume_probe(token: &str) -> Result<ProbeSnapshot, String> {
    let mut probes = PROBES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "SSH 探测令牌存储不可用".to_string())?;
    let probe = probes
        .remove(token)
        .ok_or_else(|| "SSH 探测令牌无效或已使用".to_string())?;
    if probe.expires_at <= Instant::now() {
        return Err("SSH 探测令牌已过期".into());
    }
    Ok(probe.snapshot)
}

#[cfg(test)]
mod tests {
    use super::{
        classify_trust, consume_probe, create_session, store_probe, validate_remote_dir,
        validate_remote_file, HostTrust, ProbeSnapshot, RemoteEndpoint,
    };

    fn snapshot() -> ProbeSnapshot {
        ProbeSnapshot {
            endpoint: RemoteEndpoint {
                host: "server.example.internal".into(),
                port: 22,
                username: "deploy".into(),
            },
            key_type: "ed25519".into(),
            fingerprint_sha256: "SHA256:new".into(),
        }
    }

    #[test]
    fn creates_an_ssh_session_without_network_access() {
        assert!(create_session().is_ok());
    }

    #[test]
    fn validates_absolute_linux_deployment_paths() {
        assert_eq!(validate_remote_dir("/srv/app/web").unwrap(), "/srv/app/web");
        assert_eq!(
            validate_remote_file("/srv/app/app.jar").unwrap(),
            "/srv/app/app.jar"
        );
        for invalid in [
            "",
            "/",
            "relative/path",
            "/srv/../root",
            "/srv/./app",
            "/srv/app\0x",
        ] {
            assert!(
                validate_remote_dir(invalid).is_err(),
                "accepted {invalid:?}"
            );
        }
        assert!(validate_remote_file("/srv/app/").is_err());
    }

    #[test]
    fn classifies_known_host_without_silent_replacement() {
        assert_eq!(classify_trust(None, "SHA256:new"), HostTrust::Unknown);
        assert_eq!(
            classify_trust(Some("SHA256:new"), "SHA256:new"),
            HostTrust::Trusted
        );
        assert_eq!(
            classify_trust(Some("SHA256:old"), "SHA256:new"),
            HostTrust::Changed
        );
    }

    #[test]
    fn probe_tokens_are_consumed_once() {
        let token = store_probe(snapshot()).unwrap();
        assert_eq!(consume_probe(&token).unwrap(), snapshot());
        assert!(consume_probe(&token).is_err());
    }
}

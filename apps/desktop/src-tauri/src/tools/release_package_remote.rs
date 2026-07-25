use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::Serialize;
use ssh2::{ErrorCode, FileType, HostKeyType, Session, Sftp};
use uuid::Uuid;
use zeroize::Zeroizing;

use super::release_package_deploy::{
    DeployError, RemoteDirEntry, RemoteFs, RemoteKind, RemoteMetadata,
};

const SSH_TIMEOUT: Duration = Duration::from_secs(10);
const PROBE_TTL: Duration = Duration::from_secs(300);
const PREFLIGHT_TTL: Duration = Duration::from_secs(300);

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTarget {
    Frontend,
    Backend,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreflightBinding {
    pub project_id: i64,
    pub endpoint: RemoteEndpoint,
    pub auth_type: String,
    pub vault_entry_id: Option<i64>,
    pub private_key_path: String,
    pub targets: Vec<RemoteTarget>,
    pub frontend_remote_dir: String,
    pub backend_remote_path: String,
}

pub enum AuthSecret {
    Password(Zeroizing<String>),
    PrivateKeyPassphrase(Option<Zeroizing<String>>),
}

pub struct ConsumedPreflight {
    pub binding: PreflightBinding,
    pub expected_fingerprint: String,
    pub secret: AuthSecret,
    pub expected_existing_targets: Vec<RemoteTarget>,
}

struct ExpiringPreflight {
    value: ConsumedPreflight,
    expires_at: Instant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssuedPreflight {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

pub struct PreflightStore {
    ttl: Duration,
    values: Mutex<HashMap<String, ExpiringPreflight>>,
}

impl PreflightStore {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            values: Mutex::new(HashMap::new()),
        }
    }

    pub fn insert(
        &self,
        binding: PreflightBinding,
        expected_fingerprint: String,
        secret: AuthSecret,
        expected_existing_targets: Vec<RemoteTarget>,
    ) -> Result<IssuedPreflight, String> {
        let token = Uuid::new_v4().to_string();
        let now = Instant::now();
        let expires_at = Utc::now()
            + chrono::Duration::from_std(self.ttl)
                .map_err(|_| "SSH 预检令牌有效期无效".to_string())?;
        let mut values = self
            .values
            .lock()
            .map_err(|_| "SSH 预检令牌存储不可用".to_string())?;
        values.retain(|_, value| value.expires_at > now);
        values.insert(
            token.clone(),
            ExpiringPreflight {
                value: ConsumedPreflight {
                    binding,
                    expected_fingerprint,
                    secret,
                    expected_existing_targets,
                },
                expires_at: now + self.ttl,
            },
        );
        Ok(IssuedPreflight { token, expires_at })
    }

    pub fn consume(
        &self,
        token: &str,
        binding: &PreflightBinding,
    ) -> Result<ConsumedPreflight, String> {
        let mut values = self
            .values
            .lock()
            .map_err(|_| "SSH 预检令牌存储不可用".to_string())?;
        let value = values
            .remove(token)
            .ok_or_else(|| "SSH 预检令牌无效或已使用".to_string())?;
        if value.expires_at <= Instant::now() {
            return Err("SSH 预检令牌已过期".into());
        }
        if &value.value.binding != binding {
            return Err("项目或远程上传配置已变化，请重新预检".into());
        }
        Ok(value.value)
    }

    fn clear(&self) {
        if let Ok(mut values) = self.values.lock() {
            values.clear();
        }
    }
}

static PROBES: OnceLock<Mutex<HashMap<String, ExpiringProbe>>> = OnceLock::new();
static PREFLIGHTS: OnceLock<PreflightStore> = OnceLock::new();

fn preflight_store() -> &'static PreflightStore {
    PREFLIGHTS.get_or_init(|| PreflightStore::new(PREFLIGHT_TTL))
}

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
    validate_remote_path(path)
}

pub fn validate_remote_file(path: &str) -> Result<String, String> {
    validate_remote_path(path)
}

fn validate_remote_path(path: &str) -> Result<String, String> {
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
    if value.ends_with('/') || value.contains("//") || value.contains('\\') {
        return Err("远程路径必须使用规范的 Linux 绝对路径".into());
    }
    if value
        .split('/')
        .any(|segment| segment == "." || segment == "..")
    {
        return Err("远程路径不能包含 . 或 .. 片段".into());
    }
    Ok(value.to_string())
}

fn validate_target_relationships(binding: &PreflightBinding) -> Result<(), String> {
    let paths = binding
        .targets
        .iter()
        .map(|target| match target {
            RemoteTarget::Frontend => binding.frontend_remote_dir.as_str(),
            RemoteTarget::Backend => binding.backend_remote_path.as_str(),
        })
        .collect::<Vec<_>>();

    for (index, path) in paths.iter().enumerate() {
        for other in &paths[index + 1..] {
            let path_segments = path
                .split('/')
                .filter(|segment| !segment.is_empty())
                .collect::<Vec<_>>();
            let other_segments = other
                .split('/')
                .filter(|segment| !segment.is_empty())
                .collect::<Vec<_>>();
            if path_segments.starts_with(&other_segments)
                || other_segments.starts_with(&path_segments)
            {
                return Err(format!("远端部署目标不能互相包含或重复：{path}；{other}"));
            }
        }
    }
    Ok(())
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

fn handshake_session(endpoint: &RemoteEndpoint) -> Result<Session, String> {
    handshake_session_with_socket(endpoint, None)
}

fn handshake_session_with_socket(
    endpoint: &RemoteEndpoint,
    socket_slot: Option<&Arc<Mutex<Option<TcpStream>>>>,
) -> Result<Session, String> {
    let stream = connect_tcp(endpoint)?;
    stream
        .set_read_timeout(Some(SSH_TIMEOUT))
        .map_err(|error| format!("设置 SSH 读超时失败：{error}"))?;
    stream
        .set_write_timeout(Some(SSH_TIMEOUT))
        .map_err(|error| format!("设置 SSH 写超时失败：{error}"))?;
    if let Some(socket_slot) = socket_slot {
        let socket = stream
            .try_clone()
            .map_err(|error| format!("保存 SSH 连接失败：{error}"))?;
        *socket_slot
            .lock()
            .map_err(|_| "SSH 连接状态不可用".to_string())? = Some(socket);
    }
    let mut session = create_session()?;
    session.set_tcp_stream(stream);
    session
        .handshake()
        .map_err(|error| format!("SSH 握手失败：{error}"))?;
    Ok(session)
}
pub fn probe_host(endpoint: &RemoteEndpoint) -> Result<ProbeSnapshot, String> {
    let session = handshake_session(endpoint)?;
    let (key, key_type) = session
        .host_key()
        .ok_or_else(|| "SSH 服务器未返回主机公钥".to_string())?;
    Ok(ProbeSnapshot {
        endpoint: endpoint.clone(),
        key_type: host_key_type_name(key_type),
        fingerprint_sha256: fingerprint_sha256(key),
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTargetCheck {
    pub target: RemoteTarget,
    pub remote_path: String,
    pub exists: bool,
    pub parent_ready: bool,
    pub writable: bool,
}

fn session_fingerprint(session: &Session) -> Result<String, String> {
    let (key, _) = session
        .host_key()
        .ok_or_else(|| "SSH 服务器未返回主机公钥".to_string())?;
    Ok(fingerprint_sha256(key))
}

fn authenticate_session(
    session: &Session,
    endpoint: &RemoteEndpoint,
    private_key_path: &str,
    secret: &AuthSecret,
) -> Result<(), String> {
    match secret {
        AuthSecret::Password(password) => session
            .userauth_password(&endpoint.username, password.as_str())
            .map_err(|_| "SSH 用户名或密码认证失败".to_string())?,
        AuthSecret::PrivateKeyPassphrase(passphrase) => {
            let private_key = Path::new(private_key_path);
            if !private_key.is_file() {
                return Err("SSH 私钥文件不存在或不是常规文件".into());
            }
            session
                .userauth_pubkey_file(
                    &endpoint.username,
                    None,
                    private_key,
                    passphrase.as_ref().map(|value| value.as_str()),
                )
                .map_err(|_| "SSH 私钥认证失败，请检查私钥和口令".to_string())?;
        }
    }
    if !session.authenticated() {
        return Err("SSH 认证失败".into());
    }
    Ok(())
}

#[cfg(test)]
fn authenticate_for_test(_username: &str, secret: AuthSecret) -> Result<(), String> {
    match secret {
        AuthSecret::Password(_) => Err("SSH 用户名或密码认证失败".into()),
        AuthSecret::PrivateKeyPassphrase(_) => Err("SSH 私钥认证失败，请检查私钥和口令".into()),
    }
}

fn is_remote_missing(error: &ssh2::Error) -> bool {
    matches!(error.code(), ErrorCode::SFTP(2))
}

fn remote_stat(sftp: &Sftp, path: &Path) -> Result<Option<ssh2::FileStat>, String> {
    match sftp.stat(path) {
        Ok(stat) => Ok(Some(stat)),
        Err(error) if is_remote_missing(&error) => Ok(None),
        Err(error) => Err(format!("读取远程路径失败：{error}")),
    }
}

fn remote_parent(path: &str) -> Result<String, String> {
    let (parent, name) = path
        .rsplit_once('/')
        .ok_or_else(|| "远程路径缺少父目录".to_string())?;
    if parent.is_empty() || name.is_empty() {
        return Err("远程路径缺少有效父目录或名称".into());
    }
    Ok(parent.to_string())
}

fn check_target(
    sftp: &Sftp,
    target: RemoteTarget,
    path: &str,
    run_suffix: &str,
) -> Result<RemoteTargetCheck, String> {
    let parent = remote_parent(path)?;
    let parent_path = Path::new(&parent);
    let parent_stat =
        remote_stat(sftp, parent_path)?.ok_or_else(|| format!("远程父目录不存在：{parent}"))?;
    if !parent_stat.is_dir() {
        return Err(format!("远程父路径不是目录：{parent}"));
    }

    let target_path = Path::new(path);
    let target_stat = remote_stat(sftp, target_path)?;
    if let Some(stat) = &target_stat {
        let expected_type = match target {
            RemoteTarget::Frontend => stat.is_dir(),
            RemoteTarget::Backend => stat.is_file(),
        };
        if !expected_type {
            return Err(format!("远程目标类型不符合配置：{path}"));
        }
    }

    for suffix in ["tmp", "backup"] {
        let transaction_path = PathBuf::from(format!("{path}.__lazycat_{suffix}_{run_suffix}"));
        if remote_stat(sftp, &transaction_path)?.is_some() {
            return Err(format!(
                "远程部署临时路径已存在：{}",
                transaction_path.display()
            ));
        }
    }

    let probe = PathBuf::from(format!("{parent}/.lazycat-preflight-{run_suffix}"));
    let renamed = PathBuf::from(format!("{parent}/.lazycat-preflight-{run_suffix}-renamed"));
    if remote_stat(sftp, &probe)?.is_some() || remote_stat(sftp, &renamed)?.is_some() {
        return Err("远程预检探针路径已存在，请稍后重试".into());
    }
    let file = sftp
        .create(&probe)
        .map_err(|_| format!("远程父目录不可写：{parent}"))?;
    drop(file);
    if let Err(error) = sftp.rename(&probe, &renamed, None) {
        let _ = sftp.unlink(&probe);
        return Err(format!("远程父目录不支持重命名：{error}"));
    }
    if let Err(error) = sftp.unlink(&renamed) {
        return Err(format!("远程预检探针清理失败：{error}"));
    }

    Ok(RemoteTargetCheck {
        target,
        remote_path: path.to_string(),
        exists: target_stat.is_some(),
        parent_ready: true,
        writable: true,
    })
}

pub fn run_remote_preflight(
    binding: &PreflightBinding,
    expected_fingerprint: &str,
    secret: &AuthSecret,
) -> Result<Vec<RemoteTargetCheck>, String> {
    validate_target_relationships(binding)?;
    let session = handshake_session(&binding.endpoint)?;
    if session_fingerprint(&session)? != expected_fingerprint {
        return Err("SSH 主机指纹与已信任记录不一致".into());
    }
    authenticate_session(
        &session,
        &binding.endpoint,
        &binding.private_key_path,
        secret,
    )?;
    let sftp = session
        .sftp()
        .map_err(|_| "初始化 SFTP 会话失败".to_string())?;
    let run_suffix = Uuid::new_v4().simple().to_string();
    let run_suffix = &run_suffix[..12];
    binding
        .targets
        .iter()
        .map(|target| {
            let path = match target {
                RemoteTarget::Frontend => &binding.frontend_remote_dir,
                RemoteTarget::Backend => &binding.backend_remote_path,
            };
            check_target(&sftp, *target, path, run_suffix)
        })
        .collect()
}

pub struct SftpRemoteFs {
    sftp: Sftp,
}

fn remote_kind(file_type: FileType) -> RemoteKind {
    match file_type {
        FileType::RegularFile => RemoteKind::File,
        FileType::Directory => RemoteKind::Directory,
        FileType::Symlink => RemoteKind::Symlink,
        _ => RemoteKind::Other,
    }
}

impl SftpRemoteFs {
    pub fn connect(
        binding: &PreflightBinding,
        expected_fingerprint: &str,
        secret: &AuthSecret,
        socket_slot: &Arc<Mutex<Option<TcpStream>>>,
    ) -> Result<Self, DeployError> {
        let session = handshake_session_with_socket(&binding.endpoint, Some(socket_slot))
            .map_err(DeployError::failed)?;
        if session_fingerprint(&session).map_err(DeployError::failed)? != expected_fingerprint {
            return Err(DeployError::failed("SSH 主机指纹与已信任记录不一致"));
        }
        authenticate_session(
            &session,
            &binding.endpoint,
            &binding.private_key_path,
            secret,
        )
        .map_err(DeployError::failed)?;
        let sftp = session
            .sftp()
            .map_err(|_| DeployError::failed("初始化 SFTP 会话失败"))?;
        Ok(Self { sftp })
    }

    fn metadata_inner(&self, path: &str) -> Result<Option<RemoteMetadata>, DeployError> {
        match self.sftp.lstat(Path::new(path)) {
            Ok(stat) => Ok(Some(RemoteMetadata {
                kind: remote_kind(stat.file_type()),
                size: stat.size.unwrap_or(0),
            })),
            Err(error) if is_remote_missing(&error) => Ok(None),
            Err(error) => Err(DeployError::failed(format!(
                "读取远端路径失败（{path}）：{error}"
            ))),
        }
    }
}

impl RemoteFs for SftpRemoteFs {
    fn metadata(&self, path: &str) -> Result<Option<RemoteMetadata>, DeployError> {
        self.metadata_inner(path)
    }

    fn create_dir(&mut self, path: &str) -> Result<(), DeployError> {
        self.sftp
            .mkdir(Path::new(path), 0o755)
            .map_err(|error| DeployError::failed(format!("创建远端目录失败（{path}）：{error}")))
    }

    fn read_dir(&self, path: &str) -> Result<Vec<RemoteDirEntry>, DeployError> {
        self.sftp
            .readdir(Path::new(path))
            .map_err(|error| DeployError::failed(format!("读取远端目录失败（{path}）：{error}")))?
            .into_iter()
            .map(|(entry_path, stat)| {
                let entry_path = entry_path
                    .to_str()
                    .ok_or_else(|| DeployError::failed("远端路径不是有效 UTF-8"))?
                    .replace('\\', "/");
                Ok(RemoteDirEntry {
                    path: entry_path,
                    metadata: RemoteMetadata {
                        kind: remote_kind(stat.file_type()),
                        size: stat.size.unwrap_or(0),
                    },
                })
            })
            .collect()
    }

    fn write_file(
        &mut self,
        remote_path: &str,
        local_path: &Path,
        cancelled: &AtomicBool,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), DeployError> {
        let mut local = File::open(local_path).map_err(DeployError::local_io)?;
        let mut remote = self.sftp.create(Path::new(remote_path)).map_err(|error| {
            DeployError::failed(format!("创建远端文件失败（{remote_path}）：{error}"))
        })?;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            if cancelled.load(Ordering::Acquire) {
                return Err(DeployError::cancelled());
            }
            let size = local.read(&mut buffer).map_err(DeployError::local_io)?;
            if size == 0 {
                break;
            }
            remote.write_all(&buffer[..size]).map_err(|error| {
                DeployError::failed(format!("写入远端文件失败（{remote_path}）：{error}"))
            })?;
            if cancelled.load(Ordering::Acquire) {
                return Err(DeployError::cancelled());
            }
            progress(size as u64);
        }
        remote.flush().map_err(|error| {
            DeployError::failed(format!("完成远端文件失败（{remote_path}）：{error}"))
        })
    }

    fn rename(&mut self, source: &str, target: &str) -> Result<(), DeployError> {
        self.sftp
            .rename(Path::new(source), Path::new(target), None)
            .map_err(|error| {
                DeployError::failed(format!(
                    "重命名远端路径失败（{source} → {target}）：{error}"
                ))
            })
    }

    fn remove_tree(&mut self, path: &str) -> Result<(), DeployError> {
        let Some(metadata) = self.metadata_inner(path)? else {
            return Ok(());
        };
        match metadata.kind {
            RemoteKind::Directory => {
                for entry in self.read_dir(path)? {
                    self.remove_tree(&entry.path)?;
                }
                self.sftp.rmdir(Path::new(path)).map_err(|error| {
                    DeployError::failed(format!("删除远端目录失败（{path}）：{error}"))
                })
            }
            RemoteKind::File | RemoteKind::Symlink => {
                self.sftp.unlink(Path::new(path)).map_err(|error| {
                    DeployError::failed(format!("删除远端文件失败（{path}）：{error}"))
                })
            }
            RemoteKind::Other => Err(DeployError::failed(format!(
                "拒绝删除未知类型的远端路径：{path}"
            ))),
        }
    }
}
pub fn issue_preflight(
    binding: PreflightBinding,
    expected_fingerprint: String,
    secret: AuthSecret,
    checks: &[RemoteTargetCheck],
) -> Result<IssuedPreflight, String> {
    let expected_existing_targets = checks
        .iter()
        .filter(|check| check.exists)
        .map(|check| check.target)
        .collect();
    preflight_store().insert(
        binding,
        expected_fingerprint,
        secret,
        expected_existing_targets,
    )
}

pub fn consume_preflight(
    token: &str,
    binding: &PreflightBinding,
) -> Result<ConsumedPreflight, String> {
    preflight_store().consume(token, binding)
}

pub fn clear_temporary_stores() {
    if let Some(probes) = PROBES.get() {
        if let Ok(mut probes) = probes.lock() {
            probes.clear();
        }
    }
    if let Some(preflights) = PREFLIGHTS.get() {
        preflights.clear();
    }
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

pub fn load_probe(token: &str) -> Result<ProbeSnapshot, String> {
    let mut probes = PROBES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "SSH 探测令牌存储不可用".to_string())?;
    let now = Instant::now();
    probes.retain(|_, probe| probe.expires_at > now);
    probes
        .get(token)
        .map(|probe| probe.snapshot.clone())
        .ok_or_else(|| "SSH 探测令牌无效或已过期".to_string())
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
        authenticate_for_test, classify_trust, consume_probe, create_session, load_probe,
        probe_host, store_probe, validate_remote_dir, validate_remote_file,
        validate_target_relationships, AuthSecret, HostTrust, PreflightBinding, PreflightStore,
        ProbeSnapshot, RemoteEndpoint, RemoteTarget, SftpRemoteFs,
    };
    use crate::tools::release_package::ReleaseTarget;
    use crate::tools::release_package_deploy::{
        deploy, ArtifactManifest, DeploymentRequest, DeploymentTarget, RemoteFs, RemoteKind,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use uuid::Uuid;
    use zeroize::Zeroizing;

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
            "/srv/app/web/",
            "/srv//app",
            r"/srv\app",
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

    fn binding(targets: Vec<RemoteTarget>) -> PreflightBinding {
        PreflightBinding {
            project_id: 7,
            endpoint: RemoteEndpoint {
                host: "server.example.internal".into(),
                port: 22,
                username: "deploy".into(),
            },
            auth_type: "password".into(),
            vault_entry_id: None,
            private_key_path: String::new(),
            targets,
            frontend_remote_dir: "/srv/app/web".into(),
            backend_remote_path: "/srv/app/app.jar".into(),
        }
    }

    #[test]
    fn rejects_duplicate_or_nested_selected_remote_targets() {
        for (frontend, backend) in [
            ("/srv/app", "/srv/app"),
            ("/srv/app", "/srv/app/app.jar"),
            ("/srv/app/web", "/srv/app"),
        ] {
            let mut binding = binding(vec![RemoteTarget::Frontend, RemoteTarget::Backend]);
            binding.frontend_remote_dir = frontend.into();
            binding.backend_remote_path = backend.into();

            let error = validate_target_relationships(&binding).unwrap_err();

            assert!(
                error.contains(frontend),
                "missing {frontend:?} in {error:?}"
            );
            assert!(error.contains(backend), "missing {backend:?} in {error:?}");
        }
    }

    #[test]
    fn accepts_sibling_and_similar_prefix_remote_targets() {
        for (frontend, backend) in [
            ("/srv/app/web", "/srv/app/app.jar"),
            ("/srv/app", "/srv/app2"),
        ] {
            let mut binding = binding(vec![RemoteTarget::Frontend, RemoteTarget::Backend]);
            binding.frontend_remote_dir = frontend.into();
            binding.backend_remote_path = backend.into();

            validate_target_relationships(&binding).unwrap();
        }
    }

    #[test]
    fn ignores_unselected_remote_target_when_validating_relationships() {
        let mut binding = binding(vec![RemoteTarget::Frontend]);
        binding.frontend_remote_dir = "/srv/app".into();
        binding.backend_remote_path = "/srv/app/app.jar".into();

        validate_target_relationships(&binding).unwrap();
    }

    #[test]
    fn preflight_token_is_bound_and_consumed_once() {
        let store = PreflightStore::new(Duration::from_secs(300));
        let binding = binding(vec![RemoteTarget::Frontend]);
        let issued = store
            .insert(
                binding.clone(),
                "SHA256:trusted".into(),
                AuthSecret::Password(Zeroizing::new("secret".into())),
                vec![],
            )
            .unwrap();
        let consumed = store.consume(&issued.token, &binding).unwrap();
        assert_eq!(consumed.expected_fingerprint, "SHA256:trusted");
        assert!(store.consume(&issued.token, &binding).is_err());
    }

    #[test]
    fn preflight_token_rejects_changed_remote_paths() {
        let store = PreflightStore::new(Duration::from_secs(300));
        let binding = binding(vec![RemoteTarget::Backend]);
        let issued = store
            .insert(
                binding.clone(),
                "SHA256:trusted".into(),
                AuthSecret::Password(Zeroizing::new("secret".into())),
                vec![],
            )
            .unwrap();
        let mut changed = binding;
        changed.backend_remote_path = "/srv/other/app.jar".into();
        assert!(store.consume(&issued.token, &changed).is_err());
    }

    #[test]
    fn preflight_token_rejects_changed_endpoint_port() {
        let store = PreflightStore::new(Duration::from_secs(300));
        let binding = binding(vec![RemoteTarget::Frontend]);
        let issued = store
            .insert(
                binding.clone(),
                "SHA256:trusted".into(),
                AuthSecret::Password(Zeroizing::new("secret".into())),
                vec![],
            )
            .unwrap();
        let mut changed = binding;
        changed.endpoint.port = 2200;

        assert_eq!(
            store.consume(&issued.token, &changed).err().unwrap(),
            "项目或远程上传配置已变化，请重新预检"
        );
    }

    #[test]
    fn preflight_binding_changes_when_vault_credential_changes() {
        let mut first = binding(vec![RemoteTarget::Frontend]);
        first.vault_entry_id = Some(1);
        let mut second = first.clone();
        second.vault_entry_id = Some(2);

        assert_ne!(first, second);
    }

    #[test]
    fn authentication_errors_never_include_the_secret() {
        let error = authenticate_for_test(
            "deploy",
            AuthSecret::Password(Zeroizing::new("top-secret".into())),
        )
        .unwrap_err();
        assert!(!error.contains("top-secret"));
    }
    #[test]
    fn probe_tokens_can_be_read_for_repeated_authentication_attempts() {
        let token = store_probe(snapshot()).unwrap();
        assert_eq!(load_probe(&token).unwrap(), snapshot());
        assert_eq!(load_probe(&token).unwrap(), snapshot());
        assert_eq!(consume_probe(&token).unwrap(), snapshot());
        assert!(load_probe(&token).is_err());
    }
    #[test]
    fn probe_tokens_are_consumed_once() {
        let token = store_probe(snapshot()).unwrap();
        assert_eq!(consume_probe(&token).unwrap(), snapshot());
        assert!(consume_probe(&token).is_err());
    }

    struct SshTestFixture {
        endpoint: RemoteEndpoint,
        password: String,
        private_key_path: String,
        private_key_passphrase: Option<String>,
    }

    impl SshTestFixture {
        fn from_env() -> Result<Self, String> {
            let required = |name: &str| std::env::var(name).map_err(|_| format!("missing {name}"));
            let host = required("LAZYCAT_SSH_TEST_HOST")?;
            if !matches!(host.as_str(), "127.0.0.1" | "localhost" | "::1") {
                return Err("LAZYCAT_SSH_TEST_HOST must be loopback".into());
            }
            let port = required("LAZYCAT_SSH_TEST_PORT")?
                .parse::<u16>()
                .map_err(|error| format!("invalid LAZYCAT_SSH_TEST_PORT: {error}"))?;
            Ok(Self {
                endpoint: RemoteEndpoint {
                    host,
                    port,
                    username: required("LAZYCAT_SSH_TEST_USERNAME")?,
                },
                password: required("LAZYCAT_SSH_TEST_PASSWORD")?,
                private_key_path: required("LAZYCAT_SSH_TEST_PRIVATE_KEY_PATH")?,
                private_key_passphrase: std::env::var("LAZYCAT_SSH_TEST_PRIVATE_KEY_PASSPHRASE")
                    .ok()
                    .filter(|value| !value.is_empty()),
            })
        }

        fn binding(&self, remote_root: &str, auth_type: &str) -> PreflightBinding {
            PreflightBinding {
                project_id: 1,
                endpoint: self.endpoint.clone(),
                auth_type: auth_type.into(),
                vault_entry_id: None,
                private_key_path: self.private_key_path.clone(),
                targets: vec![RemoteTarget::Frontend, RemoteTarget::Backend],
                frontend_remote_dir: format!("{remote_root}/web"),
                backend_remote_path: format!("{remote_root}/app.jar"),
            }
        }

        fn password_auth(&self) -> AuthSecret {
            AuthSecret::Password(Zeroizing::new(self.password.clone()))
        }

        fn private_key_auth(&self) -> AuthSecret {
            AuthSecret::PrivateKeyPassphrase(
                self.private_key_passphrase.clone().map(Zeroizing::new),
            )
        }
    }

    struct LocalFixtureDir(PathBuf);

    impl LocalFixtureDir {
        fn create() -> Result<Self, String> {
            let path = std::env::temp_dir().join(format!(
                "lazycat-release-package-test-{}",
                Uuid::new_v4().simple()
            ));
            fs::create_dir_all(&path).map_err(|error| error.to_string())?;
            Ok(Self(path))
        }
    }

    impl Drop for LocalFixtureDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_local_fixture(
        root: &Path,
        empty_frontend: bool,
    ) -> Result<(PathBuf, PathBuf), String> {
        let frontend = root.join("frontend");
        let backend = root.join("app.jar");
        fs::create_dir_all(&frontend).map_err(|error| error.to_string())?;
        if !empty_frontend {
            let assets = frontend.join("assets");
            fs::create_dir_all(assets.join("empty")).map_err(|error| error.to_string())?;
            fs::write(assets.join("app.js"), b"console.log('lazycat');")
                .map_err(|error| error.to_string())?;
        }
        let backend_bytes = if empty_frontend {
            b"replacement".to_vec()
        } else {
            vec![0x5a; 2 * 1024 * 1024 + 17]
        };
        fs::write(&backend, backend_bytes).map_err(|error| error.to_string())?;
        Ok((frontend, backend))
    }

    fn deployment_request(
        local_root: &Path,
        remote_root: &str,
        run_id: &str,
        expected_exists: bool,
        empty_frontend: bool,
    ) -> Result<DeploymentRequest, String> {
        let (frontend, backend) = write_local_fixture(local_root, empty_frontend)?;
        Ok(DeploymentRequest {
            run_id: run_id.into(),
            targets: vec![
                DeploymentTarget {
                    manifest: ArtifactManifest::from_directory(ReleaseTarget::Frontend, &frontend)?,
                    remote_path: format!("{remote_root}/web"),
                    expected_exists,
                },
                DeploymentTarget {
                    manifest: ArtifactManifest::from_file(ReleaseTarget::Backend, &backend)?,
                    remote_path: format!("{remote_root}/app.jar"),
                    expected_exists,
                },
            ],
        })
    }

    fn run_fixture_deployment(
        fixture: &SshTestFixture,
        fingerprint: &str,
        auth_type: &str,
        auth: AuthSecret,
    ) -> Result<(), String> {
        let suffix = Uuid::new_v4().simple().to_string();
        let remote_root = format!("/tmp/lazycat-release-package-test-{suffix}");
        let binding = fixture.binding(&remote_root, auth_type);
        let socket_slot = Arc::new(Mutex::new(None));
        let mut remote = SftpRemoteFs::connect(&binding, fingerprint, &auth, &socket_slot)
            .map_err(|error| error.to_string())?;
        let local = LocalFixtureDir::create()?;

        let scenario = (|| -> Result<(), String> {
            let initial = deployment_request(&local.0, &remote_root, "initial", false, false)?;
            let mut uploaded = 0_u64;
            deploy(
                &mut remote,
                &initial,
                &AtomicBool::new(false),
                |bytes, _| uploaded += bytes,
            )
            .map_err(|error| error.to_string())?;
            if uploaded
                != initial
                    .targets
                    .iter()
                    .map(|target| target.manifest.total_bytes)
                    .sum::<u64>()
            {
                return Err("uploaded byte count mismatch".into());
            }
            let nested = remote
                .metadata(&format!("{remote_root}/web/assets/app.js"))
                .map_err(|error| error.to_string())?
                .ok_or("recursive frontend file missing")?;
            if nested.kind != RemoteKind::File {
                return Err("recursive frontend path is not a file".into());
            }

            fs::remove_dir_all(local.0.join("frontend")).map_err(|error| error.to_string())?;
            let replacement =
                deployment_request(&local.0, &remote_root, "replacement", true, true)?;
            deploy(
                &mut remote,
                &replacement,
                &AtomicBool::new(false),
                |_, _| {},
            )
            .map_err(|error| error.to_string())?;
            if !remote
                .read_dir(&format!("{remote_root}/web"))
                .map_err(|error| error.to_string())?
                .is_empty()
            {
                return Err("empty frontend directory was not preserved".into());
            }

            let cancelled = deployment_request(&local.0, &remote_root, "cancelled", true, true)?;
            let error = deploy(&mut remote, &cancelled, &AtomicBool::new(true), |_, _| {})
                .expect_err("cancelled deployment should fail");
            let formal_target = remote
                .metadata(&format!("{remote_root}/web"))
                .map_err(|error| error.to_string())?;
            if !error.cancelled
                || !matches!(formal_target, Some(metadata) if metadata.kind == RemoteKind::Directory)
            {
                return Err("cancelled deployment damaged the formal target".into());
            }
            Ok(())
        })();

        let cleanup = remote
            .remove_tree(&remote_root)
            .map_err(|error| error.to_string());
        scenario?;
        cleanup
    }

    #[test]
    #[ignore = "requires LAZYCAT_SSH_TEST_* variables and a loopback SSH fixture"]
    fn password_and_private_key_upload_to_local_fixture() {
        let fixture = SshTestFixture::from_env().unwrap();
        let probe = probe_host(&fixture.endpoint).unwrap();

        let password_binding =
            fixture.binding("/tmp/lazycat-release-package-test-auth", "password");
        let socket_slot = Arc::new(Mutex::new(None));
        assert!(SftpRemoteFs::connect(
            &password_binding,
            "SHA256:untrusted",
            &fixture.password_auth(),
            &socket_slot,
        )
        .is_err());
        let wrong_password = AuthSecret::Password(Zeroizing::new("definitely-wrong".into()));
        assert!(SftpRemoteFs::connect(
            &password_binding,
            &probe.fingerprint_sha256,
            &wrong_password,
            &Arc::new(Mutex::new(None)),
        )
        .is_err());

        run_fixture_deployment(
            &fixture,
            &probe.fingerprint_sha256,
            "password",
            fixture.password_auth(),
        )
        .unwrap();
        run_fixture_deployment(
            &fixture,
            &probe.fingerprint_sha256,
            "private_key",
            fixture.private_key_auth(),
        )
        .unwrap();
    }
}

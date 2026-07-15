use std::any::Any;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, UdpSocket as StdUdpSocket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use tokio::net::{lookup_host, UdpSocket};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use super::model::ForwardRule;
#[cfg(test)]
pub(super) use super::observability::UdpEventKind;
use super::observability::UdpObservability;
#[cfg(test)]
use super::observability::UdpObservationSnapshot;
use super::runtime::{RuleRunner, RunningHandle};

pub(crate) const UDP_MAX_SESSIONS_PER_RULE: usize = 256;
pub(crate) const UDP_SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
pub(crate) const UDP_SESSION_CLEANUP_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy)]
pub(crate) struct UdpLimits {
    max_sessions: usize,
    idle_timeout: Duration,
    cleanup_interval: Duration,
}

impl UdpLimits {
    const fn production() -> Self {
        Self {
            max_sessions: UDP_MAX_SESSIONS_PER_RULE,
            idle_timeout: UDP_SESSION_IDLE_TIMEOUT,
            cleanup_interval: UDP_SESSION_CLEANUP_INTERVAL,
        }
    }

    #[cfg(test)]
    fn for_test(max_sessions: usize, idle_timeout: Duration, cleanup_interval: Duration) -> Self {
        Self {
            max_sessions,
            idle_timeout,
            cleanup_interval,
        }
    }

    fn assert_valid(self) {
        assert!(self.max_sessions > 0, "UDP session limit must be positive");
        assert!(
            !self.idle_timeout.is_zero(),
            "UDP idle timeout must be positive"
        );
        assert!(
            !self.cleanup_interval.is_zero(),
            "UDP cleanup interval must be positive"
        );
    }
}

pub(crate) struct UdpRuleRunner {
    next_handle: AtomicU64,
    running: Mutex<HashMap<u64, UdpRunningRule>>,
    limits: UdpLimits,
}

struct UdpRunningRule {
    #[cfg(test)]
    listener_addr: SocketAddr,
    cancellation: CancellationToken,
    observability: Arc<UdpObservability>,
    #[cfg(test)]
    sessions: Arc<UdpSessionRegistry>,
    completion: Arc<UdpWorkerCompletion>,
    worker: JoinHandle<Result<(), String>>,
}

struct UdpSession {
    downstream: Arc<UdpSocket>,
    cancellation: CancellationToken,
    last_active: Mutex<Instant>,
    target_addr: SocketAddr,
}

impl UdpSession {
    fn new(downstream: UdpSocket, target_addr: SocketAddr) -> Self {
        Self {
            downstream: Arc::new(downstream),
            cancellation: CancellationToken::new(),
            last_active: Mutex::new(Instant::now()),
            target_addr,
        }
    }

    fn touch(&self) {
        *self
            .last_active
            .lock()
            .expect("UDP session activity lock poisoned") = Instant::now();
    }

    fn is_idle(&self, now: Instant, timeout: Duration) -> bool {
        now.saturating_duration_since(
            *self
                .last_active
                .lock()
                .expect("UDP session activity lock poisoned"),
        ) >= timeout
    }
}

#[derive(Default)]
struct UdpSessionRegistry {
    sessions: Mutex<HashMap<SocketAddr, Arc<UdpSession>>>,
    changed: Condvar,
}

impl UdpSessionRegistry {
    fn get(&self, client_addr: SocketAddr) -> Option<Arc<UdpSession>> {
        self.sessions
            .lock()
            .expect("UDP sessions lock poisoned")
            .get(&client_addr)
            .cloned()
    }

    fn has_capacity(&self, limit: usize) -> bool {
        self.sessions
            .lock()
            .expect("UDP sessions lock poisoned")
            .len()
            < limit
    }

    fn insert(&self, client_addr: SocketAddr, session: Arc<UdpSession>) {
        self.sessions
            .lock()
            .expect("UDP sessions lock poisoned")
            .insert(client_addr, session);
        self.changed.notify_all();
    }

    fn remove_if_current(&self, client_addr: SocketAddr, expected: &Arc<UdpSession>) -> bool {
        let removed = {
            let mut sessions = self.sessions.lock().expect("UDP sessions lock poisoned");
            let is_current = sessions
                .get(&client_addr)
                .is_some_and(|current| Arc::ptr_eq(current, expected));
            if is_current {
                sessions.remove(&client_addr)
            } else {
                None
            }
        };
        if let Some(session) = removed {
            session.cancellation.cancel();
            self.changed.notify_all();
            true
        } else {
            false
        }
    }

    fn remove_idle(&self, now: Instant, timeout: Duration) -> Vec<(SocketAddr, Arc<UdpSession>)> {
        let removed = {
            let mut sessions = self.sessions.lock().expect("UDP sessions lock poisoned");
            let expired = sessions
                .iter()
                .filter_map(|(client_addr, session)| {
                    session
                        .is_idle(now, timeout)
                        .then_some((*client_addr, Arc::clone(session)))
                })
                .collect::<Vec<_>>();
            for (client_addr, _) in &expired {
                sessions.remove(client_addr);
            }
            expired
        };
        if !removed.is_empty() {
            for (_, session) in &removed {
                session.cancellation.cancel();
            }
            self.changed.notify_all();
        }
        removed
    }

    fn cancel_all(&self) {
        let sessions = {
            let mut sessions = self.sessions.lock().expect("UDP sessions lock poisoned");
            sessions
                .drain()
                .map(|(_, session)| session)
                .collect::<Vec<_>>()
        };
        if !sessions.is_empty() {
            for session in sessions {
                session.cancellation.cancel();
            }
            self.changed.notify_all();
        }
    }

    #[cfg(test)]
    fn wait_for_count(&self, expected: usize, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        let mut sessions = self.sessions.lock().expect("UDP sessions lock poisoned");
        loop {
            if sessions.len() == expected {
                return Ok(());
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Err(format!(
                    "等待 UDP 会话数变为 {expected} 超时，当前为 {}",
                    sessions.len()
                ));
            };
            let (next_sessions, timeout_result) = self
                .changed
                .wait_timeout(sessions, remaining)
                .expect("UDP sessions lock poisoned");
            sessions = next_sessions;
            if timeout_result.timed_out() && sessions.len() != expected {
                return Err(format!(
                    "等待 UDP 会话数变为 {expected} 超时，当前为 {}",
                    sessions.len()
                ));
            }
        }
    }
}

#[derive(Default)]
struct UdpWorkerCompletion {
    failure: Mutex<Option<String>>,
}

impl UdpWorkerCompletion {
    fn record_failure(&self, error: String) {
        *self
            .failure
            .lock()
            .expect("UDP worker completion lock poisoned") = Some(error);
    }

    fn failure(&self) -> Option<String> {
        self.failure
            .lock()
            .expect("UDP worker completion lock poisoned")
            .clone()
    }
}

impl UdpRuleRunner {
    pub(crate) fn new() -> Self {
        Self::with_limits(UdpLimits::production())
    }

    pub(crate) fn with_limits(limits: UdpLimits) -> Self {
        limits.assert_valid();
        Self {
            next_handle: AtomicU64::new(1),
            running: Mutex::new(HashMap::new()),
            limits,
        }
    }

    pub(crate) fn observability(
        &self,
        handle: RunningHandle,
    ) -> Result<Arc<UdpObservability>, String> {
        self.running
            .lock()
            .expect("UDP runner lock poisoned")
            .get(&handle.0)
            .map(|rule| Arc::clone(&rule.observability))
            .ok_or_else(|| "UDP 转发规则运行句柄不存在".to_string())
    }

    #[cfg(test)]
    fn listener_addr(&self, handle: RunningHandle) -> Result<SocketAddr, String> {
        self.running
            .lock()
            .expect("UDP runner lock poisoned")
            .get(&handle.0)
            .map(|rule| rule.listener_addr)
            .ok_or_else(|| "UDP 转发规则运行句柄不存在".to_string())
    }

    #[cfg(test)]
    fn only_handle(&self) -> Result<RunningHandle, String> {
        let running = self.running.lock().expect("UDP runner lock poisoned");
        if running.len() != 1 {
            return Err("测试期望恰好一个运行中的 UDP 规则".into());
        }
        Ok(RunningHandle(
            *running.keys().next().expect("checked UDP rule count"),
        ))
    }

    #[cfg(test)]
    fn sessions_for_test(&self, handle: RunningHandle) -> Result<Arc<UdpSessionRegistry>, String> {
        self.running
            .lock()
            .expect("UDP runner lock poisoned")
            .get(&handle.0)
            .map(|rule| Arc::clone(&rule.sessions))
            .ok_or_else(|| "UDP 转发规则运行句柄不存在".to_string())
    }

    #[cfg(test)]
    fn wait_for_snapshot(
        &self,
        handle: RunningHandle,
        predicate: impl Fn(&UdpObservationSnapshot) -> bool,
    ) -> Result<UdpObservationSnapshot, String> {
        let observability = self
            .running
            .lock()
            .expect("UDP runner lock poisoned")
            .get(&handle.0)
            .map(|rule| Arc::clone(&rule.observability))
            .ok_or_else(|| "UDP 转发规则运行句柄不存在".to_string())?;
        observability
            .wait_for(Duration::from_secs(2), predicate)
            .ok_or_else(|| "等待 UDP 转发统计超时".to_string())
    }
}

impl RuleRunner for UdpRuleRunner {
    fn start(&self, rule: &ForwardRule) -> Result<RunningHandle, String> {
        let bind_ip = rule
            .bind_host
            .parse::<IpAddr>()
            .map_err(|_| "UDP 监听地址必须是 IP 字面量".to_string())?;
        let target_host = rule
            .target_host
            .clone()
            .filter(|host| !host.is_empty())
            .ok_or_else(|| "UDP 规则缺少目标主机".to_string())?;
        let target_port = rule
            .target_port
            .ok_or_else(|| "UDP 规则缺少目标端口".to_string())?;
        let target = format!("{target_host}:{target_port}");
        let target_addr_hint = target_host
            .parse::<IpAddr>()
            .ok()
            .map(|ip| SocketAddr::new(ip, target_port));

        let std_listener = StdUdpSocket::bind(SocketAddr::new(bind_ip, rule.listen_port))
            .map_err(|error| format!("UDP 监听绑定失败: {error}"))?;
        std_listener
            .set_nonblocking(true)
            .map_err(|error| format!("UDP 监听器无法设为非阻塞: {error}"))?;
        #[cfg(test)]
        let listener_addr = std_listener
            .local_addr()
            .map_err(|error| format!("无法读取 UDP 监听地址: {error}"))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("无法创建 UDP 转发运行时: {error}"))?;
        let cancellation = CancellationToken::new();
        let observability = Arc::new(UdpObservability::default());
        let sessions = Arc::new(UdpSessionRegistry::default());
        let completion = Arc::new(UdpWorkerCompletion::default());
        let worker_cancellation = cancellation.clone();
        let worker_observability = Arc::clone(&observability);
        let worker_sessions = Arc::clone(&sessions);
        let worker_completion = Arc::clone(&completion);
        let limits = self.limits;
        let worker = thread::Builder::new()
            .name(format!("request-forward-udp-{}", rule.id))
            .spawn(move || {
                let result = runtime.block_on(async move {
                    let listener = UdpSocket::from_std(std_listener)
                        .map_err(|error| format!("无法创建 UDP 异步监听器: {error}"))?;
                    run_listener(
                        Arc::new(listener),
                        target_host,
                        target_port,
                        target,
                        target_addr_hint,
                        worker_cancellation,
                        worker_observability,
                        worker_sessions,
                        limits,
                    )
                    .await
                });
                if let Err(error) = &result {
                    worker_completion.record_failure(error.clone());
                }
                result
            })
            .map_err(|error| format!("无法启动 UDP 转发线程: {error}"))?;

        let handle = RunningHandle(self.next_handle.fetch_add(1, Ordering::Relaxed));
        self.running
            .lock()
            .expect("UDP runner lock poisoned")
            .insert(
                handle.0,
                UdpRunningRule {
                    #[cfg(test)]
                    listener_addr,
                    cancellation,
                    observability,
                    #[cfg(test)]
                    sessions,
                    completion,
                    worker,
                },
            );
        Ok(handle)
    }

    fn stop(&self, handle: RunningHandle) -> Result<(), String> {
        let running = self
            .running
            .lock()
            .expect("UDP runner lock poisoned")
            .remove(&handle.0)
            .ok_or_else(|| "UDP 转发规则运行句柄不存在".to_string())?;

        running.cancellation.cancel();
        match running.worker.join() {
            Ok(result) => result,
            Err(_) => Err("UDP 转发线程异常退出".into()),
        }
    }

    fn take_failure(&self, handle: RunningHandle) -> Option<String> {
        let (failure, is_finished) = self
            .running
            .lock()
            .expect("UDP runner lock poisoned")
            .get(&handle.0)
            .map(|running| (running.completion.failure(), running.worker.is_finished()))?;
        if failure.is_none() && !is_finished {
            return None;
        }

        let running = self
            .running
            .lock()
            .expect("UDP runner lock poisoned")
            .remove(&handle.0)?;
        match running.worker.join() {
            Ok(Err(error)) => Some(failure.unwrap_or(error)),
            Ok(Ok(())) => Some(failure.unwrap_or_else(|| "UDP 转发线程意外退出".into())),
            Err(payload) => Some(failure.unwrap_or_else(|| worker_panic_error(payload))),
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_listener(
    listener: Arc<UdpSocket>,
    target_host: String,
    target_port: u16,
    target: String,
    target_addr_hint: Option<SocketAddr>,
    cancellation: CancellationToken,
    observability: Arc<UdpObservability>,
    sessions: Arc<UdpSessionRegistry>,
    limits: UdpLimits,
) -> Result<(), String> {
    let mut responses = JoinSet::new();
    let cleanup_cancellation = cancellation.clone();
    let cleanup_sessions = Arc::clone(&sessions);
    let cleanup_observability = Arc::clone(&observability);
    let cleanup_target = target.clone();
    let mut cleanup = Some(tokio::spawn(async move {
        cleanup_idle_sessions(
            cleanup_cancellation,
            cleanup_sessions,
            cleanup_observability,
            cleanup_target,
            limits,
        )
        .await;
    }));
    let mut buffer = [0_u8; 65_535];
    let mut rule_error = None;

    loop {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => break,
            cleanup_result = cleanup.as_mut().expect("UDP cleanup task missing"), if cleanup.is_some() => {
                cleanup = None;
                if !cancellation.is_cancelled() {
                    let error = match cleanup_result {
                        Ok(()) => "UDP 会话清理任务意外退出".to_string(),
                        Err(error) => format!("UDP 会话清理任务异常退出: {error}"),
                    };
                    observability.child_task_failed(&target, error.clone());
                    rule_error = Some(error);
                    cancellation.cancel();
                    break;
                }
            },
            received = listener.recv_from(&mut buffer) => match received {
                Ok((size, client_addr)) => {
                    observability.client_datagram(client_addr, &target, target_addr_hint);
                    observability.transferred(size as u64, 0);
                    let session = match sessions.get(client_addr) {
                        Some(session) => session,
                        None if !sessions.has_capacity(limits.max_sessions) => {
                            observability.overloaded(
                                client_addr,
                                &target,
                                target_addr_hint,
                                format!("UDP 会话数已达到上限 {}", limits.max_sessions),
                            );
                            continue;
                        }
                        None => match create_downstream_socket(&target_host, target_port).await {
                            Ok((downstream, target_addr)) => {
                                let session = Arc::new(UdpSession::new(downstream, target_addr));
                                sessions.insert(client_addr, Arc::clone(&session));
                                observability.session_created(client_addr, &target, target_addr);
                                let response_listener = Arc::clone(&listener);
                                let response_sessions = Arc::clone(&sessions);
                                let response_observability = Arc::clone(&observability);
                                let response_target = target.clone();
                                let response_session = Arc::clone(&session);
                                let response_cancellation = cancellation.clone();
                                responses.spawn(async move {
                                    forward_responses(
                                        client_addr,
                                        response_listener,
                                        response_sessions,
                                        response_observability,
                                        response_target,
                                        response_session,
                                        response_cancellation,
                                    )
                                    .await;
                                });
                                session
                            }
                            Err(error) => {
                                observability.downstream_connect_failed(
                                    client_addr,
                                    &target,
                                    target_addr_hint,
                                    error,
                                );
                                continue;
                            }
                        },
                    };
                    session.touch();
                    if let Err(error) = session.downstream.send(&buffer[..size]).await {
                        observability.downstream_send_failed(
                            client_addr,
                            &target,
                            session.target_addr,
                            format!("发送到下游 {} 失败: {error}", session.target_addr),
                        );
                    }
                }
                Err(error) => {
                    let error = format!("UDP 接收客户端数据失败: {error}");
                    observability.listener_failed(&target, error.clone());
                    rule_error = Some(error);
                    cancellation.cancel();
                    break;
                }
            },
            completed = responses.join_next(), if !responses.is_empty() => {
                if let Some(Err(error)) = completed {
                    let error = format!("UDP 响应任务异常退出: {error}");
                    observability.child_task_failed(&target, error.clone());
                    rule_error = Some(error);
                    cancellation.cancel();
                    break;
                }
            },
        }
    }

    drop(listener);
    cancellation.cancel();
    sessions.cancel_all();
    if let Some(cleanup) = cleanup {
        if let Err(error) = cleanup.await {
            let error = format!("UDP 会话清理任务异常退出: {error}");
            observability.child_task_failed(&target, error.clone());
            if rule_error.is_none() {
                rule_error = Some(error);
            }
        }
    }
    while let Some(completed) = responses.join_next().await {
        if let Err(error) = completed {
            let error = format!("UDP 响应任务异常退出: {error}");
            observability.child_task_failed(&target, error.clone());
            if rule_error.is_none() {
                rule_error = Some(error);
            }
        }
    }

    rule_error.map_or(Ok(()), Err)
}

async fn create_downstream_socket(
    target_host: &str,
    target_port: u16,
) -> Result<(UdpSocket, SocketAddr), String> {
    let target_addr = lookup_host((target_host, target_port))
        .await
        .map_err(|error| format!("解析下游 {target_host}:{target_port} 失败: {error}"))?
        .next()
        .ok_or_else(|| format!("下游 {target_host}:{target_port} 未解析到地址"))?;
    let bind_addr = if target_addr.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let downstream = UdpSocket::bind(bind_addr)
        .await
        .map_err(|error| format!("创建下游 UDP socket 失败: {error}"))?;
    downstream
        .connect(target_addr)
        .await
        .map_err(|error| format!("连接下游 {target_addr} 失败: {error}"))?;
    Ok((downstream, target_addr))
}

async fn cleanup_idle_sessions(
    cancellation: CancellationToken,
    sessions: Arc<UdpSessionRegistry>,
    observability: Arc<UdpObservability>,
    target: String,
    limits: UdpLimits,
) {
    let mut interval = tokio::time::interval(limits.cleanup_interval);
    loop {
        tokio::select! {
            _ = cancellation.cancelled() => break,
            _ = interval.tick() => {
                for (client_addr, session) in sessions.remove_idle(Instant::now(), limits.idle_timeout) {
                    observability.session_expired(client_addr, &target, session.target_addr);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn forward_responses(
    client_addr: SocketAddr,
    listener: Arc<UdpSocket>,
    sessions: Arc<UdpSessionRegistry>,
    observability: Arc<UdpObservability>,
    target: String,
    session: Arc<UdpSession>,
    rule_cancellation: CancellationToken,
) {
    let mut buffer = [0_u8; 65_535];
    loop {
        tokio::select! {
            _ = rule_cancellation.cancelled() => break,
            _ = session.cancellation.cancelled() => break,
            received = session.downstream.recv(&mut buffer) => match received {
                Ok(size) => {
                    session.touch();
                    match listener.send_to(&buffer[..size], client_addr).await {
                        Ok(sent) if sent == size => observability.transferred(0, size as u64),
                        Ok(sent) => {
                            observability.client_send_failed(
                                client_addr,
                                &target,
                                session.target_addr,
                                format!("向客户端 {client_addr} 发送 UDP 响应不完整: {sent}/{size}"),
                            );
                            sessions.remove_if_current(client_addr, &session);
                            break;
                        }
                        Err(error) => {
                            observability.client_send_failed(
                                client_addr,
                                &target,
                                session.target_addr,
                                format!("向客户端 {client_addr} 发送 UDP 响应失败: {error}"),
                            );
                            sessions.remove_if_current(client_addr, &session);
                            break;
                        }
                    }
                }
                Err(error) => {
                    observability.downstream_receive_failed(
                        client_addr,
                        &target,
                        session.target_addr,
                        format!("接收下游 {} 的 UDP 响应失败: {error}", session.target_addr),
                    );
                    sessions.remove_if_current(client_addr, &session);
                    break;
                }
            }
        }
    }
}

fn worker_panic_error(payload: Box<dyn Any + Send>) -> String {
    let detail = payload
        .downcast_ref::<&str>()
        .map(|value| (*value).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "非文本 panic payload".into());
    format!("UDP 转发线程 panic: {detail}")
}

#[cfg(test)]
mod tests {
    use std::net::{SocketAddr, UdpSocket};
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    use super::{UdpEventKind, UdpLimits, UdpRuleRunner};
    use crate::tools::request_forward::model::{ForwardProtocol, ForwardRule};
    use crate::tools::request_forward::runtime::{
        AutoStartPersistence, RuleRunner, RuntimeManager, RuntimeState,
    };

    const SOCKET_TIMEOUT: Duration = Duration::from_secs(2);

    #[derive(Default)]
    struct TestPersistence {
        values: Mutex<Vec<(i64, bool)>>,
    }

    impl AutoStartPersistence for TestPersistence {
        fn set_auto_start(&self, rule_id: i64, value: bool) -> Result<(), String> {
            self.values
                .lock()
                .expect("lock test persistence")
                .push((rule_id, value));
            Ok(())
        }
    }

    fn udp_rule(id: i64, target_addr: SocketAddr) -> ForwardRule {
        ForwardRule {
            id,
            name: format!("UDP 测试规则 {id}"),
            protocol: ForwardProtocol::Udp,
            bind_host: "127.0.0.1".into(),
            listen_port: 0,
            target_url: None,
            target_host: Some(target_addr.ip().to_string()),
            target_port: Some(target_addr.port()),
            capture_http_headers: false,
            capture_http_body: false,
            auto_start: false,
            created_at: "2026-07-14 00:00:00".into(),
            updated_at: "2026-07-14 00:00:00".into(),
        }
    }

    fn bind_client() -> UdpSocket {
        let socket = UdpSocket::bind("127.0.0.1:0").expect("bind UDP client");
        socket
            .set_read_timeout(Some(SOCKET_TIMEOUT))
            .expect("set UDP client read timeout");
        socket
    }

    fn receive(client: &UdpSocket) -> Vec<u8> {
        let mut buffer = [0_u8; 1024];
        let (size, _) = client.recv_from(&mut buffer).expect("receive UDP response");
        buffer[..size].to_vec()
    }

    fn spawn_scripted_echo(
        expected_datagrams: usize,
    ) -> (SocketAddr, mpsc::Receiver<Vec<SocketAddr>>, JoinHandle<()>) {
        let socket = UdpSocket::bind("127.0.0.1:0").expect("bind scripted UDP downstream");
        let address = socket.local_addr().expect("read downstream address");
        let (peers_tx, peers_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut peers = Vec::with_capacity(expected_datagrams);
            for _ in 0..expected_datagrams {
                let mut buffer = [0_u8; 1024];
                let (size, peer) = socket
                    .recv_from(&mut buffer)
                    .expect("receive forwarded UDP datagram");
                peers.push(peer);
                let mut response = b"reply:".to_vec();
                response.extend_from_slice(&buffer[..size]);
                socket
                    .send_to(&response, peer)
                    .expect("send scripted UDP response");
            }
            peers_tx.send(peers).expect("report downstream peers");
        });
        (address, peers_rx, worker)
    }

    #[test]
    fn udp_keeps_responses_isolated_between_two_clients() {
        let (downstream_addr, peers_rx, downstream) = spawn_scripted_echo(2);
        let runner = UdpRuleRunner::new();
        let handle = runner
            .start(&udp_rule(1, downstream_addr))
            .expect("start UDP rule");
        let listener_addr = runner.listener_addr(handle).expect("read listener address");
        let first = bind_client();
        let second = bind_client();

        first
            .send_to(b"first", listener_addr)
            .expect("send first client datagram");
        second
            .send_to(b"second", listener_addr)
            .expect("send second client datagram");

        assert_eq!(receive(&first), b"reply:first");
        assert_eq!(receive(&second), b"reply:second");
        let peers = peers_rx
            .recv_timeout(SOCKET_TIMEOUT)
            .expect("receive downstream peers");
        assert_eq!(peers.len(), 2);
        assert_ne!(peers[0], peers[1], "each client needs a downstream socket");
        downstream.join().expect("join scripted downstream");

        runner.stop(handle).expect("stop UDP rule");
    }

    #[test]
    fn udp_event_count_means_client_datagrams_received() {
        let downstream = UdpSocket::bind("127.0.0.1:0").expect("bind UDP downstream");
        let rule = udp_rule(2, downstream.local_addr().expect("read downstream address"));
        let runner = UdpRuleRunner::new();
        let handle = runner.start(&rule).expect("start UDP rule");
        let listener_addr = runner.listener_addr(handle).expect("read listener address");
        let client = bind_client();

        for datagram in [b"a".as_slice(), b"bb", b"ccc"] {
            client
                .send_to(datagram, listener_addr)
                .expect("send client datagram");
        }

        let snapshot = runner
            .wait_for_snapshot(handle, |snapshot| snapshot.event_count == 3)
            .expect("wait for UDP datagram counters");
        assert_eq!(snapshot.upload_bytes, 6);
        assert_eq!(snapshot.download_bytes, 0);
        assert_eq!(snapshot.error_count, 0);

        runner.stop(handle).expect("stop UDP rule");
    }

    #[test]
    fn udp_reclaims_idle_sessions() {
        let downstream = UdpSocket::bind("127.0.0.1:0").expect("bind UDP downstream");
        let rule = udp_rule(3, downstream.local_addr().expect("read downstream address"));
        let limits = UdpLimits::for_test(4, Duration::from_millis(30), Duration::from_millis(5));
        let runner = UdpRuleRunner::with_limits(limits);
        let handle = runner.start(&rule).expect("start UDP rule");
        let listener_addr = runner.listener_addr(handle).expect("read listener address");
        let sessions = runner
            .sessions_for_test(handle)
            .expect("read UDP session registry");
        let client = bind_client();

        client
            .send_to(b"idle", listener_addr)
            .expect("send idle client datagram");
        sessions
            .wait_for_count(1, SOCKET_TIMEOUT)
            .expect("wait for UDP session creation");
        sessions
            .wait_for_count(0, SOCKET_TIMEOUT)
            .expect("wait for idle UDP session cleanup");
        let snapshot = runner
            .wait_for_snapshot(handle, |snapshot| {
                snapshot
                    .events
                    .iter()
                    .any(|event| event.kind == UdpEventKind::SessionExpired)
            })
            .expect("observe idle UDP session cleanup");
        assert_eq!(snapshot.error_count, 0);

        runner.stop(handle).expect("stop UDP rule");
    }

    #[test]
    fn udp_drops_new_client_when_session_limit_is_reached() {
        let (downstream_addr, peers_rx, downstream) = spawn_scripted_echo(2);
        let limits = UdpLimits::for_test(1, Duration::from_secs(1), Duration::from_millis(10));
        let runner = UdpRuleRunner::with_limits(limits);
        let handle = runner
            .start(&udp_rule(4, downstream_addr))
            .expect("start UDP rule");
        let listener_addr = runner.listener_addr(handle).expect("read listener address");
        let first = bind_client();
        let second = bind_client();

        first
            .send_to(b"open", listener_addr)
            .expect("open first client session");
        runner
            .wait_for_snapshot(handle, |snapshot| snapshot.event_count == 1)
            .expect("wait for first client datagram");
        assert_eq!(receive(&first), b"reply:open");

        second
            .send_to(b"drop", listener_addr)
            .expect("send overloaded client datagram");
        let snapshot = runner
            .wait_for_snapshot(handle, |snapshot| {
                snapshot.event_count == 2 && snapshot.error_count == 1
            })
            .expect("wait for UDP overload event");
        let overloaded = snapshot
            .events
            .iter()
            .find(|event| event.kind == UdpEventKind::Overloaded)
            .expect("record UDP overload event");
        assert_eq!(
            overloaded.client_addr,
            Some(second.local_addr().expect("read second client"))
        );
        assert_eq!(overloaded.target_addr, Some(downstream_addr));

        first
            .send_to(b"alive", listener_addr)
            .expect("send existing client datagram");
        assert_eq!(receive(&first), b"reply:alive");
        let peers = peers_rx
            .recv_timeout(SOCKET_TIMEOUT)
            .expect("existing client reaches downstream after overload");
        assert_eq!(peers.len(), 2);
        assert!(peers.iter().all(|peer| *peer == peers[0]));
        downstream.join().expect("join scripted downstream");

        runner.stop(handle).expect("stop UDP rule");
    }

    #[test]
    fn stopping_udp_rule_closes_listener_and_sessions() {
        let downstream = UdpSocket::bind("127.0.0.1:0").expect("bind UDP downstream");
        let rule = udp_rule(5, downstream.local_addr().expect("read downstream address"));
        let runner = Arc::new(UdpRuleRunner::new());
        let manager = RuntimeManager::new(runner.clone());
        let persistence = TestPersistence::default();

        assert_eq!(
            manager
                .start(&rule, &persistence)
                .expect("start UDP rule")
                .state,
            RuntimeState::Running
        );
        let handle = runner.only_handle().expect("read UDP running handle");
        let listener_addr = runner.listener_addr(handle).expect("read listener address");
        let sessions = runner
            .sessions_for_test(handle)
            .expect("read UDP session registry");
        let client = bind_client();
        client
            .send_to(b"keep-open", listener_addr)
            .expect("send active client datagram");
        sessions
            .wait_for_count(1, SOCKET_TIMEOUT)
            .expect("wait for active UDP session");

        let stopped = manager.stop(&rule, &persistence).expect("stop UDP rule");
        assert_eq!(stopped.state, RuntimeState::Stopped);
        sessions
            .wait_for_count(0, SOCKET_TIMEOUT)
            .expect("wait for UDP session cancellation");
        assert!(
            UdpSocket::bind(listener_addr).is_ok(),
            "UDP listener must close"
        );
        assert_eq!(
            persistence
                .values
                .lock()
                .expect("lock persistence")
                .as_slice(),
            &[(5, true), (5, false)]
        );
    }
}

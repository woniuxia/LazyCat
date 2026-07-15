use std::any::Any;
use std::collections::HashMap;
use std::future::pending;
use std::io::{Error, ErrorKind};
use std::net::{IpAddr, SocketAddr, TcpListener as StdTcpListener};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
#[cfg(test)]
use std::time::Instant;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Notify, Semaphore};
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use super::model::ForwardRule;
#[cfg(test)]
pub(super) use super::observability::TcpEventKind;
#[cfg(test)]
use super::observability::TcpObservationSnapshot;
use super::observability::{TcpConnectionObservation, TcpObservability};
use super::runtime::{RuleRunner, RunningHandle};

pub(crate) const TCP_MAX_CONNECTIONS_PER_RULE: usize = 64;
const TCP_DOWNSTREAM_CONNECT_TIMEOUT: Duration = Duration::from_secs(1);

pub(crate) struct TcpRuleRunner {
    next_handle: AtomicU64,
    running: Mutex<HashMap<u64, TcpRunningRule>>,
    connection_limit: usize,
    worker_failure: Option<Arc<Notify>>,
    worker_abrupt_exit: Option<WorkerAbruptExit>,
}

#[derive(Clone)]
struct WorkerAbruptExit {
    notify: Arc<Notify>,
    triggered: Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(test)]
#[derive(Clone)]
pub(crate) struct TcpWorkerFailureTrigger {
    notify: Arc<Notify>,
}

#[cfg(test)]
impl TcpWorkerFailureTrigger {
    fn trigger(&self) {
        self.notify.notify_one();
    }
}

#[cfg(test)]
#[derive(Clone)]
pub(crate) struct TcpWorkerAbruptExitTrigger {
    notify: Arc<Notify>,
    triggered: Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(test)]
impl TcpWorkerAbruptExitTrigger {
    fn trigger(&self) {
        self.triggered.store(true, Ordering::SeqCst);
        self.notify.notify_one();
    }
}

struct TcpRunningRule {
    #[cfg(test)]
    listener_addr: SocketAddr,
    cancellation: CancellationToken,
    observability: Arc<TcpObservability>,
    completion: Arc<WorkerCompletion>,
    worker: JoinHandle<Result<(), String>>,
}

#[derive(Default)]
struct WorkerCompletion {
    failure: Mutex<Option<String>>,
    changed: Condvar,
}

impl WorkerCompletion {
    fn record_failure(&self, error: String) {
        *self
            .failure
            .lock()
            .expect("TCP worker completion lock poisoned") = Some(error);
        self.changed.notify_all();
    }

    fn failure(&self) -> Option<String> {
        self.failure
            .lock()
            .expect("TCP worker completion lock poisoned")
            .clone()
    }

    #[cfg(test)]
    fn wait_for_failure(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut failure = self
            .failure
            .lock()
            .expect("TCP worker completion lock poisoned");
        while failure.is_none() {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            let (next_failure, timeout_result) = self
                .changed
                .wait_timeout(failure, remaining)
                .expect("TCP worker completion lock poisoned");
            failure = next_failure;
            if timeout_result.timed_out() {
                return failure.is_some();
            }
        }
        true
    }
}

impl TcpRuleRunner {
    pub(crate) fn new() -> Self {
        Self::with_connection_limit(TCP_MAX_CONNECTIONS_PER_RULE)
    }

    pub(crate) fn with_connection_limit(connection_limit: usize) -> Self {
        assert!(
            connection_limit > 0,
            "TCP connection limit must be positive"
        );
        Self {
            next_handle: AtomicU64::new(1),
            running: Mutex::new(HashMap::new()),
            connection_limit,
            worker_failure: None,
            worker_abrupt_exit: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_worker_failure_for_test() -> (Self, TcpWorkerFailureTrigger) {
        let notify = Arc::new(Notify::new());
        let mut runner = Self::new();
        runner.worker_failure = Some(Arc::clone(&notify));
        (runner, TcpWorkerFailureTrigger { notify })
    }

    #[cfg(test)]
    pub(crate) fn with_worker_abrupt_exit_for_test() -> (Self, TcpWorkerAbruptExitTrigger) {
        let notify = Arc::new(Notify::new());
        let triggered = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut runner = Self::new();
        runner.worker_abrupt_exit = Some(WorkerAbruptExit {
            notify: Arc::clone(&notify),
            triggered: Arc::clone(&triggered),
        });
        (runner, TcpWorkerAbruptExitTrigger { notify, triggered })
    }

    #[cfg(test)]
    pub(crate) fn listener_addr(&self, handle: RunningHandle) -> Result<SocketAddr, String> {
        self.running
            .lock()
            .expect("TCP runner lock poisoned")
            .get(&handle.0)
            .map(|rule| rule.listener_addr)
            .ok_or_else(|| "TCP 转发规则运行句柄不存在".to_string())
    }

    #[cfg(test)]
    pub(crate) fn observation_snapshot(
        &self,
        handle: RunningHandle,
    ) -> Result<TcpObservationSnapshot, String> {
        self.running
            .lock()
            .expect("TCP runner lock poisoned")
            .get(&handle.0)
            .map(|rule| rule.observability.snapshot())
            .ok_or_else(|| "TCP 转发规则运行句柄不存在".to_string())
    }

    pub(crate) fn observability(
        &self,
        handle: RunningHandle,
    ) -> Result<Arc<TcpObservability>, String> {
        self.running
            .lock()
            .expect("TCP runner lock poisoned")
            .get(&handle.0)
            .map(|rule| Arc::clone(&rule.observability))
            .ok_or_else(|| "TCP 转发规则运行句柄不存在".to_string())
    }

    #[cfg(test)]
    pub(crate) fn only_listener_addr(&self) -> Result<SocketAddr, String> {
        let running = self.running.lock().expect("TCP runner lock poisoned");
        if running.len() != 1 {
            return Err("测试期望恰好一个运行中的 TCP 规则".into());
        }
        Ok(running
            .values()
            .next()
            .expect("checked TCP rule count")
            .listener_addr)
    }

    #[cfg(test)]
    pub(crate) fn only_handle(&self) -> Result<RunningHandle, String> {
        let running = self.running.lock().expect("TCP runner lock poisoned");
        if running.len() != 1 {
            return Err("测试期望恰好一个运行中的 TCP 规则".into());
        }
        Ok(RunningHandle(
            *running.keys().next().expect("checked TCP rule count"),
        ))
    }

    #[cfg(test)]
    pub(crate) fn observability_for_test(
        &self,
        handle: RunningHandle,
    ) -> Result<Arc<TcpObservability>, String> {
        self.observability(handle)
    }

    #[cfg(test)]
    pub(crate) fn wait_for_worker_failure(&self, handle: RunningHandle) -> Result<(), String> {
        let completion = self
            .running
            .lock()
            .expect("TCP runner lock poisoned")
            .get(&handle.0)
            .map(|rule| Arc::clone(&rule.completion))
            .ok_or_else(|| "TCP 转发规则运行句柄不存在".to_string())?;
        if completion.wait_for_failure(Duration::from_secs(2)) {
            Ok(())
        } else {
            Err("等待 TCP worker 清理超时".into())
        }
    }

    #[cfg(test)]
    pub(crate) fn wait_for_worker_exit(&self, handle: RunningHandle) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let finished = self
                .running
                .lock()
                .expect("TCP runner lock poisoned")
                .get(&handle.0)
                .is_some_and(|rule| rule.worker.is_finished());
            if finished {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("等待 TCP worker 退出超时".into());
            }
            thread::yield_now();
        }
    }

    #[cfg(test)]
    pub(crate) fn wait_for_snapshot(
        &self,
        handle: RunningHandle,
        predicate: impl Fn(&TcpObservationSnapshot) -> bool,
    ) -> Result<TcpObservationSnapshot, String> {
        let observability = self
            .running
            .lock()
            .expect("TCP runner lock poisoned")
            .get(&handle.0)
            .map(|rule| Arc::clone(&rule.observability))
            .ok_or_else(|| "TCP 转发规则运行句柄不存在".to_string())?;
        observability
            .wait_for(Duration::from_secs(2), predicate)
            .ok_or_else(|| "等待 TCP 转发统计超时".to_string())
    }
}

impl RuleRunner for TcpRuleRunner {
    fn start(&self, rule: &ForwardRule) -> Result<RunningHandle, String> {
        let bind_ip = rule
            .bind_host
            .parse::<IpAddr>()
            .map_err(|_| "TCP 监听地址必须是 IP 字面量".to_string())?;
        let target_host = rule
            .target_host
            .clone()
            .filter(|host| !host.is_empty())
            .ok_or_else(|| "TCP 规则缺少目标主机".to_string())?;
        let target_port = rule
            .target_port
            .ok_or_else(|| "TCP 规则缺少目标端口".to_string())?;

        let std_listener = StdTcpListener::bind(SocketAddr::new(bind_ip, rule.listen_port))
            .map_err(|error| format!("TCP 监听绑定失败: {error}"))?;
        std_listener
            .set_nonblocking(true)
            .map_err(|error| format!("TCP 监听器无法设为非阻塞: {error}"))?;
        #[cfg(test)]
        let listener_addr = std_listener
            .local_addr()
            .map_err(|error| format!("无法读取 TCP 监听地址: {error}"))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("无法创建 TCP 转发运行时: {error}"))?;
        let cancellation = CancellationToken::new();
        let observability = Arc::new(TcpObservability::default());
        let completion = Arc::new(WorkerCompletion::default());
        let worker_cancellation = cancellation.clone();
        let worker_observability = Arc::clone(&observability);
        let worker_completion = Arc::clone(&completion);
        let connection_limit = self.connection_limit;
        let worker_failure = self.worker_failure.clone();
        let worker_abrupt_exit = self.worker_abrupt_exit.clone();
        let worker_abrupt_exit_for_listener = worker_abrupt_exit.clone();
        let worker = thread::Builder::new()
            .name(format!("request-forward-tcp-{}", rule.id))
            .spawn(move || {
                let result = runtime.block_on(async move {
                    let listener = TcpListener::from_std(std_listener)
                        .map_err(|error| format!("无法创建 TCP 异步监听器: {error}"))?;
                    run_listener(
                        listener,
                        target_host,
                        target_port,
                        worker_cancellation,
                        worker_observability,
                        connection_limit,
                        worker_failure,
                        worker_abrupt_exit_for_listener.map(|exit| exit.notify),
                    )
                    .await
                });
                if worker_abrupt_exit
                    .as_ref()
                    .is_some_and(|exit| exit.triggered.load(Ordering::SeqCst))
                {
                    return Ok(());
                }
                if let Err(error) = &result {
                    worker_completion.record_failure(error.clone());
                }
                result
            })
            .map_err(|error| format!("无法启动 TCP 转发线程: {error}"))?;

        let handle = RunningHandle(self.next_handle.fetch_add(1, Ordering::Relaxed));
        self.running
            .lock()
            .expect("TCP runner lock poisoned")
            .insert(
                handle.0,
                TcpRunningRule {
                    #[cfg(test)]
                    listener_addr,
                    cancellation,
                    observability,
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
            .expect("TCP runner lock poisoned")
            .remove(&handle.0)
            .ok_or_else(|| "TCP 转发规则运行句柄不存在".to_string())?;

        running.cancellation.cancel();
        match running.worker.join() {
            Ok(result) => result,
            Err(_) => Err("TCP 转发线程异常退出".into()),
        }
    }

    fn take_failure(&self, handle: RunningHandle) -> Option<String> {
        let (failure, is_finished) = self
            .running
            .lock()
            .expect("TCP runner lock poisoned")
            .get(&handle.0)
            .map(|running| (running.completion.failure(), running.worker.is_finished()))?;
        if failure.is_none() && !is_finished {
            return None;
        }

        let running = self
            .running
            .lock()
            .expect("TCP runner lock poisoned")
            .remove(&handle.0)?;
        match running.worker.join() {
            Ok(Err(error)) => Some(failure.unwrap_or(error)),
            Ok(Ok(())) => Some(failure.unwrap_or_else(|| "TCP 转发线程意外退出".into())),
            Err(payload) => Some(failure.unwrap_or_else(|| worker_panic_error(payload))),
        }
    }
}

async fn run_listener(
    listener: TcpListener,
    target_host: String,
    target_port: u16,
    cancellation: CancellationToken,
    observability: Arc<TcpObservability>,
    connection_limit: usize,
    worker_failure: Option<Arc<Notify>>,
    worker_abrupt_exit: Option<Arc<Notify>>,
) -> Result<(), String> {
    let semaphore = Arc::new(Semaphore::new(connection_limit));
    let mut children = JoinSet::new();
    let mut rule_error = None;

    loop {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => break,
            _ = wait_for_worker_failure(worker_failure.clone()) => {
                let error = "injected TCP worker failure".to_string();
                observability.listener_failed(error.clone());
                rule_error = Some(error);
                cancellation.cancel();
                break;
            },
            _ = wait_for_worker_failure(worker_abrupt_exit.clone()) => {
                cancellation.cancel();
                break;
            },
            accepted = listener.accept() => match accepted {
                Ok((client, client_addr)) => {
                    let connection = observability.accepted_connection(
                        Some(client_addr),
                        Some(format!("{target_host}:{target_port}")),
                    );
                    match semaphore.clone().try_acquire_owned() {
                        Ok(permit) => {
                            let child_cancellation = cancellation.clone();
                            let child_target_host = target_host.clone();
                            children.spawn(async move {
                                let _permit = permit;
                                forward_connection(
                                    client,
                                    child_target_host,
                                    target_port,
                                    child_cancellation,
                                    connection,
                                ).await;
                            });
                        }
                        Err(_) => {
                            connection.overloaded(format!(
                                "TCP 并发连接已达到上限 {connection_limit}"
                            ));
                            drop(client);
                        }
                    }
                }
                Err(error) => {
                    let error = format!("TCP 接受连接失败: {error}");
                    observability.listener_failed(error.clone());
                    rule_error = Some(error);
                    cancellation.cancel();
                    break;
                }
            },
            completed = children.join_next(), if !children.is_empty() => {
                if let Some(Err(error)) = completed {
                    let error = format!("TCP 转发子任务异常退出: {error}");
                    observability.child_task_failed(error.clone());
                    rule_error = Some(error);
                    cancellation.cancel();
                    break;
                }
            },
        }
    }

    drop(listener);
    cancellation.cancel();
    while let Some(completed) = children.join_next().await {
        if let Err(error) = completed {
            let error = format!("TCP 转发子任务异常退出: {error}");
            observability.child_task_failed(error.clone());
            if rule_error.is_none() {
                rule_error = Some(error);
            }
        }
    }

    rule_error.map_or(Ok(()), Err)
}

async fn forward_connection(
    client: TcpStream,
    target_host: String,
    target_port: u16,
    cancellation: CancellationToken,
    connection: Arc<TcpConnectionObservation>,
) {
    let downstream = tokio::select! {
        _ = cancellation.cancelled() => return,
        connected = timeout(
            TCP_DOWNSTREAM_CONNECT_TIMEOUT,
            TcpStream::connect((target_host.as_str(), target_port)),
        ) => match connected {
            Ok(Ok(stream)) => stream,
            Ok(Err(error)) => {
                connection.downstream_connect_failed(format!(
                    "连接下游 {target_host}:{target_port} 失败: {error}"
                ));
                return;
            }
            Err(_) => {
                connection.downstream_connect_failed(format!(
                    "连接下游 {target_host}:{target_port} 超时（{} ms）",
                    TCP_DOWNSTREAM_CONNECT_TIMEOUT.as_millis()
                ));
                return;
            }
        },
    };

    let (client_reader, client_writer) = client.into_split();
    let (downstream_reader, downstream_writer) = downstream.into_split();
    let upload_observation = Arc::clone(&connection);
    let transferred = tokio::select! {
        _ = cancellation.cancelled() => return,
        transferred = relay_bidirectionally(
            client_reader,
            downstream_writer,
            upload_observation,
            downstream_reader,
            client_writer,
            Arc::clone(&connection),
        ) => transferred,
    };
    match transferred {
        Ok(()) => connection.completed(),
        Err(error) => connection.relay_failed(format!("TCP 双向转发失败: {error}")),
    }
}

async fn relay_bidirectionally(
    client_reader: tokio::net::tcp::OwnedReadHalf,
    downstream_writer: tokio::net::tcp::OwnedWriteHalf,
    observation_upload: Arc<TcpConnectionObservation>,
    downstream_reader: tokio::net::tcp::OwnedReadHalf,
    client_writer: tokio::net::tcp::OwnedWriteHalf,
    observation_download: Arc<TcpConnectionObservation>,
) -> Result<(), Error> {
    tokio::try_join!(
        copy_direction(client_reader, downstream_writer, observation_upload, true),
        copy_direction(
            downstream_reader,
            client_writer,
            observation_download,
            false
        ),
    )?;
    Ok(())
}

async fn copy_direction<R, W>(
    mut reader: R,
    mut writer: W,
    observation: Arc<TcpConnectionObservation>,
    upload: bool,
) -> Result<(), Error>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            writer.shutdown().await?;
            return Ok(());
        }

        let mut offset = 0;
        while offset < read {
            let written = writer.write(&buffer[offset..read]).await?;
            if written == 0 {
                return Err(Error::new(ErrorKind::WriteZero, "TCP 写入返回零字节"));
            }
            offset += written;
            if upload {
                observation.transferred(written as u64, 0);
            } else {
                observation.transferred(0, written as u64);
            }
        }
    }
}

async fn wait_for_worker_failure(failure: Option<Arc<Notify>>) {
    match failure {
        Some(notify) => notify.notified().await,
        None => pending().await,
    }
}

fn worker_panic_error(payload: Box<dyn Any + Send>) -> String {
    let detail = payload
        .downcast_ref::<&str>()
        .map(|value| (*value).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "非文本 panic payload".into());
    format!("TCP 转发线程 panic: {detail}")
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    use super::{TcpEventKind, TcpRuleRunner};
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

    fn tcp_rule(id: i64, target_addr: SocketAddr) -> ForwardRule {
        ForwardRule {
            id,
            name: format!("TCP 测试规则 {id}"),
            protocol: ForwardProtocol::Tcp,
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

    fn accept_once(
        handler: impl FnOnce(TcpStream) + Send + 'static,
    ) -> (SocketAddr, JoinHandle<()>) {
        accept_once_on(
            "127.0.0.1:0".parse().expect("parse loopback address"),
            handler,
        )
    }

    fn accept_once_on(
        address: SocketAddr,
        handler: impl FnOnce(TcpStream) + Send + 'static,
    ) -> (SocketAddr, JoinHandle<()>) {
        let listener = TcpListener::bind(address).expect("bind downstream listener");
        let address = listener.local_addr().expect("read downstream address");
        let worker = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept downstream connection");
            handler(stream);
        });
        (address, worker)
    }

    fn connect(address: SocketAddr) -> TcpStream {
        let stream = TcpStream::connect_timeout(&address, SOCKET_TIMEOUT)
            .expect("connect to forwarding listener");
        stream
            .set_read_timeout(Some(SOCKET_TIMEOUT))
            .expect("set client read timeout");
        stream
            .set_write_timeout(Some(SOCKET_TIMEOUT))
            .expect("set client write timeout");
        stream
    }

    #[test]
    fn tcp_forwards_both_directions_and_preserves_half_close() {
        let (downstream_addr, downstream) = accept_once(|mut stream| {
            let mut request = Vec::new();
            stream
                .read_to_end(&mut request)
                .expect("read request after client half-close");
            assert_eq!(request, b"request");
            stream.write_all(b"response").expect("write response");
            stream
                .shutdown(Shutdown::Write)
                .expect("half-close response");
        });
        let rule = tcp_rule(1, downstream_addr);
        let runner = TcpRuleRunner::new();
        let handle = runner.start(&rule).expect("start TCP rule");
        let listener_addr = runner.listener_addr(handle).expect("read listener address");

        let mut client = connect(listener_addr);
        client.write_all(b"request").expect("write request");
        client
            .shutdown(Shutdown::Write)
            .expect("half-close client write");
        let mut response = Vec::new();
        client
            .read_to_end(&mut response)
            .expect("read response after client half-close");
        assert_eq!(response, b"response");
        downstream.join().expect("join downstream server");

        let snapshot = runner
            .wait_for_snapshot(handle, |snapshot| {
                snapshot.upload_bytes == 7
                    && snapshot.download_bytes == 8
                    && snapshot.events.len() == 1
            })
            .expect("wait for byte counters");
        assert_eq!(snapshot.event_count, 1);
        assert_eq!(snapshot.error_count, 0);
        assert_eq!(snapshot.events.len(), 1);
        assert_eq!(snapshot.events[0].kind, TcpEventKind::Accepted);
        assert_eq!(snapshot.events[0].upload_bytes, 7);
        assert_eq!(snapshot.events[0].download_bytes, 8);
        assert_eq!(snapshot.events[0].error, None);

        runner.stop(handle).expect("stop TCP rule");
    }

    #[test]
    fn tcp_downstream_failure_only_closes_current_client() {
        let unavailable = TcpListener::bind("127.0.0.1:0").expect("reserve downstream port");
        let downstream_addr = unavailable
            .local_addr()
            .expect("read reserved downstream address");
        drop(unavailable);
        assert!(
            TcpStream::connect_timeout(&downstream_addr, SOCKET_TIMEOUT).is_err(),
            "released downstream port must refuse direct connections"
        );

        let rule = tcp_rule(2, downstream_addr);
        let runner = TcpRuleRunner::new();
        let handle = runner.start(&rule).expect("start TCP rule");
        let listener_addr = runner.listener_addr(handle).expect("read listener address");

        let mut failed_client = connect(listener_addr);
        failed_client
            .write_all(b"trigger")
            .expect("write failed downstream request");
        let failure_snapshot = runner
            .wait_for_snapshot(handle, |snapshot| {
                snapshot.error_count == 1 && snapshot.events.len() == 1
            })
            .expect("wait for downstream failure event");
        assert_eq!(failure_snapshot.event_count, 1);
        let failure_event = failure_snapshot
            .events
            .iter()
            .find(|event| event.kind == TcpEventKind::DownstreamConnectFailed)
            .expect("record downstream failure event");
        assert!(failure_event
            .error
            .as_deref()
            .is_some_and(|error| error.contains("127.0.0.1")));
        assert_eq!(failure_event.upload_bytes, 0);
        assert_eq!(failure_event.download_bytes, 0);
        let mut closed = Vec::new();
        assert_client_closed(&mut failed_client, &mut closed);

        let (ready_tx, ready_rx) = mpsc::channel();
        let (downstream_addr, downstream) = accept_once_on(downstream_addr, move |mut stream| {
            ready_tx.send(()).expect("signal downstream readiness");
            let mut request = Vec::new();
            stream
                .read_to_end(&mut request)
                .expect("read forwarded request");
            stream.write_all(&request).expect("echo forwarded request");
            stream
                .shutdown(Shutdown::Write)
                .expect("close echoed response");
        });
        assert_eq!(downstream_addr, rule_target_addr(&rule));

        let mut client = connect(listener_addr);
        client.write_all(b"retry").expect("write retry request");
        client
            .shutdown(Shutdown::Write)
            .expect("finish retry request");
        ready_rx
            .recv_timeout(SOCKET_TIMEOUT)
            .expect("downstream accepts retry client");
        let mut response = Vec::new();
        client
            .read_to_end(&mut response)
            .expect("read retry response");
        assert_eq!(response, b"retry");
        downstream.join().expect("join downstream server");

        let snapshot = runner
            .wait_for_snapshot(handle, |snapshot| {
                snapshot.event_count == 2
                    && snapshot.upload_bytes == 5
                    && snapshot.download_bytes == 5
                    && snapshot.error_count == 1
            })
            .expect("wait for retry counters");
        assert!(snapshot
            .events
            .iter()
            .any(|event| event.kind == TcpEventKind::DownstreamConnectFailed));

        runner.stop(handle).expect("stop TCP rule");
    }

    #[test]
    fn tcp_overload_closes_new_connection_and_keeps_existing_connections() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (downstream_addr, downstream) = accept_once(move |mut stream| {
            entered_tx
                .send(())
                .expect("signal first downstream connection");
            let mut request = Vec::new();
            stream
                .read_to_end(&mut request)
                .expect("read first request");
            assert_eq!(request, b"alive");
            release_rx.recv().expect("release downstream response");
            stream.write_all(b"ok").expect("write first response");
            stream
                .shutdown(Shutdown::Write)
                .expect("close first response");
        });
        let rule = tcp_rule(3, downstream_addr);
        let runner = TcpRuleRunner::with_connection_limit(1);
        let handle = runner.start(&rule).expect("start TCP rule");
        let listener_addr = runner.listener_addr(handle).expect("read listener address");

        let mut first = connect(listener_addr);
        first.write_all(b"alive").expect("write first request");
        entered_rx
            .recv_timeout(SOCKET_TIMEOUT)
            .expect("downstream accepts first client");

        let mut overloaded = connect(listener_addr);
        let mut closed = Vec::new();
        overloaded
            .read_to_end(&mut closed)
            .expect("overloaded client closes");
        assert!(closed.is_empty());

        first
            .shutdown(Shutdown::Write)
            .expect("finish first request");
        release_tx.send(()).expect("release first response");
        let mut response = Vec::new();
        first
            .read_to_end(&mut response)
            .expect("read existing client response");
        assert_eq!(response, b"ok");
        downstream.join().expect("join downstream server");

        let snapshot = runner
            .wait_for_snapshot(handle, |snapshot| {
                snapshot.event_count == 2
                    && snapshot.upload_bytes == 5
                    && snapshot.download_bytes == 2
                    && snapshot.error_count == 1
            })
            .expect("wait for overload counters");
        assert!(snapshot
            .events
            .iter()
            .any(|event| event.kind == TcpEventKind::Overloaded));

        runner.stop(handle).expect("stop TCP rule");
    }

    #[test]
    fn stopping_tcp_rule_closes_listener_and_existing_connections() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (downstream_addr, downstream) = accept_once(move |mut stream| {
            entered_tx.send(()).expect("signal downstream connection");
            let mut bytes = Vec::new();
            stream
                .read_to_end(&mut bytes)
                .expect("read cancellation close");
        });
        let rule = tcp_rule(4, downstream_addr);
        let runner = Arc::new(TcpRuleRunner::new());
        let manager = RuntimeManager::new(runner.clone());
        let persistence = TestPersistence::default();

        assert_eq!(
            manager
                .start(&rule, &persistence)
                .expect("start TCP rule")
                .state,
            RuntimeState::Running
        );
        let listener_addr = runner
            .only_listener_addr()
            .expect("read TCP listener address");
        let mut client = connect(listener_addr);
        client
            .write_all(b"keep-open")
            .expect("write active request");
        entered_rx
            .recv_timeout(SOCKET_TIMEOUT)
            .expect("downstream accepts active client");

        let stopped = manager.stop(&rule, &persistence).expect("stop TCP rule");
        assert_eq!(stopped.state, RuntimeState::Stopped);
        assert_eq!(
            persistence
                .values
                .lock()
                .expect("lock persistence")
                .as_slice(),
            &[(4, true), (4, false)]
        );

        let mut closed = Vec::new();
        match client.read_to_end(&mut closed) {
            Ok(_) => assert!(closed.is_empty()),
            Err(error) => assert!(matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::NotConnected
            )),
        }
        assert!(TcpStream::connect_timeout(&listener_addr, SOCKET_TIMEOUT).is_err());
        downstream.join().expect("join downstream server");
    }

    #[test]
    fn tcp_worker_failure_marks_rule_failed_and_retires_handle() {
        let downstream = TcpListener::bind("127.0.0.1:0").expect("bind downstream listener");
        let downstream_addr = downstream.local_addr().expect("read downstream address");
        let rule = tcp_rule(5, downstream_addr);
        let (runner, failure) = TcpRuleRunner::with_worker_failure_for_test();
        let runner = Arc::new(runner);
        let manager = RuntimeManager::new(runner.clone());
        let persistence = TestPersistence::default();

        assert_eq!(
            manager
                .start(&rule, &persistence)
                .expect("start TCP rule")
                .state,
            RuntimeState::Running
        );
        let handle = runner.only_handle().expect("read running TCP handle");
        let listener_addr = runner.listener_addr(handle).expect("read listener address");
        failure.trigger();
        runner
            .wait_for_snapshot(handle, |snapshot| {
                snapshot.events.iter().any(|event| {
                    event.kind == TcpEventKind::ListenerFailed
                        && event.error.as_deref() == Some("injected TCP worker failure")
                })
            })
            .expect("wait for injected worker failure");
        runner
            .wait_for_worker_failure(handle)
            .expect("wait for worker cleanup");
        let snapshot = runner
            .observation_snapshot(handle)
            .expect("read production TCP observation");
        assert!(snapshot.events.iter().any(|event| {
            event.kind == TcpEventKind::ListenerFailed
                && event.error.as_deref() == Some("injected TCP worker failure")
        }));

        let status = manager.status(rule.id);
        assert_eq!(status.state, RuntimeState::Failed);
        assert_eq!(
            status.last_error.as_deref(),
            Some("injected TCP worker failure")
        );
        assert!(manager.ensure_rule_mutable(rule.id).is_ok());
        assert!(TcpStream::connect_timeout(&listener_addr, SOCKET_TIMEOUT).is_err());

        assert_eq!(
            manager
                .start(&rule, &persistence)
                .expect("restart failed TCP rule")
                .state,
            RuntimeState::Running
        );
        manager
            .stop(&rule, &persistence)
            .expect("stop restarted TCP rule");
        drop(downstream);
    }

    #[test]
    fn tcp_worker_abrupt_exit_marks_rule_failed_and_allows_restart() {
        let downstream = TcpListener::bind("127.0.0.1:0").expect("bind downstream listener");
        let downstream_addr = downstream.local_addr().expect("read downstream address");
        let rule = tcp_rule(7, downstream_addr);
        let (runner, abrupt_exit) = TcpRuleRunner::with_worker_abrupt_exit_for_test();
        let runner = Arc::new(runner);
        let manager = RuntimeManager::new(runner.clone());
        let persistence = TestPersistence::default();

        manager.start(&rule, &persistence).expect("start TCP rule");
        let handle = runner.only_handle().expect("read running TCP handle");
        abrupt_exit.trigger();
        runner
            .wait_for_worker_exit(handle)
            .expect("wait for abrupt worker exit");

        let status = manager.status(rule.id);
        assert_eq!(status.state, RuntimeState::Failed);
        assert!(status
            .last_error
            .as_deref()
            .is_some_and(|error| !error.is_empty()));
        assert!(manager.ensure_rule_mutable(rule.id).is_ok());

        assert_eq!(
            manager
                .start(&rule, &persistence)
                .expect("restart abruptly exited TCP rule")
                .state,
            RuntimeState::Running
        );
        manager
            .stop(&rule, &persistence)
            .expect("stop restarted TCP rule");
        drop(downstream);
    }

    #[test]
    fn tcp_counts_partial_upload_before_cancellation() {
        let (received_tx, received_rx) = mpsc::channel();
        let (downstream_addr, downstream) = accept_once(move |mut stream| {
            let mut request = [0_u8; 7];
            stream
                .read_exact(&mut request)
                .expect("read partial forwarded request");
            assert_eq!(&request, b"partial");
            received_tx
                .send(())
                .expect("signal forwarded partial request");
            let mut rest = Vec::new();
            stream
                .read_to_end(&mut rest)
                .expect("read cancellation close");
        });
        let rule = tcp_rule(6, downstream_addr);
        let runner = TcpRuleRunner::new();
        let handle = runner.start(&rule).expect("start TCP rule");
        let listener_addr = runner.listener_addr(handle).expect("read listener address");
        let observability = runner
            .observability_for_test(handle)
            .expect("get observability");

        let mut client = connect(listener_addr);
        client.write_all(b"partial").expect("write partial request");
        received_rx
            .recv_timeout(SOCKET_TIMEOUT)
            .expect("downstream receives partial request");
        runner
            .wait_for_snapshot(handle, |snapshot| snapshot.upload_bytes == 7)
            .expect("record partial upload before cancellation");

        runner.stop(handle).expect("cancel TCP rule");
        let snapshot = observability.snapshot();
        assert_eq!(snapshot.upload_bytes, 7);
        assert_eq!(snapshot.download_bytes, 0);
        assert_client_closed(&mut client, &mut Vec::new());
        downstream.join().expect("join downstream server");
    }

    fn rule_target_addr(rule: &ForwardRule) -> SocketAddr {
        SocketAddr::new(
            rule.target_host
                .as_deref()
                .expect("target host")
                .parse()
                .expect("parse target IP"),
            rule.target_port.expect("target port"),
        )
    }

    fn assert_client_closed(client: &mut TcpStream, bytes: &mut Vec<u8>) {
        match client.read_to_end(bytes) {
            Ok(_) => assert!(bytes.is_empty()),
            Err(error) => assert!(matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::NotConnected
            )),
        }
    }
}

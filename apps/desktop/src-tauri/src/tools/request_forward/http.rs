#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fmt;
    use std::net::IpAddr;

    use hyper::http::{
        header::{
            CONNECTION, FORWARDED, HOST, PROXY_AUTHENTICATE, PROXY_AUTHORIZATION, TE, TRAILER,
            TRANSFER_ENCODING, UPGRADE,
        },
        HeaderMap, HeaderValue, Uri,
    };
    use url::Url;

    use super::{
        build_target_uri, format_error_chain, rebuild_forward_headers, replace_host_header,
        strip_hop_by_hop,
    };

    #[derive(Debug)]
    struct ChainedTestError {
        message: &'static str,
        source: Option<Box<ChainedTestError>>,
    }

    impl fmt::Display for ChainedTestError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(self.message)
        }
    }

    impl Error for ChainedTestError {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            self.source.as_deref().map(|source| source as &dyn Error)
        }
    }

    #[test]
    fn formats_error_source_chain_without_duplicate_messages() {
        let error = ChainedTestError {
            message: "client error (Connect)",
            source: Some(Box::new(ChainedTestError {
                message: "client error (Connect)",
                source: Some(Box::new(ChainedTestError {
                    message: "invalid peer certificate: certificate not valid for name",
                    source: None,
                })),
            })),
        };

        assert_eq!(
            format_error_chain(&error),
            "client error (Connect): invalid peer certificate: certificate not valid for name"
        );
    }

    #[test]
    fn joins_base_path_and_inbound_path_and_query() {
        let base = Url::parse("https://example.com/api").unwrap();
        let inbound = Uri::from_static("/users?x=1");

        let target = build_target_uri(&base, &inbound).unwrap();

        assert_eq!(target.to_string(), "https://example.com/api/users?x=1");
    }

    #[test]
    fn supports_http_and_ipv6_base_urls_without_reinterpreting_inbound_authority() {
        let base = Url::parse("http://[::1]:8080/api").unwrap();
        let inbound = Uri::from_static("/v1/items?tag=a");
        assert_eq!(
            build_target_uri(&base, &inbound).unwrap().to_string(),
            "http://[::1]:8080/api/v1/items?tag=a"
        );

        let absolute_inbound = Uri::from_static("http://untrusted.example/items");
        assert!(build_target_uri(&base, &absolute_inbound).is_err());
    }

    #[test]
    fn rejects_non_http_base_and_base_query_or_fragment() {
        let inbound = Uri::from_static("/users");

        assert!(build_target_uri(&Url::parse("ftp://example.com/api").unwrap(), &inbound).is_err());
        assert!(build_target_uri(
            &Url::parse("https://example.com/api?x=1").unwrap(),
            &inbound
        )
        .is_err());
        assert!(build_target_uri(
            &Url::parse("https://example.com/api#part").unwrap(),
            &inbound
        )
        .is_err());
    }

    #[test]
    fn strips_static_and_connection_nominated_hop_by_hop_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONNECTION,
            HeaderValue::from_static("keep-alive, x-remove-me"),
        );
        headers.insert("keep-alive", HeaderValue::from_static("timeout=5"));
        headers.insert(PROXY_AUTHENTICATE, HeaderValue::from_static("Basic"));
        headers.insert(
            PROXY_AUTHORIZATION,
            HeaderValue::from_static("Basic secret"),
        );
        headers.insert(TE, HeaderValue::from_static("trailers"));
        headers.insert(TRAILER, HeaderValue::from_static("x-checksum"));
        headers.insert(TRANSFER_ENCODING, HeaderValue::from_static("chunked"));
        headers.insert(UPGRADE, HeaderValue::from_static("websocket"));
        headers.insert("x-remove-me", HeaderValue::from_static("yes"));
        headers.insert("x-keep-me", HeaderValue::from_static("yes"));

        strip_hop_by_hop(&mut headers);

        for name in [
            "connection",
            "keep-alive",
            "proxy-authenticate",
            "proxy-authorization",
            "te",
            "trailer",
            "transfer-encoding",
            "upgrade",
            "x-remove-me",
        ] {
            assert!(headers.get(name).is_none(), "{name} must be removed");
        }
        assert_eq!(headers["x-keep-me"], "yes");
    }

    #[test]
    fn replaces_client_forwarding_chain_and_replaces_downstream_host_separately() {
        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_static("public.example"));
        headers.insert(FORWARDED, HeaderValue::from_static("for=spoofed"));
        headers.insert("x-forwarded-for", HeaderValue::from_static("spoofed"));
        headers.insert("x-forwarded-host", HeaderValue::from_static("spoofed"));
        headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        let original_host = headers.get(HOST).cloned();

        rebuild_forward_headers(
            &mut headers,
            "203.0.113.7".parse::<IpAddr>().unwrap(),
            original_host.as_ref(),
        )
        .unwrap();

        assert_eq!(
            headers[FORWARDED],
            "for=203.0.113.7;host=public.example;proto=http"
        );
        assert_eq!(headers["x-forwarded-for"], "203.0.113.7");
        assert_eq!(headers["x-forwarded-host"], "public.example");
        assert_eq!(headers["x-forwarded-proto"], "http");
        assert_eq!(headers[HOST], "public.example");

        replace_host_header(
            &mut headers,
            &Uri::from_static("https://target.example/api"),
        )
        .unwrap();
        assert_eq!(headers[HOST], "target.example");
    }
}

use std::net::IpAddr;
use std::str::FromStr;

use hyper::http::{HeaderMap, HeaderName, HeaderValue, Uri};
use url::Url;

const HOP_BY_HOP_HEADERS: [&str; 8] = [
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

const FORWARDING_HEADERS: [&str; 4] = [
    "forwarded",
    "x-forwarded-for",
    "x-forwarded-host",
    "x-forwarded-proto",
];

pub(crate) fn build_target_uri(base: &Url, inbound: &Uri) -> Result<Uri, String> {
    if !matches!(base.scheme(), "http" | "https") {
        return Err("HTTP 目标 URL 仅支持 http 或 https".into());
    }
    if base.host_str().is_none() {
        return Err("HTTP 目标 URL 必须包含主机名".into());
    }
    if base.query().is_some() || base.fragment().is_some() {
        return Err("HTTP 目标 URL 不能包含 query 或 fragment".into());
    }
    if inbound.scheme().is_some() || inbound.authority().is_some() {
        return Err("入站请求必须使用 path 和 query".into());
    }

    let inbound_path_and_query = inbound
        .path_and_query()
        .ok_or_else(|| "入站请求缺少 path".to_string())?;
    let inbound_path = inbound_path_and_query.path();
    if !inbound_path.starts_with('/') {
        return Err("入站请求 path 必须以 / 开头".into());
    }

    let target_path = join_target_path(base.path(), inbound_path);
    let target_path_and_query = match inbound_path_and_query.query() {
        Some(query) => format!("{target_path}?{query}"),
        None => target_path,
    };

    let mut target = base
        .as_str()
        .parse::<Uri>()
        .map_err(|_| "HTTP 目标 URL 格式不正确".to_string())?
        .into_parts();
    target.path_and_query = Some(
        target_path_and_query
            .parse()
            .map_err(|_| "入站请求 path 或 query 格式不正确".to_string())?,
    );
    Uri::from_parts(target).map_err(|_| "无法构造 HTTP 目标 URL".to_string())
}

pub(crate) fn strip_hop_by_hop(headers: &mut HeaderMap) {
    let nominated_headers = headers
        .get_all("connection")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(|name| HeaderName::from_str(name.trim()).ok())
        .collect::<Vec<_>>();

    for name in HOP_BY_HOP_HEADERS {
        headers.remove(name);
    }
    for name in nominated_headers {
        headers.remove(name);
    }
}

pub(crate) fn rebuild_forward_headers(
    headers: &mut HeaderMap,
    client_ip: IpAddr,
    original_host: Option<&HeaderValue>,
) -> Result<(), String> {
    let original_host = original_host
        .map(|value| {
            value
                .to_str()
                .map(|value| (value, value.to_owned()))
                .map_err(|_| "原始 Host 包含非文本值".to_string())
        })
        .transpose()?;

    for name in FORWARDING_HEADERS {
        headers.remove(name);
    }

    let mut forwarded = format!("for={}", format_forwarded_client_ip(client_ip));
    if let Some((host, _)) = original_host.as_ref() {
        forwarded.push_str(";host=");
        forwarded.push_str(&format_forwarded_parameter(host));
    }
    forwarded.push_str(";proto=http");

    headers.insert(
        "forwarded",
        HeaderValue::from_str(&forwarded).map_err(|_| "无法构造 Forwarded 请求头".to_string())?,
    );
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_str(&client_ip.to_string())
            .map_err(|_| "无法构造 X-Forwarded-For 请求头".to_string())?,
    );
    if let Some((_, host)) = original_host {
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_str(&host)
                .map_err(|_| "无法构造 X-Forwarded-Host 请求头".to_string())?,
        );
    }
    headers.insert("x-forwarded-proto", HeaderValue::from_static("http"));
    Ok(())
}

pub(crate) fn replace_host_header(headers: &mut HeaderMap, target: &Uri) -> Result<(), String> {
    let authority = target
        .authority()
        .ok_or_else(|| "HTTP 目标 URL 必须包含 authority".to_string())?;
    headers.insert(
        "host",
        HeaderValue::from_str(authority.as_str())
            .map_err(|_| "无法构造下游 Host 请求头".to_string())?,
    );
    Ok(())
}

fn join_target_path(base_path: &str, inbound_path: &str) -> String {
    if base_path.is_empty() || base_path == "/" {
        inbound_path.to_owned()
    } else {
        format!("{base_path}{inbound_path}")
    }
}

fn format_forwarded_client_ip(client_ip: IpAddr) -> String {
    match client_ip {
        IpAddr::V4(value) => value.to_string(),
        IpAddr::V6(value) => format!("\"[{value}]\""),
    }
}

fn format_forwarded_parameter(value: &str) -> String {
    if value.bytes().all(is_token_byte) {
        return value.to_owned();
    }

    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

use std::any::Any;
use std::collections::HashMap;
use std::convert::Infallible;
use std::error::Error;
use std::future::Future;
use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::{Body, Frame, Incoming};
use hyper::http::header::{CONNECTION, CONTENT_LENGTH, UPGRADE};
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_rustls::{ConfigBuilderExt, HttpsConnector, HttpsConnectorBuilder};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::{TokioExecutor, TokioIo};
use rustls::{ClientConfig, RootCertStore};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpListener;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_util::sync::{CancellationToken, WaitForCancellationFutureOwned};
use tokio_util::task::TaskTracker;

use super::model::ForwardRule;
#[cfg(test)]
use super::observability::HttpObservationSnapshot;
use super::observability::{HttpObservability, HttpRequestTrace};
use super::runtime::{RuleRunner, RunningHandle};

pub(crate) const HTTP_MAX_CONNECTIONS_PER_RULE: usize = 64;
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
const HTTP_PRE_RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

type ProxyError = Box<dyn Error + Send + Sync>;
type ProxyBody = BoxBody<Bytes, ProxyError>;
type DownstreamConnector = HttpsConnector<HttpConnector>;
type HttpClient = Client<DownstreamConnector, ProxyBody>;
type TlsConfigLoader = Arc<dyn Fn() -> Result<ClientConfig, String> + Send + Sync>;

fn load_native_tls_config() -> Result<ClientConfig, String> {
    Ok(ClientConfig::builder()
        .with_native_roots()
        .map_err(|error| format!("无法加载系统 TLS 根证书: {error}"))?
        .with_no_client_auth())
}

pub(crate) struct HttpRuleRunner {
    next_handle: AtomicU64,
    running: Mutex<HashMap<u64, HttpRunningRule>>,
    connection_limit: usize,
    pre_response_timeout: Duration,
    tls_config_loader: TlsConfigLoader,
}

struct HttpRunningRule {
    #[cfg(test)]
    listener_addr: SocketAddr,
    cancellation: CancellationToken,
    observability: Arc<HttpObservability>,
    completion: Arc<HttpWorkerCompletion>,
    worker: JoinHandle<Result<(), String>>,
}

#[derive(Default)]
struct HttpWorkerCompletion {
    failure: Mutex<Option<String>>,
}

impl HttpWorkerCompletion {
    fn record_failure(&self, error: String) {
        *self
            .failure
            .lock()
            .expect("HTTP worker completion lock poisoned") = Some(error);
    }

    fn failure(&self) -> Option<String> {
        self.failure
            .lock()
            .expect("HTTP worker completion lock poisoned")
            .clone()
    }
}

impl HttpRuleRunner {
    pub(crate) fn new() -> Self {
        Self::with_options(HTTP_MAX_CONNECTIONS_PER_RULE, HTTP_PRE_RESPONSE_TIMEOUT)
    }

    #[cfg(test)]
    pub(crate) fn with_connection_limit(connection_limit: usize) -> Self {
        Self::with_options(connection_limit, HTTP_PRE_RESPONSE_TIMEOUT)
    }

    fn with_options(connection_limit: usize, pre_response_timeout: Duration) -> Self {
        Self::with_options_and_tls_loader(
            connection_limit,
            pre_response_timeout,
            Arc::new(load_native_tls_config),
        )
    }

    fn with_options_and_tls_loader(
        connection_limit: usize,
        pre_response_timeout: Duration,
        tls_config_loader: TlsConfigLoader,
    ) -> Self {
        assert!(
            connection_limit > 0,
            "HTTP connection limit must be positive"
        );
        Self {
            next_handle: AtomicU64::new(1),
            running: Mutex::new(HashMap::new()),
            connection_limit,
            pre_response_timeout,
            tls_config_loader,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_tls_config_for_test(tls_config: Arc<ClientConfig>) -> Self {
        Self::with_options_and_tls_loader(
            HTTP_MAX_CONNECTIONS_PER_RULE,
            HTTP_PRE_RESPONSE_TIMEOUT,
            Arc::new(move || Ok((*tls_config).clone())),
        )
    }

    #[cfg(test)]
    pub(crate) fn with_tls_config_loader_for_test(
        loader: impl Fn() -> Result<ClientConfig, String> + Send + Sync + 'static,
    ) -> Self {
        Self::with_options_and_tls_loader(
            HTTP_MAX_CONNECTIONS_PER_RULE,
            HTTP_PRE_RESPONSE_TIMEOUT,
            Arc::new(loader),
        )
    }

    #[cfg(test)]
    pub(crate) fn with_response_timeout_for_test(pre_response_timeout: Duration) -> Self {
        Self::with_options(HTTP_MAX_CONNECTIONS_PER_RULE, pre_response_timeout)
    }

    #[cfg(test)]
    pub(crate) fn listener_addr(&self, handle: RunningHandle) -> Result<SocketAddr, String> {
        self.running
            .lock()
            .expect("HTTP runner lock poisoned")
            .get(&handle.0)
            .map(|rule| rule.listener_addr)
            .ok_or_else(|| "HTTP 转发规则运行句柄不存在".to_string())
    }

    #[cfg(test)]
    pub(crate) fn observation_snapshot(
        &self,
        handle: RunningHandle,
    ) -> Result<HttpObservationSnapshot, String> {
        self.running
            .lock()
            .expect("HTTP runner lock poisoned")
            .get(&handle.0)
            .map(|rule| rule.observability.snapshot())
            .ok_or_else(|| "HTTP 转发规则运行句柄不存在".to_string())
    }

    pub(crate) fn observability(
        &self,
        handle: RunningHandle,
    ) -> Result<Arc<HttpObservability>, String> {
        self.running
            .lock()
            .expect("HTTP runner lock poisoned")
            .get(&handle.0)
            .map(|rule| Arc::clone(&rule.observability))
            .ok_or_else(|| "HTTP 转发规则运行句柄不存在".to_string())
    }

    #[cfg(test)]
    pub(crate) fn wait_for_snapshot(
        &self,
        handle: RunningHandle,
        predicate: impl Fn(&HttpObservationSnapshot) -> bool,
    ) -> Result<HttpObservationSnapshot, String> {
        let observability = self
            .running
            .lock()
            .expect("HTTP runner lock poisoned")
            .get(&handle.0)
            .map(|rule| Arc::clone(&rule.observability))
            .ok_or_else(|| "HTTP 转发规则运行句柄不存在".to_string())?;
        observability
            .wait_for(Duration::from_secs(2), predicate)
            .ok_or_else(|| "等待 HTTP 转发统计超时".to_string())
    }

    fn connector(&self, target_scheme: &str) -> Result<DownstreamConnector, String> {
        let mut http = HttpConnector::new();
        // Windows 对不可达地址的异步 connect 可能长时间保持 pending；连接阶段必须有独立上限。
        http.set_connect_timeout(Some(HTTP_CONNECT_TIMEOUT));
        // hyper-rustls 的 wrap_connector 不会调整自定义 HttpConnector 的 scheme 限制。
        http.enforce_http(false);
        let tls_config = match target_scheme {
            "http" => ClientConfig::builder()
                .with_root_certificates(RootCertStore::empty())
                .with_no_client_auth(),
            "https" => (self.tls_config_loader)()?,
            _ => return Err("HTTP 目标 URL 仅支持 http 或 https".into()),
        };
        Ok(HttpsConnectorBuilder::new()
            .with_tls_config(tls_config)
            .https_or_http()
            .enable_http1()
            .wrap_connector(http))
    }
}

impl RuleRunner for HttpRuleRunner {
    fn start(&self, rule: &ForwardRule) -> Result<RunningHandle, String> {
        let bind_ip = rule
            .bind_host
            .parse::<IpAddr>()
            .map_err(|_| "HTTP 监听地址必须是 IP 字面量".to_string())?;
        let target_url = rule
            .target_url
            .as_deref()
            .ok_or_else(|| "HTTP 规则缺少目标 URL".to_string())?
            .parse::<Url>()
            .map_err(|_| "HTTP 目标 URL 格式不正确".to_string())?;
        let connector = self.connector(target_url.scheme())?;
        let std_listener = StdTcpListener::bind(SocketAddr::new(bind_ip, rule.listen_port))
            .map_err(|error| format!("HTTP 监听绑定失败: {error}"))?;
        std_listener
            .set_nonblocking(true)
            .map_err(|error| format!("HTTP 监听器无法设为非阻塞: {error}"))?;
        #[cfg(test)]
        let listener_addr = std_listener
            .local_addr()
            .map_err(|error| format!("无法读取 HTTP 监听地址: {error}"))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("无法创建 HTTP 转发运行时: {error}"))?;
        let cancellation = CancellationToken::new();
        let observability = Arc::new(HttpObservability::default());
        let completion = Arc::new(HttpWorkerCompletion::default());
        let worker_cancellation = cancellation.clone();
        let worker_observability = Arc::clone(&observability);
        let worker_completion = Arc::clone(&completion);
        let connection_limit = self.connection_limit;
        let pre_response_timeout = self.pre_response_timeout;
        let capture_http_headers = rule.capture_http_headers;
        let capture_http_body = rule.capture_http_body;
        let worker = thread::Builder::new()
            .name(format!("request-forward-http-{}", rule.id))
            .spawn(move || {
                let result = runtime.block_on(async move {
                    let listener = TcpListener::from_std(std_listener)
                        .map_err(|error| format!("无法创建 HTTP 异步监听器: {error}"))?;
                    run_listener(
                        listener,
                        target_url,
                        connector,
                        worker_cancellation,
                        worker_observability,
                        connection_limit,
                        pre_response_timeout,
                        capture_http_headers,
                        capture_http_body,
                    )
                    .await
                });
                if let Err(error) = &result {
                    worker_completion.record_failure(error.clone());
                }
                result
            })
            .map_err(|error| format!("无法启动 HTTP 转发线程: {error}"))?;

        let handle = RunningHandle(self.next_handle.fetch_add(1, Ordering::Relaxed));
        self.running
            .lock()
            .expect("HTTP runner lock poisoned")
            .insert(
                handle.0,
                HttpRunningRule {
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
            .expect("HTTP runner lock poisoned")
            .remove(&handle.0)
            .ok_or_else(|| "HTTP 转发规则运行句柄不存在".to_string())?;
        running.cancellation.cancel();
        match running.worker.join() {
            Ok(result) => result,
            Err(payload) => Err(worker_panic_error(payload)),
        }
    }

    fn take_failure(&self, handle: RunningHandle) -> Option<String> {
        let (failure, is_finished) = self
            .running
            .lock()
            .expect("HTTP runner lock poisoned")
            .get(&handle.0)
            .map(|running| (running.completion.failure(), running.worker.is_finished()))?;
        if failure.is_none() && !is_finished {
            return None;
        }
        let running = self
            .running
            .lock()
            .expect("HTTP runner lock poisoned")
            .remove(&handle.0)?;
        match running.worker.join() {
            Ok(Err(error)) => Some(failure.unwrap_or(error)),
            Ok(Ok(())) => Some(failure.unwrap_or_else(|| "HTTP 转发线程意外退出".into())),
            Err(payload) => Some(failure.unwrap_or_else(|| worker_panic_error(payload))),
        }
    }
}

async fn run_listener(
    listener: TcpListener,
    target_url: Url,
    connector: DownstreamConnector,
    cancellation: CancellationToken,
    observability: Arc<HttpObservability>,
    connection_limit: usize,
    pre_response_timeout: Duration,
    capture_http_headers: bool,
    capture_http_body: bool,
) -> Result<(), String> {
    let client = Client::builder(TokioExecutor::new()).build(connector);
    let semaphore = Arc::new(Semaphore::new(connection_limit));
    let upgrade_tasks = TaskTracker::new();
    let mut children = JoinSet::new();

    loop {
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, client_addr)) => {
                    let child_client = client.clone();
                    let child_target_url = target_url.clone();
                    let child_cancellation = cancellation.clone();
                    let child_observability = Arc::clone(&observability);
                    let child_semaphore = Arc::clone(&semaphore);
                    let child_upgrade_tasks = upgrade_tasks.clone();
                    children.spawn(async move {
                        let service_cancellation = child_cancellation.clone();
                        let service_observability = Arc::clone(&child_observability);
                        let service = service_fn(move |request| {
                            forward_request(
                                request,
                                client_addr,
                                child_target_url.clone(),
                                child_client.clone(),
                                service_cancellation.clone(),
                                Arc::clone(&service_observability),
                                Arc::clone(&child_semaphore),
                                child_upgrade_tasks.clone(),
                                pre_response_timeout,
                                capture_http_headers,
                                capture_http_body,
                            )
                        });
                        let serve = hyper::server::conn::http1::Builder::new()
                            .serve_connection(TokioIo::new(stream), service)
                            .with_upgrades();
                        tokio::select! {
                            _ = child_cancellation.cancelled() => {}
                            result = serve => {
                                if let Err(error) = result {
                                    if !is_observed_response_stream_error(&error) {
                                        child_observability.child_task_failed(format!("HTTP 客户端连接失败: {error}"));
                                    }
                                }
                            }
                        }
                    });
                }
                Err(error) => {
                    let error = format!("HTTP 接受连接失败: {error}");
                    observability.listener_failed(error.clone());
                    cancellation.cancel();
                    while children.join_next().await.is_some() {}
                    upgrade_tasks.close();
                    upgrade_tasks.wait().await;
                    return Err(error);
                }
            },
            completed = children.join_next(), if !children.is_empty() => {
                if completed.is_some_and(|result| result.is_err()) {
                    let error = "HTTP 转发子任务 panic".to_string();
                    observability.child_task_failed(error.clone());
                    cancellation.cancel();
                    while children.join_next().await.is_some() {}
                    upgrade_tasks.close();
                    upgrade_tasks.wait().await;
                    return Err(error);
                }
            },
        }
    }

    drop(listener);
    cancellation.cancel();
    while children.join_next().await.is_some() {}
    upgrade_tasks.close();
    upgrade_tasks.wait().await;
    Ok(())
}

async fn forward_request(
    mut request: Request<Incoming>,
    client_addr: SocketAddr,
    target_url: Url,
    client: HttpClient,
    cancellation: CancellationToken,
    observability: Arc<HttpObservability>,
    semaphore: Arc<Semaphore>,
    upgrade_tasks: TaskTracker,
    pre_response_timeout: Duration,
    capture_http_headers: bool,
    capture_http_body: bool,
) -> Result<Response<ProxyBody>, Infallible> {
    let original_host = request.headers().get("host").cloned();
    let target_addr = format!(
        "{}:{}",
        target_url.host_str().unwrap_or_default(),
        target_url.port_or_known_default().unwrap_or(80)
    );
    let method = request.method().to_string();
    let path = request
        .uri()
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/")
        .to_string();
    let trace = observability.accepted(
        client_addr,
        target_addr,
        method,
        path,
        request.headers(),
        capture_http_headers,
        capture_http_body,
    );

    let websocket_upgrade = is_websocket_upgrade(request.headers());
    if is_upgrade_request(request.headers()) && !websocket_upgrade {
        let error = "HTTP 转发仅支持标准 WebSocket Upgrade 请求".to_string();
        trace.upgrade_rejected(error.clone());
        return Ok(text_response(StatusCode::BAD_REQUEST, &error));
    }

    let permit = match semaphore.try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            let error = "HTTP 并发请求已达到上限".to_string();
            trace.overloaded(error.clone());
            return Ok(text_response(StatusCode::SERVICE_UNAVAILABLE, &error));
        }
    };

    let inbound_upgrade = websocket_upgrade.then(|| hyper::upgrade::on(&mut request));

    let (mut parts, body) = request.into_parts();
    let target_uri = match build_target_uri(&target_url, &parts.uri) {
        Ok(uri) => uri,
        Err(error) => {
            trace.downstream_failed(error.clone());
            return Ok(text_response(StatusCode::BAD_REQUEST, &error));
        }
    };
    let upgrade_header = websocket_upgrade
        .then(|| parts.headers.get(UPGRADE).cloned())
        .flatten();
    strip_hop_by_hop(&mut parts.headers);
    if let Some(upgrade_header) = upgrade_header {
        restore_upgrade_headers(&mut parts.headers, upgrade_header);
    }
    if let Err(error) =
        rebuild_forward_headers(&mut parts.headers, client_addr.ip(), original_host.as_ref())
    {
        trace.downstream_failed(error.clone());
        return Ok(text_response(StatusCode::BAD_REQUEST, &error));
    }
    if let Err(error) = replace_host_header(&mut parts.headers, &target_uri) {
        trace.downstream_failed(error.clone());
        return Ok(text_response(StatusCode::BAD_REQUEST, &error));
    }
    parts.uri = target_uri;
    let outbound = Request::from_parts(parts, observe_request_body(body, Arc::clone(&trace)));
    let mut response = tokio::select! {
        _ = cancellation.cancelled() => return Ok(text_response(StatusCode::SERVICE_UNAVAILABLE, "HTTP 转发已停止")),
        response = timeout(pre_response_timeout, client.request(outbound)) => match response {
            Ok(Ok(response)) => response,
            Ok(Err(error)) => {
                let error = format!("连接下游 HTTP 服务失败: {}", format_error_chain(&error));
                trace.downstream_failed(error.clone());
                return Ok(text_response(StatusCode::BAD_GATEWAY, &error));
            }
            Err(_) => {
                let error = format!("等待下游 HTTP 响应头超时（{} ms）", pre_response_timeout.as_millis());
                trace.response_timeout(error.clone());
                return Ok(text_response(StatusCode::GATEWAY_TIMEOUT, &error));
            }
        },
    };

    if websocket_upgrade && response.status() == StatusCode::SWITCHING_PROTOCOLS {
        if !is_websocket_upgrade(response.headers()) {
            let error = "下游返回了无效的 WebSocket Upgrade 响应".to_string();
            trace.upgrade_failed(error.clone());
            return Ok(text_response(StatusCode::BAD_GATEWAY, &error));
        }

        let downstream_upgrade = hyper::upgrade::on(&mut response);
        let (mut parts, _body) = response.into_parts();
        let upgrade_header = parts
            .headers
            .get(UPGRADE)
            .cloned()
            .expect("validated WebSocket response has Upgrade header");
        strip_hop_by_hop(&mut parts.headers);
        restore_upgrade_headers(&mut parts.headers, upgrade_header);
        trace.response_started(parts.status.as_u16(), &parts.headers);
        upgrade_tasks.spawn(relay_websocket_upgrade(
            inbound_upgrade.expect("validated WebSocket request has upgrade future"),
            downstream_upgrade,
            cancellation,
            Arc::clone(&trace),
            permit,
        ));
        return Ok(Response::from_parts(parts, empty_body()));
    }

    let (mut parts, body) = response.into_parts();
    strip_hop_by_hop(&mut parts.headers);
    trace.response_started(parts.status.as_u16(), &parts.headers);
    Ok(Response::from_parts(
        parts,
        observe_response_body(body, trace, cancellation),
    ))
}

async fn relay_websocket_upgrade(
    inbound_upgrade: hyper::upgrade::OnUpgrade,
    downstream_upgrade: hyper::upgrade::OnUpgrade,
    cancellation: CancellationToken,
    trace: Arc<HttpRequestTrace>,
    _permit: OwnedSemaphorePermit,
) {
    let upgraded = tokio::select! {
        _ = cancellation.cancelled() => {
            trace.response_completed();
            return;
        }
        result = async { tokio::try_join!(inbound_upgrade, downstream_upgrade) } => result,
    };
    let (inbound, downstream) = match upgraded {
        Ok(upgraded) => upgraded,
        Err(error) => {
            trace.upgrade_failed(format!("建立 WebSocket 隧道失败: {error}"));
            return;
        }
    };

    let mut inbound = CountingIo::new(
        TokioIo::new(inbound),
        Arc::clone(&trace),
        TransferDirection::Upload,
    );
    let mut downstream = CountingIo::new(
        TokioIo::new(downstream),
        Arc::clone(&trace),
        TransferDirection::Download,
    );
    let relay = tokio::io::copy_bidirectional(&mut inbound, &mut downstream);
    tokio::select! {
        _ = cancellation.cancelled() => trace.response_completed(),
        result = relay => match result {
            Ok(_) => trace.response_completed(),
            Err(error) => trace.upgrade_failed(format!("WebSocket 双向转发失败: {error}")),
        }
    }
}

#[derive(Clone, Copy)]
enum TransferDirection {
    Upload,
    Download,
}

struct CountingIo<T> {
    inner: T,
    trace: Arc<HttpRequestTrace>,
    direction: TransferDirection,
}

impl<T> CountingIo<T> {
    fn new(inner: T, trace: Arc<HttpRequestTrace>, direction: TransferDirection) -> Self {
        Self {
            inner,
            trace,
            direction,
        }
    }
}

impl<T: AsyncRead + Unpin> AsyncRead for CountingIo<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buffer.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(cx, buffer);
        if matches!(result, Poll::Ready(Ok(()))) {
            let bytes = buffer.filled().len().saturating_sub(before);
            match self.direction {
                TransferDirection::Upload => self.trace.uploaded(bytes),
                TransferDirection::Download => self.trace.downloaded(bytes),
            }
        }
        result
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for CountingIo<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buffer)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

fn format_error_chain(error: &(dyn Error + 'static)) -> String {
    let mut messages = Vec::new();
    let mut current = Some(error);
    for _ in 0..16 {
        let Some(cause) = current else {
            break;
        };
        let message = cause.to_string();
        if !messages.iter().any(|existing| existing == &message) {
            messages.push(message);
        }
        current = cause.source();
    }
    messages.join(": ")
}

fn observe_request_body(body: Incoming, trace: Arc<HttpRequestTrace>) -> ProxyBody {
    body.map_frame(move |frame| {
        if let Some(bytes) = frame.data_ref() {
            trace.uploaded(bytes.len());
            trace.observe_request(bytes);
        }
        frame
    })
    .map_err(|error| -> ProxyError { Box::new(error) })
    .boxed()
}

fn observe_response_body(
    body: Incoming,
    trace: Arc<HttpRequestTrace>,
    cancellation: CancellationToken,
) -> ProxyBody {
    let success_trace = Arc::clone(&trace);
    let body = body
        .map_frame(move |frame| {
            if let Some(bytes) = frame.data_ref() {
                success_trace.downloaded(bytes.len());
                success_trace.observe_response(bytes);
            }
            frame
        })
        .map_err(|error| -> ProxyError { Box::new(ObservedResponseStreamError::new(error)) });
    let body = CompletionBody::new(body, trace).boxed();
    CancellableBody::new(body, cancellation).boxed()
}

struct CompletionBody<B> {
    inner: Pin<Box<B>>,
    trace: Arc<HttpRequestTrace>,
    completed: bool,
}

impl<B> CompletionBody<B> {
    fn new(inner: B, trace: Arc<HttpRequestTrace>) -> Self {
        Self {
            inner: Box::pin(inner),
            trace,
            completed: false,
        }
    }

    fn complete_successfully(&mut self) {
        if !self.completed {
            self.completed = true;
            self.trace.response_completed();
        }
    }

    fn fail(&mut self, error: String) {
        if !self.completed {
            self.completed = true;
            self.trace.response_stream_failed(error);
        }
    }
}

impl<B> Drop for CompletionBody<B> {
    fn drop(&mut self) {
        self.complete_successfully();
    }
}

impl<B> Body for CompletionBody<B>
where
    B: Body<Data = Bytes>,
    B::Error: std::fmt::Display,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        let result = this.inner.as_mut().poll_frame(cx);
        match &result {
            Poll::Ready(None) => this.complete_successfully(),
            Poll::Ready(Some(Err(error))) => {
                this.fail(format!("下游 HTTP 响应流失败: {error}"));
            }
            Poll::Pending | Poll::Ready(Some(Ok(_))) => {}
        }
        result
    }
}

#[derive(Debug)]
struct ObservedResponseStreamError {
    source: ProxyError,
}

impl ObservedResponseStreamError {
    fn new(error: impl Error + Send + Sync + 'static) -> Self {
        Self {
            source: Box::new(error),
        }
    }
}

impl std::fmt::Display for ObservedResponseStreamError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.source.fmt(formatter)
    }
}

impl Error for ObservedResponseStreamError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

fn is_observed_response_stream_error(error: &hyper::Error) -> bool {
    let mut current: Option<&(dyn Error + 'static)> = Some(error);
    while let Some(source) = current {
        if source.is::<ObservedResponseStreamError>() {
            return true;
        }
        current = source.source();
    }
    false
}

struct CancellableBody<B> {
    inner: Pin<Box<B>>,
    cancelled: Pin<Box<WaitForCancellationFutureOwned>>,
}

impl<B> CancellableBody<B> {
    fn new(inner: B, cancellation: CancellationToken) -> Self {
        Self {
            inner: Box::pin(inner),
            cancelled: Box::pin(cancellation.cancelled_owned()),
        }
    }
}

impl<B> Body for CancellableBody<B>
where
    B: Body<Data = Bytes>,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        if this.cancelled.as_mut().poll(cx).is_ready() {
            return Poll::Ready(None);
        }
        this.inner.as_mut().poll_frame(cx)
    }
}

fn is_upgrade_request(headers: &HeaderMap) -> bool {
    headers.contains_key(UPGRADE)
        || headers
            .get_all(CONNECTION)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(','))
            .any(|value| value.trim().eq_ignore_ascii_case("upgrade"))
}

fn is_websocket_upgrade(headers: &HeaderMap) -> bool {
    headers
        .get_all(UPGRADE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| value.trim().eq_ignore_ascii_case("websocket"))
        && headers
            .get_all(CONNECTION)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(','))
            .any(|value| value.trim().eq_ignore_ascii_case("upgrade"))
}

fn restore_upgrade_headers(headers: &mut HeaderMap, upgrade: HeaderValue) {
    headers.insert(CONNECTION, HeaderValue::from_static("upgrade"));
    headers.insert(UPGRADE, upgrade);
}

fn empty_body() -> ProxyBody {
    Full::new(Bytes::new())
        .map_err(|never| -> ProxyError { match never {} })
        .boxed()
}

fn text_response(status: StatusCode, message: &str) -> Response<ProxyBody> {
    let body = message.as_bytes().to_vec();
    let mut response = Response::new(
        Full::new(Bytes::from(body.clone()))
            .map_err(|never| -> ProxyError { match never {} })
            .boxed(),
    );
    *response.status_mut() = status;
    response.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&body.len().to_string()).expect("HTTP error body length is valid"),
    );
    response
}

fn worker_panic_error(payload: Box<dyn Any + Send>) -> String {
    let detail = payload
        .downcast_ref::<&str>()
        .map(|value| (*value).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "非文本 panic payload".into());
    format!("HTTP 转发线程 panic: {detail}")
}

#[cfg(test)]
pub(crate) mod integration_tests {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    use openssl::asn1::Asn1Time;
    use openssl::bn::{BigNum, MsbOption};
    use openssl::hash::MessageDigest;
    use openssl::pkey::{PKey, Private};
    use openssl::rsa::Rsa;
    use openssl::ssl::{SslAcceptor, SslMethod};
    use openssl::x509::extension::{
        BasicConstraints, ExtendedKeyUsage, KeyUsage, SubjectAlternativeName,
    };
    use openssl::x509::{X509NameBuilder, X509};
    use rustls::pki_types::CertificateDer;
    use rustls::{ClientConfig, RootCertStore};

    use super::super::model::{ForwardProtocol, ForwardRule};
    use super::super::observability::{HttpEventKind, ObservationCursor, HTTP_BODY_PREVIEW_LIMIT};
    use super::super::runtime::RuleRunner;
    use super::HttpRuleRunner;

    const SOCKET_TIMEOUT: Duration = Duration::from_secs(2);
    const RESPONSE_TIMEOUT: Duration = Duration::from_millis(80);
    const TLS_FIXTURE_ACCEPT_TIMEOUT: Duration = Duration::from_millis(500);

    fn http_rule(
        id: i64,
        target_url: String,
        capture_http_headers: bool,
        capture_http_body: bool,
    ) -> ForwardRule {
        ForwardRule {
            id,
            name: format!("HTTP 测试规则 {id}"),
            protocol: ForwardProtocol::Http,
            bind_host: "127.0.0.1".into(),
            listen_port: 0,
            target_url: Some(target_url),
            target_host: None,
            target_port: None,
            capture_http_headers,
            capture_http_body,
            auto_start: false,
            created_at: "2026-07-14 00:00:00".into(),
            updated_at: "2026-07-14 00:00:00".into(),
        }
    }

    fn accept_once(
        handler: impl FnOnce(TcpStream) + Send + 'static,
    ) -> (SocketAddr, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind HTTP upstream");
        let address = listener.local_addr().expect("read HTTP upstream address");
        let worker = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept HTTP upstream request");
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

    fn read_head(stream: &mut impl Read) -> Vec<u8> {
        let mut head = Vec::new();
        let mut byte = [0_u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).expect("read HTTP head");
            head.push(byte[0]);
            assert!(head.len() <= 16 * 1024, "HTTP head exceeds test limit");
        }
        head
    }

    fn header_value(head: &[u8], wanted: &str) -> Option<String> {
        let head = std::str::from_utf8(head).expect("HTTP headers are UTF-8 in test");
        head.split("\r\n").skip(1).find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case(wanted)
                .then(|| value.trim().to_string())
        })
    }

    fn content_length(head: &[u8]) -> usize {
        header_value(head, "content-length")
            .expect("HTTP response must include content length")
            .parse()
            .expect("content length is numeric")
    }

    fn read_response(stream: &mut TcpStream) -> (Vec<u8>, Vec<u8>) {
        let head = read_head(stream);
        let mut body = vec![0; content_length(&head)];
        stream
            .read_exact(&mut body)
            .expect("read complete HTTP body");
        (head, body)
    }

    fn write_response(stream: &mut impl Write, status: &str, content_type: &str, body: &[u8]) {
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .expect("write HTTP response head");
        stream.write_all(body).expect("write HTTP response body");
        stream.flush().expect("flush HTTP response");
    }

    fn certificate_serial() -> openssl::asn1::Asn1Integer {
        let mut serial = BigNum::new().expect("create certificate serial");
        serial
            .rand(128, MsbOption::MAYBE_ZERO, false)
            .expect("generate certificate serial");
        serial.to_asn1_integer().expect("encode certificate serial")
    }

    fn fixture_ca() -> (PKey<Private>, X509) {
        let key = PKey::from_rsa(Rsa::generate(2048).expect("generate fixture CA key"))
            .expect("create fixture CA key");
        let mut name = X509NameBuilder::new().expect("create fixture CA name");
        name.append_entry_by_text("CN", "LazyCat Request Forward Fixture CA")
            .expect("set fixture CA name");
        let name = name.build();
        let mut certificate = X509::builder().expect("create fixture CA certificate");
        certificate.set_version(2).expect("set fixture CA version");
        certificate
            .set_serial_number(&certificate_serial())
            .expect("set fixture CA serial");
        certificate
            .set_subject_name(&name)
            .expect("set fixture CA subject");
        certificate
            .set_issuer_name(&name)
            .expect("set fixture CA issuer");
        certificate
            .set_pubkey(&key)
            .expect("set fixture CA public key");
        certificate
            .set_not_before(&Asn1Time::days_from_now(0).expect("fixture CA not-before"))
            .expect("set fixture CA not-before");
        certificate
            .set_not_after(&Asn1Time::days_from_now(1).expect("fixture CA not-after"))
            .expect("set fixture CA not-after");
        certificate
            .append_extension(
                BasicConstraints::new()
                    .critical()
                    .ca()
                    .build()
                    .expect("build fixture CA constraints"),
            )
            .expect("set fixture CA constraints");
        certificate
            .append_extension(
                KeyUsage::new()
                    .critical()
                    .key_cert_sign()
                    .crl_sign()
                    .build()
                    .expect("build fixture CA key usage"),
            )
            .expect("set fixture CA key usage");
        certificate
            .sign(&key, MessageDigest::sha256())
            .expect("sign fixture CA certificate");
        (key, certificate.build())
    }

    fn fixture_server_certificate(
        ca_key: &PKey<Private>,
        ca: &X509,
        trust_ipv4_loopback: bool,
    ) -> (PKey<Private>, X509) {
        let key = PKey::from_rsa(Rsa::generate(2048).expect("generate fixture server key"))
            .expect("create fixture server key");
        let mut name = X509NameBuilder::new().expect("create fixture server name");
        name.append_entry_by_text("CN", "localhost")
            .expect("set fixture server name");
        let name = name.build();
        let mut certificate = X509::builder().expect("create fixture server certificate");
        certificate
            .set_version(2)
            .expect("set fixture server version");
        certificate
            .set_serial_number(&certificate_serial())
            .expect("set fixture server serial");
        certificate
            .set_subject_name(&name)
            .expect("set fixture server subject");
        certificate
            .set_issuer_name(ca.subject_name())
            .expect("set fixture server issuer");
        certificate
            .set_pubkey(&key)
            .expect("set fixture server public key");
        certificate
            .set_not_before(&Asn1Time::days_from_now(0).expect("fixture server not-before"))
            .expect("set fixture server not-before");
        certificate
            .set_not_after(&Asn1Time::days_from_now(1).expect("fixture server not-after"))
            .expect("set fixture server not-after");
        certificate
            .append_extension(
                BasicConstraints::new()
                    .critical()
                    .build()
                    .expect("build fixture server constraints"),
            )
            .expect("set fixture server constraints");
        certificate
            .append_extension(
                KeyUsage::new()
                    .critical()
                    .digital_signature()
                    .key_encipherment()
                    .build()
                    .expect("build fixture server key usage"),
            )
            .expect("set fixture server key usage");
        certificate
            .append_extension(
                ExtendedKeyUsage::new()
                    .server_auth()
                    .build()
                    .expect("build fixture server extended key usage"),
            )
            .expect("set fixture server extended key usage");
        let mut subject_alt_name = SubjectAlternativeName::new();
        subject_alt_name.dns("localhost");
        if trust_ipv4_loopback {
            subject_alt_name.ip("127.0.0.1");
        }
        let subject_alt_name = subject_alt_name
            .build(&certificate.x509v3_context(Some(ca), None))
            .expect("build fixture server subject alternative name");
        certificate
            .append_extension(subject_alt_name)
            .expect("set fixture server subject alternative name");
        certificate
            .sign(ca_key, MessageDigest::sha256())
            .expect("sign fixture server certificate");
        (key, certificate.build())
    }

    pub(crate) struct TlsFixture {
        pub(crate) address: SocketAddr,
        pub(crate) client_config: ClientConfig,
        result_rx: mpsc::Receiver<Result<(), String>>,
        worker: Option<JoinHandle<()>>,
    }

    impl TlsFixture {
        pub(crate) fn finish(mut self) -> Result<(), String> {
            let result = match self.result_rx.recv_timeout(SOCKET_TIMEOUT) {
                Ok(result) => result,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let _ = TcpStream::connect_timeout(&self.address, RESPONSE_TIMEOUT);
                    self.result_rx
                        .recv_timeout(SOCKET_TIMEOUT)
                        .map_err(|error| {
                            format!("TLS fixture did not finish after wake-up: {error}")
                        })?
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    Err("TLS fixture worker exited without a result".into())
                }
            };

            let worker = self.worker.take().expect("TLS fixture worker exists");
            let join_deadline = std::time::Instant::now() + SOCKET_TIMEOUT;
            while !worker.is_finished() && std::time::Instant::now() < join_deadline {
                thread::sleep(Duration::from_millis(5));
            }
            if !worker.is_finished() {
                return Err("TLS fixture worker did not exit within the timeout".into());
            }
            worker
                .join()
                .map_err(|_| "TLS fixture worker panicked".to_string())?;
            result
        }
    }

    pub(crate) fn accept_tls_once(
        trust_ipv4_loopback: bool,
        handler: impl FnOnce(&mut openssl::ssl::SslStream<TcpStream>) + Send + 'static,
    ) -> TlsFixture {
        let (ca_key, ca) = fixture_ca();
        let (server_key, server_certificate) =
            fixture_server_certificate(&ca_key, &ca, trust_ipv4_loopback);
        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls_server())
            .expect("create TLS fixture acceptor");
        acceptor
            .set_private_key(&server_key)
            .expect("set TLS fixture private key");
        acceptor
            .set_certificate(&server_certificate)
            .expect("set TLS fixture certificate");
        acceptor
            .check_private_key()
            .expect("validate TLS fixture certificate and key");
        let acceptor = acceptor.build();

        let mut roots = RootCertStore::empty();
        roots
            .add(CertificateDer::from(
                ca.to_der().expect("encode fixture CA certificate"),
            ))
            .expect("trust fixture CA certificate");
        let client_config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind HTTPS fixture");
        let address = listener.local_addr().expect("read HTTPS fixture address");
        listener
            .set_nonblocking(true)
            .expect("make HTTPS fixture listener non-blocking");
        let (result_tx, result_rx) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            let deadline = std::time::Instant::now() + TLS_FIXTURE_ACCEPT_TIMEOUT;
            let result = loop {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if let Err(error) = stream.set_nonblocking(false) {
                            break Err(format!("make TLS fixture stream blocking failed: {error}"));
                        }
                        if let Err(error) = stream.set_read_timeout(Some(SOCKET_TIMEOUT)) {
                            break Err(format!("set TLS fixture read timeout failed: {error}"));
                        }
                        if let Err(error) = stream.set_write_timeout(Some(SOCKET_TIMEOUT)) {
                            break Err(format!("set TLS fixture write timeout failed: {error}"));
                        }
                        break match acceptor.accept(stream) {
                            Ok(mut stream) => {
                                handler(&mut stream);
                                Ok(())
                            }
                            Err(error) => Err(format!("TLS fixture handshake failed: {error}")),
                        };
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if std::time::Instant::now() >= deadline {
                            break Err("TLS fixture accept timeout".into());
                        }
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => break Err(format!("TLS fixture accept failed: {error}")),
                }
            };
            let _ = result_tx.send(result);
        });
        TlsFixture {
            address,
            client_config,
            result_rx,
            worker: Some(worker),
        }
    }

    #[test]
    fn http_forwards_method_path_query_headers_and_streaming_body() {
        let request_body = vec![b'u'; HTTP_BODY_PREVIEW_LIMIT + 4096];
        let response_body = vec![b'd'; HTTP_BODY_PREVIEW_LIMIT + 2048];
        let (first_chunk_tx, first_chunk_rx) = mpsc::channel();
        let upstream_response = response_body.clone();
        let (upstream_addr, upstream) = accept_once(move |mut stream| {
            let head = read_head(&mut stream);
            let head_text = std::str::from_utf8(&head).expect("request head text");
            assert!(head_text.starts_with("POST /api/items?tag=blue HTTP/1.1\r\n"));
            assert_eq!(
                header_value(&head, "x-request-id").as_deref(),
                Some("trace-7")
            );
            assert_eq!(content_length(&head), HTTP_BODY_PREVIEW_LIMIT + 4096);

            let mut first = vec![0; 4096];
            stream
                .read_exact(&mut first)
                .expect("upstream receives first streamed chunk");
            assert_eq!(first, vec![b'u'; 4096]);
            first_chunk_tx
                .send(())
                .expect("signal first body chunk reaches upstream");
            let mut rest = vec![0; HTTP_BODY_PREVIEW_LIMIT];
            stream
                .read_exact(&mut rest)
                .expect("upstream receives rest of streamed body");
            assert_eq!(rest, vec![b'u'; HTTP_BODY_PREVIEW_LIMIT]);
            write_response(&mut stream, "200 OK", "text/plain", &upstream_response);
        });
        let rule = http_rule(1, format!("http://{upstream_addr}/api"), true, true);
        let runner = HttpRuleRunner::new();
        let handle = runner.start(&rule).expect("start HTTP rule");
        let listener_addr = runner.listener_addr(handle).expect("read listener address");

        let mut client = connect(listener_addr);
        write!(
            client,
            "POST /items?tag=blue HTTP/1.1\r\nHost: public.example\r\nX-Request-Id: trace-7\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            request_body.len()
        )
        .expect("write streaming request head");
        client
            .write_all(&request_body[..4096])
            .expect("write first body chunk");
        client.flush().expect("flush first body chunk");
        first_chunk_rx
            .recv_timeout(SOCKET_TIMEOUT)
            .expect("upstream sees first body chunk before client finishes upload");
        client
            .write_all(&request_body[4096..])
            .expect("write rest of body");
        let (response_head, received_body) = read_response(&mut client);
        assert!(std::str::from_utf8(&response_head)
            .expect("response head text")
            .starts_with("HTTP/1.1 200"));
        assert_eq!(received_body, response_body);
        upstream.join().expect("join HTTP upstream");

        let snapshot = runner
            .wait_for_snapshot(handle, |snapshot| {
                snapshot.upload_bytes == request_body.len() as u64
                    && snapshot.download_bytes == response_body.len() as u64
            })
            .expect("wait for HTTP counters");
        assert_eq!(snapshot.event_count, 1);
        assert_eq!(snapshot.error_count, 0);
        let completed = snapshot
            .events
            .iter()
            .find(|event| event.kind == HttpEventKind::Accepted)
            .expect("accepted HTTP event");
        assert_eq!(
            completed
                .request_body_preview
                .as_ref()
                .expect("captured request body")
                .bytes
                .len(),
            HTTP_BODY_PREVIEW_LIMIT
        );
        assert!(
            completed
                .request_body_preview
                .as_ref()
                .expect("captured request body")
                .truncated
        );
        assert_eq!(
            completed
                .response_body_preview
                .as_ref()
                .expect("captured response body")
                .bytes
                .len(),
            HTTP_BODY_PREVIEW_LIMIT
        );
        assert!(
            completed
                .response_body_preview
                .as_ref()
                .expect("captured response body")
                .truncated
        );
        runner.stop(handle).expect("stop HTTP rule");

        let (upstream_addr, upstream) = accept_once(|mut stream| {
            let head = read_head(&mut stream);
            let mut body = vec![0; content_length(&head)];
            stream.read_exact(&mut body).expect("read uncaptured body");
            write_response(&mut stream, "200 OK", "text/plain", b"ok");
        });
        let uncaptured_rule = http_rule(2, format!("http://{upstream_addr}"), false, false);
        let uncaptured_runner = HttpRuleRunner::new();
        let uncaptured_handle = uncaptured_runner
            .start(&uncaptured_rule)
            .expect("start body-capture-disabled rule");
        let mut client = connect(
            uncaptured_runner
                .listener_addr(uncaptured_handle)
                .expect("read uncaptured listener"),
        );
        client
            .write_all(b"POST / HTTP/1.1\r\nHost: public.example\r\nContent-Type: text/plain\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello")
            .expect("write uncaptured request");
        assert_eq!(read_response(&mut client).1, b"ok");
        upstream.join().expect("join uncaptured upstream");
        let snapshot = uncaptured_runner
            .wait_for_snapshot(uncaptured_handle, |snapshot| snapshot.download_bytes == 2)
            .expect("wait for uncaptured response");
        let completed = snapshot
            .events
            .iter()
            .find(|event| event.kind == HttpEventKind::Accepted)
            .expect("uncaptured accepted event");
        assert!(completed.request_body_preview.is_none());
        assert!(completed.response_body_preview.is_none());
        uncaptured_runner
            .stop(uncaptured_handle)
            .expect("stop uncaptured HTTP rule");
    }

    #[test]
    fn http_filters_hop_headers_and_rebuilds_forward_chain() {
        let (upstream_addr, upstream) = accept_once(|mut stream| {
            let head = read_head(&mut stream);
            for removed in [
                "connection",
                "keep-alive",
                "proxy-authorization",
                "te",
                "trailer",
                "upgrade",
                "x-remove-me",
            ] {
                assert!(
                    header_value(&head, removed).is_none(),
                    "{removed} was forwarded"
                );
            }
            assert_eq!(
                header_value(&head, "forwarded").as_deref(),
                Some("for=127.0.0.1;host=public.example;proto=http")
            );
            assert_eq!(
                header_value(&head, "x-forwarded-for").as_deref(),
                Some("127.0.0.1")
            );
            assert_eq!(
                header_value(&head, "x-forwarded-host").as_deref(),
                Some("public.example")
            );
            assert_eq!(
                header_value(&head, "x-forwarded-proto").as_deref(),
                Some("http")
            );
            assert!(header_value(&head, "host")
                .as_deref()
                .is_some_and(|host| host.starts_with("127.0.0.1:") && host != "public.example"));
            write_response(&mut stream, "204 No Content", "text/plain", b"");
        });
        let rule = http_rule(3, format!("http://{upstream_addr}"), true, false);
        let runner = HttpRuleRunner::new();
        let handle = runner.start(&rule).expect("start HTTP rule");
        let mut client = connect(runner.listener_addr(handle).expect("read listener"));
        client
            .write_all(b"GET / HTTP/1.1\r\nHost: public.example\r\nContent-Length: 0\r\nConnection: keep-alive, x-remove-me\r\nKeep-Alive: timeout=5\r\nProxy-Authorization: Basic secret\r\nTE: trailers\r\nTrailer: x-checksum\r\nX-Remove-Me: yes\r\nForwarded: for=spoofed\r\nX-Forwarded-For: spoofed\r\nX-Forwarded-Host: spoofed\r\nX-Forwarded-Proto: https\r\nConnection: close\r\n\r\n")
            .expect("write header filtering request");
        let (head, body) = read_response(&mut client);
        assert!(std::str::from_utf8(&head)
            .expect("response head text")
            .starts_with("HTTP/1.1 204"));
        assert!(body.is_empty());
        upstream.join().expect("join header upstream");
        runner.stop(handle).expect("stop HTTP rule");
    }

    #[test]
    fn http_returns_502_for_connect_or_tls_failure() {
        let unavailable = TcpListener::bind("127.0.0.1:0").expect("reserve unavailable port");
        let unavailable_addr = unavailable.local_addr().expect("read unavailable port");
        drop(unavailable);
        let rule = http_rule(4, format!("http://{unavailable_addr}"), false, false);
        let runner = HttpRuleRunner::new();
        let handle = runner.start(&rule).expect("start HTTP rule");
        let mut client = connect(runner.listener_addr(handle).expect("read listener"));
        client
            .write_all(b"GET /unavailable HTTP/1.1\r\nHost: public.example\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("write request to unavailable downstream");
        let snapshot = runner
            .wait_for_snapshot(handle, |snapshot| snapshot.error_count == 1)
            .unwrap_or_else(|error| {
                panic!(
                    "wait for 502 error event: {error}; current snapshot: {:?}",
                    runner
                        .observation_snapshot(handle)
                        .expect("read current HTTP observation")
                )
            });
        assert!(snapshot
            .events
            .iter()
            .any(|event| event.kind == HttpEventKind::DownstreamFailed));
        let (head, _) = read_response(&mut client);
        assert!(std::str::from_utf8(&head)
            .expect("response head text")
            .starts_with("HTTP/1.1 502"));
        runner.stop(handle).expect("stop HTTP rule");
    }

    #[test]
    fn http_returns_504_for_timeout_before_response_starts() {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (upstream_addr, upstream) = accept_once(move |mut stream| {
            let _ = read_head(&mut stream);
            started_tx.send(()).expect("signal upstream request");
            release_rx.recv().expect("release timed out upstream");
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 4\r\nConnection: close\r\n\r\nlate",
            );
        });
        let rule = http_rule(5, format!("http://{upstream_addr}"), false, false);
        let runner = HttpRuleRunner::with_response_timeout_for_test(RESPONSE_TIMEOUT);
        let handle = runner.start(&rule).expect("start HTTP rule");
        let mut client = connect(runner.listener_addr(handle).expect("read listener"));
        client
            .write_all(b"GET /slow HTTP/1.1\r\nHost: public.example\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("write slow request");
        started_rx
            .recv_timeout(SOCKET_TIMEOUT)
            .expect("upstream receives timeout test request");
        let (head, _) = read_response(&mut client);
        assert!(std::str::from_utf8(&head)
            .expect("response head text")
            .starts_with("HTTP/1.1 504"));
        let snapshot = runner
            .wait_for_snapshot(handle, |snapshot| snapshot.error_count == 1)
            .expect("wait for timeout event");
        assert!(snapshot
            .events
            .iter()
            .any(|event| event.kind == HttpEventKind::ResponseTimeout));
        release_tx.send(()).expect("release upstream worker");
        upstream.join().expect("join timeout upstream");
        runner.stop(handle).expect("stop HTTP rule");
    }

    #[test]
    fn http_response_stream_failure_finalizes_the_current_request_once() {
        let (upstream_addr, upstream) = accept_once(|mut stream| {
            let _ = read_head(&mut stream);
            stream
                .write_all(
                    b"HTTP/1.1 206 Partial Content\r\nContent-Type: text/plain\r\nX-Upstream: kept\r\nContent-Length: 10\r\nConnection: close\r\n\r\nabc",
                )
                .expect("write truncated upstream response");
            stream.flush().expect("flush truncated upstream response");
        });
        let rule = http_rule(50, format!("http://{upstream_addr}"), true, true);
        let runner = HttpRuleRunner::new();
        let handle = runner.start(&rule).expect("start HTTP rule");
        let mut client = connect(runner.listener_addr(handle).expect("read listener"));
        client
            .write_all(
                b"GET /stream-fail?q=1 HTTP/1.1\r\nHost: public.example\r\nX-Request-Id: stream-50\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .expect("write stream failure request");
        let mut response = Vec::new();
        client
            .read_to_end(&mut response)
            .expect("read response until failed stream closes");
        upstream.join().expect("join truncated upstream");

        let snapshot = runner
            .wait_for_snapshot(handle, |snapshot| {
                snapshot.error_count == 1
                    && (snapshot.events.len() >= 2
                        || snapshot.events.iter().any(|event| {
                            event.kind == HttpEventKind::ResponseStreamFailed
                                && event.method.is_some()
                        }))
            })
            .unwrap_or_else(|error| {
                panic!(
                    "wait for response stream failure finalization: {error}; current snapshot: {:?}",
                    runner
                        .observation_snapshot(handle)
                        .expect("read current HTTP observation")
                )
            });
        assert_eq!(snapshot.event_count, 1);
        assert_eq!(snapshot.error_count, 1);
        assert_eq!(snapshot.events.len(), 1);
        let failed = &snapshot.events[0];
        assert_eq!(failed.kind, HttpEventKind::ResponseStreamFailed);
        assert!(failed
            .error
            .as_deref()
            .is_some_and(|error| error.contains("下游 HTTP 响应流失败")));
        assert!(failed.client_addr.is_some());
        assert_eq!(failed.target_addr, upstream_addr.to_string());
        assert_eq!(failed.method.as_deref(), Some("GET"));
        assert_eq!(failed.path.as_deref(), Some("/stream-fail?q=1"));
        assert_eq!(failed.status_code, Some(206));
        assert_eq!(failed.upload_bytes, 0);
        assert_eq!(failed.download_bytes, 3);
        assert!(failed
            .request_headers
            .as_ref()
            .is_some_and(|headers| headers.iter().any(|(name, value)| {
                name.eq_ignore_ascii_case("x-request-id") && value == "stream-50"
            })));
        assert!(failed
            .response_headers
            .as_ref()
            .is_some_and(|headers| headers.iter().any(|(name, value)| {
                name.eq_ignore_ascii_case("x-upstream") && value == "kept"
            })));
        assert_eq!(
            failed
                .response_body_preview
                .as_ref()
                .map(|preview| preview.bytes.as_slice()),
            Some(b"abc".as_slice())
        );

        let batch = runner
            .observability(handle)
            .expect("read HTTP observability")
            .batch_since(ObservationCursor::default());
        assert_eq!(batch.events.len(), 1);
        assert_eq!(batch.events[0].kind, HttpEventKind::ResponseStreamFailed);

        runner.stop(handle).expect("stop HTTP rule");
    }

    #[test]
    fn http_returns_503_without_queueing_when_concurrency_is_full() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (upstream_addr, upstream) = accept_once(move |mut stream| {
            let _ = read_head(&mut stream);
            entered_tx.send(()).expect("signal first request upstream");
            release_rx.recv().expect("release first request");
            write_response(&mut stream, "200 OK", "text/plain", b"first");
        });
        let rule = http_rule(6, format!("http://{upstream_addr}"), false, false);
        let runner = HttpRuleRunner::with_connection_limit(1);
        let handle = runner.start(&rule).expect("start limited HTTP rule");
        let listener_addr = runner.listener_addr(handle).expect("read listener");

        let mut first = connect(listener_addr);
        first
            .write_all(b"GET /first HTTP/1.1\r\nHost: public.example\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("write first request");
        entered_rx
            .recv_timeout(SOCKET_TIMEOUT)
            .expect("first request reaches upstream");

        let mut overloaded = connect(listener_addr);
        overloaded
            .write_all(b"GET /second HTTP/1.1\r\nHost: public.example\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("write overloaded request");
        let (head, _) = read_response(&mut overloaded);
        assert!(std::str::from_utf8(&head)
            .expect("response head text")
            .starts_with("HTTP/1.1 503"));
        let snapshot = runner
            .wait_for_snapshot(handle, |snapshot| snapshot.error_count == 1)
            .expect("wait for overload event");
        assert_eq!(snapshot.event_count, 2);
        assert!(snapshot
            .events
            .iter()
            .any(|event| event.kind == HttpEventKind::Overloaded));

        release_tx.send(()).expect("release first request");
        assert_eq!(read_response(&mut first).1, b"first");
        upstream.join().expect("join overload upstream");
        runner.stop(handle).expect("stop HTTP rule");
    }

    #[test]
    fn http_streams_sse_without_waiting_for_completion() {
        let (first_chunk_tx, first_chunk_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (upstream_addr, upstream) = accept_once(move |mut stream| {
            let _ = read_head(&mut stream);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n")
                .expect("write SSE response head");
            stream
                .write_all(b"d\r\ndata: first\n\n\r\n")
                .expect("write first SSE chunk");
            stream.flush().expect("flush first SSE chunk");
            first_chunk_tx.send(()).expect("signal first SSE chunk");
            release_rx.recv().expect("release second SSE chunk");
            stream
                .write_all(b"e\r\ndata: second\n\n\r\n0\r\n\r\n")
                .expect("write final SSE chunks");
            stream.flush().expect("flush final SSE chunks");
        });
        let rule = http_rule(7, format!("http://{upstream_addr}"), false, false);
        let runner = HttpRuleRunner::with_response_timeout_for_test(RESPONSE_TIMEOUT);
        let handle = runner.start(&rule).expect("start SSE HTTP rule");
        let mut client = connect(runner.listener_addr(handle).expect("read listener"));
        client
            .write_all(b"GET /events HTTP/1.1\r\nHost: public.example\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("write SSE request");
        let head = read_head(&mut client);
        assert!(std::str::from_utf8(&head)
            .expect("SSE response head text")
            .starts_with("HTTP/1.1 200"));
        first_chunk_rx
            .recv_timeout(SOCKET_TIMEOUT)
            .expect("upstream emits first SSE chunk");
        let mut first_chunk = [0_u8; 3];
        client
            .read_exact(&mut first_chunk)
            .expect("client receives first SSE chunk immediately");
        assert!(matches!(&first_chunk, b"d\r\n" | b"D\r\n"));
        thread::sleep(RESPONSE_TIMEOUT + Duration::from_millis(40));
        release_tx
            .send(())
            .expect("release second SSE chunk after timeout window");
        let mut rest = Vec::new();
        client
            .read_to_end(&mut rest)
            .expect("read remaining SSE bytes");
        assert!(rest
            .windows(b"data: second".len())
            .any(|window| window == b"data: second"));
        upstream.join().expect("join SSE upstream");
        runner.stop(handle).expect("stop SSE HTTP rule");
    }

    #[test]
    fn http_forwards_websocket_upgrade_and_bidirectional_bytes() {
        const CLIENT_PAYLOAD: &[u8] = b"client-websocket-frame";
        const SERVER_PAYLOAD: &[u8] = b"server-websocket-frame";

        let (upstream_addr, upstream) = accept_once(|mut stream| {
            let head = read_head(&mut stream);
            let head_text = std::str::from_utf8(&head).expect("WebSocket request head text");
            assert!(head_text.starts_with("GET /api/socket?room=7 HTTP/1.1\r\n"));
            assert_eq!(
                header_value(&head, "connection").as_deref(),
                Some("upgrade")
            );
            assert_eq!(header_value(&head, "upgrade").as_deref(), Some("websocket"));
            assert_eq!(
                header_value(&head, "sec-websocket-key").as_deref(),
                Some("fixture-key")
            );
            assert!(header_value(&head, "x-remove-me").is_none());
            stream
                .write_all(b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: fixture-accept\r\n\r\n")
                .expect("write WebSocket upgrade response");
            stream.flush().expect("flush WebSocket upgrade response");

            let mut payload = vec![0; CLIENT_PAYLOAD.len()];
            stream
                .read_exact(&mut payload)
                .expect("upstream receives WebSocket bytes");
            assert_eq!(payload, CLIENT_PAYLOAD);
            stream
                .write_all(SERVER_PAYLOAD)
                .expect("upstream writes WebSocket bytes");
            stream.flush().expect("flush upstream WebSocket bytes");
        });
        let rule = http_rule(8, format!("http://{upstream_addr}/api"), true, false);
        let runner = HttpRuleRunner::new();
        let handle = runner.start(&rule).expect("start WebSocket HTTP rule");
        let mut client = connect(runner.listener_addr(handle).expect("read listener"));
        client
            .write_all(b"GET /socket?room=7 HTTP/1.1\r\nHost: public.example\r\nConnection: keep-alive, Upgrade, x-remove-me\r\nUpgrade: websocket\r\nSec-WebSocket-Key: fixture-key\r\nSec-WebSocket-Version: 13\r\nX-Remove-Me: yes\r\n\r\n")
            .expect("write WebSocket upgrade request");

        let head = read_head(&mut client);
        let head_text = std::str::from_utf8(&head).expect("WebSocket response head text");
        assert!(head_text.starts_with("HTTP/1.1 101"));
        assert_eq!(
            header_value(&head, "connection").as_deref(),
            Some("upgrade")
        );
        assert_eq!(header_value(&head, "upgrade").as_deref(), Some("websocket"));
        assert_eq!(
            header_value(&head, "sec-websocket-accept").as_deref(),
            Some("fixture-accept")
        );

        client
            .write_all(CLIENT_PAYLOAD)
            .expect("client writes WebSocket bytes");
        client.flush().expect("flush client WebSocket bytes");
        let mut payload = vec![0; SERVER_PAYLOAD.len()];
        client
            .read_exact(&mut payload)
            .expect("client receives WebSocket bytes");
        assert_eq!(payload, SERVER_PAYLOAD);
        drop(client);
        upstream.join().expect("join WebSocket upstream");

        let snapshot = runner
            .wait_for_snapshot(handle, |snapshot| {
                snapshot.events.iter().any(|event| {
                    event.kind == HttpEventKind::Accepted
                        && event.status_code == Some(101)
                        && event.upload_bytes == CLIENT_PAYLOAD.len() as u64
                        && event.download_bytes == SERVER_PAYLOAD.len() as u64
                })
            })
            .expect("wait for completed WebSocket event");
        assert_eq!(snapshot.error_count, 0);
        runner.stop(handle).expect("stop WebSocket HTTP rule");
    }

    #[test]
    fn http_rejects_non_websocket_upgrade_explicitly() {
        let unavailable = TcpListener::bind("127.0.0.1:0").expect("reserve target port");
        let target_addr = unavailable.local_addr().expect("read target port");
        drop(unavailable);
        let rule = http_rule(81, format!("http://{target_addr}"), false, false);
        let runner = HttpRuleRunner::new();
        let handle = runner.start(&rule).expect("start HTTP rule");
        let mut client = connect(runner.listener_addr(handle).expect("read listener"));
        client
            .write_all(b"GET /h2 HTTP/1.1\r\nHost: public.example\r\nContent-Length: 0\r\nConnection: Upgrade\r\nUpgrade: h2c\r\n\r\n")
            .expect("write unsupported upgrade request");
        let (head, _) = read_response(&mut client);
        let head = std::str::from_utf8(&head).expect("response head text");
        assert!(head.starts_with("HTTP/1.1 400"));
        let snapshot = runner
            .wait_for_snapshot(handle, |snapshot| snapshot.error_count == 1)
            .expect("wait for upgrade rejection event");
        assert!(snapshot
            .events
            .iter()
            .any(|event| event.kind == HttpEventKind::UpgradeRejected));
        runner.stop(handle).expect("stop HTTP rule");
    }

    #[test]
    fn stopping_http_rule_closes_active_websocket_tunnel() {
        let (upgraded_tx, upgraded_rx) = mpsc::channel();
        let (closed_tx, closed_rx) = mpsc::channel();
        let (upstream_addr, upstream) = accept_once(move |mut stream| {
            let _ = read_head(&mut stream);
            stream
                .write_all(b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n")
                .expect("write active WebSocket response");
            stream.flush().expect("flush active WebSocket response");
            upgraded_tx.send(()).expect("signal WebSocket upgraded");
            let mut remaining = Vec::new();
            let _ = stream.read_to_end(&mut remaining);
            closed_tx
                .send(())
                .expect("signal WebSocket downstream closed");
        });
        let rule = http_rule(82, format!("http://{upstream_addr}"), false, false);
        let runner = HttpRuleRunner::new();
        let handle = runner.start(&rule).expect("start active WebSocket rule");
        let listener_addr = runner.listener_addr(handle).expect("read listener");
        let mut client = connect(listener_addr);
        client
            .write_all(b"GET /socket HTTP/1.1\r\nHost: public.example\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n")
            .expect("write active WebSocket request");
        let response = read_head(&mut client);
        assert!(std::str::from_utf8(&response)
            .expect("active WebSocket response text")
            .starts_with("HTTP/1.1 101"));
        upgraded_rx
            .recv_timeout(SOCKET_TIMEOUT)
            .expect("upstream accepted WebSocket upgrade");

        runner.stop(handle).expect("stop active WebSocket rule");
        closed_rx
            .recv_timeout(SOCKET_TIMEOUT)
            .expect("downstream WebSocket closes on stop");
        upstream.join().expect("join stopped WebSocket upstream");
        assert!(TcpStream::connect_timeout(&listener_addr, RESPONSE_TIMEOUT).is_err());

        let mut remaining = Vec::new();
        match client.read_to_end(&mut remaining) {
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::NotConnected
                ) => {}
            result => panic!("client WebSocket did not close after HTTP stop: {result:?}"),
        }
    }

    #[test]
    fn stopping_http_rule_closes_listener_and_active_request() {
        let (started_tx, started_rx) = mpsc::channel();
        let (closed_tx, closed_rx) = mpsc::channel();
        let (upstream_addr, upstream) = accept_once(move |mut stream| {
            let _ = read_head(&mut stream);
            started_tx.send(()).expect("signal active HTTP request");
            let mut remaining = Vec::new();
            let _ = stream.read_to_end(&mut remaining);
            closed_tx.send(()).expect("signal downstream closed");
        });
        let rule = http_rule(9, format!("http://{upstream_addr}"), false, false);
        let runner = HttpRuleRunner::new();
        let handle = runner.start(&rule).expect("start cancellable HTTP rule");
        let listener_addr = runner.listener_addr(handle).expect("read listener");
        let mut client = connect(listener_addr);
        client
            .write_all(b"GET /pending HTTP/1.1\r\nHost: public.example\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("write pending request");
        started_rx
            .recv_timeout(SOCKET_TIMEOUT)
            .expect("upstream receives pending request");

        runner.stop(handle).expect("stop active HTTP rule");
        closed_rx
            .recv_timeout(SOCKET_TIMEOUT)
            .expect("downstream connection closes on stop");
        upstream.join().expect("join cancelled upstream");
        assert!(TcpStream::connect_timeout(&listener_addr, RESPONSE_TIMEOUT).is_err());

        client
            .set_read_timeout(Some(RESPONSE_TIMEOUT))
            .expect("set stopped client read timeout");
        let mut remaining = Vec::new();
        match client.read_to_end(&mut remaining) {
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::NotConnected
                ) => {}
            result => panic!("client connection did not close after HTTP stop: {result:?}"),
        }
    }

    #[test]
    fn https_forwards_through_a_trusted_fixture_ca() {
        let fixture = accept_tls_once(true, |stream| {
            let head = read_head(stream);
            let head_text = std::str::from_utf8(&head).expect("HTTPS request head text");
            assert!(head_text.starts_with("POST /api/items?tag=secure HTTP/1.1\r\n"));
            assert_eq!(
                header_value(&head, "x-request-id").as_deref(),
                Some("tls-10")
            );
            let mut body = vec![0; content_length(&head)];
            stream
                .read_exact(&mut body)
                .expect("read HTTPS request body");
            assert_eq!(body, b"hello tls");
            write_response(stream, "200 OK", "text/plain", b"secure response");
        });
        let rule = http_rule(10, format!("https://{}/api", fixture.address), true, true);
        let runner =
            HttpRuleRunner::with_tls_config_for_test(Arc::new(fixture.client_config.clone()));
        let handle = runner.start(&rule).expect("start HTTPS rule");
        let mut client = connect(runner.listener_addr(handle).expect("read HTTPS listener"));
        client
            .write_all(b"POST /items?tag=secure HTTP/1.1\r\nHost: public.example\r\nX-Request-Id: tls-10\r\nContent-Type: text/plain\r\nContent-Length: 9\r\nConnection: close\r\n\r\nhello tls")
            .expect("write HTTPS forwarding request");

        let (head, body) = read_response(&mut client);
        assert!(
            std::str::from_utf8(&head)
                .expect("HTTPS response head text")
                .starts_with("HTTP/1.1 200"),
            "unexpected HTTPS response: head={:?}, body={:?}",
            String::from_utf8_lossy(&head),
            String::from_utf8_lossy(&body),
        );
        assert_eq!(body, b"secure response");
        fixture.finish().expect("finish trusted HTTPS fixture");
        runner.stop(handle).expect("stop HTTPS rule");
    }

    #[test]
    fn https_returns_502_when_the_certificate_hostname_is_wrong() {
        let fixture = accept_tls_once(false, |_| {
            panic!("hostname-mismatched TLS connection must not send HTTP");
        });
        let rule = http_rule(11, format!("https://{}", fixture.address), false, false);
        let runner =
            HttpRuleRunner::with_tls_config_for_test(Arc::new(fixture.client_config.clone()));
        let handle = runner
            .start(&rule)
            .expect("start hostname mismatch HTTPS rule");
        let mut client = connect(runner.listener_addr(handle).expect("read HTTPS listener"));
        client
            .write_all(b"GET /wrong-host HTTP/1.1\r\nHost: public.example\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .expect("write hostname mismatch request");

        let (head, body) = read_response(&mut client);
        assert!(std::str::from_utf8(&head)
            .expect("hostname mismatch response head text")
            .starts_with("HTTP/1.1 502"));
        let body_text = std::str::from_utf8(&body).expect("hostname mismatch response body text");
        assert!(body_text.contains("连接下游 HTTP 服务失败"));
        assert!(body_text.to_ascii_lowercase().contains("certificate"));
        assert!(body_text
            .to_ascii_lowercase()
            .contains("not valid for name"));
        let snapshot = runner
            .wait_for_snapshot(handle, |snapshot| snapshot.error_count == 1)
            .expect("wait for hostname mismatch event");
        let failed = snapshot
            .events
            .iter()
            .find(|event| event.kind == HttpEventKind::DownstreamFailed)
            .expect("hostname mismatch is recorded as downstream failure");
        let observed_error = failed
            .error
            .as_deref()
            .expect("TLS failure keeps error detail");
        assert!(observed_error.to_ascii_lowercase().contains("certificate"));
        assert!(observed_error
            .to_ascii_lowercase()
            .contains("not valid for name"));
        let fixture_error = fixture
            .finish()
            .expect_err("hostname mismatch must fail the TLS fixture handshake");
        assert!(fixture_error.contains("handshake failed"));
        runner
            .stop(handle)
            .expect("stop hostname mismatch HTTPS rule");
    }

    #[test]
    fn tls_fixture_exits_within_timeout_without_a_client_connection() {
        let fixture = accept_tls_once(true, |_| {});
        let started = std::time::Instant::now();

        let error = fixture
            .finish()
            .expect_err("unused TLS fixture must report an accept timeout");

        assert!(error.contains("accept timeout"));
        assert!(started.elapsed() < SOCKET_TIMEOUT + Duration::from_secs(1));
    }

    #[test]
    fn http_start_does_not_load_tls_roots_but_https_start_does() {
        let load_count = Arc::new(AtomicUsize::new(0));
        let counted_loads = Arc::clone(&load_count);
        let runner = HttpRuleRunner::with_tls_config_loader_for_test(move || {
            counted_loads.fetch_add(1, Ordering::SeqCst);
            Err("fixture native roots unavailable".into())
        });
        let http = http_rule(12, "http://127.0.0.1:9".into(), false, false);

        let handle = runner
            .start(&http)
            .expect("plain HTTP starts without loading TLS roots");
        assert_eq!(load_count.load(Ordering::SeqCst), 0);
        runner.stop(handle).expect("stop plain HTTP rule");

        let https = http_rule(13, "https://127.0.0.1:9".into(), false, false);
        let error = runner
            .start(&https)
            .expect_err("HTTPS start must surface TLS root loading failure");
        assert!(error.contains("fixture native roots unavailable"));
        assert_eq!(load_count.load(Ordering::SeqCst), 1);
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use hyper::http::{
        header::{
            CONNECTION, FORWARDED, HOST, PROXY_AUTHENTICATE, PROXY_AUTHORIZATION, TE, TRAILER,
            TRANSFER_ENCODING, UPGRADE,
        },
        HeaderMap, HeaderValue, Uri,
    };
    use url::Url;

    use super::{build_target_uri, rebuild_forward_headers, replace_host_header, strip_hop_by_hop};

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
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_util::sync::{CancellationToken, WaitForCancellationFutureOwned};

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
type HttpClient = Client<HttpConnector, ProxyBody>;

pub(crate) struct HttpRuleRunner {
    next_handle: AtomicU64,
    running: Mutex<HashMap<u64, HttpRunningRule>>,
    connection_limit: usize,
    pre_response_timeout: Duration,
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
        assert!(
            connection_limit > 0,
            "HTTP connection limit must be positive"
        );
        Self {
            next_handle: AtomicU64::new(1),
            running: Mutex::new(HashMap::new()),
            connection_limit,
            pre_response_timeout,
        }
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

    fn connector(&self) -> HttpConnector {
        let mut http = HttpConnector::new();
        // Windows 对不可达地址的异步 connect 可能长时间保持 pending；连接阶段必须有独立上限。
        http.set_connect_timeout(Some(HTTP_CONNECT_TIMEOUT));
        http
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
        if target_url.scheme() != "http" {
            return Err("当前版本暂不支持 HTTPS 下游".to_string());
        }
        let connector = self.connector();
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
    connector: HttpConnector,
    cancellation: CancellationToken,
    observability: Arc<HttpObservability>,
    connection_limit: usize,
    pre_response_timeout: Duration,
    capture_http_headers: bool,
    capture_http_body: bool,
) -> Result<(), String> {
    let client = Client::builder(TokioExecutor::new()).build(connector);
    let semaphore = Arc::new(Semaphore::new(connection_limit));
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
                                pre_response_timeout,
                                capture_http_headers,
                                capture_http_body,
                            )
                        });
                        let serve = hyper::server::conn::http1::Builder::new()
                            .serve_connection(TokioIo::new(stream), service);
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
                    return Err(error);
                }
            },
            completed = children.join_next(), if !children.is_empty() => {
                if completed.is_some_and(|result| result.is_err()) {
                    let error = "HTTP 转发子任务 panic".to_string();
                    observability.child_task_failed(error.clone());
                    cancellation.cancel();
                    while children.join_next().await.is_some() {}
                    return Err(error);
                }
            },
        }
    }

    drop(listener);
    cancellation.cancel();
    while children.join_next().await.is_some() {}
    Ok(())
}

async fn forward_request(
    request: Request<Incoming>,
    client_addr: SocketAddr,
    target_url: Url,
    client: HttpClient,
    cancellation: CancellationToken,
    observability: Arc<HttpObservability>,
    semaphore: Arc<Semaphore>,
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

    if is_upgrade_request(request.headers()) {
        let error = "HTTP 转发不支持 WebSocket 或 Upgrade 请求".to_string();
        trace.upgrade_rejected(error.clone());
        return Ok(text_response(StatusCode::BAD_REQUEST, &error));
    }

    let _permit = match semaphore.try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            let error = "HTTP 并发请求已达到上限".to_string();
            trace.overloaded(error.clone());
            return Ok(text_response(StatusCode::SERVICE_UNAVAILABLE, &error));
        }
    };

    let (mut parts, body) = request.into_parts();
    let target_uri = match build_target_uri(&target_url, &parts.uri) {
        Ok(uri) => uri,
        Err(error) => {
            trace.downstream_failed(error.clone());
            return Ok(text_response(StatusCode::BAD_REQUEST, &error));
        }
    };
    strip_hop_by_hop(&mut parts.headers);
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
    let response = tokio::select! {
        _ = cancellation.cancelled() => return Ok(text_response(StatusCode::SERVICE_UNAVAILABLE, "HTTP 转发已停止")),
        response = timeout(pre_response_timeout, client.request(outbound)) => match response {
            Ok(Ok(response)) => response,
            Ok(Err(error)) => {
                let error = format!("连接下游 HTTP 服务失败: {error}");
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

    let (mut parts, body) = response.into_parts();
    strip_hop_by_hop(&mut parts.headers);
    trace.response_started(parts.status.as_u16(), &parts.headers);
    Ok(Response::from_parts(
        parts,
        observe_response_body(body, trace, cancellation),
    ))
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
mod integration_tests {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::sync::mpsc;
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    use super::super::model::{ForwardProtocol, ForwardRule};
    use super::super::observability::{
        HttpEventKind, ObservationCursor, HTTP_BODY_PREVIEW_LIMIT,
    };
    use super::super::runtime::RuleRunner;
    use super::HttpRuleRunner;

    const SOCKET_TIMEOUT: Duration = Duration::from_secs(2);
    const RESPONSE_TIMEOUT: Duration = Duration::from_millis(80);

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
    fn http_rejects_websocket_upgrade_explicitly() {
        let unavailable = TcpListener::bind("127.0.0.1:0").expect("reserve target port");
        let target_addr = unavailable.local_addr().expect("read target port");
        drop(unavailable);
        let rule = http_rule(8, format!("http://{target_addr}"), false, false);
        let runner = HttpRuleRunner::new();
        let handle = runner.start(&rule).expect("start HTTP rule");
        let mut client = connect(runner.listener_addr(handle).expect("read listener"));
        client
            .write_all(b"GET /socket HTTP/1.1\r\nHost: public.example\r\nContent-Length: 0\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nConnection: close\r\n\r\n")
            .expect("write websocket upgrade request");
        let (head, _) = read_response(&mut client);
        let head = std::str::from_utf8(&head).expect("response head text");
        assert!(head.starts_with("HTTP/1.1 400") || head.starts_with("HTTP/1.1 426"));
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
    fn https_downstream_is_rejected_explicitly() {
        let rule = http_rule(10, "https://example.com".into(), false, false);
        let error = HttpRuleRunner::new()
            .start(&rule)
            .expect_err("HTTPS downstream must remain disabled in this version");
        assert!(error.contains("暂不支持 HTTPS 下游"));
    }
}

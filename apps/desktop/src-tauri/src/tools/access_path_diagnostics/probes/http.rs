use serde::Serialize;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use url::Url;

const USER_AGENT: &str = "LazyCat-Access-Path-Diagnostics/1";

#[derive(Debug, Clone)]
pub(crate) struct HttpProbeLimits {
    pub timeout: Duration,
    pub max_redirects: usize,
    pub max_response_header_bytes: usize,
    pub max_response_body_bytes: usize,
}

impl Default for HttpProbeLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(15),
            max_redirects: 5,
            max_response_header_bytes: 32 * 1024,
            max_response_body_bytes: 16 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum HttpMethod {
    Head,
    Get,
}

impl HttpMethod {
    fn as_str(self) -> &'static str {
        match self {
            Self::Head => "HEAD",
            Self::Get => "GET",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HttpHeaderEvidence {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HttpExchangeEvidence {
    pub url: String,
    pub method: HttpMethod,
    pub status: u16,
    pub reason: String,
    pub headers: Vec<HttpHeaderEvidence>,
    pub response_header_bytes: usize,
    pub response_body_bytes: usize,
    pub response_body_truncated: bool,
    pub elapsed_ms: u64,
    pub connection_ip: Option<String>,
    pub http_host: String,
    pub via_proxy: bool,
    pub proxy: Option<String>,
    pub head_fallback: bool,
    pub redirects_to: Option<String>,
    pub redirect_cross_host: bool,
    pub redirect_cross_scheme: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HttpProbeResult {
    pub final_url: String,
    pub final_status: u16,
    pub redirect_count: usize,
    pub exchanges: Vec<HttpExchangeEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProxyConnectEvidence {
    pub target_authority: String,
    pub proxy_address: String,
    pub status: u16,
    pub reason: String,
    pub headers: Vec<HttpHeaderEvidence>,
    pub response_header_bytes: usize,
    pub elapsed_ms: u64,
}

#[derive(Debug)]
pub(crate) struct EstablishedProxyTunnel {
    pub stream: TcpStream,
    pub evidence: ProxyConnectEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HttpProbeError {
    pub code: String,
    pub message: String,
    pub raw_error: Option<String>,
    pub io_kind: Option<String>,
    pub exchanges: Vec<HttpExchangeEvidence>,
    pub proxy_connect: Option<ProxyConnectEvidence>,
}

impl HttpProbeError {
    pub(crate) fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            raw_error: None,
            io_kind: None,
            exchanges: Vec::new(),
            proxy_connect: None,
        }
    }

    pub(crate) fn from_io(code: &str, context: &str, error: io::Error) -> Self {
        Self {
            code: code.into(),
            message: format!("{context}: {error}"),
            raw_error: Some(error.to_string()),
            io_kind: Some(format!("{:?}", error.kind()).to_ascii_lowercase()),
            exchanges: Vec::new(),
            proxy_connect: None,
        }
    }

    fn with_exchanges(mut self, exchanges: Vec<HttpExchangeEvidence>) -> Self {
        self.exchanges = exchanges;
        self
    }
}

impl fmt::Display for HttpProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for HttpProbeError {}

#[derive(Debug, Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct PlainHttpProbeRequest {
    pub url: Url,
    pub http_host: Option<String>,
    pub connection_ip: Option<IpAddr>,
    pub limits: HttpProbeLimits,
}

#[derive(Debug, Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct HttpProxyProbeRequest {
    pub url: Url,
    pub http_host: Option<String>,
    pub proxy_address: SocketAddr,
    pub proxy_label: String,
    pub limits: HttpProbeLimits,
}

pub(crate) trait HttpTransport: Read + Write {}

impl<T: Read + Write> HttpTransport for T {}

pub(crate) struct ConnectedHttpTransport {
    pub stream: Box<dyn HttpTransport + Send>,
    pub connection_ip: Option<String>,
    pub absolute_form: bool,
    pub via_proxy: bool,
    pub proxy: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ConnectorHttpProbeRequest {
    pub url: Url,
    pub http_host: Option<String>,
    pub limits: HttpProbeLimits,
    pub cancellation: CancellationToken,
}

#[derive(Debug, Clone, Copy)]
enum RequestTargetForm {
    Origin,
    Absolute,
}

#[derive(Debug)]
struct ParsedResponse {
    status: u16,
    reason: String,
    headers: Vec<HttpHeaderEvidence>,
    all_headers: Vec<(String, String)>,
    header_bytes: usize,
    body_prefix: Vec<u8>,
    body_bytes: usize,
    body_truncated: bool,
}

impl ParsedResponse {
    fn header(&self, name: &str) -> Option<&str> {
        self.all_headers
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn probe_plain_http(
    request: PlainHttpProbeRequest,
) -> Result<HttpProbeResult, HttpProbeError> {
    if request.url.scheme() != "http" {
        return Err(HttpProbeError::new(
            "tls_transport_required",
            "HTTPS 必须先完成 TLS 握手，再调用 HTTP exchange",
        ));
    }
    probe_http_with_connector(
        ConnectorHttpProbeRequest {
            url: request.url,
            http_host: request.http_host,
            limits: request.limits,
            cancellation: CancellationToken::new(),
        },
        move |url, remaining, first_exchange| {
            let connector_started = Instant::now();
            if url.scheme() != "http" {
                return Err(HttpProbeError::new(
                    "tls_transport_required",
                    format!("重定向目标 {} 需要 TLS transport", evidence_url(url)),
                ));
            }
            let address = if first_exchange {
                request
                    .connection_ip
                    .map(|ip| SocketAddr::new(ip, url.port_or_known_default().unwrap_or(80)))
            } else {
                None
            };
            let stream = connect_origin(url, address, remaining)?;
            configure_stream(
                &stream,
                remaining
                    .checked_sub(connector_started.elapsed())
                    .filter(|timeout| !timeout.is_zero())
                    .ok_or_else(|| {
                        HttpProbeError::new("http_timeout", "HTTP 连接耗尽整体超时时间")
                    })?,
            )?;
            let connection_ip = stream
                .peer_addr()
                .ok()
                .map(|address| address.ip().to_string());
            Ok(ConnectedHttpTransport {
                stream: Box::new(stream),
                connection_ip,
                absolute_form: false,
                via_proxy: false,
                proxy: None,
            })
        },
    )
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn probe_plain_http_via_proxy(
    request: HttpProxyProbeRequest,
) -> Result<HttpProbeResult, HttpProbeError> {
    if request.url.scheme() != "http" {
        return Err(HttpProbeError::new(
            "proxy_connect_required",
            "HTTPS 代理访问必须先建立 CONNECT 隧道，不能按明文 HTTP 标记成功",
        ));
    }
    let proxy_address = request.proxy_address;
    let proxy_label = request.proxy_label.clone();
    probe_http_with_connector(
        ConnectorHttpProbeRequest {
            url: request.url,
            http_host: request.http_host,
            limits: request.limits,
            cancellation: CancellationToken::new(),
        },
        move |url, remaining, _first_exchange| {
            let connector_started = Instant::now();
            if url.scheme() != "http" {
                return Err(HttpProbeError::new(
                    "proxy_connect_required",
                    format!(
                        "重定向目标 {} 需要 CONNECT 和 TLS transport",
                        evidence_url(url)
                    ),
                ));
            }
            let stream = connect_socket(proxy_address, remaining)?;
            configure_stream(
                &stream,
                remaining
                    .checked_sub(connector_started.elapsed())
                    .filter(|timeout| !timeout.is_zero())
                    .ok_or_else(|| {
                        HttpProbeError::new("http_timeout", "HTTP 代理连接耗尽整体超时时间")
                    })?,
            )?;
            let connection_ip = stream
                .peer_addr()
                .ok()
                .map(|address| address.ip().to_string());
            Ok(ConnectedHttpTransport {
                stream: Box::new(stream),
                connection_ip,
                absolute_form: true,
                via_proxy: true,
                proxy: Some(proxy_label.clone()),
            })
        },
    )
}

/// Drives HEAD/fallback GET and redirects over caller-provided transports.
///
/// The connector receives the remaining overall timeout and must apply it to
/// every blocking operation on the returned transport.
pub(crate) fn probe_http_with_connector<F>(
    request: ConnectorHttpProbeRequest,
    mut connector: F,
) -> Result<HttpProbeResult, HttpProbeError>
where
    F: FnMut(&Url, Duration, bool) -> Result<ConnectedHttpTransport, HttpProbeError>,
{
    let mut url = request.url;
    let initial_http_host = request.http_host;
    let limits = request.limits;
    let cancellation = request.cancellation;
    let started = Instant::now();
    let mut exchanges = Vec::new();
    let mut redirects = 0usize;
    let mut first_exchange = true;

    loop {
        ensure_not_cancelled(&cancellation)
            .map_err(|error| error.with_exchanges(exchanges.clone()))?;
        let host = if first_exchange {
            initial_http_host
                .as_deref()
                .map(str::to_owned)
                .unwrap_or_else(|| host_header_for_url(&url))
        } else {
            host_header_for_url(&url)
        };
        let mut method = HttpMethod::Head;
        let mut head_fallback = false;
        let redirect_location;

        loop {
            ensure_not_cancelled(&cancellation)
                .map_err(|error| error.with_exchanges(exchanges.clone()))?;
            let remaining = remaining_timeout(started, limits.timeout)
                .map_err(|error| error.with_exchanges(exchanges.clone()))?;
            let mut transport = connector(&url, remaining, first_exchange)
                .map_err(|error| error.with_exchanges(exchanges.clone()))?;
            ensure_not_cancelled(&cancellation)
                .map_err(|error| error.with_exchanges(exchanges.clone()))?;
            remaining_timeout(started, limits.timeout)
                .map_err(|error| error.with_exchanges(exchanges.clone()))?;
            let exchange_started = Instant::now();
            write_request(
                &mut transport.stream,
                &url,
                &host,
                method,
                if transport.absolute_form {
                    RequestTargetForm::Absolute
                } else {
                    RequestTargetForm::Origin
                },
                limits.max_response_body_bytes,
            )
            .map_err(|error| error.with_exchanges(exchanges.clone()))?;
            let response = read_response(&mut transport.stream, method, &limits)
                .map_err(|error| error.with_exchanges(exchanges.clone()))?;
            let status = response.status;
            let response_location = response.header("location").map(str::to_owned);
            let fallback = method == HttpMethod::Head && matches!(status, 405 | 501);
            let exchange = to_exchange_evidence(
                &url,
                &host,
                method,
                response,
                exchange_started.elapsed(),
                transport.connection_ip,
                transport.via_proxy,
                transport.proxy,
                head_fallback,
            );
            exchanges.push(exchange);
            if started.elapsed() > limits.timeout {
                return Err(
                    HttpProbeError::new("http_timeout", "HTTP 探测超过整体耗时限制")
                        .with_exchanges(exchanges),
                );
            }

            if transport.via_proxy && status == 407 {
                return Err(HttpProbeError::new(
                    "proxy_authentication_required",
                    "HTTP 代理返回 407，需要代理认证；诊断器未发送凭据",
                )
                .with_exchanges(exchanges));
            }
            if fallback {
                ensure_not_cancelled(&cancellation)
                    .map_err(|error| error.with_exchanges(exchanges.clone()))?;
                method = HttpMethod::Get;
                head_fallback = true;
                continue;
            }
            redirect_location = response_location;
            break;
        }

        let latest_index = exchanges.len() - 1;
        let latest_status = exchanges[latest_index].status;
        let redirect_target = if is_redirect(latest_status) {
            redirect_location
        } else {
            None
        };
        let Some(location) = redirect_target else {
            return Ok(HttpProbeResult {
                final_url: evidence_url(&url),
                final_status: latest_status,
                redirect_count: redirects,
                exchanges,
            });
        };
        if redirects >= limits.max_redirects {
            return Err(HttpProbeError::new(
                "redirect_limit_exceeded",
                format!("HTTP 重定向超过 {} 次限制", limits.max_redirects),
            )
            .with_exchanges(exchanges));
        }
        let next = url.join(&location).map_err(|error| {
            HttpProbeError::new(
                "invalid_redirect_location",
                format!("无法解析重定向 Location: {error}"),
            )
            .with_exchanges(exchanges.clone())
        })?;
        ensure_not_cancelled(&cancellation)
            .map_err(|error| error.with_exchanges(exchanges.clone()))?;
        if !matches!(next.scheme(), "http" | "https") {
            return Err(HttpProbeError::new(
                "unsupported_redirect_scheme",
                format!("不支持重定向协议 {}", next.scheme()),
            )
            .with_exchanges(exchanges));
        }
        let redirect_cross_host = normalized_authority(&url) != normalized_authority(&next);
        let redirect_cross_scheme = url.scheme() != next.scheme();
        let latest = &mut exchanges[latest_index];
        latest.redirects_to = Some(evidence_url(&next));
        latest.redirect_cross_host = redirect_cross_host;
        latest.redirect_cross_scheme = redirect_cross_scheme;
        url = next;
        redirects += 1;
        first_exchange = false;
    }
}

pub(crate) fn connect_http_proxy_tunnel(
    proxy_address: SocketAddr,
    target_authority: &str,
    limits: &HttpProbeLimits,
) -> Result<EstablishedProxyTunnel, HttpProbeError> {
    connect_http_proxy_tunnel_cancellable(
        proxy_address,
        target_authority,
        limits,
        &CancellationToken::new(),
    )
}

pub(crate) fn connect_http_proxy_tunnel_cancellable(
    proxy_address: SocketAddr,
    target_authority: &str,
    limits: &HttpProbeLimits,
    cancellation: &CancellationToken,
) -> Result<EstablishedProxyTunnel, HttpProbeError> {
    ensure_not_cancelled(cancellation)?;
    validate_authority(target_authority)?;
    let started = Instant::now();
    let mut stream = connect_socket(proxy_address, limits.timeout)?;
    configure_stream(&stream, remaining_timeout(started, limits.timeout)?)?;
    ensure_not_cancelled(cancellation)?;
    let request = format!(
        "CONNECT {target_authority} HTTP/1.1\r\nHost: {target_authority}\r\nUser-Agent: {USER_AGENT}\r\nProxy-Connection: keep-alive\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).map_err(|error| {
        HttpProbeError::from_io("proxy_write_failed", "写入 CONNECT 请求失败", error)
    })?;
    ensure_not_cancelled(cancellation)?;
    stream.flush().map_err(|error| {
        HttpProbeError::from_io("proxy_write_failed", "提交 CONNECT 请求失败", error)
    })?;
    let response = read_response_head(&mut stream, limits.max_response_header_bytes)?;
    let evidence = ProxyConnectEvidence {
        target_authority: target_authority.into(),
        proxy_address: proxy_address.to_string(),
        status: response.status,
        reason: response.reason,
        headers: response.headers,
        response_header_bytes: response.header_bytes,
        elapsed_ms: duration_ms(started.elapsed()),
    };
    if evidence.status == 407 {
        let mut error = HttpProbeError::new(
            "proxy_authentication_required",
            "代理 CONNECT 返回 407，需要认证；诊断器未发送凭据",
        );
        error.proxy_connect = Some(evidence);
        return Err(error);
    }
    if !(200..300).contains(&evidence.status) {
        let mut error = HttpProbeError::new(
            "proxy_connect_rejected",
            format!(
                "代理 CONNECT 返回非成功状态 {} {}",
                evidence.status, evidence.reason
            ),
        );
        error.proxy_connect = Some(evidence);
        return Err(error);
    }
    Ok(EstablishedProxyTunnel { stream, evidence })
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), HttpProbeError> {
    if cancellation.is_cancelled() {
        return Err(HttpProbeError::new(
            "diagnosis_cancelled",
            "诊断已取消，未继续发送 HTTP 请求",
        ));
    }
    Ok(())
}

fn write_request<S: Write>(
    stream: &mut S,
    url: &Url,
    http_host: &str,
    method: HttpMethod,
    target_form: RequestTargetForm,
    max_body_bytes: usize,
) -> Result<(), HttpProbeError> {
    validate_header_value("Host", http_host)?;
    let target = match target_form {
        RequestTargetForm::Origin => origin_form(url),
        RequestTargetForm::Absolute => request_url(url),
    };
    let mut request = format!(
        "{} {target} HTTP/1.1\r\nHost: {http_host}\r\nUser-Agent: {USER_AGENT}\r\nAccept: */*\r\nAccept-Encoding: identity\r\nConnection: close\r\n",
        method.as_str()
    );
    if method == HttpMethod::Get {
        let end = max_body_bytes.saturating_sub(1);
        request.push_str(&format!("Range: bytes=0-{end}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).map_err(|error| {
        HttpProbeError::from_io("http_write_failed", "写入 HTTP 请求失败", error)
    })?;
    stream
        .flush()
        .map_err(|error| HttpProbeError::from_io("http_write_failed", "提交 HTTP 请求失败", error))
}

fn read_response<S: Read>(
    stream: &mut S,
    method: HttpMethod,
    limits: &HttpProbeLimits,
) -> Result<ParsedResponse, HttpProbeError> {
    let mut parsed = read_response_head(stream, limits.max_response_header_bytes)?;
    if method == HttpMethod::Head || response_has_no_body(parsed.status) {
        return Ok(parsed);
    }

    let declared_length = parsed
        .header("content-length")
        .and_then(|value| value.trim().parse::<usize>().ok());
    let read_limit = limits.max_response_body_bytes.saturating_add(1);
    let target = declared_length.map_or(read_limit, |length| length.min(read_limit));
    let mut body = vec![0u8; target];
    let mut read = parsed.body_prefix.len().min(target);
    body[..read].copy_from_slice(&parsed.body_prefix[..read]);
    while read < target {
        match stream.read(&mut body[read..]) {
            Ok(0) => break,
            Ok(count) => read += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(HttpProbeError::from_io(
                    "http_body_read_failed",
                    "读取受限 HTTP 响应体失败",
                    error,
                ));
            }
        }
    }
    parsed.body_bytes = read.min(limits.max_response_body_bytes);
    parsed.body_truncated = read > limits.max_response_body_bytes
        || declared_length.is_some_and(|length| length > limits.max_response_body_bytes);
    Ok(parsed)
}

fn read_response_head<S: Read>(
    stream: &mut S,
    max_header_bytes: usize,
) -> Result<ParsedResponse, HttpProbeError> {
    if max_header_bytes < 64 {
        return Err(HttpProbeError::new(
            "invalid_http_limits",
            "响应头上限不能小于 64 字节",
        ));
    }
    let mut bytes = Vec::with_capacity(max_header_bytes.min(4096));
    let mut chunk = [0u8; 1024];
    let header_end = loop {
        if let Some(index) = find_header_end(&bytes) {
            break index;
        }
        if bytes.len() >= max_header_bytes {
            return Err(HttpProbeError::new(
                "response_headers_too_large",
                format!("HTTP 响应头超过 {max_header_bytes} 字节限制"),
            ));
        }
        let remaining = max_header_bytes
            .saturating_add(4)
            .saturating_sub(bytes.len());
        let read_len = chunk.len().min(remaining);
        let count = stream.read(&mut chunk[..read_len]).map_err(|error| {
            HttpProbeError::from_io("http_header_read_failed", "读取 HTTP 响应头失败", error)
        })?;
        if count == 0 {
            return Err(HttpProbeError::new(
                "incomplete_http_headers",
                "连接在完整 HTTP 响应头到达前关闭",
            ));
        }
        bytes.extend_from_slice(&chunk[..count]);
    };
    if header_end > max_header_bytes {
        return Err(HttpProbeError::new(
            "response_headers_too_large",
            format!("HTTP 响应头超过 {max_header_bytes} 字节限制"),
        ));
    }
    let mut parsed = parse_response_head(&bytes[..header_end], header_end)?;
    parsed.body_prefix.extend_from_slice(&bytes[header_end..]);
    Ok(parsed)
}

fn parse_response_head(
    bytes: &[u8],
    header_bytes: usize,
) -> Result<ParsedResponse, HttpProbeError> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    let mut status_parts = status_line.splitn(3, ' ');
    let version = status_parts.next().unwrap_or_default();
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1") {
        return Err(HttpProbeError::new(
            "invalid_http_response",
            format!("无效 HTTP 状态行: {status_line}"),
        ));
    }
    let status = status_parts
        .next()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|value| (100..=999).contains(value))
        .ok_or_else(|| {
            HttpProbeError::new(
                "invalid_http_response",
                format!("无效 HTTP 状态码: {status_line}"),
            )
        })?;
    let reason = status_parts.next().unwrap_or_default().trim().to_string();
    let mut all_headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if line.starts_with([' ', '\t']) {
            return Err(HttpProbeError::new(
                "invalid_http_response",
                "不接受折叠 HTTP 响应头",
            ));
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(HttpProbeError::new(
                "invalid_http_response",
                format!("无效 HTTP 响应头: {line}"),
            ));
        };
        let name = name.trim();
        if name.is_empty() || !name.bytes().all(is_header_name_byte) {
            return Err(HttpProbeError::new(
                "invalid_http_response",
                format!("无效 HTTP 响应头名称: {name}"),
            ));
        }
        all_headers.push((name.to_ascii_lowercase(), value.trim().to_string()));
    }
    let headers = all_headers
        .iter()
        .filter(|(name, _)| is_evidence_header(name))
        .map(|(name, value)| HttpHeaderEvidence {
            name: name.clone(),
            value: if name.eq_ignore_ascii_case("proxy-authenticate") {
                sanitized_authenticate_scheme(value)
            } else {
                value.clone()
            },
        })
        .collect();
    Ok(ParsedResponse {
        status,
        reason,
        headers,
        all_headers,
        header_bytes,
        body_prefix: Vec::new(),
        body_bytes: 0,
        body_truncated: false,
    })
}

#[allow(clippy::too_many_arguments)]
fn to_exchange_evidence(
    url: &Url,
    http_host: &str,
    method: HttpMethod,
    mut response: ParsedResponse,
    elapsed: Duration,
    connection_ip: Option<String>,
    via_proxy: bool,
    proxy: Option<String>,
    head_fallback: bool,
) -> HttpExchangeEvidence {
    for header in &mut response.headers {
        if header.name.eq_ignore_ascii_case("location") {
            header.value = url
                .join(&header.value)
                .map(|location| evidence_url(&location))
                .unwrap_or_else(|_| "<invalid-redacted>".into());
        } else if header.name.eq_ignore_ascii_case("proxy-authenticate") {
            header.value = sanitized_authenticate_scheme(&header.value);
        }
    }
    HttpExchangeEvidence {
        url: evidence_url(url),
        method,
        status: response.status,
        reason: response.reason,
        headers: response.headers,
        response_header_bytes: response.header_bytes,
        response_body_bytes: response.body_bytes,
        response_body_truncated: response.body_truncated,
        elapsed_ms: duration_ms(elapsed),
        connection_ip,
        http_host: http_host.into(),
        via_proxy,
        proxy,
        head_fallback,
        redirects_to: None,
        redirect_cross_host: false,
        redirect_cross_scheme: false,
    }
}

fn sanitized_authenticate_scheme(value: &str) -> String {
    value
        .split_ascii_whitespace()
        .next()
        .filter(|scheme| {
            !scheme.is_empty()
                && scheme
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
        })
        .map(|scheme| format!("{scheme} <parameters-redacted>"))
        .unwrap_or_else(|| "<invalid-redacted>".into())
}

#[cfg_attr(not(test), allow(dead_code))]
fn connect_origin(
    url: &Url,
    override_address: Option<SocketAddr>,
    timeout: Duration,
) -> Result<TcpStream, HttpProbeError> {
    if let Some(address) = override_address {
        return connect_socket(address, timeout);
    }
    let host = url
        .host_str()
        .ok_or_else(|| HttpProbeError::new("invalid_http_url", "URL 缺少主机名"))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| HttpProbeError::new("invalid_http_url", "URL 缺少有效端口"))?;
    let addresses = (host, port).to_socket_addrs().map_err(|error| {
        HttpProbeError::from_io("http_name_resolution_failed", "解析 HTTP 目标失败", error)
    })?;
    let started = Instant::now();
    let mut last_error = None;
    for address in addresses {
        let remaining = match timeout.checked_sub(started.elapsed()) {
            Some(value) if !value.is_zero() => value,
            _ => break,
        };
        match TcpStream::connect_timeout(&address, remaining) {
            Ok(stream) => return Ok(stream),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.map_or_else(
        || HttpProbeError::new("http_connect_failed", "目标未解析出可连接地址"),
        |error| HttpProbeError::from_io("http_connect_failed", "连接 HTTP 目标失败", error),
    ))
}

fn connect_socket(address: SocketAddr, timeout: Duration) -> Result<TcpStream, HttpProbeError> {
    TcpStream::connect_timeout(&address, timeout)
        .map_err(|error| HttpProbeError::from_io("http_connect_failed", "建立 TCP 连接失败", error))
}

fn configure_stream(stream: &TcpStream, timeout: Duration) -> Result<(), HttpProbeError> {
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|_| stream.set_write_timeout(Some(timeout)))
        .map_err(|error| {
            HttpProbeError::from_io("http_timeout_setup_failed", "设置 HTTP I/O 超时失败", error)
        })
}

fn remaining_timeout(started: Instant, timeout: Duration) -> Result<Duration, HttpProbeError> {
    timeout
        .checked_sub(started.elapsed())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| HttpProbeError::new("http_timeout", "HTTP 探测超过整体耗时限制"))
}

fn request_url(url: &Url) -> String {
    let mut request_url = url.clone();
    let _ = request_url.set_username("");
    let _ = request_url.set_password(None);
    request_url.set_fragment(None);
    request_url.to_string()
}

fn evidence_url(url: &Url) -> String {
    let mut evidence_url = url.clone();
    let _ = evidence_url.set_username("");
    let _ = evidence_url.set_password(None);
    evidence_url.set_fragment(None);
    if evidence_url.query().is_some() {
        let pairs = evidence_url
            .query_pairs()
            .map(|(name, value)| {
                let value = if is_sensitive_query_name(&name) {
                    "<redacted>".into()
                } else {
                    value.into_owned()
                };
                (name.into_owned(), value)
            })
            .collect::<Vec<_>>();
        evidence_url.query_pairs_mut().clear().extend_pairs(pairs);
    }
    evidence_url.to_string()
}

fn is_sensitive_query_name(name: &str) -> bool {
    let normalized = name.trim().to_ascii_lowercase().replace('-', "_");
    matches!(
        normalized.as_str(),
        "token"
            | "access_token"
            | "auth_token"
            | "api_key"
            | "apikey"
            | "key"
            | "secret"
            | "client_secret"
            | "password"
            | "passwd"
            | "signature"
            | "sig"
            | "auth"
            | "authorization"
            | "code"
            | "session"
            | "session_id"
            | "sessionid"
    ) || [
        "_token",
        "_secret",
        "_password",
        "_signature",
        "_api_key",
        "_session",
    ]
    .iter()
    .any(|suffix| normalized.ends_with(suffix))
}

fn origin_form(url: &Url) -> String {
    let mut target = if url.path().is_empty() {
        "/".to_string()
    } else {
        url.path().to_string()
    };
    if let Some(query) = url.query() {
        target.push('?');
        target.push_str(query);
    }
    target
}

fn host_header_for_url(url: &Url) -> String {
    let host = match url.host() {
        Some(url::Host::Ipv6(address)) => format!("[{address}]"),
        Some(host) => host.to_string(),
        None => String::new(),
    };
    match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host,
    }
}

fn normalized_authority(url: &Url) -> (Option<String>, Option<u16>) {
    (
        url.host_str().map(|host| host.to_ascii_lowercase()),
        url.port_or_known_default(),
    )
}

fn validate_header_value(name: &str, value: &str) -> Result<(), HttpProbeError> {
    if value.is_empty() || value.contains(['\r', '\n']) {
        return Err(HttpProbeError::new(
            "invalid_http_header",
            format!("{name} 包含无效字符"),
        ));
    }
    Ok(())
}

fn validate_authority(authority: &str) -> Result<(), HttpProbeError> {
    validate_header_value("CONNECT authority", authority)?;
    if authority.contains('/') || authority.contains('@') {
        return Err(HttpProbeError::new(
            "invalid_proxy_target",
            "CONNECT 目标必须是 host:port authority，且不能包含凭据",
        ));
    }
    Ok(())
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
}

fn is_header_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

fn is_evidence_header(name: &str) -> bool {
    matches!(
        name,
        "location"
            | "server"
            | "content-type"
            | "content-length"
            | "transfer-encoding"
            | "via"
            | "x-cache"
            | "retry-after"
            | "allow"
            | "proxy-authenticate"
    )
}

fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

fn response_has_no_body(status: u16) -> bool {
    (100..200).contains(&status) || matches!(status, 204 | 304)
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::io::Cursor;
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;

    struct ScriptedTransport {
        response: Cursor<Vec<u8>>,
        requests: Arc<Mutex<Vec<Vec<u8>>>>,
        request_index: usize,
    }

    impl Read for ScriptedTransport {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.response.read(buffer)
        }
    }

    impl Write for ScriptedTransport {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.requests.lock().unwrap()[self.request_index].extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn limits() -> HttpProbeLimits {
        HttpProbeLimits {
            timeout: Duration::from_secs(2),
            max_redirects: 3,
            max_response_header_bytes: 1024,
            max_response_body_bytes: 8,
        }
    }

    fn spawn_http_fixture<F>(requests: usize, handler: F) -> SocketAddr
    where
        F: Fn(String) -> String + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handler = Arc::new(handler);
        thread::spawn(move || {
            for stream in listener.incoming().take(requests) {
                let mut stream = stream.unwrap();
                let request = read_fixture_request(&mut stream);
                stream.write_all(handler(request).as_bytes()).unwrap();
            }
        });
        address
    }

    fn read_fixture_request(stream: &mut TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 512];
        while find_header_end(&bytes).is_none() {
            let count = stream.read(&mut chunk).unwrap();
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..count]);
        }
        String::from_utf8(bytes).unwrap()
    }

    fn request(address: SocketAddr, path: &str) -> PlainHttpProbeRequest {
        PlainHttpProbeRequest {
            url: Url::parse(&format!("http://localhost:{}{path}", address.port())).unwrap(),
            http_host: None,
            connection_ip: Some(address.ip()),
            limits: limits(),
        }
    }

    #[test]
    fn preserves_application_4xx_and_5xx_as_http_results() {
        for status in [404, 503] {
            let address = spawn_http_fixture(1, move |_| {
                format!("HTTP/1.1 {status} Test\r\nContent-Length: 0\r\n\r\n")
            });
            let result = probe_plain_http(request(address, "/status")).unwrap();
            assert_eq!(result.final_status, status);
            assert_eq!(result.exchanges[0].method, HttpMethod::Head);
        }
    }

    #[test]
    fn follows_redirect_and_marks_cross_host_and_scheme() {
        let target = spawn_http_fixture(1, |_| {
            "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".into()
        });
        let first = spawn_http_fixture(1, move |_| {
            format!(
                "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{}/done\r\nContent-Length: 0\r\n\r\n",
                target.port()
            )
        });
        let result = probe_plain_http(request(first, "/start")).unwrap();
        assert_eq!(result.final_status, 204);
        assert_eq!(result.redirect_count, 1);
        assert!(result.exchanges[0].redirect_cross_host);
        assert!(!result.exchanges[0].redirect_cross_scheme);
        assert_eq!(
            result.exchanges[0].redirects_to,
            Some(format!("http://127.0.0.1:{}/done", target.port()))
        );
    }

    #[test]
    fn retries_405_head_with_bounded_get_without_credentials_or_body() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&requests);
        let address = spawn_http_fixture(2, move |request| {
            captured.lock().unwrap().push(request.clone());
            if request.starts_with("HEAD ") {
                "HTTP/1.1 405 Method Not Allowed\r\nAllow: GET\r\nContent-Length: 0\r\n\r\n".into()
            } else {
                "HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\nabcdefghijklmnopqrst".into()
            }
        });
        let result = probe_plain_http(request(address, "/fallback")).unwrap();
        assert_eq!(result.final_status, 200);
        assert_eq!(result.exchanges.len(), 2);
        assert_eq!(result.exchanges[1].method, HttpMethod::Get);
        assert!(result.exchanges[1].head_fallback);
        assert_eq!(result.exchanges[1].response_body_bytes, 8);
        assert!(result.exchanges[1].response_body_truncated);
        let captured = requests.lock().unwrap();
        assert!(captured[1].contains("Range: bytes=0-7\r\n"));
        assert!(!captured[1].to_ascii_lowercase().contains("authorization:"));
        assert!(!captured[1].to_ascii_lowercase().contains("cookie:"));
        assert!(captured[1].ends_with("\r\n\r\n"));
    }

    #[test]
    fn custom_transport_connector_supports_https_head_fallback() {
        let responses = Arc::new(Mutex::new(VecDeque::from([
            b"HTTP/1.1 405 Method Not Allowed\r\nAllow: GET\r\nContent-Length: 0\r\n\r\n".to_vec(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ntest".to_vec(),
        ])));
        let requests = Arc::new(Mutex::new(vec![Vec::new(), Vec::new()]));
        let call_index = Arc::new(Mutex::new(0usize));
        let result = probe_http_with_connector(
            ConnectorHttpProbeRequest {
                url: Url::parse("https://api.test/health").unwrap(),
                http_host: Some("api.test".into()),
                limits: limits(),
                cancellation: CancellationToken::new(),
            },
            {
                let responses = Arc::clone(&responses);
                let requests = Arc::clone(&requests);
                let call_index = Arc::clone(&call_index);
                move |_url, _remaining, _first_exchange| {
                    let mut index = call_index.lock().unwrap();
                    let request_index = *index;
                    *index += 1;
                    Ok(ConnectedHttpTransport {
                        stream: Box::new(ScriptedTransport {
                            response: Cursor::new(responses.lock().unwrap().pop_front().unwrap()),
                            requests: Arc::clone(&requests),
                            request_index,
                        }),
                        connection_ip: Some("192.0.2.10".into()),
                        absolute_form: false,
                        via_proxy: true,
                        proxy: Some("connect-fixture".into()),
                    })
                }
            },
        )
        .unwrap();
        assert_eq!(result.final_status, 200);
        assert_eq!(result.exchanges.len(), 2);
        assert_eq!(result.exchanges[0].method, HttpMethod::Head);
        assert_eq!(result.exchanges[1].method, HttpMethod::Get);
        assert_eq!(
            result.exchanges[1].connection_ip.as_deref(),
            Some("192.0.2.10")
        );
        assert_eq!(
            result.exchanges[1].proxy.as_deref(),
            Some("connect-fixture")
        );
        let requests = requests.lock().unwrap();
        assert!(requests[0].starts_with(b"HEAD /health HTTP/1.1\r\n"));
        assert!(requests[1].starts_with(b"GET /health HTTP/1.1\r\n"));
    }

    #[test]
    fn ordinary_http_proxy_uses_absolute_form_and_reports_407() {
        let captured = Arc::new(Mutex::new(String::new()));
        let observed = Arc::clone(&captured);
        let proxy = spawn_http_fixture(1, move |request| {
            *observed.lock().unwrap() = request;
            "HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic realm=fixture\r\nContent-Length: 0\r\n\r\n".into()
        });
        let error = probe_plain_http_via_proxy(HttpProxyProbeRequest {
            url: Url::parse("http://example.test/path?q=1").unwrap(),
            http_host: None,
            proxy_address: proxy,
            proxy_label: "fixture-proxy".into(),
            limits: limits(),
        })
        .unwrap_err();
        assert_eq!(error.code, "proxy_authentication_required");
        assert_eq!(error.exchanges[0].status, 407);
        let challenge = error.exchanges[0]
            .headers
            .iter()
            .find(|header| header.name == "proxy-authenticate")
            .unwrap();
        assert_eq!(challenge.value, "Basic <parameters-redacted>");
        assert!(!challenge.value.contains("fixture"));
        let request = captured.lock().unwrap();
        assert!(request.starts_with("HEAD http://example.test/path?q=1 HTTP/1.1\r\n"));
        assert!(!request
            .to_ascii_lowercase()
            .contains("proxy-authorization:"));
    }

    #[test]
    fn evidence_redacts_sensitive_query_values_without_changing_request_target() {
        let captured = Arc::new(Mutex::new(String::new()));
        let observed = Arc::clone(&captured);
        let address = spawn_http_fixture(1, move |request| {
            *observed.lock().unwrap() = request;
            "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".into()
        });
        let result = probe_plain_http(request(
            address,
            "/check?access_token=top-secret&mode=full&session_id=sensitive",
        ))
        .unwrap();
        let evidence = Url::parse(&result.exchanges[0].url).unwrap();
        let values = evidence.query_pairs().collect::<Vec<_>>();
        assert!(values.contains(&("access_token".into(), "<redacted>".into())));
        assert!(values.contains(&("mode".into(), "full".into())));
        assert!(values.contains(&("session_id".into(), "<redacted>".into())));
        let request = captured.lock().unwrap();
        assert!(request.starts_with(
            "HEAD /check?access_token=top-secret&mode=full&session_id=sensitive HTTP/1.1\r\n"
        ));
    }

    #[test]
    fn redirect_location_is_redacted_in_evidence_but_followed_with_original_value() {
        let target_request = Arc::new(Mutex::new(String::new()));
        let observed = Arc::clone(&target_request);
        let target = spawn_http_fixture(1, move |request| {
            *observed.lock().unwrap() = request;
            "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n".into()
        });
        let first = spawn_http_fixture(1, move |_| {
            format!(
                "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{}/done?token=secret&mode=full\r\nContent-Length: 0\r\n\r\n",
                target.port()
            )
        });
        let result = probe_plain_http(request(first, "/start")).unwrap();
        let location = result.exchanges[0]
            .headers
            .iter()
            .find(|header| header.name == "location")
            .unwrap();
        let evidence_location = Url::parse(&location.value).unwrap();
        assert!(evidence_location
            .query_pairs()
            .any(|pair| pair == ("token".into(), "<redacted>".into())));
        assert!(target_request
            .lock()
            .unwrap()
            .starts_with("HEAD /done?token=secret&mode=full HTTP/1.1\r\n"));
    }

    #[test]
    fn malformed_redirect_location_never_enters_evidence() {
        let address = spawn_http_fixture(1, |_| {
            "HTTP/1.1 302 Found\r\nLocation: http://[invalid?token=secret\r\nContent-Length: 0\r\n\r\n"
                .into()
        });
        let error = probe_plain_http(request(address, "/start")).unwrap_err();
        assert_eq!(error.code, "invalid_redirect_location");
        let location = error.exchanges[0]
            .headers
            .iter()
            .find(|header| header.name == "location")
            .unwrap();
        assert_eq!(location.value, "<invalid-redacted>");
        assert!(!serde_json::to_string(&error).unwrap().contains("secret"));
    }

    #[test]
    fn pre_cancelled_driver_does_not_invoke_connector() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let called = Arc::new(Mutex::new(false));
        let observed = Arc::clone(&called);
        let error = probe_http_with_connector(
            ConnectorHttpProbeRequest {
                url: Url::parse("https://api.test/health").unwrap(),
                http_host: Some("api.test".into()),
                limits: limits(),
                cancellation,
            },
            move |_url, _remaining, _first_exchange| {
                *observed.lock().unwrap() = true;
                panic!("cancelled probe must not invoke connector");
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "diagnosis_cancelled");
        assert!(!*called.lock().unwrap());
    }

    #[test]
    fn connect_returns_live_tunnel_and_serializable_handshake_evidence() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy = listener.local_addr().unwrap();
        let observed = Arc::new(Mutex::new(String::new()));
        let captured = Arc::clone(&observed);
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            *captured.lock().unwrap() = read_fixture_request(&mut stream);
            stream
                .write_all(b"HTTP/1.1 200 Connection Established\r\nVia: fixture\r\n\r\n")
                .unwrap();
            let mut byte = [0u8; 1];
            stream.read_exact(&mut byte).unwrap();
            stream.write_all(&byte).unwrap();
        });
        let mut tunnel = connect_http_proxy_tunnel(proxy, "api.test:443", &limits()).unwrap();
        assert_eq!(tunnel.evidence.status, 200);
        tunnel.stream.write_all(b"x").unwrap();
        let mut echoed = [0u8; 1];
        tunnel.stream.read_exact(&mut echoed).unwrap();
        assert_eq!(echoed, [b'x']);
        let request = observed.lock().unwrap();
        assert!(request.starts_with("CONNECT api.test:443 HTTP/1.1\r\n"));
        assert!(!request
            .to_ascii_lowercase()
            .contains("proxy-authorization:"));
        assert_eq!(
            serde_json::to_value(&tunnel.evidence).unwrap()["status"],
            200
        );
    }

    #[test]
    fn connect_classifies_rejection_and_header_limit() {
        let proxy = spawn_http_fixture(1, |_| {
            "HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n".into()
        });
        let error = connect_http_proxy_tunnel(proxy, "api.test:443", &limits()).unwrap_err();
        assert_eq!(error.code, "proxy_connect_rejected");
        assert_eq!(error.proxy_connect.unwrap().status, 502);

        let proxy = spawn_http_fixture(1, |_| {
            format!("HTTP/1.1 200 OK\r\nX-Large: {}\r\n\r\n", "x".repeat(2_000))
        });
        let error = connect_http_proxy_tunnel(proxy, "api.test:443", &limits()).unwrap_err();
        assert_eq!(error.code, "response_headers_too_large");
    }

    #[test]
    fn timeout_and_https_without_tls_are_explicit_failures() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(150));
        });
        let mut request = request(address, "/slow");
        request.limits.timeout = Duration::from_millis(30);
        let error = probe_plain_http(request).unwrap_err();
        assert_eq!(error.code, "http_header_read_failed");
        assert!(matches!(
            error.io_kind.as_deref(),
            Some("timedout") | Some("wouldblock")
        ));

        let error = probe_plain_http(PlainHttpProbeRequest {
            url: Url::parse("https://example.test/").unwrap(),
            http_host: None,
            connection_ip: None,
            limits: limits(),
        })
        .unwrap_err();
        assert_eq!(error.code, "tls_transport_required");
    }
}

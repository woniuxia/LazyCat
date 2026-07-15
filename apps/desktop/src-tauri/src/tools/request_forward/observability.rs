#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use hyper::http::{HeaderMap, HeaderValue};

    use super::{redact_headers, should_capture_body, PreviewTap, HTTP_BODY_PREVIEW_LIMIT};

    #[test]
    fn redacts_all_sensitive_headers_case_insensitively() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("Bearer secret"));
        headers.insert(
            "proxy-authorization",
            HeaderValue::from_static("Basic secret"),
        );
        headers.insert("cookie", HeaderValue::from_static("session=secret"));
        headers.insert("set-cookie", HeaderValue::from_static("session=secret"));
        headers.insert("x-request-id", HeaderValue::from_static("safe"));

        let redacted = redact_headers(&headers)
            .into_iter()
            .collect::<BTreeMap<_, _>>();

        for name in [
            "authorization",
            "proxy-authorization",
            "cookie",
            "set-cookie",
        ] {
            assert_eq!(redacted[name], "[REDACTED]");
        }
        assert_eq!(redacted["x-request-id"], "safe");
    }

    #[test]
    fn captures_only_textual_or_structured_identity_bodies() {
        for content_type in [
            "text/plain",
            "application/json; charset=utf-8",
            "application/problem+json",
            "application/xml",
            "application/custom+xml",
            "application/x-www-form-urlencoded",
        ] {
            let mut headers = HeaderMap::new();
            headers.insert("content-type", HeaderValue::from_str(content_type).unwrap());
            assert!(
                should_capture_body(&headers),
                "{content_type} should be captured"
            );
        }

        let mut binary = HeaderMap::new();
        binary.insert("content-type", HeaderValue::from_static("image/png"));
        assert!(!should_capture_body(&binary));

        let mut compressed = HeaderMap::new();
        compressed.insert("content-type", HeaderValue::from_static("application/json"));
        compressed.insert("content-encoding", HeaderValue::from_static("gzip"));
        assert!(!should_capture_body(&compressed));
    }

    #[test]
    fn preview_tap_keeps_exact_cap_without_truncation_and_passes_through_chunks() {
        let body = vec![b'a'; HTTP_BODY_PREVIEW_LIMIT];
        let mut tap = PreviewTap::new();

        assert_eq!(tap.observe(&body), body.as_slice());
        let preview = tap.preview();
        assert_eq!(preview.bytes.len(), HTTP_BODY_PREVIEW_LIMIT);
        assert!(!preview.truncated);
    }

    #[test]
    fn preview_tap_marks_truncation_after_cap_without_limiting_observed_chunk() {
        let body = vec![b'b'; HTTP_BODY_PREVIEW_LIMIT + 1024];
        let mut tap = PreviewTap::new();

        assert_eq!(tap.observe(&body), body.as_slice());
        let preview = tap.preview();
        assert_eq!(preview.bytes.len(), HTTP_BODY_PREVIEW_LIMIT);
        assert!(preview.truncated);
    }
}

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Condvar, Mutex};
#[cfg(test)]
use std::time::{Duration, Instant};

use hyper::http::HeaderMap;

pub(crate) const HTTP_BODY_PREVIEW_LIMIT: usize = 64 * 1024;

const REDACTED_VALUE: &str = "[REDACTED]";
const SENSITIVE_HEADERS: [&str; 4] = [
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
];

const TCP_EVENT_BUFFER_LIMIT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TcpEventKind {
    Accepted,
    DownstreamConnectFailed,
    Overloaded,
    RelayFailed,
    ListenerFailed,
    ChildTaskFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TcpEvent {
    pub(crate) kind: TcpEventKind,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TcpObservationSnapshot {
    pub(crate) event_count: u64,
    pub(crate) upload_bytes: u64,
    pub(crate) download_bytes: u64,
    pub(crate) error_count: u64,
    pub(crate) events: Vec<TcpEvent>,
}

#[derive(Default)]
struct TcpObservationState {
    event_count: u64,
    upload_bytes: u64,
    download_bytes: u64,
    error_count: u64,
    events: VecDeque<TcpEvent>,
}

#[derive(Default)]
pub(crate) struct TcpObservability {
    state: Mutex<TcpObservationState>,
    changed: Condvar,
}

impl TcpObservability {
    pub(crate) fn accepted(&self) {
        self.update(|state| {
            state.event_count = state.event_count.saturating_add(1);
            push_tcp_event(state, TcpEventKind::Accepted, None);
        });
    }

    pub(crate) fn downstream_connect_failed(&self, error: String) {
        self.failed(TcpEventKind::DownstreamConnectFailed, error);
    }

    pub(crate) fn overloaded(&self, error: String) {
        self.failed(TcpEventKind::Overloaded, error);
    }

    pub(crate) fn relay_failed(&self, error: String) {
        self.failed(TcpEventKind::RelayFailed, error);
    }

    pub(crate) fn listener_failed(&self, error: String) {
        self.failed(TcpEventKind::ListenerFailed, error);
    }

    pub(crate) fn child_task_failed(&self, error: String) {
        self.failed(TcpEventKind::ChildTaskFailed, error);
    }

    pub(crate) fn transferred(&self, upload_bytes: u64, download_bytes: u64) {
        self.update(|state| {
            state.upload_bytes = state.upload_bytes.saturating_add(upload_bytes);
            state.download_bytes = state.download_bytes.saturating_add(download_bytes);
        });
    }

    pub(crate) fn snapshot(&self) -> TcpObservationSnapshot {
        let state = self.state.lock().expect("TCP observability lock poisoned");
        snapshot_tcp_state(&state)
    }

    #[cfg(test)]
    pub(crate) fn wait_for(
        &self,
        timeout: Duration,
        predicate: impl Fn(&TcpObservationSnapshot) -> bool,
    ) -> Option<TcpObservationSnapshot> {
        let deadline = Instant::now() + timeout;
        let mut state = self.state.lock().expect("TCP observability lock poisoned");
        loop {
            let snapshot = snapshot_tcp_state(&state);
            if predicate(&snapshot) {
                return Some(snapshot);
            }

            let remaining = deadline.checked_duration_since(Instant::now())?;
            let (next_state, timeout_result) = self
                .changed
                .wait_timeout(state, remaining)
                .expect("TCP observability lock poisoned");
            state = next_state;
            if timeout_result.timed_out() {
                let snapshot = snapshot_tcp_state(&state);
                return predicate(&snapshot).then_some(snapshot);
            }
        }
    }

    fn failed(&self, event: TcpEventKind, error: String) {
        self.update(|state| {
            state.error_count = state.error_count.saturating_add(1);
            push_tcp_event(state, event, Some(error));
        });
    }

    fn update(&self, update: impl FnOnce(&mut TcpObservationState)) {
        let mut state = self.state.lock().expect("TCP observability lock poisoned");
        update(&mut state);
        self.changed.notify_all();
    }
}

fn push_tcp_event(state: &mut TcpObservationState, kind: TcpEventKind, error: Option<String>) {
    if state.events.len() == TCP_EVENT_BUFFER_LIMIT {
        state.events.pop_front();
    }
    state.events.push_back(TcpEvent { kind, error });
}

fn snapshot_tcp_state(state: &TcpObservationState) -> TcpObservationSnapshot {
    TcpObservationSnapshot {
        event_count: state.event_count,
        upload_bytes: state.upload_bytes,
        download_bytes: state.download_bytes,
        error_count: state.error_count,
        events: state.events.iter().cloned().collect(),
    }
}

const UDP_EVENT_BUFFER_LIMIT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UdpEventKind {
    ClientDatagram,
    SessionCreated,
    DownstreamConnectFailed,
    DownstreamSendFailed,
    DownstreamReceiveFailed,
    ClientSendFailed,
    Overloaded,
    SessionExpired,
    ListenerFailed,
    ChildTaskFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UdpEvent {
    pub(crate) kind: UdpEventKind,
    pub(crate) client_addr: Option<SocketAddr>,
    pub(crate) target: String,
    pub(crate) target_addr: Option<SocketAddr>,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UdpObservationSnapshot {
    pub(crate) event_count: u64,
    pub(crate) upload_bytes: u64,
    pub(crate) download_bytes: u64,
    pub(crate) error_count: u64,
    pub(crate) events: Vec<UdpEvent>,
}

#[derive(Default)]
struct UdpObservationState {
    event_count: u64,
    upload_bytes: u64,
    download_bytes: u64,
    error_count: u64,
    events: VecDeque<UdpEvent>,
}

#[derive(Default)]
pub(crate) struct UdpObservability {
    state: Mutex<UdpObservationState>,
    changed: Condvar,
}

impl UdpObservability {
    pub(crate) fn client_datagram(
        &self,
        client_addr: SocketAddr,
        target: &str,
        target_addr: Option<SocketAddr>,
    ) {
        self.update(|state| {
            state.event_count = state.event_count.saturating_add(1);
            push_udp_event(
                state,
                UdpEventKind::ClientDatagram,
                Some(client_addr),
                target,
                target_addr,
                None,
            );
        });
    }

    pub(crate) fn session_created(
        &self,
        client_addr: SocketAddr,
        target: &str,
        target_addr: SocketAddr,
    ) {
        self.event(
            UdpEventKind::SessionCreated,
            Some(client_addr),
            target,
            Some(target_addr),
            None,
        );
    }

    pub(crate) fn downstream_connect_failed(
        &self,
        client_addr: SocketAddr,
        target: &str,
        target_addr: Option<SocketAddr>,
        error: String,
    ) {
        self.failed(
            UdpEventKind::DownstreamConnectFailed,
            Some(client_addr),
            target,
            target_addr,
            error,
        );
    }

    pub(crate) fn downstream_send_failed(
        &self,
        client_addr: SocketAddr,
        target: &str,
        target_addr: SocketAddr,
        error: String,
    ) {
        self.failed(
            UdpEventKind::DownstreamSendFailed,
            Some(client_addr),
            target,
            Some(target_addr),
            error,
        );
    }

    pub(crate) fn downstream_receive_failed(
        &self,
        client_addr: SocketAddr,
        target: &str,
        target_addr: SocketAddr,
        error: String,
    ) {
        self.failed(
            UdpEventKind::DownstreamReceiveFailed,
            Some(client_addr),
            target,
            Some(target_addr),
            error,
        );
    }

    pub(crate) fn client_send_failed(
        &self,
        client_addr: SocketAddr,
        target: &str,
        target_addr: SocketAddr,
        error: String,
    ) {
        self.failed(
            UdpEventKind::ClientSendFailed,
            Some(client_addr),
            target,
            Some(target_addr),
            error,
        );
    }

    pub(crate) fn overloaded(
        &self,
        client_addr: SocketAddr,
        target: &str,
        target_addr: Option<SocketAddr>,
        error: String,
    ) {
        self.failed(
            UdpEventKind::Overloaded,
            Some(client_addr),
            target,
            target_addr,
            error,
        );
    }

    pub(crate) fn session_expired(
        &self,
        client_addr: SocketAddr,
        target: &str,
        target_addr: SocketAddr,
    ) {
        self.event(
            UdpEventKind::SessionExpired,
            Some(client_addr),
            target,
            Some(target_addr),
            None,
        );
    }

    pub(crate) fn listener_failed(&self, target: &str, error: String) {
        self.failed(UdpEventKind::ListenerFailed, None, target, None, error);
    }

    pub(crate) fn child_task_failed(&self, target: &str, error: String) {
        self.failed(UdpEventKind::ChildTaskFailed, None, target, None, error);
    }

    pub(crate) fn transferred(&self, upload_bytes: u64, download_bytes: u64) {
        self.update(|state| {
            state.upload_bytes = state.upload_bytes.saturating_add(upload_bytes);
            state.download_bytes = state.download_bytes.saturating_add(download_bytes);
        });
    }

    #[cfg(test)]
    pub(crate) fn wait_for(
        &self,
        timeout: Duration,
        predicate: impl Fn(&UdpObservationSnapshot) -> bool,
    ) -> Option<UdpObservationSnapshot> {
        let deadline = Instant::now() + timeout;
        let mut state = self.state.lock().expect("UDP observability lock poisoned");
        loop {
            let snapshot = snapshot_udp_state(&state);
            if predicate(&snapshot) {
                return Some(snapshot);
            }

            let remaining = deadline.checked_duration_since(Instant::now())?;
            let (next_state, timeout_result) = self
                .changed
                .wait_timeout(state, remaining)
                .expect("UDP observability lock poisoned");
            state = next_state;
            if timeout_result.timed_out() {
                let snapshot = snapshot_udp_state(&state);
                return predicate(&snapshot).then_some(snapshot);
            }
        }
    }

    fn failed(
        &self,
        kind: UdpEventKind,
        client_addr: Option<SocketAddr>,
        target: &str,
        target_addr: Option<SocketAddr>,
        error: String,
    ) {
        self.update(|state| {
            state.error_count = state.error_count.saturating_add(1);
            push_udp_event(state, kind, client_addr, target, target_addr, Some(error));
        });
    }

    fn event(
        &self,
        kind: UdpEventKind,
        client_addr: Option<SocketAddr>,
        target: &str,
        target_addr: Option<SocketAddr>,
        error: Option<String>,
    ) {
        self.update(|state| {
            push_udp_event(state, kind, client_addr, target, target_addr, error);
        });
    }

    fn update(&self, update: impl FnOnce(&mut UdpObservationState)) {
        let mut state = self.state.lock().expect("UDP observability lock poisoned");
        update(&mut state);
        self.changed.notify_all();
    }
}

fn push_udp_event(
    state: &mut UdpObservationState,
    kind: UdpEventKind,
    client_addr: Option<SocketAddr>,
    target: &str,
    target_addr: Option<SocketAddr>,
    error: Option<String>,
) {
    if state.events.len() == UDP_EVENT_BUFFER_LIMIT {
        state.events.pop_front();
    }
    state.events.push_back(UdpEvent {
        kind,
        client_addr,
        target: target.to_string(),
        target_addr,
        error,
    });
}

fn snapshot_udp_state(state: &UdpObservationState) -> UdpObservationSnapshot {
    UdpObservationSnapshot {
        event_count: state.event_count,
        upload_bytes: state.upload_bytes,
        download_bytes: state.download_bytes,
        error_count: state.error_count,
        events: state.events.iter().cloned().collect(),
    }
}

const HTTP_EVENT_BUFFER_LIMIT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HttpEventKind {
    Accepted,
    DownstreamFailed,
    ResponseTimeout,
    Overloaded,
    UpgradeRejected,
    ResponseStreamFailed,
    ListenerFailed,
    ChildTaskFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HttpEvent {
    pub(crate) kind: HttpEventKind,
    pub(crate) error: Option<String>,
    pub(crate) request_headers: Option<Vec<(String, String)>>,
    pub(crate) response_headers: Option<Vec<(String, String)>>,
    pub(crate) request_body_preview: Option<BodyPreview>,
    pub(crate) response_body_preview: Option<BodyPreview>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HttpObservationSnapshot {
    pub(crate) event_count: u64,
    pub(crate) upload_bytes: u64,
    pub(crate) download_bytes: u64,
    pub(crate) error_count: u64,
    pub(crate) events: Vec<HttpEvent>,
}

struct HttpEventRecord {
    kind: HttpEventKind,
    error: Option<String>,
    request_headers: Option<Vec<(String, String)>>,
    response_headers: Mutex<Option<Vec<(String, String)>>>,
    request_body_preview: Option<Arc<Mutex<PreviewTap>>>,
    response_body_preview: Mutex<Option<Arc<Mutex<PreviewTap>>>>,
}

impl HttpEventRecord {
    fn snapshot(&self) -> HttpEvent {
        let preview = |tap: &Option<Arc<Mutex<PreviewTap>>>| {
            tap.as_ref()
                .map(|tap| tap.lock().expect("HTTP preview lock poisoned").preview())
        };
        HttpEvent {
            kind: self.kind,
            error: self.error.clone(),
            request_headers: self.request_headers.clone(),
            response_headers: self
                .response_headers
                .lock()
                .expect("HTTP response headers lock poisoned")
                .clone(),
            request_body_preview: preview(&self.request_body_preview),
            response_body_preview: preview(
                &self
                    .response_body_preview
                    .lock()
                    .expect("HTTP response preview lock poisoned"),
            ),
        }
    }
}

#[derive(Default)]
struct HttpObservationState {
    event_count: u64,
    upload_bytes: u64,
    download_bytes: u64,
    error_count: u64,
    events: VecDeque<Arc<HttpEventRecord>>,
}

#[derive(Default)]
pub(crate) struct HttpObservability {
    state: Mutex<HttpObservationState>,
    changed: Condvar,
}

pub(crate) struct HttpRequestTrace {
    observability: Arc<HttpObservability>,
    record: Arc<HttpEventRecord>,
    capture_response_headers: bool,
    capture_response_body: bool,
}

impl HttpObservability {
    pub(crate) fn accepted(
        self: &Arc<Self>,
        request_headers: &HeaderMap,
        capture_headers: bool,
        capture_body: bool,
    ) -> Arc<HttpRequestTrace> {
        let request_body_preview = (capture_body && should_capture_body(request_headers))
            .then(|| Arc::new(Mutex::new(PreviewTap::new())));
        let record = Arc::new(HttpEventRecord {
            kind: HttpEventKind::Accepted,
            error: None,
            request_headers: capture_headers.then(|| redact_headers(request_headers)),
            response_headers: Mutex::new(None),
            request_body_preview,
            response_body_preview: Mutex::new(None),
        });
        self.update(|state| {
            state.event_count = state.event_count.saturating_add(1);
            push_http_event(state, Arc::clone(&record));
        });
        Arc::new(HttpRequestTrace {
            observability: Arc::clone(self),
            record,
            capture_response_headers: capture_headers,
            capture_response_body: capture_body,
        })
    }

    pub(crate) fn uploaded(&self, bytes: usize) {
        self.update(|state| {
            state.upload_bytes = state.upload_bytes.saturating_add(bytes as u64);
        });
    }

    pub(crate) fn downloaded(&self, bytes: usize) {
        self.update(|state| {
            state.download_bytes = state.download_bytes.saturating_add(bytes as u64);
        });
    }

    pub(crate) fn downstream_failed(&self, error: String) {
        self.failed(HttpEventKind::DownstreamFailed, error);
    }

    pub(crate) fn response_timeout(&self, error: String) {
        self.failed(HttpEventKind::ResponseTimeout, error);
    }

    pub(crate) fn overloaded(&self, error: String) {
        self.failed(HttpEventKind::Overloaded, error);
    }

    pub(crate) fn upgrade_rejected(&self, error: String) {
        self.failed(HttpEventKind::UpgradeRejected, error);
    }

    pub(crate) fn response_stream_failed(&self, error: String) {
        self.failed(HttpEventKind::ResponseStreamFailed, error);
    }

    pub(crate) fn listener_failed(&self, error: String) {
        self.failed(HttpEventKind::ListenerFailed, error);
    }

    pub(crate) fn child_task_failed(&self, error: String) {
        self.failed(HttpEventKind::ChildTaskFailed, error);
    }

    pub(crate) fn snapshot(&self) -> HttpObservationSnapshot {
        let state = self.state.lock().expect("HTTP observability lock poisoned");
        HttpObservationSnapshot {
            event_count: state.event_count,
            upload_bytes: state.upload_bytes,
            download_bytes: state.download_bytes,
            error_count: state.error_count,
            events: state.events.iter().map(|event| event.snapshot()).collect(),
        }
    }

    #[cfg(test)]
    pub(crate) fn wait_for(
        &self,
        timeout: Duration,
        predicate: impl Fn(&HttpObservationSnapshot) -> bool,
    ) -> Option<HttpObservationSnapshot> {
        let deadline = Instant::now() + timeout;
        let mut state = self.state.lock().expect("HTTP observability lock poisoned");
        loop {
            let snapshot = snapshot_http_state(&state);
            if predicate(&snapshot) {
                return Some(snapshot);
            }

            let remaining = deadline.checked_duration_since(Instant::now())?;
            let (next_state, timeout_result) = self
                .changed
                .wait_timeout(state, remaining)
                .expect("HTTP observability lock poisoned");
            state = next_state;
            if timeout_result.timed_out() {
                let snapshot = snapshot_http_state(&state);
                return predicate(&snapshot).then_some(snapshot);
            }
        }
    }

    fn failed(&self, kind: HttpEventKind, error: String) {
        self.update(|state| {
            state.error_count = state.error_count.saturating_add(1);
            push_http_event(
                state,
                Arc::new(HttpEventRecord {
                    kind,
                    error: Some(error),
                    request_headers: None,
                    response_headers: Mutex::new(None),
                    request_body_preview: None,
                    response_body_preview: Mutex::new(None),
                }),
            );
        });
    }

    fn update(&self, update: impl FnOnce(&mut HttpObservationState)) {
        let mut state = self.state.lock().expect("HTTP observability lock poisoned");
        update(&mut state);
        self.changed.notify_all();
    }
}

impl HttpRequestTrace {
    pub(crate) fn observe_request(&self, chunk: &[u8]) {
        if let Some(tap) = &self.record.request_body_preview {
            tap.lock()
                .expect("HTTP request preview lock poisoned")
                .observe(chunk);
        }
    }

    pub(crate) fn response_started(&self, headers: &HeaderMap) {
        if self.capture_response_headers {
            *self
                .record
                .response_headers
                .lock()
                .expect("HTTP response headers lock poisoned") = Some(redact_headers(headers));
        }
        if self.capture_response_body && should_capture_body(headers) {
            *self
                .record
                .response_body_preview
                .lock()
                .expect("HTTP response preview lock poisoned") =
                Some(Arc::new(Mutex::new(PreviewTap::new())));
        }
    }

    pub(crate) fn observe_response(&self, chunk: &[u8]) {
        let tap = self
            .record
            .response_body_preview
            .lock()
            .expect("HTTP response preview lock poisoned")
            .clone();
        if let Some(tap) = tap {
            tap.lock()
                .expect("HTTP response preview lock poisoned")
                .observe(chunk);
        }
    }

    pub(crate) fn uploaded(&self, bytes: usize) {
        self.observability.uploaded(bytes);
    }

    pub(crate) fn downloaded(&self, bytes: usize) {
        self.observability.downloaded(bytes);
    }

    pub(crate) fn response_stream_failed(&self, error: String) {
        self.observability.response_stream_failed(error);
    }
}

fn push_http_event(state: &mut HttpObservationState, event: Arc<HttpEventRecord>) {
    if state.events.len() == HTTP_EVENT_BUFFER_LIMIT {
        state.events.pop_front();
    }
    state.events.push_back(event);
}

fn snapshot_http_state(state: &HttpObservationState) -> HttpObservationSnapshot {
    HttpObservationSnapshot {
        event_count: state.event_count,
        upload_bytes: state.upload_bytes,
        download_bytes: state.download_bytes,
        error_count: state.error_count,
        events: state.events.iter().map(|event| event.snapshot()).collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BodyPreview {
    pub(crate) bytes: Vec<u8>,
    pub(crate) truncated: bool,
}

#[derive(Debug, Default)]
pub(crate) struct PreviewTap {
    bytes: Vec<u8>,
    truncated: bool,
}

impl PreviewTap {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn observe<'a>(&mut self, chunk: &'a [u8]) -> &'a [u8] {
        let remaining = HTTP_BODY_PREVIEW_LIMIT.saturating_sub(self.bytes.len());
        let captured_len = remaining.min(chunk.len());
        self.bytes.extend_from_slice(&chunk[..captured_len]);
        if captured_len < chunk.len() {
            self.truncated = true;
        }
        chunk
    }

    pub(crate) fn preview(&self) -> BodyPreview {
        BodyPreview {
            bytes: self.bytes.clone(),
            truncated: self.truncated,
        }
    }
}

pub(crate) fn redact_headers(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| {
            let value = if SENSITIVE_HEADERS
                .iter()
                .any(|sensitive| name.as_str().eq_ignore_ascii_case(sensitive))
            {
                REDACTED_VALUE.to_string()
            } else {
                String::from_utf8_lossy(value.as_bytes()).into_owned()
            };
            (name.as_str().to_string(), value)
        })
        .collect()
}

pub(crate) fn should_capture_body(headers: &HeaderMap) -> bool {
    content_encoding_is_identity(headers)
        && headers
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .is_some_and(is_textual_or_structured_content_type)
}

fn content_encoding_is_identity(headers: &HeaderMap) -> bool {
    headers.get_all("content-encoding").iter().all(|value| {
        value.to_str().is_ok_and(|value| {
            value
                .split(',')
                .all(|encoding| encoding.trim().eq_ignore_ascii_case("identity"))
        })
    })
}

fn is_textual_or_structured_content_type(value: &str) -> bool {
    let media_type = value
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    media_type.starts_with("text/")
        || matches!(
            media_type.as_str(),
            "application/json" | "application/xml" | "application/x-www-form-urlencoded"
        )
        || media_type.ends_with("+json")
        || media_type.ends_with("+xml")
}

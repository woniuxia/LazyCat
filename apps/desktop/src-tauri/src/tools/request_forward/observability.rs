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
use std::sync::{Condvar, Mutex};
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

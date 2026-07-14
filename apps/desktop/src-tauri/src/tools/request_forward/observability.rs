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

use hyper::http::HeaderMap;

pub(crate) const HTTP_BODY_PREVIEW_LIMIT: usize = 64 * 1024;

const REDACTED_VALUE: &str = "[REDACTED]";
const SENSITIVE_HEADERS: [&str; 4] = [
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
];

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

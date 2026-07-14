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

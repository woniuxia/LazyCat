use serde_json::{json, Value};
use std::time::Instant;
use std::net::IpAddr;
use std::str::FromStr;
use std::collections::HashSet;
use std::sync::OnceLock;
use hickory_resolver::config::{ResolverConfig, ResolverOpts, NameServerConfig, Protocol};
use hickory_resolver::TokioAsyncResolver;
use hickory_resolver::proto::rr::RecordType;

static DNS_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn get_runtime() -> &'static tokio::runtime::Runtime {
    DNS_RUNTIME.get_or_init(|| {
        tokio::runtime::Runtime::new().expect("create DNS runtime")
    })
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "resolve" => resolve(payload),
        "system_dns" => system_dns(),
        "compare" => compare(payload),
        _ => Err(format!("unsupported dns action: {action}")),
    }
}

fn system_dns() -> Result<Value, String> {
    let config = ResolverConfig::default();
    let mut ipv4 = Vec::<String>::new();
    let mut all = Vec::<String>::new();
    let mut seen = HashSet::<String>::new();

    for ns in config.name_servers() {
        let ip = ns.socket_addr.ip().to_string();
        if seen.insert(ip.clone()) {
            if ns.socket_addr.ip().is_ipv4() {
                ipv4.push(ip.clone());
            }
            all.push(ip);
        }
    }

    Ok(json!({
        "ipv4": ipv4,
        "all": all,
    }))
}

/// 将 IPv4 地址转为 PTR 查询名，如 8.8.8.8 -> 8.8.8.8.in-addr.arpa.
fn ipv4_to_ptr(addr: &std::net::Ipv4Addr) -> String {
    let octets = addr.octets();
    format!("{}.{}.{}.{}.in-addr.arpa.", octets[3], octets[2], octets[1], octets[0])
}

/// 将 IPv6 地址转为 PTR 查询名（ip6.arpa.）
fn ipv6_to_ptr(addr: &std::net::Ipv6Addr) -> String {
    let octets = addr.octets();
    let dotted = octets.iter()
        .flat_map(|b| [b >> 4, b & 0xf])
        .rev()
        .map(|n| char::from_digit(n as u32, 16).expect("nibble 0-15 is always valid hex"))
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(".");
    format!("{}.ip6.arpa.", dotted)
}

fn resolve(payload: &Value) -> Result<Value, String> {
    let domain = payload["domain"]
        .as_str()
        .unwrap_or("")
        .trim();
    if domain.is_empty() {
        return Err("domain is required".to_string());
    }

    let server = payload["server"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();

    let started = Instant::now();
    let rt = get_runtime();

    // 判断输入是否为 IP，若是则构造 PTR 查询名
    let ptr_name = match IpAddr::from_str(domain) {
        Ok(IpAddr::V4(v4)) => ipv4_to_ptr(&v4),
        Ok(IpAddr::V6(v6)) => ipv6_to_ptr(&v6),
        Err(_) => String::new(),
    };

    let server_display = if server.is_empty() {
        "system".to_string()
    } else {
        server.clone()
    };

    let domain_owned = domain.to_string();
    let result = rt.block_on(async move {
        let resolver = if server.is_empty() {
            TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default())
        } else {
            let ip: IpAddr = IpAddr::from_str(&server)
                .map_err(|e| format!("invalid DNS server address: {e}"))?;
            let ns = NameServerConfig::new(std::net::SocketAddr::new(ip, 53), Protocol::Udp);
            let mut config = ResolverConfig::new();
            config.add_name_server(ns);
            TokioAsyncResolver::tokio(config, ResolverOpts::default())
        };

        let domain_name = format!("{}.", domain_owned.trim_end_matches('.'));

        let (a_records, aaaa_records, cname_records, mx_records, ns_records, txt_records, soa_records, srv_records, ptr_records) = tokio::join!(
            query_a(&resolver, &domain_name),
            query_aaaa(&resolver, &domain_name),
            query_cname(&resolver, &domain_name),
            query_mx(&resolver, &domain_name),
            query_ns(&resolver, &domain_name),
            query_txt(&resolver, &domain_name),
            query_soa(&resolver, &domain_name),
            query_srv(&resolver, &domain_name),
            query_ptr(&resolver, &ptr_name),
        );

        Ok::<Value, String>(json!({
            "A": a_records,
            "AAAA": aaaa_records,
            "CNAME": cname_records,
            "MX": mx_records,
            "NS": ns_records,
            "TXT": txt_records,
            "SOA": soa_records,
            "SRV": srv_records,
            "PTR": ptr_records,
        }))
    })?;

    Ok(json!({
        "domain": domain,
        "server": server_display,
        "records": result,
        "elapsed_ms": started.elapsed().as_millis() as u64,
    }))
}

fn compare(payload: &Value) -> Result<Value, String> {
    let domain = payload["domain"]
        .as_str()
        .unwrap_or("")
        .trim();
    if domain.is_empty() {
        return Err("domain is required".to_string());
    }

    let servers: Vec<String> = match payload["servers"].as_array() {
        Some(arr) if !arr.is_empty() => arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        _ => return Err("servers must be a non-empty array".to_string()),
    };

    let rt = get_runtime();
    let domain_owned = domain.to_string();

    let mut results: Vec<Value> = rt.block_on(async move {
        let mut set = tokio::task::JoinSet::new();

        for server_ip in servers {
            let d = domain_owned.clone();
            let s = server_ip.clone();
            set.spawn(async move {
                let started = Instant::now();
                let display = if s.is_empty() { "系统".to_string() } else { s.clone() };

                let resolver_result: Result<TokioAsyncResolver, String> = if s.is_empty() {
                    Ok(TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default()))
                } else {
                    IpAddr::from_str(&s)
                        .map_err(|e| format!("invalid server: {e}"))
                        .map(|ip| {
                            let ns = NameServerConfig::new(std::net::SocketAddr::new(ip, 53), Protocol::Udp);
                            let mut cfg = ResolverConfig::new();
                            cfg.add_name_server(ns);
                            TokioAsyncResolver::tokio(cfg, ResolverOpts::default())
                        })
                };

                match resolver_result {
                    Err(e) => json!({
                        "server": display,
                        "ip": s,
                        "elapsed_ms": serde_json::Value::Null,
                        "addresses": [],
                        "error": e,
                    }),
                    Ok(resolver) => {
                        let domain_name = format!("{}.", d.trim_end_matches('.'));
                        match resolver.lookup(&domain_name, RecordType::A).await {
                            Ok(lookup) => {
                                let addrs: Vec<String> = lookup.record_iter()
                                    .filter_map(|r| r.data().and_then(|d| d.as_a()).map(|a| a.0.to_string()))
                                    .collect();
                                let elapsed = started.elapsed().as_millis() as u64;
                                json!({
                                    "server": display,
                                    "ip": s,
                                    "elapsed_ms": elapsed,
                                    "addresses": addrs,
                                    "error": serde_json::Value::Null,
                                })
                            },
                            Err(e) => json!({
                                "server": display,
                                "ip": s,
                                "elapsed_ms": serde_json::Value::Null,
                                "addresses": [],
                                "error": e.to_string(),
                            }),
                        }
                    }
                }
            });
        }

        let mut out = vec![];
        while let Some(r) = set.join_next().await {
            match r {
                Ok(v) => out.push(v),
                Err(e) => out.push(json!({
                    "server": "unknown",
                    "ip": "",
                    "elapsed_ms": serde_json::Value::Null,
                    "addresses": [],
                    "error": format!("task error: {e}"),
                })),
            }
        }
        out
    });

    // 按 elapsed_ms 升序，error（Null）排最后
    results.sort_by_key(|v| {
        v["elapsed_ms"].as_u64().unwrap_or(u64::MAX)
    });

    Ok(serde_json::Value::Array(results))
}

async fn query_a(resolver: &TokioAsyncResolver, name: &str) -> Value {
    match resolver.lookup(name, RecordType::A).await {
        Ok(lookup) => {
            let records: Vec<Value> = lookup.record_iter()
                .filter_map(|r| {
                    if let Some(data) = r.data() {
                        if let Some(a) = data.as_a() {
                            return Some(json!({
                                "address": a.0.to_string(),
                                "ttl": r.ttl(),
                            }));
                        }
                    }
                    None
                })
                .collect();
            json!(records)
        }
        Err(_) => json!([]),
    }
}

async fn query_aaaa(resolver: &TokioAsyncResolver, name: &str) -> Value {
    match resolver.lookup(name, RecordType::AAAA).await {
        Ok(lookup) => {
            let records: Vec<Value> = lookup.record_iter()
                .filter_map(|r| {
                    if let Some(data) = r.data() {
                        if let Some(aaaa) = data.as_aaaa() {
                            return Some(json!({
                                "address": aaaa.0.to_string(),
                                "ttl": r.ttl(),
                            }));
                        }
                    }
                    None
                })
                .collect();
            json!(records)
        }
        Err(_) => json!([]),
    }
}

async fn query_cname(resolver: &TokioAsyncResolver, name: &str) -> Value {
    match resolver.lookup(name, RecordType::CNAME).await {
        Ok(lookup) => {
            let records: Vec<Value> = lookup.record_iter()
                .filter_map(|r| {
                    if let Some(data) = r.data() {
                        if let Some(cname) = data.as_cname() {
                            return Some(json!({
                                "target": cname.0.to_string(),
                                "ttl": r.ttl(),
                            }));
                        }
                    }
                    None
                })
                .collect();
            json!(records)
        }
        Err(_) => json!([]),
    }
}

async fn query_mx(resolver: &TokioAsyncResolver, name: &str) -> Value {
    match resolver.lookup(name, RecordType::MX).await {
        Ok(lookup) => {
            let records: Vec<Value> = lookup.record_iter()
                .filter_map(|r| {
                    if let Some(data) = r.data() {
                        if let Some(mx) = data.as_mx() {
                            return Some(json!({
                                "preference": mx.preference(),
                                "exchange": mx.exchange().to_string(),
                                "ttl": r.ttl(),
                            }));
                        }
                    }
                    None
                })
                .collect();
            json!(records)
        }
        Err(_) => json!([]),
    }
}

async fn query_ns(resolver: &TokioAsyncResolver, name: &str) -> Value {
    match resolver.lookup(name, RecordType::NS).await {
        Ok(lookup) => {
            let records: Vec<Value> = lookup.record_iter()
                .filter_map(|r| {
                    if let Some(data) = r.data() {
                        if let Some(ns) = data.as_ns() {
                            return Some(json!({
                                "host": ns.0.to_string(),
                                "ttl": r.ttl(),
                            }));
                        }
                    }
                    None
                })
                .collect();
            json!(records)
        }
        Err(_) => json!([]),
    }
}

async fn query_txt(resolver: &TokioAsyncResolver, name: &str) -> Value {
    match resolver.lookup(name, RecordType::TXT).await {
        Ok(lookup) => {
            let records: Vec<Value> = lookup.record_iter()
                .filter_map(|r| {
                    if let Some(data) = r.data() {
                        if let Some(txt) = data.as_txt() {
                            let text = txt.iter()
                                .map(|bytes| String::from_utf8_lossy(bytes).to_string())
                                .collect::<Vec<_>>()
                                .join("");
                            return Some(json!({
                                "text": text,
                                "ttl": r.ttl(),
                            }));
                        }
                    }
                    None
                })
                .collect();
            json!(records)
        }
        Err(_) => json!([]),
    }
}

async fn query_soa(resolver: &TokioAsyncResolver, name: &str) -> Value {
    match resolver.lookup(name, RecordType::SOA).await {
        Ok(lookup) => {
            let records: Vec<Value> = lookup.record_iter()
                .filter_map(|r| {
                    if let Some(data) = r.data() {
                        if let Some(soa) = data.as_soa() {
                            return Some(json!({
                                "mname": soa.mname().to_string(),
                                "rname": soa.rname().to_string(),
                                "serial": soa.serial(),
                                "refresh": soa.refresh(),
                                "retry": soa.retry(),
                                "expire": soa.expire(),
                                "minimum": soa.minimum(),
                                "ttl": r.ttl(),
                            }));
                        }
                    }
                    None
                })
                .collect();
            json!(records)
        }
        Err(_) => json!([]),
    }
}

async fn query_srv(resolver: &TokioAsyncResolver, name: &str) -> Value {
    match resolver.lookup(name, RecordType::SRV).await {
        Ok(lookup) => {
            let records: Vec<Value> = lookup.record_iter()
                .filter_map(|r| {
                    if let Some(data) = r.data() {
                        if let Some(srv) = data.as_srv() {
                            return Some(json!({
                                "priority": srv.priority(),
                                "weight": srv.weight(),
                                "port": srv.port(),
                                "target": srv.target().to_string(),
                                "ttl": r.ttl(),
                            }));
                        }
                    }
                    None
                })
                .collect();
            json!(records)
        }
        Err(_) => json!([]),
    }
}

async fn query_ptr(resolver: &TokioAsyncResolver, ptr_name: &str) -> Value {
    if ptr_name.is_empty() {
        return json!([]);
    }
    match resolver.lookup(ptr_name, RecordType::PTR).await {
        Ok(lookup) => {
            let records: Vec<Value> = lookup.record_iter()
                .filter_map(|r| {
                    if let Some(data) = r.data() {
                        if let Some(ptr) = data.as_ptr() {
                            return Some(json!({
                                "hostname": ptr.0.to_string(),
                                "ttl": r.ttl(),
                            }));
                        }
                    }
                    None
                })
                .collect();
            json!(records)
        }
        Err(_) => json!([]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn system_dns_should_return_arrays() {
        let out = execute("system_dns", &json!({})).expect("system dns");
        assert!(out["ipv4"].is_array());
        assert!(out["all"].is_array());
    }

    #[test]
    fn resolve_empty_domain_should_fail() {
        let err = execute("resolve", &json!({ "domain": "" })).expect_err("must fail");
        assert!(err.contains("domain is required"));
    }

    #[test]
    fn resolve_invalid_server_should_fail() {
        let err = execute(
            "resolve",
            &json!({ "domain": "example.com", "server": "not-an-ip" }),
        )
        .expect_err("must fail");
        assert!(err.contains("invalid DNS server address"));
    }

    #[test]
    fn compare_empty_domain_should_fail() {
        let err = execute("compare", &json!({ "domain": "", "servers": [""] }))
            .expect_err("must fail");
        assert!(err.contains("domain"));
    }

    #[test]
    fn compare_empty_servers_should_fail() {
        let err = execute("compare", &json!({ "domain": "example.com", "servers": [] }))
            .expect_err("must fail");
        assert!(err.contains("servers"));
    }

    #[test]
    fn ipv4_to_ptr_test() {
        use std::net::Ipv4Addr;
        let addr = Ipv4Addr::new(8, 8, 8, 8);
        assert_eq!(ipv4_to_ptr(&addr), "8.8.8.8.in-addr.arpa.");
    }

    #[test]
    fn ipv6_to_ptr_test() {
        use std::net::Ipv6Addr;
        // 2001:4860:4860::8888 (Google IPv6 DNS)
        let addr: Ipv6Addr = "2001:4860:4860::8888".parse().unwrap();
        assert_eq!(
            ipv6_to_ptr(&addr),
            "8.8.8.8.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.6.8.4.0.6.8.4.1.0.0.2.ip6.arpa."
        );
    }
}

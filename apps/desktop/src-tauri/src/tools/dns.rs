use serde_json::{json, Value};
use std::time::Instant;
use std::net::IpAddr;
use std::str::FromStr;
use std::collections::HashSet;
use hickory_resolver::config::{ResolverConfig, ResolverOpts, NameServerConfig, Protocol};
use hickory_resolver::TokioAsyncResolver;
use hickory_resolver::proto::rr::RecordType;

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "resolve" => resolve(payload),
        "system_dns" => system_dns(),
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

    use std::sync::OnceLock;
    static DNS_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    let rt = DNS_RUNTIME.get_or_init(|| {
        tokio::runtime::Runtime::new().expect("create DNS runtime")
    });

    let result = rt.block_on(async {
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

        let domain_name = format!("{}.", domain.trim_end_matches('.'));

        let (a_records, aaaa_records, cname_records, mx_records, ns_records, txt_records, soa_records, srv_records) = tokio::join!(
            query_a(&resolver, &domain_name),
            query_aaaa(&resolver, &domain_name),
            query_cname(&resolver, &domain_name),
            query_mx(&resolver, &domain_name),
            query_ns(&resolver, &domain_name),
            query_txt(&resolver, &domain_name),
            query_soa(&resolver, &domain_name),
            query_srv(&resolver, &domain_name),
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
        }))
    })?;

    let server_display = if server.is_empty() {
        "system".to_string()
    } else {
        server
    };

    Ok(json!({
        "domain": domain,
        "server": server_display,
        "records": result,
        "elapsed_ms": started.elapsed().as_millis() as u64,
    }))
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
}

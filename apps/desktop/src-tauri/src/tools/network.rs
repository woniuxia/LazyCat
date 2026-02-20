use serde_json::{json, Value};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "tcp_test" => {
            let host = payload["host"].as_str().unwrap_or("127.0.0.1");
            let port = payload["port"].as_u64().unwrap_or(80) as u16;
            let timeout_ms = payload["timeoutMs"].as_u64().unwrap_or(2000);
            let started = Instant::now();
            let addr: SocketAddr = format!("{host}:{port}")
                .parse()
                .map_err(|e| format!("invalid address: {e}"))?;
            let result = TcpStream::connect_timeout(&addr, Duration::from_millis(timeout_ms));
            Ok(json!({
                "host": host,
                "port": port,
                "reachable": result.is_ok(),
                "latencyMs": started.elapsed().as_millis(),
                "error": result.err().map(|e| e.to_string())
            }))
        }
        "http_test" => {
            let url = payload["url"].as_str().unwrap_or("http://127.0.0.1");
            let timeout_ms = payload["timeoutMs"].as_u64().unwrap_or(5000);
            let started = Instant::now();
            let agent = ureq::AgentBuilder::new()
                .timeout(Duration::from_millis(timeout_ms))
                .build();
            match agent.head(url).call() {
                Ok(resp) => Ok(json!({
                    "url": url,
                    "reachable": true,
                    "statusCode": resp.status(),
                    "latencyMs": started.elapsed().as_millis(),
                    "error": null
                })),
                Err(ureq::Error::Status(code, _resp)) => Ok(json!({
                    "url": url,
                    "reachable": true,
                    "statusCode": code,
                    "latencyMs": started.elapsed().as_millis(),
                    "error": null
                })),
                Err(e) => Ok(json!({
                    "url": url,
                    "reachable": false,
                    "statusCode": null,
                    "latencyMs": started.elapsed().as_millis(),
                    "error": e.to_string()
                }))
            }
        }
        _ => Err(format!("unsupported network action: {action}")),
    }
}

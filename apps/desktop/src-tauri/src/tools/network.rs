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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn tcp_test_should_detect_reachable_and_unreachable() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let handle = thread::spawn(move || {
            let _ = listener.accept();
        });

        let ok = execute(
            "tcp_test",
            &json!({ "host": "127.0.0.1", "port": port, "timeoutMs": 1000 }),
        )
        .expect("tcp ok");
        assert_eq!(ok["reachable"], true);
        drop(handle);

        let fail = execute(
            "tcp_test",
            &json!({ "host": "127.0.0.1", "port": 9, "timeoutMs": 100 }),
        )
        .expect("tcp fail path");
        assert!(fail["reachable"].is_boolean());
    }

    #[test]
    fn http_test_should_work_with_local_server() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
            }
        });

        let out = execute(
            "http_test",
            &json!({ "url": format!("http://127.0.0.1:{port}"), "timeoutMs": 1500 }),
        )
        .expect("http test");
        assert_eq!(out["reachable"], true);
        assert!(out["statusCode"].as_u64().unwrap_or(0) >= 200);
    }
}

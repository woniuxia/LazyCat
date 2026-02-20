use serde_json::{json, Value};

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "generate" => generate(payload),
        "lint" => lint(payload),
        _ => Err(format!("unsupported nginx action: {action}")),
    }
}

fn generate(payload: &Value) -> Result<Value, String> {
    let server_name = payload["serverName"].as_str().unwrap_or_default().trim();
    let root = payload["root"].as_str().unwrap_or_default().trim();
    let api_prefix = payload["apiPrefix"].as_str().unwrap_or("/api/").trim();
    let api_upstream = payload["apiUpstream"]
        .as_str()
        .unwrap_or("http://127.0.0.1:8080")
        .trim();
    let listen = payload["listen"].as_i64().unwrap_or(80);
    let index = payload["index"].as_str().unwrap_or("index.html").trim();
    let enable_spa_fallback = payload["enableSpaFallback"].as_bool().unwrap_or(true);
    let enable_gzip = payload["enableGzip"].as_bool().unwrap_or(true);

    if server_name.is_empty() {
        return Err("serverName is required".into());
    }
    if root.is_empty() {
        return Err("root is required".into());
    }
    if api_upstream.is_empty() {
        return Err("apiUpstream is required".into());
    }

    let mut lines = Vec::new();
    lines.push("server {".to_string());
    lines.push(format!("  listen {listen};"));
    lines.push(format!("  server_name {server_name};"));
    lines.push(format!("  root {root};"));
    lines.push(format!("  index {index};"));
    lines.push(String::new());
    lines.push("  location / {".to_string());
    if enable_spa_fallback {
        lines.push("    try_files $uri $uri/ /index.html;".to_string());
    } else {
        lines.push("    try_files $uri $uri/ =404;".to_string());
    }
    lines.push("  }".to_string());
    lines.push(String::new());
    lines.push(format!("  location {api_prefix} {{"));
    lines.push(format!("    proxy_pass {api_upstream};"));
    lines.push("    proxy_set_header Host $host;".to_string());
    lines.push("    proxy_set_header X-Real-IP $remote_addr;".to_string());
    lines.push("    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;".to_string());
    lines.push("    proxy_set_header X-Forwarded-Proto $scheme;".to_string());
    lines.push("  }".to_string());

    if enable_gzip {
        lines.push(String::new());
        lines.push("  gzip on;".to_string());
        lines.push("  gzip_types text/plain text/css application/json application/javascript text/xml application/xml application/xml+rss text/javascript;".to_string());
    }
    lines.push("}".to_string());

    Ok(json!({
        "config": lines.join("\n"),
        "hints": [
            "Set client_max_body_size if your API accepts large uploads.",
            "If backend uses a context path, align apiPrefix and proxy_pass carefully."
        ]
    }))
}

fn lint(payload: &Value) -> Result<Value, String> {
    let config = payload["config"].as_str().unwrap_or_default();
    if config.trim().is_empty() {
        return Err("config is empty".into());
    }

    let mut issues = Vec::new();
    let mut braces = 0i32;
    let mut has_server_block = false;

    for (i, raw_line) in config.lines().enumerate() {
        let line_no = i + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("server") && line.ends_with('{') {
            has_server_block = true;
        }
        braces += line.matches('{').count() as i32;
        braces -= line.matches('}').count() as i32;
        if braces < 0 {
            issues.push(json!({
                "line": line_no,
                "level": "error",
                "message": "unexpected closing brace"
            }));
            braces = 0;
        }

        let is_block_line = line.ends_with('{') || line.ends_with('}') || line == "}";
        if !is_block_line && !line.ends_with(';') {
            issues.push(json!({
                "line": line_no,
                "level": "error",
                "message": "directive must end with semicolon"
            }));
        }
        if line.starts_with("proxy_pass") && !line.contains("http://") && !line.contains("https://")
        {
            issues.push(json!({
                "line": line_no,
                "level": "warn",
                "message": "proxy_pass usually starts with http:// or https://"
            }));
        }
    }

    if braces != 0 {
        issues.push(json!({
            "line": 0,
            "level": "error",
            "message": "unbalanced braces in config"
        }));
    }
    if !has_server_block {
        issues.push(json!({
            "line": 0,
            "level": "error",
            "message": "server block is required"
        }));
    }

    Ok(json!({
        "valid": issues.iter().all(|v| v["level"] != "error"),
        "issues": issues
    }))
}

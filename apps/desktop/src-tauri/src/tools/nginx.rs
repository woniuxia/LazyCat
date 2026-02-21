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
    let api_prefix = normalize_api_prefix(payload["apiPrefix"].as_str().unwrap_or("/api/"));
    let api_upstream = payload["apiUpstream"]
        .as_str()
        .unwrap_or("http://127.0.0.1:8080")
        .trim();
    let listen = payload["listen"].as_i64().unwrap_or(80);
    let index = payload["index"].as_str().unwrap_or("index.html").trim();
    let client_max_body_size = payload["clientMaxBodySize"].as_str().unwrap_or("20m").trim();
    let enable_spa_fallback = payload["enableSpaFallback"].as_bool().unwrap_or(true);
    let enable_gzip = payload["enableGzip"].as_bool().unwrap_or(true);
    let enable_https = payload["enableHttps"].as_bool().unwrap_or(false);
    let enable_http2 = payload["enableHttp2"].as_bool().unwrap_or(false);
    let enable_websocket = payload["enableWebsocket"].as_bool().unwrap_or(false);
    let enable_access_log = payload["enableAccessLog"].as_bool().unwrap_or(true);
    let generate_access_log = payload["generateAccessLog"].as_bool().unwrap_or(true);
    let generate_error_log = payload["generateErrorLog"].as_bool().unwrap_or(true);
    let access_log_path = payload["accessLogPath"]
        .as_str()
        .unwrap_or("/var/log/nginx/access.log")
        .trim();
    let access_log_format = payload["accessLogFormat"].as_str().unwrap_or("main").trim();
    let error_log_path = payload["errorLogPath"]
        .as_str()
        .unwrap_or("/var/log/nginx/error.log")
        .trim();
    let error_log_level = payload["errorLogLevel"].as_str().unwrap_or("warn").trim();
    let ssl_cert = payload["sslCert"].as_str().unwrap_or_default().trim();
    let ssl_key = payload["sslKey"].as_str().unwrap_or_default().trim();

    if server_name.is_empty() {
        return Err("serverName is required".into());
    }
    if root.is_empty() {
        return Err("root is required".into());
    }
    if api_upstream.is_empty() {
        return Err("apiUpstream is required".into());
    }
    if listen < 1 || listen > 65535 {
        return Err("listen port must be 1..65535".into());
    }
    if enable_https && (ssl_cert.is_empty() || ssl_key.is_empty()) {
        return Err("sslCert and sslKey are required when HTTPS is enabled".into());
    }

    let mut lines = Vec::new();
    lines.push("server {".to_string());
    if enable_https {
        if enable_http2 {
            lines.push(format!("  listen {listen} ssl http2;"));
        } else {
            lines.push(format!("  listen {listen} ssl;"));
        }
        lines.push(format!("  ssl_certificate {ssl_cert};"));
        lines.push(format!("  ssl_certificate_key {ssl_key};"));
        lines.push("  ssl_session_timeout 10m;".to_string());
        lines.push("  ssl_protocols TLSv1.2 TLSv1.3;".to_string());
    } else {
        lines.push(format!("  listen {listen};"));
    }
    lines.push(format!("  server_name {server_name};"));
    lines.push(format!("  root {root};"));
    lines.push(format!("  index {index};"));
    lines.push(format!("  client_max_body_size {client_max_body_size};"));
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
    if enable_websocket {
        lines.push("    proxy_http_version 1.1;".to_string());
        lines.push("    proxy_set_header Upgrade $http_upgrade;".to_string());
        lines.push("    proxy_set_header Connection \"upgrade\";".to_string());
    }
    lines.push("  }".to_string());
    lines.push(String::new());
    if enable_gzip {
        lines.push("  gzip on;".to_string());
        lines.push("  gzip_min_length 1k;".to_string());
        lines.push("  gzip_comp_level 5;".to_string());
        lines.push("  gzip_vary on;".to_string());
        lines.push("  gzip_types text/plain text/css application/json application/javascript text/xml application/xml application/xml+rss text/javascript;".to_string());
    }
    if enable_access_log {
        let access_log_target = if access_log_path.is_empty() {
            "/var/log/nginx/access.log"
        } else {
            access_log_path
        };
        let access_log_fmt = if access_log_format.is_empty() {
            "main"
        } else {
            access_log_format
        };
        let err_log_target = if error_log_path.is_empty() {
            "/var/log/nginx/error.log"
        } else {
            error_log_path
        };
        let err_log_level = if error_log_level.is_empty() {
            "warn"
        } else {
            error_log_level
        };
        if generate_access_log {
            lines.push(format!("  access_log {access_log_target} {access_log_fmt};"));
        } else {
            lines.push("  access_log off;".to_string());
        }
        if generate_error_log {
            lines.push(format!("  error_log {err_log_target} {err_log_level};"));
        }
    } else {
        lines.push("  access_log off;".to_string());
    }
    lines.push("}".to_string());

    let mut hints = Vec::new();
    hints.push("如果 API 有大文件上传，适当调大 client_max_body_size。".to_string());
    if enable_https {
        hints.push("HTTPS 已开启，建议同时配置 80 -> 443 的重定向 server 块。".to_string());
    }
    if enable_websocket {
        hints.push("已启用 WebSocket 透传，确保后端也支持 Upgrade 头。".to_string());
    }
    if enable_access_log {
        hints.push("如需 JSON 日志，请在 http 块中定义对应 log_format 后再使用。".to_string());
        if !generate_error_log {
            hints.push("你已关闭 error_log 生成，建议在全局 nginx.conf 统一配置错误日志。".to_string());
        }
    }

    Ok(json!({
        "config": lines.join("\n"),
        "hints": hints
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
    let mut has_location_root = false;
    let mut has_try_files = false;
    let mut has_proxy_pass = false;
    let mut has_ssl_certificate = false;
    let mut has_ssl_certificate_key = false;
    let mut has_listen_ssl = false;
    let mut has_proxy_upgrade = false;
    let mut has_proxy_connection_upgrade = false;

    for (i, raw_line) in config.lines().enumerate() {
        let line_no = i + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("server") && line.ends_with('{') {
            has_server_block = true;
        }
        if line.starts_with("location /") && line.ends_with('{') {
            has_location_root = true;
        }
        if line.starts_with("try_files ") {
            has_try_files = true;
        }
        if line.starts_with("proxy_pass ") {
            has_proxy_pass = true;
            if !line.contains("http://") && !line.contains("https://") {
                issues.push(json!({
                    "line": line_no,
                    "level": "warn",
                    "message": "proxy_pass 通常应以 http:// 或 https:// 开头"
                }));
            }
        }
        if line.starts_with("access_log ") && line.contains(" json;") {
            issues.push(json!({
                "line": line_no,
                "level": "warn",
                "message": "access_log 使用 json 格式时，请确保在 http 块中定义 log_format json"
            }));
        }
        if line.starts_with("ssl_certificate ") {
            has_ssl_certificate = true;
        }
        if line.starts_with("ssl_certificate_key ") {
            has_ssl_certificate_key = true;
        }
        if line.starts_with("listen ") && line.contains("ssl") {
            has_listen_ssl = true;
        }
        if line.starts_with("proxy_set_header Upgrade ") {
            has_proxy_upgrade = true;
        }
        if line.starts_with("proxy_set_header Connection ") && line.contains("upgrade") {
            has_proxy_connection_upgrade = true;
        }

        braces += line.matches('{').count() as i32;
        braces -= line.matches('}').count() as i32;
        if braces < 0 {
            issues.push(json!({
                "line": line_no,
                "level": "error",
                "message": "存在多余的右花括号"
            }));
            braces = 0;
        }

        let is_block_line = line.ends_with('{') || line.ends_with('}') || line == "}";
        if !is_block_line && !line.ends_with(';') {
            issues.push(json!({
                "line": line_no,
                "level": "error",
                "message": "指令行必须以分号结尾"
            }));
        }
    }

    if braces != 0 {
        issues.push(json!({
            "line": 0,
            "level": "error",
            "message": "花括号不平衡"
        }));
    }
    if !has_server_block {
        issues.push(json!({
            "line": 0,
            "level": "error",
            "message": "必须包含 server 块"
        }));
    }
    if !has_location_root {
        issues.push(json!({
            "line": 0,
            "level": "warn",
            "message": "建议包含 location / 用于静态资源与首页处理"
        }));
    } else if !has_try_files {
        issues.push(json!({
            "line": 0,
            "level": "warn",
            "message": "建议在 location / 中使用 try_files 处理 SPA 或 404"
        }));
    }
    if !has_proxy_pass {
        issues.push(json!({
            "line": 0,
            "level": "warn",
            "message": "未检测到 proxy_pass，若需要 API 反代请补充 location"
        }));
    }
    if has_listen_ssl && (!has_ssl_certificate || !has_ssl_certificate_key) {
        issues.push(json!({
            "line": 0,
            "level": "error",
            "message": "listen 启用 ssl 但缺少 ssl_certificate 或 ssl_certificate_key"
        }));
    }
    if has_proxy_upgrade ^ has_proxy_connection_upgrade {
        issues.push(json!({
            "line": 0,
            "level": "warn",
            "message": "WebSocket 透传头不完整，建议同时配置 Upgrade 与 Connection upgrade"
        }));
    }

    Ok(json!({
        "valid": issues.iter().all(|v| v["level"] != "error"),
        "issues": issues
    }))
}

fn normalize_api_prefix(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "/api/".to_string();
    }
    let with_leading = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };
    if with_leading.ends_with('/') {
        with_leading
    } else {
        format!("{with_leading}/")
    }
}

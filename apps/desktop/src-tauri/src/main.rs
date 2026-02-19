#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use base64::{engine::general_purpose::{STANDARD as BASE64, URL_SAFE_NO_PAD as BASE64URL}, Engine};
use chrono::{Local, TimeZone, Utc};
use image::ImageFormat;
use openssl::pkey::{Private, Public};
use openssl::rsa::{Padding, Rsa};
use openssl::symm::{decrypt, encrypt, Cipher};
use qrcode::QrCode;
use rand::Rng;
use cron::Schedule;
use regex::Regex;
use std::str::FromStr;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use uuid::Uuid;

static MANUAL_SERVERS: OnceLock<HashMap<String, u16>> = OnceLock::new();

fn start_manual_server(root_dir: PathBuf) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind manual server");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let dir = root_dir.clone();
            std::thread::spawn(move || handle_manual_request(stream, &dir));
        }
    });
    port
}

fn handle_manual_request(mut stream: TcpStream, root_dir: &Path) {
    let mut buf = [0u8; 4096];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return,
    };
    let request = String::from_utf8_lossy(&buf[..n]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    // 解码 URL 编码的路径 (%xx)
    let decoded_path = urlencoding::decode(path).unwrap_or_else(|_| path.into());
    let rel = decoded_path.trim_start_matches('/');
    let file_path = root_dir.join(rel);
    // 安全检查：防止路径穿越
    if !file_path.starts_with(root_dir) {
        let resp = "HTTP/1.1 403 Forbidden\r\nContent-Length: 9\r\n\r\nForbidden";
        let _ = stream.write_all(resp.as_bytes());
        return;
    }
    // 如果是目录，尝试 index.html；如果文件不存在且无扩展名，尝试加 .html
    let file_path = if file_path.is_dir() {
        file_path.join("index.html")
    } else if !file_path.exists() && file_path.extension().is_none() {
        let with_html = file_path.with_extension("html");
        if with_html.exists() {
            with_html
        } else {
            // 也尝试作为目录 + index.html（无扩展名的无文件情况）
            file_path.join("index.html")
        }
    } else {
        file_path
    };
    // VitePress lean.js fallback: 请求 foo.js 但磁盘只有 foo.lean.js
    let file_path = if !file_path.exists() {
        if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
            if ext == "js" {
                let lean = file_path.with_extension("lean.js");
                if lean.exists() { lean } else { file_path }
            } else { file_path }
        } else { file_path }
    } else { file_path };

    match fs::read(&file_path) {
        Ok(body) => {
            let mime = match file_path.extension().and_then(|e| e.to_str()) {
                Some("html") | Some("htm") => "text/html; charset=utf-8",
                Some("css")  => "text/css",
                Some("js") | Some("mjs") => "application/javascript",
                Some("json") => "application/json",
                Some("png")  => "image/png",
                Some("jpg") | Some("jpeg") => "image/jpeg",
                Some("gif")  => "image/gif",
                Some("svg")  => "image/svg+xml",
                Some("woff") => "font/woff",
                Some("woff2")=> "font/woff2",
                Some("ttf")  => "font/ttf",
                Some("ico")  => "image/x-icon",
                Some("xml")  => "application/xml",
                Some("txt")  => "text/plain; charset=utf-8",
                Some("wasm") => "application/wasm",
                None             => {
                    // 无扩展名：检测 body 是否以 HTML doctype 开头
                    if body.starts_with(b"<!DOCTYPE") || body.starts_with(b"<html") {
                        "text/html; charset=utf-8"
                    } else {
                        "application/octet-stream"
                    }
                }
                Some(_)          => "application/octet-stream",
            };
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&body);
        }
        Err(_) => {
            let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\nNot Found";
            let _ = stream.write_all(resp.as_bytes());
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct ToolRequest {
    request_id: String,
    domain: String,
    action: String,
    payload: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct ToolError {
    code: String,
    message: String,
    details: Option<Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct ToolMeta {
    duration_ms: u128,
    warnings: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct ToolResponse {
    request_id: String,
    ok: bool,
    data: Option<Value>,
    error: Option<ToolError>,
    meta: ToolMeta,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortUsageEntry {
    protocol: String,
    local_address: String,
    remote_address: String,
    state: Option<String>,
    pid: u32,
    process_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortProcessSummary {
    pid: u32,
    process_name: String,
    listening_ports: Vec<String>,
    connection_count: usize,
}

#[tauri::command]
fn tool_execute(request: ToolRequest) -> ToolResponse {
    let start = Instant::now();
    match execute_tool(&request.domain, &request.action, &request.payload) {
        Ok(data) => ToolResponse {
            request_id: request.request_id,
            ok: true,
            data: Some(data),
            error: None,
            meta: ToolMeta {
                duration_ms: start.elapsed().as_millis(),
                warnings: None,
            },
        },
        Err(message) => ToolResponse {
            request_id: request.request_id,
            ok: false,
            data: None,
            error: Some(ToolError {
                code: "TOOL_EXECUTION_FAILED".to_string(),
                message,
                details: None,
            }),
            meta: ToolMeta {
                duration_ms: start.elapsed().as_millis(),
                warnings: None,
            },
        },
    }
}

fn execute_tool(domain: &str, action: &str, payload: &Value) -> Result<Value, String> {
    match (domain, action) {
        ("encode", "base64_encode") => {
            let input = payload["input"].as_str().unwrap_or_default();
            Ok(json!(BASE64.encode(input.as_bytes())))
        }
        ("encode", "base64_decode") => {
            let input = payload["input"].as_str().unwrap_or_default();
            let decoded = BASE64
                .decode(input)
                .map_err(|e| format!("base64 decode failed: {e}"))?;
            Ok(json!(String::from_utf8_lossy(&decoded).to_string()))
        }
        ("encode", "base64_url_encode") => {
            let input = payload["input"].as_str().unwrap_or_default();
            Ok(json!(BASE64URL.encode(input.as_bytes())))
        }
        ("encode", "base64_url_decode") => {
            let input = payload["input"].as_str().unwrap_or_default();
            let decoded = BASE64URL
                .decode(input)
                .map_err(|e| format!("base64url decode failed: {e}"))?;
            Ok(json!(String::from_utf8_lossy(&decoded).to_string()))
        }
        ("encode", "url_encode") => {
            let input = payload["input"].as_str().unwrap_or_default();
            Ok(json!(urlencoding::encode(input).to_string()))
        }
        ("encode", "url_decode") => {
            let input = payload["input"].as_str().unwrap_or_default();
            Ok(json!(
                urlencoding::decode(input)
                    .map_err(|e| format!("url decode failed: {e}"))?
                    .to_string()
            ))
        }
        ("encode", "md5") => {
            let input = payload["input"].as_str().unwrap_or_default();
            Ok(json!(format!("{:x}", md5::compute(input.as_bytes()))))
        }
        ("encode", "qr_generate") => {
            let input = payload["input"].as_str().unwrap_or_default();
            let code = QrCode::new(input.as_bytes()).map_err(|e| format!("qr generation failed: {e}"))?;
            let image = code.render::<image::Luma<u8>>().build();
            let mut cursor = std::io::Cursor::new(Vec::new());
            image
                .write_to(&mut cursor, ImageFormat::Png)
                .map_err(|e| format!("png encode failed: {e}"))?;
            Ok(json!(format!(
                "data:image/png;base64,{}",
                BASE64.encode(cursor.into_inner())
            )))
        }
        ("convert", "json_to_xml") => {
            let input = payload["input"].as_str().unwrap_or_default();
            let v: Value = serde_json::from_str(input).map_err(|e| format!("invalid json: {e}"))?;
            let root_tag = payload["rootTag"]
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("root");
            Ok(json!(json_to_xml(root_tag, &v)))
        }
        ("convert", "xml_to_json") => {
            let input = payload["input"].as_str().unwrap_or_default();
            let v: Value = quick_xml::de::from_str(input).map_err(|e| format!("invalid xml: {e}"))?;
            Ok(json!(serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into())))
        }
        ("convert", "json_to_yaml") => {
            let input = payload["input"].as_str().unwrap_or_default();
            let v: Value = serde_json::from_str(input).map_err(|e| format!("invalid json: {e}"))?;
            let out = serde_yaml::to_string(&v).map_err(|e| format!("json->yaml failed: {e}"))?;
            Ok(json!(out))
        }
        ("convert", "csv_to_json") => {
            let input = payload["input"].as_str().unwrap_or_default();
            let delimiter = payload["delimiter"].as_str().unwrap_or(",").as_bytes()[0];
            let has_header = payload["hasHeader"].as_bool().unwrap_or(true);
            let custom_headers: Option<Vec<String>> = payload["customHeaders"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());
            let selected_columns: Option<Vec<usize>> = payload["selectedColumns"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_u64().map(|n| n as usize)).collect());

            let mut rdr = csv::ReaderBuilder::new()
                .delimiter(delimiter)
                .has_headers(has_header)
                .from_reader(input.as_bytes());

            let headers: Vec<String> = if let Some(ref custom) = custom_headers {
                custom.clone()
            } else if has_header {
                rdr.headers()
                    .map_err(|e| format!("csv read header failed: {e}"))?
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            } else {
                // peek first record to determine column count
                let mut peek_rdr = csv::ReaderBuilder::new()
                    .delimiter(delimiter)
                    .has_headers(false)
                    .from_reader(input.as_bytes());
                let count = peek_rdr.records().next()
                    .and_then(|r| r.ok())
                    .map(|r| r.len())
                    .unwrap_or(0);
                (0..count).map(|i| format!("col{}", i + 1)).collect()
            };

            let mut rows = Vec::new();
            for rec in rdr.records() {
                let record = rec.map_err(|e| format!("csv record failed: {e}"))?;
                let mut obj = serde_json::Map::new();
                for (i, col) in headers.iter().enumerate() {
                    if let Some(ref sel) = selected_columns {
                        if !sel.contains(&i) {
                            continue;
                        }
                    }
                    obj.insert(col.clone(), json!(record.get(i).unwrap_or("")));
                }
                rows.push(Value::Object(obj));
            }
            Ok(json!(serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".into())))
        }
        ("convert", "csv_read_file") => {
            let path = payload["path"].as_str().unwrap_or_default();
            if path.is_empty() {
                return Err("file path is empty".into());
            }
            let bytes = fs::read(path)
                .map_err(|e| format!("read csv file failed: {e}"))?;
            // Try UTF-8 first; fall back to GBK (common on Windows for Chinese text)
            let content = match String::from_utf8(bytes.clone()) {
                Ok(s) => s,
                Err(_) => {
                    let (cow, _, had_errors) = encoding_rs::GBK.decode(&bytes);
                    if had_errors {
                        return Err("文件编码无法识别，请使用 UTF-8 或 GBK 编码的文件".into());
                    }
                    cow.into_owned()
                }
            };
            Ok(json!(content))
        }
        ("text", "unique_lines") => {
            let input = payload["input"].as_str().unwrap_or_default();
            let case_sensitive = payload["caseSensitive"].as_bool().unwrap_or(false);
            let mut seen = HashSet::new();
            let mut out = Vec::new();
            for line in input.lines() {
                let key = if case_sensitive {
                    line.to_string()
                } else {
                    line.to_lowercase()
                };
                if !seen.contains(&key) {
                    seen.insert(key);
                    out.push(line.to_string());
                }
            }
            Ok(json!(out.join("\n")))
        }
        ("text", "sort_lines") => {
            let input = payload["input"].as_str().unwrap_or_default();
            let case_sensitive = payload["caseSensitive"].as_bool().unwrap_or(false);
            let mut lines = input.lines().map(|s| s.to_string()).collect::<Vec<_>>();
            if case_sensitive {
                lines.sort();
            } else {
                lines.sort_by_key(|x| x.to_lowercase());
            }
            Ok(json!(lines.join("\n")))
        }
        ("time", "timestamp_to_date") => {
            let input = payload["input"].as_i64().unwrap_or_default();
            let ts_ms = if input < 1_000_000_000_000 { input * 1000 } else { input };
            let dt_local = Local
                .timestamp_millis_opt(ts_ms)
                .single()
                .ok_or("invalid timestamp".to_string())?;
            Ok(json!(dt_local.format("%Y-%m-%d %H:%M:%S").to_string()))
        }
        ("time", "date_to_timestamp") => {
            let input = payload["input"].as_str().unwrap_or_default();
            let dt = chrono::DateTime::parse_from_rfc3339(input)
                .map(|d| d.with_timezone(&Utc))
                .or_else(|_| {
                    Local
                        .datetime_from_str(input, "%Y-%m-%d %H:%M:%S")
                        .map(|d| d.with_timezone(&Utc))
                })
                .map_err(|e| format!("invalid datetime: {e}"))?;
            Ok(json!({
                "seconds": dt.timestamp(),
                "milliseconds": dt.timestamp_millis()
            }))
        }
        ("gen", "uuid") => Ok(json!(Uuid::new_v4().to_string())),
        ("gen", "guid") => Ok(json!(format!("{{{}}}", Uuid::new_v4().to_string().to_uppercase()))),
        ("gen", "password") => {
            let length = payload["length"].as_u64().unwrap_or(16) as usize;
            let uppercase = payload["uppercase"].as_bool().unwrap_or(true);
            let lowercase = payload["lowercase"].as_bool().unwrap_or(true);
            let numbers = payload["numbers"].as_bool().unwrap_or(true);
            let symbols = payload["symbols"].as_bool().unwrap_or(false);
            let mut chars = String::new();
            if uppercase {
                chars.push_str("ABCDEFGHIJKLMNOPQRSTUVWXYZ");
            }
            if lowercase {
                chars.push_str("abcdefghijklmnopqrstuvwxyz");
            }
            if numbers {
                chars.push_str("0123456789");
            }
            if symbols {
                chars.push_str("!@#$%^&*()-_=+[]{};:,.<>?");
            }
            if chars.is_empty() {
                return Err("password charset is empty".into());
            }
            let mut rng = rand::thread_rng();
            let bytes = chars.as_bytes();
            let out = (0..length)
                .map(|_| bytes[rng.gen_range(0..bytes.len())] as char)
                .collect::<String>();
            Ok(json!(out))
        }
        ("regex", "test") => {
            let pattern = payload["pattern"].as_str().unwrap_or_default();
            let flags = payload["flags"].as_str().unwrap_or("");
            let input = payload["input"].as_str().unwrap_or_default();
            let mut p = pattern.to_string();
            if flags.contains('i') {
                p = format!("(?i){pattern}");
            }
            let re = Regex::new(&p).map_err(|e| format!("regex invalid: {e}"))?;
            let mut out = Vec::new();
            for m in re.find_iter(input) {
                out.push(json!({
                    "index": m.start(),
                    "match": m.as_str(),
                    "groups": []
                }));
            }
            Ok(Value::Array(out))
        }
        ("regex", "generate") => {
            let kind = payload["kind"].as_str().unwrap_or("email");
            let exp = match kind {
                "ipv4" => "^(25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)(\\.(25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)){3}$",
                "url" => "^(https?:\\/\\/)?([\\w-]+\\.)+[\\w-]+([\\w\\-./?%&=]*)?$",
                "phone-cn" => "^1[3-9]\\d{9}$",
                _ => "^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}$",
            };
            Ok(json!(exp))
        }
        ("regex", "templates") => Ok(json!([
            {"name":"邮箱地址","category":"common","expression":"^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}$"},
            {"name":"IPv4 地址","category":"network","expression":"^(25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)(\\.(25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)){3}$"},
            {"name":"URL 链接","category":"common","expression":"^(https?:\\/\\/)?([\\w-]+\\.)+[\\w-]+([\\w\\-./?%&=]*)?$"},
            {"name":"中国手机号","category":"common","expression":"^1[3-9]\\d{9}$"}
        ])),
        ("cron", "generate") => {
            let second = payload["second"].as_str().unwrap_or("0");
            let minute = payload["minute"].as_str().unwrap_or("*");
            let hour = payload["hour"].as_str().unwrap_or("*");
            let day_of_month = payload["dayOfMonth"].as_str().unwrap_or("*");
            let month = payload["month"].as_str().unwrap_or("*");
            let day_of_week = payload["dayOfWeek"].as_str().unwrap_or("*");
            Ok(json!(format!(
                "{second} {minute} {hour} {day_of_month} {month} {day_of_week}"
            )))
        }
        ("cron", "preview") => {
            let expression = payload["expression"].as_str().unwrap_or("0 * * * * *");
            let count = payload["count"].as_u64().unwrap_or(5) as usize;
            let schedule = Schedule::from_str(expression)
                .map_err(|e| format!("无效的 Cron 表达式: {e}"))?;
            let now = Local::now();
            let times: Vec<String> = schedule
                .after(&now)
                .take(count)
                .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                .collect();
            Ok(json!(times))
        }
        ("cron", "parse") => {
            let expression = payload["expression"].as_str().unwrap_or("").trim();
            let parts: Vec<&str> = expression.split_whitespace().collect();
            if parts.len() != 6 {
                return Err(format!(
                    "表达式必须包含 6 个字段（秒 分 时 日 月 周），当前 {} 个",
                    parts.len()
                ));
            }
            Schedule::from_str(expression)
                .map_err(|e| format!("无效的 Cron 表达式: {e}"))?;
            Ok(json!({
                "second":     parts[0],
                "minute":     parts[1],
                "hour":       parts[2],
                "dayOfMonth": parts[3],
                "month":      parts[4],
                "dayOfWeek":  parts[5]
            }))
        }
        ("crypto", "rsa_encrypt") => {
            let plaintext = payload["plaintext"].as_str().unwrap_or_default().as_bytes().to_vec();
            let public_pem = payload["publicKeyPem"].as_str().unwrap_or_default();
            let rsa: Rsa<Public> =
                Rsa::public_key_from_pem(public_pem.as_bytes()).map_err(|e| format!("invalid public key: {e}"))?;
            let mut buf = vec![0; rsa.size() as usize];
            let len = rsa
                .public_encrypt(&plaintext, &mut buf, Padding::PKCS1_OAEP)
                .map_err(|e| format!("rsa encrypt failed: {e}"))?;
            buf.truncate(len);
            Ok(json!(BASE64.encode(buf)))
        }
        ("crypto", "rsa_decrypt") => {
            let cipher = payload["cipherTextBase64"].as_str().unwrap_or_default();
            let data = BASE64.decode(cipher).map_err(|e| format!("invalid base64: {e}"))?;
            let private_pem = payload["privateKeyPem"].as_str().unwrap_or_default();
            let rsa: Rsa<Private> =
                Rsa::private_key_from_pem(private_pem.as_bytes()).map_err(|e| format!("invalid private key: {e}"))?;
            let mut buf = vec![0; rsa.size() as usize];
            let len = rsa
                .private_decrypt(&data, &mut buf, Padding::PKCS1_OAEP)
                .map_err(|e| format!("rsa decrypt failed: {e}"))?;
            buf.truncate(len);
            Ok(json!(String::from_utf8_lossy(&buf).to_string()))
        }
        ("crypto", "aes_encrypt") => {
            let plaintext = payload["plaintext"].as_str().unwrap_or_default().as_bytes();
            let key = payload["key"].as_str().unwrap_or_default().as_bytes();
            let iv = payload["iv"].as_str().unwrap_or_default().as_bytes();
            let algorithm = payload["algorithm"].as_str().unwrap_or("aes-256-cbc");
            let cipher = match algorithm {
                "aes-128-cbc" => Cipher::aes_128_cbc(),
                "aes-192-cbc" => Cipher::aes_192_cbc(),
                _ => Cipher::aes_256_cbc(),
            };
            let out = encrypt(cipher, key, Some(iv), plaintext).map_err(|e| format!("aes encrypt failed: {e}"))?;
            Ok(json!(BASE64.encode(out)))
        }
        ("crypto", "aes_decrypt") => {
            let cipher_text = payload["cipherTextBase64"].as_str().unwrap_or_default();
            let cipher_data = BASE64
                .decode(cipher_text)
                .map_err(|e| format!("invalid base64: {e}"))?;
            let key = payload["key"].as_str().unwrap_or_default().as_bytes();
            let iv = payload["iv"].as_str().unwrap_or_default().as_bytes();
            let algorithm = payload["algorithm"].as_str().unwrap_or("aes-256-cbc");
            let cipher = match algorithm {
                "aes-128-cbc" => Cipher::aes_128_cbc(),
                "aes-192-cbc" => Cipher::aes_192_cbc(),
                _ => Cipher::aes_256_cbc(),
            };
            let out = decrypt(cipher, key, Some(iv), &cipher_data).map_err(|e| format!("aes decrypt failed: {e}"))?;
            Ok(json!(String::from_utf8_lossy(&out).to_string()))
        }
        ("crypto", "des_encrypt") => {
            let plaintext = payload["plaintext"].as_str().unwrap_or_default().as_bytes();
            let key = payload["key"].as_str().unwrap_or_default().as_bytes();
            let iv = payload["iv"].as_str().unwrap_or_default().as_bytes();
            let algorithm = payload["algorithm"].as_str().unwrap_or("des-ede3-cbc");
            let cipher = if algorithm == "des-cbc" {
                Cipher::des_cbc()
            } else {
                Cipher::des_ede3_cbc()
            };
            let out = encrypt(cipher, key, Some(iv), plaintext).map_err(|e| format!("des encrypt failed: {e}"))?;
            Ok(json!(BASE64.encode(out)))
        }
        ("crypto", "des_decrypt") => {
            let cipher_text = payload["cipherTextBase64"].as_str().unwrap_or_default();
            let cipher_data = BASE64
                .decode(cipher_text)
                .map_err(|e| format!("invalid base64: {e}"))?;
            let key = payload["key"].as_str().unwrap_or_default().as_bytes();
            let iv = payload["iv"].as_str().unwrap_or_default().as_bytes();
            let algorithm = payload["algorithm"].as_str().unwrap_or("des-ede3-cbc");
            let cipher = if algorithm == "des-cbc" {
                Cipher::des_cbc()
            } else {
                Cipher::des_ede3_cbc()
            };
            let out = decrypt(cipher, key, Some(iv), &cipher_data).map_err(|e| format!("des decrypt failed: {e}"))?;
            Ok(json!(String::from_utf8_lossy(&out).to_string()))
        }
        ("format", "json") => {
            let input = payload["input"].as_str().unwrap_or_default();
            let v: Value = serde_json::from_str(input).map_err(|e| format!("invalid json: {e}"))?;
            Ok(json!(serde_json::to_string_pretty(&v).unwrap_or_else(|_| input.to_string())))
        }
        ("format", "xml") => Ok(json!(payload["input"].as_str().unwrap_or_default().to_string())),
        ("format", "html") => Ok(json!(payload["input"].as_str().unwrap_or_default().to_string())),
        ("format", "java") => Ok(json!(
            payload["input"]
                .as_str()
                .unwrap_or_default()
                .lines()
                .map(|l| l.trim_end().to_string())
                .collect::<Vec<_>>()
                .join("\n")
        )),
        ("format", "sql") => Ok(json!(payload["input"].as_str().unwrap_or_default().to_string())),
        ("network", "tcp_test") => {
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
        ("network", "http_test") => {
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
        ("env", "detect") => {
            let node = Command::new("node")
                .arg("-v")
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|| "NOT_FOUND".into());
            let java = Command::new("java")
                .arg("-version")
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(if o.stderr.is_empty() { &o.stdout } else { &o.stderr }).lines().next().unwrap_or("UNKNOWN").to_string())
                .unwrap_or_else(|| "NOT_FOUND".into());
            Ok(json!({"node": node, "java": java}))
        }
        ("port", "usage") => {
            let output = Command::new("netstat")
                .arg("-ano")
                .output()
                .map_err(|e| format!("netstat failed: {e}"))?;
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            let mut entries = parse_netstat_entries(&text);
            let proc_names = list_process_names();
            for item in &mut entries {
                item.process_name = proc_names
                    .get(&item.pid)
                    .cloned()
                    .unwrap_or_else(|| "UNKNOWN".to_string());
            }
            let mut state_counts: HashMap<String, usize> = HashMap::new();
            let mut tcp_count = 0usize;
            let mut udp_count = 0usize;
            for item in &entries {
                match item.protocol.as_str() {
                    "TCP" => tcp_count += 1,
                    "UDP" => udp_count += 1,
                    _ => {}
                }
                if let Some(state) = &item.state {
                    *state_counts.entry(state.clone()).or_insert(0) += 1;
                }
            }
            let process_summaries = build_process_summaries(&entries);
            Ok(json!({
                "summary": {
                    "total": entries.len(),
                    "tcp": tcp_count,
                    "udp": udp_count
                },
                "stateCounts": state_counts,
                "processSummaries": process_summaries,
                "connections": entries
            }))
        }
        ("file", "split") => file_split(payload),
        ("file", "merge") => file_merge(payload),
        ("image", "convert") => image_convert(payload),
        ("hosts", "save") => hosts_save(payload),
        ("hosts", "list") => hosts_list(),
        ("hosts", "delete") => hosts_delete(payload),
        ("hosts", "activate") => hosts_activate(payload),
        ("manuals", "list") => {
            let servers = MANUAL_SERVERS.get();
            let mut list = Vec::new();
            let known = [
                ("vue3",         "Vue 3 开发手册",       "/guide/introduction.html"),
                ("element-plus", "Element Plus 组件库",  "/zh-CN/guide/design"),
            ];
            for (id, name, home) in known {
                if let Some(port) = servers.and_then(|m| m.get(id)) {
                    list.push(json!({"id": id, "name": name, "url": format!("http://127.0.0.1:{port}{home}")}));
                }
            }
            Ok(json!(list))
        }
        _ => Err(format!("unsupported command: {domain}.{action}")),
    }
}

fn get_data_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("home dir not found".to_string())?;
    let p = home.join(".lazycat");
    fs::create_dir_all(&p).map_err(|e| format!("create data dir failed: {e}"))?;
    Ok(p)
}

fn parse_netstat_entries(raw: &str) -> Vec<PortUsageEntry> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts = trimmed.split_whitespace().collect::<Vec<_>>();
        if parts.is_empty() {
            continue;
        }
        let proto = parts[0].to_ascii_uppercase();
        if proto != "TCP" && proto != "UDP" {
            continue;
        }
        if proto == "TCP" {
            if parts.len() < 5 {
                continue;
            }
            let pid = parts[parts.len() - 1].parse::<u32>().unwrap_or(0);
            let state = parts[parts.len() - 2].to_string();
            let remote = parts[parts.len() - 3].to_string();
            let local = parts[parts.len() - 4].to_string();
            out.push(PortUsageEntry {
                protocol: "TCP".to_string(),
                local_address: local,
                remote_address: remote,
                state: Some(state),
                pid,
                process_name: String::new(),
            });
            continue;
        }
        if parts.len() < 4 {
            continue;
        }
        let pid = parts[parts.len() - 1].parse::<u32>().unwrap_or(0);
        let remote = parts[parts.len() - 2].to_string();
        let local = parts[parts.len() - 3].to_string();
        out.push(PortUsageEntry {
            protocol: "UDP".to_string(),
            local_address: local,
            remote_address: remote,
            state: None,
            pid,
            process_name: String::new(),
        });
    }
    out
}

fn list_process_names() -> HashMap<u32, String> {
    let mut out = HashMap::new();
    let output = match Command::new("tasklist").args(["/FO", "CSV", "/NH"]).output() {
        Ok(v) => v,
        Err(_) => return out,
    };
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(text.as_bytes());
    for rec in rdr.records().flatten() {
        if rec.len() < 2 {
            continue;
        }
        let name = rec.get(0).unwrap_or("UNKNOWN").trim().to_string();
        let pid = rec
            .get(1)
            .unwrap_or_default()
            .replace(',', "")
            .trim()
            .parse::<u32>()
            .unwrap_or(0);
        if pid > 0 && !name.is_empty() {
            out.insert(pid, name);
        }
    }
    out
}

fn extract_port(local_address: &str) -> Option<String> {
    let port = local_address.rsplit(':').next().unwrap_or_default().trim();
    if port.is_empty() || port == "*" {
        return None;
    }
    if port.chars().all(|c| c.is_ascii_digit()) {
        return Some(port.to_string());
    }
    None
}

fn build_process_summaries(entries: &[PortUsageEntry]) -> Vec<PortProcessSummary> {
    let mut grouped: HashMap<u32, (String, BTreeSet<String>, usize)> = HashMap::new();
    for item in entries {
        let entry = grouped
            .entry(item.pid)
            .or_insert_with(|| (item.process_name.clone(), BTreeSet::new(), 0usize));
        entry.2 += 1;
        let is_listening = item
            .state
            .as_ref()
            .map(|s| s.eq_ignore_ascii_case("LISTENING"))
            .unwrap_or(false)
            || (item.protocol == "UDP" && item.remote_address == "*:*");
        if is_listening {
            if let Some(port) = extract_port(&item.local_address) {
                entry.1.insert(port);
            }
        }
    }
    let mut out = grouped
        .into_iter()
        .map(|(pid, (process_name, listening_ports, connection_count))| PortProcessSummary {
            pid,
            process_name,
            listening_ports: listening_ports.into_iter().collect(),
            connection_count,
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| {
        b.connection_count
            .cmp(&a.connection_count)
            .then_with(|| a.process_name.cmp(&b.process_name))
    });
    out
}

fn db_conn() -> Result<Connection, String> {
    let db_path = get_data_dir()?.join("lazycat.sqlite");
    let conn = Connection::open(db_path).map_err(|e| format!("open db failed: {e}"))?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS hosts_profiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            content TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
    ",
    )
    .map_err(|e| format!("init db failed: {e}"))?;
    Ok(conn)
}

fn hosts_save(payload: &Value) -> Result<Value, String> {
    let name = payload["name"].as_str().unwrap_or_default();
    let content = payload["content"].as_str().unwrap_or_default();
    if name.is_empty() {
        return Err("hosts profile name is empty".into());
    }
    let conn = db_conn()?;
    conn.execute(
        "INSERT INTO hosts_profiles(name, content, enabled, updated_at) VALUES(?1, ?2, 0, CURRENT_TIMESTAMP)
        ON CONFLICT(name) DO UPDATE SET content=excluded.content, updated_at=CURRENT_TIMESTAMP",
        params![name, content],
    )
    .map_err(|e| format!("save hosts profile failed: {e}"))?;
    Ok(json!({"ok": true}))
}

fn hosts_list() -> Result<Value, String> {
    let conn = db_conn()?;
    let mut stmt = conn
        .prepare("SELECT id, name, content, enabled, updated_at FROM hosts_profiles ORDER BY updated_at DESC")
        .map_err(|e| format!("prepare query failed: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "content": row.get::<_, String>(2)?,
                "enabled": row.get::<_, i64>(3)? == 1,
                "updatedAt": row.get::<_, String>(4)?,
            }))
        })
        .map_err(|e| format!("query hosts failed: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(Value::Array(out))
}

fn hosts_delete(payload: &Value) -> Result<Value, String> {
    let name = payload["name"].as_str().unwrap_or_default();
    let conn = db_conn()?;
    conn.execute("DELETE FROM hosts_profiles WHERE name = ?1", params![name])
        .map_err(|e| format!("delete hosts profile failed: {e}"))?;
    Ok(json!({"ok": true}))
}

fn hosts_activate(payload: &Value) -> Result<Value, String> {
    let profile_name = payload["profileName"].as_str().unwrap_or_default();
    let mut content = payload["content"].as_str().unwrap_or_default().to_string();
    let conn = db_conn()?;
    if content.is_empty() {
        let mut stmt = conn
            .prepare("SELECT content FROM hosts_profiles WHERE name=?1 LIMIT 1")
            .map_err(|e| format!("prepare get profile failed: {e}"))?;
        let mut rows = stmt
            .query(params![profile_name])
            .map_err(|e| format!("query profile failed: {e}"))?;
        if let Some(row) = rows.next().map_err(|e| e.to_string())? {
            content = row.get::<_, String>(0).map_err(|e| e.to_string())?;
        }
    }
    if content.is_empty() {
        return Err("Hosts profile content is empty.".into());
    }
    let hosts_path = PathBuf::from("C:\\Windows\\System32\\drivers\\etc\\hosts");
    let backup_dir = get_data_dir()?.join("hosts-backups");
    fs::create_dir_all(&backup_dir).map_err(|e| format!("create backup dir failed: {e}"))?;
    let original = fs::read_to_string(&hosts_path).map_err(|e| format!("read hosts failed: {e}"))?;
    let stamp = Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let backup_path = backup_dir.join(format!("{stamp}-{profile_name}.hosts.bak"));
    fs::write(&backup_path, original).map_err(|e| format!("write backup failed: {e}"))?;
    fs::write(&hosts_path, content.as_bytes()).map_err(|e| format!("write hosts failed: {e}"))?;
    conn.execute("UPDATE hosts_profiles SET enabled = 0", [])
        .map_err(|e| format!("disable previous profiles failed: {e}"))?;
    conn.execute(
        "UPDATE hosts_profiles SET enabled = 1, updated_at = CURRENT_TIMESTAMP WHERE name = ?1",
        params![profile_name],
    )
    .map_err(|e| format!("mark profile enabled failed: {e}"))?;
    Ok(json!({
      "backupPath": backup_path.to_string_lossy().to_string(),
      "digest": format!("{:x}", md5::compute(content.as_bytes()))
    }))
}

fn file_split(payload: &Value) -> Result<Value, String> {
    let source_path = PathBuf::from(payload["sourcePath"].as_str().unwrap_or_default());
    let output_dir = PathBuf::from(payload["outputDir"].as_str().unwrap_or_default());
    let chunk_mb = payload["chunkSizeMb"].as_u64().unwrap_or(100) as usize;
    if !source_path.exists() {
        return Err("source file not found".into());
    }
    fs::create_dir_all(&output_dir).map_err(|e| format!("create output dir failed: {e}"))?;
    let metadata = fs::metadata(&source_path).map_err(|e| format!("stat source failed: {e}"))?;
    let chunk_size = chunk_mb * 1024 * 1024;
    let total = metadata.len() as usize;
    let mut reader = File::open(&source_path).map_err(|e| format!("open source failed: {e}"))?;
    let mut idx = 0usize;
    let filename = source_path
        .file_name()
        .and_then(|x| x.to_str())
        .ok_or("invalid source filename".to_string())?;
    loop {
        let mut buf = vec![0u8; chunk_size];
        let n = reader.read(&mut buf).map_err(|e| format!("read source failed: {e}"))?;
        if n == 0 {
            break;
        }
        buf.truncate(n);
        let part_name = format!("{filename}.part{:04}", idx + 1);
        let part_path = output_dir.join(&part_name);
        fs::write(&part_path, &buf).map_err(|e| format!("write part failed: {e}"))?;
        idx += 1;
    }
    Ok(json!({
      "chunkCount": idx,
      "outputDir": output_dir.to_string_lossy().to_string(),
      "totalBytes": total
    }))
}

fn file_merge(payload: &Value) -> Result<Value, String> {
    let parts = payload["parts"]
        .as_array()
        .ok_or("parts should be array".to_string())?;
    let output_path = PathBuf::from(payload["outputPath"].as_str().unwrap_or_default());
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create output parent failed: {e}"))?;
    }
    let mut writer = File::create(&output_path).map_err(|e| format!("create output failed: {e}"))?;
    let mut total_bytes = 0usize;
    for p in parts {
        let part_path = PathBuf::from(p.as_str().unwrap_or_default());
        let bytes = fs::read(&part_path).map_err(|e| format!("read part failed: {e}"))?;
        total_bytes += bytes.len();
        writer
            .write_all(&bytes)
            .map_err(|e| format!("write output failed: {e}"))?;
    }
    Ok(json!({
      "outputPath": output_path.to_string_lossy().to_string(),
      "totalBytes": total_bytes
    }))
}

fn image_convert(payload: &Value) -> Result<Value, String> {
    let input_path = PathBuf::from(payload["inputPath"].as_str().unwrap_or_default());
    let output_path = PathBuf::from(payload["outputPath"].as_str().unwrap_or_default());
    if !input_path.exists() {
        return Err("input image not found".into());
    }
    let mut img = image::open(&input_path).map_err(|e| format!("open image failed: {e}"))?;
    if let (Some(cw), Some(ch)) = (payload["cropWidth"].as_u64(), payload["cropHeight"].as_u64()) {
        let x = payload["cropX"].as_u64().unwrap_or(0);
        let y = payload["cropY"].as_u64().unwrap_or(0);
        img = img.crop_imm(x as u32, y as u32, cw as u32, ch as u32);
    }
    let width = payload["width"].as_u64().unwrap_or(img.width() as u64) as u32;
    let height = payload["height"].as_u64().unwrap_or(img.height() as u64) as u32;
    let resized = img.resize(width, height, image::imageops::FilterType::Lanczos3);
    let format = payload["format"].as_str().unwrap_or("png").to_lowercase();
    let img_format = match format.as_str() {
        "jpeg" | "jpg" => ImageFormat::Jpeg,
        "webp" => ImageFormat::WebP,
        "avif" => ImageFormat::Avif,
        _ => ImageFormat::Png,
    };
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create output parent failed: {e}"))?;
    }
    resized
        .save_with_format(&output_path, img_format)
        .map_err(|e| format!("save image failed: {e}"))?;
    let metadata = fs::metadata(&output_path).map_err(|e| format!("stat output failed: {e}"))?;
    Ok(json!({
      "outputPath": output_path.to_string_lossy().to_string(),
      "width": resized.width(),
      "height": resized.height(),
      "size": metadata.len()
    }))
}

fn json_to_xml(root_tag: &str, value: &Value) -> String {
    let root = sanitize_xml_tag(root_tag, "root");
    let mut out = String::new();
    append_xml_node_pretty(&mut out, &root, value, 0);
    out.trim_end_matches('\n').to_string()
}

fn append_xml_node_pretty(out: &mut String, tag: &str, value: &Value, depth: usize) {
    match value {
        Value::Array(items) => {
            if items.is_empty() {
                write_indent(out, depth);
                out.push('<');
                out.push_str(tag);
                out.push_str("/>");
                out.push('\n');
                return;
            }
            for item in items {
                append_xml_node_pretty(out, tag, item, depth);
            }
        }
        Value::Object(map) => {
            if map.is_empty() {
                write_indent(out, depth);
                out.push('<');
                out.push_str(tag);
                out.push_str("/>");
                out.push('\n');
                return;
            }

            write_indent(out, depth);
            out.push('<');
            out.push_str(tag);
            out.push('>');
            out.push('\n');
            for (key, child) in map {
                let child_tag = sanitize_xml_tag(key, "item");
                append_xml_node_pretty(out, &child_tag, child, depth + 1);
            }
            write_indent(out, depth);
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
            out.push('\n');
        }
        Value::Null => {
            write_indent(out, depth);
            out.push('<');
            out.push_str(tag);
            out.push_str("/>");
            out.push('\n');
        }
        Value::String(s) => {
            write_indent(out, depth);
            out.push('<');
            out.push_str(tag);
            out.push('>');
            out.push_str(&escape_xml_text(s));
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
            out.push('\n');
        }
        Value::Bool(b) => {
            write_indent(out, depth);
            out.push('<');
            out.push_str(tag);
            out.push('>');
            out.push_str(if *b { "true" } else { "false" });
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
            out.push('\n');
        }
        Value::Number(n) => {
            write_indent(out, depth);
            out.push('<');
            out.push_str(tag);
            out.push('>');
            out.push_str(&n.to_string());
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
            out.push('\n');
        }
    }
}

fn write_indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn sanitize_xml_tag(input: &str, fallback: &str) -> String {
    let mut out = String::new();
    for ch in input.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        return fallback.to_string();
    }
    if let Some(first) = out.chars().next() {
        if !first.is_ascii_alphabetic() && first != '_' {
            out.insert(0, '_');
        }
    }
    out
}

fn escape_xml_text(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[tauri::command]
fn register_hotkey(app: tauri::AppHandle, shortcut: String) -> Result<(), String> {
    let manager = app.global_shortcut();
    // Unregister all existing shortcuts first
    manager.unregister_all().map_err(|e| e.to_string())?;
    if shortcut.is_empty() {
        return Ok(());
    }
    let sc: Shortcut = shortcut.parse().map_err(|e| format!("{e}"))?;
    manager
        .on_shortcut(sc, move |app_handle, _sc, event| {
            if event.state == ShortcutState::Pressed {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let visible = window.is_visible().unwrap_or(false);
                    if visible {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn unregister_hotkey(app: tauri::AppHandle) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // 启动离线文档 HTTP 服务器
            // 打包后从 resource_dir/manuals 读取；开发模式下 fallback 到源码目录
            let manuals_dir = {
                let rd = app.path().resource_dir().ok().map(|d| d.join("manuals"));
                if rd.as_ref().is_some_and(|d| d.exists()) {
                    rd.unwrap()
                } else {
                    // 开发模式：src-tauri/../../../resources/manuals
                    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../../resources/manuals");
                    std::fs::canonicalize(&dev).unwrap_or(dev)
                }
            };
            if manuals_dir.exists() {
                let mut ports = HashMap::new();
                if let Ok(entries) = fs::read_dir(&manuals_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            if let Some(id) = path.file_name().and_then(|n| n.to_str()) {
                                let port = start_manual_server(path.clone());
                                ports.insert(id.to_string(), port);
                            }
                        }
                    }
                }
                let _ = MANUAL_SERVERS.set(ports);
            }

            let show_item = MenuItem::with_id(app, "show", "显示", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let visible = window.is_visible().unwrap_or(false);
                            if visible {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            tool_execute,
            register_hotkey,
            unregister_hotkey
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

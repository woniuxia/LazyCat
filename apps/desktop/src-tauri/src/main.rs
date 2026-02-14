#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::{Local, TimeZone, Utc};
use image::ImageFormat;
use openssl::pkey::{Private, Public};
use openssl::rsa::{Padding, Rsa};
use openssl::symm::{decrypt, encrypt, Cipher};
use qrcode::QrCode;
use rand::Rng;
use regex::Regex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use uuid::Uuid;

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
            quick_xml::se::to_string(&v)
                .map(|s| json!(s))
                .map_err(|e| format!("json->xml failed: {e}"))
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
            let mut rdr = csv::ReaderBuilder::new()
                .delimiter(delimiter)
                .from_reader(input.as_bytes());
            let headers = rdr
                .headers()
                .map_err(|e| format!("csv read header failed: {e}"))?
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>();
            let mut rows = Vec::new();
            for rec in rdr.records() {
                let record = rec.map_err(|e| format!("csv record failed: {e}"))?;
                let mut obj = serde_json::Map::new();
                for (i, col) in headers.iter().enumerate() {
                    obj.insert(col.clone(), json!(record.get(i).unwrap_or("")));
                }
                rows.push(Value::Object(obj));
            }
            Ok(json!(serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".into())))
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
            let mut out = Vec::new();
            for i in 0..count {
                out.push(format!("Preview {} for {}", i + 1, expression));
            }
            Ok(json!(out))
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
            Ok(json!(text))
        }
        ("file", "split") => file_split(payload),
        ("file", "merge") => file_merge(payload),
        ("image", "convert") => image_convert(payload),
        ("hosts", "save") => hosts_save(payload),
        ("hosts", "list") => hosts_list(),
        ("hosts", "delete") => hosts_delete(payload),
        ("hosts", "activate") => hosts_activate(payload),
        ("manuals", "list") => {
            let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
            Ok(json!([
              {"id":"vue2","name":"Vue 2 开发手册","path":cwd.join("resources").join("manuals").join("vue2").join("index.html")},
              {"id":"vue3","name":"Vue 3 开发手册","path":cwd.join("resources").join("manuals").join("vue3").join("index.html")},
              {"id":"element-plus","name":"Element Plus 开发手册","path":cwd.join("resources").join("manuals").join("element-plus").join("index.html")}
            ]))
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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![tool_execute])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

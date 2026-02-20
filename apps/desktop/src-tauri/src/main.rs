#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tools;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Instant;

use tools::hotkey::HOTKEY_MAPPINGS_DIR;
use tools::manuals::MANUAL_SERVERS;
use tools::regex::REGEX_TEMPLATES_DIR;

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
                    // 无扩展名：检测 body 是否以 HTML doctype 开头（跳过可能的 UTF-8 BOM）
                    let content = if body.starts_with(&[0xEF, 0xBB, 0xBF]) { &body[3..] } else { &body[..] };
                    if content.starts_with(b"<!DOCTYPE") || content.starts_with(b"<!doctype") || content.starts_with(b"<html") {
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

#[tauri::command]
fn tool_execute(request: ToolRequest) -> ToolResponse {
    let start = Instant::now();
    match tools::execute_tool(&request.domain, &request.action, &request.payload) {
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

            // 初始化正则模板目录
            let regex_dir = {
                let rd = app.path().resource_dir().ok().map(|d| d.join("regex-library"));
                if rd.as_ref().is_some_and(|d| d.join("templates.json").exists()) {
                    rd.unwrap()
                } else {
                    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../../resources/regex-library");
                    std::fs::canonicalize(&dev).unwrap_or(dev)
                }
            };
            let _ = REGEX_TEMPLATES_DIR.set(regex_dir);

            // 初始化热键映射目录
            let hotkey_dir = {
                let rd = app.path().resource_dir().ok().map(|d| d.join("hotkey-library"));
                if rd.as_ref().is_some_and(|d| d.join("app-hotkey-mappings.json").exists()) {
                    rd.unwrap()
                } else {
                    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../../resources/hotkey-library");
                    std::fs::canonicalize(&dev).unwrap_or(dev)
                }
            };
            let _ = HOTKEY_MAPPINGS_DIR.set(hotkey_dir);

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

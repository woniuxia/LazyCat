use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use crate::tools::manuals::MANUAL_SERVERS;

pub(crate) fn initialize_manual_servers(manuals_dir: &Path) {
    if !manuals_dir.exists() {
        return;
    }
    let mut ports = HashMap::new();
    if let Ok(entries) = fs::read_dir(manuals_dir) {
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

fn start_manual_server(root_dir: PathBuf) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind manual server");
    let port = listener
        .local_addr()
        .expect("get manual server port")
        .port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let dir = root_dir.clone();
            std::thread::spawn(move || handle_manual_request(stream, &dir));
        }
    });
    port
}

fn handle_manual_request(mut stream: TcpStream, root_dir: &Path) {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 4096];
    loop {
        let n = match stream.read(&mut tmp) {
            Ok(0) => return,
            Ok(n) => n,
            Err(_) => return,
        };
        buf.extend_from_slice(&tmp[..n]);
        // Check if we've received the full HTTP headers
        if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() > 8192 {
            break;
        }
    }
    let request = String::from_utf8_lossy(&buf);
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
                if lean.exists() {
                    lean
                } else {
                    file_path
                }
            } else {
                file_path
            }
        } else {
            file_path
        }
    } else {
        file_path
    };

    match fs::read(&file_path) {
        Ok(body) => {
            let mime = match file_path.extension().and_then(|e| e.to_str()) {
                Some("html") | Some("htm") => "text/html; charset=utf-8",
                Some("css") => "text/css",
                Some("js") | Some("mjs") => "application/javascript",
                Some("json") => "application/json",
                Some("png") => "image/png",
                Some("jpg") | Some("jpeg") => "image/jpeg",
                Some("gif") => "image/gif",
                Some("svg") => "image/svg+xml",
                Some("woff") => "font/woff",
                Some("woff2") => "font/woff2",
                Some("ttf") => "font/ttf",
                Some("ico") => "image/x-icon",
                Some("xml") => "application/xml",
                Some("txt") => "text/plain; charset=utf-8",
                Some("wasm") => "application/wasm",
                None => {
                    // 无扩展名：检测 body 是否以 HTML doctype 开头（跳过可能的 UTF-8 BOM）
                    let content = if body.starts_with(&[0xEF, 0xBB, 0xBF]) {
                        &body[3..]
                    } else {
                        &body[..]
                    };
                    if content.starts_with(b"<!DOCTYPE")
                        || content.starts_with(b"<!doctype")
                        || content.starts_with(b"<html")
                    {
                        "text/html; charset=utf-8"
                    } else {
                        "application/octet-stream"
                    }
                }
                Some(_) => "application/octet-stream",
            };
            // 对 HTML 响应注入 CSS+JS，隐藏离线无用的 MDN 导航弹窗和 UI 元素
            let body = if mime.starts_with("text/html") {
                const INJECT: &str = "<style>\
                    .notification-bar,.mdn-cta,.article-actions-container,.place,.top-banner,\
                    .page-layout__banner,mdn-placement-top,mdn-placement-bottom,\
                    .navigation__popup,.menu__panel,.menu__panel-title,.menu__panel-content,\
                    .page-layout__footer,.pong-box,.top-level-entry-container .menu__panel,\
                    .content-section.article-footer,.bb-banner,#bb-banner,.spsr-container,\
                    .preference-tooltip,.vp-repl,[class*=\"repl\"]\
                    {display:none!important}\
                    .repl-notice{padding:1rem;background:#f5f5f5;border:1px solid #ddd;border-radius:4px;margin:1rem 0;color:#666;}\
                    </style>\
                    <script>\
                    (function(){\
                      window.aa=function(){};\
                      function removePopups(){\
                        document.querySelectorAll('[class*=\"popup\"],[class*=\"modal\"],[class*=\"banner\"],[class*=\"notification\"],[class*=\"cta\"],[class*=\"overlay\"]').forEach(function(el){\
                          var s=window.getComputedStyle(el);\
                          if((s.position==='fixed'||s.position==='sticky')&&el.getBoundingClientRect().width>100){\
                            el.style.setProperty('display','none','important');\
                          }\
                        });\
                      }\
                      function disableRepl(){\
                        document.querySelectorAll('.vp-repl,[class*=\"repl\"]').forEach(function(el){\
                          el.style.display='none';\
                          var notice=document.createElement('div');\
                          notice.className='repl-notice';\
                          notice.textContent='离线模式下交互式示例不可用,请参考静态代码示例';\
                          if(el.parentNode){el.parentNode.insertBefore(notice,el);}\
                        });\
                        document.querySelectorAll('script[src*=\"rom3\"],script[src*=\"cdn.jsdelivr.net\"],script[src*=\"unpkg.com\"],script[src*=\"perfops.net\"]').forEach(function(s){s.remove();});\
                      }\
                      document.addEventListener('DOMContentLoaded',function(){\
                        removePopups();\
                        disableRepl();\
                      });\
                      setTimeout(removePopups,1000);\
                      setTimeout(removePopups,3000);\
                      setTimeout(disableRepl,500);\
                    })();\
                    </script>";
                if let Some(pos) = body.windows(7).position(|w| w == b"</head>") {
                    let mut patched = Vec::with_capacity(body.len() + INJECT.len());
                    patched.extend_from_slice(&body[..pos]);
                    patched.extend_from_slice(INJECT.as_bytes());
                    patched.extend_from_slice(&body[pos..]);
                    patched
                } else {
                    body
                }
            } else {
                body
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

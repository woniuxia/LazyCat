#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tools;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_notification::NotificationExt;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Stores currently registered shortcut strings keyed by name (e.g. "toggle", "snippets", "vault").
static REGISTERED_SHORTCUTS: std::sync::LazyLock<Mutex<HashMap<String, String>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Flag: when true, the window subclass suppresses WM_SYSCOMMAND (SC_KEYMENU)
/// so Alt+Space does not open the system menu during shortcut recording.
static RECORDING_MODE: AtomicBool = AtomicBool::new(false);
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tools::hotkey::HOTKEY_MAPPINGS_DIR;
use tools::manuals::MANUAL_SERVERS;
use tools::regex::REGEX_TEMPLATES_DIR;

fn start_manual_server(root_dir: PathBuf) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind manual server");
    let port = listener.local_addr().expect("get manual server port").port();
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
fn tool_execute(app: tauri::AppHandle, request: ToolRequest) -> ToolResponse {
    let start = Instant::now();
    match tools::execute_tool_with_app(&request.domain, &request.action, &request.payload, &app) {
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

/// Bring the main window to foreground/focus.
/// Keep tray/hotkey/single-instance behavior consistent.
fn reveal_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        #[cfg(windows)]
        force_foreground(&window);
    }
}

fn same_monitor(lhs: &tauri::Monitor, rhs: &tauri::Monitor) -> bool {
    let lp = lhs.position();
    let ls = lhs.size();
    let rp = rhs.position();
    let rs = rhs.size();
    lp.x == rp.x && lp.y == rp.y && ls.width == rs.width && ls.height == rs.height
}

fn clamp_i64_to_i32(value: i64) -> i32 {
    value.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CursorMonitorRelation {
    MovedToCursorMonitor,
    AlreadyOnCursorMonitor,
    Unknown,
}

/// Move the main window to the monitor under current cursor.
/// Returns the relationship between the window and the cursor monitor.
fn move_window_to_cursor_monitor(window: &tauri::WebviewWindow) -> CursorMonitorRelation {
    let Ok(cursor) = window.cursor_position() else {
        return CursorMonitorRelation::Unknown;
    };

    let Ok(Some(target_monitor)) = window.monitor_from_point(cursor.x, cursor.y) else {
        return CursorMonitorRelation::Unknown;
    };

    if let Ok(Some(current_monitor)) = window.current_monitor() {
        if same_monitor(&current_monitor, &target_monitor) {
            return CursorMonitorRelation::AlreadyOnCursorMonitor;
        }
    }

    let work_area = target_monitor.work_area();
    let mut x = work_area.position.x;
    let mut y = work_area.position.y;

    if let Ok(window_size) = window.outer_size() {
        let target_w = work_area.size.width as i64;
        let target_h = work_area.size.height as i64;
        let win_w = window_size.width as i64;
        let win_h = window_size.height as i64;
        x = clamp_i64_to_i32(
            work_area.position.x as i64 + ((target_w - win_w).max(0) / 2),
        );
        y = clamp_i64_to_i32(
            work_area.position.y as i64 + ((target_h - win_h).max(0) / 2),
        );
    }

    if window
        .set_position(tauri::PhysicalPosition::new(x, y))
        .is_ok()
    {
        CursorMonitorRelation::MovedToCursorMonitor
    } else {
        CursorMonitorRelation::Unknown
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HotkeyNavigatePayload {
    target: String,
    did_move_to_cursor_monitor: bool,
    was_window_visible: bool,
    was_window_focused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainWindowShortcutMode {
    Toggle,
    Navigate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainWindowShortcutDecision {
    Hide,
    Reveal,
    RevealAndNavigate { did_move_to_cursor_monitor: bool },
}

/// 统一主窗口快捷键的动作决策，便于复用与测试。
fn decide_main_window_shortcut(
    mode: MainWindowShortcutMode,
    visible: bool,
    focused: bool,
    cursor_monitor_relation: CursorMonitorRelation,
) -> MainWindowShortcutDecision {
    match mode {
        MainWindowShortcutMode::Toggle => {
            if visible
                && focused
                && cursor_monitor_relation != CursorMonitorRelation::MovedToCursorMonitor
            {
                MainWindowShortcutDecision::Hide
            } else {
                MainWindowShortcutDecision::Reveal
            }
        }
        MainWindowShortcutMode::Navigate => MainWindowShortcutDecision::RevealAndNavigate {
            did_move_to_cursor_monitor: cursor_monitor_relation
                == CursorMonitorRelation::MovedToCursorMonitor,
        },
    }
}

/// 统一处理主窗口类快捷键：共享异屏迁移、显示恢复与导航行为。
fn handle_main_window_shortcut(app: &tauri::AppHandle, shortcut_name: &str) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    let mode = if shortcut_name == "toggle" {
        MainWindowShortcutMode::Toggle
    } else {
        MainWindowShortcutMode::Navigate
    };

    let visible = window.is_visible().unwrap_or(false);
    let focused = window.is_focused().unwrap_or(false);
    let cursor_monitor_relation = move_window_to_cursor_monitor(&window);
    let decision = decide_main_window_shortcut(mode, visible, focused, cursor_monitor_relation);

    match decision {
        MainWindowShortcutDecision::Hide => {
            let _ = window.hide();
        }
        MainWindowShortcutDecision::Reveal => {
            reveal_main_window(app);
        }
        MainWindowShortcutDecision::RevealAndNavigate { did_move_to_cursor_monitor } => {
            reveal_main_window(app);
            let _ = window.emit(
                "hotkey-navigate",
                HotkeyNavigatePayload {
                    target: shortcut_name.to_string(),
                    did_move_to_cursor_monitor,
                    was_window_visible: visible,
                    was_window_focused: focused,
                },
            );
        }
    }
}

/// Re-register all shortcuts from the global map.
/// Called after any add/remove to keep the shortcut manager in sync.
fn sync_all_shortcuts(app: &tauri::AppHandle) -> Result<(), String> {
    let manager = app.global_shortcut();
    manager.unregister_all().map_err(|e| e.to_string())?;

    let map = REGISTERED_SHORTCUTS.lock().map_err(|e| format!("快捷键锁定失败: {e}"))?;
    for (name, shortcut_str) in map.iter() {
        if shortcut_str.is_empty() {
            continue;
        }
        let sc: Shortcut = shortcut_str.parse().map_err(|e| format!("{e}"))?;
        let name_owned = name.clone();
        manager
            .on_shortcut(sc, move |app_handle, _sc, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                handle_main_window_shortcut(app_handle, name_owned.as_str());
            })
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn register_hotkey(app: tauri::AppHandle, shortcut: String) -> Result<(), String> {
    // Backward-compatible: register as "toggle"
    register_named_hotkey(app, "toggle".into(), shortcut)
}

#[tauri::command]
fn unregister_hotkey(app: tauri::AppHandle) -> Result<(), String> {
    unregister_named_hotkey(app, "toggle".into())
}

#[tauri::command]
fn register_named_hotkey(app: tauri::AppHandle, name: String, shortcut: String) -> Result<(), String> {
    {
        let mut map = REGISTERED_SHORTCUTS.lock().map_err(|e| format!("快捷键锁定失败: {e}"))?;
        if shortcut.is_empty() {
            map.remove(&name);
        } else {
            // Validate first
            let _: Shortcut = shortcut.parse().map_err(|e| format!("{e}"))?;
            map.insert(name, shortcut);
        }
    }
    sync_all_shortcuts(&app)
}

#[tauri::command]
fn unregister_named_hotkey(app: tauri::AppHandle, name: String) -> Result<(), String> {
    {
        let mut map = REGISTERED_SHORTCUTS.lock().map_err(|e| format!("快捷键锁定失败: {e}"))?;
        map.remove(&name);
    }
    sync_all_shortcuts(&app)
}

/// Window subclass ID (arbitrary unique value)
/// Force a window to the foreground on Windows using the AttachThreadInput trick.
/// Windows 10+ restricts SetForegroundWindow to the current foreground process;
/// this workaround temporarily attaches our thread's input queue to the foreground
/// thread so that the call succeeds reliably.
#[cfg(windows)]
fn force_foreground(window: &tauri::WebviewWindow) {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
        ShowWindow, SW_RESTORE, IsIconic,
    };

    let Ok(hwnd_raw) = window.hwnd() else { return };
    let hwnd = hwnd_raw.0;

    unsafe {
        // Restore if minimized
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }

        let foreground = GetForegroundWindow();
        if foreground.is_null() || foreground == hwnd {
            // No foreground window or we are already it – just call directly
            SetForegroundWindow(hwnd);
            return;
        }

        let fg_thread = GetWindowThreadProcessId(foreground, std::ptr::null_mut());
        let our_thread = GetCurrentThreadId();

        if fg_thread != our_thread && fg_thread != 0 {
            // Attach our input queue to the foreground thread's
            AttachThreadInput(our_thread, fg_thread, 1); // TRUE
            SetForegroundWindow(hwnd);
            AttachThreadInput(our_thread, fg_thread, 0); // FALSE – detach
        } else {
            SetForegroundWindow(hwnd);
        }
    }
}

#[cfg(windows)]
const SUBCLASS_ID: usize = 0x4C5A_4341; // "LZCA"

/// Subclass procedure: intercepts WM_SYSCOMMAND to block system menu during recording.
#[cfg(windows)]
unsafe extern "system" fn recording_subclass_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
    _uid_subclass: usize,
    _ref_data: usize,
) -> windows_sys::Win32::Foundation::LRESULT {
    use windows_sys::Win32::UI::WindowsAndMessaging::{WM_SYSCOMMAND, SC_KEYMENU};
    // SC_KEYMENU is triggered by Alt+Space (and Alt+key for menu mnemonics).
    // Block it when recording mode is active so the system menu won't appear.
    if msg == WM_SYSCOMMAND && (wparam & 0xFFF0) == SC_KEYMENU as usize {
        if RECORDING_MODE.load(Ordering::Relaxed) {
            return 0;
        }
    }
    windows_sys::Win32::UI::Shell::DefSubclassProc(hwnd, msg, wparam, lparam)
}

#[tauri::command]
fn pause_all_shortcuts(app: tauri::AppHandle) -> Result<(), String> {
    let manager = app.global_shortcut();
    manager.unregister_all().map_err(|e| e.to_string())?;
    RECORDING_MODE.store(true, Ordering::Relaxed);
    // Install subclass on first call; subsequent calls are harmless (same ID = update)
    #[cfg(windows)]
    if let Some(window) = app.get_webview_window("main") {
        if let Ok(hwnd) = window.hwnd() {
            unsafe {
                windows_sys::Win32::UI::Shell::SetWindowSubclass(
                    hwnd.0,
                    Some(recording_subclass_proc),
                    SUBCLASS_ID,
                    0,
                );
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn resume_all_shortcuts(app: tauri::AppHandle) -> Result<(), String> {
    RECORDING_MODE.store(false, Ordering::Relaxed);
    // Subclass remains installed but becomes a no-op (flag is false).
    // No need to remove it — keeps things simple and avoids race conditions.
    sync_all_shortcuts(&app)
}

// ─── Packet Capture Commands ───────────────────────────────────

#[tauri::command]
fn check_npcap_installed() -> bool {
    tools::capture::check_npcap()
}

#[tauri::command]
fn list_capture_interfaces() -> Result<Vec<tools::capture::InterfaceInfo>, String> {
    tools::capture::list_interfaces()
}

#[tauri::command]
fn start_capture(
    session_id: String,
    interface: String,
    filter: String,
    on_packet: tauri::ipc::Channel<tools::capture::CaptureEvent>,
) -> Result<(), String> {
    tools::capture::start_capture(session_id, interface, filter, on_packet)
}

#[tauri::command]
fn stop_capture(session_id: String) -> Result<tools::capture::CaptureStats, String> {
    tools::capture::stop_capture(&session_id)
}

#[tauri::command]
fn clear_capture_session(session_id: String) -> Result<(), String> {
    tools::capture::clear_session(&session_id)
}

#[tauri::command]
fn export_pcap(session_id: String, path: String) -> Result<(), String> {
    tools::capture::export_pcap(&session_id, &path)
}

fn start_todo_scheduler(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        match tools::todo::scheduler_tick() {
            Ok(reminders) => {
                for reminder in reminders {
                    let _ = app
                        .notification()
                        .builder()
                        .title(&reminder.title)
                        .body(&reminder.body)
                        .show();

                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.emit("todo-reminder-fired", &reminder);
                    }
                }
            }
            Err(_) => {
                // 调度失败不影响主流程，等待下一轮重试
            }
        }

        std::thread::sleep(Duration::from_secs(30));
    });
}

fn main() {
    // 绿色免安装包支持：检测 exe 同级目录下的 WebView2 Fixed Runtime，
    // 设置环境变量让 WebView2Loader.dll 使用本地运行时而非系统安装版本。
    // 必须在 Tauri 初始化之前调用。
    #[cfg(target_os = "windows")]
    {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                // 查找 exe 同级的 Microsoft.WebView2.FixedVersionRuntime.* 目录
                if let Ok(entries) = std::fs::read_dir(exe_dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name();
                        let name_str = name.to_string_lossy();
                        if name_str.starts_with("Microsoft.WebView2.FixedVersionRuntime.")
                            && entry.path().is_dir()
                        {
                            unsafe {
                                std::env::set_var(
                                    "WEBVIEW2_BROWSER_EXECUTABLE_FOLDER",
                                    entry.path(),
                                );
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    let mut builder = tauri::Builder::default();
    #[cfg(windows)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(
            |app, _args, _cwd| {
                reveal_main_window(app);
            },
        ));
    }

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
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

            // 检查是否为开机自启动且用户设置了启动时最小化
            let args: Vec<String> = std::env::args().collect();
            let is_autostart = args.contains(&"--minimized".to_string());

            if is_autostart {
                // 读取用户设置
                if let Ok(conn) = tools::helpers::db_conn() {
                    let value: Result<String, _> = conn.query_row(
                        "SELECT value FROM user_settings WHERE key = ?1",
                        ["autostart_minimized"],
                        |row| row.get(0),
                    );
                    if let Ok(val) = value {
                        if val == "true" {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.hide();
                            }
                        }
                    }
                }
            }

            let show_item = MenuItem::with_id(app, "show", "显示", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().expect("app icon missing").clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        reveal_main_window(app);
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
                        handle_main_window_shortcut(app, "toggle");
                    }
                })
                .build(app)?;

            // 启动待办调度线程（周期实例生成 + 到期提醒派发）
            start_todo_scheduler(app.handle().clone());

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // 读取用户设置
                let close_to_tray = match tools::helpers::db_conn() {
                    Ok(conn) => {
                        let value: Result<String, _> = conn.query_row(
                            "SELECT value FROM user_settings WHERE key = ?1",
                            ["close_to_tray"],
                            |row| row.get(0),
                        );
                        value.unwrap_or_else(|_| "true".to_string()) == "true"
                    }
                    Err(_) => true, // 数据库连接失败，使用默认行为（隐藏到托盘）
                };

                if close_to_tray {
                    api.prevent_close();
                    tools::vault::force_lock();
                    let _ = window.hide();
                }
                // 否则允许窗口关闭（应用退出）
            }
        })
        .invoke_handler(tauri::generate_handler![
            tool_execute,
            register_hotkey,
            unregister_hotkey,
            register_named_hotkey,
            unregister_named_hotkey,
            pause_all_shortcuts,
            resume_all_shortcuts,
            check_npcap_installed,
            list_capture_interfaces,
            start_capture,
            stop_capture,
            clear_capture_session,
            export_pcap
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}


#[cfg(test)]
mod tests {
    use super::{
        CursorMonitorRelation, HotkeyNavigatePayload, MainWindowShortcutDecision,
        MainWindowShortcutMode, decide_main_window_shortcut,
    };
    use serde_json::json;

    #[test]
    fn toggle_same_screen_and_focused_hides() {
        assert_eq!(
            decide_main_window_shortcut(
                MainWindowShortcutMode::Toggle,
                true,
                true,
                CursorMonitorRelation::AlreadyOnCursorMonitor,
            ),
            MainWindowShortcutDecision::Hide
        );
        assert_eq!(
            decide_main_window_shortcut(
                MainWindowShortcutMode::Toggle,
                true,
                true,
                CursorMonitorRelation::Unknown,
            ),
            MainWindowShortcutDecision::Hide
        );
    }

    #[test]
    fn toggle_cross_screen_or_inactive_reveals() {
        assert_eq!(
            decide_main_window_shortcut(
                MainWindowShortcutMode::Toggle,
                true,
                true,
                CursorMonitorRelation::MovedToCursorMonitor,
            ),
            MainWindowShortcutDecision::Reveal
        );
        assert_eq!(
            decide_main_window_shortcut(
                MainWindowShortcutMode::Toggle,
                false,
                false,
                CursorMonitorRelation::AlreadyOnCursorMonitor,
            ),
            MainWindowShortcutDecision::Reveal
        );
        assert_eq!(
            decide_main_window_shortcut(
                MainWindowShortcutMode::Toggle,
                true,
                false,
                CursorMonitorRelation::AlreadyOnCursorMonitor,
            ),
            MainWindowShortcutDecision::Reveal
        );
    }

    #[test]
    fn navigate_same_screen_does_not_mark_cross_screen_move() {
        assert_eq!(
            decide_main_window_shortcut(
                MainWindowShortcutMode::Navigate,
                true,
                true,
                CursorMonitorRelation::AlreadyOnCursorMonitor,
            ),
            MainWindowShortcutDecision::RevealAndNavigate {
                did_move_to_cursor_monitor: false,
            }
        );
    }

    #[test]
    fn navigate_cross_screen_marks_cursor_monitor_move() {
        assert_eq!(
            decide_main_window_shortcut(
                MainWindowShortcutMode::Navigate,
                true,
                true,
                CursorMonitorRelation::MovedToCursorMonitor,
            ),
            MainWindowShortcutDecision::RevealAndNavigate {
                did_move_to_cursor_monitor: true,
            }
        );
    }

    #[test]
    fn navigate_same_screen_ignores_focus_and_visibility() {
        assert_eq!(
            decide_main_window_shortcut(
                MainWindowShortcutMode::Navigate,
                false,
                false,
                CursorMonitorRelation::AlreadyOnCursorMonitor,
            ),
            MainWindowShortcutDecision::RevealAndNavigate {
                did_move_to_cursor_monitor: false,
            }
        );
        assert_eq!(
            decide_main_window_shortcut(
                MainWindowShortcutMode::Navigate,
                true,
                false,
                CursorMonitorRelation::AlreadyOnCursorMonitor,
            ),
            MainWindowShortcutDecision::RevealAndNavigate {
                did_move_to_cursor_monitor: false,
            }
        );
    }

    #[test]
    fn navigate_unknown_monitor_relation_does_not_report_cross_screen_move() {
        assert_eq!(
            decide_main_window_shortcut(
                MainWindowShortcutMode::Navigate,
                true,
                true,
                CursorMonitorRelation::Unknown,
            ),
            MainWindowShortcutDecision::RevealAndNavigate {
                did_move_to_cursor_monitor: false,
            }
        );
    }

    #[test]
    fn hotkey_navigate_payload_serializes_with_camel_case_keys() {
        assert_eq!(
            serde_json::to_value(HotkeyNavigatePayload {
                target: "launcher".to_string(),
                did_move_to_cursor_monitor: true,
                was_window_visible: true,
                was_window_focused: false,
            })
            .unwrap(),
            json!({
                "target": "launcher",
                "didMoveToCursorMonitor": true,
                "wasWindowVisible": true,
                "wasWindowFocused": false,
            })
        );
    }
}

//! 环境探测 / 跨平台系统交互域。
//!
//! 当前提供：
//! - get_paths：一次性返回 dataDir / attachmentsDir，供前端拼接 convertFileSrc 源路径
//! - open_external：通过 OS 默认程序打开外链，严格限制协议白名单（http/https/mailto），
//!                   避免在 WebView 内跳转或被钓鱼协议利用
//! - read_clipboard_files：读取 Windows 剪贴板中 CF_HDROP 的本地文件路径数组；
//!                         非 Windows 平台固定返回空数组
//! - open_local_path：通过 OS 默认程序打开本地文件；路径必须是经过校验的绝对路径
//! - reveal_in_folder：在系统文件管理器中定位到目标路径（Windows: explorer /select,）
//! - check_paths_exist：批量检测一组路径是否存在，用于 Viewer 失效标红

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

use super::helpers::{get_attachments_dir, get_data_dir};

const ACTIONS: &[&str] = &[
    "get_paths",
    "open_external",
    "read_clipboard_files",
    "open_local_path",
    "reveal_in_folder",
    "check_paths_exist",
    "local_ips",
];

pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported system action: {action}"));
    }
    match action {
        "get_paths" => get_paths(),
        "open_external" => open_external(payload),
        "read_clipboard_files" => read_clipboard_files(),
        "open_local_path" => open_local_path(payload),
        "reveal_in_folder" => reveal_in_folder(payload),
        "check_paths_exist" => check_paths_exist(payload),
        "local_ips" => local_ips(),
        _ => Err(format!("unsupported system action: {action}")),
    }
}

fn get_paths() -> Result<Value, String> {
    let data_dir = get_data_dir()?;
    let attachments_dir = get_attachments_dir()?;
    Ok(json!({
        "dataDir": data_dir.to_string_lossy(),
        "attachmentsDir": attachments_dir.to_string_lossy(),
    }))
}

fn open_external(payload: &Value) -> Result<Value, String> {
    let url = payload
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or("url is required")?
        .trim()
        .to_string();
    if url.is_empty() {
        return Err("url is empty".into());
    }

    // 协议白名单：仅允许 http/https/mailto。注意取 scheme 时要 case-insensitive。
    let scheme_lower = url
        .split(':')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match scheme_lower.as_str() {
        "http" | "https" | "mailto" => {}
        other => return Err(format!("scheme not allowed: {other}")),
    }

    open::that(&url).map_err(|e| format!("open url failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

// ── 本地路径校验 ────────────────────────────────────────
//
// 所有接受路径参数的 action 共用此校验，避免被构造出的"看起来是路径、实际是 URL"
// 字符串或带跳转符的相对路径利用。规则：
// - 必须非空
// - 必须是绝对路径（Path::is_absolute）
// - 不得包含 `..` 段（杜绝相对跳转后再 canonicalize 被绕过）
// - Windows：拒绝 `\\.\` 设备命名空间（允许常规 `\\?\` 长路径前缀与 UNC `\\server\share`）

fn validate_local_path(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("path is empty".into());
    }
    let pb = PathBuf::from(trimmed);
    if !pb.is_absolute() {
        return Err(format!("path must be absolute: {trimmed}"));
    }
    for comp in pb.components() {
        if matches!(comp, std::path::Component::ParentDir) {
            return Err(format!("path contains `..`: {trimmed}"));
        }
    }
    #[cfg(windows)]
    {
        // `\\.\...` 是 Win32 device namespace，直接拒绝；`\\?\...` 是长路径前缀，保留
        if trimmed.starts_with(r"\\.\") {
            return Err("device namespace path not allowed".into());
        }
    }
    Ok(pb)
}

fn open_local_path(payload: &Value) -> Result<Value, String> {
    let path_raw = payload
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("path is required")?;
    let path = validate_local_path(path_raw)?;
    if !path.exists() {
        return Err("file not found".into());
    }
    open::that(&path).map_err(|e| format!("open path failed: {e}"))?;
    Ok(json!({ "ok": true }))
}

fn reveal_in_folder(payload: &Value) -> Result<Value, String> {
    let path_raw = payload
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("path is required")?;
    let path = validate_local_path(path_raw)?;
    if !path.exists() {
        return Err("file not found".into());
    }
    reveal_impl(&path)?;
    Ok(json!({ "ok": true }))
}

#[cfg(windows)]
fn reveal_impl(path: &Path) -> Result<(), String> {
    // explorer /select,<path>：在资源管理器中打开所在目录并选中该文件
    // 注意 explorer 的返回码在正常情况下也可能是 1，不能按 status 判
    Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn()
        .map_err(|e| format!("explorer launch failed: {e}"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn reveal_impl(path: &Path) -> Result<(), String> {
    Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn()
        .map_err(|e| format!("open -R failed: {e}"))?;
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn reveal_impl(path: &Path) -> Result<(), String> {
    // Linux：xdg-open 目标目录（无法精确 select）
    let parent = path
        .parent()
        .ok_or_else(|| "path has no parent".to_string())?;
    Command::new("xdg-open")
        .arg(parent)
        .spawn()
        .map_err(|e| format!("xdg-open failed: {e}"))?;
    Ok(())
}

fn check_paths_exist(payload: &Value) -> Result<Value, String> {
    let arr = payload
        .get("paths")
        .and_then(|v| v.as_array())
        .ok_or("paths is required")?;
    let mut missing: Vec<String> = Vec::new();
    for item in arr {
        let Some(s) = item.as_str() else { continue };
        // 这里不调 validate_local_path：传入的可能是 tmp/相对路径等，
        // 检测目的是视觉标红，失败就当作"不存在"处理
        match fs::metadata(s) {
            Ok(_) => {}
            Err(_) => missing.push(s.to_string()),
        }
    }
    Ok(json!({ "missing": missing }))
}

// ── Windows 剪贴板文件读取 ─────────────────────────────
//
// 参考 inbox.rs:1646 的既有实现，单独拆到 system 域以便前端统一调用。
// 非 Windows 平台直接返回空数组，前端以 paths.length === 0 作为"无原始路径"信号。

#[cfg(windows)]
fn read_clipboard_files() -> Result<Value, String> {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows_sys::Win32::UI::Shell::DragQueryFileW;

    const CF_HDROP: u32 = 15;

    struct ClipboardGuard;
    impl ClipboardGuard {
        fn open() -> Result<Self, String> {
            unsafe {
                if OpenClipboard(HWND::default()) == 0 {
                    return Err("open clipboard failed".into());
                }
            }
            Ok(Self)
        }
    }
    impl Drop for ClipboardGuard {
        fn drop(&mut self) {
            unsafe {
                CloseClipboard();
            }
        }
    }

    let _guard = ClipboardGuard::open()?;
    let mut paths: Vec<String> = Vec::new();
    unsafe {
        if IsClipboardFormatAvailable(CF_HDROP) == 0 {
            return Ok(json!({ "paths": paths }));
        }
        let handle = GetClipboardData(CF_HDROP);
        if handle.is_null() {
            return Ok(json!({ "paths": paths }));
        }
        let count = DragQueryFileW(handle as _, u32::MAX, std::ptr::null_mut(), 0);
        if count == 0 {
            return Ok(json!({ "paths": paths }));
        }
        for index in 0..count {
            let len = DragQueryFileW(handle as _, index, std::ptr::null_mut(), 0);
            if len == 0 {
                continue;
            }
            let mut buf = vec![0u16; len as usize + 1];
            DragQueryFileW(handle as _, index, buf.as_mut_ptr(), len + 1);
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            if !path.is_empty() {
                paths.push(path);
            }
        }
    }
    Ok(json!({ "paths": paths }))
}

#[cfg(not(windows))]
fn read_clipboard_files() -> Result<Value, String> {
    // 非 Windows 平台：暂不实现剪贴板文件路径读取
    Ok(json!({ "paths": [] as [&str; 0] }))
}

// ── 网卡 IP 列表 ────────────────────────────────────────
//
// 用于 Spotlight `;ip` 关键字命令。聚合所有可用网卡的 IPv4 / IPv6 地址。
// 同一个网卡的 IPv4 / IPv6 合并到一个对象,前端按行展示。
//
// 失败时返回空 interfaces 数组,不抛错,让前端给出友好降级。

fn local_ips() -> Result<Value, String> {
    use std::collections::BTreeMap;
    use std::net::IpAddr;

    let nics = match local_ip_address::list_afinet_netifas() {
        Ok(v) => v,
        Err(_) => {
            let empty: Vec<Value> = Vec::new();
            return Ok(json!({ "interfaces": empty }));
        }
    };

    // 按网卡名分组,保留枚举顺序
    let mut order: Vec<String> = Vec::new();
    let mut grouped: BTreeMap<String, (Vec<String>, Vec<String>)> = BTreeMap::new();
    for (name, ip) in nics {
        if !grouped.contains_key(&name) {
            order.push(name.clone());
            grouped.insert(name.clone(), (Vec::new(), Vec::new()));
        }
        let entry = grouped.get_mut(&name).expect("just inserted");
        match ip {
            IpAddr::V4(v4) => {
                let s = v4.to_string();
                if !entry.0.contains(&s) {
                    entry.0.push(s);
                }
            }
            IpAddr::V6(v6) => {
                let s = v6.to_string();
                if !entry.1.contains(&s) {
                    entry.1.push(s);
                }
            }
        }
    }

    let mut interfaces: Vec<Value> = Vec::new();
    for name in order {
        let (v4, v6) = grouped.remove(&name).unwrap_or_default();
        // 过滤掉完全无地址的网卡(理论上不会出现,防御性处理)
        if v4.is_empty() && v6.is_empty() {
            continue;
        }
        interfaces.push(json!({
            "name": name,
            "ipv4": v4,
            "ipv6": v6,
        }));
    }

    Ok(json!({ "interfaces": interfaces }))
}

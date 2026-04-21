//! 环境探测 / 跨平台系统交互域。
//!
//! 当前提供：
//! - get_paths：一次性返回 dataDir / attachmentsDir，供前端拼接 convertFileSrc 源路径
//! - open_external：通过 OS 默认程序打开外链，严格限制协议白名单（http/https/mailto），
//!                   避免在 WebView 内跳转或被钓鱼协议利用

use serde_json::{json, Value};

use super::helpers::{get_attachments_dir, get_data_dir};

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    match action {
        "get_paths" => get_paths(),
        "open_external" => open_external(payload),
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

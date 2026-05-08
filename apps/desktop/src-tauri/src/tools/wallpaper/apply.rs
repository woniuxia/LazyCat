//! `tool:wallpaper:apply` 主流程（plan §2.5 / design §7）
//!
//! 串联：dashboard_data → load_base → region/sample → ensure hidden window →
//! emit dashboard-data + color-mode → 等 canvas-ready → CapturePreview →
//! compose → persist → IDesktopWallpaper::SetWallpaper → state.write
//!
//! 全程同步实现（no async）：
//! - emit / set_wallpaper / image 编解码 本身就是同步 API
//! - canvas 握手通过 std::sync::mpsc + Tauri Listener trait 完成
//! - CapturePreview 在 `with_webview` UI-thread 闭包内调用，主线程通过 Mutex 轮询
//!
//! 该流程被 mod.rs 的 `apply` action 直接调用；调度（心跳 / debounce）见 Phase 3。

#![allow(dead_code)] // Phase 3 调度接入前部分常量未引用

use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Listener, Manager, WebviewWindow};

use crate::tools::helpers::db_conn;
use crate::tools::wallpaper::{
    capture, compose, config, dashboard_logic, data, desktop, hidden, state,
};

/// 等 canvas-ready 的最长时间。冷启动（hidden WebView 首次创建）
/// 实测 ~1.2s（plan §2 验收要求 < 1500ms），留 0.8s 余量。
const READY_TIMEOUT: Duration = Duration::from_millis(2500);
/// 等 canvas-mounted 的最长时间。仅冷启动时使用。
const MOUNTED_TIMEOUT: Duration = Duration::from_millis(2000);
/// CapturePreview 总超时（含 with_webview 调度延迟）；cap_inner 自身已有 5s 上限。
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(8);
/// 主屏 monitor 索引；阶段 1 单屏。
const MONITOR_INDEX: u32 = 0;
/// load_base_cached 的 monitor 标识（与缓存 key 对齐）。
const PRIMARY_MONITOR_ID: &str = "primary";

/// 上一次成功合成的输入 hash（dashboard + position + mode + base mtime）。
/// 0 表示无历史 / 已失效；调用 [`invalidate_input_hash`] 强制下一轮重渲。
static LAST_INPUT_HASH: AtomicU64 = AtomicU64::new(0);

/// 重置内容 hash；enable / restore / boss key cycle 后调用，避免被旧 hash 卡住。
pub fn invalidate_input_hash() {
    LAST_INPUT_HASH.store(0, Ordering::SeqCst);
}

/// `tool:wallpaper:apply` 入口；返回 `{ ok, path, method, coldStart, skipped }`。
///
/// `force = true` 时跳过 hash 去重（手动「立即刷新」/ 老板键恢复 / resume 走这里）；
/// `force = false` 用于心跳 / 事件驱动，命中相同输入时直接返回 skipped。
pub fn apply(app: &AppHandle) -> Result<Value, String> {
    apply_with_force(app, true)
}

/// 带 force 标记的入口；调度 / 事件驱动调用方走此版本传 false 启用 hash 去重。
pub fn apply_with_force(app: &AppHandle, force: bool) -> Result<Value, String> {
    let start = Instant::now();

    // 1. 拉数据 + 读配置（fast path，纯 SQL）
    let dashboard = data::dashboard_data(&Value::Null)?;
    let cfg = config::read_config();

    // 2. 取 base 路径 → 解码（命中缓存就秒回）
    let base_path = desktop::get_current_wallpaper(MONITOR_INDEX)?;
    let base = compose::load_base_cached(PRIMARY_MONITOR_ID, &base_path)?;

    // 3. 计算贴边 region + 采样色彩模式
    //    region 物理尺寸 = 逻辑 360×800 × 主屏 DPI scale；scale 取自 hidden window
    //    （hidden 模块已用同一来源），保证 base 区域大小与抓出来的 PNG 尺寸一致
    let scale = primary_scale_factor(app);
    let region_w = (compose::BASE_REGION_W as f64 * scale).round() as u32;
    let region_h = (compose::BASE_REGION_H as f64 * scale).round() as u32;
    let position = compose::Position::from_str(&cfg.position);
    let region = compose::region_for(base.width(), base.height(), position, region_w, region_h);
    let mode = compose::sample_color_mode(&base, region);

    // 3b. 内容 hash 去重（plan §3.1 v0.5 优化点 2）：输入未变化直接跳过整条
    //     compose + persist + set_wallpaper 链路，节省 IO + GPU。
    //     force=true 时（手动刷新 / 老板键恢复 / resume）跳过此判定。
    //
    //     dashboard 自带 generatedAt 时间戳，每次必不同；这里仅取 overview +
    //     todoList 两个稳定字段（口径与 dashboard_logic::compute_dashboard_hash
    //     一致），再混入 base 路径 / mtime / position / mode。
    let overview_value = dashboard.get("overview").cloned().unwrap_or(Value::Null);
    let todo_list_value: Vec<Value> = dashboard.get("todoList").and_then(|v| v.as_array().cloned()).unwrap_or_default();
    let input_hash = compute_input_hash(&overview_value, &todo_list_value, &base_path, position, mode);
    if !force && input_hash != 0 && input_hash == LAST_INPUT_HASH.load(Ordering::SeqCst) {
        return Ok(json!({
            "ok": true,
            "skipped": true,
            "reason": "no-change",
            "elapsedMs": start.elapsed().as_millis() as u64,
        }));
    }

    // 4. 确保 hidden window；记录是否冷启
    let cold_start = !hidden::is_canvas_open(app);
    let win = hidden::ensure_canvas_window(app)?;

    // 5. 注册 ready 监听器（必须在 emit 之前注册，避免 emit 后立即响应漏接）
    let (ready_tx, ready_rx) = mpsc::channel::<()>();
    let ready_handler = once_send_listener(app, "wallpaper://canvas-ready", ready_tx);

    // 5b. 冷启时还要等 canvas 挂载（Vue createApp + 事件监听就绪）
    if cold_start {
        wait_for_event(app, "wallpaper://canvas-mounted", MOUNTED_TIMEOUT)?;
    }

    // 6. 注入敏感模式标记（design §9）：自动到期则写回 false 并清除时间戳
    let privacy_mask = resolve_privacy_mask(&cfg);
    let mut dashboard_emit = dashboard.clone();
    if let Some(obj) = dashboard_emit.as_object_mut() {
        obj.insert(
            "privacyMask".into(),
            Value::Bool(privacy_mask),
        );
    }

    // 7. emit 配色 + 数据 → 前端渲染 → 2 RAF 后回 canvas-ready
    app.emit("wallpaper://color-mode", mode.as_str())
        .map_err(|e| format!("emit color-mode failed: {e}"))?;
    app.emit("wallpaper://dashboard-data", &dashboard_emit)
        .map_err(|e| format!("emit dashboard-data failed: {e}"))?;

    // 7. 等 ready；超时不 fatal，仍尝试 capture（前端可能渲染慢但已绘制）
    let ready_ok = ready_rx.recv_timeout(READY_TIMEOUT).is_ok();
    app.unlisten(ready_handler);
    if !ready_ok {
        eprintln!(
            "[wallpaper] canvas-ready timeout after {}ms, capturing anyway",
            READY_TIMEOUT.as_millis()
        );
    }

    // 8. CapturePreview 抓 PNG（with_webview 跨线程 + Mutex 轮询）
    let png = capture_window_png(&win)?;
    let info_layer =
        image::load_from_memory(&png).map_err(|e| format!("decode info-layer png: {e}"))?;

    // 9. 合成 → persist → set wallpaper
    let composed = compose::compose(&base, &info_layer, region, mode);
    let format = parse_image_format(&cfg.image_format);
    let path = compose::persist(&composed, format, cfg.keep_history_count.max(0) as usize)?;
    let method = desktop::set_wallpaper(MONITOR_INDEX, &path)?;

    // 10. 首次 apply 成功 / 回退时记录 set_method（plan §2.5）
    persist_set_method_if_first(method.as_str())?;

    // 11. 写运行时状态：成功路径清掉 burnout / last_error
    let path_str = path.to_string_lossy().to_string();
    state::write(|s| {
        s.last_rendered_path = Some(path_str.clone());
        s.last_rendered_at = Some(now_iso());
        s.last_error = None;
        s.burnout = 0;
    });

    // 12. 落地内容 hash 供下次去重比对
    if input_hash != 0 {
        LAST_INPUT_HASH.store(input_hash, Ordering::SeqCst);
    }

    Ok(json!({
        "ok": true,
        "path": path_str,
        "method": method.as_str(),
        "coldStart": cold_start,
        "elapsedMs": start.elapsed().as_millis() as u64,
        "readyOk": ready_ok,
        "skipped": false,
    }))
}

// ── 内部 ──────────────────────────────────────────

/// 用 mpsc::Sender 触发一次性事件；listener 内部 send 后自然关闭。
fn once_send_listener(
    app: &AppHandle,
    event: &str,
    tx: mpsc::Sender<()>,
) -> tauri::EventId {
    let tx = Mutex::new(Some(tx));
    app.listen(event, move |_evt| {
        if let Ok(mut guard) = tx.lock() {
            if let Some(t) = guard.take() {
                let _ = t.send(());
            }
        }
    })
}

/// 阻塞等指定事件出现一次（基于 once_send_listener 的薄封装）。
fn wait_for_event(app: &AppHandle, event: &str, timeout: Duration) -> Result<(), String> {
    let (tx, rx) = mpsc::channel::<()>();
    let handler = once_send_listener(app, event, tx);
    let res = rx.recv_timeout(timeout);
    app.unlisten(handler);
    res.map_err(|_| format!("wait_for_event {event} timeout after {}ms", timeout.as_millis()))
        .map(|_| ())
}

/// 跨线程同步抓 PNG：with_webview 把闭包丢到 UI 线程，主线程轮询 Mutex 拿结果。
fn capture_window_png(win: &WebviewWindow) -> Result<Vec<u8>, String> {
    let slot: Arc<Mutex<Option<Result<Vec<u8>, String>>>> = Arc::new(Mutex::new(None));
    let done = Arc::new(AtomicBool::new(false));

    let slot_cb = slot.clone();
    let done_cb = done.clone();
    win.with_webview(move |webview| {
        let res = capture::capture_inner(webview);
        if let Ok(mut g) = slot_cb.lock() {
            *g = Some(res);
        }
        done_cb.store(true, Ordering::SeqCst);
    })
    .map_err(|e| format!("with_webview failed: {e}"))?;

    let start = Instant::now();
    while !done.load(Ordering::SeqCst) {
        if start.elapsed() > CAPTURE_TIMEOUT {
            return Err(format!(
                "capture_window_png timeout after {}ms",
                CAPTURE_TIMEOUT.as_millis()
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    let mut guard = slot.lock().map_err(|e| format!("slot poisoned: {e}"))?;
    guard
        .take()
        .unwrap_or_else(|| Err("capture_window_png: slot empty after done flag".into()))
}

/// 仅当 `wallpaper.original_set_method` 为空时写入；之后由 restore 路径读用。
fn persist_set_method_if_first(method: &str) -> Result<(), String> {
    let conn = db_conn()?;
    let existing = config::read_string(&conn, config::KEY_ORIGINAL_SET_METHOD).unwrap_or_default();
    if !existing.is_empty() {
        return Ok(());
    }
    config::set_string(config::KEY_ORIGINAL_SET_METHOD, method)
}

fn parse_image_format(raw: &str) -> compose::ImageFormat {
    match raw.to_ascii_lowercase().as_str() {
        "png" => compose::ImageFormat::Png,
        _ => compose::ImageFormat::Jpeg,
    }
}

fn primary_scale_factor(app: &AppHandle) -> f64 {
    app.get_webview_window("main")
        .and_then(|w| w.primary_monitor().ok().flatten())
        .map(|m| m.scale_factor())
        .unwrap_or(1.0)
}

fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

/// 判定当前是否处于敏感模式；过期则同步写回 wallpaper.privacy_mask=false +
/// 清空 wallpaper.privacy_mask_until，避免下次 apply 重复检查。
fn resolve_privacy_mask(cfg: &config::WallpaperConfig) -> bool {
    if !cfg.privacy_mask {
        return false;
    }
    let Some(until) = cfg.privacy_mask_until.as_deref() else {
        return true;
    };
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(until) else {
        return true;
    };
    if chrono::Utc::now() <= dt.with_timezone(&chrono::Utc) {
        return true;
    }
    // 已过期：清零（写失败仅 log，不阻塞 apply）
    if let Err(e) = config::set_string(config::KEY_PRIVACY_MASK, "false") {
        eprintln!("[wallpaper] auto-clear privacy_mask failed: {e}");
    }
    if let Err(e) = config::set_string(config::KEY_PRIVACY_MASK_UNTIL, "") {
        eprintln!("[wallpaper] auto-clear privacy_mask_until failed: {e}");
    }
    false
}

/// 计算 apply 输入哈希（用于内容去重）。组成：
/// - overview + 排序后的 todoList（复用 dashboard_logic::compute_dashboard_hash，
///   排除 dashboard.generatedAt 时间戳，保证内容相同时哈希稳定）
/// - base 文件路径 + mtime（user 手改壁纸即变）
/// - 贴边位置 + 颜色模式（配置 / 采样结果）
///
/// 取 blake3 输出前 8 字节作为 u64；结果为 0 时映射到 1 避免与 sentinel 冲突。
fn compute_input_hash(
    overview: &Value,
    todo_list: &[Value],
    base_path: &std::path::Path,
    position: compose::Position,
    mode: compose::ColorMode,
) -> u64 {
    let dashboard_hex = dashboard_logic::compute_dashboard_hash(overview, todo_list);
    let mtime_secs = std::fs::metadata(base_path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut hasher = blake3::Hasher::new();
    hasher.update(dashboard_hex.as_bytes());
    hasher.update(b"|");
    hasher.update(base_path.to_string_lossy().as_bytes());
    hasher.update(b"|");
    hasher.update(&mtime_secs.to_le_bytes());
    hasher.update(b"|");
    hasher.update(position.as_str().as_bytes());
    hasher.update(b"|");
    hasher.update(mode.as_str().as_bytes());

    let bytes = hasher.finalize();
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes.as_bytes()[..8]);
    let result = u64::from_le_bytes(buf);
    if result == 0 { 1 } else { result }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fake_base() -> PathBuf {
        PathBuf::from("/nonexistent/wallpaper-apply-test/base.png")
    }

    #[test]
    fn hash_ignores_generated_at_timestamp() {
        let p = fake_base();
        let overview = json!({ "completedToday": 1, "totalToday": 5 });
        let todos = vec![json!({ "id": "pm:1", "title": "x" })];
        let a = compute_input_hash(&overview, &todos, &p, compose::Position::Right, compose::ColorMode::Light);
        let b = compute_input_hash(&overview, &todos, &p, compose::Position::Right, compose::ColorMode::Light);
        assert_eq!(a, b);
    }

    #[test]
    fn hash_changes_with_todo_list() {
        let p = fake_base();
        let overview = json!({});
        let h1 = compute_input_hash(&overview, &[json!({ "id": "a" })], &p, compose::Position::Right, compose::ColorMode::Light);
        let h2 = compute_input_hash(&overview, &[json!({ "id": "b" })], &p, compose::Position::Right, compose::ColorMode::Light);
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_changes_with_overview() {
        let p = fake_base();
        let todos: Vec<Value> = Vec::new();
        let h1 = compute_input_hash(&json!({ "p0Pending": 0 }), &todos, &p, compose::Position::Right, compose::ColorMode::Light);
        let h2 = compute_input_hash(&json!({ "p0Pending": 1 }), &todos, &p, compose::Position::Right, compose::ColorMode::Light);
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_changes_with_position_and_mode() {
        let p = fake_base();
        let overview = json!({});
        let todos: Vec<Value> = Vec::new();
        let right_light = compute_input_hash(&overview, &todos, &p, compose::Position::Right, compose::ColorMode::Light);
        let left_light = compute_input_hash(&overview, &todos, &p, compose::Position::Left, compose::ColorMode::Light);
        let right_dark = compute_input_hash(&overview, &todos, &p, compose::Position::Right, compose::ColorMode::Dark);
        assert_ne!(right_light, left_light);
        assert_ne!(right_light, right_dark);
    }

    #[test]
    fn hash_never_returns_sentinel_zero() {
        let p = fake_base();
        let h = compute_input_hash(&json!({}), &[], &p, compose::Position::Right, compose::ColorMode::Light);
        assert_ne!(h, 0);
    }
}

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
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Listener, Manager, WebviewWindow};

use crate::tools::helpers::db_conn;
use crate::tools::wallpaper::{
    capture, compose, config, data, desktop, hidden, state,
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

/// `tool:wallpaper:apply` 入口；返回 `{ ok, path, method, coldStart }`。
pub fn apply(app: &AppHandle) -> Result<Value, String> {
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

    // 6. emit 配色 + 数据 → 前端渲染 → 2 RAF 后回 canvas-ready
    app.emit("wallpaper://color-mode", mode.as_str())
        .map_err(|e| format!("emit color-mode failed: {e}"))?;
    app.emit("wallpaper://dashboard-data", &dashboard)
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

    Ok(json!({
        "ok": true,
        "path": path_str,
        "method": method.as_str(),
        "coldStart": cold_start,
        "elapsedMs": start.elapsed().as_millis() as u64,
        "readyOk": ready_ok,
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

//! Hidden WebView 生命周期管理（plan §2.4 / design §7.5）
//!
//! 提供按需创建、按需销毁、连续渲染失败后强制重建的原语；
//! 「什么时候调用」由 apply / scheduler 决定（Phase 2.4 / Phase 3），本模块
//! 只负责创建 / 销毁 / 状态查询，不耦合调度逻辑。
//!
//! ## 设计要点
//!
//! - 窗口 label 固定 `wallpaper-canvas`，与 PoC 的 `wallpaper-poc-canvas` 隔离
//! - 物理尺寸 = 360×800（逻辑像素）× 主屏 DPI scale；避免 viewport 裁剪
//! - `visible=false` + `skip_taskbar=true` + `decorations=false` 保持隐身
//! - 连续渲染失败 N 次（`BURNOUT_REBUILD_THRESHOLD`）由调度方负责调
//!   [`rebuild_on_burnout`]，本模块不持有失败计数器

#![allow(dead_code)] // Phase 2.4+ 由 apply 路径接入

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// hidden WebView 窗口 label。
pub const CANVAS_LABEL: &str = "wallpaper-canvas";

/// 信息层逻辑宽（dp）。与 [`crate::tools::wallpaper::compose::BASE_REGION_W`] 保持一致。
const LOGICAL_WIDTH: f64 = 360.0;
/// 信息层逻辑高（dp）。与 [`crate::tools::wallpaper::compose::BASE_REGION_H`] 保持一致。
const LOGICAL_HEIGHT: f64 = 800.0;

/// 连续渲染失败达到此阈值时由调度方触发 [`rebuild_on_burnout`]；
/// 此处只做常量定义，计数与触发由 `state` / `scheduler` 维护。
pub const BURNOUT_REBUILD_THRESHOLD: u32 = 3;

/// 取主屏 scale_factor；查询失败回退 1.0（不抛错，避免阻塞渲染）。
///
/// 通过 `main` 窗口取主屏；在 main 窗口尚未创建时退化到 1.0，
/// 此时通常也不会真正去创建 hidden window（`enable` 路径在 main 之后才触发）。
fn primary_scale_factor(app: &AppHandle) -> f64 {
    app.get_webview_window("main")
        .and_then(|w| w.primary_monitor().ok().flatten())
        .map(|m| m.scale_factor())
        .unwrap_or(1.0)
}

/// 按需创建 hidden WebView；已存在时直接返回句柄。
///
/// 失败原因：tauri 自身错误（WebView2 不可用、URL 解析失败等）。
/// 创建后窗口处于 `visible=false`，外部需通过 [`crate::tools::wallpaper::capture`]
/// 在 `with_webview` 闭包内调用 CapturePreview 抓帧。
pub fn ensure_canvas_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(w) = app.get_webview_window(CANVAS_LABEL) {
        return Ok(w);
    }

    let scale = primary_scale_factor(app);
    let url = WebviewUrl::App("index.html?view=wallpaper-canvas".into());

    WebviewWindowBuilder::new(app, CANVAS_LABEL, url)
        .title("Wallpaper Canvas")
        .inner_size(LOGICAL_WIDTH * scale, LOGICAL_HEIGHT * scale)
        .visible(false)
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .build()
        .map_err(|e| format!("ensure_canvas_window build failed: {e}"))
}

/// 销毁 hidden WebView；不存在时 no-op，不报错。
pub fn destroy_canvas_window(app: &AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(CANVAS_LABEL) {
        w.close()
            .map_err(|e| format!("destroy_canvas_window close failed: {e}"))?;
    }
    Ok(())
}

/// 查询 hidden WebView 是否已存在；不创建。供调度 / 状态卡片用。
pub fn is_canvas_open(app: &AppHandle) -> bool {
    app.get_webview_window(CANVAS_LABEL).is_some()
}

/// 连续渲染失败达阈值时的恢复策略：先销毁，下次 [`ensure_canvas_window`] 自动重建。
///
/// 防止 WebView 内存泄漏 / 长期黑帧（design §7.5 末行）。
pub fn rebuild_on_burnout(app: &AppHandle) -> Result<(), String> {
    destroy_canvas_window(app)
}

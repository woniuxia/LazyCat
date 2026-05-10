//! 桌面挂件窗口生命周期（design 重构版）
//!
//! 替代旧 `hidden.rs`：不再做 PNG 抓帧，挂件本身就是用户最终看到的窗口。
//!
//! 四种可视状态（由 session.rs 定义）：
//! - `Peek`：贴右边缘仅露 8px 提示条（默认稳态，不抢镜）
//! - `Full`：完全展开 360px（鼠标靠近右边缘触发）
//! - `Hidden`：完全不可见（老板键 / 全屏 / 锁屏 触发）
//! - `Windowless`：窗口未创建/已销毁
//!
//! 全程 `always_on_top = true`（Win+D 唯一可靠豁免方案）；
//! 通过后台线程 `GetCursorPos` 80ms 轮询驱动 Peek↔Full 状态机；
//! 用户在 Full 状态拖拽手柄沿 Y 轴移动（Moved 事件持久化 KEY_WIDGET_Y）。

#![allow(dead_code)]

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tauri::{
    AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

use crate::tools::widget::{config, session};
use crate::tools::widget::diagnostics::WidgetEvent;

pub use session::VisualState;

/// 挂件窗口 label。
pub const WIDGET_LABEL: &str = "widget";

const LOGICAL_W: f64 = 360.0;
const LOGICAL_H: f64 = 800.0;
const PEEK_WIDTH: f64 = 8.0;
const HOVER_TRIGGER: f64 = 8.0;
const COLLAPSE_DELAY_MS: u64 = 800;
const CURSOR_POLL_MS: u64 = 80;
const HOVER_TOLERANCE_PX: f64 = 16.0;

// ── 公开 API ─────────────────────────────────────

/// 检查挂件窗口是否存在（兼容旧 API，新代码直接用 session().is_window_open()）。
pub fn is_open(app: &AppHandle) -> bool {
    app.get_webview_window(WIDGET_LABEL).is_some()
}

/// 创建挂件（visible=false 起步，初始化完毕后强制定位 + show 到 Peek 状态）。
/// 已存在则直接返回句柄。
pub fn ensure(app: &AppHandle) -> Result<WebviewWindow, String> {
    let s = session::session();
    if s.is_window_open() {
        if let Some(w) = s.window_handle() {
            return Ok(w);
        }
    }

    eprintln!("[widget] widget: building widget window");
    let start = Instant::now();

    let url = WebviewUrl::App("index.html?view=widget-canvas".into());
    let win = WebviewWindowBuilder::new(app, WIDGET_LABEL, url)
        .title("LazyCat Widget")
        .inner_size(LOGICAL_W, LOGICAL_H)
        .transparent(true)
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .build()
        .map_err(|e| format!("widget build failed: {e}"))?;

    apply_win32_styles(&win);
    install_position_listener(&win);

    // 存储窗口 + 自增 generation
    s.set_window(win.clone());
    s.set_ready_deadline();

    // 通过 transition 设置 Peek 状态（positioning + show）
    if let Err(e) = s.transition(app, VisualState::Peek) {
        eprintln!("[widget] widget: initial transition to Peek failed: {e}");
    }

    s.record(WidgetEvent::WindowCreated {
        elapsed_ms: start.elapsed().as_millis() as u64,
    });

    start_background_loops_once(app);
    Ok(win)
}

/// 销毁挂件（通过 session.transition 统一治理）。
pub fn destroy(app: &AppHandle) -> Result<(), String> {
    session::session().transition(app, VisualState::Windowless)
}

/// 切换挂件可见状态（兼容旧 API，新代码直接用 session().transition()）。
pub fn set_state(app: &AppHandle, target: VisualState) -> Result<(), String> {
    session::session().transition(app, target)
}

// ── 内部：窗口创建辅助 ─────────────────────────────

#[cfg(windows)]
fn apply_win32_styles(win: &WebviewWindow) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    let Ok(hwnd) = win.hwnd() else {
        eprintln!("[widget] widget: hwnd unavailable, skip ex-style");
        return;
    };
    unsafe {
        let cur = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let extra = (WS_EX_NOACTIVATE.0 as isize) | (WS_EX_TOOLWINDOW.0 as isize);
        let new = cur | extra;
        if new != cur {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new);
        }
    }
}

#[cfg(not(windows))]
fn apply_win32_styles(_win: &WebviewWindow) {}

// ── 内部：位置计算 ─────────────────────────────────

/// 取主屏物理尺寸 + scale_factor。失败回退 (1920, 1080, 1.0)。
fn primary_screen_phys(app: &AppHandle) -> (f64, f64, f64) {
    if let Some(main) = app.get_webview_window("main") {
        if let Ok(Some(monitor)) = main.primary_monitor() {
            let size = monitor.size();
            return (
                size.width as f64,
                size.height as f64,
                monitor.scale_factor(),
            );
        }
    }
    (1920.0, 1080.0, 1.0)
}

/// 读取持久化的 Y（物理像素），越界自动 clamp，无值时居中。
pub(crate) fn restore_y_phys(screen_h_phys: f64, scale: f64) -> i32 {
    let widget_h_phys = LOGICAL_H * scale;
    let max_y = (screen_h_phys - widget_h_phys).max(0.0) as i32;
    let center = ((screen_h_phys - widget_h_phys) / 2.0).max(0.0) as i32;

    let Ok(conn) = crate::tools::helpers::db_conn() else {
        return center;
    };
    match config::read_string(&conn, config::KEY_WIDGET_Y) {
        Some(s) => match s.parse::<i32>() {
            Ok(y) => y.clamp(0, max_y),
            Err(_) => center,
        },
        None => center,
    }
}

/// 按 VisualState 计算 X 并 set_position（由 session.transition 调用）。
pub(crate) fn apply_position(
    app: &AppHandle,
    win: &WebviewWindow,
    vstate: VisualState,
) -> Result<(), String> {
    let (screen_w, screen_h, scale) = primary_screen_phys(app);
    let widget_w_phys = LOGICAL_W * scale;
    let peek_phys = PEEK_WIDTH * scale;
    let y = restore_y_phys(screen_h, scale);
    let x = match vstate {
        VisualState::Peek => (screen_w - peek_phys).round() as i32,
        VisualState::Full => (screen_w - widget_w_phys).round() as i32,
        VisualState::Hidden | VisualState::Windowless => return Ok(()),
    };
    win.set_position(PhysicalPosition::new(x, y))
        .map_err(|e| format!("set_position failed: {e}"))?;
    Ok(())
}

// ── 内部：事件监听 ─────────────────────────────────

/// 监听 WindowEvent::Moved；仅在 Full 状态记录新 Y 到 session.pending_y（无锁）。
fn install_position_listener(win: &WebviewWindow) {
    win.on_window_event(|evt| {
        if let WindowEvent::Moved(pos) = evt {
            if session::session().visual_state() != VisualState::Full {
                return;
            }
            session::session().set_pending_y(pos.y);
        }
    });
}

// ── 内部：后台线程 ─────────────────────────────────

fn start_background_loops_once(app: &AppHandle) {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }
    let app_for_cursor = app.clone();
    std::thread::spawn(move || cursor_loop(app_for_cursor));

    std::thread::spawn(move || flush_loop());
    eprintln!("[widget] widget: background loops started");
}

/// 把 session.pending_y 中的最新值写到 user_settings；200ms 检查一次。
fn flush_loop() {
    let mut last_flushed: Option<i32> = None;
    loop {
        std::thread::sleep(Duration::from_millis(200));
        let s = session::session();
        let cur = s.pending_y_val();
        if cur == i32::MIN {
            continue;
        }
        if last_flushed == Some(cur) {
            continue;
        }
        if let Err(e) = config::set_string(config::KEY_WIDGET_Y, &cur.to_string()) {
            eprintln!("[widget] widget: persist Y failed: {e}");
            continue;
        }
        last_flushed = Some(cur);
    }
}

/// 光标轮询：80ms GetCursorPos 驱动 Peek↔Full。
///
/// 每次迭代捕获 window_generation，在 transition 前校验，不匹配则跳过。
/// 整个循环体用 catch_unwind 包裹，panic 后记录 Event 并自动恢复。
#[cfg(windows)]
fn cursor_loop(app: AppHandle) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut last_in_widget = Instant::now();
    let mut was_in_full = false;

    loop {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            std::thread::sleep(Duration::from_millis(CURSOR_POLL_MS));

            let s = session::session();
            let cur = s.visual_state();
            if cur == VisualState::Hidden || cur == VisualState::Windowless {
                return;
            }
            if !s.is_window_open() {
                return;
            }

            // 捕获 generation，transition 前校验
            let gen = s.generation();

            let (screen_w, screen_h, scale) = primary_screen_phys(&app);
            let widget_w_phys = LOGICAL_W * scale;
            let widget_h_phys = LOGICAL_H * scale;
            let trigger_phys = HOVER_TRIGGER * scale;
            let tolerance_phys = HOVER_TOLERANCE_PX * scale;
            let y_top = restore_y_phys(screen_h, scale) as f64;
            let y_bot = y_top + widget_h_phys;

            let mut p = POINT::default();
            unsafe {
                if GetCursorPos(&mut p).is_err() {
                    return;
                }
            }
            let cx = p.x as f64;
            let cy = p.y as f64;

            match cur {
                VisualState::Peek => {
                    let in_trigger =
                        cx >= screen_w - trigger_phys && cy >= y_top && cy < y_bot;
                    if in_trigger {
                        if s.generation() == gen {
                            let _ = s.transition(&app, VisualState::Full);
                        }
                        last_in_widget = Instant::now();
                        was_in_full = true;
                    }
                }
                VisualState::Full => {
                    let widget_left = screen_w - widget_w_phys;
                    let in_rect = cx >= widget_left - tolerance_phys
                        && cx <= screen_w
                        && cy >= y_top - tolerance_phys
                        && cy <= y_bot + tolerance_phys;
                    if in_rect {
                        last_in_widget = Instant::now();
                        was_in_full = true;
                    } else if was_in_full
                        && last_in_widget.elapsed()
                            >= Duration::from_millis(COLLAPSE_DELAY_MS)
                    {
                        if s.generation() == gen {
                            let _ = s.transition(&app, VisualState::Peek);
                        }
                        was_in_full = false;
                    }
                }
                _ => {}
            }
        }));

        if let Err(panic_info) = result {
            let msg = format!(
                "cursor_loop panic: {:?}",
                panic_info
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| panic_info
                        .downcast_ref::<String>()
                        .cloned())
                    .unwrap_or_else(|| "unknown".into())
            );
            eprintln!("[widget] {msg}");
            session::session().record(WidgetEvent::Error {
                source: "cursor_loop".into(),
                message: msg,
            });
            std::thread::sleep(Duration::from_millis(1000));
        }
    }
}

#[cfg(not(windows))]
fn cursor_loop(_app: AppHandle) {}

// ── 测试 ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_state_round_trip() {
        for v in [
            VisualState::Peek,
            VisualState::Full,
            VisualState::Hidden,
            VisualState::Windowless,
        ] {
            assert_eq!(VisualState::from_u8(v as u8), v);
        }
    }

    #[test]
    fn visual_state_as_str() {
        assert_eq!(VisualState::Peek.as_str(), "peek");
        assert_eq!(VisualState::Full.as_str(), "full");
        assert_eq!(VisualState::Hidden.as_str(), "hidden");
    }

    #[test]
    fn visual_state_unknown_byte_falls_back_to_peek() {
        assert_eq!(VisualState::from_u8(255), VisualState::Peek);
    }
}
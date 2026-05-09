//! 桌面挂件窗口生命周期（design 重构版）
//!
//! 替代旧 `hidden.rs`：不再做 PNG 抓帧，挂件本身就是用户最终看到的窗口。
//!
//! 三种可视状态：
//! - `Peek`：贴右边缘仅露 8px 提示条（默认稳态，不抢镜）
//! - `Full`：完全展开 360px（鼠标靠近右边缘触发）
//! - `Hidden`：完全不可见（老板键 / 全屏 / 锁屏 触发）
//!
//! 全程 `always_on_top = true`（Win+D 唯一可靠豁免方案）；
//! 通过后台线程 `GetCursorPos` 80ms 轮询驱动 Peek↔Full 状态机；
//! 用户在 Full 状态拖拽手柄沿 Y 轴移动（Moved 事件持久化 KEY_WIDGET_Y）。

#![allow(dead_code)]

use std::sync::atomic::{AtomicI32, AtomicU8, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tauri::{
    AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

use crate::tools::wallpaper::config;

/// 挂件窗口 label（与旧 hidden 的 "wallpaper-canvas" 区分）。
pub const WIDGET_LABEL: &str = "wallpaper-widget";

const LOGICAL_W: f64 = 360.0;
const LOGICAL_H: f64 = 800.0;
/// Peek 态贴边露出的提示条宽（逻辑像素）。
const PEEK_WIDTH: f64 = 8.0;
/// 鼠标进入屏幕右边缘多少像素内触发展开（逻辑像素，要 >= PEEK_WIDTH）。
const HOVER_TRIGGER: f64 = 8.0;
/// 鼠标离开挂件 rect 后多久收回 Peek。
const COLLAPSE_DELAY_MS: u64 = 800;
/// GetCursorPos 轮询间隔（80ms ≈ 12.5Hz，单核占用 < 0.1%）。
const CURSOR_POLL_MS: u64 = 80;
/// Full 状态下挂件 rect 的 hover 容差（避免边缘抖动）。
const HOVER_TOLERANCE_PX: f64 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualState {
    Peek,
    Full,
    Hidden,
}

impl VisualState {
    fn as_u8(self) -> u8 {
        match self {
            Self::Peek => 0,
            Self::Full => 1,
            Self::Hidden => 2,
        }
    }
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Full,
            2 => Self::Hidden,
            _ => Self::Peek,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Peek => "peek",
            Self::Full => "full",
            Self::Hidden => "hidden",
        }
    }
}

static CURRENT_STATE: AtomicU8 = AtomicU8::new(0); // default Peek
/// 缓存最近一次 Moved 报告的 Y（物理像素）；后台线程定期 flush 到 SQL。
static PENDING_Y: AtomicI32 = AtomicI32::new(i32::MIN);

pub fn snapshot_state() -> VisualState {
    VisualState::from_u8(CURRENT_STATE.load(Ordering::SeqCst))
}

fn store_state(s: VisualState) {
    CURRENT_STATE.store(s.as_u8(), Ordering::SeqCst);
}

pub fn is_open(app: &AppHandle) -> bool {
    app.get_webview_window(WIDGET_LABEL).is_some()
}

/// 创建挂件（visible=false 起步，初始化完毕后强制定位 + show 到 Peek 状态）。
/// 已存在则直接返回句柄。
pub fn ensure(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(w) = app.get_webview_window(WIDGET_LABEL) {
        return Ok(w);
    }
    eprintln!("[wallpaper] widget: building widget window");

    let url = WebviewUrl::App("index.html?view=wallpaper-canvas".into());
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

    store_state(VisualState::Peek);
    if let Err(e) = apply_position(app, &win, VisualState::Peek) {
        eprintln!("[wallpaper] widget: initial apply_position failed: {e}");
    }
    win.show().map_err(|e| format!("widget show failed: {e}"))?;

    start_background_loops_once(app);
    Ok(win)
}

/// 销毁挂件；在独立线程关 close() 避免主线程 sync 命令死锁（同 hidden.rs 老办法）。
pub fn destroy(app: &AppHandle) -> Result<(), String> {
    store_state(VisualState::Hidden);
    if let Some(w) = app.get_webview_window(WIDGET_LABEL) {
        eprintln!("[wallpaper] widget: scheduling close");
        std::thread::spawn(move || match w.close() {
            Ok(()) => eprintln!("[wallpaper] widget: close ok"),
            Err(e) => eprintln!("[wallpaper] widget: close failed: {e}"),
        });
    }
    Ok(())
}

/// 切换挂件可见状态。
///
/// - target == 当前 → 幂等返回
/// - target == Hidden → win.hide()
/// - target == Peek/Full → 若之前 Hidden 先 win.show()，再 set_position 到目标位置
pub fn set_state(app: &AppHandle, target: VisualState) -> Result<(), String> {
    let cur = snapshot_state();
    if cur == target {
        return Ok(());
    }
    let win = app
        .get_webview_window(WIDGET_LABEL)
        .ok_or("widget not open")?;

    eprintln!(
        "[wallpaper] widget: state {} → {}",
        cur.as_str(),
        target.as_str(),
    );

    match target {
        VisualState::Hidden => {
            store_state(VisualState::Hidden);
            let _ = win.hide();
        }
        VisualState::Peek | VisualState::Full => {
            if cur == VisualState::Hidden {
                let _ = win.show();
            }
            store_state(target);
            apply_position(app, &win, target)?;
        }
    }
    Ok(())
}

// ── 内部 ─────────────────────────────────────────

#[cfg(windows)]
fn apply_win32_styles(win: &WebviewWindow) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    let Ok(hwnd) = win.hwnd() else {
        eprintln!("[wallpaper] widget: hwnd unavailable, skip ex-style");
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
fn restore_y_phys(screen_h_phys: f64, scale: f64) -> i32 {
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

fn apply_position(
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
        VisualState::Hidden => return Ok(()),
    };
    win.set_position(PhysicalPosition::new(x, y))
        .map_err(|e| format!("set_position failed: {e}"))?;
    Ok(())
}

/// 监听 WindowEvent::Moved；仅在 Full 状态记录新 Y 到 PENDING_Y atomic（无锁）。
/// 实际 SQL 写入由后台 flush 线程做 200ms 节流，避免拖动期间高频写库。
fn install_position_listener(win: &WebviewWindow) {
    win.on_window_event(|evt| {
        if let WindowEvent::Moved(pos) = evt {
            // Peek 的 set_position 也会触发 Moved，但状态此时已是 Peek/切换中，
            // 我们只在用户实际拖拽（Full 稳态）时记录 Y。
            if snapshot_state() != VisualState::Full {
                return;
            }
            PENDING_Y.store(pos.y, Ordering::SeqCst);
        }
    });
}

fn start_background_loops_once(app: &AppHandle) {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }
    let app_for_cursor = app.clone();
    std::thread::spawn(move || cursor_loop(app_for_cursor));

    std::thread::spawn(move || flush_loop());
    eprintln!("[wallpaper] widget: background loops started");
}

/// 把 PENDING_Y 中的最新值写到 user_settings；200ms 检查一次，
/// 写库前与 db 现存值比对避免无效写。
fn flush_loop() {
    let mut last_flushed: Option<i32> = None;
    loop {
        std::thread::sleep(Duration::from_millis(200));
        let cur = PENDING_Y.load(Ordering::SeqCst);
        if cur == i32::MIN {
            continue;
        }
        if last_flushed == Some(cur) {
            continue;
        }
        if let Err(e) = config::set_string(config::KEY_WIDGET_Y, &cur.to_string()) {
            eprintln!("[wallpaper] widget: persist Y failed: {e}");
            continue;
        }
        last_flushed = Some(cur);
    }
}

#[cfg(windows)]
fn cursor_loop(app: AppHandle) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut last_in_widget = Instant::now();
    let mut was_in_full = false;

    loop {
        std::thread::sleep(Duration::from_millis(CURSOR_POLL_MS));

        let cur = snapshot_state();
        if cur == VisualState::Hidden {
            continue;
        }
        if !is_open(&app) {
            continue;
        }

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
                continue;
            }
        }
        let cx = p.x as f64;
        let cy = p.y as f64;

        match cur {
            VisualState::Peek => {
                // 鼠标在屏幕最右 trigger_phys 像素 + Y 在挂件 band 内 → 展开
                let in_trigger = cx >= screen_w - trigger_phys
                    && cy >= y_top
                    && cy < y_bot;
                if in_trigger {
                    let _ = set_state(&app, VisualState::Full);
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
                    && last_in_widget.elapsed() >= Duration::from_millis(COLLAPSE_DELAY_MS)
                {
                    let _ = set_state(&app, VisualState::Peek);
                    was_in_full = false;
                }
            }
            VisualState::Hidden => {}
        }
    }
}

#[cfg(not(windows))]
fn cursor_loop(_app: AppHandle) {
    // 非 Windows：状态机不工作；挂件保持初始 Peek 位置（仅供编译通过）
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visual_state_round_trip() {
        for v in [VisualState::Peek, VisualState::Full, VisualState::Hidden] {
            assert_eq!(VisualState::from_u8(v.as_u8()), v);
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

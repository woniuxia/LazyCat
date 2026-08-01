use serde::Serialize;
use serde_json::json;
use tauri::{
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Window,
    WindowEvent,
};

use crate::events::{
    EVENT_HOTKEY_NAVIGATE, EVENT_MAIN_WINDOW_TOGGLE, EVENT_QUICK_CAPTURE_RESET,
    EVENT_SPOTLIGHT_RESET,
};
use crate::global_notification::{GLOBAL_NOTIFICATION_LABEL, GLOBAL_NOTIFICATION_TITLE};
use crate::reference_card;
use crate::tools;

pub(crate) fn reveal_main_window(app: &tauri::AppHandle) {
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

pub(crate) const MAIN_WINDOW_LABEL: &str = "main";
const MAIN_WINDOW_TITLE: &str = "Lazycat 懒猫";
const POMODORO_PROMPT_MARGIN: i64 = 16;

const POMODORO_PROMPT_LABEL: &str = "pomodoro-prompt";
const POMODORO_PROMPT_TITLE: &str = "番茄钟";
const POMODORO_PROMPT_WIDTH: i64 = 420;
const POMODORO_PROMPT_HEIGHT: i64 = 260;
const POMODORO_PROMPT_VIEW_SCRIPT: &str = r#"
window.__LAZYCAT_VIEW__ = 'pomodoro-prompt';
if (!window.location.search.includes('view=pomodoro-prompt')) {
  const hash = window.location.hash || '';
  window.history.replaceState(window.history.state, '', `${window.location.pathname}?view=pomodoro-prompt${hash}`);
}
"#;

const QUICK_CAPTURE_LABEL: &str = "quick-capture";
const QUICK_CAPTURE_TITLE: &str = "快速捕获";
const QUICK_CAPTURE_WIDTH: i64 = 520;
const QUICK_CAPTURE_HEIGHT: i64 = 56;
const QUICK_CAPTURE_VIEW_SCRIPT: &str = r#"
window.__LAZYCAT_VIEW__ = 'quick-capture';
if (!window.location.search.includes('view=quick-capture')) {
  const hash = window.location.hash || '';
  window.history.replaceState(window.history.state, '', `${window.location.pathname}?view=quick-capture${hash}`);
}
"#;

pub(crate) const SPOTLIGHT_LABEL: &str = "spotlight";
const SPOTLIGHT_TITLE: &str = "Spotlight";
const SPOTLIGHT_WIDTH: i64 = 560;
const SPOTLIGHT_HEIGHT: i64 = 420;

pub(crate) fn expected_window_title(window_label: &str) -> Option<&'static str> {
    if window_label.starts_with(reference_card::REFERENCE_CARD_PREFIX) {
        return Some(reference_card::REFERENCE_CARD_TITLE);
    }
    match window_label {
        MAIN_WINDOW_LABEL => Some(MAIN_WINDOW_TITLE),
        GLOBAL_NOTIFICATION_LABEL => Some(GLOBAL_NOTIFICATION_TITLE),
        POMODORO_PROMPT_LABEL => Some(POMODORO_PROMPT_TITLE),
        QUICK_CAPTURE_LABEL => Some(QUICK_CAPTURE_TITLE),
        SPOTLIGHT_LABEL => Some(SPOTLIGHT_TITLE),
        _ => None,
    }
}

pub(crate) fn handle_window_event(window: &Window, event: &WindowEvent) {
    match event {
        WindowEvent::CloseRequested { api, .. } => {
            if window.label() != MAIN_WINDOW_LABEL {
                reference_card::on_window_closed(window.label());
                tools::access_path_diagnostics::runtime::on_window_closed(window.label());
                return;
            }

            let close_to_tray = match tools::helpers::db_conn() {
                Ok(conn) => {
                    let value: Result<String, _> = conn.query_row(
                        "SELECT value FROM user_settings WHERE key = ?1",
                        ["close_to_tray"],
                        |row| row.get(0),
                    );
                    value.unwrap_or_else(|_| "true".to_string()) == "true"
                }
                Err(_) => true,
            };

            if close_to_tray {
                api.prevent_close();
                tools::vault::force_lock();
                let _ = window.hide();
            } else {
                tools::access_path_diagnostics::runtime::on_window_closed(window.label());
            }
        }
        WindowEvent::Focused(false) => {
            if window.label() == SPOTLIGHT_LABEL {
                let _ = window.hide();
            }
        }
        WindowEvent::Destroyed => {
            reference_card::on_window_closed(window.label());
        }
        _ => {}
    }
}

fn pomodoro_prompt_url() -> WebviewUrl {
    if cfg!(debug_assertions) {
        WebviewUrl::External(
            "http://localhost:5173/?view=pomodoro-prompt"
                .parse()
                .expect("valid pomodoro prompt dev url"),
        )
    } else {
        WebviewUrl::App("index.html".into())
    }
}

fn position_pomodoro_prompt(window: &WebviewWindow) {
    let Ok(Some(monitor)) = window.primary_monitor() else {
        return;
    };

    let work_area = monitor.work_area();
    let relative_x =
        (work_area.size.width as i64 - POMODORO_PROMPT_WIDTH - POMODORO_PROMPT_MARGIN).max(0);
    let relative_y =
        (work_area.size.height as i64 - POMODORO_PROMPT_HEIGHT - POMODORO_PROMPT_MARGIN).max(0);
    let x = clamp_i64_to_i32(work_area.position.x as i64 + relative_x);
    let y = clamp_i64_to_i32(work_area.position.y as i64 + relative_y);
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

pub(crate) fn show_pomodoro_prompt(app: &AppHandle) {
    let app_handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = app_handle.get_webview_window(POMODORO_PROMPT_LABEL) {
            position_pomodoro_prompt(&window);
            let _ = window.show();
            let _ = window.set_focus();
            #[cfg(windows)]
            force_foreground(&window);
            return;
        }

        let builder =
            WebviewWindowBuilder::new(&app_handle, POMODORO_PROMPT_LABEL, pomodoro_prompt_url())
                .title(POMODORO_PROMPT_TITLE)
                .inner_size(POMODORO_PROMPT_WIDTH as f64, POMODORO_PROMPT_HEIGHT as f64)
                .decorations(false)
                .always_on_top(true)
                .resizable(false)
                .skip_taskbar(true)
                .focused(true)
                .transparent(false)
                .visible(false)
                .initialization_script(POMODORO_PROMPT_VIEW_SCRIPT);

        let Ok(window) = builder.build() else {
            return;
        };

        position_pomodoro_prompt(&window);
        let _ = window.show();
        let _ = window.set_focus();
        #[cfg(windows)]
        force_foreground(&window);
    });
}

fn quick_capture_url() -> WebviewUrl {
    if cfg!(debug_assertions) {
        WebviewUrl::External(
            "http://localhost:5173/?view=quick-capture"
                .parse()
                .expect("valid quick capture dev url"),
        )
    } else {
        WebviewUrl::App("index.html".into())
    }
}

fn position_quick_capture(window: &WebviewWindow) {
    let Ok(Some(monitor)) = window.primary_monitor() else {
        return;
    };
    let size = monitor.size();
    let x = clamp_i64_to_i32((size.width as i64 - QUICK_CAPTURE_WIDTH) / 2);
    let y = clamp_i64_to_i32((size.height as i64 - QUICK_CAPTURE_HEIGHT) / 2);
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

pub(crate) fn show_quick_capture(app: &AppHandle) {
    let app_handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = app_handle.get_webview_window(QUICK_CAPTURE_LABEL) {
            position_quick_capture(&window);
            let _ = window.show();
            let _ = window.set_focus();
            #[cfg(windows)]
            force_foreground(&window);
            let _ = window.emit(EVENT_QUICK_CAPTURE_RESET, json!({}));
            return;
        }

        let builder =
            WebviewWindowBuilder::new(&app_handle, QUICK_CAPTURE_LABEL, quick_capture_url())
                .title(QUICK_CAPTURE_TITLE)
                .inner_size(QUICK_CAPTURE_WIDTH as f64, QUICK_CAPTURE_HEIGHT as f64)
                .decorations(false)
                .always_on_top(true)
                .resizable(false)
                .skip_taskbar(true)
                .focused(true)
                .visible(false)
                .initialization_script(QUICK_CAPTURE_VIEW_SCRIPT);

        let Ok(window) = builder.build() else {
            return;
        };
        position_quick_capture(&window);
        let _ = window.show();
        let _ = window.set_focus();
        #[cfg(windows)]
        force_foreground(&window);
    });
}

fn spotlight_url() -> WebviewUrl {
    if cfg!(debug_assertions) {
        WebviewUrl::External(
            "http://localhost:5173/spotlight.html"
                .parse()
                .expect("valid spotlight dev url"),
        )
    } else {
        WebviewUrl::App("spotlight.html".into())
    }
}

fn position_spotlight(window: &WebviewWindow) {
    let monitor = window
        .cursor_position()
        .ok()
        .and_then(|cursor| window.monitor_from_point(cursor.x, cursor.y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());

    let Some(monitor) = monitor else {
        return;
    };

    let work_area = monitor.work_area();
    let area_w = work_area.size.width as i64;
    let area_h = work_area.size.height as i64;
    let relative_x = ((area_w - SPOTLIGHT_WIDTH).max(0)) / 2;
    // 居中偏上：上 1/3 处
    let relative_y = ((area_h - SPOTLIGHT_HEIGHT).max(0)) / 3;
    let x = clamp_i64_to_i32(work_area.position.x as i64 + relative_x);
    let y = clamp_i64_to_i32(work_area.position.y as i64 + relative_y);
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

pub(crate) fn build_spotlight_window(app: &AppHandle) -> Option<WebviewWindow> {
    let builder = WebviewWindowBuilder::new(app, SPOTLIGHT_LABEL, spotlight_url())
        .title(SPOTLIGHT_TITLE)
        .inner_size(SPOTLIGHT_WIDTH as f64, SPOTLIGHT_HEIGHT as f64)
        .decorations(false)
        .always_on_top(true)
        .resizable(false)
        .skip_taskbar(true)
        .focused(false)
        .visible(false);
    let window = builder.build().ok()?;
    position_spotlight(&window);
    Some(window)
}

pub(crate) fn show_spotlight(app: &AppHandle) {
    let app_handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = app_handle.get_webview_window(SPOTLIGHT_LABEL) {
            let visible = window.is_visible().unwrap_or(false);
            let focused = window.is_focused().unwrap_or(false);
            if visible && focused {
                let _ = window.hide();
                return;
            }
            position_spotlight(&window);
            let _ = window.show();
            let _ = window.set_focus();
            #[cfg(windows)]
            force_foreground(&window);
            let _ = window.emit(EVENT_SPOTLIGHT_RESET, json!({}));
            return;
        }

        let Some(window) = build_spotlight_window(&app_handle) else {
            return;
        };
        let _ = window.show();
        let _ = window.set_focus();
        #[cfg(windows)]
        force_foreground(&window);
    });
}

#[tauri::command]
pub(crate) fn spotlight_open(app: tauri::AppHandle) -> Result<(), String> {
    show_spotlight(&app);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CursorMonitorRelation {
    MovedToCursorMonitor,
    AlreadyOnCursorMonitor,
    Unknown,
}

/// Move the main window to the monitor under current cursor.
/// Returns the relationship between the window and the cursor monitor.
pub(crate) fn move_window_to_cursor_monitor(
    window: &tauri::WebviewWindow,
) -> CursorMonitorRelation {
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
        x = clamp_i64_to_i32(work_area.position.x as i64 + ((target_w - win_w).max(0) / 2));
        y = clamp_i64_to_i32(work_area.position.y as i64 + ((target_h - win_h).max(0) / 2));
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
pub(crate) struct HotkeyNavigatePayload {
    pub(crate) target: String,
    pub(crate) did_move_to_cursor_monitor: bool,
    pub(crate) was_window_visible: bool,
    pub(crate) was_window_focused: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) item_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) view: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainWindowShortcutMode {
    Toggle,
    Navigate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainWindowShortcutDecision {
    Hide,
    Reveal,
    RevealAndNavigate { did_move_to_cursor_monitor: bool },
}

/// 统一主窗口快捷键的动作决策，便于复用与测试。
pub(crate) fn decide_main_window_shortcut(
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
pub(crate) fn handle_main_window_shortcut(app: &tauri::AppHandle, shortcut_name: &str) {
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
            if shortcut_name == "toggle" {
                let _ = window.emit(EVENT_MAIN_WINDOW_TOGGLE, json!({}));
            }
        }
        MainWindowShortcutDecision::RevealAndNavigate {
            did_move_to_cursor_monitor,
        } => {
            reveal_main_window(app);
            let _ = window.emit(
                EVENT_HOTKEY_NAVIGATE,
                HotkeyNavigatePayload {
                    target: shortcut_name.to_string(),
                    did_move_to_cursor_monitor,
                    was_window_visible: visible,
                    was_window_focused: focused,
                    text: None,
                    source: None,
                    item_id: None,
                    project_id: None,
                    view: None,
                },
            );
        }
    }
}

pub(crate) fn navigate_main_window_to_tool(
    app: &tauri::AppHandle,
    tool_id: &str,
) -> Result<(), String> {
    navigate_main_window_to_tool_context(app, tool_id, None, None)
}

pub(crate) fn navigate_main_window_to_tool_context(
    app: &tauri::AppHandle,
    tool_id: &str,
    item_id: Option<String>,
    view: Option<String>,
) -> Result<(), String> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Err("主窗口不可用".into());
    };
    let visible = window.is_visible().unwrap_or(false);
    let focused = window.is_focused().unwrap_or(false);
    let cursor_monitor_relation = move_window_to_cursor_monitor(&window);
    let did_move_to_cursor_monitor =
        cursor_monitor_relation == CursorMonitorRelation::MovedToCursorMonitor;
    reveal_main_window(app);
    window
        .emit(
            EVENT_HOTKEY_NAVIGATE,
            HotkeyNavigatePayload {
                target: tool_id.to_string(),
                did_move_to_cursor_monitor,
                was_window_visible: visible,
                was_window_focused: focused,
                text: None,
                source: None,
                item_id,
                project_id: None,
                view,
            },
        )
        .map_err(|error| error.to_string())
}

/// Re-register all shortcuts from the global map.

/// Window subclass ID (arbitrary unique value)
/// Force a window to the foreground on Windows using the AttachThreadInput trick.
/// Windows 10+ restricts SetForegroundWindow to the current foreground process;
/// this workaround temporarily attaches our thread's input queue to the foreground
/// thread so that the call succeeds reliably.
#[cfg(windows)]
pub(crate) fn force_foreground(window: &tauri::WebviewWindow) {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, IsIconic, SetForegroundWindow, ShowWindow,
        SW_RESTORE,
    };

    let Ok(hwnd_raw) = window.hwnd() else {
        return;
    };
    let hwnd = hwnd_raw.0;

    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }

        let foreground = GetForegroundWindow();
        if foreground.is_null() || foreground == hwnd {
            SetForegroundWindow(hwnd);
        } else {
            let fg_thread = GetWindowThreadProcessId(foreground, std::ptr::null_mut());
            let our_thread = GetCurrentThreadId();
            if fg_thread != our_thread && fg_thread != 0 {
                AttachThreadInput(our_thread, fg_thread, 1);
                SetForegroundWindow(hwnd);
                AttachThreadInput(our_thread, fg_thread, 0);
            } else {
                SetForegroundWindow(hwnd);
            }
        }

        // 消化 Windows/WebView2 在窗口激活第一帧吞掉的首个 keystroke。
        // 现象: 热键唤出 spotlight 后, 用户按下的第一个字符键被吞、第二个键才进 input;
        // 鼠标点击之所以能立刻输入, 是因为 WM_LBUTTONDOWN 前 Windows 自动派发的
        // WM_MOUSEACTIVATE 已经填占了那个被吞的位。这里注入一次 VK_NONAME (0xFC,
        // 无字符映射、无副作用) 主动消化该位, 用户真实首键就能进 input。
        let mut inp_down: INPUT = std::mem::zeroed();
        inp_down.r#type = INPUT_KEYBOARD;
        inp_down.Anonymous.ki = KEYBDINPUT {
            wVk: 0xFC,
            wScan: 0,
            dwFlags: 0,
            time: 0,
            dwExtraInfo: 0,
        };
        let mut inp_up: INPUT = std::mem::zeroed();
        inp_up.r#type = INPUT_KEYBOARD;
        inp_up.Anonymous.ki = KEYBDINPUT {
            wVk: 0xFC,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: 0,
        };
        let inputs = [inp_down, inp_up];
        SendInput(2, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

#[tauri::command]
pub(crate) fn spotlight_pick(
    app: tauri::AppHandle,
    target: String,
    text: Option<String>,
    source: Option<String>,
    item_id: Option<String>,
    project_id: Option<String>,
    view: Option<String>,
) -> Result<(), String> {
    if let Some(spot) = app.get_webview_window(SPOTLIGHT_LABEL) {
        let _ = spot.hide();
    }
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Err("主窗口不可用".into());
    };
    let visible = window.is_visible().unwrap_or(false);
    let focused = window.is_focused().unwrap_or(false);
    let cursor_monitor_relation = move_window_to_cursor_monitor(&window);
    let did_move_to_cursor_monitor =
        cursor_monitor_relation == CursorMonitorRelation::MovedToCursorMonitor;
    reveal_main_window(&app);
    let _ = window.emit(
        EVENT_HOTKEY_NAVIGATE,
        HotkeyNavigatePayload {
            target,
            did_move_to_cursor_monitor,
            was_window_visible: visible,
            was_window_focused: focused,
            text,
            source,
            item_id,
            project_id,
            view,
        },
    );
    Ok(())
}

#[tauri::command]
pub(crate) fn spotlight_close(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(SPOTLIGHT_LABEL) {
        let _ = window.hide();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        decide_main_window_shortcut, CursorMonitorRelation, HotkeyNavigatePayload,
        MainWindowShortcutDecision, MainWindowShortcutMode,
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
                text: None,
                source: None,
                item_id: None,
                project_id: None,
                view: None,
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

    #[test]
    fn hotkey_navigate_payload_serializes_optional_text_and_source() {
        assert_eq!(
            serde_json::to_value(HotkeyNavigatePayload {
                target: "json-formatter".to_string(),
                did_move_to_cursor_monitor: false,
                was_window_visible: false,
                was_window_focused: false,
                text: Some("{\"a\":1}".to_string()),
                source: Some("clipboard-suggestion".to_string()),
                item_id: None,
                project_id: None,
                view: None,
            })
            .unwrap(),
            json!({
                "target": "json-formatter",
                "didMoveToCursorMonitor": false,
                "wasWindowVisible": false,
                "wasWindowFocused": false,
                "text": "{\"a\":1}",
                "source": "clipboard-suggestion",
            })
        );
    }
}

//! 全屏切净检测（plan §3.3 / design §9）
//!
//! 三层判定，命中任一即视为「不该刷新」：
//! 1. SHQueryUserNotificationState：QUNS_BUSY / QUNS_RUNNING_D3D_FULL_SCREEN /
//!    QUNS_PRESENTATION_MODE → 演示 / 录屏 / 全屏 D3D
//! 2. 前台窗口 rect 是否完全覆盖整块主屏（含 Chrome / VLC 全屏播放）
//! 3. 前台进程名是否在 widget.fullscreen_blacklist（OBS / PowerPoint / Zoom 等）
//!
//! 任一 Win32 失败回退为 false，避免误切净。

#![allow(dead_code)]

#[cfg(windows)]
pub use imp::is_fullscreen_busy;

#[cfg(not(windows))]
pub fn is_fullscreen_busy() -> bool {
    false
}

#[cfg(windows)]
mod imp {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, HWND, RECT};
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTOPRIMARY};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::Shell::{
        SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId};

    use crate::tools::widget::config;

    pub fn is_fullscreen_busy() -> bool {
        if check_notification_state() { return true; }
        if check_foreground_full_screen() { return true; }
        if check_foreground_blacklisted() { return true; }
        false
    }

    fn check_notification_state() -> bool {
        unsafe {
            match SHQueryUserNotificationState() {
                Ok(state) => matches!(state, QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE),
                Err(_) => false,
            }
        }
    }

    fn check_foreground_full_screen() -> bool {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() { return false; }
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_err() { return false; }
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY);
            if monitor.is_invalid() { return false; }
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if !GetMonitorInfoW(monitor, &mut info).as_bool() { return false; }
            // 严格相等 → 用户化 Chrome 全屏 / VLC 全屏视频也命中；非全屏窗口不会误判
            rect.left == info.rcMonitor.left
                && rect.top == info.rcMonitor.top
                && rect.right == info.rcMonitor.right
                && rect.bottom == info.rcMonitor.bottom
        }
    }

    fn check_foreground_blacklisted() -> bool {
        let cfg = config::read_config();
        if cfg.fullscreen_blacklist.is_empty() { return false; }
        let Some(name) = foreground_process_name() else { return false; };
        let lower = name.to_ascii_lowercase();
        cfg.fullscreen_blacklist.iter().any(|raw| raw.to_ascii_lowercase() == lower)
    }

    fn foreground_process_name() -> Option<String> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() { return None; }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 { return None; }
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            if process.is_invalid() { return None; }
            let mut buf = [0u16; 1024];
            let mut size = buf.len() as u32;
            let q = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_FORMAT(0),
                PWSTR(buf.as_mut_ptr()),
                &mut size,
            );
            let _ = CloseHandle(process);
            if q.is_err() { return None; }
            let full_path = OsString::from_wide(&buf[..size as usize]).to_string_lossy().into_owned();
            // 取最末段 .exe 名
            std::path::Path::new(&full_path).file_name().and_then(|s| s.to_str()).map(|s| s.to_string())
        }
    }

    // 抑制可能未使用的 import
    #[allow(dead_code)]
    fn _silence() {
        let _ = HWND::default();
    }
}

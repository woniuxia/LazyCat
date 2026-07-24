//! 防护检测模块
//!
//! 合并 fullscreen.rs / lock.rs / idle.rs 三个模块。
//! 提供四个公开函数：
//! - `fullscreen_busy_app()` — 全屏切净检测，返回触发的进程名/原因；None = 未触发
//! - `is_fullscreen_busy()` — `fullscreen_busy_app().is_some()` 的快捷封装
//! - `is_locked()` — 锁屏检测（OpenInputDesktop）
//! - `try_system_input_snapshot()` — 最近输入 tick 与用户空闲秒数
//! - `seconds_idle()` — 用户空闲秒数兼容接口

#![allow(dead_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SystemInputSnapshot {
    pub last_input_tick_ms: u32,
    pub idle_secs: u64,
}

#[cfg(windows)]
pub use imp::fullscreen_busy_app;
#[cfg(windows)]
pub use imp::is_locked;
#[cfg(windows)]
pub use imp::try_system_input_snapshot;

#[cfg(windows)]
pub fn is_fullscreen_busy() -> bool {
    fullscreen_busy_app().is_some()
}

#[cfg(not(windows))]
pub fn fullscreen_busy_app() -> Option<String> {
    None
}
#[cfg(not(windows))]
pub fn is_fullscreen_busy() -> bool {
    false
}
#[cfg(not(windows))]
pub fn is_locked() -> bool {
    false
}
#[cfg(not(windows))]
pub fn try_system_input_snapshot() -> Option<SystemInputSnapshot> {
    None
}

pub fn seconds_idle() -> u32 {
    try_system_input_snapshot()
        .map(|value| value.idle_secs.min(u32::MAX as u64) as u32)
        .unwrap_or(0)
}

// ── Windows 实现 ────────────────────────────────

#[cfg(windows)]
mod imp {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE, RECT};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
    };
    use windows::Win32::System::StationsAndDesktops::{
        CloseDesktop, GetUserObjectInformationW, OpenInputDesktop, DESKTOP_SWITCHDESKTOP,
        UOI_NAME,
    };
    use windows::Win32::System::SystemInformation::GetTickCount64;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    use windows::Win32::UI::Shell::{
        SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE,
        QUNS_RUNNING_D3D_FULL_SCREEN,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId,
    };

    use crate::tools::widget::config;
    use super::SystemInputSnapshot;

    // ── 全屏切净检测（原 fullscreen.rs） ──────────

    /// 三层判定：通知状态 / 前台窗口覆盖全屏 / 黑名单进程。
    /// 返回 `Some(进程名 或 原因标签)` 表示需要切净，`None` 表示无需切净。
    pub fn fullscreen_busy_app() -> Option<String> {
        // 通知状态：拿不到具体进程名，先取前台进程兜底；仍取不到则给标签。
        if check_notification_state() {
            return Some(
                foreground_process_name().unwrap_or_else(|| "系统通知态".into()),
            );
        }
        // 前台全屏窗口：拿前台进程名，取不到给标签。
        if check_foreground_full_screen() {
            return Some(
                foreground_process_name().unwrap_or_else(|| "前台全屏窗口".into()),
            );
        }
        // 黑名单：直接拿前台进程名（命中时必然有值）。
        if let Some(name) = check_foreground_blacklisted() {
            return Some(name);
        }
        None
    }

    fn check_notification_state() -> bool {
        unsafe {
            match SHQueryUserNotificationState() {
                Ok(state) => matches!(
                    state,
                    QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE
                ),
                Err(_) => false,
            }
        }
    }

    fn check_foreground_full_screen() -> bool {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() {
                return false;
            }
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_err() {
                return false;
            }
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY);
            if monitor.is_invalid() {
                return false;
            }
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if !GetMonitorInfoW(monitor, &mut info).as_bool() {
                return false;
            }
            rect.left == info.rcMonitor.left
                && rect.top == info.rcMonitor.top
                && rect.right == info.rcMonitor.right
                && rect.bottom == info.rcMonitor.bottom
        }
    }

    fn check_foreground_blacklisted() -> Option<String> {
        let cfg = config::read_config();
        if cfg.fullscreen_blacklist.is_empty() {
            return None;
        }
        let name = foreground_process_name()?;
        let lower = name.to_ascii_lowercase();
        if cfg
            .fullscreen_blacklist
            .iter()
            .any(|raw| raw.to_ascii_lowercase() == lower)
        {
            Some(name)
        } else {
            None
        }
    }

    fn foreground_process_name() -> Option<String> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() {
                return None;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return None;
            }
            let process =
                OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            if process.is_invalid() {
                return None;
            }
            let mut buf = [0u16; 1024];
            let mut size = buf.len() as u32;
            let q = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_FORMAT(0),
                PWSTR(buf.as_mut_ptr()),
                &mut size,
            );
            let _ = CloseHandle(process);
            if q.is_err() {
                return None;
            }
            let full_path =
                OsString::from_wide(&buf[..size as usize])
                    .to_string_lossy()
                    .into_owned();
            std::path::Path::new(&full_path)
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        }
    }

    // ── 锁屏检测（原 lock.rs） ─────────────────────

    const NAME_BUF_LEN: usize = 64;

    /// 通过 OpenInputDesktop 取当前输入桌面名；非 "Default" 即为锁定。
    pub fn is_locked() -> bool {
        unsafe {
            let h = match OpenInputDesktop(Default::default(), false, DESKTOP_SWITCHDESKTOP) {
                Ok(h) if !h.is_invalid() => h,
                _ => return false,
            };

            let mut name_buf = [0u16; NAME_BUF_LEN];
            let mut needed: u32 = 0;
            let info_res = GetUserObjectInformationW(
                HANDLE(h.0),
                UOI_NAME,
                Some(name_buf.as_mut_ptr() as *mut _),
                (NAME_BUF_LEN * std::mem::size_of::<u16>()) as u32,
                Some(&mut needed),
            );
            let _ = CloseDesktop(h);

            if info_res.is_err() {
                return false;
            }

            let len = name_buf.iter().position(|&c| c == 0).unwrap_or(NAME_BUF_LEN);
            let name = String::from_utf16_lossy(&name_buf[..len]);
            !name.eq_ignore_ascii_case("Default")
        }
    }

    // ── 空闲检测（原 idle.rs） ─────────────────────

    /// 最近输入 tick 与距该输入的秒数；失败时返回 None。
    pub fn try_system_input_snapshot() -> Option<SystemInputSnapshot> {
        unsafe {
            let mut info = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };
            if !GetLastInputInfo(&mut info).as_bool() {
                return None;
            }
            let now_ms = GetTickCount64() as u32;
            let last_ms = info.dwTime;
            let diff_ms = now_ms.wrapping_sub(last_ms);
            Some(SystemInputSnapshot {
                last_input_tick_ms: last_ms,
                idle_secs: (diff_ms / 1000) as u64,
            })
        }
    }
}

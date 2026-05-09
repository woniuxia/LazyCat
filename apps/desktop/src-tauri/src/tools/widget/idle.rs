//! 空闲检测（plan §3.1 / design §8）
//!
//! 通过 [`GetLastInputInfo`] 取距上次系统级用户输入（鼠标 / 键盘）的秒数。
//! 任何 Win32 调用失败 / 不可达回退 0（视为「刚有输入」），避免误降频。
//!
//! 单调时间用 [`GetTickCount64`]，避免 32 位 GetTickCount 49.7 天回环。

#![allow(dead_code)] // scheduler 在 sleep 间隙轮询此函数

#[cfg(windows)]
pub use imp::seconds_idle;

#[cfg(not(windows))]
pub fn seconds_idle() -> u32 {
    0
}

#[cfg(windows)]
mod imp {
    use windows::Win32::System::SystemInformation::GetTickCount64;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    /// 距离上次系统级用户输入的秒数；失败回退 0。
    pub fn seconds_idle() -> u32 {
        unsafe {
            let mut info = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };
            if !GetLastInputInfo(&mut info).as_bool() {
                return 0;
            }

            // GetTickCount64 returns ms since boot (u64); LASTINPUTINFO.dwTime
            // 是 32 位 ms tick，与低 32 位对齐即可（差值不超过 49.7 天，OK）
            let now_ms = GetTickCount64() as u32;
            let last_ms = info.dwTime;
            let diff_ms = now_ms.wrapping_sub(last_ms);
            diff_ms / 1000
        }
    }
}

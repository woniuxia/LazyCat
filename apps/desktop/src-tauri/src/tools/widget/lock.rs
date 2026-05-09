//! 锁屏检测（plan §3.3 / design §8）
//!
//! 通过 [`OpenInputDesktop`] 取当前输入桌面名：
//! - `"Default"` → 用户已登录，正常工作
//! - `"Winlogon"` / `"Screen-saver"` 等 → 锁屏 / 用户切换 / 屏保
//!
//! 此方法相比 `WTSRegisterSessionNotification` 优点：
//! - 无需创建 message-only window 与 windows-rs 的 LRESULT 回调
//! - 同步轮询即可，融入 scheduler.should_skip 不需要额外线程
//!
//! 缺点：要求当前进程对输入桌面有 `DESKTOP_SWITCHDESKTOP` 访问权；
//! 在标准用户会话下默认满足。失败回退到「未锁定」，避免误暂停。

#![allow(dead_code)] // is_locked 由 scheduler::should_skip 调用

#[cfg(windows)]
pub use imp::is_locked;

#[cfg(not(windows))]
pub fn is_locked() -> bool {
    false
}

#[cfg(windows)]
mod imp {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::StationsAndDesktops::{
        CloseDesktop, GetUserObjectInformationW, OpenInputDesktop, DESKTOP_SWITCHDESKTOP,
        UOI_NAME,
    };

    /// 桌面名缓冲长度；实际名字最长 ~8 字符（"Default" / "Winlogon"），
    /// 留出额外余量保护未知扩展。
    const NAME_BUF_LEN: usize = 64;

    /// 当前会话是否处于锁屏 / 用户切换 / 屏保态。
    ///
    /// 任何 Win32 调用失败 / 不可达都视为「未锁定」，避免无限暂停。
    pub fn is_locked() -> bool {
        unsafe {
            let h = match OpenInputDesktop(Default::default(), false, DESKTOP_SWITCHDESKTOP) {
                Ok(h) if !h.is_invalid() => h,
                _ => return false, // 拿不到输入桌面句柄 → 不主动暂停
            };

            let mut name_buf = [0u16; NAME_BUF_LEN];
            let mut needed: u32 = 0;
            // HDESK 与 HANDLE 同构（*mut c_void）但 windows-rs 不自动转，手工包一次
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

            // GetUserObjectInformationW 写入以 NUL 结尾的 UTF-16 串
            let len = name_buf.iter().position(|&c| c == 0).unwrap_or(NAME_BUF_LEN);
            let name = String::from_utf16_lossy(&name_buf[..len]);

            // "Default" 之外的桌面（"Winlogon" / "Screen-saver" / "Disconnect" 等）都视为锁定
            !name.eq_ignore_ascii_case("Default")
        }
    }
}

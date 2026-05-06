//! IDesktopWallpaper + SystemParametersInfoW 双层封装（Phase 1.6）
//!
//! 设计依据：plan §1.6 / design §13.2
//!
//! - 主路径：`IDesktopWallpaper::SetPosition(DWPOS_FILL)` + `SetWallpaper(monitor, path)`
//! - 失败回退：`SystemParametersInfoW(SPI_SETDESKWALLPAPER)`，并记录 `set_method = sysparam`
//! - COM 线程仅初始化一次（`OnceLock`）；`RPC_E_CHANGED_MODE` 视为已初始化忽略
//!
//! 整文件通过 cfg gate 在非 Windows 平台提供 stub，保证 wallpaper 模块跨平台编译通过。

#![allow(dead_code)] // Phase 1.6：apply / restore 在 Phase 1.8+ 接入
#![allow(unused_imports)] // cfg gate 下 Path/PathBuf 仅 stub 路径用

use std::path::{Path, PathBuf};

/// 设置壁纸时实际走的路径；用于 `wallpaper.original_set_method` 持久化，
/// `restore` 时按相同方式回写原图。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetMethod {
    /// 主路径：`IDesktopWallpaper::SetWallpaper`
    Com,
    /// 回退：`SystemParametersInfoW(SPI_SETDESKWALLPAPER)`
    SysParam,
}

impl SetMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Com => "com",
            Self::SysParam => "sysparam",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "sysparam" => Self::SysParam,
            _ => Self::Com,
        }
    }
}

/// 主屏的 monitor 索引；阶段 1 仅支持单屏。
pub const PRIMARY_MONITOR_INDEX: u32 = 0;

// ── Windows 真实实现 ─────────────────────────────

#[cfg(windows)]
mod imp {
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;

    use windows::core::{Interface, PCWSTR, PWSTR};
    use windows::Win32::Foundation::{HWND, RPC_E_CHANGED_MODE, S_FALSE, S_OK};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_LOCAL_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{DesktopWallpaper, IDesktopWallpaper, DWPOS_FILL};
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_SETDESKWALLPAPER,
        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };

    use super::SetMethod;

    /// 进程内 COM init 标记（per-thread 行为由 Windows 处理；此处只防止重复调）。
    fn ensure_com_init() -> Result<(), String> {
        static INIT: OnceLock<()> = OnceLock::new();
        if INIT.get().is_some() {
            return Ok(());
        }
        unsafe {
            let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            // S_OK / S_FALSE / RPC_E_CHANGED_MODE 均视为可继续：
            //  - S_FALSE：本线程已 init 同模式
            //  - RPC_E_CHANGED_MODE：本线程已 init 不同模式（如 Tauri 主线程 MTA），
            //    我们仍可使用现有 apartment，文档允许
            if hr != S_OK && hr != S_FALSE && hr != RPC_E_CHANGED_MODE {
                return Err(format!("CoInitializeEx failed: {hr:?}"));
            }
        }
        let _ = INIT.set(());
        Ok(())
    }

    fn create_idw() -> Result<IDesktopWallpaper, String> {
        ensure_com_init()?;
        unsafe {
            CoCreateInstance::<_, IDesktopWallpaper>(&DesktopWallpaper, None, CLSCTX_LOCAL_SERVER)
                .map_err(|e| format!("CoCreateInstance(DesktopWallpaper) failed: {e}"))
        }
    }

    /// 把 `&Path` 编码为 NUL 结尾的 UTF-16 缓冲（PCWSTR 兼容）
    fn to_wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    /// 取得指定 monitor 的设备路径（COM SetWallpaper 第一参数）。
    ///
    /// 返回的 `PWSTR` 由调用者通过 `CoTaskMemFree(ptr)` 释放（COM 协议）。
    fn monitor_device_path(dw: &IDesktopWallpaper, monitor_index: u32) -> Result<PWSTR, String> {
        unsafe {
            let count = dw
                .GetMonitorDevicePathCount()
                .map_err(|e| format!("GetMonitorDevicePathCount: {e}"))?;
            if monitor_index >= count {
                return Err(format!(
                    "monitor_index {monitor_index} >= count {count}"
                ));
            }
            dw.GetMonitorDevicePathAt(monitor_index)
                .map_err(|e| format!("GetMonitorDevicePathAt({monitor_index}): {e}"))
        }
    }

    /// 主路径：IDesktopWallpaper。
    fn try_set_via_com(monitor_index: u32, image_path: &Path) -> Result<(), String> {
        let dw = create_idw()?;
        unsafe {
            dw.SetPosition(DWPOS_FILL)
                .map_err(|e| format!("SetPosition(DWPOS_FILL): {e}"))?;
            let monitor = monitor_device_path(&dw, monitor_index)?;
            let wide = to_wide(image_path);
            let result = dw
                .SetWallpaper(
                    PCWSTR::from_raw(monitor.0),
                    PCWSTR::from_raw(wide.as_ptr()),
                )
                .map_err(|e| format!("SetWallpaper: {e}"));
            CoTaskMemFree(Some(monitor.0.cast()));
            result?;
        }
        Ok(())
    }

    /// 回退：SystemParametersInfoW。
    fn try_set_via_sysparam(image_path: &Path) -> Result<(), String> {
        let mut wide = to_wide(image_path);
        unsafe {
            SystemParametersInfoW(
                SPI_SETDESKWALLPAPER,
                0,
                Some(wide.as_mut_ptr().cast()),
                SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
            )
            .map_err(|e| format!("SystemParametersInfoW(SPI_SETDESKWALLPAPER): {e}"))
        }
    }

    pub fn set_wallpaper(monitor_index: u32, image_path: &Path) -> Result<SetMethod, String> {
        match try_set_via_com(monitor_index, image_path) {
            Ok(()) => Ok(SetMethod::Com),
            Err(com_err) => {
                eprintln!(
                    "[wallpaper] COM SetWallpaper failed, falling back to sysparam: {com_err}"
                );
                try_set_via_sysparam(image_path)?;
                Ok(SetMethod::SysParam)
            }
        }
    }

    pub fn get_current_wallpaper(monitor_index: u32) -> Result<PathBuf, String> {
        let dw = create_idw()?;
        unsafe {
            let monitor = monitor_device_path(&dw, monitor_index)?;
            let result = dw
                .GetWallpaper(PCWSTR::from_raw(monitor.0))
                .map_err(|e| format!("GetWallpaper: {e}"));
            CoTaskMemFree(Some(monitor.0.cast()));
            let pwstr = result?;
            let path = pwstr_to_pathbuf(pwstr.0);
            CoTaskMemFree(Some(pwstr.0.cast()));
            Ok(path)
        }
    }

    /// 主屏物理像素尺寸 (width, height)；compose::region_for 需要它确定 region 像素位置。
    pub fn monitor_rect(monitor_index: u32) -> Result<(u32, u32), String> {
        let dw = create_idw()?;
        unsafe {
            let monitor = monitor_device_path(&dw, monitor_index)?;
            let result = dw
                .GetMonitorRECT(PCWSTR::from_raw(monitor.0))
                .map_err(|e| format!("GetMonitorRECT: {e}"));
            CoTaskMemFree(Some(monitor.0.cast()));
            let rect = result?;
            let w = (rect.right - rect.left).max(0) as u32;
            let h = (rect.bottom - rect.top).max(0) as u32;
            Ok((w, h))
        }
    }

    fn pwstr_to_pathbuf(ptr: *const u16) -> PathBuf {
        if ptr.is_null() {
            return PathBuf::new();
        }
        let mut len = 0usize;
        unsafe {
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(ptr, len);
            use std::os::windows::ffi::OsStringExt;
            PathBuf::from(std::ffi::OsString::from_wide(slice))
        }
    }

    // 抑制未使用 import 警告：HWND / Interface 在编译期由 windows-rs 内部 trait 系统使用
    #[allow(dead_code)]
    fn _silence_unused() {
        let _ = HWND::default();
        let _: SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS = SPIF_UPDATEINIFILE;
        let _ = <IDesktopWallpaper as Interface>::IID;
    }
}

#[cfg(windows)]
pub use imp::{get_current_wallpaper, monitor_rect, set_wallpaper};

// ── 非 Windows stub（保持模块跨平台编译） ────────

#[cfg(not(windows))]
pub fn set_wallpaper(_monitor_index: u32, _image_path: &Path) -> Result<SetMethod, String> {
    Err("set_wallpaper only supported on Windows".into())
}

#[cfg(not(windows))]
pub fn get_current_wallpaper(_monitor_index: u32) -> Result<PathBuf, String> {
    Err("get_current_wallpaper only supported on Windows".into())
}

#[cfg(not(windows))]
pub fn monitor_rect(_monitor_index: u32) -> Result<(u32, u32), String> {
    Err("monitor_rect only supported on Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_method_round_trip() {
        assert_eq!(SetMethod::Com.as_str(), "com");
        assert_eq!(SetMethod::SysParam.as_str(), "sysparam");
        assert_eq!(SetMethod::from_str("com"), SetMethod::Com);
        assert_eq!(SetMethod::from_str("sysparam"), SetMethod::SysParam);
        // 未知值 → Com（COM 是主路径）
        assert_eq!(SetMethod::from_str(""), SetMethod::Com);
        assert_eq!(SetMethod::from_str("nope"), SetMethod::Com);
    }
}

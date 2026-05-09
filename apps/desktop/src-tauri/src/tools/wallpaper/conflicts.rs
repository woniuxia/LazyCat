//! Spotlight / 第三方壁纸引擎冲突检测（design §13.4）
//!
//! - Spotlight：读注册表 BackgroundType（==3）或当前壁纸路径指向 Spotlight 缓存
//! - 第三方引擎：枚举进程列表，匹配 wallpaper32/64.exe / Lively.exe / DeskScapes11.exe
//!
//! 由 scheduler 启动一次 + 每 10min 重查；结果写入 state.spotlight_detected /
//! state.third_party_engine。任何 Win32 失败回退安全默认（false / None）。

#![allow(dead_code)]

#[cfg(windows)]
pub use imp::refresh;

#[cfg(not(windows))]
pub fn refresh() {}

#[cfg(windows)]
mod imp {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
        REG_VALUE_TYPE,
    };

    use crate::tools::wallpaper::state;

    const KNOWN_ENGINES: &[&str] = &[
        "wallpaper32.exe",
        "wallpaper64.exe",
        "lively.exe",
        "deskscapes11.exe",
        "deskscapes.exe",
        "desktophut.exe",
    ];

    pub fn refresh() {
        let spotlight = detect_spotlight();
        let engine = detect_third_party_engine();
        state::write(|s| {
            s.spotlight_detected = spotlight;
            s.third_party_engine = engine;
        });
    }

    fn detect_spotlight() -> bool {
        if read_background_type_is_spotlight() { return true; }
        read_wallpaper_path_in_spotlight_cache()
    }

    fn read_background_type_is_spotlight() -> bool {
        let subkey = wide(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Wallpapers");
        let value = wide("BackgroundType");
        // BackgroundType 取值（W10 1903+ / W11）：
        //   0 = Picture, 1 = SolidColor, 2 = Slideshow, 3 = Spotlight
        // 旧实现误判 ==2 为 Spotlight（实际是幻灯片）；正确值是 3。
        read_dword(HKEY_CURRENT_USER, &subkey, &value)
            .map(|v| v == 3)
            .unwrap_or(false)
    }

    fn read_wallpaper_path_in_spotlight_cache() -> bool {
        let subkey = wide(r"Control Panel\Desktop");
        let value = wide("WallPaper");
        let Some(path) = read_string(HKEY_CURRENT_USER, &subkey, &value) else { return false; };
        let lower = path.to_ascii_lowercase();
        // Spotlight 缓存路径通常落在：
        //   %LOCALAPPDATA%\Packages\Microsoft.Windows.ContentDeliveryManager_cw5n1h2txyewy\
        //     LocalState\Assets\<hash>
        // 部分 W11 较新版本路径里出现 MicrosoftWindows.Client.CBS_*，旧关键词保留兼容。
        (lower.contains("contentdeliverymanager") || lower.contains("microsoftwindows.client.cbs"))
            && lower.contains(r"localstate\assets")
    }

    fn detect_third_party_engine() -> Option<String> {
        let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        let snap = snap.ok()?;
        if snap.is_invalid() { return None; }

        let mut entry = PROCESSENTRY32W::default();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        let mut found: Option<String> = None;
        unsafe {
            if Process32FirstW(snap, &mut entry).is_ok() {
                loop {
                    let name = exe_name_from_entry(&entry);
                    let lower = name.to_ascii_lowercase();
                    if KNOWN_ENGINES.iter().any(|known| *known == lower) {
                        found = Some(name);
                        break;
                    }
                    if Process32NextW(snap, &mut entry).is_err() { break; }
                }
            }
            let _ = CloseHandle(snap);
        }
        found
    }

    fn exe_name_from_entry(entry: &PROCESSENTRY32W) -> String {
        let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
        OsString::from_wide(&entry.szExeFile[..len]).to_string_lossy().into_owned()
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn read_dword(root: HKEY, subkey: &[u16], value: &[u16]) -> Option<u32> {
        unsafe {
            let mut hkey = HKEY::default();
            if RegOpenKeyExW(root, PCWSTR(subkey.as_ptr()), Some(0), KEY_READ, &mut hkey).is_err() {
                return None;
            }
            let mut data = 0u32;
            let mut size = std::mem::size_of::<u32>() as u32;
            let mut typ = REG_VALUE_TYPE::default();
            let q = RegQueryValueExW(
                hkey,
                PCWSTR(value.as_ptr()),
                Some(std::ptr::null_mut()),
                Some(&mut typ),
                Some((&mut data as *mut u32) as *mut u8),
                Some(&mut size),
            );
            let _ = RegCloseKey(hkey);
            q.is_ok().then_some(data)
        }
    }

    fn read_string(root: HKEY, subkey: &[u16], value: &[u16]) -> Option<String> {
        unsafe {
            let mut hkey = HKEY::default();
            if RegOpenKeyExW(root, PCWSTR(subkey.as_ptr()), Some(0), KEY_READ, &mut hkey).is_err() {
                return None;
            }
            let mut size = 0u32;
            let mut typ = REG_VALUE_TYPE::default();
            if RegQueryValueExW(hkey, PCWSTR(value.as_ptr()), Some(std::ptr::null_mut()), Some(&mut typ), Some(std::ptr::null_mut()), Some(&mut size)).is_err() {
                let _ = RegCloseKey(hkey);
                return None;
            }
            let mut buf = vec![0u16; (size as usize / 2).max(1)];
            let q = RegQueryValueExW(
                hkey,
                PCWSTR(value.as_ptr()),
                Some(std::ptr::null_mut()),
                Some(&mut typ),
                Some(buf.as_mut_ptr() as *mut u8),
                Some(&mut size),
            );
            let _ = RegCloseKey(hkey);
            if q.is_err() { return None; }
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            Some(OsString::from_wide(&buf[..len]).to_string_lossy().into_owned())
        }
    }
}

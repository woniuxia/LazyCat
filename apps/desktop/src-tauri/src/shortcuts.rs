use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::reference_card;
use crate::window_manager;

static REGISTERED_SHORTCUTS: std::sync::LazyLock<Mutex<HashMap<String, String>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
static RECORDING_MODE: AtomicBool = AtomicBool::new(false);

/// Re-register all shortcuts from the global map.
/// Called after any add/remove to keep the shortcut manager in sync.
fn sync_all_shortcuts(app: &tauri::AppHandle) -> Result<(), String> {
    let manager = app.global_shortcut();
    manager.unregister_all().map_err(|e| e.to_string())?;

    let map = REGISTERED_SHORTCUTS
        .lock()
        .map_err(|e| format!("快捷键锁定失败: {e}"))?;
    for (name, shortcut_str) in map.iter() {
        if shortcut_str.is_empty() {
            continue;
        }
        let sc: Shortcut = shortcut_str.parse().map_err(|e| format!("{e}"))?;
        let name_owned = name.clone();
        manager
            .on_shortcut(sc, move |app_handle, _sc, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                if name_owned == "quick-capture" {
                    window_manager::show_quick_capture(app_handle);
                    return;
                }
                if name_owned == "spotlight" {
                    window_manager::show_spotlight(app_handle);
                    return;
                }
                if name_owned == "reference-card" {
                    reference_card::show_from_clipboard(app_handle);
                    return;
                }
                window_manager::handle_main_window_shortcut(app_handle, name_owned.as_str());
            })
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn register_hotkey(app: tauri::AppHandle, shortcut: String) -> Result<(), String> {
    // Backward-compatible: register as "toggle"
    register_named_hotkey(app, "toggle".into(), shortcut)
}

#[tauri::command]
pub(crate) fn unregister_hotkey(app: tauri::AppHandle) -> Result<(), String> {
    unregister_named_hotkey(app, "toggle".into())
}

#[tauri::command]
pub(crate) fn register_named_hotkey(
    app: tauri::AppHandle,
    name: String,
    shortcut: String,
) -> Result<(), String> {
    {
        let mut map = REGISTERED_SHORTCUTS
            .lock()
            .map_err(|e| format!("快捷键锁定失败: {e}"))?;
        if shortcut.is_empty() {
            map.remove(&name);
        } else {
            // Validate first
            let _: Shortcut = shortcut.parse().map_err(|e| format!("{e}"))?;
            map.insert(name, shortcut);
        }
    }
    sync_all_shortcuts(&app)
}

#[tauri::command]
pub(crate) fn unregister_named_hotkey(app: tauri::AppHandle, name: String) -> Result<(), String> {
    {
        let mut map = REGISTERED_SHORTCUTS
            .lock()
            .map_err(|e| format!("快捷键锁定失败: {e}"))?;
        map.remove(&name);
    }
    sync_all_shortcuts(&app)
}

#[cfg(windows)]
const SUBCLASS_ID: usize = 0x4C5A_4341; // "LZCA"

/// Subclass procedure: intercepts WM_SYSCOMMAND to block system menu during recording.
#[cfg(windows)]
unsafe extern "system" fn recording_subclass_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
    _uid_subclass: usize,
    _ref_data: usize,
) -> windows_sys::Win32::Foundation::LRESULT {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SC_KEYMENU, WM_SYSCOMMAND};
    // SC_KEYMENU is triggered by Alt+Space (and Alt+key for menu mnemonics).
    // Block it when recording mode is active so the system menu won't appear.
    if msg == WM_SYSCOMMAND && (wparam & 0xFFF0) == SC_KEYMENU as usize {
        if RECORDING_MODE.load(Ordering::Relaxed) {
            return 0;
        }
    }
    windows_sys::Win32::UI::Shell::DefSubclassProc(hwnd, msg, wparam, lparam)
}

#[tauri::command]
pub(crate) fn pause_all_shortcuts(app: tauri::AppHandle) -> Result<(), String> {
    let manager = app.global_shortcut();
    manager.unregister_all().map_err(|e| e.to_string())?;
    RECORDING_MODE.store(true, Ordering::Relaxed);
    // Install subclass on first call; subsequent calls are harmless (same ID = update)
    #[cfg(windows)]
    if let Some(window) = app.get_webview_window("main") {
        if let Ok(hwnd) = window.hwnd() {
            unsafe {
                windows_sys::Win32::UI::Shell::SetWindowSubclass(
                    hwnd.0,
                    Some(recording_subclass_proc),
                    SUBCLASS_ID,
                    0,
                );
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn resume_all_shortcuts(app: tauri::AppHandle) -> Result<(), String> {
    RECORDING_MODE.store(false, Ordering::Relaxed);
    // Subclass remains installed but becomes a no-op (flag is false).
    // No need to remove it — keeps things simple and avoids race conditions.
    sync_all_shortcuts(&app)
}

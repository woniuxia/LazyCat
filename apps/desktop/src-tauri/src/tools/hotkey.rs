use serde_json::Value;
use std::path::PathBuf;
use std::sync::OnceLock;

pub static HOTKEY_MAPPINGS_DIR: OnceLock<PathBuf> = OnceLock::new();

#[cfg(target_os = "windows")]
mod win {
    use super::HOTKEY_MAPPINGS_DIR;
    use serde::{Deserialize, Serialize};
    use serde_json::{json, Value};
    use std::collections::{HashMap, HashSet};
    use std::process::Command;
    use std::thread;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    // ── JSON mapping structures ──────────────────────────────────────────

    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AppHotkeyEntry {
        pub id: String,
        pub display_name: String,
        pub process_names: Vec<String>,
        #[allow(dead_code)]
        pub category: String,
        pub default_hotkeys: Vec<HotkeyMapping>,
    }

    #[derive(Debug, Clone, Deserialize)]
    pub struct HotkeyMapping {
        pub shortcut: String,
        pub action: String,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ShortcutSuspect {
        app_id: String,
        display_name: String,
        confidence: &'static str, // "high" | "low"
        matched_hotkeys: Vec<MatchedHotkey>,
    }

    #[derive(Debug, Serialize)]
    struct MatchedHotkey {
        shortcut: String,
        action: String,
    }

    // ── Mapping loader ───────────────────────────────────────────────────

    fn load_app_hotkey_mappings() -> Vec<AppHotkeyEntry> {
        let dir = match HOTKEY_MAPPINGS_DIR.get() {
            Some(d) => d,
            None => return Vec::new(),
        };
        let path = dir.join("app-hotkey-mappings.json");
        let data = match std::fs::read_to_string(&path) {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        };
        serde_json::from_str(&data).unwrap_or_default()
    }

    // ── Shortcut normalisation ───────────────────────────────────────────

    /// Normalise a shortcut string so that modifier order is canonical
    /// (Ctrl > Alt > Shift > Win) and the key name is uppercase.
    fn normalize_shortcut(s: &str) -> String {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        let mut has_ctrl = false;
        let mut has_alt = false;
        let mut has_shift = false;
        let mut has_win = false;
        let mut key: Option<String> = None;

        for part in &parts {
            match part.to_uppercase().as_str() {
                "CTRL" | "CONTROL" => has_ctrl = true,
                "ALT" => has_alt = true,
                "SHIFT" => has_shift = true,
                "WIN" | "META" | "SUPER" => has_win = true,
                _ => {
                    if key.is_none() {
                        key = Some(part.to_uppercase());
                    }
                }
            }
        }

        let mut out = Vec::new();
        if has_ctrl { out.push("Ctrl"); }
        if has_alt { out.push("Alt"); }
        if has_shift { out.push("Shift"); }
        if has_win { out.push("Win"); }
        if let Some(k) = &key {
            out.push(k);
        }
        out.join("+")
    }

    // ── Process detection ────────────────────────────────────────────────

    /// Parse a shortcut string like "Ctrl+Shift+A" into (modifiers, vk).
    pub fn parse_shortcut(s: &str) -> Result<(u32, u32), String> {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        if parts.len() < 2 {
            return Err(format!("快捷键至少需要一个修饰键和一个普通键: {s}"));
        }

        let mut modifiers: u32 = MOD_NOREPEAT;
        let mut key_name: Option<&str> = None;

        for part in &parts {
            let upper = part.to_uppercase();
            match upper.as_str() {
                "CTRL" | "CONTROL" => modifiers |= MOD_CONTROL,
                "ALT" => modifiers |= MOD_ALT,
                "SHIFT" => modifiers |= MOD_SHIFT,
                "WIN" | "META" | "SUPER" => modifiers |= MOD_WIN,
                _ => {
                    if key_name.is_some() {
                        return Err(format!("快捷键包含多个普通键: {s}"));
                    }
                    key_name = Some(part);
                }
            }
        }

        let key = key_name.ok_or_else(|| format!("快捷键缺少普通键: {s}"))?;
        let vk = key_name_to_vk(key)?;

        // Must have at least one modifier besides MOD_NOREPEAT
        if modifiers == MOD_NOREPEAT {
            return Err(format!("快捷键至少需要一个修饰键 (Ctrl/Alt/Shift/Win): {s}"));
        }

        Ok((modifiers, vk))
    }

    fn key_name_to_vk(name: &str) -> Result<u32, String> {
        let upper = name.to_uppercase();
        match upper.as_str() {
            // Letters A-Z
            s if s.len() == 1 && s.as_bytes()[0].is_ascii_uppercase() => {
                Ok(s.as_bytes()[0] as u32)
            }
            // Digits 0-9
            s if s.len() == 1 && s.as_bytes()[0].is_ascii_digit() => Ok(s.as_bytes()[0] as u32),
            // Function keys F1-F24
            s if s.starts_with('F') && s.len() >= 2 => {
                let num: u32 = s[1..]
                    .parse()
                    .map_err(|_| format!("无效的功能键: {name}"))?;
                if num >= 1 && num <= 24 {
                    Ok(VK_F1 as u32 + num - 1)
                } else {
                    Err(format!("功能键超出范围 (F1-F24): {name}"))
                }
            }
            // Special keys
            "SPACE" | "SPACEBAR" => Ok(VK_SPACE as u32),
            "ENTER" | "RETURN" => Ok(VK_RETURN as u32),
            "TAB" => Ok(VK_TAB as u32),
            "ESC" | "ESCAPE" => Ok(VK_ESCAPE as u32),
            "BACKSPACE" | "BACK" => Ok(VK_BACK as u32),
            "DELETE" | "DEL" => Ok(VK_DELETE as u32),
            "INSERT" | "INS" => Ok(VK_INSERT as u32),
            "HOME" => Ok(VK_HOME as u32),
            "END" => Ok(VK_END as u32),
            "PAGEUP" | "PGUP" => Ok(VK_PRIOR as u32),
            "PAGEDOWN" | "PGDN" => Ok(VK_NEXT as u32),
            "UP" => Ok(VK_UP as u32),
            "DOWN" => Ok(VK_DOWN as u32),
            "LEFT" => Ok(VK_LEFT as u32),
            "RIGHT" => Ok(VK_RIGHT as u32),
            "PRINTSCREEN" | "PRTSC" => Ok(VK_SNAPSHOT as u32),
            "SCROLLLOCK" => Ok(VK_SCROLL as u32),
            "PAUSE" | "BREAK" => Ok(VK_PAUSE as u32),
            "NUMLOCK" => Ok(VK_NUMLOCK as u32),
            "CAPSLOCK" => Ok(VK_CAPITAL as u32),
            // Punctuation / symbols
            ";" | "SEMICOLON" => Ok(VK_OEM_1 as u32),
            "=" | "EQUAL" | "EQUALS" => Ok(VK_OEM_PLUS as u32),
            "," | "COMMA" => Ok(VK_OEM_COMMA as u32),
            "-" | "MINUS" | "DASH" => Ok(VK_OEM_MINUS as u32),
            "." | "PERIOD" | "DOT" => Ok(VK_OEM_PERIOD as u32),
            "/" | "SLASH" => Ok(VK_OEM_2 as u32),
            "`" | "BACKQUOTE" | "GRAVE" => Ok(VK_OEM_3 as u32),
            "[" | "BRACKETLEFT" => Ok(VK_OEM_4 as u32),
            "\\" | "BACKSLASH" => Ok(VK_OEM_5 as u32),
            "]" | "BRACKETRIGHT" => Ok(VK_OEM_6 as u32),
            "'" | "QUOTE" => Ok(VK_OEM_7 as u32),
            // Numpad
            "NUM0" | "NUMPAD0" => Ok(VK_NUMPAD0 as u32),
            "NUM1" | "NUMPAD1" => Ok(VK_NUMPAD1 as u32),
            "NUM2" | "NUMPAD2" => Ok(VK_NUMPAD2 as u32),
            "NUM3" | "NUMPAD3" => Ok(VK_NUMPAD3 as u32),
            "NUM4" | "NUMPAD4" => Ok(VK_NUMPAD4 as u32),
            "NUM5" | "NUMPAD5" => Ok(VK_NUMPAD5 as u32),
            "NUM6" | "NUMPAD6" => Ok(VK_NUMPAD6 as u32),
            "NUM7" | "NUMPAD7" => Ok(VK_NUMPAD7 as u32),
            "NUM8" | "NUMPAD8" => Ok(VK_NUMPAD8 as u32),
            "NUM9" | "NUMPAD9" => Ok(VK_NUMPAD9 as u32),
            _ => Err(format!("无法识别的按键名称: {name}")),
        }
    }

    /// Get the list of running process names (lowercase).
    fn get_running_processes() -> Vec<String> {
        let output = match Command::new("tasklist")
            .args(["/FO", "CSV", "/NH"])
            .output()
        {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(text.as_bytes());
        let mut names: Vec<String> = Vec::new();
        let mut seen = HashSet::new();
        for rec in rdr.records().flatten() {
            if rec.is_empty() {
                continue;
            }
            let name = rec.get(0).unwrap_or("").trim().to_lowercase();
            if !name.is_empty() && seen.insert(name.clone()) {
                names.push(name);
            }
        }
        names
    }

    /// For a single occupied shortcut, find which running apps might own it.
    fn get_shortcut_suspects(
        shortcut: &str,
        mappings: &[AppHotkeyEntry],
        running_set: &HashSet<String>,
    ) -> Vec<ShortcutSuspect> {
        let normalised = normalize_shortcut(shortcut);
        let mut suspects = Vec::new();
        let mut seen_ids = HashSet::new();

        for entry in mappings {
            // Check if any of the app's process names are running
            let is_running = entry
                .process_names
                .iter()
                .any(|p| running_set.contains(&p.to_lowercase()));
            if !is_running {
                continue;
            }
            if !seen_ids.insert(&entry.id) {
                continue;
            }

            // Check for exact hotkey match
            let matched: Vec<MatchedHotkey> = entry
                .default_hotkeys
                .iter()
                .filter(|hk| normalize_shortcut(&hk.shortcut) == normalised)
                .map(|hk| MatchedHotkey {
                    shortcut: hk.shortcut.clone(),
                    action: hk.action.clone(),
                })
                .collect();

            if !matched.is_empty() {
                suspects.push(ShortcutSuspect {
                    app_id: entry.id.clone(),
                    display_name: entry.display_name.clone(),
                    confidence: "high",
                    matched_hotkeys: matched,
                });
            } else {
                // Low confidence: app is running and known to register hotkeys
                suspects.push(ShortcutSuspect {
                    app_id: entry.id.clone(),
                    display_name: entry.display_name.clone(),
                    confidence: "low",
                    matched_hotkeys: Vec::new(),
                });
            }
        }

        suspects
    }

    /// Detect running suspect processes with v2 confidence system.
    /// Returns (per-shortcut suspects map, global suspect list).
    fn detect_suspect_processes_v2(
        occupied_shortcuts: &[String],
    ) -> (HashMap<String, Vec<Value>>, Vec<Value>) {
        let mappings = load_app_hotkey_mappings();
        let running = get_running_processes();
        let running_set: HashSet<String> = running.into_iter().collect();

        // Per-shortcut suspects
        let mut per_shortcut: HashMap<String, Vec<Value>> = HashMap::new();
        for sc in occupied_shortcuts {
            let suspects = get_shortcut_suspects(sc, &mappings, &running_set);
            let suspect_values: Vec<Value> = suspects
                .into_iter()
                .map(|s| {
                    json!({
                        "appId": s.app_id,
                        "displayName": s.display_name,
                        "confidence": s.confidence,
                        "matchedHotkeys": s.matched_hotkeys.iter().map(|mh| {
                            json!({ "shortcut": mh.shortcut, "action": mh.action })
                        }).collect::<Vec<Value>>()
                    })
                })
                .collect();
            per_shortcut.insert(sc.clone(), suspect_values);
        }

        // Global suspect list (deduplicated, all running known-hotkey apps)
        let mut global_suspects: Vec<Value> = Vec::new();
        let mut seen_ids = HashSet::new();
        for entry in &mappings {
            let is_running = entry
                .process_names
                .iter()
                .any(|p| running_set.contains(&p.to_lowercase()));
            if is_running && seen_ids.insert(&entry.id) {
                global_suspects.push(json!({
                    "processName": entry.process_names.first().unwrap_or(&String::new()),
                    "displayName": entry.display_name
                }));
            }
        }

        (per_shortcut, global_suspects)
    }

    /// Check if a single hotkey is available by attempting RegisterHotKey on a worker thread.
    pub fn check_hotkey(shortcut: &str) -> Result<Value, String> {
        let (modifiers, vk) = parse_shortcut(shortcut)?;
        let shortcut_owned = shortcut.to_string();

        // Run on a separate thread to avoid interfering with the main thread's message loop
        let handle = thread::spawn(move || -> (bool, String) {
            unsafe {
                let result = RegisterHotKey(std::ptr::null_mut(), 1, modifiers, vk);
                if result != 0 {
                    UnregisterHotKey(std::ptr::null_mut(), 1);
                    (true, shortcut_owned)
                } else {
                    (false, shortcut_owned)
                }
            }
        });

        let (available, sc) = handle.join().map_err(|_| "检测线程异常退出".to_string())?;

        let (suspects, global_suspects) = if !available {
            let (per, global) = detect_suspect_processes_v2(&[sc.clone()]);
            let sc_suspects = per.get(&sc).cloned().unwrap_or_default();
            (sc_suspects, global)
        } else {
            (Vec::new(), Vec::new())
        };

        Ok(json!({
            "shortcut": sc,
            "available": available,
            "suspects": suspects,
            "suspectedOwners": global_suspects
        }))
    }

    /// Batch scan multiple shortcuts (or use defaults).
    pub fn scan_hotkeys(shortcuts: Option<Vec<String>>) -> Result<Value, String> {
        let list = shortcuts.unwrap_or_else(default_scan_list);

        // Parse all shortcuts first, skip invalid ones
        let parsed: Vec<(String, u32, u32)> = list
            .into_iter()
            .filter_map(|s| parse_shortcut(&s).ok().map(|(m, v)| (s, m, v)))
            .collect();

        // Run all checks on a single worker thread (sequential, each < 1ms)
        let handle = thread::spawn(move || -> Vec<(String, bool)> {
            parsed
                .into_iter()
                .map(|(shortcut, modifiers, vk)| unsafe {
                    let result = RegisterHotKey(std::ptr::null_mut(), 1, modifiers, vk);
                    if result != 0 {
                        UnregisterHotKey(std::ptr::null_mut(), 1);
                        (shortcut, true)
                    } else {
                        (shortcut, false)
                    }
                })
                .collect()
        });

        let results = handle.join().map_err(|_| "扫描线程异常退出".to_string())?;
        let scanned_count = results.len();

        // Collect occupied shortcuts
        let occupied_shortcuts: Vec<String> = results
            .iter()
            .filter(|(_, avail)| !avail)
            .map(|(sc, _)| sc.clone())
            .collect();
        let occupied_count = occupied_shortcuts.len();

        // Detect suspects with v2 confidence system
        let (per_shortcut, global_suspects) = if occupied_count > 0 {
            detect_suspect_processes_v2(&occupied_shortcuts)
        } else {
            (HashMap::new(), Vec::new())
        };

        let result_items: Vec<Value> = results
            .into_iter()
            .map(|(shortcut, available)| {
                let suspects = if !available {
                    per_shortcut.get(&shortcut).cloned().unwrap_or_default()
                } else {
                    Vec::new()
                };
                json!({
                    "shortcut": shortcut,
                    "available": available,
                    "suspects": suspects
                })
            })
            .collect();

        Ok(json!({
            "results": result_items,
            "scannedCount": scanned_count,
            "occupiedCount": occupied_count,
            "suspectedOwners": global_suspects
        }))
    }

    /// Return full mappings database for frontend consumption.
    pub fn get_mappings() -> Result<Value, String> {
        let mappings = load_app_hotkey_mappings();
        let values: Vec<Value> = mappings
            .iter()
            .map(|entry| {
                json!({
                    "id": entry.id,
                    "displayName": entry.display_name,
                    "processNames": entry.process_names,
                    "category": entry.category,
                    "defaultHotkeys": entry.default_hotkeys.iter().map(|hk| {
                        json!({ "shortcut": hk.shortcut, "action": hk.action })
                    }).collect::<Vec<Value>>()
                })
            })
            .collect();
        Ok(json!({ "mappings": values }))
    }

    fn default_scan_list() -> Vec<String> {
        let mut list = Vec::new();

        // Ctrl+Shift+A~Z
        for c in b'A'..=b'Z' {
            list.push(format!("Ctrl+Shift+{}", c as char));
        }
        // Ctrl+Alt+A~Z
        for c in b'A'..=b'Z' {
            list.push(format!("Ctrl+Alt+{}", c as char));
        }
        // Alt+1~9
        for n in 1..=9 {
            list.push(format!("Alt+{n}"));
        }
        // Ctrl+F1~F12
        for n in 1..=12 {
            list.push(format!("Ctrl+F{n}"));
        }
        // Ctrl+Shift+F1~F12
        for n in 1..=12 {
            list.push(format!("Ctrl+Shift+F{n}"));
        }
        // Win+Shift+A~Z (common utility shortcuts)
        for c in b'A'..=b'Z' {
            list.push(format!("Win+Shift+{}", c as char));
        }

        list
    }

    // ── Window / Process snapshot for detect_owner ────────────────────────

    #[derive(Debug, Clone)]
    struct WindowInfo {
        hwnd: HWND,
        pid: u32,
        title: String,
    }

    #[derive(Debug)]
    struct SystemSnapshot {
        foreground_hwnd: HWND,
        foreground_pid: u32,
        visible_windows: Vec<WindowInfo>,
        clipboard_seq: u32,
    }

    /// Get PID from HWND
    fn hwnd_to_pid(hwnd: HWND) -> u32 {
        let mut pid: u32 = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
        }
        pid
    }

    /// Get window title from HWND
    fn get_window_title(hwnd: HWND) -> String {
        let mut buf = [0u16; 512];
        let len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        if len > 0 {
            String::from_utf16_lossy(&buf[..len as usize])
        } else {
            String::new()
        }
    }

    /// Get full exe path from PID
    fn get_exe_path(pid: u32) -> Option<String> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return None;
            }
            let mut buf = [0u16; 1024];
            let mut size = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size);
            windows_sys::Win32::Foundation::CloseHandle(handle);
            if ok != 0 && size > 0 {
                Some(String::from_utf16_lossy(&buf[..size as usize]))
            } else {
                None
            }
        }
    }

    /// Extract filename from a full path
    fn path_to_filename(path: &str) -> String {
        path.rsplit('\\')
            .next()
            .or_else(|| path.rsplit('/').next())
            .unwrap_or(path)
            .to_string()
    }

    /// Callback context for EnumWindows
    struct EnumCtx {
        windows: Vec<WindowInfo>,
    }

    unsafe extern "system" fn enum_windows_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam as *mut EnumCtx);
        if IsWindowVisible(hwnd) != 0 {
            let pid = hwnd_to_pid(hwnd);
            let title = get_window_title(hwnd);
            // Only collect windows with a title (real app windows)
            if !title.is_empty() {
                ctx.windows.push(WindowInfo {
                    hwnd,
                    pid,
                    title,
                });
            }
        }
        1 // TRUE = continue enumeration
    }

    /// Take a snapshot of current system state
    fn snapshot_state() -> SystemSnapshot {
        unsafe {
            let fg = GetForegroundWindow();
            let fg_pid = hwnd_to_pid(fg);
            let clip_seq = GetClipboardSequenceNumber();

            let mut ctx = EnumCtx {
                windows: Vec::new(),
            };
            EnumWindows(Some(enum_windows_cb), &mut ctx as *mut EnumCtx as LPARAM);

            SystemSnapshot {
                foreground_hwnd: fg,
                foreground_pid: fg_pid,
                visible_windows: ctx.windows,
                clipboard_seq: clip_seq,
            }
        }
    }

    /// Parse shortcut into (modifier VKs, main VK) for SendInput
    fn parse_shortcut_for_send(s: &str) -> Result<(Vec<u16>, u16), String> {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        let mut modifiers = Vec::new();
        let mut main_key: Option<&str> = None;

        for part in &parts {
            let upper = part.to_uppercase();
            match upper.as_str() {
                "CTRL" | "CONTROL" => modifiers.push(VK_CONTROL),
                "ALT" => modifiers.push(VK_MENU),
                "SHIFT" => modifiers.push(VK_SHIFT),
                "WIN" | "META" | "SUPER" => modifiers.push(VK_LWIN),
                _ => {
                    if main_key.is_some() {
                        return Err(format!("快捷键包含多个普通键: {s}"));
                    }
                    main_key = Some(part);
                }
            }
        }

        let key = main_key.ok_or_else(|| format!("快捷键缺少普通键: {s}"))?;
        let vk = key_name_to_vk(key)? as u16;
        Ok((modifiers, vk))
    }

    /// Build a KEYBDINPUT for SendInput
    fn make_key_input(vk: u16, flags: u32) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    /// Simulate a hotkey press via SendInput
    fn simulate_hotkey(modifier_vks: &[u16], main_vk: u16) {
        let mut inputs = Vec::new();

        // Press modifiers
        for &vk in modifier_vks {
            inputs.push(make_key_input(vk, 0));
        }
        // Press main key
        inputs.push(make_key_input(main_vk, 0));
        // Release main key
        inputs.push(make_key_input(main_vk, KEYEVENTF_KEYUP));
        // Release modifiers (reverse order)
        for &vk in modifier_vks.iter().rev() {
            inputs.push(make_key_input(vk, KEYEVENTF_KEYUP));
        }

        unsafe {
            SendInput(
                inputs.len() as u32,
                inputs.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            );
        }
    }

    /// Detect which process owns a hotkey by simulating the key press and observing changes
    pub fn detect_hotkey_owner(shortcut: &str) -> Result<Value, String> {
        let (modifier_vks, main_vk) = parse_shortcut_for_send(shortcut)?;

        // 1. Take snapshot
        let before = snapshot_state();

        // 2. Simulate the hotkey
        simulate_hotkey(&modifier_vks, main_vk);

        // 3. Wait for the target app to respond
        thread::sleep(std::time::Duration::from_millis(400));

        // 4. Take after snapshot
        let after = snapshot_state();

        // 5. Try to restore foreground window
        if after.foreground_hwnd != before.foreground_hwnd {
            unsafe {
                SetForegroundWindow(before.foreground_hwnd);
            }
        }

        // 6. Analyze changes
        let fg_changed = after.foreground_hwnd != before.foreground_hwnd;
        let clip_changed = after.clipboard_seq != before.clipboard_seq;

        // Check for new windows
        let before_hwnds: HashSet<usize> = before.visible_windows.iter().map(|w| w.hwnd as usize).collect();
        let new_windows: Vec<&WindowInfo> = after
            .visible_windows
            .iter()
            .filter(|w| !before_hwnds.contains(&(w.hwnd as usize)))
            .collect();
        let new_window_appeared = !new_windows.is_empty();

        // Determine owner
        let (detected, owner_pid, owner_title, confidence) = if fg_changed {
            // Foreground window changed -> high confidence
            let pid = after.foreground_pid;
            let title = get_window_title(after.foreground_hwnd);
            (true, Some(pid), Some(title), "high")
        } else if new_window_appeared {
            // New window appeared -> medium confidence
            let w = new_windows[0];
            (true, Some(w.pid), Some(w.title.clone()), "medium")
        } else if clip_changed {
            // Only clipboard changed -> low confidence (probably screenshot tool)
            // Can't determine PID directly, check if foreground changed even briefly
            (true, None, None, "low")
        } else {
            (false, None, None, "none")
        };

        // Get process info if we have a PID
        let owner = if let Some(pid) = owner_pid {
            let exe_path = get_exe_path(pid).unwrap_or_default();
            let process_name = if exe_path.is_empty() {
                String::from("unknown")
            } else {
                path_to_filename(&exe_path)
            };
            let title = owner_title.unwrap_or_default();
            Some(json!({
                "pid": pid,
                "processName": process_name,
                "windowTitle": title,
                "exePath": exe_path,
            }))
        } else {
            None
        };

        Ok(json!({
            "shortcut": shortcut,
            "detected": detected,
            "owner": owner,
            "signals": {
                "foregroundChanged": fg_changed,
                "newWindowAppeared": new_window_appeared,
                "clipboardChanged": clip_changed,
            },
            "confidence": confidence,
        }))
    }
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    #[cfg(target_os = "windows")]
    {
        match action {
            "check" => {
                let shortcut = payload
                    .get("shortcut")
                    .and_then(|v| v.as_str())
                    .ok_or("缺少 shortcut 参数")?;
                win::check_hotkey(shortcut)
            }
            "scan" => {
                let shortcuts = payload
                    .get("shortcuts")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect::<Vec<_>>()
                    });
                // If empty array provided, treat as None (use defaults)
                let shortcuts =
                    shortcuts.and_then(|v| if v.is_empty() { None } else { Some(v) });
                win::scan_hotkeys(shortcuts)
            }
            "mappings" => win::get_mappings(),
            "detect_owner" => {
                let shortcut = payload
                    .get("shortcut")
                    .and_then(|v| v.as_str())
                    .ok_or("缺少 shortcut 参数")?;
                win::detect_hotkey_owner(shortcut)
            }
            _ => Err(format!("unsupported hotkey action: {action}")),
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (action, payload);
        Err("快捷键冲突检测仅支持 Windows 平台".to_string())
    }
}

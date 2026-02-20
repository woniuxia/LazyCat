use serde_json::Value;

#[cfg(target_os = "windows")]
mod win {
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::process::Command;
    use std::thread;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

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
        let suspects = if !available {
            detect_suspect_processes()
        } else {
            Vec::new()
        };

        Ok(json!({
            "shortcut": sc,
            "available": available,
            "suspectedOwners": suspects
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
        let occupied_count = results.iter().filter(|(_, avail)| !avail).count();

        // Only detect suspects if there are occupied hotkeys
        let suspects = if occupied_count > 0 {
            detect_suspect_processes()
        } else {
            Vec::new()
        };

        let result_items: Vec<Value> = results
            .into_iter()
            .map(|(shortcut, available)| {
                json!({ "shortcut": shortcut, "available": available })
            })
            .collect();

        Ok(json!({
            "results": result_items,
            "scannedCount": scanned_count,
            "occupiedCount": occupied_count,
            "suspectedOwners": suspects
        }))
    }

    /// Known applications that commonly register global hotkeys on Windows.
    /// Maps process name (lowercase, with .exe) to display name.
    const KNOWN_HOTKEY_APPS: &[(&str, &str)] = &[
        ("discord.exe", "Discord"),
        ("slack.exe", "Slack"),
        ("teams.exe", "Microsoft Teams"),
        ("ms-teams.exe", "Microsoft Teams"),
        ("wechat.exe", "WeChat (微信)"),
        ("wecom.exe", "WeCom (企业微信)"),
        ("wxwork.exe", "WeCom (企业微信)"),
        ("qq.exe", "QQ"),
        ("dingtalk.exe", "DingTalk (钉钉)"),
        ("lark.exe", "Lark (飞书)"),
        ("feishu.exe", "Feishu (飞书)"),
        ("telegram.exe", "Telegram"),
        ("sharex.exe", "ShareX"),
        ("snipaste.exe", "Snipaste"),
        ("greenshot.exe", "Greenshot"),
        ("snagit32.exe", "Snagit"),
        ("snagiteditor.exe", "Snagit"),
        ("flameshot.exe", "Flameshot"),
        ("picpick.exe", "PicPick"),
        ("obs64.exe", "OBS Studio"),
        ("obs32.exe", "OBS Studio"),
        ("steam.exe", "Steam"),
        ("steamwebhelper.exe", "Steam"),
        ("gameoverlayui.exe", "Steam Overlay"),
        ("nvidia share.exe", "NVIDIA GeForce Experience"),
        ("nvspcaps64.exe", "NVIDIA GeForce Experience"),
        ("nvcontainer.exe", "NVIDIA Container"),
        ("code.exe", "Visual Studio Code"),
        ("devenv.exe", "Visual Studio"),
        ("idea64.exe", "IntelliJ IDEA"),
        ("webstorm64.exe", "WebStorm"),
        ("pycharm64.exe", "PyCharm"),
        ("goland64.exe", "GoLand"),
        ("rider64.exe", "Rider"),
        ("powertoys.exe", "PowerToys"),
        ("powertoys.runner.exe", "PowerToys"),
        ("powertoys.fancyzones.exe", "PowerToys FancyZones"),
        ("everything.exe", "Everything"),
        ("listary.exe", "Listary"),
        ("utools.exe", "uTools"),
        ("autohotkey.exe", "AutoHotkey"),
        ("autohotkey64.exe", "AutoHotkey"),
        ("autohotkey32.exe", "AutoHotkey"),
        ("keypirinha-x64.exe", "Keypirinha"),
        ("clipboardfusion.exe", "ClipboardFusion"),
        ("ditto.exe", "Ditto"),
        ("1clipboard.exe", "1Clipboard"),
        ("bandizip.exe", "Bandizip"),
        ("potplayer.exe", "PotPlayer"),
        ("potplayer64.exe", "PotPlayer"),
        ("potplayermini.exe", "PotPlayer"),
        ("potplayermini64.exe", "PotPlayer"),
        ("foobar2000.exe", "Foobar2000"),
        ("spotify.exe", "Spotify"),
        ("cloudmusic.exe", "NetEase Cloud Music (网易云音乐)"),
        ("qqmusic.exe", "QQ Music (QQ音乐)"),
        ("kugou.exe", "Kugou Music (酷狗音乐)"),
        ("kuwo.exe", "Kuwo Music (酷我音乐)"),
        ("quicker.exe", "Quicker"),
        ("captura.exe", "Captura"),
        ("faststone.exe", "FastStone Capture"),
        ("fscapture.exe", "FastStone Capture"),
        ("screentogif.exe", "ScreenToGif"),
        ("clipclip.exe", "ClipClip"),
        ("nomacs.exe", "nomacs"),
        ("logioptionsplus.exe", "Logi Options+"),
        ("microsoftedge.exe", "Microsoft Edge"),
    ];

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
        let mut seen = std::collections::HashSet::new();
        for rec in rdr.records().flatten() {
            if rec.len() < 1 {
                continue;
            }
            let name = rec.get(0).unwrap_or("").trim().to_lowercase();
            if !name.is_empty() && seen.insert(name.clone()) {
                names.push(name);
            }
        }
        names
    }

    /// Detect running processes known to register global hotkeys.
    fn detect_suspect_processes() -> Vec<Value> {
        let running = get_running_processes();
        let running_set: std::collections::HashSet<&str> =
            running.iter().map(|s| s.as_str()).collect();

        let mut suspects: Vec<Value> = Vec::new();
        let mut seen_display = std::collections::HashSet::new();

        for &(proc_name, display_name) in KNOWN_HOTKEY_APPS {
            if running_set.contains(proc_name) && seen_display.insert(display_name) {
                suspects.push(json!({
                    "processName": proc_name,
                    "displayName": display_name
                }));
            }
        }

        suspects
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
            _ => Err(format!("unsupported hotkey action: {action}")),
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (action, payload);
        Err("快捷键冲突检测仅支持 Windows 平台".to_string())
    }
}

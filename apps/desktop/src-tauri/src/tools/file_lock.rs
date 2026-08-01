use serde_json::{json, Value};
use std::fs;
use std::path::Path;

const ACTIONS: &[&str] = &["inspect"];

#[cfg(test)]
pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("unsupported file_lock action: {action}"));
    }

    match action {
        "inspect" => inspect(payload),
        _ => Err(format!("unsupported file_lock action: {action}")),
    }
}

fn inspect(payload: &Value) -> Result<Value, String> {
    let raw_path = payload
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "path is required".to_string())?;
    let input_path = Path::new(raw_path);
    let metadata = fs::metadata(input_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!("file not found: {raw_path}")
        } else {
            format!("cannot access file: {raw_path}: {error}")
        }
    })?;
    if !metadata.is_file() {
        return Err(format!("path is not a file: {raw_path}"));
    }

    let canonical_path = input_path
        .canonicalize()
        .map_err(|error| format!("canonicalize file failed: {raw_path}: {error}"))?;

    #[cfg(windows)]
    {
        inspect_windows(raw_path, &canonical_path)
    }

    #[cfg(not(windows))]
    {
        let _ = canonical_path;
        Err("file lock inspection is only supported on Windows".to_string())
    }
}

#[cfg(windows)]
fn inspect_windows(raw_path: &str, canonical_path: &Path) -> Result<Value, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::RestartManager::{
        RmEndSession, RmRegisterResources, RmStartSession, CCH_RM_SESSION_KEY,
    };

    struct SessionGuard(u32);

    impl Drop for SessionGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = RmEndSession(self.0);
            }
        }
    }

    let mut session_handle = 0u32;
    let mut session_key = [0u16; (CCH_RM_SESSION_KEY + 1) as usize];
    let status = unsafe { RmStartSession(&mut session_handle, 0, session_key.as_mut_ptr()) };
    if status != ERROR_SUCCESS {
        return Err(format_windows_error("RmStartSession", status));
    }
    let _session = SessionGuard(session_handle);

    let filename: Vec<u16> = canonical_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let filenames = [filename.as_ptr()];
    let status = unsafe {
        RmRegisterResources(
            session_handle,
            1,
            filenames.as_ptr(),
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
        )
    };
    if status != ERROR_SUCCESS {
        return Err(format_windows_error("RmRegisterResources", status));
    }

    let affected_processes = get_affected_processes(session_handle)?;
    let mut warnings = Vec::new();
    let mut processes = Vec::with_capacity(affected_processes.len());

    for process_info in affected_processes {
        let pid = process_info.Process.dwProcessId;
        let app_name = wide_string(&process_info.strAppName);
        let app_name = if app_name.is_empty() {
            warnings.push(format!("PID {pid} 未返回应用名"));
            "UNKNOWN".to_string()
        } else {
            app_name
        };
        let executable_path = match process_executable_path(pid) {
            Ok(path) => Some(path),
            Err(error) => {
                warnings.push(format!("PID {pid} 的可执行文件路径读取失败: {error}"));
                None
            }
        };

        processes.push(json!({
            "pid": pid,
            "appName": app_name,
            "appType": app_type_label(process_info.ApplicationType),
            "status": app_status_label(process_info.AppStatus),
            "executablePath": executable_path,
        }));
    }
    processes.sort_by_key(|process| process["pid"].as_u64().unwrap_or(0));

    Ok(json!({
        "path": raw_path,
        "canonicalPath": canonical_path.to_string_lossy(),
        "scannedAt": chrono::Utc::now().to_rfc3339(),
        "processes": processes,
        "warnings": warnings,
    }))
}

#[cfg(windows)]
fn get_affected_processes(
    session_handle: u32,
) -> Result<Vec<windows_sys::Win32::System::RestartManager::RM_PROCESS_INFO>, String> {
    use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows_sys::Win32::System::RestartManager::{RmGetList, RM_PROCESS_INFO};

    for _ in 0..3 {
        let mut process_count_needed = 0u32;
        let mut process_count = 0u32;
        let mut reboot_reasons = 0u32;
        let status = unsafe {
            RmGetList(
                session_handle,
                &mut process_count_needed,
                &mut process_count,
                std::ptr::null_mut(),
                &mut reboot_reasons,
            )
        };
        if status == ERROR_SUCCESS && process_count_needed == 0 {
            return Ok(Vec::new());
        }
        if status != ERROR_MORE_DATA && status != ERROR_SUCCESS {
            return Err(format_windows_error("RmGetList", status));
        }

        let capacity = process_count_needed.max(1) as usize;
        let mut processes: Vec<RM_PROCESS_INFO> = vec![unsafe { std::mem::zeroed() }; capacity];
        process_count = capacity as u32;
        let status = unsafe {
            RmGetList(
                session_handle,
                &mut process_count_needed,
                &mut process_count,
                processes.as_mut_ptr(),
                &mut reboot_reasons,
            )
        };
        if status == ERROR_MORE_DATA {
            continue;
        }
        if status != ERROR_SUCCESS {
            return Err(format_windows_error("RmGetList", status));
        }
        processes.truncate(process_count as usize);
        return Ok(processes);
    }

    Err("RmGetList returned a changing process list".to_string())
}

#[cfg(windows)]
fn process_executable_path(pid: u32) -> Result<String, String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return Err("OpenProcess denied".to_string());
        }

        let mut buffer = [0u16; 32_768];
        let mut length = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut length);
        let _ = CloseHandle(handle);
        if result == 0 || length == 0 {
            return Err("QueryFullProcessImageNameW failed".to_string());
        }
        Ok(String::from_utf16_lossy(&buffer[..length as usize]))
    }
}

#[cfg(windows)]
fn format_windows_error(api: &str, status: u32) -> String {
    format!("{api} failed with Windows error {status}")
}

#[cfg(windows)]
fn wide_string(value: &[u16]) -> String {
    let end = value
        .iter()
        .position(|item| *item == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..end])
}

#[cfg(windows)]
fn app_type_label(value: i32) -> &'static str {
    use windows_sys::Win32::System::RestartManager::{
        RmConsole as RM_CONSOLE, RmCritical as RM_CRITICAL, RmExplorer as RM_EXPLORER,
        RmMainWindow as RM_MAIN_WINDOW, RmOtherWindow as RM_OTHER_WINDOW, RmService as RM_SERVICE,
    };

    match value {
        RM_MAIN_WINDOW => "main-window",
        RM_OTHER_WINDOW => "other-window",
        RM_SERVICE => "service",
        RM_EXPLORER => "explorer",
        RM_CONSOLE => "console",
        RM_CRITICAL => "critical",
        _ => "unknown",
    }
}

#[cfg(windows)]
fn app_status_label(value: u32) -> &'static str {
    use windows_sys::Win32::System::RestartManager::{
        RmStatusRunning, RmStatusStopped, RmStatusStoppedOther,
    };

    if value & RmStatusRunning as u32 != 0 {
        "running"
    } else if value & RmStatusStopped as u32 != 0 {
        "stopped"
    } else if value & RmStatusStoppedOther as u32 != 0 {
        "stopped-other"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn exposes_only_inspect_action() {
        assert_eq!(supported_actions(), &["inspect"]);
    }

    #[test]
    fn inspect_requires_a_non_empty_file_path() {
        let empty = execute("inspect", &json!({})).expect_err("missing path");
        assert_eq!(empty, "path is required");

        let whitespace = execute("inspect", &json!({ "path": "  " })).expect_err("empty path");
        assert_eq!(whitespace, "path is required");
    }

    #[test]
    fn inspect_rejects_missing_path() {
        let error = execute("inspect", &json!({ "path": "Z:\\lazycat\\missing.file" }))
            .expect_err("missing file");
        assert!(error.contains("file not found") || error.contains("only supported on Windows"));
    }

    #[cfg(windows)]
    #[test]
    fn inspect_existing_file_returns_a_stable_response_shape() {
        let file = tempfile::NamedTempFile::new().expect("temporary file");
        let response = execute(
            "inspect",
            &json!({ "path": file.path().to_string_lossy().to_string() }),
        )
        .expect("inspect temporary file");

        assert_eq!(response["path"], file.path().to_string_lossy().as_ref());
        assert!(response["canonicalPath"].as_str().is_some());
        assert!(response["scannedAt"].as_str().is_some());
        assert!(response["processes"].is_array());
        assert!(response["warnings"].is_array());
    }

    #[test]
    fn unsupported_action_is_explicit() {
        let error = execute("close_handle", &json!({})).expect_err("unsupported action");
        assert!(error.contains("unsupported file_lock action"));
    }
}

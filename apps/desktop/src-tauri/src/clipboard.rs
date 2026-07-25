#[cfg(test)]
use std::time::Duration;

#[cfg(windows)]
pub(crate) struct ClipboardGuard;

#[cfg(windows)]
impl ClipboardGuard {
    pub(crate) fn open() -> Result<Self, String> {
        use windows_sys::Win32::Foundation::HWND;
        use windows_sys::Win32::System::DataExchange::OpenClipboard;

        unsafe {
            if OpenClipboard(HWND::default()) == 0 {
                return Err("打开剪贴板失败".to_string());
            }
        }
        Ok(Self)
    }
}

#[cfg(windows)]
impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::System::DataExchange::CloseClipboard();
        }
    }
}

#[cfg(windows)]
pub(crate) fn read_unicode_text_from_open_clipboard() -> Result<Option<String>, String> {
    use windows_sys::Win32::System::DataExchange::{GetClipboardData, IsClipboardFormatAvailable};
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalUnlock};

    const CF_UNICODETEXT: u32 = 13;
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
            return Ok(None);
        }
        let handle = GetClipboardData(CF_UNICODETEXT);
        if handle.is_null() {
            return Err("读取剪贴板文本失败".to_string());
        }
        let pointer = GlobalLock(handle) as *const u16;
        if pointer.is_null() {
            return Err("锁定剪贴板文本失败".to_string());
        }
        let mut length = 0usize;
        while *pointer.add(length) != 0 {
            length += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(pointer, length));
        GlobalUnlock(handle);
        Ok(Some(text))
    }
}

#[cfg(not(windows))]
pub(crate) fn read_unicode_text_from_open_clipboard() -> Result<Option<String>, String> {
    Ok(None)
}

#[cfg(test)]
pub(crate) fn retry_read<T, F>(attempts: usize, mut read: F) -> Result<T, String>
where
    F: FnMut() -> Result<T, String>,
{
    let mut last_error = None;
    for attempt in 0..attempts {
        match read() {
            Ok(value) => return Ok(value),
            Err(error) => last_error = Some(error),
        }
        if attempt + 1 < attempts {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    Err(last_error.unwrap_or_else(|| "读取剪贴板失败".to_string()))
}

#[cfg(test)]
pub(crate) fn read_unicode_text_with_retry() -> Result<Option<String>, String> {
    retry_read(3, || {
        #[cfg(windows)]
        let _guard = ClipboardGuard::open()?;
        read_unicode_text_from_open_clipboard()
    })
}

#[cfg(test)]
mod tests {
    use super::{read_unicode_text_with_retry, retry_read};
    const _: fn() -> Result<Option<String>, String> = read_unicode_text_with_retry;
    #[test]
    fn retry_read_succeeds_on_third_attempt() {
        let mut attempts = 0;
        let result = retry_read(3, || {
            attempts += 1;
            if attempts < 3 {
                Err(format!("failure-{attempts}"))
            } else {
                Ok("done")
            }
        });
        assert_eq!(result, Ok("done"));
        assert_eq!(attempts, 3);
    }
    #[test]
    fn retry_read_returns_last_error_when_exhausted() {
        let mut attempts = 0;
        let error = retry_read::<(), _>(3, || {
            attempts += 1;
            Err(format!("failure-{attempts}"))
        })
        .expect_err("retries should be exhausted");
        assert_eq!(error, "failure-3");
        assert_eq!(attempts, 3);
    }
}

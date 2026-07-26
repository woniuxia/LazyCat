use std::time::Duration;

fn utf16_unit_count(byte_len: usize) -> Result<usize, String> {
    if byte_len == 0 {
        return Err("剪贴板文本缓冲区为空".to_string());
    }
    if byte_len % size_of::<u16>() != 0 {
        return Err("剪贴板文本缓冲区字节长度必须为偶数".to_string());
    }
    Ok(byte_len / size_of::<u16>())
}

fn decode_null_terminated_utf16(units: &[u16]) -> Result<String, String> {
    let length = units
        .iter()
        .position(|unit| *unit == 0)
        .ok_or_else(|| "剪贴板文本缺少 NUL 终止符".to_string())?;
    Ok(String::from_utf16_lossy(&units[..length]))
}

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
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    const CF_UNICODETEXT: u32 = 13;
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
            return Ok(None);
        }
        let handle = GetClipboardData(CF_UNICODETEXT);
        if handle.is_null() {
            return Err("读取剪贴板文本失败".to_string());
        }
        let unit_count = utf16_unit_count(GlobalSize(handle))?;
        let pointer = GlobalLock(handle) as *const u16;
        if pointer.is_null() {
            return Err("锁定剪贴板文本失败".to_string());
        }
        let units = std::slice::from_raw_parts(pointer, unit_count);
        let result = decode_null_terminated_utf16(units);
        GlobalUnlock(handle);
        result.map(Some)
    }
}

#[cfg(not(windows))]
pub(crate) fn read_unicode_text_from_open_clipboard() -> Result<Option<String>, String> {
    Ok(None)
}

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

pub(crate) fn read_unicode_text_with_retry() -> Result<Option<String>, String> {
    retry_read(3, || {
        #[cfg(windows)]
        let _guard = ClipboardGuard::open()?;
        read_unicode_text_from_open_clipboard()
    })
}

#[cfg(test)]
mod tests {
    use super::{
        decode_null_terminated_utf16, read_unicode_text_with_retry, retry_read, utf16_unit_count,
    };
    const _: fn() -> Result<Option<String>, String> = read_unicode_text_with_retry;
    #[test]
    fn utf16_unit_count_rejects_empty_buffer() {
        assert_eq!(utf16_unit_count(0).unwrap_err(), "剪贴板文本缓冲区为空");
    }
    #[test]
    fn utf16_unit_count_rejects_odd_byte_length() {
        assert_eq!(
            utf16_unit_count(3).unwrap_err(),
            "剪贴板文本缓冲区字节长度必须为偶数"
        );
        assert_eq!(utf16_unit_count(4), Ok(2));
    }
    #[test]
    fn decode_utf16_accepts_nul_at_slice_boundary() {
        assert_eq!(
            decode_null_terminated_utf16(&[0x4f60, 0x597d, 0]),
            Ok("你好".to_string())
        );
    }
    #[test]
    fn decode_utf16_rejects_missing_nul() {
        assert_eq!(
            decode_null_terminated_utf16(&[0x4f60, 0x597d]).unwrap_err(),
            "剪贴板文本缺少 NUL 终止符"
        );
    }
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

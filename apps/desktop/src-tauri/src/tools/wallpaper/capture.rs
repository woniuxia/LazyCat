//! WebView2 `CapturePreview` 抓图（plan §2.1）
//!
//! 从 `wallpaper_poc.rs` 抽取的低层原语，供 PoC 与正式 apply 路径共用：
//!
//! - [`capture_inner`]：在 `with_webview` 闭包内同步调用，取 PNG 字节
//! - 内部 `pump_messages`：抽 Win32 消息队列驱动 COM 回调
//! - 内部 `read_stream_to_vec`：把 `IStream` 全量拉到 `Vec<u8>`
//!
//! 整文件用 `cfg(windows)` 门控；非 Windows 平台提供 stub 让 wallpaper 模块
//! 在跨平台构建下保持可编译。

#![allow(dead_code)] // Phase 2.2+ 由 hidden WebView 调度方接入

#[cfg(windows)]
pub use imp::capture_inner;

#[cfg(not(windows))]
pub fn capture_inner(_webview: tauri::webview::PlatformWebview) -> Result<Vec<u8>, String> {
    Err("wallpaper::capture::capture_inner: Windows only".into())
}

#[cfg(windows)]
mod imp {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use webview2_com::CapturePreviewCompletedHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG;
    use windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal;
    use windows::Win32::System::Com::{IStream, STREAM_SEEK_SET};

    /// CapturePreview 回调最长等待；与 PoC 实测 P99 留 ~10x 余量。
    const PUMP_TIMEOUT: Duration = Duration::from_secs(5);
    /// 消息泵循环间歇睡眠；不能为 0 否则 100% CPU。
    const PUMP_TICK: Duration = Duration::from_millis(2);

    /// 同步抓取 webview 当前帧的 PNG 字节。
    ///
    /// 必须在 `WebviewWindow::with_webview(...)` 闭包内调用，因为
    /// `PlatformWebview::controller()` 仅在 UI 线程有效。
    ///
    /// 失败原因：CoreWebView2 不可用、CreateStreamOnHGlobal 失败、
    /// CapturePreview 直接报 HRESULT、消息泵超时。
    pub fn capture_inner(
        webview: tauri::webview::PlatformWebview,
    ) -> Result<Vec<u8>, String> {
        unsafe {
            let controller = webview.controller();
            let core = controller
                .CoreWebView2()
                .map_err(|e| format!("CoreWebView2 failed: {e:?}"))?;

            let stream: IStream = CreateStreamOnHGlobal(Default::default(), true)
                .map_err(|e| format!("CreateStreamOnHGlobal failed: {e:?}"))?;

            let stream_for_cb = stream.clone();
            let bytes_slot: Arc<Mutex<Result<Vec<u8>, String>>> =
                Arc::new(Mutex::new(Err("callback not invoked".to_string())));
            let bytes_slot_cb = bytes_slot.clone();
            let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let done_cb = done.clone();

            let handler = CapturePreviewCompletedHandler::create(Box::new(move |hr| {
                let result = if hr.is_err() {
                    Err(format!("CapturePreview HRESULT: {hr:?}"))
                } else {
                    read_stream_to_vec(&stream_for_cb)
                };
                *bytes_slot_cb.lock().unwrap() = result;
                done_cb.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }));

            core.CapturePreview(
                COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                &stream,
                &handler,
            )
            .map_err(|e| format!("CapturePreview call failed: {e:?}"))?;

            let pump_start = Instant::now();
            while !done.load(std::sync::atomic::Ordering::SeqCst) {
                if pump_start.elapsed() > PUMP_TIMEOUT {
                    return Err(format!(
                        "capture_inner timeout ({}s pump)",
                        PUMP_TIMEOUT.as_secs()
                    ));
                }
                pump_messages();
                std::thread::sleep(PUMP_TICK);
            }

            let _ = stream.Seek(0, STREAM_SEEK_SET, None);
            let res = bytes_slot.lock().unwrap();
            match &*res {
                Ok(v) => Ok(v.clone()),
                Err(e) => Err(e.clone()),
            }
        }
    }

    /// 排空当前线程的 Win32 消息队列；驱动 WebView2 的 COM 回调到达。
    unsafe fn pump_messages() {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
        };
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, Some(HWND(std::ptr::null_mut())), 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    /// 把 `IStream`（HGLOBAL 流）从头读到尾返回 `Vec<u8>`。
    unsafe fn read_stream_to_vec(stream: &IStream) -> Result<Vec<u8>, String> {
        stream
            .Seek(0, STREAM_SEEK_SET, None)
            .map_err(|e| format!("Seek failed: {e:?}"))?;

        let mut buf = Vec::<u8>::with_capacity(64 * 1024);
        let mut chunk = [0u8; 8192];
        loop {
            let mut read: u32 = 0;
            let _ = stream.Read(
                chunk.as_mut_ptr() as *mut _,
                chunk.len() as u32,
                Some(&mut read as *mut u32),
            );
            if read == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..read as usize]);
        }
        Ok(buf)
    }
}

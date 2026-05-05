// B1 Spike: WebView2 CapturePreview PoC
// 仅 debug 构建启用真实实现；release 构建为 stub。
// 验证 hidden Tauri WebView 能否通过 ICoreWebView2::CapturePreview
// 输出非黑帧，并测量耗时（P1-P5）。
//
// 关联文档：docs/superpowers/spikes/2026-05-05-wallpaper-screenshot.md

use serde::Serialize;
use tauri::AppHandle;

#[derive(Debug, Serialize)]
pub struct CaptureResult {
    pub ok: bool,
    pub elapsed_ms: u128,
    pub bytes: usize,
    pub path: Option<String>,
    pub error: Option<String>,
}

#[cfg(all(windows, debug_assertions))]
mod imp {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    pub const POC_WINDOW_LABEL: &str = "wallpaper-poc-canvas";
    const LOGICAL_WIDTH: f64 = 360.0;
    const LOGICAL_HEIGHT: f64 = 800.0;

    pub async fn open(app: AppHandle, visible: Option<bool>) -> Result<String, String> {
        if app.get_webview_window(POC_WINDOW_LABEL).is_some() {
            return Ok(format!("window already exists, label={POC_WINDOW_LABEL}"));
        }

        let url = WebviewUrl::App("index.html?view=wallpaper-poc-canvas".into());
        let visible = visible.unwrap_or(false);

        WebviewWindowBuilder::new(&app, POC_WINDOW_LABEL, url)
            .title("Wallpaper PoC Canvas")
            .inner_size(LOGICAL_WIDTH, LOGICAL_HEIGHT)
            .visible(visible)
            .decorations(false)
            .resizable(false)
            .skip_taskbar(true)
            .build()
            .map_err(|e| format!("build window failed: {e}"))?;

        Ok(format!(
            "window created, label={POC_WINDOW_LABEL}, visible={visible}"
        ))
    }

    pub async fn show(app: AppHandle, visible: bool) -> Result<(), String> {
        let win = app
            .get_webview_window(POC_WINDOW_LABEL)
            .ok_or_else(|| "window not open".to_string())?;
        if visible {
            win.show().map_err(|e| e.to_string())?;
        } else {
            win.hide().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub async fn close(app: AppHandle) -> Result<(), String> {
        if let Some(win) = app.get_webview_window(POC_WINDOW_LABEL) {
            win.close().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub async fn capture(app: AppHandle, output_path: String) -> Result<CaptureResult, String> {
        let win = app
            .get_webview_window(POC_WINDOW_LABEL)
            .ok_or_else(|| "window not open, call wallpaper_poc_open first".to_string())?;

        let started = Instant::now();
        let result_slot: Arc<Mutex<Option<Result<Vec<u8>, String>>>> =
            Arc::new(Mutex::new(None));
        let slot_for_cb = result_slot.clone();

        win.with_webview(move |webview| {
            let res = capture_inner(webview);
            *slot_for_cb.lock().unwrap() = Some(res);
        })
        .map_err(|e| format!("with_webview failed: {e}"))?;

        let result = wait_for_result(&result_slot, std::time::Duration::from_secs(8))
            .ok_or_else(|| "capture wait timed out (8s)".to_string())?;

        let elapsed = started.elapsed().as_millis();

        match result {
            Ok(bytes) => {
                let path_buf = std::path::PathBuf::from(&output_path);
                if let Some(parent) = path_buf.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                std::fs::write(&path_buf, &bytes).map_err(|e| e.to_string())?;
                Ok(CaptureResult {
                    ok: true,
                    elapsed_ms: elapsed,
                    bytes: bytes.len(),
                    path: Some(output_path),
                    error: None,
                })
            }
            Err(e) => Ok(CaptureResult {
                ok: false,
                elapsed_ms: elapsed,
                bytes: 0,
                path: None,
                error: Some(e),
            }),
        }
    }

    fn wait_for_result<T>(
        slot: &Arc<Mutex<Option<T>>>,
        timeout: std::time::Duration,
    ) -> Option<T> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if let Some(v) = slot.lock().unwrap().take() {
                return Some(v);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        None
    }

    fn capture_inner(webview: tauri::webview::PlatformWebview) -> Result<Vec<u8>, String> {
        use webview2_com::CapturePreviewCompletedHandler;
        use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG;
        use windows::Win32::System::Com::{IStream, STREAM_SEEK_SET};
        use windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal;

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
                if pump_start.elapsed() > std::time::Duration::from_secs(5) {
                    return Err("capture inner timeout (5s pump)".into());
                }
                pump_messages();
                std::thread::sleep(std::time::Duration::from_millis(2));
            }

            let _ = stream.Seek(0, STREAM_SEEK_SET, None);
            let res = bytes_slot.lock().unwrap();
            match &*res {
                Ok(v) => Ok(v.clone()),
                Err(e) => Err(e.clone()),
            }
        }
    }

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

    unsafe fn read_stream_to_vec(
        stream: &windows::Win32::System::Com::IStream,
    ) -> Result<Vec<u8>, String> {
        use windows::Win32::System::Com::STREAM_SEEK_SET;
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

#[cfg(not(all(windows, debug_assertions)))]
mod imp {
    use super::*;
    pub async fn open(_app: AppHandle, _visible: Option<bool>) -> Result<String, String> {
        Err("wallpaper_poc only available in windows debug builds".into())
    }
    pub async fn show(_app: AppHandle, _visible: bool) -> Result<(), String> {
        Err("wallpaper_poc only available in windows debug builds".into())
    }
    pub async fn close(_app: AppHandle) -> Result<(), String> {
        Err("wallpaper_poc only available in windows debug builds".into())
    }
    pub async fn capture(
        _app: AppHandle,
        _output_path: String,
    ) -> Result<CaptureResult, String> {
        Err("wallpaper_poc only available in windows debug builds".into())
    }
}

#[tauri::command]
pub async fn wallpaper_poc_open(app: AppHandle, visible: Option<bool>) -> Result<String, String> {
    imp::open(app, visible).await
}

#[tauri::command]
pub async fn wallpaper_poc_show(app: AppHandle, visible: bool) -> Result<(), String> {
    imp::show(app, visible).await
}

#[tauri::command]
pub async fn wallpaper_poc_close(app: AppHandle) -> Result<(), String> {
    imp::close(app).await
}

#[tauri::command]
pub async fn wallpaper_poc_capture(
    app: AppHandle,
    output_path: String,
) -> Result<CaptureResult, String> {
    imp::capture(app, output_path).await
}


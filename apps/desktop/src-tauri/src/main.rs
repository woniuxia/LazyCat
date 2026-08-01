#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod background;
mod clipboard;
mod events;
mod global_notification;
mod manual_server;
mod reference_card;
mod shortcuts;
mod tools;
mod window_manager;

#[cfg(windows)]
pub(crate) use window_manager::force_foreground;
pub(crate) use window_manager::{
    navigate_main_window_to_tool, navigate_main_window_to_tool_context,
};

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::PageLoadEvent,
    Emitter, Manager, RunEvent,
};
use tauri_plugin_autostart::MacosLauncher;

use events::EVENT_MAIN_WINDOW_TOGGLE;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Instant;

use tools::hotkey::HOTKEY_MAPPINGS_DIR;
use tools::regex::REGEX_TEMPLATES_DIR;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct ToolRequest {
    request_id: String,
    domain: String,
    action: String,
    payload: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct ToolError {
    code: String,
    message: String,
    details: Option<Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct ToolMeta {
    duration_ms: u128,
    warnings: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct ToolResponse {
    request_id: String,
    ok: bool,
    data: Option<Value>,
    error: Option<ToolError>,
    meta: ToolMeta,
}

#[tauri::command]
fn tool_execute(app: tauri::AppHandle, request: ToolRequest) -> ToolResponse {
    let start = Instant::now();
    match tools::execute_tool_with_app(&request.domain, &request.action, &request.payload, &app) {
        Ok(data) => ToolResponse {
            request_id: request.request_id,
            ok: true,
            data: Some(data),
            error: None,
            meta: ToolMeta {
                duration_ms: start.elapsed().as_millis(),
                warnings: None,
            },
        },
        Err(message) => ToolResponse {
            request_id: request.request_id,
            ok: false,
            data: None,
            error: Some(ToolError {
                code: "TOOL_EXECUTION_FAILED".to_string(),
                message,
                details: None,
            }),
            meta: ToolMeta {
                duration_ms: start.elapsed().as_millis(),
                warnings: None,
            },
        },
    }
}

#[tauri::command]
async fn request_forward_preflight(payload: Value) -> Result<Value, ToolError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        tools::request_forward::execute("preflight", &payload)
    })
    .await
    .map_err(|error| {
        let message = tools::request_forward::encode_preflight_task_error(&format!(
            "配置预检任务异常结束: {error}"
        ));
        ToolError {
            code: "REQUEST_FORWARD_PREFLIGHT_TASK_FAILED".to_string(),
            message,
            details: None,
        }
    })?;

    result.map_err(|message| ToolError {
        code: "TOOL_EXECUTION_FAILED".to_string(),
        message,
        details: None,
    })
}

fn main() {
    // 绿色免安装包支持：检测 exe 同级目录下的 WebView2 Fixed Runtime，
    // 设置环境变量让 WebView2Loader.dll 使用本地运行时而非系统安装版本。
    // 必须在 Tauri 初始化之前调用。
    #[cfg(target_os = "windows")]
    {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                // 查找 exe 同级的 Microsoft.WebView2.FixedVersionRuntime.* 目录
                if let Ok(entries) = std::fs::read_dir(exe_dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name();
                        let name_str = name.to_string_lossy();
                        if name_str.starts_with("Microsoft.WebView2.FixedVersionRuntime.")
                            && entry.path().is_dir()
                        {
                            unsafe {
                                std::env::set_var(
                                    "WEBVIEW2_BROWSER_EXECUTABLE_FOLDER",
                                    entry.path(),
                                );
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    let mut builder = tauri::Builder::default();
    #[cfg(windows)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            window_manager::reveal_main_window(app);
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.emit(EVENT_MAIN_WINDOW_TOGGLE, json!({}));
            }
        }));
    }

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .on_page_load(|webview, payload| {
            if let PageLoadEvent::Finished = payload.event() {
                if let Some(title) = window_manager::expected_window_title(webview.window().label())
                {
                    // 绿色包在 Win10 + WebView2 下可能把 HTML 标题里的中文解错，
                    // 这里在页面加载完成后强制回写原生标题，绕开错误同步链路。
                    let _ = webview.window().set_title(title);
                }
            }
        })
        .setup(|app| {
            tools::request_forward::initialize_manager().map_err(std::io::Error::other)?;
            std::thread::spawn(
                || match tools::request_forward::restore_auto_start_rules() {
                    Ok(results) => {
                        for result in results.into_iter().filter(|result| !result.ok) {
                            eprintln!(
                                "request-forward restore failed for rule {}: {}",
                                result.rule_id,
                                result.error.as_deref().unwrap_or("未知错误")
                            );
                        }
                    }
                    Err(error) => eprintln!("request-forward restore failed: {error}"),
                },
            );

            // 允许附件目录通过 asset:// 协议访问，覆盖默认目录与用户自定义数据目录两种场景
            if let Ok(dir) = tools::helpers::get_attachments_dir() {
                if let Err(e) = app.asset_protocol_scope().allow_directory(&dir, true) {
                    eprintln!("allow attachments dir failed: {e}");
                }
            }

            // 启动离线文档 HTTP 服务器
            // 打包后从 resource_dir/manuals 读取；开发模式下 fallback 到源码目录
            let manuals_dir = {
                let rd = app.path().resource_dir().ok().map(|d| d.join("manuals"));
                if rd.as_ref().is_some_and(|d| d.exists()) {
                    rd.unwrap()
                } else {
                    // 开发模式：src-tauri/../../../resources/manuals
                    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../../resources/manuals");
                    std::fs::canonicalize(&dev).unwrap_or(dev)
                }
            };
            manual_server::initialize_manual_servers(&manuals_dir);

            // 初始化正则模板目录
            let regex_dir = {
                let rd = app
                    .path()
                    .resource_dir()
                    .ok()
                    .map(|d| d.join("regex-library"));
                if rd
                    .as_ref()
                    .is_some_and(|d| d.join("templates.json").exists())
                {
                    rd.unwrap()
                } else {
                    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../../resources/regex-library");
                    std::fs::canonicalize(&dev).unwrap_or(dev)
                }
            };
            let _ = REGEX_TEMPLATES_DIR.set(regex_dir);

            // 初始化热键映射目录
            let hotkey_dir = {
                let rd = app
                    .path()
                    .resource_dir()
                    .ok()
                    .map(|d| d.join("hotkey-library"));
                if rd
                    .as_ref()
                    .is_some_and(|d| d.join("app-hotkey-mappings.json").exists())
                {
                    rd.unwrap()
                } else {
                    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../../resources/hotkey-library");
                    std::fs::canonicalize(&dev).unwrap_or(dev)
                }
            };
            let _ = HOTKEY_MAPPINGS_DIR.set(hotkey_dir);

            // 检查是否为开机自启动且用户设置了启动时最小化
            let args: Vec<String> = std::env::args().collect();
            let is_autostart = args.contains(&"--minimized".to_string());

            if is_autostart {
                // 读取用户设置
                if let Ok(conn) = tools::helpers::db_conn() {
                    let value: Result<String, _> = conn.query_row(
                        "SELECT value FROM user_settings WHERE key = ?1",
                        ["autostart_minimized"],
                        |row| row.get(0),
                    );
                    if let Ok(val) = value {
                        if val == "true" {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.hide();
                            }
                        }
                    }
                }
            }

            let show_item = MenuItem::with_id(app, "show", "显示", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().expect("app icon missing").clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        window_manager::reveal_main_window(app);
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.emit(EVENT_MAIN_WINDOW_TOGGLE, json!({}));
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        window_manager::handle_main_window_shortcut(app, "toggle");
                    }
                })
                .build(app)?;

            // 启动待办调度线程（周期实例生成 + 到期提醒派发）
            if let Err(error) = tools::action_center::recover_interrupted_dispatches() {
                eprintln!("action-center recovery failed: {error}");
            }
            if let Err(error) = tools::action_center::recover_interrupted_combination_runs() {
                eprintln!("action-center combination recovery failed: {error}");
            }
            background::start_todo_scheduler(app.handle().clone());
            background::start_pomodoro_scheduler(app.handle().clone());
            background::start_clipboard_monitor(app.handle().clone());
            tools::vault::start_auto_lock_monitor(app.handle().clone());

            // 启动挂件统一脉冲调度（心跳 + 事件 debounce + 看门狗 + 跨日立刷）
            tools::widget::pulse::start(app.handle().clone());

            // 延迟预创建 spotlight 隐藏窗口，首次呼出仅需 show/focus，避免 WebView 冷启动延迟
            let app_for_preload = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(500));
                let handle_inner = app_for_preload.clone();
                let _ = app_for_preload.run_on_main_thread(move || {
                    if handle_inner
                        .get_webview_window(window_manager::SPOTLIGHT_LABEL)
                        .is_some()
                    {
                        return;
                    }
                    let _ = window_manager::build_spotlight_window(&handle_inner);
                });
            });

            Ok(())
        })
        .on_window_event(window_manager::handle_window_event)
        .invoke_handler(tauri::generate_handler![
            tool_execute,
            request_forward_preflight,
            shortcuts::register_hotkey,
            shortcuts::unregister_hotkey,
            shortcuts::register_named_hotkey,
            shortcuts::unregister_named_hotkey,
            shortcuts::pause_all_shortcuts,
            shortcuts::resume_all_shortcuts,
            background::reminder_popup_complete,
            background::reminder_popup_snooze,
            background::reminder_popup_dismiss,
            global_notification::global_notification_open_tool,
            global_notification::global_notification_open_action_run,
            background::suppress_clipboard_capture,
            window_manager::spotlight_open,
            window_manager::spotlight_pick,
            window_manager::spotlight_close,
            reference_card::reference_card_show,
            reference_card::reference_card_ready,
            tools::access_path_diagnostics::runtime::diagnosis_start,
            tools::access_path_diagnostics::runtime::diagnosis_get,
            tools::access_path_diagnostics::runtime::diagnosis_cancel,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // 应用退出时取消长任务并销毁挂件窗口
            if let RunEvent::ExitRequested { .. } = event {
                tools::release_package_runtime::on_app_exit();
                tools::access_path_diagnostics::runtime::on_app_exit();
                tools::request_forward::on_app_exit();
                tools::widget::on_app_exit(app_handle);
            }
        });
}

use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::events::{
    EVENT_CLIPBOARD_CHANGED, EVENT_FOLLOW_UP_REVIEW_DUE, EVENT_POMODORO_STATE_CHANGED,
    EVENT_TODO_REMINDER_FIRED,
};
use crate::{global_notification, tools, window_manager};

static CLIPBOARD_MONITOR_RUNNING: AtomicBool = AtomicBool::new(false);

pub(crate) fn emit_todo_refresh_event(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit(EVENT_TODO_REMINDER_FIRED, json!({ "refresh": true }));
    }
}

fn emit_follow_up_refresh_event(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit(EVENT_FOLLOW_UP_REVIEW_DUE, json!({ "refresh": true }));
    }
}

#[tauri::command]
pub(crate) fn reminder_popup_complete(
    app: tauri::AppHandle,
    task_id: i64,
) -> Result<Value, String> {
    let result = tools::todo::execute(
        "item_change_status",
        &json!({
            "id": task_id,
            "status": "completed",
            "kind": "one_off",
            "recordRole": "occurrence"
        }),
    )?;
    emit_todo_refresh_event(&app);
    Ok(result)
}

#[tauri::command]
pub(crate) fn reminder_popup_snooze(
    app: tauri::AppHandle,
    task_id: i64,
    task_reminder_id: i64,
    minutes: i64,
) -> Result<Value, String> {
    let result = tools::todo::execute(
        "item_snooze",
        &json!({
            "id": task_id,
            "taskReminderId": task_reminder_id,
            "minutes": minutes
        }),
    )?;
    emit_todo_refresh_event(&app);
    Ok(result)
}

#[tauri::command]
pub(crate) fn follow_up_popup_snooze(
    app: tauri::AppHandle,
    item_id: i64,
    minutes: i64,
) -> Result<Value, String> {
    let result =
        tools::follow_up::execute("item_snooze", &json!({ "id": item_id, "minutes": minutes }))?;
    emit_follow_up_refresh_event(&app);
    Ok(result)
}

#[tauri::command]
pub(crate) fn reminder_popup_dismiss(
    app: tauri::AppHandle,
    event_id: i64,
) -> Result<Value, String> {
    let result = tools::todo::execute("reminder_mark_read", &json!({ "id": event_id }))?;
    emit_todo_refresh_event(&app);
    Ok(result)
}

#[tauri::command]
pub(crate) fn suppress_clipboard_capture(content: String) -> Result<Value, String> {
    tools::inbox::suppress_clipboard_capture(&content)?;
    Ok(json!({ "ok": true }))
}

pub(crate) fn start_todo_scheduler(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        match tools::todo::scheduler_tick() {
            Ok(reminders) => {
                if !reminders.is_empty() {
                    global_notification::show_notifications(
                        &app,
                        global_notification::todo_notifications(reminders.clone()),
                    );

                    if let Some(window) = app.get_webview_window("main") {
                        for reminder in reminders {
                            let _ = window.emit(EVENT_TODO_REMINDER_FIRED, &reminder);
                        }
                    }
                }
            }
            Err(_) => {
                // 调度失败不影响主流程，等待下一轮重试
            }
        }

        match tools::follow_up::scheduler_tick() {
            Ok(reminders) if !reminders.is_empty() => {
                let notifications = global_notification::follow_up_notifications(reminders.clone());
                if let Err(display_error) =
                    global_notification::try_show_notifications(&app, notifications)
                {
                    eprintln!(
                        "follow-up notification display failed; pending reminders will retry: {display_error}"
                    );
                } else {
                    match tools::follow_up::acknowledge_scheduler_dispatches(&reminders) {
                        Ok(()) => emit_follow_up_refresh_event(&app),
                        Err(error) => eprintln!(
                            "follow-up notification displayed but acknowledgement failed; reminders may retry: {error}"
                        ),
                    }
                }
            }
            Ok(_) => {}
            Err(error) => eprintln!("follow-up scheduler tick failed: {error}"),
        }

        std::thread::sleep(Duration::from_secs(30));
    });
}

fn emit_pomodoro_refresh_event(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit(EVENT_POMODORO_STATE_CHANGED, json!({ "refresh": true }));
    }
}

pub(crate) fn start_pomodoro_scheduler(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        match tools::pomodoro::scheduler_tick(chrono::Local::now()) {
            Ok(Some(_prompt)) => {
                window_manager::show_pomodoro_prompt(&app);
                emit_pomodoro_refresh_event(&app);
            }
            Ok(None) => {}
            Err(_) => {
                // 番茄钟调度失败不影响主流程，等待下一轮重试
            }
        }

        std::thread::sleep(Duration::from_secs(30));
    });
}

pub(crate) fn start_clipboard_monitor(app: tauri::AppHandle) {
    if CLIPBOARD_MONITOR_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    #[cfg(windows)]
    std::thread::spawn(move || {
        use windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber;

        let mut last_seq = unsafe { GetClipboardSequenceNumber() };

        loop {
            std::thread::sleep(Duration::from_millis(700));

            let current_seq = unsafe { GetClipboardSequenceNumber() };
            if current_seq == last_seq {
                continue;
            }
            last_seq = current_seq;

            let window_visible = app
                .get_webview_window("main")
                .and_then(|window| window.is_visible().ok())
                .unwrap_or(true);

            let _ = tools::inbox::process_clipboard_change(window_visible);

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.emit(EVENT_CLIPBOARD_CHANGED, json!({ "sequence": current_seq }));
            }
        }
    });

    #[cfg(not(windows))]
    let _ = app;
}

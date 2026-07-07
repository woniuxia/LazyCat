pub const EVENT_MAIN_WINDOW_TOGGLE: &str = "main-window-toggle";
pub const EVENT_HOTKEY_NAVIGATE: &str = "hotkey-navigate";
pub const EVENT_CLIPBOARD_CHANGED: &str = "clipboard-changed";
pub const EVENT_TODO_REMINDER_FIRED: &str = "todo-reminder-fired";
pub const EVENT_REMINDER_PUSH: &str = "reminder-push";
pub const EVENT_POMODORO_STATE_CHANGED: &str = "pomodoro-state-changed";
pub const EVENT_QUICK_CAPTURE_RESET: &str = "quick-capture-reset";
pub const EVENT_SPOTLIGHT_RESET: &str = "spotlight-reset";
pub const EVENT_WIDGET_COLOR_MODE: &str = "widget://color-mode";
pub const EVENT_WIDGET_DASHBOARD_DATA: &str = "widget://dashboard-data";
pub const EVENT_WIDGET_NAVIGATE: &str = "widget://navigate";

/// 供契约对账测试使用；由具名常量引用组成，无双写漂移。
pub const ALL: &[&str] = &[
    EVENT_MAIN_WINDOW_TOGGLE,
    EVENT_HOTKEY_NAVIGATE,
    EVENT_CLIPBOARD_CHANGED,
    EVENT_TODO_REMINDER_FIRED,
    EVENT_REMINDER_PUSH,
    EVENT_POMODORO_STATE_CHANGED,
    EVENT_QUICK_CAPTURE_RESET,
    EVENT_SPOTLIGHT_RESET,
    EVENT_WIDGET_COLOR_MODE,
    EVENT_WIDGET_DASHBOARD_DATA,
    EVENT_WIDGET_NAVIGATE,
];

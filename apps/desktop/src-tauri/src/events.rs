pub const EVENT_MAIN_WINDOW_TOGGLE: &str = "main-window-toggle";
pub const EVENT_HOTKEY_NAVIGATE: &str = "hotkey-navigate";
pub const EVENT_CLIPBOARD_CHANGED: &str = "clipboard-changed";
pub const EVENT_TODO_REMINDER_FIRED: &str = "todo-reminder-fired";
pub const EVENT_GLOBAL_NOTIFICATION_PUSH: &str = "global-notification-push";
pub const EVENT_POMODORO_STATE_CHANGED: &str = "pomodoro-state-changed";
pub const EVENT_QUICK_CAPTURE_RESET: &str = "quick-capture-reset";
pub const EVENT_SPOTLIGHT_RESET: &str = "spotlight-reset";
pub const EVENT_WIDGET_COLOR_MODE: &str = "widget://color-mode";
pub const EVENT_WIDGET_DASHBOARD_DATA: &str = "widget://dashboard-data";
pub const EVENT_WIDGET_NAVIGATE: &str = "widget://navigate";
pub const EVENT_ACCESS_PATH_DIAGNOSIS_SNAPSHOT: &str = "access-path-diagnosis://snapshot";
pub const EVENT_RELEASE_PACKAGE_LOG: &str = "release-package://log";
pub const EVENT_RELEASE_PACKAGE_STATUS: &str = "release-package://status";
pub const EVENT_VAULT_LOCKED: &str = "vault://locked";
pub const EVENT_ACTION_CENTER_DISPATCH_REQUEST: &str = "action-center://dispatch-request";
pub const EVENT_REFERENCE_CARD_INIT: &str = "reference-card://init";
pub const EVENT_ACTION_CENTER_COMBINATION_RUN_UPDATED: &str =
    "action-center://combination-run-updated";

/// 供契约对账测试使用；由具名常量引用组成，无双写漂移。
#[cfg(test)]
pub const ALL: &[&str] = &[
    EVENT_MAIN_WINDOW_TOGGLE,
    EVENT_HOTKEY_NAVIGATE,
    EVENT_CLIPBOARD_CHANGED,
    EVENT_TODO_REMINDER_FIRED,
    EVENT_GLOBAL_NOTIFICATION_PUSH,
    EVENT_POMODORO_STATE_CHANGED,
    EVENT_QUICK_CAPTURE_RESET,
    EVENT_SPOTLIGHT_RESET,
    EVENT_WIDGET_COLOR_MODE,
    EVENT_WIDGET_DASHBOARD_DATA,
    EVENT_WIDGET_NAVIGATE,
    EVENT_ACCESS_PATH_DIAGNOSIS_SNAPSHOT,
    EVENT_RELEASE_PACKAGE_LOG,
    EVENT_RELEASE_PACKAGE_STATUS,
    EVENT_VAULT_LOCKED,
    EVENT_ACTION_CENTER_DISPATCH_REQUEST,
    EVENT_REFERENCE_CARD_INIT,
    EVENT_ACTION_CENTER_COMBINATION_RUN_UPDATED,
];

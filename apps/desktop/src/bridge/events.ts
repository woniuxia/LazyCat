// 本文件被 src-tauri 契约对账测试逐行解析：常量保持 `NAME: "value",` 一行一条目。
export const APP_EVENTS = {
  /** Rust -> 主窗口：托盘/快捷键切换主窗口显隐 */
  MAIN_WINDOW_TOGGLE: "main-window-toggle",
  /** Rust -> 主窗口：命名快捷键导航 payload HotkeyNavigatePayload */
  HOTKEY_NAVIGATE: "hotkey-navigate",
  /** Rust -> 主窗口：剪贴板序列号变化 payload { sequence: number } */
  CLIPBOARD_CHANGED: "clipboard-changed",
  /** Rust -> 主窗口：Todo 提醒触发 payload ReminderDispatch 或 { refresh: true } */
  TODO_REMINDER_FIRED: "todo-reminder-fired",
  /** Rust -> 全局通知弹窗：通知队列 payload GlobalNotification[] */
  GLOBAL_NOTIFICATION_PUSH: "global-notification-push",
  /** Rust / 前端弹窗 -> 主窗口：番茄钟状态变化 payload { refresh: true } */
  POMODORO_STATE_CHANGED: "pomodoro-state-changed",
  /** Rust -> 快速捕获窗口：重置输入 */
  QUICK_CAPTURE_RESET: "quick-capture-reset",
  /** Rust -> Spotlight 窗口：重置查询 */
  SPOTLIGHT_RESET: "spotlight-reset",
  /** 仅前端：Spotlight 窗口 -> 主窗口 payload { name: string } */
  HOSTS_APPLIED: "hosts-applied",
  /** Rust -> Widget 窗口：颜色模式 payload string */
  WIDGET_COLOR_MODE: "widget://color-mode",
  /** Rust -> Widget 窗口：仪表盘数据 payload WidgetDashboardData */
  WIDGET_DASHBOARD_DATA: "widget://dashboard-data",
  /** Rust -> 主窗口：Widget 快捷导航 payload { kind: string; toolId?: string } */
  WIDGET_NAVIGATE: "widget://navigate",
  /** Rust -> 启动诊断的窗口：访问链路诊断运行快照 */
  ACCESS_PATH_DIAGNOSIS_SNAPSHOT: "access-path-diagnosis://snapshot",
  RELEASE_PACKAGE_LOG: "release-package://log",
  RELEASE_PACKAGE_STATUS: "release-package://status",
  /** Rust -> 主窗口：密码库因空闲策略进入锁定状态 */
  VAULT_LOCKED: "vault://locked",
  /** Rust -> 主窗口：动作中心打开目标工具并请求确认 */
  ACTION_CENTER_DISPATCH_REQUEST: "action-center://dispatch-request",
  /** Rust -> 参考卡窗口：初始化正文 payload { content: string } */
  REFERENCE_CARD_INIT: "reference-card://init",
  /** Rust -> 主窗口：组合动作运行状态变化，payload { runId, status } */
  ACTION_CENTER_COMBINATION_RUN_UPDATED: "action-center://combination-run-updated",
} as const;

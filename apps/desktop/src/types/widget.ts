// Desktop Widget 类型定义
//
// v2 (挂件改造)：删除原 PNG 链路相关类型，统一为 widget 架构。

export type WidgetPauseReason = "fullscreen" | "lock" | "manual";

export type WidgetPriority = "P0" | "P1" | "P2" | "P3";

export type WidgetItemSource = "pm" | "todo";

export interface WidgetHotTool {
  /** 工具 ID（如 "pm", "inbox"） */
  id: string;
  /** 近 30 天点击次数 */
  count: number;
}

export interface WidgetTodoItem {
  /** `pm:<id>` | `todo:<id>` */
  id: string;
  title: string;
  priority: WidgetPriority;
  pinned: boolean;
  /** ISO 日期，无截止则为 null */
  endAt: string | null;
  status: string;
  source: WidgetItemSource;
}

export interface WidgetDashboardData {
  todoList: WidgetTodoItem[];
  generatedAt: string;
  /** design §9 敏感模式：开启时 canvas 把 todo 标题打码 */
  privacyMask?: boolean;
  /** 动态工具推荐（id + count，name 由前端查 toolCatalog） */
  hotTools: WidgetHotTool[];
}

export interface WidgetStatus {
  enabled: boolean;
  paused: boolean;
  pauseReason?: WidgetPauseReason | null;
  lastRenderedAt: string | null;
  lastError?: string | null;
  spotlightDetected: boolean;
  thirdPartyEngine?: string | null;
  /** 当前敏感模式开启状态（与 config.privacyMask 联动；自动到期时由后端清零） */
  privacyMaskActive?: boolean;
  /** 敏感模式自动到期时间（ISO；null = 直到手动关） */
  privacyMaskUntil?: string | null;
  /** 调度上轮自动跳过原因（"lock" | "fullscreen"）；null = 未跳过 */
  autoSkipReason?: "lock" | "fullscreen" | null;
}

export interface WidgetConfig {
  enabled: boolean;
  style: string;
  refreshIntervalMin: number;
  fullscreenBlacklist: string[];
  privacyMask: boolean;
  /** ISO 时间字符串，null = 永久 */
  privacyMaskUntil: string | null;
  /** 挂件 Y 位置（物理像素）；null = 居中 */
  widgetY: number | null;
  /** 挂件停靠边："left" | "right" */
  edge: "left" | "right";
}

export interface WidgetHealth {
  status: string;
  visualState: string;
  lastPingSecsAgo: number;
  lastApplySecsAgo: number;
  lastApplyResult: string;
  todaySkipCount: number;
  todayWatchdogCount: number;
  todayRebuildCount: number;
}

export interface WidgetEventEntry {
  sequenceId: number;
  timestamp: string;
  type: string;
  detail: string;
}

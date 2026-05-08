// Living Wallpaper 类型定义
// 关联设计：docs/superpowers/specs/2026-05-05-living-wallpaper-design.md (v0.5)
// 关联实施：docs/superpowers/specs/2026-05-05-living-wallpaper-plan.md

export type WallpaperPosition =
  | "right"
  | "left"
  | "top"
  | "bottom"
  | "tl"
  | "tr"
  | "bl"
  | "br";

export type WallpaperStyle = "dashboard" | "sticky" | "banner";

export type WallpaperExitBehavior = "keep_last" | "restore_original";

export type WallpaperImageFormat = "jpeg" | "png";

export type WallpaperPauseReason = "boss_key" | "fullscreen" | "lock" | "manual";

export type WallpaperPriority = "P0" | "P1" | "P2" | "P3";

export type WallpaperItemSource = "pm" | "todo";

export interface WallpaperOverview {
  completedToday: number;
  totalToday: number;
  p0Pending: number;
  /** null = 无截止 */
  nearestDeadlineHours: number | null;
}

export interface WallpaperTodoItem {
  /** `pm:<id>` | `todo:<id>` */
  id: string;
  title: string;
  priority: WallpaperPriority;
  pinned: boolean;
  /** ISO 日期，无截止则为 null */
  endAt: string | null;
  status: string;
  source: WallpaperItemSource;
}

export interface WallpaperDashboardData {
  overview: WallpaperOverview;
  todoList: WallpaperTodoItem[];
  /** 阶段 1 始终为 null（扩展位预留） */
  echo: string | null;
  generatedAt: string;
  /** design §9 敏感模式：开启时 canvas 把 todo 标题打码 */
  privacyMask?: boolean;
}

export interface WallpaperStatus {
  enabled: boolean;
  paused: boolean;
  pauseReason?: WallpaperPauseReason | null;
  originalPath: string | null;
  lastRenderedAt: string | null;
  lastRenderedPath: string | null;
  lastError?: string | null;
  spotlightDetected: boolean;
  thirdPartyEngine?: string | null;
  /** design §9 老板键注册失败时的提示文案；null/undefined = 正常 */
  bossKeyError?: string | null;
  /** 当前敏感模式开启状态（与 config.privacyMask 联动；自动到期时由后端清零） */
  privacyMaskActive?: boolean;
  /** 敏感模式自动到期时间（ISO；null = 直到手动关） */
  privacyMaskUntil?: string | null;
}

export interface WallpaperConfig {
  enabled: boolean;
  style: WallpaperStyle;
  position: WallpaperPosition;
  refreshIntervalMin: number;
  fullscreenBlacklist: string[];
  privacyMask: boolean;
  /** ISO 时间字符串，null = 永久 */
  privacyMaskUntil: string | null;
  exitBehavior: WallpaperExitBehavior;
  bossKey: string;
  imageFormat: WallpaperImageFormat;
  keepHistoryCount: number;
}

export interface WallpaperHistoryEntry {
  path: string;
  size: number;
  /** ISO 时间字符串 */
  createdAt: string;
}

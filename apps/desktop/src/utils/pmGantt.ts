import type { Task } from "frappe-gantt";

import type { PmItem, PmItemStatus } from "../types/pm";
import { PM_PRIORITY_MAP, PM_STATUS_COLUMNS } from "../types/pm";
import {
  hasPmDateSchedule,
  isPmItemOverdue,
  normalizePmDateString,
} from "./pmDate";

export interface PmGanttTask extends Task {
  itemId: number;
  status: PmItemStatus;
  statusLabel: string;
  priorityLabel: string;
  projectName: string | null;
  projectColor: string | null;
  startLabel: string;
  endLabel: string;
  pinned: boolean;
  overdue: boolean;
}

export interface PmGanttPopupPositionInput {
  anchorX: number;
  anchorY: number;
  popupWidth: number;
  popupHeight: number;
  viewportWidth: number;
  viewportHeight: number;
  scrollLeft: number;
  scrollTop: number;
  padding?: number;
  gap?: number;
}

export interface PmGanttPopupPosition {
  left: number;
  top: number;
}

const STATUS_PROGRESS_MAP: Record<PmItemStatus, number> = {
  todo: 0,
  in_progress: 40,
  testing: 75,
  done: 100,
};

function normalizeDateRange(start: string, end: string): { start: string; end: string } {
  if (start <= end) {
    return { start, end };
  }
  return { start: end, end: start };
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

export function hasPmGanttSchedule(item: PmItem): boolean {
  return hasPmDateSchedule(item.startAt, item.endAt);
}

export function countPmGanttUnscheduledItems(items: PmItem[]): number {
  return items.filter((item) => !hasPmGanttSchedule(item)).length;
}

export function getPmGanttProgress(status: PmItemStatus): number {
  return STATUS_PROGRESS_MAP[status];
}

export function isPmGanttItemOverdue(item: PmItem): boolean {
  return isPmItemOverdue(item);
}

export function clampPmGanttPopupPosition(
  input: PmGanttPopupPositionInput,
): PmGanttPopupPosition {
  const padding = Math.max(0, input.padding ?? 12);
  const gap = Math.max(0, input.gap ?? 10);
  const viewportLeft = input.scrollLeft + padding;
  const viewportTop = input.scrollTop + padding;
  const viewportRight = input.scrollLeft + input.viewportWidth - padding;
  const viewportBottom = input.scrollTop + input.viewportHeight - padding;

  let left = input.anchorX + gap;
  if (left + input.popupWidth > viewportRight) {
    left = input.anchorX - input.popupWidth - gap;
  }

  let top = input.anchorY - gap;
  if (top + input.popupHeight > viewportBottom) {
    top = input.anchorY - input.popupHeight - gap;
  }

  return {
    left: Math.min(Math.max(left, viewportLeft), Math.max(viewportLeft, viewportRight - input.popupWidth)),
    top: Math.min(Math.max(top, viewportTop), Math.max(viewportTop, viewportBottom - input.popupHeight)),
  };
}

export function buildPmGanttTask(item: PmItem): PmGanttTask {
  const rawStart = normalizePmDateString(item.startAt)
    ?? normalizePmDateString(item.endAt)
    ?? normalizePmDateString(item.createdAt)
    ?? item.createdAt.slice(0, 10);
  const rawEnd = normalizePmDateString(item.endAt)
    ?? normalizePmDateString(item.startAt)
    ?? normalizePmDateString(item.createdAt)
    ?? item.createdAt.slice(0, 10);
  const { start, end } = normalizeDateRange(rawStart, rawEnd);
  const overdue = isPmGanttItemOverdue(item);

  return {
    id: String(item.id),
    itemId: item.id,
    name: item.title,
    start,
    end,
    progress: getPmGanttProgress(item.status),
    custom_class: `gantt-${item.priority.toLowerCase()}`,
    status: item.status,
    statusLabel: PM_STATUS_COLUMNS.find((column) => column.key === item.status)?.label ?? item.status,
    priorityLabel: PM_PRIORITY_MAP[item.priority]?.label ?? item.priority,
    projectName: item.projectName ?? null,
    projectColor: item.projectColor ?? null,
    startLabel: start,
    endLabel: end,
    pinned: item.pinned,
    overdue,
  };
}

export function buildPmGanttTasks(
  items: PmItem[],
): PmGanttTask[] {
  return items
    .filter(hasPmGanttSchedule)
    .map((item) => buildPmGanttTask(item));
}

export function buildPmGanttPopupHtml(
  task: PmGanttTask,
  options: { showProjectMeta?: boolean } = {},
): string {
  const projectRow = options.showProjectMeta && task.projectName
    ? `
      <div class="pm-gantt-popup-project">
        <span class="pm-gantt-popup-project-dot" style="background-color: ${escapeHtml(task.projectColor ?? "#909399")}"></span>
        <span>${escapeHtml(task.projectName)}</span>
      </div>
    `
    : "";
  const pinnedBadge = task.pinned
    ? '<span class="pm-gantt-popup-badge is-muted">已置顶</span>'
    : "";
  const overdueBadge = task.overdue
    ? '<span class="pm-gantt-popup-badge is-danger">已逾期</span>'
    : "";

  return `
    <div class="pm-gantt-popup-card">
      <div class="pm-gantt-popup-title">${escapeHtml(task.name)}</div>
      ${projectRow}
      <div class="pm-gantt-popup-badges">
        <span class="pm-gantt-popup-badge is-status">${escapeHtml(task.statusLabel)}</span>
        <span class="pm-gantt-popup-badge is-priority">${escapeHtml(task.priorityLabel)}</span>
        ${pinnedBadge}
        ${overdueBadge}
      </div>
      <div class="pm-gantt-popup-dates">${escapeHtml(task.startLabel)} ~ ${escapeHtml(task.endLabel)}</div>
    </div>
  `;
}

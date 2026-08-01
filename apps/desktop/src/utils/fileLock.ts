import type { FileLockProcess } from "../types";

export type FileLockSortKey = "pid-asc" | "pid-desc" | "app" | "status";

const APP_TYPE_LABELS: Record<string, string> = {
  "main-window": "主窗口",
  "other-window": "其他窗口",
  service: "服务",
  explorer: "资源管理器",
  console: "控制台",
  critical: "关键进程",
  unknown: "未知",
};

const STATUS_LABELS: Record<string, string> = {
  running: "运行中",
  stopped: "已停止",
  "stopped-other": "其他状态",
  unknown: "未知",
};

export function normalizeFileLockPath(value: string): string {
  const normalized = value.trim().replaceAll("/", "\\");
  if (normalized.length <= 3) return normalized.toLocaleLowerCase();
  return normalized.replace(/[\\]+$/, "").toLocaleLowerCase();
}

export function fileLockPathsMatch(left: string, right: string): boolean {
  const normalizedLeft = normalizeFileLockPath(left);
  const normalizedRight = normalizeFileLockPath(right);
  return normalizedLeft.length > 0 && normalizedLeft === normalizedRight;
}

export function fileLockAppTypeLabel(value: string): string {
  const normalized = value.trim();
  return APP_TYPE_LABELS[normalized] ?? (normalized || "未知");
}

export function fileLockStatusLabel(value: string): string {
  const normalized = value.trim();
  return STATUS_LABELS[normalized] ?? (normalized || "未知");
}

export function fileLockStatusTagType(
  value: string,
): "success" | "warning" | "info" | "danger" | "" {
  if (value === "running") return "success";
  if (value === "stopped") return "info";
  if (value === "stopped-other") return "warning";
  return "";
}

function processSearchText(process: FileLockProcess): string {
  return [
    process.pid,
    process.appName,
    process.appType,
    fileLockAppTypeLabel(process.appType),
    process.status,
    fileLockStatusLabel(process.status),
    process.executablePath ?? "",
  ]
    .join(" ")
    .toLocaleLowerCase();
}

function compareText(left: string, right: string): number {
  return left.localeCompare(right, undefined, { numeric: true, sensitivity: "base" });
}

export function filterAndSortFileLockProcesses(
  processes: FileLockProcess[],
  query: string,
  sortKey: FileLockSortKey,
): FileLockProcess[] {
  const needle = query.trim().toLocaleLowerCase();
  const filtered = needle
    ? processes.filter((process) => processSearchText(process).includes(needle))
    : processes;

  return [...filtered].sort((left, right) => {
    if (sortKey === "pid-asc") return left.pid - right.pid;
    if (sortKey === "pid-desc") return right.pid - left.pid;
    if (sortKey === "status") {
      return (
        compareText(fileLockStatusLabel(left.status), fileLockStatusLabel(right.status)) ||
        left.pid - right.pid
      );
    }
    return compareText(left.appName, right.appName) || left.pid - right.pid;
  });
}

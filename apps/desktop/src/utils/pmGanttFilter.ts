import type { PmItem, PmItemStatus } from "../types/pm";
import { PM_STATUS_COLUMNS } from "../types/pm";

const PM_GANTT_STATUS_ORDER = PM_STATUS_COLUMNS.map((column) => column.key);
const PM_GANTT_STATUS_SET = new Set<PmItemStatus>(PM_GANTT_STATUS_ORDER);

function isPmGanttStatus(value: unknown): value is PmItemStatus {
  return typeof value === "string" && PM_GANTT_STATUS_SET.has(value as PmItemStatus);
}

export function getPmGanttDefaultStatuses(): PmItemStatus[] {
  return PM_GANTT_STATUS_ORDER.filter((s) => s !== "done");
}

export function normalizePmGanttSelectedStatuses(
  input: readonly unknown[] | null | undefined,
): PmItemStatus[] {
  const selected = new Set<PmItemStatus>();

  for (const value of input ?? []) {
    if (isPmGanttStatus(value)) {
      selected.add(value);
    }
  }

  return PM_GANTT_STATUS_ORDER.filter((status) => selected.has(status));
}

export function togglePmGanttStatus(
  selectedStatuses: readonly unknown[] | null | undefined,
  status: PmItemStatus,
): PmItemStatus[] {
  const nextSelected = new Set(normalizePmGanttSelectedStatuses(selectedStatuses));

  if (nextSelected.has(status)) {
    nextSelected.delete(status);
  } else {
    nextSelected.add(status);
  }

  return PM_GANTT_STATUS_ORDER.filter((currentStatus) => nextSelected.has(currentStatus));
}

export function selectAllPmGanttStatuses(): PmItemStatus[] {
  return [...PM_GANTT_STATUS_ORDER];
}

export function clearPmGanttStatuses(): PmItemStatus[] {
  return [];
}

export function coercePmItemStatusForGanttFilter(status: unknown): PmItemStatus {
  return isPmGanttStatus(status) ? status : "todo";
}

export function filterPmItemsByGanttStatuses(
  items: readonly PmItem[],
  selectedStatuses: readonly unknown[] | null | undefined,
): PmItem[] {
  const normalizedStatuses = normalizePmGanttSelectedStatuses(selectedStatuses);
  if (normalizedStatuses.length === 0) {
    return [];
  }

  const selected = new Set(normalizedStatuses);
  return items.filter((item) => selected.has(coercePmItemStatusForGanttFilter(item.status)));
}

import type { PmItem, PmItemStatus } from "../types/pm";
import { PM_STATUS_COLUMNS } from "../types/pm";

const PM_STATUS_ORDER = PM_STATUS_COLUMNS.map((column) => column.key);
const PM_STATUS_SET = new Set<PmItemStatus>(PM_STATUS_ORDER);

type PmStatusColumn = (typeof PM_STATUS_COLUMNS)[number];

function isPmStatus(value: unknown): value is PmItemStatus {
  return typeof value === "string" && PM_STATUS_SET.has(value as PmItemStatus);
}

export function getPmDefaultSelectedStatuses(): PmItemStatus[] {
  return PM_STATUS_ORDER.filter((status) => status !== "done");
}

export function normalizePmSelectedStatuses(
  input: readonly unknown[] | null | undefined,
): PmItemStatus[] {
  const selected = new Set<PmItemStatus>();

  for (const value of input ?? []) {
    if (isPmStatus(value)) {
      selected.add(value);
    }
  }

  return PM_STATUS_ORDER.filter((status) => selected.has(status));
}

export function togglePmSelectedStatus(
  selectedStatuses: readonly unknown[] | null | undefined,
  status: PmItemStatus,
): PmItemStatus[] {
  const nextSelected = new Set(normalizePmSelectedStatuses(selectedStatuses));

  if (nextSelected.has(status)) {
    nextSelected.delete(status);
  } else {
    nextSelected.add(status);
  }

  return PM_STATUS_ORDER.filter((currentStatus) => nextSelected.has(currentStatus));
}

export function selectAllPmStatuses(): PmItemStatus[] {
  return [...PM_STATUS_ORDER];
}

export function clearPmStatuses(): PmItemStatus[] {
  return [];
}

export function coercePmItemStatusForFilter(status: unknown): PmItemStatus {
  return isPmStatus(status) ? status : "todo";
}

export function filterPmItemsBySelectedStatuses(
  items: readonly PmItem[],
  selectedStatuses: readonly unknown[] | null | undefined,
): PmItem[] {
  const normalizedStatuses = normalizePmSelectedStatuses(selectedStatuses);
  if (normalizedStatuses.length === 0) {
    return [];
  }

  const selected = new Set(normalizedStatuses);
  return items.filter((item) => selected.has(coercePmItemStatusForFilter(item.status)));
}

export function getVisiblePmStatusColumns(
  selectedStatuses: readonly unknown[] | null | undefined,
): PmStatusColumn[] {
  const selected = new Set(normalizePmSelectedStatuses(selectedStatuses));
  return PM_STATUS_COLUMNS.filter((column) => selected.has(column.key));
}

export function groupPmItemsByStatus(items: readonly PmItem[]): Map<PmItemStatus, PmItem[]> {
  const map = new Map<PmItemStatus, PmItem[]>();

  for (const column of PM_STATUS_COLUMNS) {
    map.set(column.key, []);
  }

  for (const item of items) {
    map.get(coercePmItemStatusForFilter(item.status))?.push(item);
  }

  return map;
}

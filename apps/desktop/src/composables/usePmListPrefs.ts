import { ref, watch, type Ref } from "vue";
import { getSettingJson, setSettingJson } from "./useSettings";
import type { PmContextId } from "./usePmViewMemory";

export type PmListColId =
  | "title"
  | "project"
  | "itemType"
  | "priority"
  | "status"
  | "endAt"
  | "updatedAt"
  | "tags";

export type PmListGroupBy = "none" | "project" | "status" | "priority" | "tag";

export interface PmListFilters {
  tags: string[];
  dateRange: [string, string] | null;
}

export const ALL_LIST_COLS: PmListColId[] = [
  "title",
  "project",
  "itemType",
  "priority",
  "status",
  "endAt",
  "updatedAt",
  "tags",
];

export const COL_LABELS: Record<PmListColId, string> = {
  title: "标题",
  project: "项目",
  itemType: "类型",
  priority: "优先级",
  status: "状态",
  endAt: "截止",
  updatedAt: "更新",
  tags: "标签",
};

function defaultVisibleCols(): PmListColId[] {
  return ["title", "project", "itemType", "priority", "status", "endAt", "updatedAt"];
}

function defaultFilters(): PmListFilters {
  return {
    tags: [],
    dateRange: null,
  };
}

function contextToken(ctx: PmContextId): string {
  return ctx === "overview" ? "overview" : `project-${ctx}`;
}

function colsKey(ctx: PmContextId): string {
  return `pm:list:${contextToken(ctx)}:cols`;
}

function filtersKey(ctx: PmContextId): string {
  return `pm:list:${contextToken(ctx)}:filters`;
}

function groupByKey(ctx: PmContextId): string {
  return `pm:list:${contextToken(ctx)}:groupBy`;
}

function sanitizeCols(raw: unknown): PmListColId[] {
  if (!Array.isArray(raw)) return defaultVisibleCols();
  const known = new Set<PmListColId>(ALL_LIST_COLS);
  const result: PmListColId[] = [];
  for (const v of raw) {
    if (typeof v === "string" && known.has(v as PmListColId) && !result.includes(v as PmListColId)) {
      result.push(v as PmListColId);
    }
  }
  return result.length > 0 ? result : defaultVisibleCols();
}

function sanitizeFilters(raw: unknown): PmListFilters {
  if (!raw || typeof raw !== "object") return defaultFilters();
  const r = raw as Record<string, unknown>;
  const rangeSource = r.dateRange ?? r.endRange;
  return {
    tags: Array.isArray(r.tags) ? (r.tags.filter((x) => typeof x === "string") as string[]) : [],
    dateRange:
      Array.isArray(rangeSource) && rangeSource.length === 2 && rangeSource.every((x) => typeof x === "string")
        ? ([rangeSource[0], rangeSource[1]] as [string, string])
        : null,
  };
}

function sanitizeGroupBy(raw: unknown): PmListGroupBy {
  const valid: PmListGroupBy[] = ["none", "project", "status", "priority", "tag"];
  if (typeof raw === "string" && (valid as string[]).includes(raw)) return raw as PmListGroupBy;
  return "none";
}

export function usePmListPrefs(contextRef: Ref<PmContextId | null>) {
  const visibleCols = ref<PmListColId[]>(defaultVisibleCols());
  const filters = ref<PmListFilters>(defaultFilters());
  const groupBy = ref<PmListGroupBy>("none");

  watch(
    contextRef,
    (ctx) => {
      if (ctx === null) return;
      visibleCols.value = sanitizeCols(getSettingJson<unknown>(colsKey(ctx), null));
      filters.value = sanitizeFilters(getSettingJson<unknown>(filtersKey(ctx), null));
      groupBy.value = sanitizeGroupBy(getSettingJson<unknown>(groupByKey(ctx), null));
    },
    { immediate: true },
  );

  function setVisibleCols(cols: PmListColId[]) {
    visibleCols.value = cols;
    const ctx = contextRef.value;
    if (ctx === null) return;
    setSettingJson(colsKey(ctx), cols);
  }

  function setFilters(next: PmListFilters) {
    filters.value = next;
    const ctx = contextRef.value;
    if (ctx === null) return;
    setSettingJson(filtersKey(ctx), next);
  }

  function setGroupBy(value: PmListGroupBy) {
    groupBy.value = value;
    const ctx = contextRef.value;
    if (ctx === null) return;
    setSettingJson(groupByKey(ctx), value);
  }

  function resetFilters() {
    setFilters(defaultFilters());
  }

  return {
    visibleCols,
    filters,
    groupBy,
    setVisibleCols,
    setFilters,
    setGroupBy,
    resetFilters,
  };
}

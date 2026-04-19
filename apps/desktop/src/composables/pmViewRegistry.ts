import { defineAsyncComponent, type Component } from "vue";

export type ViewId = "kanban" | "gantt" | "today" | "list" | "calendar" | "matrix";

export interface ViewDefinition {
  id: ViewId;
  label: string;
  icon: string;
  component: Component;
}

export const PM_VIEWS: ViewDefinition[] = [
  {
    id: "kanban",
    label: "看板",
    icon: "▦",
    component: defineAsyncComponent(() => import("../components/PmKanbanView.vue")),
  },
  {
    id: "gantt",
    label: "甘特",
    icon: "▤",
    component: defineAsyncComponent(() => import("../components/PmGanttView.vue")),
  },
  {
    id: "today",
    label: "今日",
    icon: "◷",
    component: defineAsyncComponent(() => import("../components/PmTodayView.vue")),
  },
  {
    id: "list",
    label: "列表",
    icon: "≡",
    component: defineAsyncComponent(() => import("../components/PmListView.vue")),
  },
  {
    id: "calendar",
    label: "日历",
    icon: "▥",
    component: defineAsyncComponent(() => import("../components/PmCalendarView.vue")),
  },
  {
    id: "matrix",
    label: "四象限",
    icon: "⊞",
    component: defineAsyncComponent(() => import("../components/PmMatrixView.vue")),
  },
];

export function getViewById(id: ViewId): ViewDefinition {
  return PM_VIEWS.find((v) => v.id === id) ?? PM_VIEWS[0];
}

export function hasView(id: string): id is ViewId {
  return PM_VIEWS.some((v) => v.id === id);
}

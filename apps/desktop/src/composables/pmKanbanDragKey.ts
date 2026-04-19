import type { InjectionKey, Ref } from "vue";
import type { PmItemStatus } from "../types/pm";

export interface PmKanbanDragState {
  draggingItemId: Ref<number | null>;
  dropTargetProjectId: Ref<number | null>;
  dragConsumed: Ref<boolean>;
  draggingOverColumn: Ref<PmItemStatus | null>;
}

export const PM_KANBAN_DRAG_KEY: InjectionKey<PmKanbanDragState> = Symbol("pm-kanban-drag");

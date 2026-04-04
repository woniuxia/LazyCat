<template>
  <div class="gantt-container">
    <div class="gantt-toolbar">
      <div class="gantt-toolbar-left">
        <el-radio-group v-model="ganttViewMode" size="small" @change="changeViewMode">
          <el-radio-button value="Day">日</el-radio-button>
          <el-radio-button value="Week">周</el-radio-button>
          <el-radio-button value="Month">月</el-radio-button>
        </el-radio-group>
        <div class="gantt-toolbar-filters" aria-label="状态筛选">
          <button
            v-for="column in PM_STATUS_COLUMNS"
            :key="column.key"
            type="button"
            class="gantt-filter-chip"
            :class="{ 'is-selected': selectedStatusSet.has(column.key) }"
            @click="emit('toggle-status', { status: column.key })"
          >
            {{ column.label }}
          </button>
          <button
            type="button"
            class="gantt-filter-chip is-action"
            :class="{ 'is-muted': allStatusesSelected }"
            @click="emit('select-all-statuses')"
          >
            全选
          </button>
          <button
            type="button"
            class="gantt-filter-chip is-action"
            :class="{ 'is-muted': selectedStatusSet.size === 0 }"
            @click="emit('clear-statuses')"
          >
            清空
          </button>
        </div>
      </div>
      <div class="gantt-toolbar-meta">
        <span class="gantt-toolbar-summary">已排期 {{ ganttTasks.length }} 项</span>
        <span v-if="unscheduledCount > 0" class="gantt-toolbar-hint">另有 {{ unscheduledCount }} 项未设置日期</span>
      </div>
    </div>
    <div v-if="ganttTasks.length === 0" class="gantt-empty">
      <el-empty :description="emptyDescription" />
    </div>
    <template v-else>
      <div
        ref="ganttRef"
        class="gantt-wrapper"
        @contextmenu="onGanttContextMenu"
      />
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import Gantt from "frappe-gantt";
import "frappe-gantt/dist/frappe-gantt.css";

import type { PmItem, PmItemStatus } from "../types/pm";
import { PM_STATUS_COLUMNS } from "../types/pm";
import {
  buildPmGanttPopupHtml,
  buildPmGanttTasks,
  clampPmGanttPopupPosition,
  countPmGanttUnscheduledItems,
} from "../utils/pmGantt";
import { normalizePmGanttSelectedStatuses } from "../utils/pmGanttFilter";
import type { PmGanttTask } from "../utils/pmGantt";

const props = withDefaults(defineProps<{
  items: PmItem[];
  selectedStatuses: PmItemStatus[];
  selectedItemId?: number | null;
  showProjectMeta?: boolean;
}>(), {
  selectedStatuses: () => [],
  selectedItemId: null,
  showProjectMeta: false,
});

const emit = defineEmits<{
  (e: "select", item: PmItem): void;
  (e: "edit", item: PmItem): void;
  (e: "item-context", payload: { item: PmItem; anchorX: number; anchorY: number }): void;
  (e: "date-change", item: PmItem, start: string, end: string): void;
  (e: "view-change", mode: string): void;
  (e: "viewport-scroll"): void;
  (e: "toggle-status", payload: { status: PmItemStatus }): void;
  (e: "select-all-statuses"): void;
  (e: "clear-statuses"): void;
}>();

const ganttRef = ref<HTMLElement | null>(null);
const ganttViewMode = ref("Day");
let ganttInstance: Gantt | null = null;
let isDragging = false;
let dragTimer: ReturnType<typeof setTimeout> | null = null;
let skipNextRefresh = false;
let ganttViewportEl: HTMLElement | null = null;
let popupObserver: MutationObserver | null = null;

const GANTT_POPUP_GAP = 10;

type GanttWithInternalOptions = Gantt & {
  options?: {
    scroll_to?: string | null;
  };
};

const ganttTasks = computed(() => buildPmGanttTasks(props.items));
const unscheduledCount = computed(() => countPmGanttUnscheduledItems(props.items));
const normalizedSelectedStatuses = computed(() => normalizePmGanttSelectedStatuses(props.selectedStatuses));
const selectedStatusSet = computed(() => new Set(normalizedSelectedStatuses.value));
const allStatusesSelected = computed(() => normalizedSelectedStatuses.value.length === PM_STATUS_COLUMNS.length);
const emptyDescription = computed(() => {
  if (props.items.length === 0) {
    return "当前筛选结果没有可显示的工作项";
  }
  if (unscheduledCount.value > 0) {
    return `当前筛选结果中有 ${unscheduledCount.value} 项未设置日期，无法显示甘特图`;
  }
  return "当前筛选结果没有可显示的工作项";
});

function syncGanttBarStateClasses() {
  if (!ganttRef.value) return;
  const taskMap = new Map(ganttTasks.value.map((task) => [String(task.itemId), task]));
  const barWrappers = ganttRef.value.querySelectorAll<SVGGElement>(".bar-wrapper");

  for (const barWrapper of barWrappers) {
    barWrapper.classList.remove("gantt-selected", "gantt-pinned", "gantt-overdue");
    const taskId = barWrapper.getAttribute("data-id");
    if (!taskId) continue;

    const task = taskMap.get(taskId);
    if (!task) continue;

    if (props.selectedItemId === task.itemId) {
      barWrapper.classList.add("gantt-selected");
    }
    if (task.pinned) {
      barWrapper.classList.add("gantt-pinned");
    }
    if (task.overdue) {
      barWrapper.classList.add("gantt-overdue");
    }
  }
}

function formatDate(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function resolveItem(taskId: string): PmItem | undefined {
  return props.items.find((item) => String(item.id) === taskId);
}

function getGanttViewport(): HTMLElement | null {
  return ganttRef.value?.querySelector<HTMLElement>(".gantt-container") ?? null;
}

function getGanttPopupWrapper(): HTMLElement | null {
  return getGanttViewport()?.querySelector<HTMLElement>(".popup-wrapper") ?? null;
}

function unbindGanttViewportScroll() {
  if (!ganttViewportEl) return;
  ganttViewportEl.removeEventListener("scroll", onViewportScroll);
  ganttViewportEl = null;
}

function bindGanttViewportScroll() {
  unbindGanttViewportScroll();
  ganttViewportEl = getGanttViewport();
  ganttViewportEl?.addEventListener("scroll", onViewportScroll, { passive: true });
}

function disconnectPopupObserver() {
  popupObserver?.disconnect();
  popupObserver = null;
}

function repositionPopupWithinViewport() {
  const viewport = getGanttViewport();
  const popup = getGanttPopupWrapper();
  if (!viewport || !popup || popup.classList.contains("hide")) return;

  const popupWidth = popup.offsetWidth;
  const popupHeight = popup.offsetHeight;
  if (popupWidth <= 0 || popupHeight <= 0) return;

  const currentLeft = Number.parseFloat(popup.style.left || "0");
  const currentTop = Number.parseFloat(popup.style.top || "0");
  const position = clampPmGanttPopupPosition({
    anchorX: currentLeft - GANTT_POPUP_GAP,
    anchorY: currentTop + GANTT_POPUP_GAP,
    popupWidth,
    popupHeight,
    viewportWidth: viewport.clientWidth,
    viewportHeight: viewport.clientHeight,
    scrollLeft: viewport.scrollLeft,
    scrollTop: viewport.scrollTop,
    gap: GANTT_POPUP_GAP,
  });

  const nextLeft = `${position.left}px`;
  const nextTop = `${position.top}px`;
  if (popup.style.left !== nextLeft) {
    popup.style.left = nextLeft;
  }
  if (popup.style.top !== nextTop) {
    popup.style.top = nextTop;
  }
}

function observePopupPosition() {
  disconnectPopupObserver();
  const popup = getGanttPopupWrapper();
  if (!popup) return;

  popupObserver = new MutationObserver(() => {
    requestAnimationFrame(repositionPopupWithinViewport);
  });
  popupObserver.observe(popup, {
    attributes: true,
    attributeFilter: ["class", "style"],
    childList: true,
    subtree: true,
  });
}

function cleanupGanttDomBindings() {
  disconnectPopupObserver();
  unbindGanttViewportScroll();
}

function clearGantt() {
  cleanupGanttDomBindings();
  ganttInstance?.hide_popup();
  ganttInstance = null;
  if (ganttRef.value) {
    ganttRef.value.innerHTML = "";
  }
}

function renderGantt() {
  if (!ganttRef.value || ganttTasks.value.length === 0) {
    clearGantt();
    return;
  }

  ganttRef.value.innerHTML = "";
  ganttInstance = new Gantt(ganttRef.value, ganttTasks.value, {
    view_mode: ganttViewMode.value,
    date_format: "YYYY-MM-DD",
    language: "zh",
    popup_on: "hover",
    popup: (context) => buildPmGanttPopupHtml(context.task as PmGanttTask, {
      showProjectMeta: props.showProjectMeta,
    }),
    on_click: (task) => {
      if (isDragging) return;
      const found = resolveItem(task.id);
      if (found) emit("select", found);
    },
    on_double_click: (task) => {
      if (isDragging) return;
      const found = resolveItem(task.id);
      if (found) emit("edit", found);
    },
    on_date_change: (task, start, end) => {
      isDragging = true;
      if (dragTimer) clearTimeout(dragTimer);
      dragTimer = setTimeout(() => {
        isDragging = false;
        const found = resolveItem(task.id);
        if (found) {
          skipNextRefresh = true;
          emit("date-change", found, formatDate(start), formatDate(end));
        }
      }, 300);
    },
  });
  bindGanttViewportScroll();
  observePopupPosition();
  syncGanttBarStateClasses();
}

function changeViewMode(mode: string) {
  ganttInstance?.hide_popup();
  emit("view-change", mode);
  ganttInstance?.change_view_mode(mode, true);
  nextTick(syncGanttBarStateClasses);
}

function onViewportScroll() {
  ganttInstance?.hide_popup();
  emit("viewport-scroll");
}

function onGanttContextMenu(event: MouseEvent) {
  const target = event.target;
  if (!(target instanceof Element)) return;
  const barWrapper = target.closest(".bar-wrapper");
  if (!barWrapper || !ganttRef.value?.contains(barWrapper)) return;

  const taskId = barWrapper.getAttribute("data-id");
  if (!taskId) return;

  const found = resolveItem(taskId);
  if (!found) return;

  event.preventDefault();
  event.stopPropagation();
  ganttInstance?.hide_popup();
  emit("item-context", {
    item: found,
    anchorX: event.clientX,
    anchorY: event.clientY,
  });
}

watch(
  () => props.selectedItemId,
  () => {
    nextTick(syncGanttBarStateClasses);
  },
);

watch(
  () => ganttTasks.value,
  () => {
    if (isDragging) return;
    if (skipNextRefresh) {
      skipNextRefresh = false;
      nextTick(syncGanttBarStateClasses);
      return;
    }

    nextTick(() => {
      if (ganttTasks.value.length === 0) {
        clearGantt();
        return;
      }

      if (ganttInstance) {
        const viewport = getGanttViewport();
        const scrollLeft = viewport?.scrollLeft ?? 0;
        const scrollTop = viewport?.scrollTop ?? 0;
        const internalGantt = ganttInstance as GanttWithInternalOptions;
        const previousScrollTo = internalGantt.options?.scroll_to ?? null;
        if (internalGantt.options) {
          // frappe-gantt refresh() 默认会按 scroll_to 重新定位，这里临时关闭以保留当前视口。
          internalGantt.options.scroll_to = null;
        }
        ganttInstance.refresh(ganttTasks.value);
        if (internalGantt.options) {
          internalGantt.options.scroll_to = previousScrollTo;
        }
        syncGanttBarStateClasses();
        requestAnimationFrame(() => {
          const restoredViewport = getGanttViewport();
          if (!restoredViewport) return;
          restoredViewport.scrollLeft = scrollLeft;
          restoredViewport.scrollTop = scrollTop;
        });
      } else {
        renderGantt();
      }
    });
  },
  { deep: true },
);

onMounted(() => {
  nextTick(renderGantt);
});

onBeforeUnmount(() => {
  if (dragTimer) clearTimeout(dragTimer);
  clearGantt();
});
</script>

<style scoped>
.gantt-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.gantt-toolbar {
  padding: 8px 12px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  flex-shrink: 0;
  border-bottom: 1px solid var(--el-border-color-extra-light);
  background: linear-gradient(180deg, var(--el-bg-color), var(--el-fill-color-extra-light));
}

.gantt-toolbar-left {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex-wrap: wrap;
}

.gantt-toolbar-filters {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.gantt-filter-chip {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-height: 30px;
  padding: 0 12px;
  border: 1px solid var(--el-border-color);
  border-radius: 999px;
  background: var(--el-bg-color);
  color: var(--el-text-color-regular);
  font: inherit;
  font-size: 12px;
  line-height: 1;
  white-space: nowrap;
  cursor: pointer;
  transition:
    border-color 0.2s ease,
    background-color 0.2s ease,
    color 0.2s ease,
    opacity 0.2s ease;
}

.gantt-filter-chip:hover {
  border-color: var(--el-color-primary-light-5);
  color: var(--el-color-primary);
}

.gantt-filter-chip.is-selected {
  border-color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary-dark-2);
}

.gantt-filter-chip.is-action {
  background: var(--el-fill-color-extra-light);
}

.gantt-filter-chip.is-muted {
  opacity: 0.7;
}

.gantt-toolbar-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex-wrap: wrap;
  color: var(--el-text-color-secondary);
  font-size: 12px;
}

.gantt-toolbar-summary {
  color: var(--el-text-color-primary);
  font-weight: 600;
}

.gantt-toolbar-hint {
  white-space: nowrap;
}

.gantt-wrapper {
  flex: 1;
  overflow: auto;
  min-width: 600px;
}

.gantt-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}
</style>

<style>
.gantt .bar-wrapper {
  cursor: pointer;
}

.gantt .bar-wrapper:hover .bar {
  filter: brightness(1.03);
}

.gantt .bar-wrapper:hover .bar-progress {
  filter: brightness(1.03);
}

.gantt .bar {
  fill-opacity: 1;
}

.gantt .bar-progress {
  fill-opacity: 1;
}

.gantt-p0 .bar,
.gantt-p0 .bar-progress {
  fill: #f56c6c !important;
}

.gantt-p1 .bar,
.gantt-p1 .bar-progress {
  fill: #e6a23c !important;
}

.gantt-p2 .bar,
.gantt-p2 .bar-progress {
  fill: #409eff !important;
}

.gantt-p3 .bar,
.gantt-p3 .bar-progress {
  fill: #909399 !important;
}

.gantt-selected .bar {
  stroke: #1d4ed8;
  stroke-width: 2;
  filter: drop-shadow(0 0 0.35rem rgba(64, 158, 255, 0.35));
}

.gantt-selected .bar-progress {
  stroke: #1d4ed8;
  stroke-width: 1.5;
}

.gantt-pinned .bar {
  stroke-dasharray: 5 2;
}

.gantt-overdue .bar {
  fill-opacity: 0.86;
}

.gantt .bar-label {
  font-size: 12px;
  font-weight: 500;
}

.gantt .grid-header {
  fill: var(--el-fill-color-lighter);
}

.gantt .popup-wrapper {
  z-index: 5;
  box-sizing: border-box;
  max-width: min(320px, calc(100% - 24px));
}

.pm-gantt-popup-card {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 220px;
}

.pm-gantt-popup-title {
  color: var(--el-text-color-primary);
  font-size: 13px;
  font-weight: 600;
  line-height: 1.4;
  word-break: break-word;
}

.pm-gantt-popup-project {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--el-text-color-secondary);
  font-size: 12px;
}

.pm-gantt-popup-project-dot {
  width: 8px;
  height: 8px;
  border-radius: 999px;
  flex-shrink: 0;
}

.pm-gantt-popup-badges {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.pm-gantt-popup-badge {
  display: inline-flex;
  align-items: center;
  border-radius: 999px;
  padding: 2px 8px;
  font-size: 11px;
  line-height: 1.4;
  white-space: nowrap;
}

.pm-gantt-popup-badge.is-status {
  background: rgba(64, 158, 255, 0.12);
  color: #2563eb;
}

.pm-gantt-popup-badge.is-priority {
  background: rgba(230, 162, 60, 0.15);
  color: #b45309;
}

.pm-gantt-popup-badge.is-muted {
  background: rgba(144, 147, 153, 0.14);
  color: var(--el-text-color-secondary);
}

.pm-gantt-popup-badge.is-danger {
  background: rgba(245, 108, 108, 0.14);
  color: #dc2626;
}

.pm-gantt-popup-dates {
  color: var(--el-text-color-secondary);
  font-size: 12px;
}
</style>

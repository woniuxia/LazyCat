<template>
  <div class="gantt-container">
    <div class="gantt-toolbar">
      <div class="gantt-toolbar-left">
        <el-radio-group v-model="ganttViewMode" size="small" @change="changeViewMode">
          <el-radio-button value="Day">日</el-radio-button>
          <el-radio-button value="Week">周</el-radio-button>
          <el-radio-button value="Month">月</el-radio-button>
        </el-radio-group>
      </div>
      <div class="gantt-toolbar-meta">
        <span class="gantt-toolbar-summary">已排期 {{ ganttTasks.length }} 项</span>
        <span v-if="unscheduledCount > 0" class="gantt-toolbar-hint"
          >另有 {{ unscheduledCount }} 项未设置日期</span
        >
      </div>
    </div>
    <div v-if="ganttTasks.length === 0" class="gantt-empty">
      <el-empty :description="emptyDescription" />
    </div>
    <template v-else>
      <div ref="ganttRef" class="gantt-wrapper" @contextmenu="onGanttContextMenu" />
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import Gantt from "frappe-gantt";
import "frappe-gantt/dist/frappe-gantt.css";

import type { PmItem } from "../../types/pm";
import {
  buildPmGanttPopupHtml,
  buildPmGanttTasks,
  clampPmGanttPopupPosition,
  computePmGanttInitialScrollLeft,
  countPmGanttUnscheduledItems,
  shouldHighlightPmGanttWeekendLabel,
} from "../../utils/pmGantt";
import type { PmGanttTask } from "../../utils/pmGantt";

const props = withDefaults(
  defineProps<{
    items: PmItem[];
    selectedItemId?: number | null;
    showProjectMeta?: boolean;
  }>(),
  {
    selectedItemId: null,
    showProjectMeta: false,
  },
);

const emit = defineEmits<{
  (e: "select", item: PmItem): void;
  (e: "edit", item: PmItem): void;
  (e: "item-context", payload: { item: PmItem; anchorX: number; anchorY: number }): void;
  (e: "date-change", item: PmItem, start: string, end: string): void;
  (e: "view-change", mode: string): void;
  (e: "viewport-scroll"): void;
}>();

const ganttRef = ref<HTMLElement | null>(null);
const ganttViewMode = ref("Day");
let ganttInstance: Gantt | null = null;
let isDragging = false;
let dragTimer: ReturnType<typeof setTimeout> | null = null;
let clickTimer: ReturnType<typeof setTimeout> | null = null;
let skipNextRefresh = false;
let ganttViewportEl: HTMLElement | null = null;
let popupObserver: MutationObserver | null = null;
let ganttContentObserver: MutationObserver | null = null;
let didApplyInitialScroll = false;
let initialScrollFrame: number | null = null;
let ganttDecorationsFrame: number | null = null;

const GANTT_POPUP_GAP = 10;

type GanttWithInternalOptions = Gantt & {
  options?: {
    infinite_padding?: boolean;
    scroll_to?: string | null;
  };
  scroll_current?: () => void;
};

const ganttTasks = computed(() => buildPmGanttTasks(props.items));
const unscheduledCount = computed(() => countPmGanttUnscheduledItems(props.items));
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

function syncGanttWeekendDateClasses() {
  if (!ganttRef.value) return;

  const lowerTexts = ganttRef.value.querySelectorAll<HTMLElement>(".lower-text");
  for (const lowerText of lowerTexts) {
    lowerText.classList.toggle(
      "pm-gantt-weekend-date",
      shouldHighlightPmGanttWeekendLabel(ganttViewMode.value, lowerText.className),
    );
  }
}

function syncGanttDecorations() {
  stripGanttBarAnimations();
  syncGanttBarStateClasses();
  syncGanttWeekendDateClasses();
  syncTodayButtonBehavior();
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

function getCurrentHighlight(): HTMLElement | null {
  return getGanttViewport()?.querySelector<HTMLElement>(".current-highlight") ?? null;
}

function stripGanttBarAnimations() {
  if (!ganttRef.value) return;

  // frappe-gantt 会给 bar / progress rect 注入 <animate>，导致任务条从左往右“长出来”。
  // PM 视图这里统一移除这些节点，保留静态最终宽度。
  const animations = ganttRef.value.querySelectorAll(".bar-group animate");
  for (const animation of animations) {
    animation.remove();
  }
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

function disconnectGanttContentObserver() {
  ganttContentObserver?.disconnect();
  ganttContentObserver = null;
}

function cancelInitialScrollFrame() {
  if (initialScrollFrame === null) return;
  cancelAnimationFrame(initialScrollFrame);
  initialScrollFrame = null;
}

function cancelGanttDecorationsFrame() {
  if (ganttDecorationsFrame === null) return;
  cancelAnimationFrame(ganttDecorationsFrame);
  ganttDecorationsFrame = null;
}

function resetInitialScrollState() {
  didApplyInitialScroll = false;
  cancelInitialScrollFrame();
  cancelGanttDecorationsFrame();
}

function markInitialScrollHandled() {
  didApplyInitialScroll = true;
  cancelInitialScrollFrame();
}

function scheduleInitialScrollRetry(attempt: number) {
  cancelInitialScrollFrame();
  initialScrollFrame = requestAnimationFrame(() => {
    initialScrollFrame = null;
    applyInitialScroll(attempt);
  });
}

function applyInitialScroll(attempt = 0) {
  if (didApplyInitialScroll) return;

  const viewport = getGanttViewport();
  if (!viewport || viewport.clientWidth <= 0 || viewport.scrollWidth <= 0) {
    if (attempt === 0) {
      scheduleInitialScrollRetry(1);
      return;
    }
    markInitialScrollHandled();
    return;
  }

  if (viewport.scrollWidth <= viewport.clientWidth) {
    viewport.scrollLeft = 0;
    markInitialScrollHandled();
    return;
  }

  const highlight = getCurrentHighlight();
  if (!highlight) {
    if (attempt === 0) {
      scheduleInitialScrollRetry(1);
      return;
    }
    markInitialScrollHandled();
    return;
  }

  const viewportRect = viewport.getBoundingClientRect();
  const highlightRect = highlight.getBoundingClientRect();
  const currentX = highlightRect.left - viewportRect.left + viewport.scrollLeft;
  const targetScrollLeft = computePmGanttInitialScrollLeft({
    currentX,
    viewportWidth: viewport.clientWidth,
    scrollWidth: viewport.scrollWidth,
  });

  viewport.scrollLeft = targetScrollLeft;
  markInitialScrollHandled();
}

function scrollTodayToPreferredOffset(): boolean {
  const viewport = getGanttViewport();
  const highlight = getCurrentHighlight();
  if (
    !viewport ||
    !highlight ||
    viewport.clientWidth <= 0 ||
    viewport.scrollWidth <= viewport.clientWidth
  ) {
    return false;
  }

  const viewportRect = viewport.getBoundingClientRect();
  const highlightRect = highlight.getBoundingClientRect();
  const currentX = highlightRect.left - viewportRect.left + viewport.scrollLeft;
  const targetScrollLeft = computePmGanttInitialScrollLeft({
    currentX,
    viewportWidth: viewport.clientWidth,
    scrollWidth: viewport.scrollWidth,
  });

  viewport.scrollLeft = targetScrollLeft;
  return true;
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

function syncTodayButtonBehavior() {
  if (!ganttRef.value) return;

  const todayButton = ganttRef.value.querySelector<HTMLButtonElement>(".today-button");
  if (!todayButton) return;

  todayButton.onclick = (event) => {
    event.preventDefault();

    if (scrollTodayToPreferredOffset()) {
      return;
    }

    (ganttInstance as GanttWithInternalOptions | null)?.scroll_current?.();
  };
}

function scheduleGanttDecorationsSync() {
  if (ganttDecorationsFrame !== null) {
    return;
  }
  ganttDecorationsFrame = requestAnimationFrame(() => {
    ganttDecorationsFrame = null;
    syncGanttDecorations();
  });
}

function observeGanttContent() {
  disconnectGanttContentObserver();
  const viewport = getGanttViewport();
  if (!viewport) return;

  ganttContentObserver = new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      if (mutation.type !== "childList") continue;
      if (mutation.addedNodes.length === 0 && mutation.removedNodes.length === 0) continue;

      // frappe-gantt 在 infinite padding 的 mousewheel 链路里会直接 render() 整个 header/grid。
      // 这里监听 DOM 重建并把 PM 自己补的周末圆底/条目样式重新同步回来。
      scheduleGanttDecorationsSync();
      return;
    }
  });
  ganttContentObserver.observe(viewport, {
    childList: true,
    subtree: true,
  });
}

function cleanupGanttDomBindings() {
  disconnectPopupObserver();
  disconnectGanttContentObserver();
  unbindGanttViewportScroll();
  cancelGanttDecorationsFrame();
}

function clearGantt() {
  resetInitialScrollState();
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

  clearGantt();
  ganttInstance = new Gantt(ganttRef.value, ganttTasks.value, {
    view_mode: ganttViewMode.value,
    date_format: "YYYY-MM-DD",
    infinite_padding: true,
    language: "zh",
    popup_on: "hover",
    scroll_to: "start",
    popup: (context) =>
      buildPmGanttPopupHtml(context.task as PmGanttTask, {
        showProjectMeta: props.showProjectMeta,
      }),
    on_click: (task) => {
      if (isDragging) return;
      if (clickTimer) return;
      clickTimer = setTimeout(() => {
        clickTimer = null;
        const found = resolveItem(task.id);
        if (found) emit("select", found);
      }, 320);
    },
    on_double_click: (task) => {
      if (isDragging) return;
      if (clickTimer) {
        clearTimeout(clickTimer);
        clickTimer = null;
      }
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
  applyInitialScroll();
  bindGanttViewportScroll();
  observePopupPosition();
  observeGanttContent();
  syncGanttDecorations();
}

function changeViewMode(mode: string) {
  ganttInstance?.hide_popup();
  emit("view-change", mode);
  // 传 false 让 frappe-gantt 按新视图模式的 padding 重新扩展 gantt_start/end，
  // 避免保留旧 scrollLeft 在新 column_width/step 下产生位置漂移，
  // 同时保证 Month/Week 视图下 dates.length * column_width 足够填满容器。
  ganttInstance?.change_view_mode(mode, false);
  resetInitialScrollState();
  nextTick(() => {
    applyInitialScroll();
    syncGanttDecorations();
  });
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
        syncGanttDecorations();
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
  if (clickTimer) clearTimeout(clickTimer);
  clearGantt();
});

// 失败回滚场景：父组件在乐观更新失败后调用，强制按当前 props.items 重绘
function forceRefresh() {
  skipNextRefresh = false;
  if (dragTimer) {
    clearTimeout(dragTimer);
    dragTimer = null;
  }
  isDragging = false;
  nextTick(() => {
    if (ganttInstance && ganttTasks.value.length > 0) {
      ganttInstance.refresh(ganttTasks.value);
      syncGanttDecorations();
    } else {
      renderGantt();
    }
  });
}

defineExpose({ forceRefresh });
</script>

<style scoped>
.gantt-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--lc-surface-1);
  border-radius: var(--lc-radius-lg);
}

.gantt-toolbar {
  min-height: 50px;
  padding: 14px 16px;
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

/*
 * frappe-gantt 会把 $svg 的 width 固定为 "100%"（容器宽度），但 grid-background
 * 和 grid-row 的 rect width 使用 dates.length * column_width（内容宽度）。当内容
 * 宽度小于容器时，右侧会露出未填充的空白。此处让两个背景 rect 充满 SVG，兜底
 * 月视图等低密度场景的视觉一致性。
 */
.gantt .grid-background,
.gantt .grid-row {
  width: 100%;
}

.gantt-wrapper {
  --pm-gantt-today-accent: #2563eb;
  --pm-gantt-today-fill: rgba(37, 99, 235, 0.14);
}

.gantt-container .current-highlight,
.gantt-container .current-ball-highlight {
  background: var(--pm-gantt-today-accent);
}

.gantt-container .lower-text.current-date-highlight {
  background: var(--pm-gantt-today-fill);
  color: var(--pm-gantt-today-accent);
  border-radius: 999px;
  font-weight: 700;
}

.gantt-container .lower-text.current-date-highlight.pm-gantt-weekend-date {
  background: transparent;
  color: #f43f5e;
}

.gantt-container .lower-text.pm-gantt-weekend-date {
  color: #f43f5e;
  font-weight: 700;
  isolation: isolate;
}

.gantt-container .lower-text.pm-gantt-weekend-date::before {
  content: "";
  position: absolute;
  left: 50%;
  top: 50%;
  width: 24px;
  height: 24px;
  transform: translate(-50%, -50%);
  border-radius: 999px;
  background: #fff1f2;
  z-index: -1;
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

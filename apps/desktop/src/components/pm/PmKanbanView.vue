<template>
  <div v-if="items.length > 0" class="kanban-board">
    <div
      v-for="col in visibleStatusColumns"
      :key="col.key"
      class="kanban-column"
      :class="{ 'is-drag-over': dragState?.draggingOverColumn.value === col.key }"
    >
      <div class="column-header" :style="{ borderBottomColor: col.color }">
        <span class="column-title">{{ col.label }}</span>
        <span class="column-count" :style="{ background: col.color + '1a', color: col.color }">{{ columnItems(col.key).length }}</span>
      </div>
      <div
        :ref="(el) => setColumnRef(col.key, el)"
        class="column-body"
        :data-status="col.key"
      >
        <div
          v-for="item in columnItems(col.key)"
          :key="item.id"
          class="kanban-card"
          :class="{
            'is-selected': selectedItemId === item.id,
            'is-pinned': item.pinned,
            'is-overdue': isOverdue(item),
            ['is-' + item.priority.toLowerCase()]: true,
          }"
          :style="{ borderLeftColor: PM_PRIORITY_MAP[item.priority]?.color }"
          :data-id="item.id"
          @click="onCardClick($event, item)"
          @dblclick="onCardDblclick(item)"
          @contextmenu.prevent="(e: MouseEvent) => emit('item-context', e, item)"
        >
          <div class="card-topbar" :class="{ 'is-overview': isOverview }">
            <div class="card-topbar-left">
              <template v-if="isOverview && item.projectName">
                <span class="card-project-badge" :style="{ backgroundColor: (item.projectColor || '#0ea5e9') + '18', color: item.projectColor || '#0ea5e9' }">
                  <span class="card-project-dot" :style="{ backgroundColor: item.projectColor || '#0ea5e9' }" />
                  <span class="card-project-name">{{ item.projectName }}</span>
                </span>
              </template>
              <template v-else>
                <span class="card-meta-pill" :style="{ color: PM_ITEM_TYPE_MAP[item.itemType]?.color, borderColor: PM_ITEM_TYPE_MAP[item.itemType]?.color + '40' }">
                  {{ PM_ITEM_TYPE_MAP[item.itemType]?.label }}
                </span>
                <span class="card-meta-pill" :style="{ color: PM_PRIORITY_MAP[item.priority]?.color, borderColor: PM_PRIORITY_MAP[item.priority]?.color + '40' }">
                  {{ PM_PRIORITY_MAP[item.priority]?.label }}
                </span>
              </template>
            </div>
            <div class="card-topbar-right">
              <el-icon v-if="item.pinned" class="badge-pin" title="已置顶"><Top /></el-icon>
              <el-icon v-if="isOverdue(item)" class="badge-overdue" title="已逾期"><AlarmClock /></el-icon>
              <span
                v-if="hasPmDateSchedule(item.startAt, item.endAt)"
                class="card-date-chip"
                :class="{ 'is-overdue-date': isOverdue(item) }"
              >
                {{ formatPmDateRangeForDisplay(item.startAt, item.endAt, { mode: 'short', emptyText: '' }) }}
              </span>
            </div>
          </div>
          <div class="card-title-row">
            <span class="card-title">{{ item.title }}</span>
            <el-tooltip v-if="item.status !== 'done'" :content="'推进到「' + nextStatusLabel(item) + '」'" placement="top">
              <button
                class="card-advance-btn"
                @click.stop="emit('quick-advance', item)"
              >
                <el-icon :size="12"><CaretRight /></el-icon>
              </button>
            </el-tooltip>
          </div>
          <div v-if="isOverview" class="card-meta">
            <span class="card-meta-pill" :style="{ color: PM_ITEM_TYPE_MAP[item.itemType]?.color, borderColor: PM_ITEM_TYPE_MAP[item.itemType]?.color + '40' }">
              {{ PM_ITEM_TYPE_MAP[item.itemType]?.label }}
            </span>
            <span class="card-meta-pill" :style="{ color: PM_PRIORITY_MAP[item.priority]?.color, borderColor: PM_PRIORITY_MAP[item.priority]?.color + '40' }">
              {{ PM_PRIORITY_MAP[item.priority]?.label }}
            </span>
          </div>
          <div v-if="item.tags.length > 0" class="card-tags">
            <el-tag v-for="tag in getItemTagSummary(item).visibleTags" :key="tag" size="small" type="info">{{ tag }}</el-tag>
            <el-tag v-if="getItemTagSummary(item).hiddenCount > 0" size="small" type="info">+{{ getItemTagSummary(item).hiddenCount }}</el-tag>
          </div>
        </div>
        <div v-if="columnItems(col.key).length === 0 && dragState?.draggingItemId.value" class="column-drop-hint">
          拖放到此列
        </div>
        <div v-if="columnItems(col.key).length === 0 && !dragState?.draggingItemId.value" class="column-empty-state">
          <span class="column-empty-text">{{ col.key === 'todo' ? '暂无待办事项' : col.key === 'done' ? '还没有完成的工作项' : '暂无工作项' }}</span>
          <el-button
            v-if="col.key === 'todo'"
            size="small"
            type="primary"
            link
            :disabled="!canCreateItems"
            :title="createBlockedReason || undefined"
            @click="emit('create-item')"
          >
            新建工作项
          </el-button>
        </div>
      </div>
    </div>
  </div>
  <div v-else class="pm-empty">
    <el-empty description="当前筛选结果没有可显示的工作项" />
  </div>
</template>

<script setup lang="ts">
import { computed, inject, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { Top, CaretRight, AlarmClock } from "@element-plus/icons-vue";
import Sortable from "sortablejs";
import type { PmItem, PmItemStatus } from "../../types/pm";
import { PM_STATUS_COLUMNS, PM_ITEM_TYPE_MAP, PM_PRIORITY_MAP } from "../../types/pm";
import {
  formatPmDateRangeForDisplay,
  hasPmDateSchedule,
  isPmItemOverdue,
} from "../../utils/pmDate";
import { summarizePmItemTags } from "../../utils/pmVisual";
import { getVisiblePmStatusColumns } from "../../utils/pmStatusFilter";
import { useToolInvoke } from "../../composables/useToolInvoke";
import { PM_KANBAN_DRAG_KEY } from "../../composables/pmKanbanDragKey";

const props = defineProps<{
  items: PmItem[];
  selectedItemId: number | null;
  selectedStatuses: PmItemStatus[];
  isOverview: boolean;
  canCreateItems: boolean;
  createBlockedReason: string;
  enabled: boolean;
}>();

const emit = defineEmits<{
  (e: "select", item: PmItem): void;
  (e: "edit", item: PmItem): void;
  (e: "item-context", event: MouseEvent, item: PmItem): void;
  (e: "quick-advance", item: PmItem): void;
  (e: "create-item"): void;
  (e: "items-changed"): void;
}>();

const { invoke } = useToolInvoke();

const dragState = inject(PM_KANBAN_DRAG_KEY, null);

const visibleStatusColumns = computed(() => getVisiblePmStatusColumns(props.selectedStatuses));
const visibleStatusColumnKey = computed(() => visibleStatusColumns.value.map((column) => column.key).join("|"));

const columnItemsMap = computed(() => {
  const map = new Map<PmItemStatus, PmItem[]>();
  for (const item of props.items) {
    const list = map.get(item.status) ?? [];
    list.push(item);
    map.set(item.status, list);
  }
  return map;
});

function columnItems(status: PmItemStatus): PmItem[] {
  return columnItemsMap.value.get(status) ?? [];
}

function isOverdue(item: PmItem): boolean {
  return isPmItemOverdue(item);
}

function getItemTagSummary(item: PmItem) {
  return summarizePmItemTags(item.tags);
}

function nextStatusLabel(item: PmItem): string {
  const idx = PM_STATUS_COLUMNS.findIndex((c) => c.key === item.status);
  return idx >= 0 && idx < PM_STATUS_COLUMNS.length - 1 ? PM_STATUS_COLUMNS[idx + 1].label : "";
}

const clickTimer = ref<ReturnType<typeof setTimeout> | null>(null);
const PM_CARD_SINGLE_CLICK_DELAY_MS = 320;

function onCardClick(event: MouseEvent, item: PmItem) {
  if (event.detail > 1 || clickTimer.value) return;
  clickTimer.value = setTimeout(() => {
    clickTimer.value = null;
    emit("select", item);
  }, PM_CARD_SINGLE_CLICK_DELAY_MS);
}

function onCardDblclick(item: PmItem) {
  if (clickTimer.value) {
    clearTimeout(clickTimer.value);
    clickTimer.value = null;
  }
  emit("edit", item);
}

// ── Sortable drag & drop ────────────────────────────────
const sortableInstances = new Map<string, Sortable>();
const columnRefs = new Map<string, HTMLElement>();

function setColumnRef(status: string, el: unknown) {
  if (el instanceof HTMLElement) {
    columnRefs.set(status, el);
    return;
  }
  columnRefs.delete(status);
}

function initSortable() {
  destroySortable();
  if (!props.enabled) return;
  for (const col of visibleStatusColumns.value) {
    const el = columnRefs.get(col.key);
    if (!el) continue;
    const instance = Sortable.create(el, {
      group: "kanban",
      animation: 150,
      forceFallback: true,
      ghostClass: "kanban-ghost",
      dragClass: "kanban-drag",
      fallbackClass: "kanban-fallback",
      onStart: (evt) => {
        if (dragState) {
          dragState.draggingItemId.value = parseInt(evt.item.dataset.id ?? "0", 10);
        }
        document.body.classList.add("pm-is-dragging");
      },
      onMove: (evt) => {
        if (dragState) {
          dragState.draggingOverColumn.value = ((evt.to as HTMLElement).dataset.status as PmItemStatus) || null;
        }
      },
      onEnd: async (evt) => {
        if (dragState) {
          dragState.draggingItemId.value = null;
          dragState.draggingOverColumn.value = null;
          dragState.dropTargetProjectId.value = null;
        }
        document.body.classList.remove("pm-is-dragging");

        if (dragState?.dragConsumed.value) {
          dragState.dragConsumed.value = false;
          return;
        }

        const itemId = parseInt(evt.item.dataset.id ?? "0", 10);
        const newStatus = (evt.to as HTMLElement).dataset.status as PmItemStatus;
        if (!itemId || !newStatus) return;

        const fromEl = evt.from as HTMLElement;
        const toEl = evt.to as HTMLElement;
        const oldStatus = fromEl.dataset.status;
        const oldIndex = evt.oldIndex ?? 0;

        try {
          const statusChanged = oldStatus !== newStatus;
          const children = Array.from(toEl.children) as HTMLElement[];
          const reorderItems = children
            .filter((c) => c.dataset.id)
            .map((child, idx) => {
              const payload = {
                id: parseInt(child.dataset.id ?? "0", 10),
                sortOrder: idx,
              };
              return statusChanged ? { ...payload, status: newStatus } : payload;
            });

          await invoke("tool:pm:item-reorder", { items: reorderItems });
          emit("items-changed");
          if (oldStatus && oldStatus !== newStatus) {
            const label = PM_STATUS_COLUMNS.find((c) => c.key === newStatus)?.label ?? newStatus;
            ElMessage.success({ message: `已移至「${label}」`, duration: 1500 });
          }
        } catch (e) {
          // 失败时把 DOM 上的卡片回滚到原列原位,避免 UI 与服务器数据不一致
          try {
            const refNode = fromEl.children[oldIndex] ?? null;
            fromEl.insertBefore(evt.item, refNode);
          } catch (rollbackError) {
            console.warn("看板卡片 DOM 回滚失败", rollbackError);
          }
          ElMessage.error((e as Error).message);
          emit("items-changed");
        }
      },
    });
    sortableInstances.set(col.key, instance);
  }
}

function destroySortable() {
  for (const inst of sortableInstances.values()) {
    inst.destroy();
  }
  sortableInstances.clear();
}

watch(
  () => [props.enabled, visibleStatusColumnKey.value, props.items.length],
  () => {
    nextTick(() => {
      if (!dragState?.draggingItemId.value) initSortable();
    });
  },
);

onMounted(() => {
  nextTick(() => initSortable());
});

onBeforeUnmount(() => {
  destroySortable();
});
</script>

<style scoped>
/* Kanban */
.kanban-board {
  display: flex;
  flex: 1;
  gap: 0;
  overflow-x: auto;
  padding: 12px;
  background: var(--lc-surface-1);
  border-radius: var(--lc-radius-lg);
}
.kanban-column {
  flex: 1;
  min-width: 240px;
  display: flex;
  flex-direction: column;
  background: var(--el-fill-color-lighter);
  border-radius: 6px;
  margin: 0 4px;
}
.column-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  font-weight: 600;
  font-size: 15px;
  border-bottom: 2px solid transparent;
  position: relative;
}
.column-count {
  border-radius: 10px;
  padding: 0 8px;
  font-size: 12px;
  font-weight: 600;
}
.column-body {
  flex: 1;
  padding: 8px;
  overflow-y: auto;
  min-height: 120px;
}

/* Cards */
.kanban-card {
  position: relative;
  background: var(--el-bg-color);
  border: 1px solid var(--el-border-color-lighter);
  border-left: 3px solid var(--el-color-primary);
  border-radius: 6px;
  padding: 12px 12px 12px 12px;
  margin-bottom: 8px;
  cursor: grab;
  transition: box-shadow 0.15s, border-color 0.15s, margin 0.15s;
}
.kanban-card:hover {
  margin-top: -1px;
  margin-bottom: 9px;
  border-color: var(--el-color-primary-light-5);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
}
.kanban-card:hover .card-advance-btn {
  opacity: 1;
}
.kanban-card.is-selected {
  border-color: var(--el-color-primary);
  box-shadow: 0 0 0 1px var(--el-color-primary-light-5);
  background: var(--el-color-primary-light-9);
}
.kanban-card.is-pinned {
  border-top: 2px solid var(--el-color-warning);
}
.kanban-card.is-p0 {
  border-left-color: #f56c6c;
}
.kanban-card.is-overdue {
  background: linear-gradient(135deg, rgba(248, 113, 113, 0.06), var(--el-bg-color) 60%);
}
.kanban-card.is-overdue::after {
  content: '';
  position: absolute;
  top: 0;
  right: 0;
  border-style: solid;
  border-width: 0 16px 16px 0;
  border-color: transparent #f56c6c transparent transparent;
  border-top-right-radius: 5px;
}
.kanban-card.is-overdue:hover {
  background: linear-gradient(135deg, rgba(248, 113, 113, 0.10), var(--el-bg-color) 60%);
}
.card-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 4px;
  margin-bottom: 8px;
}
.card-title {
  font-size: 15px;
  font-weight: 500;
  line-height: 1.4;
  word-break: break-all;
}
.card-badges {
  display: flex;
  align-items: center;
  gap: 2px;
  flex-shrink: 0;
}
.badge-pin {
  color: var(--el-color-warning);
  font-size: 14px;
}
.badge-overdue {
  color: var(--lc-danger, #f56c6c);
  font-size: 14px;
}
.card-meta {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
  margin-bottom: 4px;
}
.card-meta :deep(.el-tag) {
  font-size: 12px;
  font-weight: 500;
  height: 18px;
  line-height: 18px;
  padding: 0 6px;
}
.card-tags {
  display: flex;
  gap: 3px;
  flex-wrap: wrap;
  margin-bottom: 4px;
}
.card-tags :deep(.el-tag) {
  font-size: 12px;
  height: 18px;
}
.card-dates {
  font-size: 13px;
  color: var(--el-text-color-secondary);
}
.is-overdue-date {
  color: var(--lc-danger, #f56c6c);
  font-weight: 600;
}
.priority-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-right: 6px;
  vertical-align: middle;
  flex-shrink: 0;
}
.card-project {
  margin-top: 4px;
}
.card-project-badge {
  display: inline-block;
  font-size: 11px;
  font-weight: 500;
  padding: 1px 6px;
  border-radius: 3px;
  line-height: 16px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 100%;
}
.card-project-dot {
  width: 8px;
  height: 8px;
  border-radius: 2px;
  flex-shrink: 0;
}
.card-project-name {
  font-size: 13px;
  color: var(--el-text-color-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* Quick action button */
.card-advance-btn {
  position: absolute;
  right: 6px;
  bottom: 6px;
  width: 26px;
  height: 26px;
  border-radius: 13px;
  border: 1px solid var(--el-border-color-light);
  background: var(--el-bg-color);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  opacity: 0.35;
  transition: opacity 0.15s, background 0.15s, color 0.15s;
  color: var(--el-text-color-secondary);
}
.card-advance-btn:hover {
  background: var(--el-color-success-light-9);
  border-color: var(--el-color-success-light-5);
  color: var(--el-color-success);
}

/* Drag */
:deep(.kanban-ghost) {
  opacity: 0.4;
  border: 2px dashed var(--el-color-primary-light-5);
  background: var(--el-color-primary-light-9);
  border-radius: 6px;
  box-shadow: none;
}

:deep(.kanban-drag),
:deep(.kanban-fallback) {
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
  transform: rotate(2deg);
  opacity: 0.92;
  z-index: 100;
}

/* Column drag-over highlight */
.kanban-column.is-drag-over {
  background: var(--el-color-primary-light-9);
  box-shadow: inset 0 0 0 2px var(--el-color-primary-light-5);
  transition: background 0.15s, box-shadow 0.15s;
}
.kanban-column.is-drag-over .column-header {
  color: var(--el-color-primary);
}
.kanban-column.is-drag-over .column-header::after {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 3px;
  background: var(--el-color-primary);
  animation: drag-bar-in 0.2s ease;
}
@keyframes drag-bar-in {
  from { transform: scaleX(0); }
  to { transform: scaleX(1); }
}

/* Empty column drop hint */
.column-drop-hint {
  text-align: center;
  padding: 16px 8px;
  color: var(--el-text-color-placeholder);
  font-size: 13px;
  border: 2px dashed var(--el-border-color-light);
  border-radius: 6px;
  pointer-events: none;
}
.column-empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 24px 8px;
  border: 1px dashed var(--el-border-color-lighter);
  border-radius: 6px;
}
.column-empty-text {
  font-size: 13px;
  color: var(--el-text-color-placeholder);
}

/* Empty state */
.pm-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* ── PM visual unification overrides ─────────────────── */
.kanban-board {
  gap: 12px;
  padding: 18px;
}

.kanban-column {
  min-width: 272px;
  margin: 0;
  border: 1px solid var(--pm-edge-soft);
  border-radius: 18px;
  background: rgba(255, 255, 255, 0.72);
  box-shadow: var(--pm-shadow-soft);
  overflow: hidden;
}

.column-header {
  padding: 14px 16px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.98), rgba(244, 247, 251, 0.9));
}

.column-title {
  color: var(--pm-text-main);
}

.column-count {
  min-width: 30px;
  line-height: 22px;
  text-align: center;
}

.column-body {
  padding: 10px;
  background: linear-gradient(180deg, rgba(244, 247, 251, 0.82), rgba(255, 255, 255, 0.58));
}

.kanban-card {
  padding: 12px;
  margin-bottom: 10px;
  border-width: 1px 1px 1px 4px;
  border-radius: 16px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(248, 251, 255, 0.98));
  box-shadow: 0 10px 22px rgba(34, 48, 66, 0.05);
}

.kanban-card:hover {
  margin-top: 0;
  margin-bottom: 10px;
  border-color: rgba(14, 165, 233, 0.3);
  box-shadow: var(--pm-shadow-strong);
  transform: translateY(-1px);
}

.kanban-card.is-selected {
  border-color: rgba(14, 165, 233, 0.42);
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(233, 241, 255, 0.92));
  box-shadow:
    0 0 0 1px rgba(14, 165, 233, 0.12),
    0 18px 32px rgba(14, 165, 233, 0.14);
}

.kanban-card.is-overdue {
  background: linear-gradient(180deg, rgba(255, 247, 247, 0.96), rgba(255, 255, 255, 1));
}

.kanban-card.is-overdue:hover {
  background: linear-gradient(180deg, rgba(255, 244, 244, 1), rgba(255, 255, 255, 1));
}

.card-topbar {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 10px;
}

.card-topbar-left,
.card-topbar-right {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  min-width: 0;
}

.card-topbar-right {
  justify-content: flex-end;
}

.card-project-badge {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  max-width: 100%;
  padding: 4px 10px;
  border: 1px solid rgba(14, 165, 233, 0.12);
  border-radius: 999px;
  font-size: 12px;
  font-weight: 600;
}

.card-project-name {
  color: inherit;
  font-size: 12px;
}

.card-meta-pill {
  display: inline-flex;
  align-items: center;
  padding: 4px 10px;
  border: 1px solid;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.78);
  font-size: 12px;
  font-weight: 600;
  line-height: 1;
}

.card-date-chip {
  display: inline-flex;
  align-items: center;
  padding: 4px 8px;
  border-radius: 999px;
  background: rgba(219, 229, 241, 0.7);
  color: var(--pm-text-muted);
  font-size: 12px;
  font-weight: 600;
}

.card-title-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 8px;
}

.card-title {
  flex: 1;
  min-width: 0;
  font-size: 15px;
  font-weight: 700;
  line-height: 1.45;
  color: var(--pm-text-main);
  word-break: break-word;
}

.card-meta {
  gap: 6px;
  margin-bottom: 8px;
}

.card-tags {
  gap: 6px;
  margin-bottom: 0;
}

.card-tags :deep(.el-tag) {
  border-radius: 999px;
}

.card-advance-btn {
  position: static;
  width: 28px;
  height: 28px;
  flex-shrink: 0;
  border-radius: 10px;
  border-color: rgba(14, 165, 233, 0.16);
  background: rgba(14, 165, 233, 0.08);
  color: var(--pm-accent);
  opacity: 0;
  visibility: hidden;
  pointer-events: none;
  transform: translateY(2px);
  transition:
    opacity 0.16s ease,
    visibility 0.16s ease,
    transform 0.16s ease,
    background 0.15s ease,
    border-color 0.15s ease,
    color 0.15s ease;
}

.kanban-card:hover .card-advance-btn,
.kanban-card:focus-within .card-advance-btn {
  opacity: 1;
  visibility: visible;
  pointer-events: auto;
  transform: translateY(0);
}

.card-advance-btn:hover {
  background: var(--pm-accent);
  border-color: var(--pm-accent);
  color: #ffffff;
}

.column-drop-hint {
  border-radius: 14px;
  border-color: rgba(14, 165, 233, 0.32);
  background: rgba(14, 165, 233, 0.06);
  color: var(--pm-accent);
}

.column-empty-state {
  border-radius: 14px;
  border-color: var(--pm-edge);
  background: rgba(255, 255, 255, 0.68);
}

.column-empty-text {
  color: var(--pm-text-muted);
}
</style>

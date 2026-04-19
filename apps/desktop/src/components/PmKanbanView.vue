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
                <span class="card-project-badge" :style="{ backgroundColor: (item.projectColor || '#4d7df2') + '18', color: item.projectColor || '#4d7df2' }">
                  <span class="card-project-dot" :style="{ backgroundColor: item.projectColor || '#4d7df2' }" />
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
import type { PmItem, PmItemStatus } from "../types/pm";
import { PM_STATUS_COLUMNS, PM_ITEM_TYPE_MAP, PM_PRIORITY_MAP } from "../types/pm";
import {
  formatPmDateRangeForDisplay,
  hasPmDateSchedule,
  isPmItemOverdue,
} from "../utils/pmDate";
import { summarizePmItemTags } from "../utils/pmVisual";
import { getVisiblePmStatusColumns } from "../utils/pmStatusFilter";
import { useToolInvoke } from "../composables/useToolInvoke";
import { PM_KANBAN_DRAG_KEY } from "../composables/pmKanbanDragKey";

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

        try {
          const oldStatus = (evt.from as HTMLElement).dataset.status;
          const statusChanged = oldStatus !== newStatus;
          const children = Array.from(evt.to.children) as HTMLElement[];
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
.kanban-board {
  display: flex;
  flex: 1;
  gap: 0;
  overflow-x: auto;
  padding: 12px;
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

.kanban-card {
  position: relative;
  background: var(--el-bg-color);
  border: 1px solid var(--el-border-color-lighter);
  border-left: 3px solid var(--el-color-primary);
  border-radius: 6px;
  padding: 12px;
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

.card-title {
  font-size: 15px;
  font-weight: 500;
  line-height: 1.4;
  word-break: break-all;
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
.is-overdue-date {
  color: var(--lc-danger, #f56c6c);
  font-weight: 600;
}

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

.pm-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}
</style>

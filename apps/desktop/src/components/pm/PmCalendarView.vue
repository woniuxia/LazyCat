<template>
  <div class="pm-calendar-view">
    <div class="calendar-toolbar">
      <div class="toolbar-left">
        <el-button size="small" @click="goPrev">
          <span class="arrow">‹</span>
        </el-button>
        <el-button size="small" @click="goToday">今天</el-button>
        <el-button size="small" @click="goNext">
          <span class="arrow">›</span>
        </el-button>
        <span class="calendar-title">{{ titleText }}</span>
      </div>
      <div class="toolbar-right">
        <div class="segmented">
          <button
            type="button"
            class="seg-btn"
            :class="{ 'is-active': subview === 'month' }"
            @click="setSubview('month')"
          >
            月
          </button>
          <button
            type="button"
            class="seg-btn"
            :class="{ 'is-active': subview === 'week' }"
            @click="setSubview('week')"
          >
            周
          </button>
        </div>
        <el-select
          :model-value="colorBy"
          size="small"
          class="color-by-select"
          @update:model-value="(v) => setColorBy(v as ColorBy)"
        >
          <el-option label="按项目色" value="project" />
          <el-option label="按优先级" value="priority" />
          <el-option label="按状态" value="status" />
        </el-select>
      </div>
    </div>

    <div v-if="loading && monthItems.length === 0" class="calendar-loading">加载中…</div>

    <div v-if="subview === 'month'" class="month-grid">
      <div class="month-header">
        <div
          v-for="(label, idx) in WEEKDAY_LABELS"
          :key="label"
          class="month-weekday"
          :class="{ 'is-weekend': idx === 0 || idx === 6 }"
        >
          {{ label }}
        </div>
      </div>
      <div class="month-body">
        <div
          v-for="cell in monthCells"
          :key="cell.date"
          class="month-cell"
          :class="{
            'is-other-month': !cell.isCurrentMonth,
            'is-today': cell.isToday,
            'is-weekend': cell.isWeekend,
            'is-drop-target': dropTargetDate === cell.date,
          }"
          @click.self="onCellBlankClick(cell)"
          @dragover.prevent="onCellDragOver(cell)"
          @dragleave="onCellDragLeave(cell)"
          @drop.prevent="onCellDrop(cell)"
        >
          <div class="cell-head" @click="onCellBlankClick(cell)">
            <span class="cell-day" :class="{ 'is-today-chip': cell.isToday }">
              {{ cell.day }}
            </span>
            <span v-if="cell.isToday" class="today-badge">今天</span>
          </div>
          <div class="cell-body">
            <div
              v-for="item in cell.visibleItems"
              :key="item.id"
              class="task-bar"
              :class="[
                `is-${colorBy}`,
                `p-${item.priority.toLowerCase()}`,
                `s-${item.status}`,
                {
                  'is-overdue': item.__overdue,
                  'is-done': item.status === 'done',
                  'is-selected': selectedItemId === item.id,
                },
              ]"
              :style="buildBarStyle(item)"
              draggable="true"
              :title="item.title"
              @click.stop="onItemClick($event, item)"
              @dblclick.stop="onItemDblclick(item)"
              @contextmenu.prevent.stop="(e: MouseEvent) => onItemContext(e, item)"
              @dragstart="onItemDragStart($event, item)"
              @dragend="onItemDragEnd"
            >
              <span class="bar-dot" :style="{ backgroundColor: barDotColor(item) }" />
              <span class="bar-title">{{ item.title }}</span>
            </div>
            <div
              v-if="cell.overflowText"
              class="cell-overflow"
              @click.stop="openOverflow(cell, $event)"
            >
              {{ cell.overflowText }}
            </div>
          </div>
        </div>
      </div>
    </div>

    <div v-else class="week-grid">
      <div
        v-for="cell in weekCells"
        :key="cell.date"
        class="week-col"
        :class="{
          'is-today': cell.isToday,
          'is-weekend': cell.isWeekend,
          'is-drop-target': dropTargetDate === cell.date,
        }"
        @dragover.prevent="onCellDragOver(cell)"
        @dragleave="onCellDragLeave(cell)"
        @drop.prevent="onCellDrop(cell)"
      >
        <div class="week-col-head" @click.self="onCellBlankClick(cell)">
          <span class="wd-label">{{ WEEKDAY_LABELS[cell.weekday] }}</span>
          <span class="wd-day" :class="{ 'is-today-chip': cell.isToday }">{{ cell.day }}</span>
          <span class="wd-count" v-if="cell.items.length > 0">{{ cell.items.length }} 项</span>
          <span v-else class="wd-count wd-count-empty">&nbsp;</span>
        </div>
        <div class="week-col-body">
          <div
            v-if="cell.items.length === 0"
            class="week-col-empty"
            @click="onCellBlankClick(cell)"
          >
            无安排
          </div>
          <div
            v-else
            v-for="item in cell.items"
            :key="item.id"
            class="week-card"
            :class="[
              `p-${item.priority.toLowerCase()}`,
              {
                'is-overdue': item.__overdue,
                'is-done': item.status === 'done',
                'is-selected': selectedItemId === item.id,
              },
            ]"
            draggable="true"
            @click.stop="onItemClick($event, item)"
            @dblclick.stop="onItemDblclick(item)"
            @contextmenu.prevent.stop="(e: MouseEvent) => onItemContext(e, item)"
            @dragstart="onItemDragStart($event, item)"
            @dragend="onItemDragEnd"
          >
            <div class="week-card-title">{{ item.title }}</div>
            <div class="week-card-meta">
              <span
                v-if="item.projectName"
                class="card-project-chip"
                :style="{
                  backgroundColor: (item.projectColor || '#0ea5e9') + '18',
                  color: item.projectColor || '#0ea5e9',
                }"
              >
                <span
                  class="card-project-dot"
                  :style="{ backgroundColor: item.projectColor || '#0ea5e9' }"
                />
                {{ item.projectName }}
              </span>
              <span
                class="card-pill"
                :style="{
                  color: PM_PRIORITY_MAP[item.priority]?.color,
                  borderColor: (PM_PRIORITY_MAP[item.priority]?.color || '#999') + '40',
                }"
              >
                {{ PM_PRIORITY_MAP[item.priority]?.label }}
              </span>
              <span class="card-status" :style="{ color: statusColor(item.status) }">
                {{ statusLabel(item.status) }}
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <el-dialog
      v-model="overflowDialogVisible"
      :title="overflowDialogTitle"
      width="460px"
      align-center
      append-to-body
    >
      <div class="overflow-list">
        <div
          v-for="item in overflowItems"
          :key="item.id"
          class="overflow-row"
          @click="onOverflowRowClick(item)"
        >
          <span
            class="card-project-dot"
            :style="{ backgroundColor: item.projectColor || '#0ea5e9' }"
          />
          <span class="overflow-title">{{ item.title }}</span>
          <span
            class="card-pill"
            :style="{
              color: PM_PRIORITY_MAP[item.priority]?.color,
              borderColor: (PM_PRIORITY_MAP[item.priority]?.color || '#999') + '40',
            }"
          >
            {{ PM_PRIORITY_MAP[item.priority]?.label }}
          </span>
        </div>
      </div>
    </el-dialog>

    <el-popover
      :visible="popoverVisible"
      :virtual-ref="popoverAnchor ?? undefined"
      virtual-triggering
      placement="right-start"
      :width="280"
      :show-arrow="true"
      popper-class="pm-calendar-popover"
      @hide="onPopoverHide"
    >
      <div v-if="popoverItem" class="pop-content" @click.stop>
        <div class="pop-title">{{ popoverItem.title }}</div>
        <div class="pop-meta">
          <span
            v-if="popoverItem.projectName"
            class="card-project-chip"
            :style="{
              backgroundColor: (popoverItem.projectColor || '#0ea5e9') + '18',
              color: popoverItem.projectColor || '#0ea5e9',
            }"
          >
            <span
              class="card-project-dot"
              :style="{ backgroundColor: popoverItem.projectColor || '#0ea5e9' }"
            />
            {{ popoverItem.projectName }}
          </span>
          <span
            class="card-pill"
            :style="{
              color: PM_PRIORITY_MAP[popoverItem.priority]?.color,
              borderColor:
                (PM_PRIORITY_MAP[popoverItem.priority]?.color || '#999') + '40',
            }"
          >
            {{ PM_PRIORITY_MAP[popoverItem.priority]?.label }}
          </span>
          <span class="pop-status" :style="{ color: statusColor(popoverItem.status) }">
            {{ statusLabel(popoverItem.status) }}
          </span>
        </div>
        <div v-if="popoverDateText" class="pop-date" :class="{ 'is-overdue': popoverItem.__overdue }">
          {{ popoverDateText }}
        </div>
        <div v-if="popoverItem.description" class="pop-desc">
          {{ popoverDescText }}
        </div>
        <div class="pop-actions">
          <el-button
            v-if="popoverItem.status === 'todo'"
            size="small"
            @click="popoverQuickStart"
          >
            开始做
          </el-button>
          <el-button
            v-if="popoverItem.status !== 'done' && popoverItem.endAt"
            size="small"
            @click="popoverPostpone"
          >
            推到明天
          </el-button>
          <el-button
            v-if="popoverItem.status !== 'done'"
            size="small"
            type="success"
            @click="popoverQuickComplete"
          >
            标记完成
          </el-button>
          <el-button size="small" @click="popoverEdit">编辑</el-button>
          <el-button size="small" type="primary" @click="popoverShowDetail">详情</el-button>
        </div>
      </div>
    </el-popover>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import type { PmItem, PmItemStatus } from "../../types/pm";
import { PM_PRIORITY_MAP, PM_STATUS_COLUMNS } from "../../types/pm";
import { useToolInvoke } from "../../composables/useToolInvoke";
import { getSetting, setSetting } from "../../composables/useSettings";
import { formatPmDateRangeForDisplay, isPmItemOverdue } from "../../utils/pmDate";

type ColorBy = "project" | "priority" | "status";
type Subview = "month" | "week";

interface CalendarCell {
  date: string;
  day: number;
  weekday: number;
  isCurrentMonth: boolean;
  isToday: boolean;
  isWeekend: boolean;
  items: ItemView[];
  visibleItems: ItemView[];
  overflowText: string;
}

interface ItemView extends PmItem {
  __overdue: boolean;
}

const WEEKDAY_LABELS = ["日", "一", "二", "三", "四", "五", "六"];

const props = defineProps<{
  selectedProjectId: number | "overview" | null;
  selectedItemId: number | null;
}>();

const emit = defineEmits<{
  (e: "select", item: PmItem): void;
  (e: "edit", item: PmItem): void;
  (e: "item-context", event: MouseEvent, item: PmItem): void;
  (e: "create-at-date", date: string): void;
  (e: "items-changed"): void;
}>();

const { invoke } = useToolInvoke();

const loading = ref(false);
const monthItems = ref<ItemView[]>([]);

const anchor = ref<Date>(startOfToday());
const subview = ref<Subview>(readSubview());
const colorBy = ref<ColorBy>(readColorBy());

const dropTargetDate = ref<string | null>(null);
const draggingItemId = ref<number | null>(null);

const overflowDialogVisible = ref(false);
const overflowDialogTitle = ref("");
const overflowItems = ref<ItemView[]>([]);

const popoverVisible = ref(false);
const popoverAnchor = ref<HTMLElement | null>(null);
const popoverItem = ref<ItemView | null>(null);

const popoverDateText = computed(() => {
  if (!popoverItem.value) return "";
  const { startAt, endAt } = popoverItem.value;
  if (!startAt && !endAt) return "";
  return formatPmDateRangeForDisplay(startAt, endAt, { mode: "full", emptyText: "" });
});

const popoverDescText = computed(() => {
  const text = popoverItem.value?.description ?? "";
  if (text.length <= 120) return text;
  return text.slice(0, 120) + "…";
});

function startOfToday(): Date {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return d;
}

function formatDate(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function parseDate(value: string): Date {
  const [y, m, d] = value.split("-").map(Number);
  return new Date(y, (m || 1) - 1, d || 1);
}

function ctxKey(suffix: string): string {
  const id = props.selectedProjectId;
  const ctx = id === "overview" ? "overview" : id === null ? "overview" : `project-${id}`;
  return `pm:view:calendar:${suffix}:${ctx}`;
}

function readSubview(): Subview {
  const raw = getSetting(ctxKey("subview"));
  return raw === "week" ? "week" : "month";
}

function readColorBy(): ColorBy {
  const raw = getSetting(ctxKey("colorBy"));
  if (raw === "priority" || raw === "status") return raw;
  return "project";
}

function setSubview(v: Subview) {
  subview.value = v;
  setSetting(ctxKey("subview"), v);
  closePopover();
  reloadFrame();
}

function setColorBy(v: ColorBy) {
  colorBy.value = v;
  setSetting(ctxKey("colorBy"), v);
  closePopover();
}

const titleText = computed(() => {
  if (subview.value === "month") {
    return `${anchor.value.getFullYear()} 年 ${anchor.value.getMonth() + 1} 月`;
  }
  const weekStart = getWeekStart(anchor.value);
  const weekEnd = addDays(weekStart, 6);
  const s = `${weekStart.getMonth() + 1}.${weekStart.getDate()}`;
  const e = `${weekEnd.getMonth() + 1}.${weekEnd.getDate()}`;
  return `${anchor.value.getFullYear()} 年 ${s} - ${e}`;
});

function getWeekStart(d: Date): Date {
  const start = new Date(d);
  start.setHours(0, 0, 0, 0);
  start.setDate(start.getDate() - start.getDay());
  return start;
}

function addDays(d: Date, n: number): Date {
  const next = new Date(d);
  next.setDate(next.getDate() + n);
  return next;
}

function goPrev() {
  closePopover();
  if (subview.value === "month") {
    const next = new Date(anchor.value);
    next.setDate(1);
    next.setMonth(next.getMonth() - 1);
    anchor.value = next;
  } else {
    anchor.value = addDays(anchor.value, -7);
  }
  reloadFrame();
}

function goNext() {
  closePopover();
  if (subview.value === "month") {
    const next = new Date(anchor.value);
    next.setDate(1);
    next.setMonth(next.getMonth() + 1);
    anchor.value = next;
  } else {
    anchor.value = addDays(anchor.value, 7);
  }
  reloadFrame();
}

function goToday() {
  closePopover();
  anchor.value = startOfToday();
  reloadFrame();
}

function computeFrame(): { start: Date; end: Date } {
  if (subview.value === "month") {
    const firstOfMonth = new Date(anchor.value.getFullYear(), anchor.value.getMonth(), 1);
    const start = addDays(firstOfMonth, -firstOfMonth.getDay());
    const end = addDays(start, 41);
    return { start, end };
  }
  const start = getWeekStart(anchor.value);
  const end = addDays(start, 6);
  return { start, end };
}

async function reloadFrame() {
  if (props.selectedProjectId === null) {
    monthItems.value = [];
    return;
  }
  const { start, end } = computeFrame();
  const payload: Record<string, unknown> = {
    startDate: formatDate(start),
    endDate: formatDate(end),
  };
  const pid = props.selectedProjectId;
  if (typeof pid === "number") payload.projectId = pid;
  loading.value = true;
  try {
    const result = (await invoke<{ items: PmItem[] }>("tool:pm:item-calendar-range", payload)) ?? {
      items: [],
    };
    monthItems.value = (result.items ?? []).map((item) => ({
      ...item,
      __overdue: isPmItemOverdue(item),
    }));
  } catch (e) {
    ElMessage.error((e as Error).message);
    monthItems.value = [];
  } finally {
    loading.value = false;
  }
}

function datePrefix(value: string | null | undefined): string | null {
  if (!value) return null;
  return value.length >= 10 ? value.slice(0, 10) : null;
}

function itemEndAtDate(item: ItemView): string | null {
  return datePrefix(item.endAt);
}

function itemStartAtDate(item: ItemView): string | null {
  return datePrefix(item.startAt);
}

function itemOccupiesDate(item: ItemView, date: string): boolean {
  const end = itemEndAtDate(item);
  const start = itemStartAtDate(item);
  if (end && !start) return end === date;
  if (!end && start) return start === date;
  if (start && end) {
    return date >= start && date <= end;
  }
  return false;
}

function buildCells(start: Date, count: number): CalendarCell[] {
  const today = formatDate(startOfToday());
  const cells: CalendarCell[] = [];
  for (let i = 0; i < count; i += 1) {
    const d = addDays(start, i);
    const dateStr = formatDate(d);
    const items = monthItems.value.filter((it) => itemOccupiesDate(it, dateStr));
    const visible = items.slice(0, items.length <= 4 ? 4 : 3);
    let overflowText = "";
    if (items.length >= 5) {
      const hiddenProjects = Array.from(
        new Set(items.slice(3).map((it) => it.projectName).filter(Boolean)),
      ) as string[];
      const label = hiddenProjects.slice(0, 2).join("、");
      const suffix = label ? `${label} 等项目` : "";
      overflowText = `${suffix}还有 ${items.length - 3} 条`.trim();
    }
    cells.push({
      date: dateStr,
      day: d.getDate(),
      weekday: d.getDay(),
      isCurrentMonth:
        subview.value === "week" || d.getMonth() === anchor.value.getMonth(),
      isToday: dateStr === today,
      isWeekend: d.getDay() === 0 || d.getDay() === 6,
      items,
      visibleItems: visible,
      overflowText,
    });
  }
  return cells;
}

const monthCells = computed<CalendarCell[]>(() => {
  const { start } = computeFrame();
  return buildCells(start, 42);
});

const weekCells = computed<CalendarCell[]>(() => {
  const { start } = computeFrame();
  return buildCells(start, 7);
});

function statusColor(status: PmItemStatus): string {
  return PM_STATUS_COLUMNS.find((c) => c.key === status)?.color ?? "#909399";
}

function statusLabel(status: PmItemStatus): string {
  return PM_STATUS_COLUMNS.find((c) => c.key === status)?.label ?? status;
}

function barDotColor(item: ItemView): string {
  if (colorBy.value === "priority") {
    return PM_PRIORITY_MAP[item.priority]?.color ?? "#909399";
  }
  if (colorBy.value === "status") {
    return statusColor(item.status);
  }
  return item.projectColor || "#0ea5e9";
}

function buildBarStyle(item: ItemView): Record<string, string> {
  if (colorBy.value === "priority") {
    const color = PM_PRIORITY_MAP[item.priority]?.color ?? "#909399";
    return {
      backgroundColor: color + "1a",
      borderColor: color + "66",
      color,
    };
  }
  if (colorBy.value === "status") {
    const color = statusColor(item.status);
    return {
      backgroundColor: color + "1a",
      borderColor: color + "66",
      color,
    };
  }
  const color = item.projectColor || "#0ea5e9";
  return {
    backgroundColor: color + "1a",
    borderColor: color + "66",
    color: color,
  };
}

function onCellBlankClick(cell: CalendarCell) {
  closePopover();
  emit("create-at-date", cell.date);
}

function onItemClick(event: MouseEvent, item: ItemView) {
  const target = event.currentTarget as HTMLElement | null;
  if (!target) return;
  if (popoverVisible.value && popoverItem.value?.id === item.id) {
    closePopover();
    return;
  }
  if (popoverVisible.value) {
    popoverVisible.value = false;
    void nextTick(() => {
      popoverAnchor.value = target;
      popoverItem.value = item;
      popoverVisible.value = true;
    });
    return;
  }
  popoverAnchor.value = target;
  popoverItem.value = item;
  popoverVisible.value = true;
}

function onItemDblclick(item: ItemView) {
  closePopover();
  emit("edit", item);
}

function onItemContext(event: MouseEvent, item: ItemView) {
  closePopover();
  emit("item-context", event, item);
}

function onItemDragStart(event: DragEvent, item: ItemView) {
  closePopover();
  draggingItemId.value = item.id;
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", String(item.id));
  }
}

function onItemDragEnd() {
  draggingItemId.value = null;
  dropTargetDate.value = null;
}

function onCellDragOver(cell: CalendarCell) {
  if (draggingItemId.value === null) return;
  dropTargetDate.value = cell.date;
}

function onCellDragLeave(cell: CalendarCell) {
  if (dropTargetDate.value === cell.date) dropTargetDate.value = null;
}

async function onCellDrop(cell: CalendarCell) {
  const id = draggingItemId.value;
  draggingItemId.value = null;
  dropTargetDate.value = null;
  if (id === null) return;
  const item = monthItems.value.find((it) => it.id === id);
  if (!item) return;
  const currentEnd = itemEndAtDate(item);
  if (currentEnd === cell.date) return;

  try {
    await ElMessageBox.confirm(
      `将「${item.title}」的截止日期改为 ${cell.date} ？`,
      "移动任务",
      {
        confirmButtonText: "确认",
        cancelButtonText: "取消",
        type: "info",
      },
    );
  } catch {
    return;
  }

  try {
    const start = itemStartAtDate(item);
    const nextStart = start && start > cell.date ? cell.date : start;
    await invoke("tool:pm:item-update", {
      id: item.id,
      startAt: nextStart,
      endAt: cell.date,
    });
    ElMessage.success({ message: "已更新截止日期", duration: 1500 });
    await reloadFrame();
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

function openOverflow(cell: CalendarCell, _event: MouseEvent) {
  closePopover();
  overflowDialogTitle.value = `${cell.date} 的全部任务（${cell.items.length}）`;
  overflowItems.value = [...cell.items];
  overflowDialogVisible.value = true;
}

function onOverflowRowClick(item: ItemView) {
  overflowDialogVisible.value = false;
  emit("select", item);
}

function closePopover() {
  if (!popoverVisible.value) return;
  popoverVisible.value = false;
  popoverAnchor.value = null;
  popoverItem.value = null;
}

function onPopoverHide() {
  popoverAnchor.value = null;
  popoverItem.value = null;
}

function popoverShowDetail() {
  if (!popoverItem.value) return;
  const item = popoverItem.value;
  closePopover();
  emit("select", item);
}

function popoverEdit() {
  if (!popoverItem.value) return;
  const item = popoverItem.value;
  closePopover();
  emit("edit", item);
}

async function popoverQuickStart() {
  if (!popoverItem.value) return;
  const item = popoverItem.value;
  closePopover();
  try {
    await invoke("tool:pm:item-change-status", { id: item.id, status: "in_progress" });
    ElMessage.success({ message: "已开始", duration: 1500 });
    await reloadFrame();
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function popoverQuickComplete() {
  if (!popoverItem.value) return;
  const item = popoverItem.value;
  closePopover();
  try {
    await invoke("tool:pm:item-change-status", { id: item.id, status: "done" });
    ElMessage.success({ message: "已标记完成", duration: 1500 });
    await reloadFrame();
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

function shiftDateOneDay(value: string | null): string | null {
  if (!value) return null;
  const prefix = value.length >= 10 ? value.slice(0, 10) : value;
  const parts = prefix.split("-");
  if (parts.length !== 3) return null;
  const y = Number(parts[0]);
  const m = Number(parts[1]);
  const d = Number(parts[2]);
  if (!Number.isInteger(y) || !Number.isInteger(m) || !Number.isInteger(d)) return null;
  const date = new Date(y, m - 1, d);
  date.setDate(date.getDate() + 1);
  return formatDate(date);
}

async function popoverPostpone() {
  if (!popoverItem.value) return;
  const item = popoverItem.value;
  closePopover();
  const currentEnd = item.endAt ?? formatDate(new Date());
  const nextEnd = shiftDateOneDay(currentEnd);
  if (!nextEnd) {
    ElMessage.error("截止日期格式异常，无法推迟");
    return;
  }
  const startPrefix = itemStartAtDate(item);
  const nextStart = startPrefix && startPrefix > nextEnd ? nextEnd : item.startAt;
  try {
    await invoke("tool:pm:item-update", {
      id: item.id,
      startAt: nextStart,
      endAt: nextEnd,
    });
    ElMessage.success({ message: "已推到明天", duration: 1500 });
    await reloadFrame();
    emit("items-changed");
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

function onDocumentClick(event: MouseEvent) {
  if (!popoverVisible.value) return;
  const target = event.target as Node | null;
  if (!target) return;
  if (popoverAnchor.value && popoverAnchor.value.contains(target)) return;
  const popper = document.querySelector(".pm-calendar-popover");
  if (popper && popper.contains(target)) return;
  closePopover();
}

watch(
  () => props.selectedProjectId,
  () => {
    closePopover();
    subview.value = readSubview();
    colorBy.value = readColorBy();
    void reloadFrame();
  },
);

onMounted(() => {
  void reloadFrame();
  document.addEventListener("mousedown", onDocumentClick, true);
});

onBeforeUnmount(() => {
  document.removeEventListener("mousedown", onDocumentClick, true);
});

defineExpose({ refresh: reloadFrame });
</script>

<style scoped>
.pm-calendar-view {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 12px 20px 16px;
  gap: 12px;
  background: var(--lc-surface-1);
  border-radius: var(--lc-radius-lg);
}

.calendar-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.toolbar-left,
.toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.calendar-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  margin-left: 8px;
  min-width: 140px;
  text-align: left;
}

.arrow {
  font-size: 16px;
  line-height: 1;
}

.segmented {
  display: inline-flex;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  overflow: hidden;
  background: var(--el-fill-color-light);
}
.seg-btn {
  appearance: none;
  background: transparent;
  border: 0;
  padding: 4px 14px;
  font-size: 13px;
  color: var(--el-text-color-regular);
  cursor: pointer;
  line-height: 22px;
}
.seg-btn.is-active {
  background: var(--el-color-primary);
  color: #fff;
}

.color-by-select {
  width: 120px;
}

.calendar-loading {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  padding: 6px 2px;
}

.month-grid {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  border: 1px solid var(--pm-edge-soft, #e4e7ed);
  border-radius: 10px;
  background: var(--el-bg-color);
  overflow: hidden;
}

.month-header {
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  background: var(--el-fill-color-light);
  border-bottom: 1px solid var(--pm-edge-soft, #e4e7ed);
}

.month-weekday {
  padding: 8px 10px;
  font-size: 12px;
  color: var(--el-text-color-regular);
  text-align: center;
}
.month-weekday.is-weekend {
  color: var(--el-text-color-secondary);
}

.month-body {
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  grid-auto-rows: minmax(128px, 1fr);
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.month-cell {
  position: relative;
  border-top: 1px solid var(--pm-edge-soft, #e4e7ed);
  border-left: 1px solid var(--pm-edge-soft, #e4e7ed);
  background: var(--el-bg-color);
  padding: 6px 6px 4px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
  cursor: pointer;
  transition: background-color 0.18s;
}
.month-cell:hover {
  background: var(--el-color-primary-light-9);
}
.month-cell.is-other-month {
  background: var(--el-fill-color-lighter);
  color: var(--el-text-color-placeholder);
}
.month-cell.is-weekend {
  background: var(--el-fill-color-lighter);
}
.month-cell.is-today {
  box-shadow: inset 0 0 0 2px var(--el-color-primary);
}
.month-cell.is-drop-target {
  background: var(--el-color-primary-light-9);
  box-shadow: inset 0 0 0 2px var(--el-color-primary-light-5);
}

.cell-head {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--el-text-color-regular);
}
.cell-day {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 22px;
  height: 22px;
  padding: 0 6px;
  border-radius: 11px;
  font-weight: 500;
}
.cell-day.is-today-chip {
  background: var(--el-color-primary);
  color: #fff;
}
.today-badge {
  font-size: 11px;
  color: var(--el-color-primary);
}

.cell-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
  overflow: hidden;
}

.task-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 2px 8px;
  font-size: 12px;
  line-height: 1.4;
  border-radius: 4px;
  cursor: pointer;
  user-select: none;
  border: 1px solid transparent;
  min-width: 0;
}
.task-bar:hover {
  filter: brightness(0.98);
}
.task-bar.is-selected {
  outline: 1px solid var(--el-color-primary);
}
.task-bar.is-overdue {
  background: rgba(245, 108, 108, 0.12) !important;
  border-color: rgba(245, 108, 108, 0.5) !important;
  color: #f56c6c !important;
}
.task-bar.is-done {
  opacity: 0.6;
  text-decoration: line-through;
}

.bar-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

.bar-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cell-overflow {
  font-size: 11px;
  color: var(--el-text-color-secondary);
  padding: 2px 6px;
  border-radius: 4px;
  cursor: pointer;
  background: var(--el-fill-color-light);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.cell-overflow:hover {
  color: var(--el-color-primary);
}

/* Week view */
.week-grid {
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  gap: 8px;
  flex: 1;
  min-height: 0;
  overflow-x: auto;
}
.week-col {
  display: flex;
  flex-direction: column;
  background: var(--el-bg-color);
  border: 1px solid var(--pm-edge-soft, #e4e7ed);
  border-radius: 10px;
  min-width: 120px;
  overflow: hidden;
  transition: box-shadow 0.18s;
}
.week-col.is-today {
  box-shadow: inset 0 0 0 2px var(--el-color-primary);
}
.week-col.is-weekend .week-col-head {
  background: var(--el-fill-color-lighter);
}
.week-col.is-drop-target {
  background: var(--el-color-primary-light-9);
}

.week-col-head {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 10px;
  border-bottom: 1px solid var(--pm-edge-soft, #e4e7ed);
  background: var(--el-fill-color-light);
  cursor: pointer;
  min-height: 36px;
}
.wd-label {
  font-size: 12px;
  color: var(--el-text-color-regular);
}
.wd-day {
  font-size: 13px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  padding: 0 6px;
  border-radius: 9px;
}
.wd-day.is-today-chip {
  background: var(--el-color-primary);
  color: #fff;
}
.wd-count {
  margin-left: auto;
  font-size: 11px;
  color: var(--el-text-color-secondary);
}
.wd-count-empty {
  visibility: hidden;
}

.week-col-body {
  flex: 1;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  overflow-y: auto;
}

.week-col-empty {
  font-size: 12px;
  color: var(--el-text-color-placeholder);
  padding: 8px;
  border: 1px dashed var(--pm-edge-soft, #e4e7ed);
  border-radius: 6px;
  text-align: center;
  cursor: pointer;
}
.week-col-empty:hover {
  border-color: var(--el-color-primary-light-5);
  color: var(--el-color-primary);
}

.week-card {
  background: var(--el-bg-color-page, #fafbfc);
  border: 1px solid var(--pm-edge-soft, #e4e7ed);
  border-radius: 6px;
  padding: 8px 10px;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 6px;
  user-select: none;
}
.week-card:hover {
  border-color: var(--el-color-primary-light-5);
}
.week-card.is-selected {
  border-color: var(--el-color-primary);
  box-shadow: 0 0 0 1px var(--el-color-primary) inset;
}
.week-card.is-overdue {
  border-color: rgba(245, 108, 108, 0.4);
}
.week-card.is-done {
  opacity: 0.6;
}
.week-card.is-done .week-card-title {
  text-decoration: line-through;
}
.week-card-title {
  font-size: 13px;
  color: var(--el-text-color-primary);
  word-break: break-word;
}
.week-card-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
  font-size: 11px;
}

.card-project-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 1px 8px;
  border-radius: 10px;
}
.card-project-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}
.card-pill {
  padding: 0 6px;
  border: 1px solid;
  border-radius: 4px;
  background: transparent;
  font-size: 11px;
}
.card-status {
  font-size: 11px;
}

.overflow-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 420px;
  overflow-y: auto;
}
.overflow-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-radius: 6px;
  cursor: pointer;
  border: 1px solid var(--pm-edge-soft, #e4e7ed);
}
.overflow-row:hover {
  border-color: var(--el-color-primary-light-5);
  background: var(--el-color-primary-light-9);
}
.overflow-title {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  color: var(--el-text-color-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>

<style>
.pm-calendar-popover .pop-content {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 2px 2px 0;
}
.pm-calendar-popover .pop-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  word-break: break-word;
}
.pm-calendar-popover .pop-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
  font-size: 11px;
}
.pm-calendar-popover .card-project-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 1px 8px;
  border-radius: 10px;
}
.pm-calendar-popover .card-project-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}
.pm-calendar-popover .card-pill {
  padding: 0 6px;
  border: 1px solid;
  border-radius: 4px;
  background: transparent;
  font-size: 11px;
}
.pm-calendar-popover .pop-status {
  font-size: 11px;
}
.pm-calendar-popover .pop-date {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  background: var(--el-fill-color-light);
  padding: 2px 8px;
  border-radius: 4px;
  align-self: flex-start;
}
.pm-calendar-popover .pop-date.is-overdue {
  color: #f56c6c;
  background: rgba(245, 108, 108, 0.08);
}
.pm-calendar-popover .pop-desc {
  font-size: 12px;
  color: var(--el-text-color-regular);
  line-height: 1.5;
  word-break: break-word;
  max-height: 80px;
  overflow: hidden;
}
.pm-calendar-popover .pop-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding-top: 4px;
  border-top: 1px solid var(--pm-edge-soft, #e4e7ed);
}
</style>

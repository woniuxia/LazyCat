<template>
  <div class="cal-container">
    <div class="cal-header">
      <el-button text size="small" @click="$emit('prevMonth')">
        <el-icon><ArrowLeft /></el-icon>
      </el-button>
      <span class="cal-month-label">{{ monthLabel }}</span>
      <el-button text size="small" @click="$emit('nextMonth')">
        <el-icon><ArrowRight /></el-icon>
      </el-button>
      <el-button text size="small" class="cal-today-btn" @click="$emit('goToday')">今天</el-button>
    </div>
    <div class="cal-weekdays">
      <div v-for="wd in weekdays" :key="wd" class="cal-weekday">{{ wd }}</div>
    </div>
    <div class="cal-grid">
      <div
        v-for="(day, index) in days"
        :key="index"
        class="cal-cell"
        :class="{
          'is-other-month': !day.isCurrentMonth,
          'is-today': day.isToday,
          'has-items': (itemsByDate.get(day.dateKey)?.length ?? 0) > 0,
        }"
        @click="onCellClick(day)"
      >
        <div class="cal-day-number">{{ day.day }}</div>
        <div class="cal-items">
          <div
            v-for="item in visibleItems(day.dateKey)"
            :key="item.id"
            class="cal-item"
            :class="{ 'is-selected': item.id === selectedItemId, 'is-overdue': item.isOverdue }"
            @click.stop="$emit('selectItem', item)"
          >
            <span class="cal-item-priority" :class="'priority-' + item.priority.toLowerCase()" />
            <span class="cal-item-title">{{ item.title }}</span>
          </div>
          <div v-if="overflowCount(day.dateKey) > 0" class="cal-item-overflow">
            +{{ overflowCount(day.dateKey) }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { ArrowLeft, ArrowRight } from "@element-plus/icons-vue";
import type { TodoItem } from "../../types";
import { getCalendarDays, formatMonthLabel } from "../../utils/calendarGrid";

const MAX_VISIBLE = 3;
const weekdays = ["一", "二", "三", "四", "五", "六", "日"];

const props = defineProps<{
  items: TodoItem[];
  month: Date;
  selectedItemId: number | null;
}>();

const emit = defineEmits<{
  selectItem: [item: TodoItem];
  createOnDate: [dateKey: string];
  prevMonth: [];
  nextMonth: [];
  goToday: [];
}>();

const days = computed(() => getCalendarDays(props.month.getFullYear(), props.month.getMonth()));

const monthLabel = computed(() => formatMonthLabel(props.month));

const itemsByDate = computed(() => {
  const map = new Map<string, TodoItem[]>();
  for (const item of props.items) {
    const eventAt = item.eventAt || item.displayAt;
    if (!eventAt) continue;
    const dateKey = eventAt.slice(0, 10);
    const list = map.get(dateKey);
    if (list) {
      list.push(item);
    } else {
      map.set(dateKey, [item]);
    }
  }
  return map;
});

function visibleItems(dateKey: string) {
  const list = itemsByDate.value.get(dateKey) || [];
  return list.slice(0, MAX_VISIBLE);
}

function overflowCount(dateKey: string) {
  const list = itemsByDate.value.get(dateKey) || [];
  return Math.max(0, list.length - MAX_VISIBLE);
}

function onCellClick(day: { dateKey: string }) {
  const list = itemsByDate.value.get(day.dateKey);
  if (!list || list.length === 0) {
    emit("createOnDate", day.dateKey);
  }
}
</script>

<style scoped>
.cal-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.cal-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--lc-border-subtle);
}

.cal-month-label {
  font-size: 15px;
  font-weight: 600;
  color: var(--lc-text);
  min-width: 100px;
  text-align: center;
}

.cal-today-btn {
  margin-left: auto;
}

.cal-weekdays {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  border-bottom: 1px solid var(--lc-border-subtle);
}

.cal-weekday {
  padding: 6px 0;
  text-align: center;
  font-size: 12px;
  font-weight: 500;
  color: var(--lc-text-muted);
}

.cal-grid {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  flex: 1;
  overflow-y: auto;
}

.cal-cell {
  min-height: 80px;
  border-right: 1px solid var(--lc-border-subtle);
  border-bottom: 1px solid var(--lc-border-subtle);
  padding: 4px;
  cursor: pointer;
  transition: background 0.12s;
}

.cal-cell:nth-child(7n) {
  border-right: none;
}

.cal-cell:hover {
  background: var(--el-fill-color-lighter);
}

.cal-cell.is-other-month {
  opacity: 0.4;
}

.cal-cell.is-today {
  background: var(--el-color-primary-light-9);
}

.cal-cell.is-today .cal-day-number {
  color: var(--el-color-primary);
  font-weight: 700;
}

.cal-day-number {
  font-size: 12px;
  font-weight: 500;
  color: var(--lc-text);
  margin-bottom: 2px;
  padding: 0 2px;
}

.cal-items {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.cal-item {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 4px;
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.12s;
}

.cal-item:hover {
  background: var(--el-fill-color);
}

.cal-item.is-selected {
  background: var(--el-color-primary-light-8);
}

.cal-item.is-overdue {
  background: var(--el-color-danger-light-9);
}

.cal-item-priority {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

.cal-item-priority.priority-p0 {
  background: var(--el-color-danger);
}

.cal-item-priority.priority-p1 {
  background: var(--el-color-warning);
}

.cal-item-priority.priority-p2 {
  background: var(--el-color-primary);
}

.cal-item-priority.priority-p3 {
  background: var(--lc-text-muted);
}

.cal-item-title {
  font-size: 11px;
  line-height: 1.3;
  color: var(--lc-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cal-item-overflow {
  font-size: 10px;
  color: var(--lc-text-muted);
  padding: 0 4px;
}
</style>

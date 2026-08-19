<template>
  <div class="todo-sidebar-content">
    <div class="stats-section">
      <div class="stats-section-header">
        <div class="stats-section-title">概览</div>
        <el-button
          size="small"
          link
          type="primary"
          class="clear-btn overview-settings-btn"
          title="基础数据设置"
          aria-label="基础数据设置"
          @click="emit('openBasics')"
        >
          <el-icon><Setting /></el-icon>
          <span>基础数据</span>
        </el-button>
      </div>
      <div class="stats-grid">
        <div class="stat-card">
          <div class="stat-number">{{ activeCount }}</div>
          <div class="stat-label">任务</div>
        </div>
        <div class="stat-card">
          <div class="stat-number">{{ doneCount }}</div>
          <div class="stat-label">已完成</div>
        </div>
        <div class="stat-card">
          <div class="stat-number">{{ todayDueCount }}</div>
          <div class="stat-label">今日到期</div>
        </div>
        <div class="stat-card" :class="{ 'is-alert': overdueCount > 0 }">
          <div class="stat-number">{{ overdueCount }}</div>
          <div class="stat-label">逾期</div>
        </div>
      </div>
    </div>
    <div v-if="typeDistribution.length > 0" class="stats-section">
      <div class="stats-section-header">
        <div class="stats-section-title">分类分布</div>
        <el-button
          size="small"
          link
          type="primary"
          class="clear-btn"
          :disabled="filterType === null"
          @click="emit('update:filterType', null)"
        >
          清空
        </el-button>
      </div>
      <div class="stats-bar-list">
        <div
          v-for="entry in typeDistribution"
          :key="entry.name"
          class="stats-bar-item is-clickable"
          :class="{ 'is-active': filterType === entry.name }"
          @click="emit('update:filterType', filterType === entry.name ? null : entry.name)"
        >
          <div class="stats-bar-label">
            <span class="color-dot" :style="{ backgroundColor: entry.color }" />
            <span>{{ entry.name }}</span>
            <span class="stats-bar-count">{{ entry.count }}</span>
          </div>
          <div class="stats-bar-track">
            <div
              class="stats-bar-fill"
              :style="{
                width: statsBarWidth(entry.count, typeDistribution),
                backgroundColor: entry.color,
              }"
            />
          </div>
        </div>
      </div>
    </div>
    <div v-if="priorityDistribution.length > 0" class="stats-section">
      <div class="stats-section-header">
        <div class="stats-section-title">优先级分布</div>
        <el-button
          size="small"
          link
          type="primary"
          class="clear-btn"
          :disabled="filterPriority === null"
          @click="emit('update:filterPriority', null)"
        >
          清空
        </el-button>
      </div>
      <div class="stats-bar-list">
        <div
          v-for="entry in priorityDistribution"
          :key="entry.priority"
          class="stats-bar-item is-clickable"
          :class="{ 'is-active': filterPriority === entry.priority }"
          @click="
            emit('update:filterPriority', filterPriority === entry.priority ? null : entry.priority)
          "
        >
          <div class="stats-bar-label">
            <span class="priority-dot" :class="'priority-' + entry.priority.toLowerCase()" />
            <span>{{ entry.priority }}</span>
            <span class="stats-bar-count">{{ entry.count }}</span>
          </div>
          <div class="stats-bar-track">
            <div
              class="stats-bar-fill"
              :class="'priority-bar-' + entry.priority.toLowerCase()"
              :style="{ width: statsBarWidth(entry.count, priorityDistribution) }"
            />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { Setting } from "@element-plus/icons-vue";
import type { TodoItem, TodoPriority } from "../../types";
import { getTodayDateString } from "../../utils/todoSchedule";

const props = defineProps<{
  activeItems: TodoItem[];
  recentWeekItems: TodoItem[];
  doneItems: TodoItem[];
  filterType: string | null;
  filterPriority: TodoPriority | null;
}>();

const emit = defineEmits<{
  openBasics: [];
  "update:filterType": [value: string | null];
  "update:filterPriority": [value: TodoPriority | null];
}>();

const activeCount = computed(() => props.activeItems.length);
const doneCount = computed(() => props.doneItems.length + props.recentWeekItems.length);

const todayDueCount = computed(() => {
  const today = getTodayDateString();
  return props.activeItems.filter((item) => {
    const time = item.eventAt;
    return time && time.startsWith(today);
  }).length;
});

const overdueCount = computed(() => {
  return props.activeItems.filter((item) => {
    const time = item.eventAt;
    if (!time) return false;
    const actionable = item.status === "pending" || item.status === "in_progress";
    if (!actionable) return false;
    return new Date(time).getTime() < Date.now();
  }).length;
});

const typeDistribution = computed(() => {
  const map = new Map<string, { name: string; color: string; count: number }>();
  for (const item of props.activeItems) {
    const name = item.typeName || "未分类";
    const existing = map.get(name);
    if (existing) {
      existing.count++;
    } else {
      map.set(name, { name, color: item.typeColor || "#909399", count: 1 });
    }
  }
  return [...map.values()].sort((a, b) => b.count - a.count);
});

const priorityDistribution = computed(() => {
  const counts: Record<string, number> = { P0: 0, P1: 0, P2: 0, P3: 0 };
  for (const item of props.activeItems) {
    if (counts[item.priority] !== undefined) counts[item.priority]++;
  }
  return (["P0", "P1", "P2", "P3"] as const)
    .map((p) => ({ priority: p, count: counts[p] }))
    .filter((entry) => entry.count > 0);
});

function statsBarWidth(count: number, list: { count: number }[]) {
  const max = Math.max(...list.map((i) => i.count), 1);
  return Math.round((count / max) * 100) + "%";
}
</script>

<style scoped>
.todo-sidebar-content {
  display: flex;
  flex-direction: column;
  gap: 20px;
}
.stats-section {
  background: var(--lc-surface-1);
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-sm);
  padding: 14px;
}
.stats-section-title {
  font-family: var(--lc-font-display);
  font-size: 11px;
  font-weight: 700;
  color: var(--lc-text-muted);
  margin-bottom: 10px;
  text-transform: uppercase;
  letter-spacing: 0.8px;
}
.stats-section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 10px;
}
.stats-section-header .clear-btn {
  opacity: 0;
  transition: opacity 0.2s ease;
}
.stats-section:hover .clear-btn {
  opacity: 1;
}
.stats-section-header .stats-section-title {
  margin-bottom: 0;
}
.overview-settings-btn {
  padding: 0;
  gap: 4px;
  font-weight: 500;
}
.stats-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}
.stat-card {
  text-align: center;
  padding: 10px 8px 8px;
  background: var(--lc-surface-2);
  border-radius: 6px;
  border: 1px solid var(--lc-border-subtle);
}
.stat-number {
  font-family: var(--lc-font-display);
  font-size: 20px;
  font-weight: 700;
  color: var(--lc-text);
  line-height: 1.1;
}
.stat-label {
  font-family: var(--lc-font-body);
  font-size: 11px;
  color: var(--lc-text-muted);
  margin-top: 2px;
  letter-spacing: 0.3px;
}
.stat-card.is-alert .stat-number {
  color: var(--lc-danger);
}
.stats-bar-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.stats-bar-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.stats-bar-item.is-clickable {
  cursor: pointer;
  border-radius: 6px;
  padding: 4px 8px;
  margin: -4px -8px;
  transition: background-color 0.15s ease;
}
.stats-bar-item.is-clickable:hover {
  background-color: var(--el-fill-color-light);
}
.stats-bar-item.is-active {
  background-color: var(--lc-accent-dim);
  border-left: 3px solid var(--lc-accent);
  padding-left: 5px;
}
.stats-bar-label {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 13px;
  color: var(--lc-text);
}
.stats-bar-count {
  margin-left: auto;
  font-family: var(--lc-font-display);
  font-size: 12px;
  font-weight: 600;
  color: var(--lc-text-secondary);
}
.stats-bar-track {
  height: 5px;
  background: var(--lc-surface-3);
  border-radius: 3px;
  overflow: hidden;
}
.stats-bar-fill {
  height: 100%;
  border-radius: 3px;
  transition: width 0.35s var(--lc-ease);
}
.priority-bar-p0 {
  background-color: var(--lc-danger);
}
.priority-bar-p1 {
  background-color: var(--lc-warning);
}
.priority-bar-p2 {
  background-color: var(--lc-accent);
}
.priority-bar-p3 {
  background-color: var(--lc-text-muted);
}
.color-dot {
  display: inline-block;
  width: 10px;
  height: 10px;
  margin-right: 6px;
  border-radius: 50%;
  vertical-align: middle;
}
.priority-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-right: 6px;
}
</style>

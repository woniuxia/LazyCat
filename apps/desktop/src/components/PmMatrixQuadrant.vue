<template>
  <section
    class="matrix-quadrant"
    :style="{ '--quadrant-accent': accentColor }"
  >
    <header class="quadrant-head">
      <span class="quadrant-roman">{{ roman }}</span>
      <span class="quadrant-title">{{ title }}</span>
      <span class="quadrant-count">{{ items.length }}</span>
    </header>
    <div class="quadrant-body">
      <div v-if="items.length === 0" class="quadrant-empty">
        {{ emptyText }}
      </div>
      <div
        v-for="item in items"
        :key="item.id"
        class="quadrant-card"
        :class="{
          'is-selected': selectedItemId === item.id,
          'is-done': item.status === 'done',
          'is-overdue': overdueMap[item.id],
        }"
        :style="{ borderLeftColor: item.projectColor || '#0ea5e9' }"
        @click="emit('select', item)"
        @dblclick="emit('edit', item)"
        @contextmenu.prevent="(e: MouseEvent) => emit('context', e, item)"
      >
        <div class="card-title">{{ item.title }}</div>
        <div class="card-meta">
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
          <span v-if="dueText(item)" class="card-due" :class="{ 'is-overdue': overdueMap[item.id] }">
            {{ dueText(item) }}
          </span>
        </div>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed } from "vue";
import type { PmItem } from "../types/pm";
import { PM_PRIORITY_MAP } from "../types/pm";
import { isPmItemOverdue, parsePmDateAtLocalStart } from "../utils/pmDate";

const props = defineProps<{
  title: string;
  roman: string;
  accentColor: string;
  items: PmItem[];
  selectedItemId: number | null;
  emptyText: string;
}>();

const emit = defineEmits<{
  (e: "select", item: PmItem): void;
  (e: "edit", item: PmItem): void;
  (e: "context", event: MouseEvent, item: PmItem): void;
}>();

const overdueMap = computed<Record<number, boolean>>(() => {
  const map: Record<number, boolean> = {};
  for (const item of props.items) {
    map[item.id] = isPmItemOverdue(item);
  }
  return map;
});

function endDate(item: PmItem): string | null {
  if (!item.endAt) return null;
  return item.endAt.length >= 10 ? item.endAt.slice(0, 10) : null;
}

function dueText(item: PmItem): string {
  const end = endDate(item);
  if (!end) return "";
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const endDateObj = parsePmDateAtLocalStart(end);
  if (!endDateObj) return "";
  const diff = Math.round((endDateObj.getTime() - today.getTime()) / 86400000);
  if (diff === 0) return "今天";
  if (diff === 1) return "明天";
  if (diff < 0) return `逾期 ${-diff} 天`;
  if (diff <= 7) return `${diff} 天后`;
  return end.slice(5);
}
</script>

<style scoped>
.matrix-quadrant {
  background: var(--el-bg-color);
  border: 1px solid var(--pm-edge-soft, #e4e7ed);
  border-radius: 10px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-height: 0;
}

.quadrant-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--pm-edge-soft, #e4e7ed);
  background: var(--el-fill-color-lighter);
}
.quadrant-roman {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 22px;
  height: 22px;
  padding: 0 6px;
  border-radius: 4px;
  background: var(--quadrant-accent);
  color: #fff;
  font-size: 11px;
  font-weight: 700;
}
.quadrant-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}
.quadrant-count {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  background: var(--el-fill-color-light);
  border-radius: 9px;
  min-width: 22px;
  padding: 1px 8px;
  text-align: center;
  margin-left: auto;
}

.quadrant-body {
  flex: 1;
  overflow-y: auto;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.quadrant-empty {
  font-size: 12px;
  color: var(--el-text-color-placeholder);
  text-align: center;
  padding: 16px 4px;
  border: 1px dashed var(--pm-edge-soft, #e4e7ed);
  border-radius: 6px;
}

.quadrant-card {
  background: var(--el-bg-color-page, #fafbfc);
  border: 1px solid var(--pm-edge-soft, #e4e7ed);
  border-left: 3px solid #dcdfe6;
  border-radius: 6px;
  padding: 8px 10px;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 6px;
  transition: border-color 0.18s, box-shadow 0.18s;
}
.quadrant-card:hover {
  border-color: var(--el-color-primary-light-5);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.04);
}
.quadrant-card.is-selected {
  border-color: var(--el-color-primary);
  box-shadow: 0 0 0 1px var(--el-color-primary) inset;
}
.quadrant-card.is-done {
  opacity: 0.6;
}
.quadrant-card.is-done .card-title {
  text-decoration: line-through;
}

.card-title {
  font-size: 13px;
  color: var(--el-text-color-primary);
  font-weight: 500;
  word-break: break-word;
}

.card-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
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
}
.card-due {
  font-size: 11px;
  color: var(--el-text-color-secondary);
  background: var(--el-fill-color-light);
  padding: 0 6px;
  border-radius: 4px;
}
.card-due.is-overdue {
  color: #f56c6c;
  background: rgba(245, 108, 108, 0.08);
}
</style>

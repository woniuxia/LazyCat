<template>
  <div class="gantt-container">
    <div v-if="ganttTasks.length === 0" class="gantt-empty">
      <el-empty description="没有设置日期的工作项，无法显示甘特图" />
    </div>
    <template v-else>
      <div class="gantt-toolbar">
        <el-radio-group v-model="ganttViewMode" size="small" @change="changeViewMode">
          <el-radio-button value="Day">日</el-radio-button>
          <el-radio-button value="Week">周</el-radio-button>
          <el-radio-button value="Month">月</el-radio-button>
        </el-radio-group>
      </div>
      <div ref="ganttRef" class="gantt-wrapper" />
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, nextTick, onBeforeUnmount } from "vue";
import Gantt from "frappe-gantt";
import "frappe-gantt/dist/frappe-gantt.css";
import type { PmItem } from "../types/pm";

const props = defineProps<{
  items: PmItem[];
}>();

const emit = defineEmits<{
  (e: "select", item: PmItem): void;
  (e: "date-change", item: PmItem, start: string, end: string): void;
}>();

const ganttRef = ref<HTMLElement | null>(null);
const ganttViewMode = ref("Day");
let ganttInstance: Gantt | null = null;
let isDragging = false;
let dragTimer: ReturnType<typeof setTimeout> | null = null;
let skipNextRefresh = false;

const ganttTasks = computed(() => {
  return props.items
    .filter((i) => i.startAt || i.endAt)
    .map((item) => {
      const start = item.startAt || item.endAt || item.createdAt;
      const end = item.endAt || item.startAt || item.createdAt;
      return {
        id: String(item.id),
        name: item.title,
        start: start!.slice(0, 10),
        end: end!.slice(0, 10),
        progress: item.status === "done" ? 100 : item.status === "testing" ? 75 : item.status === "in_progress" ? 40 : 0,
        custom_class: `gantt-${item.priority.toLowerCase()}`,
      };
    });
});

function formatDate(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function renderGantt() {
  if (!ganttRef.value || ganttTasks.value.length === 0) return;
  ganttRef.value.innerHTML = "";

  ganttInstance = new Gantt(ganttRef.value, ganttTasks.value, {
    view_mode: ganttViewMode.value,
    date_format: "YYYY-MM-DD",
    language: "zh",
    on_click: (task: { id: string }) => {
      if (isDragging) return;
      const found = props.items.find((i) => String(i.id) === task.id);
      if (found) emit("select", found);
    },
    on_date_change: (task: { id: string }, start: Date, end: Date) => {
      isDragging = true;
      if (dragTimer) clearTimeout(dragTimer);
      dragTimer = setTimeout(() => {
        isDragging = false;
        const found = props.items.find((i) => String(i.id) === task.id);
        if (found) {
          skipNextRefresh = true;
          emit("date-change", found, formatDate(start), formatDate(end));
        }
      }, 300);
    },
  });
}

function changeViewMode(mode: string) {
  if (ganttInstance) {
    ganttInstance.change_view_mode(mode);
  }
}

watch(
  () => ganttTasks.value,
  () => {
    if (isDragging) return;
    if (skipNextRefresh) {
      skipNextRefresh = false;
      return;
    }
    nextTick(() => {
      if (ganttInstance) {
        const wrapper = ganttRef.value;
        const scrollLeft = wrapper?.scrollLeft ?? 0;
        const scrollTop = wrapper?.scrollTop ?? 0;
        ganttInstance.refresh(ganttTasks.value);
        if (wrapper) {
          wrapper.scrollLeft = scrollLeft;
          wrapper.scrollTop = scrollTop;
        }
      } else {
        renderGantt();
      }
    });
  },
  { deep: true }
);

onMounted(() => nextTick(renderGantt));

onBeforeUnmount(() => {
  if (dragTimer) clearTimeout(dragTimer);
  ganttInstance = null;
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
  flex-shrink: 0;
  border-bottom: 1px solid var(--el-border-color-extra-light);
}
.gantt-wrapper {
  flex: 1;
  overflow: auto;
  min-width: 600px;
}
.gantt-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
}
</style>

<style>
/* Gantt priority colors */
.gantt-p0 .bar {
  fill: #f56c6c !important;
}
.gantt-p1 .bar {
  fill: #e6a23c !important;
}
.gantt-p2 .bar {
  fill: #409eff !important;
}
.gantt-p3 .bar {
  fill: #909399 !important;
}
.gantt .bar-label {
  font-size: 12px;
}
.gantt .grid-header {
  fill: var(--el-fill-color-lighter);
}
</style>

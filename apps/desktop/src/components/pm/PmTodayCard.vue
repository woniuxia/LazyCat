<template>
  <div
    class="pm-today-card"
    :class="{
      'is-selected': selected,
      'is-pinned': item.pinned,
      'is-done': item.status === 'done',
      ['is-' + item.priority.toLowerCase()]: true,
    }"
    :style="{ borderLeftColor: PM_PRIORITY_MAP[item.priority]?.color }"
  >
    <div class="card-body">
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
          <span class="card-project-name">{{ item.projectName }}</span>
        </span>
        <span v-if="item.pinned" class="card-flag" title="已置顶">📌</span>
      </div>
      <div class="card-title" :title="cardTitleTooltip">{{ item.title }}</div>
      <span
        v-if="dateChipText"
        class="card-date-chip"
        :class="{ 'is-overdue': overdue }"
      >
        {{ dateChipText }}
      </span>
    </div>
    <div class="card-actions">
      <button
        v-if="showStart"
        class="card-action"
        @click.stop="emit('start', item)"
      >
        开始做
      </button>
      <button
        v-if="showPostpone"
        class="card-action"
        @click.stop="emit('postpone', item)"
      >
        推到明天
      </button>
      <button
        v-if="showComplete"
        class="card-action is-primary"
        @click.stop="emit('complete', item)"
      >
        标记完成
      </button>
      <button
        class="card-action"
        @click.stop="emit('detail', item)"
      >
        详情
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { PM_PRIORITY_MAP, type PmItem } from "../../types/pm";
import {
  formatPmDateRangeForDisplay,
  hasPmDateSchedule,
  isPmItemOverdue,
} from "../../utils/pmDate";

const props = defineProps<{
  item: PmItem;
  selected: boolean;
}>();

const emit = defineEmits<{
  (e: "start", item: PmItem): void;
  (e: "postpone", item: PmItem): void;
  (e: "complete", item: PmItem): void;
  (e: "detail", item: PmItem): void;
}>();

const overdue = computed(() => isPmItemOverdue(props.item));

const dateChipText = computed(() => {
  if (!hasPmDateSchedule(props.item.startAt, props.item.endAt)) return "";
  return formatPmDateRangeForDisplay(props.item.startAt, props.item.endAt, {
    mode: "short",
    emptyText: "",
  });
});

const cardTitleTooltip = computed(() => {
  const desc = (props.item.description ?? "").trim();
  return desc ? `${props.item.title}\n\n${desc}` : props.item.title;
});

const showStart = computed(() => props.item.status === "todo");
const showPostpone = computed(() => props.item.status !== "done" && Boolean(props.item.endAt));
const showComplete = computed(() => props.item.status !== "done");
</script>

<style scoped>
.pm-today-card {
  position: relative;
  background: var(--el-bg-color-page, #fafbfc);
  border: 1px solid var(--pm-edge-soft, #e4e7ed);
  border-left: 3px solid #dcdfe6;
  border-radius: 8px;
  padding: 6px 12px;
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: 12px;
  cursor: pointer;
  transition: border-color 0.18s, box-shadow 0.18s, transform 0.18s;
}

.pm-today-card:hover {
  border-color: var(--el-color-primary-light-7, #c6e2ff);
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.04);
}

.pm-today-card.is-selected {
  border-color: var(--el-color-primary, #409eff);
  box-shadow: 0 0 0 1px var(--el-color-primary, #409eff) inset;
}

.pm-today-card.is-done {
  opacity: 0.72;
}
.pm-today-card.is-done .card-title {
  text-decoration: line-through;
  color: var(--el-text-color-placeholder, #a8abb2);
}

.card-body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: 10px;
}

.card-meta {
  flex: none;
  display: flex;
  flex-wrap: nowrap;
  align-items: center;
  gap: 6px;
}

.card-project-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 1px 8px;
  border-radius: 10px;
  font-size: 11px;
  line-height: 1.5;
}
.card-project-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}
.card-project-name {
  max-width: 120px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.card-date-chip {
  flex: none;
  font-size: 11px;
  line-height: 1.5;
  color: var(--el-text-color-secondary, #909399);
  background: var(--el-fill-color-light, #f5f7fa);
  padding: 0 6px;
  border-radius: 4px;
  white-space: nowrap;
}
.card-date-chip.is-overdue {
  color: #f56c6c;
  background: rgba(245, 108, 108, 0.08);
}

.card-flag {
  font-size: 12px;
  line-height: 1;
}

.card-title {
  flex: 0 1 auto;
  min-width: 0;
  font-size: 13px;
  font-weight: 500;
  color: var(--el-text-color-primary, #303133);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.card-actions {
  flex: none;
  display: flex;
  flex-wrap: nowrap;
  gap: 4px;
}

.card-action {
  appearance: none;
  background: var(--el-bg-color, #fff);
  border: 1px solid var(--el-border-color, #dcdfe6);
  color: var(--el-text-color-regular, #606266);
  font-size: 11px;
  line-height: 1.5;
  padding: 2px 8px;
  border-radius: 4px;
  cursor: pointer;
  transition: background-color 0.15s, border-color 0.15s, color 0.15s;
  white-space: nowrap;
}
.card-action:hover {
  background: var(--el-fill-color-light, #f5f7fa);
  border-color: var(--el-color-primary-light-5, #a0cfff);
  color: var(--el-color-primary, #409eff);
}
.card-action.is-primary {
  background: var(--el-color-primary, #409eff);
  border-color: var(--el-color-primary, #409eff);
  color: #fff;
}
.card-action.is-primary:hover {
  background: var(--el-color-primary-light-3, #79bbff);
  border-color: var(--el-color-primary-light-3, #79bbff);
  color: #fff;
}
</style>

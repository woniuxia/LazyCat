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
            backgroundColor: (item.projectColor || '#4d7df2') + '18',
            color: item.projectColor || '#4d7df2',
          }"
        >
          <span
            class="card-project-dot"
            :style="{ backgroundColor: item.projectColor || '#4d7df2' }"
          />
          <span class="card-project-name">{{ item.projectName }}</span>
        </span>
        <span
          class="card-pill"
          :style="{
            color: PM_PRIORITY_MAP[item.priority]?.color,
            borderColor: PM_PRIORITY_MAP[item.priority]?.color + '40',
          }"
        >
          {{ PM_PRIORITY_MAP[item.priority]?.label }}
        </span>
        <span
          class="card-pill"
          :style="{
            color: PM_ITEM_TYPE_MAP[item.itemType]?.color,
            borderColor: PM_ITEM_TYPE_MAP[item.itemType]?.color + '40',
          }"
        >
          {{ PM_ITEM_TYPE_MAP[item.itemType]?.label }}
        </span>
        <span
          class="card-pill"
          :style="{
            color: statusMeta.color,
            borderColor: statusMeta.color + '40',
          }"
        >
          {{ statusMeta.label }}
        </span>
        <span
          v-if="dateChipText"
          class="card-date-chip"
          :class="{ 'is-overdue': overdue }"
        >
          {{ dateChipText }}
        </span>
        <span v-if="item.pinned" class="card-flag" title="已置顶">📌</span>
      </div>
      <div class="card-title">{{ item.title }}</div>
      <div v-if="item.description" class="card-desc">
        {{ truncatedDesc }}
      </div>
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
import { PM_ITEM_TYPE_MAP, PM_PRIORITY_MAP, PM_STATUS_COLUMNS, type PmItem } from "../types/pm";
import {
  formatPmDateRangeForDisplay,
  hasPmDateSchedule,
  isPmItemOverdue,
} from "../utils/pmDate";

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

const statusMeta = computed(() => {
  return (
    PM_STATUS_COLUMNS.find((c) => c.key === props.item.status) ?? {
      key: props.item.status,
      label: props.item.status,
      color: "#909399",
    }
  );
});

const dateChipText = computed(() => {
  if (!hasPmDateSchedule(props.item.startAt, props.item.endAt)) return "";
  return formatPmDateRangeForDisplay(props.item.startAt, props.item.endAt, {
    mode: "short",
    emptyText: "",
  });
});

const truncatedDesc = computed(() => {
  const text = props.item.description ?? "";
  if (text.length <= 80) return text;
  return text.slice(0, 80) + "…";
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
  padding: 10px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
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
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.card-meta {
  display: flex;
  flex-wrap: wrap;
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

.card-pill {
  font-size: 11px;
  line-height: 1.5;
  padding: 0 6px;
  border: 1px solid;
  border-radius: 4px;
  background: transparent;
}

.card-date-chip {
  font-size: 11px;
  line-height: 1.5;
  color: var(--el-text-color-secondary, #909399);
  background: var(--el-fill-color-light, #f5f7fa);
  padding: 0 6px;
  border-radius: 4px;
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
  font-size: 14px;
  font-weight: 500;
  color: var(--el-text-color-primary, #303133);
  word-break: break-word;
}

.card-desc {
  font-size: 12px;
  color: var(--el-text-color-regular, #606266);
  line-height: 1.5;
  word-break: break-word;
}

.card-actions {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}

.card-action {
  appearance: none;
  background: var(--el-bg-color, #fff);
  border: 1px solid var(--el-border-color, #dcdfe6);
  color: var(--el-text-color-regular, #606266);
  font-size: 12px;
  line-height: 1.5;
  padding: 2px 10px;
  border-radius: 4px;
  cursor: pointer;
  transition: background-color 0.15s, border-color 0.15s, color 0.15s;
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

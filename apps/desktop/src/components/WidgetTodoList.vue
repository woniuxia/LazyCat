<template>
  <!-- 待办列表（design §5.2 + 挂件交互版）：每行可点 checkbox 完成。 -->
  <section class="todo-list">
    <div class="list-header">
      <span class="header-title">待办事项</span>
      <button class="header-add" title="新建待办" @click="$emit('action', { kind: 'open-todo-create' })">+ 新建</button>
    </div>
    <div
      v-for="item in items"
      :key="item.id"
      class="todo-row"
      :class="`p-${item.priority.toLowerCase()}`"
    >
      <button
        class="check"
        :class="`p-${item.priority.toLowerCase()}`"
        :title="`完成 · ${displayTitle(item.title)}`"
        @click="onComplete(item)"
      ></button>
      <span v-if="item.pinned" class="pin">📌</span>
      <span class="title">{{ displayTitle(item.title) }}</span>
      <span class="deadline" :class="{ overdue: isOverdue(item) }">
        {{ formatDeadline(item.endAt) }}
      </span>
    </div>
    <div v-if="items.length === 0" class="empty">今日无待办</div>
  </section>
</template>

<script setup lang="ts">
import type { WidgetTodoItem } from "../types/widget";

const props = defineProps<{
  items: WidgetTodoItem[];
  /** design §9：开启敏感模式时把 title 替换为 ▓ */
  privacyMask?: boolean;
}>();

const emit = defineEmits<{
  (e: "complete", item: WidgetTodoItem): void;
  (e: "action", payload: { kind: string; [key: string]: unknown }): void;
}>();

function displayTitle(title: string): string {
  if (!props.privacyMask) return title;
  const len = Math.max(4, Math.min(title.length, 8));
  return "▓".repeat(len);
}

function onComplete(item: WidgetTodoItem) {
  emit("complete", item);
}

function isOverdue(item: WidgetTodoItem): boolean {
  if (!item.endAt) return false;
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const due = parseLocalDate(item.endAt);
  return due !== null && due.getTime() < today.getTime();
}

function formatDeadline(endAt: string | null): string {
  if (!endAt) return "";
  const due = parseLocalDate(endAt);
  if (!due) return "";

  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const diffDays = Math.round((due.getTime() - today.getTime()) / 86400000);

  if (diffDays < 0) return `已逾期 ${-diffDays} 天`;
  if (diffDays === 0) return "今天";
  if (diffDays === 1) return "明天";
  return `${due.getMonth() + 1}月${due.getDate()}日`;
}

function parseLocalDate(raw: string): Date | null {
  if (!raw || raw.length < 10) return null;
  const prefix = raw.slice(0, 10);
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(prefix);
  if (!m) return null;
  const year = Number(m[1]);
  const month = Number(m[2]);
  const day = Number(m[3]);
  if (!year || !month || !day) return null;
  return new Date(year, month - 1, day);
}
</script>

<style scoped>
.todo-list {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 0;
  border-radius: 12px;
  background: var(--wc-block-bg);
  border: 1px solid var(--wc-block-border);
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--wc-bg-tertiary) transparent;
}

.todo-list::-webkit-scrollbar {
  width: 6px;
}

.todo-list::-webkit-scrollbar-thumb {
  background: var(--wc-bg-tertiary);
  border-radius: 3px;
}

.todo-list::-webkit-scrollbar-thumb:hover {
  background: var(--wc-text-muted);
}

.todo-list::-webkit-scrollbar-track {
  background: transparent;
}

.list-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border-bottom: 1px solid var(--wc-divider);
  flex-shrink: 0;
}

.header-title {
  font-size: 12px;
  color: var(--wc-text-muted);
}

.header-add {
  padding: 4px 9px;
  border-radius: 5px;
  background: var(--wc-block-bg);
  border: 1px solid var(--wc-block-border);
  color: var(--wc-text);
  font-size: 11px;
  cursor: pointer;
  font-family: inherit;
  transition: background-color 0.15s ease;
}

.header-add:hover {
  background: var(--wc-block-border);
}

.todo-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  font-size: 13px;
  line-height: 28px;
  border-bottom: 1px solid var(--wc-divider);
}

.todo-row:last-of-type {
  border-bottom: none;
}

/* checkbox 用 button 实现：方形圆角 + 优先级描边色 + hover 填色 */
.check {
  width: 16px;
  height: 16px;
  border-radius: 4px;
  border: 2px solid currentColor;
  background: transparent;
  cursor: pointer;
  flex-shrink: 0;
  padding: 0;
  transition: background-color 0.15s ease;
}
.check:hover {
  background: currentColor;
}
.check:active {
  transform: scale(0.92);
}

.check.p-p0 {
  color: #ef4444;
}
.check.p-p1 {
  color: #f59e0b;
}
.check.p-p2 {
  color: #3b82f6;
}
.check.p-p3 {
  color: #94a3b8;
}

.pin {
  font-size: 12px;
}

.title {
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--wc-text);
}

.deadline {
  font-size: 11px;
  color: var(--wc-text-muted);
  flex-shrink: 0;
}

.deadline.overdue {
  color: #ef4444;
  font-weight: 600;
}

.empty {
  margin: auto;
  padding-bottom: 12px;
  font-size: 13px;
  color: var(--wc-text-muted);
}
</style>

<template>
  <!-- 待办列表：每行可点 checkbox 完成 -->
  <section class="todo-list" :class="{ 'is-scrollable': scrollable }">
    <div class="list-header">
      <span class="header-title">待办事项</span>
      <button class="header-add" title="新建待办" @click="$emit('action', { kind: 'open-todo-create' })">+ 新建</button>
    </div>
    <div v-show="scrollable && showTopShadow" class="scroll-shadow scroll-shadow-top" />
    <div class="list-body" ref="listBodyRef" @scroll="onScroll">
      <div
        v-for="(item, idx) in items"
        :key="item.id"
        class="todo-row"
        :class="[`p-${item.priority.toLowerCase()}`, { 'is-overdue': isOverdue(item) }]"
        :style="{ animationDelay: `${Math.min(idx * 30, 300)}ms` }"
      >
        <button
          class="check"
          :class="`p-${item.priority.toLowerCase()}`"
          :title="`完成 · ${displayTitle(item.title)}`"
          @click="onComplete(item)"
        >
          <svg class="check-icon" viewBox="0 0 12 12" fill="none">
            <path d="M2.5 6L5 8.5L9.5 3.5" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </button>
        <svg v-if="item.pinned" class="pin-icon" viewBox="0 0 16 16" fill="none" aria-label="已置顶">
          <path d="M9.5 2L10.5 3L10 3.5L9.5 3L7 5.5V9L10 12V14H6V12L9 9V5.5L6.5 3L6 3.5L5.5 3L6.5 2L8 3.5L9.5 2Z" fill="currentColor"/>
          <line x1="10" y1="12" x2="10" y2="14" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
        </svg>
        <span class="title">{{ displayTitle(item.title) }}</span>
        <span class="deadline" :class="{ overdue: isOverdue(item) }">
          <svg v-if="isOverdue(item)" class="overdue-icon" viewBox="0 0 14 14" fill="none" aria-label="已逾期">
            <circle cx="7" cy="7" r="5.5" stroke="currentColor" stroke-width="1.2"/>
            <path d="M7 4V7.5L9 9" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          {{ formatDeadline(item.endAt) }}
        </span>
      </div>
      <div v-if="items.length === 0" class="empty">
        <div class="empty-illustration">
          <svg viewBox="0 0 48 48" fill="none">
            <circle cx="24" cy="24" r="22" stroke="currentColor" stroke-width="1.2" opacity="0.25"/>
            <path d="M16 22L21.5 27.5L32 16.5" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" opacity="0.55"/>
          </svg>
        </div>
        <span class="empty-title">今日无待办</span>
        <span class="empty-desc">所有事项已处理完毕</span>
        <button class="empty-action" @click="$emit('action', { kind: 'open-todo-create' })">+ 新建待办</button>
      </div>
    </div>
    <div v-show="scrollable && showScrollShadow" class="scroll-shadow scroll-shadow-btm" />
  </section>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from "vue";
import type { WidgetTodoItem } from "../types/widget";

const props = defineProps<{
  items: WidgetTodoItem[];
  /** 开启敏感模式时把 title 替换为 ▓ */
  privacyMask?: boolean;
}>();

const emit = defineEmits<{
  (e: "complete", item: WidgetTodoItem): void;
  (e: "action", payload: { kind: string; [key: string]: unknown }): void;
}>();

const listBodyRef = ref<HTMLElement | null>(null);
const scrollable = ref(false);
const showTopShadow = ref(false);
const showScrollShadow = ref(false);

let resizeObserver: ResizeObserver | null = null;

function checkScrollable() {
  const el = listBodyRef.value;
  if (!el) return;
  scrollable.value = el.scrollHeight > el.clientHeight;
}

function onScroll() {
  const el = listBodyRef.value;
  if (!el) return;
  const atTop = el.scrollTop <= 1;
  const atBottom = el.scrollTop + el.clientHeight >= el.scrollHeight - 2;
  showTopShadow.value = !atTop && scrollable.value;
  showScrollShadow.value = !atBottom && scrollable.value;
}

onMounted(() => {
  if (listBodyRef.value) {
    resizeObserver = new ResizeObserver(() => {
      checkScrollable();
      onScroll();
    });
    resizeObserver.observe(listBodyRef.value);
  }
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  resizeObserver = null;
});

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
  min-height: 0;
  border-radius: 12px;
  background: var(--wc-block-bg);
  border: 1px solid var(--wc-block-border);
  overflow: hidden;
  position: relative;
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
  font-weight: 500;
  letter-spacing: 0.02em;
  color: var(--wc-text-muted);
}

.header-add {
  padding: 4px 9px;
  border-radius: 5px;
  background-color: var(--wc-block-bg);
  background-image: linear-gradient(135deg, #6366f1, #a855f7);
  -webkit-background-clip: text;
  background-clip: text;
  border: 1px solid var(--wc-block-border);
  color: transparent;
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  font-family: inherit;
  transition: background-color 0.15s ease;
}

.header-add:hover {
  background-color: var(--wc-block-border);
}

.header-add:active {
  transform: scale(0.96);
}

.list-body {
  flex: 1;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: var(--wc-bg-tertiary) transparent;
}

.list-body::-webkit-scrollbar {
  width: 4px;
}

.list-body::-webkit-scrollbar-thumb {
  background: var(--wc-bg-tertiary);
  border-radius: 2px;
}

.list-body::-webkit-scrollbar-thumb:hover {
  background: var(--wc-text-muted);
}

.list-body::-webkit-scrollbar-track {
  background: transparent;
}

.todo-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  font-size: 13px;
  line-height: 28px;
  min-height: 44px;
  border-bottom: 1px solid var(--wc-divider);
  border-left: 2.5px solid transparent;
  transition: background-color 0.15s ease;
  animation: row-enter 0.3s ease-out both;
}

.todo-row.is-overdue {
  border-left-color: #ef4444;
  background: rgba(239, 68, 68, 0.05);
}

.todo-row.is-overdue:hover {
  background: rgba(239, 68, 68, 0.08);
}

@keyframes row-enter {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.todo-row:hover {
  background: var(--wc-row-hover);
}

.todo-row:last-of-type {
  border-bottom: none;
}

/* checkbox：方形圆角 + 优先级描边色 + hover 填充 + 勾选图标 */
.check {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  border: 2px solid currentColor;
  background: transparent;
  cursor: pointer;
  flex-shrink: 0;
  padding: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background-color 0.2s ease, transform 0.15s ease, border-color 0.2s ease;
}

.check-icon {
  width: 10px;
  height: 10px;
  opacity: 0;
  transform: scale(0.5);
  transition: opacity 0.2s ease, transform 0.2s ease;
  color: #fff;
}

.check:hover {
  background: currentColor;
}

.check:hover .check-icon {
  opacity: 1;
  transform: scale(1);
}

.check:active {
  transform: scale(0.85);
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

.pin-icon {
  width: 14px;
  height: 14px;
  flex-shrink: 0;
  color: var(--wc-text-muted);
  opacity: 0.7;
}

/* pinned items get a subtle warm tint */
.todo-row:has(.pin-icon) {
  background: var(--wc-pin-bg, rgba(251, 191, 36, 0.04));
}

.todo-row:has(.pin-icon):hover {
  background: var(--wc-pin-bg-hover, rgba(251, 191, 36, 0.1));
}

.title {
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--wc-text);
}

.deadline {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: 11px;
  color: var(--wc-text-muted);
  flex-shrink: 0;
}

.deadline.overdue {
  color: #ef4444;
  font-weight: 600;
}

.overdue-icon {
  width: 12px;
  height: 12px;
  flex-shrink: 0;
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: 32px 12px;
  color: var(--wc-text-muted);
  flex: 1;
  min-height: 0;
}

.empty-illustration {
  width: 48px;
  height: 48px;
  margin-bottom: 8px;
  color: var(--wc-text-muted);
  opacity: 0.5;
}

.empty-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--wc-text);
}

.empty-desc {
  font-size: 12px;
  color: var(--wc-text-muted);
  margin-bottom: 8px;
}

.empty-action {
  padding: 5px 16px;
  border-radius: 14px;
  background: rgba(99, 102, 241, 0.08);
  border: 1px solid rgba(99, 102, 241, 0.12);
  color: var(--wc-accent, #6366f1);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  font-family: inherit;
  transition: background-color 0.15s ease, transform 0.1s ease;
}

.empty-action:hover {
  background: rgba(99, 102, 241, 0.14);
}

.empty-action:active {
  transform: scale(0.96);
}

.scroll-shadow {
  position: absolute;
  left: 0;
  right: 0;
  height: 20px;
  pointer-events: none;
  z-index: 1;
}

.scroll-shadow-top {
  top: 0;
  background: linear-gradient(to bottom, var(--wc-block-bg), transparent);
  border-radius: 12px 12px 0 0;
}

.scroll-shadow-btm {
  bottom: 0;
  background: linear-gradient(to top, var(--wc-block-bg), transparent);
  border-radius: 0 0 12px 12px;
}
</style>

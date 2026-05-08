<template>
  <!-- 待办列表（design §5.2）：自适应行数；超出显示 +N 件 -->
  <section class="todo-list">
    <div v-for="item in visibleItems" :key="item.id" class="todo-row">
      <span class="dot" :class="`p-${item.priority.toLowerCase()}`"></span>
      <span v-if="item.pinned" class="pin">📌</span>
      <span class="title">{{ displayTitle(item.title) }}</span>
      <span class="deadline" :class="{ overdue: isOverdue(item) }">
        {{ formatDeadline(item.endAt) }}
      </span>
    </div>
    <div v-if="overflowCount > 0" class="more">+{{ overflowCount }} 件</div>
    <div v-else-if="visibleItems.length === 0" class="empty">今日无待办</div>
  </section>
</template>

<script setup lang="ts">
import { computed } from "vue";
import type { WallpaperTodoItem } from "../types/wallpaper";

const props = defineProps<{
  items: WallpaperTodoItem[];
  /** design §9：开启敏感模式时把 title 替换为 ▓ */
  privacyMask?: boolean;
}>();

function displayTitle(title: string): string {
  if (!props.privacyMask) return title;
  // 仅打码字符数 ≥ 1 的标题，保持节奏；最少 4 个，最多 8 个
  const len = Math.max(4, Math.min(title.length, 8));
  return "▓".repeat(len);
}

// design §5.2：maxLines = floor((listHeight - paddingY * 2) / lineHeight)
//                       = floor((480 - 32) / 44) = 10
const MAX_LINES = 10;

const visibleItems = computed(() => props.items.slice(0, MAX_LINES));
const overflowCount = computed(() =>
  Math.max(0, props.items.length - MAX_LINES),
);

function isOverdue(item: WallpaperTodoItem): boolean {
  if (!item.endAt) return false;
  // 后端只回 open 项；这里再做一次本地保险：日期 < 今天 = 逾期
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const due = parseLocalDate(item.endAt);
  return due !== null && due.getTime() < today.getTime();
}

/**
 * design §5.2：今天 / 明天 / 5月7日 / 已逾期 N 天
 *
 * 本地日期用 (year, monthIndex, day) 构造，避免 `new Date('YYYY-MM-DD')`
 * 在不同时区出现 UTC 偏移（CLAUDE.md §05.5 时间语义）。
 */
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
  // 截取前 10 字符，统一按 YYYY-MM-DD 解析；非法返回 null
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
  padding: 12px;
  border-radius: 12px;
  background: var(--wc-block-bg);
  border: 1px solid var(--wc-block-border);
  overflow: hidden;
}

.todo-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 4px;
  font-size: 13px;
  line-height: 28px; /* 配合 lineHeight=44 总行高 */
  border-bottom: 1px solid var(--wc-divider);
}

.todo-row:last-of-type {
  border-bottom: none;
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.dot.p-p0 {
  background: #ef4444;
}
.dot.p-p1 {
  background: #f59e0b;
}
.dot.p-p2 {
  background: #3b82f6;
}
.dot.p-p3 {
  background: #94a3b8;
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

.more {
  font-size: 12px;
  color: var(--wc-text-muted);
  text-align: center;
  padding-top: 6px;
}

.empty {
  margin: auto;
  font-size: 13px;
  color: var(--wc-text-muted);
}
</style>

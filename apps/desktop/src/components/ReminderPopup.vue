<template>
  <div class="reminder-popup">
    <header class="popup-header">
      <div class="header-main" data-tauri-drag-region>
        <div class="header-icon">⏰</div>
        <div class="header-copy" data-tauri-drag-region>
          <div class="header-title">任务提醒</div>
          <div class="header-subtitle">
            <span v-if="queue.length > 1">{{ currentIndexLabel }}</span>
            <span v-if="currentReminder">{{ formatFireTime(currentReminder.fireAt) }}</span>
          </div>
        </div>
      </div>
      <button class="close-btn" type="button" @click="closePopup" aria-label="关闭提醒弹窗">
        ×
      </button>
    </header>

    <main class="popup-body">
      <section v-if="currentReminder" class="reminder-card">
        <div class="card-meta">
          <span v-if="queue.length > 1" class="queue-badge">{{ currentIndexLabel }}</span>
        </div>

        <div class="reminder-title-row">
          <span class="priority-badge" :class="priorityClass(currentReminder.priority)">
            {{ currentReminder.priority }}
          </span>
          <h1 class="reminder-title">{{ currentReminder.title }}</h1>
        </div>
        <p v-if="currentReminder.body" class="reminder-body">
          {{ currentReminder.body }}
        </p>

        <div class="reminder-footer">
          <div class="footer-label">提醒时间</div>
          <div class="footer-value">{{ formatFireTime(currentReminder.fireAt) }}</div>
        </div>
      </section>

      <section v-else class="empty-state">
        <div class="empty-icon">✅</div>
        <div class="empty-title">当前没有待处理提醒</div>
        <div class="empty-subtitle">队列清空后会自动关闭弹窗</div>
      </section>
    </main>

    <footer class="popup-actions">
      <button
        class="action-btn action-primary"
        type="button"
        :disabled="!currentReminder || actionPending"
        @click="completeCurrentReminder"
      >
        完成
      </button>

      <button
        class="action-btn"
        type="button"
        :disabled="!currentReminder || actionPending"
        @click="dismissCurrentReminder"
      >
        知道了
      </button>

      <el-dropdown
        trigger="click"
        placement="top-end"
        :disabled="!currentReminder || actionPending"
        @command="handleSnoozeCommand"
      >
        <button class="action-btn action-ghost" type="button" :disabled="!currentReminder || actionPending">
          稍后提醒
          <span class="dropdown-arrow">▾</span>
        </button>
        <template #dropdown>
          <el-dropdown-menu>
            <el-dropdown-item v-for="option in snoozeOptions" :key="option.minutes" :command="option.minutes">
              {{ option.label }}
            </el-dropdown-item>
          </el-dropdown-menu>
        </template>
      </el-dropdown>
    </footer>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ElMessage } from "element-plus";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { APP_EVENTS } from "../bridge/events";
import type { TodoPriority, TodoReminderDispatch } from "../types/todo";

declare global {
  interface Window {
    __LAZYCAT_REMINDER_BOOTSTRAP__?: TodoReminderDispatch[];
  }
}

const popupWindow = getCurrentWindow();

const queue = ref<TodoReminderDispatch[]>([]);
const actionPending = ref(false);
let unlistenReminderPush: UnlistenFn | null = null;

const snoozeOptions = [
  { label: "5 分钟后", minutes: 5 },
  { label: "10 分钟后", minutes: 10 },
  { label: "15 分钟后", minutes: 15 },
  { label: "30 分钟后", minutes: 30 },
  { label: "1 小时后", minutes: 60 },
  { label: "2 小时后", minutes: 120 },
  { label: "1 天后", minutes: 1440 },
];

const currentReminder = computed(() => queue.value[0] ?? null);

const currentIndexLabel = computed(() => {
  if (!queue.value.length) return "0/0";
  return `1/${queue.value.length}`;
});

function normalizePayload(payload: TodoReminderDispatch | TodoReminderDispatch[] | null | undefined) {
  if (!payload) return [];
  return Array.isArray(payload) ? payload : [payload];
}

function mergeReminderQueue(incoming: TodoReminderDispatch[]) {
  if (!incoming.length) return;
  const nextQueue = [...queue.value];
  for (const reminder of incoming) {
    if (nextQueue.some((item) => item.eventId === reminder.eventId)) {
      continue;
    }
    nextQueue.push(reminder);
  }
  queue.value = nextQueue;
}

function priorityClass(priority: TodoPriority) {
  return `priority-${priority.toLowerCase()}`;
}

function formatFireTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(date);
}

async function shiftQueueOrClose() {
  queue.value = queue.value.slice(1);
  if (!queue.value.length) {
    await closePopup();
  }
}

async function closePopup() {
  try {
    await popupWindow.close();
  } catch {
    window.close();
  }
}

async function runAction(action: () => Promise<void>) {
  if (!currentReminder.value || actionPending.value) {
    return;
  }
  actionPending.value = true;
  try {
    await action();
    await shiftQueueOrClose();
  } catch (error) {
    ElMessage.error((error as Error).message || "提醒操作失败");
  } finally {
    actionPending.value = false;
  }
}

async function completeCurrentReminder() {
  await runAction(async () => {
    await invoke("reminder_popup_complete", {
      taskId: currentReminder.value?.taskId,
    });
  });
}

async function dismissCurrentReminder() {
  await runAction(async () => {
    await invoke("reminder_popup_dismiss", {
      eventId: currentReminder.value?.eventId,
    });
  });
}

async function handleSnoozeCommand(minutes: string | number) {
  const duration = typeof minutes === "number" ? minutes : Number(minutes);
  if (!Number.isFinite(duration)) {
    return;
  }
  await runAction(async () => {
    await invoke("reminder_popup_snooze", {
      taskId: currentReminder.value?.taskId,
      taskReminderId: currentReminder.value?.taskReminderId,
      minutes: duration,
    });
  });
}

onMounted(async () => {
  mergeReminderQueue(normalizePayload(window.__LAZYCAT_REMINDER_BOOTSTRAP__));
  delete window.__LAZYCAT_REMINDER_BOOTSTRAP__;
  unlistenReminderPush = await listen<TodoReminderDispatch | TodoReminderDispatch[]>(
    APP_EVENTS.REMINDER_PUSH,
    (event) => {
      mergeReminderQueue(normalizePayload(event.payload));
    },
  );
});

onBeforeUnmount(() => {
  unlistenReminderPush?.();
  unlistenReminderPush = null;
});
</script>

<style scoped>
.reminder-popup {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  padding: 14px;
  box-sizing: border-box;
  background: linear-gradient(180deg, #ffffff 0%, #f7f9fc 100%);
  color: #1f2937;
  user-select: none;
}

.popup-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 4px 2px 12px;
}

.header-main {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.header-icon {
  width: 36px;
  height: 36px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #eef4ff;
  font-size: 18px;
  box-shadow: inset 0 0 0 1px rgba(64, 158, 255, 0.12);
}

.header-copy {
  min-width: 0;
}

.header-title {
  font-size: 15px;
  font-weight: 700;
  color: #111827;
}

.header-subtitle {
  margin-top: 4px;
  display: flex;
  gap: 8px;
  font-size: 12px;
  color: #6b7280;
}

.close-btn {
  width: 32px;
  height: 32px;
  border: none;
  border-radius: 10px;
  background: transparent;
  color: #6b7280;
  font-size: 20px;
  line-height: 1;
  cursor: pointer;
}

.close-btn:hover {
  background: rgba(15, 23, 42, 0.06);
  color: #111827;
}

.popup-body {
  flex: 1;
  display: flex;
  align-items: flex-start;
}

.reminder-card,
.empty-state {
  width: 100%;
  display: flex;
  flex-direction: column;
  border-radius: 18px;
  background: rgba(255, 255, 255, 0.96);
  border: 1px solid rgba(148, 163, 184, 0.18);
  box-shadow: 0 14px 30px rgba(15, 23, 42, 0.08);
}

.reminder-card {
  padding: 18px;
  gap: 12px;
}

.card-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.priority-badge,
.queue-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 28px;
  padding: 0 10px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 700;
}

.priority-p0 {
  color: #b42318;
  background: #fee4e2;
}

.priority-p1 {
  color: #b54708;
  background: #ffead5;
}

.priority-p2 {
  color: #175cd3;
  background: #dbeafe;
}

.priority-p3 {
  color: #475467;
  background: #eaecf0;
}

.queue-badge {
  color: #475467;
  background: #f2f4f7;
}

.reminder-title-row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.reminder-title-row .priority-badge {
  flex-shrink: 0;
}

.reminder-title {
  margin: 0;
  font-size: 22px;
  line-height: 1.4;
  font-weight: 700;
  color: #111827;
  min-width: 0;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.reminder-body {
  margin: 0;
  color: #4b5563;
  font-size: 14px;
  line-height: 1.6;
  word-break: break-word;
  display: -webkit-box;
  -webkit-line-clamp: 5;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.reminder-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding-top: 12px;
  border-top: 1px solid rgba(148, 163, 184, 0.16);
}

.footer-label {
  font-size: 12px;
  color: #6b7280;
}

.footer-value {
  font-size: 13px;
  font-weight: 600;
  color: #111827;
}

.empty-state {
  align-items: center;
  justify-content: center;
  text-align: center;
  gap: 8px;
  padding: 24px;
}

.empty-icon {
  font-size: 28px;
}

.empty-title {
  font-size: 16px;
  font-weight: 700;
  color: #111827;
}

.empty-subtitle {
  font-size: 13px;
  color: #6b7280;
}

.popup-actions {
  padding-top: 14px;
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

.action-btn {
  height: 40px;
  border: 1px solid rgba(148, 163, 184, 0.24);
  border-radius: 12px;
  background: #ffffff;
  color: #1f2937;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.18s ease;
}

.action-btn:hover:not(:disabled) {
  border-color: rgba(59, 130, 246, 0.32);
  transform: translateY(-1px);
  box-shadow: 0 10px 18px rgba(59, 130, 246, 0.12);
}

.action-btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.action-primary {
  color: #ffffff;
  border-color: #2563eb;
  background: linear-gradient(135deg, #2563eb 0%, #3b82f6 100%);
}

.action-ghost {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  width: 100%;
}

.dropdown-arrow {
  font-size: 11px;
}
</style>

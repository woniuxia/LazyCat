<template>
  <div class="notification-popup">
    <header class="popup-header">
      <div class="header-main" data-tauri-drag-region>
        <div class="header-icon" :class="headerTone"><component :is="headerIcon" /></div>
        <div class="header-copy" data-tauri-drag-region>
          <div class="header-title">{{ headerTitle }}</div>
          <div class="header-subtitle">
            <span v-if="queue.length > 1">{{ currentIndexLabel }}</span>
            <span>{{ headerSubtitle }}</span>
          </div>
        </div>
      </div>
      <button
        class="close-btn"
        type="button"
        aria-label="知道了并关闭当前通知"
        @click="acknowledgeCurrent"
      >
        <Close />
      </button>
    </header>

    <main class="popup-body">
      <section v-if="currentTodo" class="notification-card">
        <div class="title-row">
          <span class="status-badge priority-badge" :class="priorityClass(currentTodo.priority)">{{
            currentTodo.priority
          }}</span>
          <h1 class="notification-title">{{ currentTodo.title }}</h1>
        </div>
        <p v-if="currentTodo.body" class="notification-body">{{ currentTodo.body }}</p>
        <div class="notification-footer">
          <span>提醒时间</span><strong>{{ formatFireTime(currentTodo.fireAt) }}</strong>
        </div>
      </section>

      <section v-else-if="currentPackage" class="notification-card package-card">
        <div class="title-row">
          <span class="status-badge" :class="`package-${currentPackage.status}`">{{
            packageStatusLabel
          }}</span>
          <h1 class="notification-title">{{ currentPackage.projectName }}</h1>
        </div>
        <p class="notification-body">{{ packageCopy.detail }}</p>
        <p v-if="packageError" class="error-summary">{{ packageError }}</p>
        <div v-if="currentPackage.archivePath" class="path-box" :title="currentPackage.archivePath">
          {{ currentPackage.archivePath }}
        </div>
      </section>
    </main>

    <footer v-if="currentTodo" class="popup-actions">
      <button
        class="action-btn action-primary"
        type="button"
        :disabled="actionPending"
        @click="completeCurrentReminder"
      >
        完成
      </button>
      <button
        class="action-btn"
        type="button"
        :disabled="actionPending"
        @click="dismissCurrentReminder"
      >
        知道了
      </button>
      <el-dropdown
        trigger="click"
        placement="top-end"
        :disabled="actionPending"
        @command="handleSnoozeCommand"
      >
        <button class="action-btn action-ghost" type="button" :disabled="actionPending">
          稍后提醒 <span>▾</span>
        </button>
        <template #dropdown>
          <el-dropdown-menu>
            <el-dropdown-item
              v-for="option in snoozeOptions"
              :key="option.minutes"
              :command="option.minutes"
              >{{ option.label }}</el-dropdown-item
            >
          </el-dropdown-menu>
        </template>
      </el-dropdown>
    </footer>

    <footer v-else-if="currentPackage" class="popup-actions package-actions">
      <button
        class="action-btn action-primary"
        type="button"
        :disabled="actionPending"
        @click="openReleasePackageTool"
      >
        打开打包页面
      </button>
      <button
        v-if="canOpenDirectory"
        class="action-btn"
        type="button"
        :disabled="actionPending"
        @click="openReleasePackageDirectory"
      >
        打开目标目录
      </button>
      <button
        class="action-btn"
        type="button"
        :disabled="actionPending"
        @click="acknowledgeCurrent"
      >
        知道了
      </button>
    </footer>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ElMessage } from "element-plus";
import { AlarmClock, CircleCheckFilled, Close, WarningFilled } from "@element-plus/icons-vue";
import { APP_EVENTS } from "../bridge/events";
import { invokeToolByChannel } from "../bridge/tauri";
import type { GlobalNotification } from "../types/global-notification";
import {
  mergeGlobalNotificationQueue,
  normalizeGlobalNotificationPayload,
  releasePackageNotificationCopy,
  summarizeNotificationError,
} from "../utils/globalNotification";

declare global {
  interface Window {
    __LAZYCAT_NOTIFICATION_BOOTSTRAP__?: GlobalNotification[];
  }
}

const popupWindow = getCurrentWindow();
const queue = ref<GlobalNotification[]>([]);
const actionPending = ref(false);
let unlistenNotificationPush: UnlistenFn | null = null;

const snoozeOptions = [
  { label: "5 分钟后", minutes: 5 },
  { label: "10 分钟后", minutes: 10 },
  { label: "15 分钟后", minutes: 15 },
  { label: "30 分钟后", minutes: 30 },
  { label: "1 小时后", minutes: 60 },
  { label: "2 小时后", minutes: 120 },
  { label: "1 天后", minutes: 1440 },
];

const currentNotification = computed(() => queue.value[0] ?? null);
const currentTodo = computed(() =>
  currentNotification.value?.kind === "todo-reminder" ? currentNotification.value : null,
);
const currentPackage = computed(() =>
  currentNotification.value?.kind === "release-package" ? currentNotification.value : null,
);
const currentIndexLabel = computed(() => `1/${queue.value.length}`);
const packageCopy = computed(() =>
  currentPackage.value
    ? releasePackageNotificationCopy(currentPackage.value.status, currentPackage.value.packageType)
    : { title: "", detail: "" },
);
const packageStatusLabel = computed(() =>
  currentPackage.value?.status === "succeeded"
    ? "成功"
    : currentPackage.value?.status === "partially_succeeded"
      ? "部分成功"
      : currentPackage.value?.status === "package_succeeded_upload_failed"
        ? "上传失败"
        : currentPackage.value?.status === "cancelled"
          ? "已终止"
          : "失败",
);
const packageError = computed(() => summarizeNotificationError(currentPackage.value?.error));
const canOpenDirectory = computed(() =>
  Boolean(currentPackage.value?.status !== "failed" && currentPackage.value?.archivePath),
);
const headerTitle = computed(() => (currentTodo.value ? "任务提醒" : packageCopy.value.title));
const headerSubtitle = computed(() =>
  currentTodo.value ? formatFireTime(currentTodo.value.fireAt) : "上线包打包结果",
);
const headerIcon = computed(() => {
  if (currentTodo.value) return AlarmClock;
  if (
    currentPackage.value?.status === "failed"
    || currentPackage.value?.status === "package_succeeded_upload_failed"
    || currentPackage.value?.status === "cancelled"
  ) return WarningFilled;
  return CircleCheckFilled;
});
const headerTone = computed(() =>
  currentTodo.value ? "tone-reminder" : `tone-${currentPackage.value?.status ?? "failed"}`,
);

function mergeQueue(incoming: GlobalNotification[]) {
  queue.value = mergeGlobalNotificationQueue(queue.value, incoming);
}
function priorityClass(priority: string) {
  return `priority-${priority.toLowerCase()}`;
}
function formatFireTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(date);
}
async function closePopup() {
  try {
    await popupWindow.close();
  } catch {
    window.close();
  }
}
async function removeCurrentNotification() {
  queue.value = queue.value.slice(1);
  if (!queue.value.length) await closePopup();
}
async function acknowledgeCurrent() {
  if (!actionPending.value) await removeCurrentNotification();
}
async function runAction(action: () => Promise<void>) {
  if (!currentNotification.value || actionPending.value) return;
  actionPending.value = true;
  try {
    await action();
    await removeCurrentNotification();
  } catch (error) {
    ElMessage.error((error as Error).message || "通知操作失败");
  } finally {
    actionPending.value = false;
  }
}
async function completeCurrentReminder() {
  const item = currentTodo.value;
  if (item) await runAction(() => invoke("reminder_popup_complete", { taskId: item.taskId }));
}
async function dismissCurrentReminder() {
  const item = currentTodo.value;
  if (item) await runAction(() => invoke("reminder_popup_dismiss", { eventId: item.eventId }));
}
async function handleSnoozeCommand(minutes: string | number) {
  const item = currentTodo.value;
  const duration = Number(minutes);
  if (!item || !Number.isFinite(duration)) return;
  await runAction(() =>
    invoke("reminder_popup_snooze", {
      taskId: item.taskId,
      taskReminderId: item.taskReminderId,
      minutes: duration,
    }),
  );
}
async function openReleasePackageTool() {
  await runAction(() => invoke("global_notification_open_tool", { toolId: "release-package" }));
}
async function openReleasePackageDirectory() {
  const path = currentPackage.value?.archivePath;
  if (!path || !canOpenDirectory.value) return;
  await runAction(async () => {
    await invokeToolByChannel("tool:system:open-local-path", { path });
  });
}

onMounted(async () => {
  mergeQueue(normalizeGlobalNotificationPayload(window.__LAZYCAT_NOTIFICATION_BOOTSTRAP__));
  delete window.__LAZYCAT_NOTIFICATION_BOOTSTRAP__;
  unlistenNotificationPush = await listen<GlobalNotification | GlobalNotification[]>(
    APP_EVENTS.GLOBAL_NOTIFICATION_PUSH,
    ({ payload }) => {
      try {
        mergeQueue(normalizeGlobalNotificationPayload(payload));
      } catch (error) {
        ElMessage.error((error as Error).message);
      }
    },
  );
});
onBeforeUnmount(() => {
  unlistenNotificationPush?.();
  unlistenNotificationPush = null;
});
</script>

<style scoped>
.notification-popup {
  height: 100vh;
  display: flex;
  flex-direction: column;
  padding: 14px;
  box-sizing: border-box;
  background: linear-gradient(180deg, #fff 0%, #f7f9fc 100%);
  color: #1f2937;
  user-select: none;
  overflow: hidden;
}
.popup-header,
.header-main,
.title-row,
.notification-footer {
  display: flex;
  align-items: center;
}
.popup-header {
  justify-content: space-between;
  gap: 12px;
  padding: 4px 2px 12px;
}
.header-main,
.title-row {
  gap: 10px;
  min-width: 0;
}
.header-icon {
  width: 36px;
  height: 36px;
  border-radius: 8px;
  display: grid;
  place-items: center;
  background: #eef4ff;
  color: #175cd3;
  font-weight: 800;
}
.header-icon :deep(svg) {
  width: 18px;
  height: 18px;
}
.tone-succeeded {
  background: #eaf8ef;
  color: #237a3b;
}
.tone-partially_succeeded {
  background: #fff4df;
  color: #8a4b08;
}
.tone-package_succeeded_upload_failed,
.tone-cancelled {
  background: #fff4df;
  color: #8a4b08;
}
.tone-failed {
  background: #fee4e2;
  color: #b42318;
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
  border: 0;
  border-radius: 8px;
  display: grid;
  place-items: center;
  background: transparent;
  color: #6b7280;
  cursor: pointer;
}
.close-btn :deep(svg) {
  width: 16px;
  height: 16px;
}
.close-btn:hover {
  background: rgba(15, 23, 42, 0.06);
  color: #111827;
}
.popup-body {
  flex: 1;
  min-height: 0;
  display: flex;
  align-items: flex-start;
}
.notification-card {
  width: 100%;
  max-height: 100%;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  gap: 12px;
  padding: 18px;
  border: 1px solid rgba(148, 163, 184, 0.18);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.96);
  box-shadow: 0 14px 30px rgba(15, 23, 42, 0.08);
}
.notification-title {
  min-width: 0;
  margin: 0;
  color: #111827;
  font-size: 22px;
  line-height: 1.4;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.notification-body {
  margin: 0;
  color: #4b5563;
  font-size: 14px;
  line-height: 1.6;
  word-break: break-word;
}
.status-badge {
  flex-shrink: 0;
  min-height: 28px;
  display: inline-flex;
  align-items: center;
  padding: 0 10px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 700;
}
.package-succeeded {
  color: #237a3b;
  background: #eaf8ef;
}
.package-partially_succeeded {
  color: #8a4b08;
  background: #fff4df;
}
.package-package_succeeded_upload_failed,
.package-cancelled {
  color: #8a4b08;
  background: #fff4df;
}
.package-failed {
  color: #b42318;
  background: #fee4e2;
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
.notification-footer {
  justify-content: space-between;
  gap: 10px;
  padding-top: 12px;
  border-top: 1px solid rgba(148, 163, 184, 0.16);
  font-size: 12px;
  color: #6b7280;
}
.notification-footer strong {
  color: #111827;
  font-size: 13px;
}
.error-summary {
  margin: 0;
  padding: 9px 10px;
  border-left: 3px solid #d64545;
  border-radius: 6px;
  background: #fff2f0;
  color: #8f251f;
  font-size: 12px;
  line-height: 1.5;
  word-break: break-word;
}
.path-box {
  overflow: hidden;
  padding: 9px 10px;
  border-radius: 8px;
  background: #f4f7f9;
  color: #52606d;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.popup-actions {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
  padding-top: 14px;
}
.package-actions {
  grid-template-columns: repeat(auto-fit, minmax(100px, 1fr));
}
.action-btn {
  min-height: 40px;
  border: 1px solid rgba(148, 163, 184, 0.28);
  border-radius: 8px;
  background: #fff;
  color: #1f2937;
  font-size: 13px;
  font-weight: 650;
  cursor: pointer;
}
.action-btn:hover:not(:disabled) {
  border-color: rgba(37, 99, 235, 0.4);
  box-shadow: 0 8px 16px rgba(37, 99, 235, 0.1);
}
.action-btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
.action-primary {
  border-color: #2563eb;
  background: #2563eb;
  color: #fff;
}
.action-ghost {
  width: 100%;
}
</style>

<template>
  <div class="pomodoro-prompt" @keydown.esc="closeWindow">
    <header class="prompt-header" data-tauri-drag-region>
      <div data-tauri-drag-region>
        <div class="prompt-title">开始今天的番茄钟？</div>
        <div class="prompt-subtitle">{{ scheduleLabel }}</div>
      </div>
      <button class="close-btn" type="button" aria-label="关闭番茄钟提示" @click="closeWindow">
        ×
      </button>
    </header>

    <main class="prompt-body">
      <div class="time-stack">
        <span>25</span>
        <small>专注</small>
        <span>5</span>
        <small>休息</small>
      </div>
      <p>按默认节奏循环到下班，午休时间自动暂停。</p>
    </main>

    <footer class="prompt-actions">
      <button class="action-btn action-primary" type="button" :disabled="pending" @click="startToday">
        开始今天
      </button>
      <button class="action-btn" type="button" :disabled="pending" @click="skipToday">
        今天跳过
      </button>
    </footer>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { emit } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invokeToolByChannel } from "../bridge/tauri";
import type { PomodoroState } from "../types/pomodoro";

const popupWindow = getCurrentWindow();
const state = ref<PomodoroState | null>(null);
const pending = ref(false);

const scheduleLabel = computed(() => {
  const config = state.value?.config;
  if (!config) return "08:00 - 17:00，12:00 - 13:30 跳过";
  return `${config.workdayStart} - ${config.workdayEnd}，${config.lunchStart} - ${config.lunchEnd} 跳过`;
});

async function loadState() {
  try {
    state.value = (await invokeToolByChannel("tool:pomodoro:get-state", {})) as PomodoroState;
  } catch {
    state.value = null;
  }
}

async function runAction(action: () => Promise<unknown>) {
  if (pending.value) return;
  pending.value = true;
  try {
    await action();
    await emit("pomodoro-state-changed", { refresh: true });
    await closeWindow();
  } finally {
    pending.value = false;
  }
}

async function startToday() {
  await runAction(() => invokeToolByChannel("tool:pomodoro:start-today", {}));
}

async function skipToday() {
  await runAction(() => invokeToolByChannel("tool:pomodoro:skip-today", {}));
}

async function closeWindow() {
  try {
    await popupWindow.close();
  } catch {
    window.close();
  }
}

onMounted(() => {
  void loadState();
});
</script>

<style scoped>
.pomodoro-prompt {
  display: flex;
  flex-direction: column;
  min-height: 100vh;
  padding: 16px;
  box-sizing: border-box;
  background: #ffffff;
  color: #1f2937;
  user-select: none;
}

.prompt-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.prompt-title {
  font-size: 18px;
  font-weight: 700;
  color: #111827;
}

.prompt-subtitle {
  margin-top: 4px;
  font-size: 12px;
  color: #667085;
}

.close-btn {
  width: 30px;
  height: 30px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: #667085;
  font-size: 22px;
  line-height: 1;
  cursor: pointer;
}

.close-btn:hover {
  background: #f2f4f7;
  color: #111827;
}

.prompt-body {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 18px;
  padding: 18px 0 14px;
}

.time-stack {
  display: grid;
  grid-template-columns: auto auto;
  gap: 0 8px;
  align-items: baseline;
  padding: 14px 16px;
  border-radius: 8px;
  background: #f8fafc;
  border: 1px solid #e5e7eb;
}

.time-stack span {
  color: #1d4ed8;
  font-size: 34px;
  font-weight: 750;
  line-height: 1;
  font-variant-numeric: tabular-nums;
}

.time-stack small {
  color: #475467;
  font-size: 12px;
}

.prompt-body p {
  margin: 0;
  color: #344054;
  font-size: 14px;
  line-height: 1.6;
}

.prompt-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.action-btn {
  min-width: 96px;
  height: 36px;
  padding: 0 14px;
  border: 1px solid #d0d5dd;
  border-radius: 6px;
  background: #ffffff;
  color: #344054;
  font-size: 14px;
  cursor: pointer;
}

.action-btn:hover {
  background: #f9fafb;
}

.action-btn:disabled {
  cursor: default;
  opacity: 0.7;
}

.action-primary {
  border-color: #2563eb;
  background: #2563eb;
  color: #ffffff;
}

.action-primary:hover {
  background: #1d4ed8;
}
</style>

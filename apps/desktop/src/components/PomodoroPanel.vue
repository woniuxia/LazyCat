<template>
  <div class="pomodoro-panel">
    <header class="panel-header">
      <div>
        <h1>番茄钟</h1>
        <p>{{ scheduleLabel }}</p>
      </div>
      <div class="daily-toggle">
        <span>工作日自动询问</span>
        <el-switch
          :model-value="state?.config.enabled ?? true"
          :loading="loading"
          @change="setDailyEnabled"
        />
      </div>
    </header>

    <section class="timer-surface" :class="phaseClass">
      <div class="timer-copy">
        <div class="phase-label">{{ headline }}</div>
        <div class="timer-value">{{ countdownText }}</div>
        <div class="phase-meta">{{ metaText }}</div>
      </div>
      <div class="timer-ring" :style="ringStyle">
        <el-icon :size="42">
          <Timer />
        </el-icon>
      </div>
    </section>

    <section class="action-row">
      <el-button type="primary" :disabled="isRunning" :loading="actionPending" @click="startToday">
        <el-icon><VideoPlay /></el-icon>
        开始今天
      </el-button>
      <el-button :disabled="!isRunning" :loading="actionPending" @click="stopToday">
        <el-icon><Close /></el-icon>
        停止
      </el-button>
      <el-button :disabled="isRunning" :loading="actionPending" @click="skipToday">
        <el-icon><SwitchButton /></el-icon>
        今天跳过
      </el-button>
    </section>

    <section class="detail-grid">
      <div class="detail-block">
        <div class="detail-label">今日状态</div>
        <div class="detail-value" :class="statusClass">{{ statusLabel }}</div>
      </div>
      <div class="detail-block">
        <div class="detail-label">默认节奏</div>
        <div class="detail-value">
          {{ state?.config.focusMinutes ?? 25 }} / {{ state?.config.shortBreakMinutes ?? 5 }} 分钟
        </div>
      </div>
      <div class="detail-block">
        <div class="detail-label">午休跳过</div>
        <div class="detail-value">
          {{ state?.config.lunchStart ?? "12:00" }} - {{ state?.config.lunchEnd ?? "13:30" }}
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { ElMessage } from "element-plus";
import { Close, SwitchButton, Timer, VideoPlay } from "@element-plus/icons-vue";
import { invokeToolByChannel } from "../bridge/tauri";
import { APP_EVENTS } from "../bridge/events";
import {
  DEFAULT_POMODORO_CONFIG,
  formatPomodoroDuration,
  getPomodoroPhase,
  type PomodoroPhase,
} from "../utils/pomodoroSchedule";
import type { PomodoroState } from "../types/pomodoro";

const state = ref<PomodoroState | null>(null);
const loading = ref(false);
const actionPending = ref(false);
const now = ref(new Date());
let tickHandle: ReturnType<typeof window.setInterval> | null = null;
let unlistenRefresh: UnlistenFn | null = null;

const session = computed(() => state.value?.session ?? null);
const config = computed(() => state.value?.config ?? DEFAULT_POMODORO_CONFIG);
const isRunning = computed(() => session.value?.status === "running");

const phase = computed<PomodoroPhase | null>(() => {
  if (!isRunning.value) return null;
  return getPomodoroPhase(config.value, session.value?.startedAt, now.value);
});

const headline = computed(() => {
  if (phase.value) return phase.value.label;
  return statusLabel.value;
});

const countdownText = computed(() => {
  if (!phase.value) return "--:--";
  return formatPomodoroDuration(phase.value.remainingSeconds);
});

const metaText = computed(() => {
  if (phase.value?.kind === "focus") return `第 ${phase.value.cycleIndex} 个番茄`;
  if (phase.value?.kind === "break") return `第 ${phase.value.cycleIndex} 轮休息`;
  if (phase.value?.kind === "paused") return "午休结束后自动继续";
  if (phase.value?.kind === "done") return "已到下班时间";
  return "工作日 08:00 自动询问";
});

const scheduleLabel = computed(() => {
  const item = config.value;
  return `${item.workdayStart} - ${item.workdayEnd}，${item.lunchStart} - ${item.lunchEnd} 跳过`;
});

const statusLabel = computed(() => {
  switch (session.value?.status) {
    case "prompted":
      return "等待确认";
    case "running":
      return "运行中";
    case "skipped":
      return "今日已跳过";
    case "stopped":
      return "今日已停止";
    case "completed":
      return "今日已完成";
    default:
      return "今日未开始";
  }
});

const statusClass = computed(() => `is-${session.value?.status ?? "idle"}`);
const phaseClass = computed(() => `is-${phase.value?.kind ?? session.value?.status ?? "idle"}`);

const ringStyle = computed(() => {
  const item = phase.value;
  if (!item || item.kind === "paused" || item.kind === "done") {
    return { "--progress": "0deg" };
  }
  const totalSeconds =
    item.kind === "focus" ? config.value.focusMinutes * 60 : config.value.shortBreakMinutes * 60;
  const progress = totalSeconds > 0 ? 1 - item.remainingSeconds / totalSeconds : 0;
  return { "--progress": `${Math.max(0, Math.min(1, progress)) * 360}deg` };
});

async function loadState() {
  loading.value = true;
  try {
    state.value = (await invokeToolByChannel("tool:pomodoro:get-state", {})) as PomodoroState;
  } catch (error) {
    ElMessage.error((error as Error).message || "加载番茄钟状态失败");
  } finally {
    loading.value = false;
  }
}

async function runAction(action: () => Promise<unknown>, successText: string) {
  if (actionPending.value) return;
  actionPending.value = true;
  try {
    await action();
    await loadState();
    ElMessage.success(successText);
  } catch (error) {
    ElMessage.error((error as Error).message || "番茄钟操作失败");
  } finally {
    actionPending.value = false;
  }
}

async function setDailyEnabled(value: string | number | boolean) {
  const enabled = Boolean(value);
  await runAction(
    () => invokeToolByChannel("tool:pomodoro:set-enabled", { enabled }),
    enabled ? "已开启工作日自动询问" : "已关闭工作日自动询问",
  );
}

async function startToday() {
  await runAction(() => invokeToolByChannel("tool:pomodoro:start-today", {}), "番茄钟已开始");
}

async function stopToday() {
  await runAction(() => invokeToolByChannel("tool:pomodoro:stop-today", {}), "番茄钟已停止");
}

async function skipToday() {
  await runAction(() => invokeToolByChannel("tool:pomodoro:skip-today", {}), "今天已跳过");
}

onMounted(async () => {
  await loadState();
  tickHandle = window.setInterval(() => {
    now.value = new Date();
  }, 1000);
  unlistenRefresh = await listen(APP_EVENTS.POMODORO_STATE_CHANGED, () => {
    void loadState();
  });
});

onBeforeUnmount(() => {
  if (tickHandle) window.clearInterval(tickHandle);
  unlistenRefresh?.();
});
</script>

<style scoped>
.pomodoro-panel {
  display: flex;
  flex-direction: column;
  gap: 18px;
  max-width: 960px;
  margin: 0 auto;
  padding: 22px;
}

.panel-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.panel-header h1 {
  margin: 0;
  font-size: 24px;
  font-weight: 700;
  color: #111827;
}

.panel-header p {
  margin: 6px 0 0;
  color: #667085;
  font-size: 13px;
}

.daily-toggle {
  display: flex;
  align-items: center;
  gap: 10px;
  color: #344054;
  font-size: 13px;
  white-space: nowrap;
}

.timer-surface {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 220px;
  padding: 28px;
  overflow: hidden;
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  background: linear-gradient(135deg, rgba(17, 24, 39, 0.04), rgba(255, 255, 255, 0) 45%), #ffffff;
}

.timer-surface::before {
  content: "";
  position: absolute;
  inset: 0;
  border-left: 4px solid #2563eb;
  pointer-events: none;
}

.timer-surface.is-break::before,
.timer-surface.is-paused::before {
  border-left-color: #0f9f6e;
}

.timer-surface.is-skipped::before,
.timer-surface.is-stopped::before {
  border-left-color: #98a2b3;
}

.timer-copy {
  position: relative;
  z-index: 1;
  min-width: 0;
}

.phase-label {
  color: #475467;
  font-size: 15px;
  font-weight: 600;
}

.timer-value {
  margin-top: 10px;
  color: #101828;
  font-size: 72px;
  font-weight: 750;
  line-height: 1;
  font-variant-numeric: tabular-nums;
}

.phase-meta {
  margin-top: 12px;
  color: #667085;
  font-size: 14px;
}

.timer-ring {
  --progress: 0deg;
  position: relative;
  display: grid;
  place-items: center;
  width: 148px;
  height: 148px;
  flex: 0 0 auto;
  border-radius: 50%;
  color: #1d4ed8;
  background:
    radial-gradient(circle at center, #ffffff 0 58%, transparent 59%),
    conic-gradient(#2563eb var(--progress), #e5e7eb 0deg);
}

.timer-surface.is-break .timer-ring,
.timer-surface.is-paused .timer-ring {
  color: #047857;
  background:
    radial-gradient(circle at center, #ffffff 0 58%, transparent 59%),
    conic-gradient(#10b981 var(--progress), #e5e7eb 0deg);
}

.action-row {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.detail-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

.detail-block {
  min-height: 86px;
  padding: 16px;
  border: 1px solid #eaecf0;
  border-radius: 8px;
  background: #ffffff;
}

.detail-label {
  color: #667085;
  font-size: 12px;
}

.detail-value {
  margin-top: 8px;
  color: #101828;
  font-size: 16px;
  font-weight: 650;
}

.detail-value.is-running {
  color: #1d4ed8;
}

.detail-value.is-skipped,
.detail-value.is-stopped {
  color: #667085;
}

@media (max-width: 760px) {
  .panel-header,
  .timer-surface {
    flex-direction: column;
    align-items: stretch;
  }

  .timer-value {
    font-size: 52px;
  }

  .timer-ring {
    width: 120px;
    height: 120px;
  }

  .detail-grid {
    grid-template-columns: 1fr;
  }
}
</style>

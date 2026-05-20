<template>
  <div class="widget-panel">
    <!-- 状态卡片 -->
    <section class="status-card" :class="{ 'is-paused': status?.paused }">
      <div class="status-head">
        <div class="status-title">
          <span class="dot" :class="statusDotClass" />
          <span class="title-text">{{ statusLabel }}</span>
          <el-tag v-if="status?.pauseReason" size="small" type="warning">
            {{ pauseReasonLabel(status.pauseReason) }}
          </el-tag>
        </div>
        <div class="status-actions">
          <el-switch
            :model-value="config.enabled"
            :loading="toggling"
            active-text="启用"
            inactive-text="关闭"
            @update:model-value="onToggleEnabled"
          />
        </div>
      </div>

      <div v-if="status?.privacyMaskActive" class="banner banner-warn">
        <span><el-icon class="banner-icon"><Lock /></el-icon> 敏感模式已开启{{ privacyUntilLabel }}</span>
        <el-button size="small" type="primary" link @click="onPrivacyOff">一键关闭</el-button>
      </div>
      <div v-if="status?.spotlightDetected" class="banner banner-warn">
        <span><el-icon class="banner-icon"><WarningFilled /></el-icon> 检测到 Windows Spotlight 启用，可能影响桌面壁纸；不影响本工具挂件</span>
      </div>
      <div v-if="status?.thirdPartyEngine" class="banner banner-warn">
        <span><el-icon class="banner-icon"><WarningFilled /></el-icon> 检测到 {{ status.thirdPartyEngine }}，挂件不受影响但桌面壁纸可能被它改</span>
      </div>

      <div v-if="status?.lastError" class="banner banner-error">
        <span><el-icon class="banner-icon"><CircleCloseFilled /></el-icon> {{ status.lastError }}</span>
        <el-button size="small" type="primary" link :loading="applying" @click="onApply">重试</el-button>
      </div>

      <div v-if="autoSkipBanner" class="banner banner-info">
        <span><el-icon class="banner-icon"><VideoPause /></el-icon> {{ autoSkipBanner }}</span>
      </div>

      <div class="status-meta">
        <div class="meta-item">
          <span class="meta-label">上次刷新</span>
          <span class="meta-value">{{ formatTime(status?.lastRenderedAt) }}</span>
        </div>
      </div>

      <div class="status-buttons">
        <el-button
          :loading="applying"
          :disabled="!config.enabled || !!status?.paused"
          :title="status?.paused ? '当前已暂停，请先点恢复或老板键再刷新' : ''"
          @click="onApply"
        >
          立即刷新
        </el-button>
        <el-button
          v-if="!status?.paused"
          :disabled="!config.enabled"
          @click="onPause"
        >
          暂停
        </el-button>
        <el-button v-else type="primary" @click="onResume">恢复</el-button>
        <el-button
          :disabled="!config.enabled"
          :title="!config.enabled ? '需要先启用挂件' : '把挂件 Y 位置回到屏幕居中'"
          @click="onResetPosition"
        >
          重置挂件位置
        </el-button>
      </div>
    </section>

    <!-- 配置分组 -->
    <el-tabs v-model="activeTab" class="config-tabs">
      <el-tab-pane label="基础" name="basic">
        <el-form label-width="120px" label-position="left">
          <el-form-item label="刷新间隔">
            <el-radio-group
              v-model="config.refreshIntervalMin"
              @change="saveField('refreshIntervalMin')"
            >
              <el-radio-button :label="5">5 min</el-radio-button>
              <el-radio-button :label="15">15 min</el-radio-button>
              <el-radio-button :label="30">30 min</el-radio-button>
              <el-radio-button :label="60">60 min</el-radio-button>
            </el-radio-group>
            <div class="hint">
              心跳间隔（PM/Todo CRUD 后 5s 自动立刷，不依赖此间隔）
            </div>
          </el-form-item>
          <el-form-item label="停靠位置">
            <el-radio-group
              v-model="config.edge"
              @change="saveField('edge')"
            >
              <el-radio-button label="right">右侧</el-radio-button>
              <el-radio-button label="left">左侧</el-radio-button>
            </el-radio-group>
          </el-form-item>
          <el-form-item label="收起延迟">
            <el-radio-group
              v-model="config.collapseDelayMs"
              @change="saveField('collapseDelayMs')"
            >
              <el-radio-button :label="200">0.2s</el-radio-button>
              <el-radio-button :label="500">0.5s</el-radio-button>
              <el-radio-button :label="800">0.8s</el-radio-button>
              <el-radio-button :label="1200">1.2s</el-radio-button>
              <el-radio-button :label="2000">2s</el-radio-button>
            </el-radio-group>
            <div class="hint">
              鼠标离开挂件后自动收起的等待时间
            </div>
          </el-form-item>
        </el-form>
      </el-tab-pane>

      <el-tab-pane label="隐私" name="privacy">
        <el-form label-width="120px" label-position="left">
          <el-form-item label="敏感模式">
            <el-radio-group
              :model-value="privacyChoice"
              @update:model-value="onPrivacyChoiceChange"
            >
              <el-radio-button label="off">关闭</el-radio-button>
              <el-radio-button :label="30">30 分钟</el-radio-button>
              <el-radio-button :label="120">2 小时</el-radio-button>
              <el-radio-button :label="0">直到手动关</el-radio-button>
            </el-radio-group>
            <div v-if="config.privacyMaskUntil" class="hint">
              剩余 {{ privacyRemainingLabel }} 后自动关闭
            </div>
            <div v-else class="hint">开启后 todo 标题打码（▓▓▓）</div>
          </el-form-item>
          <el-form-item label="全屏切净">
            <el-input
              type="textarea"
              :rows="3"
              :model-value="config.fullscreenBlacklist.join('\n')"
              placeholder="每行一个 .exe 名，如 obs64.exe"
              @update:model-value="onBlacklistInput"
            />
            <div class="blacklist-actions">
              <el-button link type="primary" @click="onResetBlacklist">恢复默认</el-button>
              <span v-if="blacklistPendingSave" class="blacklist-saving">保存中…</span>
              <span v-if="!config.fullscreenBlacklist.length" class="blacklist-empty">
                当前名单为空，全屏判定仅依赖窗口尺寸与系统通知态
              </span>
            </div>
            <div class="hint">
              命中名单进程位于前台时挂件自动隐藏；默认含 obs / powerpnt / wpp / zoom
            </div>
          </el-form-item>
        </el-form>
      </el-tab-pane>

      <el-tab-pane label="拓展区" name="extension">
        <el-form label-width="120px" label-position="left">
          <el-form-item label="固定按钮">
            <el-select
              :model-value="config.extensionFixedTools"
              multiple
              filterable
              placeholder="选择固定按钮（按选择顺序展示）"
              @update:model-value="onExtensionFixedToolsChange"
            >
              <el-option
                v-for="tool in allToolOptions"
                :key="tool.id"
                :label="tool.name"
                :value="tool.id"
              />
            </el-select>
            <div class="hint">挂件拓展区固定显示的按钮，按选择顺序展示</div>
          </el-form-item>
          <el-form-item label="推荐数量">
            <el-radio-group
              v-model="config.extensionHotToolsLimit"
              @change="saveField('extensionHotToolsLimit')"
            >
              <el-radio-button :label="1">1</el-radio-button>
              <el-radio-button :label="2">2</el-radio-button>
              <el-radio-button :label="3">3</el-radio-button>
              <el-radio-button :label="5">5</el-radio-button>
              <el-radio-button :label="10">10</el-radio-button>
            </el-radio-group>
            <div class="hint">拓展区动态推荐工具的数量上限</div>
          </el-form-item>
        </el-form>
      </el-tab-pane>

      <el-tab-pane label="诊断" name="diagnostics">
        <div v-if="diagnostics" class="diagnostics-pane">
          <div class="health-grid">
            <div class="health-item">
              <span class="health-label">状态</span>
              <span class="health-value">{{ diagnostics.health.status }}</span>
            </div>
            <div class="health-item">
              <span class="health-label">窗口态</span>
              <span class="health-value">{{ diagnostics.health.visualState }}</span>
            </div>
            <div class="health-item">
              <span class="health-label">今日跳过</span>
              <span class="health-value">{{ diagnostics.health.todaySkipCount }}</span>
            </div>
            <div class="health-item">
              <span class="health-label">今日看门狗</span>
              <span class="health-value">{{ diagnostics.health.todayWatchdogCount }}</span>
            </div>
            <div class="health-item">
              <span class="health-label">今日重建</span>
              <span class="health-value">{{ diagnostics.health.todayRebuildCount }}</span>
            </div>
          </div>
          <div class="event-timeline">
            <div class="section-title">事件时间线（最近 20 条）</div>
            <div
              v-for="evt in diagnostics.events.slice(0, 20)"
              :key="evt.sequenceId"
              class="event-row"
            >
              <span class="event-seq">#{{ evt.sequenceId }}</span>
              <span class="event-type">{{ evt.type }}</span>
              <span class="event-detail">{{ evt.detail }}</span>
            </div>
            <div v-if="!diagnostics.events.length" class="event-empty">暂无事件</div>
          </div>
        </div>
        <div v-else class="diagnostics-loading">加载诊断数据中…</div>
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, computed, ref } from "vue";
import { ElMessage } from "element-plus";
import { Lock, WarningFilled, CircleCloseFilled, VideoPause } from "@element-plus/icons-vue";
import { invokeToolByChannel } from "../bridge/tauri";
import { getAllTools } from "../composables/toolCatalog";
import type {
  WidgetConfig,
  WidgetHealth,
  WidgetEventEntry,
  WidgetPauseReason,
  WidgetStatus,
} from "../types/widget";

const status = ref<WidgetStatus | null>(null);
const config = ref<WidgetConfig>(defaultConfig());
const activeTab = ref<"basic" | "privacy" | "diagnostics">("basic");
const diagnostics = ref<{ health: WidgetHealth; events: WidgetEventEntry[] } | null>(null);

const toggling = ref(false);
const applying = ref(false);

/** 启用时间戳；用于检测挂件是否已实际首次刷新（lastRenderedAt > enableAttemptAt）。 */
const enableAttemptAt = ref<number | null>(null);
/** 启用后短期内显示"启动中"。lastRenderedAt 更新或 10s 超时后清除。 */
const isStarting = ref(false);
/** 当前秒数 tick（用于倒计时响应式刷新） */
const nowTick = ref(Date.now());

/** 黑名单 debounce 保存计时器 + 是否处于待保存态。 */
const blacklistPendingSave = ref(false);

let pollHandle: number | null = null;
let pollDiagnosticsHandle: number | null = null;
let nowTicker: number | null = null;
let blacklistSaveTimer: number | null = null;
let startingTimeoutHandle: number | null = null;

onMounted(async () => {
  await Promise.all([refreshStatus(), refreshConfig()]);
  pollHandle = window.setInterval(refreshStatus, 5000);
  pollDiagnosticsHandle = window.setInterval(refreshDiagnostics, 5000);
  // 倒计时刷新（敏感模式 banner）
  nowTicker = window.setInterval(() => {
    nowTick.value = Date.now();
  }, 1000);
});

onBeforeUnmount(() => {
  if (pollHandle !== null) {
    window.clearInterval(pollHandle);
    pollHandle = null;
  }
  if (pollDiagnosticsHandle !== null) {
    window.clearInterval(pollDiagnosticsHandle);
    pollDiagnosticsHandle = null;
  }
  if (nowTicker !== null) {
    window.clearInterval(nowTicker);
    nowTicker = null;
  }
  if (blacklistSaveTimer !== null) {
    window.clearTimeout(blacklistSaveTimer);
    blacklistSaveTimer = null;
  }
  if (startingTimeoutHandle !== null) {
    window.clearTimeout(startingTimeoutHandle);
    startingTimeoutHandle = null;
  }
});

const statusLabel = computed(() => {
  if (!status.value) return "加载中…";
  if (!config.value.enabled) return "未启用";
  if (isStarting.value) return "启动中…";
  if (status.value.paused) return "已暂停";
  return "运行中";
});

const statusDotClass = computed(() => {
  if (!config.value.enabled) return "off";
  if (isStarting.value) return "warn";
  if (status.value?.paused) return "warn";
  if (status.value?.lastError) return "error";
  return "ok";
});

async function refreshStatus() {
  try {
    const v = (await invokeToolByChannel("tool:widget:status", {})) as WidgetStatus;
    status.value = v;
    // 启用后挂件首次渲染 → 清除"启动中"标记
    if (isStarting.value && enableAttemptAt.value && v.lastRenderedAt) {
      const renderedMs = new Date(v.lastRenderedAt).getTime();
      if (Number.isFinite(renderedMs) && renderedMs >= enableAttemptAt.value) {
        clearStarting();
      }
    }
  } catch (e) {
    console.warn("[widget] refresh status failed", e);
  }
}

function clearStarting() {
  isStarting.value = false;
  if (startingTimeoutHandle !== null) {
    window.clearTimeout(startingTimeoutHandle);
    startingTimeoutHandle = null;
  }
}

async function refreshConfig() {
  try {
    const v = (await invokeToolByChannel("tool:widget:get-config", {})) as WidgetConfig;
    config.value = { ...defaultConfig(), ...v };
  } catch (e) {
    console.warn("[widget] read config failed", e);
  }
}

async function onToggleEnabled(next: boolean) {
  toggling.value = true;
  try {
    if (next) {
      enableAttemptAt.value = Date.now();
      isStarting.value = true;
      // 10s 兜底；超时后无论 lastRenderedAt 是否变化都退出"启动中"
      if (startingTimeoutHandle !== null) {
        window.clearTimeout(startingTimeoutHandle);
      }
      startingTimeoutHandle = window.setTimeout(() => {
        clearStarting();
      }, 10_000);
      await invokeToolByChannel("tool:widget:enable", {});
      config.value.enabled = true;
      ElMessage.success("已启用，正在显示挂件…");
      // 启用后短期密集轮询 status，便于尽快从"启动中"切到"运行中"
      pollStartingStatus();
    } else {
      enableAttemptAt.value = null;
      clearStarting();
      await invokeToolByChannel("tool:widget:disable", {});
      config.value.enabled = false;
      ElMessage.success("已关闭挂件");
    }
    await Promise.all([refreshStatus(), refreshConfig()]);
  } catch (e) {
    clearStarting();
    ElMessage.error(`切换失败：${formatError(e)}`);
  } finally {
    toggling.value = false;
  }
}

/** 启用后 8s 内每 800ms 拉一次 status，尽快感知 lastRenderedAt 更新。 */
function pollStartingStatus() {
  let attempts = 0;
  const maxAttempts = 10;
  const intervalId = window.setInterval(() => {
    attempts += 1;
    void refreshStatus();
    if (!isStarting.value || attempts >= maxAttempts) {
      window.clearInterval(intervalId);
    }
  }, 800);
}

async function onApply() {
  applying.value = true;
  try {
    await invokeToolByChannel("tool:widget:apply", {});
    ElMessage.success("已刷新挂件");
    await refreshStatus();
  } catch (e) {
    ElMessage.error(`刷新失败：${formatError(e)}`);
  } finally {
    applying.value = false;
  }
}

async function onPause() {
  try {
    await invokeToolByChannel("tool:widget:pause", { reason: "manual" });
    await refreshStatus();
  } catch (e) {
    ElMessage.error(`暂停失败：${formatError(e)}`);
  }
}

async function onResume() {
  try {
    await invokeToolByChannel("tool:widget:resume", {});
    await refreshStatus();
  } catch (e) {
    ElMessage.error(`恢复失败：${formatError(e)}`);
  }
}

async function onResetPosition() {
  try {
    await invokeToolByChannel("tool:widget:set-config", { widgetY: null });
    // 若挂件已启用且可见，链式触发立即重新定位（widget:reposition）
    if (config.value.enabled && !status.value?.paused) {
      try {
        await invokeToolByChannel("tool:widget:reposition", {});
      } catch (e) {
        console.warn("[widget] reposition after reset failed", e);
      }
    }
    ElMessage.success("已重置，挂件已回到屏幕居中");
    await Promise.all([refreshStatus(), refreshConfig()]);
  } catch (e) {
    ElMessage.error(`重置失败：${formatError(e)}`);
  }
}

async function saveField(field: keyof WidgetConfig) {
  const payload: Record<string, unknown> = { [field]: config.value[field] };
  try {
    await invokeToolByChannel("tool:widget:set-config", payload);
    ElMessage.success("已保存");
  } catch (e) {
    ElMessage.error(`保存失败：${formatError(e)}`);
  }
}

function onBlacklistInput(v: string) {
  config.value.fullscreenBlacklist = v
    .split("\n")
    .map((s) => s.trim())
    .filter(Boolean);
  // 与其他字段保持一致：编辑即保存（debounce 600ms）
  blacklistPendingSave.value = true;
  if (blacklistSaveTimer !== null) {
    window.clearTimeout(blacklistSaveTimer);
  }
  blacklistSaveTimer = window.setTimeout(() => {
    blacklistSaveTimer = null;
    void persistBlacklist();
  }, 600);
}

async function persistBlacklist() {
  try {
    await invokeToolByChannel("tool:widget:set-config", {
      fullscreenBlacklist: config.value.fullscreenBlacklist,
    });
  } catch (e) {
    ElMessage.error(`黑名单保存失败：${formatError(e)}`);
  } finally {
    blacklistPendingSave.value = false;
  }
}

// 与 Rust `default_fullscreen_blacklist()` 保持一致
const DEFAULT_FULLSCREEN_BLACKLIST = [
  "obs64.exe",
  "obs32.exe",
  "powerpnt.exe",
  "wpp.exe",
  "zoom.exe",
];

async function onResetBlacklist() {
  config.value.fullscreenBlacklist = [...DEFAULT_FULLSCREEN_BLACKLIST];
  blacklistPendingSave.value = true;
  await persistBlacklist();
}

const allToolOptions = computed(() => {
  return getAllTools().filter((t) => t.id !== "widget" && t.id !== "home");
});

async function onExtensionFixedToolsChange(ids: string[]) {
  config.value.extensionFixedTools = ids;
  await saveField("extensionFixedTools");
}

function pauseReasonLabel(reason: WidgetPauseReason): string {
  switch (reason) {
    case "fullscreen":
      return "全屏切净";
    case "lock":
      return "锁屏";
    case "manual":
      return "手动";
  }
}

function formatTime(iso: string | null | undefined): string {
  if (!iso) return "—";
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

function formatError(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  return JSON.stringify(e);
}

async function onPrivacyOff() {
  try {
    await invokeToolByChannel("tool:widget:set-privacy-mask", { enabled: false });
    ElMessage.success("已关闭敏感模式");
    await Promise.all([refreshConfig(), refreshStatus()]);
  } catch (e) {
    ElMessage.error(`关闭失败：${formatError(e)}`);
  }
}

async function onPrivacyChoiceChange(choice: string | number | boolean | undefined) {
  if (choice === "off" || choice === undefined) {
    await onPrivacyOff();
    return;
  }
  const min = typeof choice === "number" ? choice : Number(choice);
  if (Number.isNaN(min) || min < 0) {
    ElMessage.warning("无效的时长选项");
    return;
  }
  try {
    await invokeToolByChannel("tool:widget:set-privacy-mask", {
      enabled: true,
      durationMin: min,
    });
    ElMessage.success(min > 0 ? `已开启敏感模式（${min} 分钟后自动关）` : "已开启敏感模式（直到手动关）");
    await Promise.all([refreshConfig(), refreshStatus()]);
  } catch (e) {
    ElMessage.error(`保存失败：${formatError(e)}`);
  }
}

const privacyDurationChoice = computed<number>(() => {
  if (!config.value.privacyMaskUntil) return 0;
  // 触发响应式刷新（每秒）；用 Date.now() 取实际当前时间
  nowTick.value;
  const remainMs = new Date(config.value.privacyMaskUntil).getTime() - Date.now();
  if (remainMs <= 0) return 0;
  const remainMin = remainMs / 60000;
  if (remainMin <= 45) return 30;
  if (remainMin <= 180) return 120;
  return 0;
});

const privacyChoice = computed<"off" | number>(() => {
  if (!config.value.privacyMask) return "off";
  return privacyDurationChoice.value;
});

const privacyUntilLabel = computed<string>(() => {
  if (!status.value?.privacyMaskUntil) return "（直到手动关）";
  // 触发响应式刷新
  nowTick.value;
  const remainMs = new Date(status.value.privacyMaskUntil).getTime() - Date.now();
  if (remainMs <= 0) return "（即将关闭）";
  const totalSec = Math.floor(remainMs / 1000);
  const min = Math.floor(totalSec / 60);
  const sec = totalSec % 60;
  return `（剩余 ${min}:${sec.toString().padStart(2, "0")}）`;
});

const privacyRemainingLabel = computed<string>(() => {
  if (!config.value.privacyMaskUntil) return "";
  nowTick.value;
  const remainMs = new Date(config.value.privacyMaskUntil).getTime() - Date.now();
  if (remainMs <= 0) return "0:00";
  const totalSec = Math.floor(remainMs / 1000);
  const min = Math.floor(totalSec / 60);
  const sec = totalSec % 60;
  return `${min}:${sec.toString().padStart(2, "0")}`;
});

const autoSkipBanner = computed<string>(() => {
  if (!config.value.enabled) return "";
  if (status.value?.paused) return "";
  switch (status.value?.autoSkipReason) {
    case "lock":
      return "检测到锁屏 / 屏保，已暂停刷新（解锁后自动恢复）";
    case "fullscreen": {
      const app = status.value?.autoSkipApp?.trim();
      return app
        ? `检测到全屏应用「${app}」，已暂停刷新（退出全屏后自动恢复）`
        : "检测到全屏应用，已暂停刷新（退出全屏后自动恢复）";
    }
    default:
      return "";
  }
});

async function refreshDiagnostics() {
  try {
    diagnostics.value = (await invokeToolByChannel("tool:widget:diagnostics", {})) as {
      health: WidgetHealth;
      events: WidgetEventEntry[];
    };
  } catch (e) {
    console.warn("[widget] diagnostics fetch failed", e);
  }
}

function defaultConfig(): WidgetConfig {
  return {
    enabled: false,
    style: "dashboard",
    refreshIntervalMin: 15,
    fullscreenBlacklist: [],
    privacyMask: false,
    privacyMaskUntil: null,
    widgetY: null,
    edge: "right",
    collapseDelayMs: 800,
    extensionFixedTools: ["pm", "todo", "inbox"],
    extensionHotToolsLimit: 3,
  };
}
</script>

<style scoped>
.widget-panel {
  padding: 20px;
  max-width: 880px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.status-card {
  border: 1px solid var(--el-border-color-light);
  border-radius: 12px;
  padding: 16px 20px;
  background: var(--el-bg-color);
}

.status-card.is-paused {
  background: var(--el-fill-color-light);
}

.status-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.status-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 16px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}

.dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  display: inline-block;
}
.dot.ok {
  background: #22c55e;
}
.dot.warn {
  background: #f59e0b;
}
.dot.error {
  background: #ef4444;
}
.dot.off {
  background: var(--el-text-color-placeholder);
}

.status-meta {
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: 13px;
  color: var(--el-text-color-regular);
  margin-bottom: 12px;
}

.meta-item {
  display: flex;
  gap: 8px;
}

.meta-label {
  color: var(--el-text-color-secondary);
  flex-shrink: 0;
  min-width: 64px;
}

.meta-value {
  color: var(--el-text-color-primary);
}

.status-buttons {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 12px;
  border-radius: 8px;
  font-size: 13px;
  margin-bottom: 8px;
}

.banner-icon {
  font-size: 14px;
  vertical-align: -2px;
}
.banner-warn {
  background: #fef3c7;
  color: #78350f;
  border: 1px solid #fcd34d;
}
html[data-theme="dark"] .banner-warn {
  background: rgba(252, 211, 77, 0.12);
  color: #fbbf24;
  border-color: rgba(252, 211, 77, 0.4);
}

.banner-error {
  background: #fee2e2;
  color: #7f1d1d;
  border: 1px solid #fca5a5;
}
html[data-theme="dark"] .banner-error {
  background: rgba(252, 165, 165, 0.12);
  color: #f87171;
  border-color: rgba(252, 165, 165, 0.4);
}

.banner-info {
  background: #e0f2fe;
  color: #075985;
  border: 1px solid #7dd3fc;
}
html[data-theme="dark"] .banner-info {
  background: rgba(125, 211, 252, 0.12);
  color: #38bdf8;
  border-color: rgba(125, 211, 252, 0.4);
}

.config-tabs :deep(.el-tabs__content) {
  padding-top: 8px;
}

.hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-top: 4px;
}

.ml {
  margin-left: 8px;
}

.mt {
  margin-top: 8px;
}

.blacklist-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-top: 8px;
}

.blacklist-empty {
  font-size: 12px;
  color: var(--el-color-warning);
}

.blacklist-saving {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.diagnostics-pane {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.diagnostics-loading {
  font-size: 13px;
  color: var(--el-text-color-secondary);
  padding: 12px 0;
}

.health-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: 8px;
}

.health-item {
  padding: 8px 12px;
  border-radius: 8px;
  background: var(--el-fill-color-light);
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.health-label {
  font-size: 11px;
  color: var(--el-text-color-secondary);
}

.health-value {
  font-size: 15px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}

.event-timeline {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.section-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  margin-bottom: 4px;
}

.event-row {
  display: flex;
  gap: 8px;
  font-size: 12px;
  color: var(--el-text-color-regular);
  padding: 3px 0;
  border-bottom: 1px solid var(--el-border-color-lighter);
}

.event-seq {
  color: var(--el-text-color-placeholder);
  min-width: 40px;
  font-family: monospace;
}

.event-type {
  color: var(--el-color-primary);
  min-width: 120px;
  font-family: monospace;
  font-size: 11px;
}

.event-detail {
  color: var(--el-text-color-secondary);
  flex: 1;
}

.event-empty {
  font-size: 12px;
  color: var(--el-text-color-placeholder);
  padding: 8px 0;
}
</style>

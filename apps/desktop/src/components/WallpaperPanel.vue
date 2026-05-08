<template>
  <div class="wallpaper-panel">
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

      <!-- design §9：敏感模式开启时显眼提示 + 一键关闭 -->
      <div v-if="status?.privacyMaskActive" class="banner banner-warn">
        <span>🔒 敏感模式已开启{{ privacyUntilLabel }}</span>
        <el-button size="small" type="primary" link @click="onPrivacyOff">一键关闭</el-button>
      </div>
      <!-- design §9：老板键注册失败提示 -->
      <div v-if="status?.bossKeyError" class="banner banner-warn">
        <span>⚠ {{ status.bossKeyError }}</span>
      </div>
      <!-- design §13.4：Spotlight / 第三方引擎冲突 -->
      <div v-if="status?.spotlightDetected" class="banner banner-warn">
        <span>⚠ 检测到 Windows Spotlight 启用，可能覆盖本工具壁纸；请到「设置 → 个性化 → 背景」改为「图片」</span>
      </div>
      <div v-if="status?.thirdPartyEngine" class="banner banner-warn">
        <span>⚠ 检测到 {{ status.thirdPartyEngine }}，可能覆盖本工具输出的壁纸</span>
      </div>

      <!-- design §13.7：合成失败时显眼提示 + 重试按钮 -->
      <div v-if="status?.lastError" class="banner banner-error">
        <span>⚠ {{ status.lastError }}</span>
        <el-button size="small" type="primary" link :loading="applying" @click="onApply">重试</el-button>
      </div>

      <div class="status-meta">
        <div class="meta-item">
          <span class="meta-label">上次刷新</span>
          <span class="meta-value">{{ formatTime(status?.lastRenderedAt) }}</span>
        </div>
        <div class="meta-item">
          <span class="meta-label">原壁纸</span>
          <span class="meta-value path" :title="status?.originalPath ?? ''">
            {{ status?.originalPath ?? "—" }}
          </span>
        </div>
      </div>

      <!-- design §11.2：当前合成图缩略图，点击放大预览 -->
      <div v-if="status?.lastRenderedPath" class="thumb">
        <img :src="thumbUrl" alt="当前合成图" @click="onThumbPreview" />
      </div>
      <el-image-viewer
        v-if="thumbViewer"
        :url-list="[thumbUrl]"
        :hide-on-click-modal="true"
        @close="thumbViewer = false"
      />

      <div class="status-buttons">
        <el-button :loading="applying" :disabled="!config.enabled" @click="onApply">
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
        <el-button :loading="restoring" @click="onRestore">恢复原壁纸</el-button>
      </div>
    </section>

    <!-- 配置分组 -->
    <el-tabs v-model="activeTab" class="config-tabs">
      <el-tab-pane label="基础" name="basic">
        <el-form label-width="120px" label-position="left">
          <el-form-item label="贴边位置">
            <el-radio-group v-model="config.position" @change="saveField('position')">
              <el-radio-button label="right">右侧</el-radio-button>
              <el-radio-button label="left" disabled>左侧（v2）</el-radio-button>
              <el-radio-button label="top" disabled>顶部（v2）</el-radio-button>
              <el-radio-button label="bottom" disabled>底部（v2）</el-radio-button>
            </el-radio-group>
          </el-form-item>
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
          </el-form-item>
          <el-form-item label="风格">
            <el-radio-group v-model="config.style" @change="saveField('style')">
              <el-radio-button label="dashboard">仪表盘</el-radio-button>
              <el-radio-button label="sticky" disabled>便利贴（v2）</el-radio-button>
              <el-radio-button label="banner" disabled>横幅（v2）</el-radio-button>
            </el-radio-group>
          </el-form-item>
        </el-form>
      </el-tab-pane>

      <el-tab-pane label="隐私与老板键" name="privacy">
        <el-form label-width="120px" label-position="left">
          <el-form-item label="老板键">
            <el-input
              v-model="config.bossKey"
              placeholder="例：Ctrl+Alt+W"
              style="max-width: 280px"
            />
            <el-button class="ml" @click="saveField('bossKey')">保存</el-button>
            <div class="hint">需重启应用才能完整生效；冲突时面板会显示警告</div>
          </el-form-item>
          <el-form-item label="退出策略">
            <el-radio-group v-model="config.exitBehavior" @change="saveField('exitBehavior')">
              <el-radio-button label="restore_original">恢复原图</el-radio-button>
              <el-radio-button label="keep_last">保留最后一帧</el-radio-button>
            </el-radio-group>
          </el-form-item>
          <el-form-item label="敏感模式">
            <el-switch
              :model-value="config.privacyMask"
              @update:model-value="onPrivacyToggle"
            />
            <span class="hint ml">开启后 todo 标题打码（▓▓▓）</span>
          </el-form-item>
          <el-form-item v-if="config.privacyMask" label="自动到期">
            <el-radio-group
              :model-value="privacyDurationChoice"
              @update:model-value="onPrivacyDurationChange"
            >
              <el-radio-button :label="30">30 分钟</el-radio-button>
              <el-radio-button :label="120">2 小时</el-radio-button>
              <el-radio-button :label="0">直到手动关</el-radio-button>
            </el-radio-group>
            <span v-if="config.privacyMaskUntil" class="hint ml">将于 {{ formatTime(config.privacyMaskUntil) }} 自动关闭</span>
          </el-form-item>
          <el-form-item label="全屏切净">
            <el-input
              type="textarea"
              :rows="3"
              :model-value="config.fullscreenBlacklist.join('\n')"
              placeholder="每行一个 .exe 名"
              @update:model-value="onBlacklistInput"
            />
            <el-button class="mt" @click="saveField('fullscreenBlacklist')">保存</el-button>
          </el-form-item>
        </el-form>
      </el-tab-pane>

      <el-tab-pane label="高级" name="advanced">
        <el-form label-width="120px" label-position="left">
          <el-form-item label="图片格式">
            <el-radio-group v-model="config.imageFormat" @change="saveField('imageFormat')">
              <el-radio-button label="jpeg">JPEG（推荐）</el-radio-button>
              <el-radio-button label="png">PNG</el-radio-button>
            </el-radio-group>
          </el-form-item>
          <el-form-item label="历史保留张数">
            <el-input-number
              v-model="config.keepHistoryCount"
              :min="1"
              :max="200"
              @change="saveField('keepHistoryCount')"
            />
          </el-form-item>
          <el-form-item label="合成历史">
            <div class="history-grid">
              <div
                v-for="entry in history"
                :key="entry.path"
                class="history-cell"
                :title="entry.path"
                @click="onHistoryPreview(entry)"
              >
                <img :src="historyUrl(entry.path)" alt="历史合成图" />
                <span class="history-meta">{{ formatTime(entry.createdAt) }}</span>
              </div>
              <div v-if="history.length === 0" class="hint">暂无合成历史</div>
            </div>
            <el-button size="small" class="mt" @click="refreshHistory">刷新列表</el-button>
            <el-image-viewer
              v-if="historyViewer"
              :url-list="historyUrls"
              :initial-index="historyViewerIndex"
              :hide-on-click-modal="true"
              @close="historyViewer = false"
            />
          </el-form-item>
          <el-form-item label="重置所有">
            <el-button type="danger" @click="onReset">重置壁纸偏好 + 恢复原图</el-button>
            <div class="hint">所有 wallpaper.* 设置回默认值，并立即恢复原壁纸</div>
          </el-form-item>
        </el-form>
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, computed, ref } from "vue";
import { ElMessage, ElMessageBox, ElImageViewer } from "element-plus";
import { convertFileSrc } from "@tauri-apps/api/core";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  WallpaperConfig,
  WallpaperHistoryEntry,
  WallpaperPauseReason,
  WallpaperStatus,
} from "../types/wallpaper";

const status = ref<WallpaperStatus | null>(null);
const config = ref<WallpaperConfig>(defaultConfig());
const activeTab = ref<"basic" | "privacy" | "advanced">("basic");

const toggling = ref(false);
const applying = ref(false);
const restoring = ref(false);

let pollHandle: number | null = null;

const history = ref<WallpaperHistoryEntry[]>([]);
const thumbViewer = ref(false);
const historyViewer = ref(false);
const historyViewerIndex = ref(0);

const thumbUrl = computed(() => {
  const path = status.value?.lastRenderedPath;
  if (!path) return "";
  return convertFileSrc(path) + "?t=" + (status.value?.lastRenderedAt ?? "");
});

const historyUrls = computed(() => history.value.map((h) => historyUrl(h.path)));

function historyUrl(path: string): string {
  return convertFileSrc(path);
}

onMounted(async () => {
  await Promise.all([refreshStatus(), refreshConfig(), refreshHistory()]);
  pollHandle = window.setInterval(refreshStatus, 5000);
});

onBeforeUnmount(() => {
  if (pollHandle !== null) {
    window.clearInterval(pollHandle);
    pollHandle = null;
  }
});

const statusLabel = computed(() => {
  if (!status.value) return "加载中…";
  if (!config.value.enabled) return "未启用";
  if (status.value.paused) return "已暂停";
  return "运行中";
});

const statusDotClass = computed(() => {
  if (!config.value.enabled) return "off";
  if (status.value?.paused) return "warn";
  if (status.value?.lastError) return "error";
  return "ok";
});

async function refreshStatus() {
  try {
    const v = (await invokeToolByChannel("tool:wallpaper:status", {})) as WallpaperStatus;
    status.value = v;
  } catch (e) {
    console.warn("[wallpaper] refresh status failed", e);
  }
}

async function refreshConfig() {
  try {
    const v = (await invokeToolByChannel("tool:wallpaper:get-config", {})) as WallpaperConfig;
    config.value = { ...defaultConfig(), ...v };
  } catch (e) {
    console.warn("[wallpaper] read config failed", e);
  }
}

async function onToggleEnabled(next: boolean) {
  toggling.value = true;
  try {
    if (next) {
      await invokeToolByChannel("tool:wallpaper:enable", {});
    } else {
      await invokeToolByChannel("tool:wallpaper:disable", {});
    }
    config.value.enabled = next;
    ElMessage.success(next ? "已启用桌面壁纸" : "已关闭桌面壁纸");
    await Promise.all([refreshStatus(), refreshConfig()]);
  } catch (e) {
    ElMessage.error(`切换失败：${formatError(e)}`);
  } finally {
    toggling.value = false;
  }
}

async function onApply() {
  applying.value = true;
  try {
    await invokeToolByChannel("tool:wallpaper:apply", {});
    ElMessage.success("已刷新壁纸");
    await refreshStatus();
  } catch (e) {
    ElMessage.error(`刷新失败：${formatError(e)}`);
  } finally {
    applying.value = false;
  }
}

async function onPause() {
  try {
    await invokeToolByChannel("tool:wallpaper:pause", { reason: "manual" });
    await refreshStatus();
  } catch (e) {
    ElMessage.error(`暂停失败：${formatError(e)}`);
  }
}

async function onResume() {
  try {
    await invokeToolByChannel("tool:wallpaper:resume", {});
    await refreshStatus();
  } catch (e) {
    ElMessage.error(`恢复失败：${formatError(e)}`);
  }
}

async function onRestore() {
  restoring.value = true;
  try {
    await invokeToolByChannel("tool:wallpaper:restore", {});
    ElMessage.success("已恢复原壁纸");
    await refreshStatus();
  } catch (e) {
    ElMessage.error(`恢复失败：${formatError(e)}`);
  } finally {
    restoring.value = false;
  }
}

async function saveField(field: keyof WallpaperConfig) {
  const payload: Record<string, unknown> = { [field]: config.value[field] };
  try {
    await invokeToolByChannel("tool:wallpaper:set-config", payload);
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
}

function pauseReasonLabel(reason: WallpaperPauseReason): string {
  switch (reason) {
    case "boss_key":
      return "老板键";
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
  // ISO 字符串里通常带时区；本地化显示交给浏览器
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

async function refreshHistory() {
  try {
    const v = (await invokeToolByChannel("tool:wallpaper:list-history", {})) as {
      items: WallpaperHistoryEntry[];
    };
    history.value = v.items.slice(0, 12);
  } catch (e) {
    console.warn("[wallpaper] list history failed", e);
  }
}

function onThumbPreview() {
  if (!status.value?.lastRenderedPath) return;
  thumbViewer.value = true;
}

function onHistoryPreview(entry: WallpaperHistoryEntry) {
  const idx = history.value.findIndex((h) => h.path === entry.path);
  historyViewerIndex.value = idx >= 0 ? idx : 0;
  historyViewer.value = true;
}

async function onReset() {
  try {
    await ElMessageBox.confirm(
      "重置后所有壁纸偏好将丢失，原壁纸会立即恢复，是否继续？",
      "重置壁纸设置",
      {
        type: "warning",
        confirmButtonText: "重置",
        cancelButtonText: "取消",
      },
    );
  } catch {
    return;
  }
  try {
    await invokeToolByChannel("tool:wallpaper:set-config", {
      enabled: false,
      style: "dashboard",
      position: "right",
      refreshIntervalMin: 15,
      fullscreenBlacklist: ["obs64.exe", "obs32.exe", "powerpnt.exe", "wpp.exe", "zoom.exe"],
      privacyMask: false,
      privacyMaskUntil: null,
      exitBehavior: "restore_original",
      bossKey: "Ctrl+Alt+W",
      imageFormat: "jpeg",
      keepHistoryCount: 20,
    });
    try {
      await invokeToolByChannel("tool:wallpaper:restore", {});
    } catch (e) {
      console.warn("[wallpaper] reset restore failed", e);
    }
    ElMessage.success("已重置壁纸偏好");
    await Promise.all([refreshConfig(), refreshStatus(), refreshHistory()]);
  } catch (e) {
    ElMessage.error("重置失败：" + formatError(e));
  }
}

/** design §9：开关敏感模式（默认 2h 自动关）。后端会写 enabled + until 两个字段。 */
async function onPrivacyToggle(next: boolean) {
  try {
    const payload = next ? { enabled: true, durationMin: 120 } : { enabled: false };
    await invokeToolByChannel("tool:wallpaper:set-privacy-mask", payload);
    ElMessage.success(next ? "已开启敏感模式（默认 2 小时）" : "已关闭敏感模式");
    await Promise.all([refreshConfig(), refreshStatus()]);
  } catch (e) {
    ElMessage.error(`切换失败：${formatError(e)}`);
  }
}

async function onPrivacyOff() {
  try {
    await invokeToolByChannel("tool:wallpaper:set-privacy-mask", { enabled: false });
    ElMessage.success("已关闭敏感模式");
    await Promise.all([refreshConfig(), refreshStatus()]);
  } catch (e) {
    ElMessage.error(`关闭失败：${formatError(e)}`);
  }
}

/** durationMin: 30 / 120 / 0（=直到手动关） */
async function onPrivacyDurationChange(min: string | number | boolean | undefined) {
  const n = typeof min === "number" ? min : Number(min ?? 0);
  try {
    await invokeToolByChannel("tool:wallpaper:set-privacy-mask", { enabled: true, durationMin: n });
    ElMessage.success(n > 0 ? `已设置 ${n} 分钟后自动关闭` : "已设置直到手动关");
    await Promise.all([refreshConfig(), refreshStatus()]);
  } catch (e) {
    ElMessage.error(`保存失败：${formatError(e)}`);
  }
}

const privacyDurationChoice = computed<number>(() => {
  if (!config.value.privacyMaskUntil) return 0;
  const remainMs = new Date(config.value.privacyMaskUntil).getTime() - Date.now();
  if (remainMs <= 0) return 0;
  // 容差 5 分钟内的归类：30 → ≤45min；120 → 45<x≤180；其他 → 0
  const remainMin = Math.round(remainMs / 60000);
  if (remainMin <= 45) return 30;
  if (remainMin <= 180) return 120;
  return 0;
});

const privacyUntilLabel = computed<string>(() => {
  if (!status.value?.privacyMaskUntil) return "（直到手动关）";
  return `（将于 ${formatTime(status.value.privacyMaskUntil)} 自动关闭）`;
});

function defaultConfig(): WallpaperConfig {
  return {
    enabled: false,
    style: "dashboard",
    position: "right",
    refreshIntervalMin: 15,
    fullscreenBlacklist: [],
    privacyMask: false,
    privacyMaskUntil: null,
    exitBehavior: "restore_original",
    bossKey: "Ctrl+Alt+W",
    imageFormat: "jpeg",
    keepHistoryCount: 20,
  };
}
</script>

<style scoped>
.wallpaper-panel {
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

.meta-value.path {
  font-family: ui-monospace, SFMono-Regular, "Cascadia Mono", monospace;
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.meta-error {
  color: #ef4444;
  font-size: 12px;
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

.thumb {
  margin-bottom: 12px;
}
.thumb img {
  max-width: 320px;
  max-height: 200px;
  border-radius: 8px;
  border: 1px solid var(--el-border-color-lighter);
  cursor: zoom-in;
  display: block;
}

.history-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 8px;
  width: 100%;
}
.history-cell {
  display: flex;
  flex-direction: column;
  gap: 4px;
  cursor: zoom-in;
  border-radius: 8px;
  overflow: hidden;
  border: 1px solid var(--el-border-color-lighter);
  background: var(--el-fill-color-lighter);
  padding: 4px;
}
.history-cell img {
  width: 100%;
  aspect-ratio: 16 / 9;
  object-fit: cover;
  border-radius: 6px;
  background: #000;
}
.history-meta {
  font-size: 11px;
  color: var(--el-text-color-secondary);
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
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
</style>

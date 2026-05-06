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
        <div v-if="status?.lastError" class="meta-error" :title="status.lastError">
          ⚠ {{ status.lastError }}
        </div>
      </div>

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
              v-model="config.privacyMask"
              @change="saveField('privacyMask')"
            />
            <span class="hint ml">开启后标题打码（▓▓▓）</span>
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
        </el-form>
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, computed, ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  WallpaperConfig,
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

onMounted(async () => {
  await Promise.all([refreshStatus(), refreshConfig()]);
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

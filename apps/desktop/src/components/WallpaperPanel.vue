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

      <!-- 调度自动跳过透出（review #11）：锁屏 / 全屏期间桌面不刷新，给用户一个解释 -->
      <div v-if="autoSkipBanner" class="banner banner-info">
        <span>⏸ {{ autoSkipBanner }}</span>
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
          :loading="restoring"
          :disabled="!status?.originalPath"
          :title="!status?.originalPath ? '尚未备份原壁纸，请先启用一次以建立备份' : ''"
          @click="onRestore"
        >
          恢复原壁纸
        </el-button>
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
            <el-button class="ml" :loading="rebindingBossKey" @click="onSaveBossKey">保存</el-button>
            <div class="hint">保存后立即生效；若快捷键被其它程序占用，状态卡片会提示重设</div>
          </el-form-item>
          <el-form-item label="退出策略">
            <el-radio-group v-model="config.exitBehavior" @change="saveField('exitBehavior')">
              <el-radio-button label="restore_original">恢复原图</el-radio-button>
              <el-radio-button label="keep_last">保留最后一帧</el-radio-button>
            </el-radio-group>
          </el-form-item>
          <el-form-item label="敏感模式">
            <!-- review #7：把"开关 + 时长（开启后才显示）"合并为四选一，
                 用户在切到 ON 那一刻即决定时长，不再被默认 2h 锁定 -->
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
              将于 {{ formatTime(config.privacyMaskUntil) }} 自动关闭
            </div>
            <div v-else class="hint">开启后 todo 标题打码（▓▓▓）</div>
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
import { invokeToolByChannel, registerNamedHotkey } from "../bridge/tauri";
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
const rebindingBossKey = ref(false);

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
  if (next && !status.value?.originalPath) {
    // review #12：首次启用（尚未备份原壁纸）先弹说明，让用户知道接下来会发生什么
    try {
      await ElMessageBox.confirm(
        `桌面壁纸工具会把今日仪表盘叠加到桌面右侧 360×800 区域，并自动备份你当前的壁纸。退出 LazyCat 时按下方「退出策略」处理（默认：恢复原图）。继续？`,
        "首次启用桌面壁纸",
        {
          type: "info",
          confirmButtonText: "继续启用",
          cancelButtonText: "取消",
        },
      );
    } catch {
      // 用户取消：不要让 toggle 视觉状态卡在 ON
      config.value.enabled = false;
      return;
    }
  }

  toggling.value = true;
  try {
    if (next) {
      await invokeToolByChannel("tool:wallpaper:enable", {});
      config.value.enabled = true;
      ElMessage.success("已启用桌面壁纸，正在合成首帧…");
      await Promise.all([refreshStatus(), refreshConfig()]);
      // 启用后立即合成一次，避免用户等到下一个心跳（默认 15 min）才看到效果
      // 异步触发不阻塞 toggle UI；失败时单独提示
      invokeToolByChannel("tool:wallpaper:apply", {})
        .then(() => refreshStatus())
        .catch((e) => ElMessage.warning(`首帧合成失败：${formatError(e)}`));
    } else {
      await invokeToolByChannel("tool:wallpaper:disable", {});
      config.value.enabled = false;
      ElMessage.success("已关闭桌面壁纸");
      await Promise.all([refreshStatus(), refreshConfig()]);
    }
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
    ElMessage.success("已恢复原壁纸（自动暂停刷新，重新启用请按上方开关）");
    await Promise.all([refreshStatus(), refreshConfig()]);
  } catch (e) {
    ElMessage.error(`恢复失败：${humanizeWallpaperError(e)}`);
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

/**
 * 保存老板键 + 立即热重绑（review #8）。
 * 步骤：set-config 持久化 → 调 register_named_hotkey 实际注册 → 写状态卡片错误位
 *   - 成功：清 boss_key_error，ElMessage success
 *   - 失败：把后端 register 的 Err（如"快捷键已被占用"）写进 boss_key_error，状态卡片显眼透出
 */
async function onSaveBossKey() {
  const trimmed = config.value.bossKey.trim();
  if (!trimmed) {
    ElMessage.warning("老板键不能为空");
    return;
  }
  rebindingBossKey.value = true;
  try {
    await invokeToolByChannel("tool:wallpaper:set-config", { bossKey: trimmed });
    try {
      await registerNamedHotkey("wallpaper-boss-key", trimmed);
      await invokeToolByChannel("tool:wallpaper:set-boss-key-error", { error: null });
      ElMessage.success(`老板键已生效：${trimmed}`);
    } catch (e) {
      const errMsg = `老板键 ${trimmed} 注册失败：${formatError(e)}`;
      await invokeToolByChannel("tool:wallpaper:set-boss-key-error", { error: errMsg });
      ElMessage.error(errMsg);
    }
    await refreshStatus();
  } catch (e) {
    ElMessage.error(`保存失败：${formatError(e)}`);
  } finally {
    rebindingBossKey.value = false;
  }
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

/**
 * 把后端壁纸模块的英文 Err 翻译成中文，用户更友好。
 * 未识别时回落 formatError 原文，避免吞错。
 */
function humanizeWallpaperError(e: unknown): string {
  const raw = formatError(e);
  if (raw.includes("no original wallpaper backed up")) {
    return "尚未备份原壁纸，请先启用一次以建立备份";
  }
  if (raw.includes("original wallpaper backup missing")) {
    return "原壁纸备份文件已丢失，请到设置中重新选择壁纸后再试";
  }
  return raw;
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
    // 1. 先走 disable：销毁 hidden WebView + 按 exit_behavior 恢复原图
    //    （注意 set_config 已禁止直接写 enabled，此处必须走 disable channel）
    await invokeToolByChannel("tool:wallpaper:disable", { restore: true });
    // 2. 再写其余偏好回默认
    await invokeToolByChannel("tool:wallpaper:set-config", {
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
    ElMessage.success("已重置壁纸偏好");
    await Promise.all([refreshConfig(), refreshStatus(), refreshHistory()]);
  } catch (e) {
    ElMessage.error("重置失败：" + humanizeWallpaperError(e));
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

/**
 * 四选一时长直接驱动开关（review #7）：用户选时长那一刻就开启 + 设到期。
 * - "off"  → 关闭敏感模式
 * - 30 / 120 / 0 → 开启 + 写到期分钟（0 = 直到手动关）
 */
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
    await invokeToolByChannel("tool:wallpaper:set-privacy-mask", {
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
  const remainMs = new Date(config.value.privacyMaskUntil).getTime() - Date.now();
  if (remainMs <= 0) return 0;
  // 容差 5 分钟内的归类：30 → ≤45min；120 → 45<x≤180；其他 → 0
  const remainMin = Math.round(remainMs / 60000);
  if (remainMin <= 45) return 30;
  if (remainMin <= 180) return 120;
  return 0;
});

/**
 * 四选一选项：'off' | 30 | 120 | 0（review #7）。
 * privacyMask=false → 'off'；否则按剩余时长归类到 30 / 120 / 0（直到手动关）。
 */
const privacyChoice = computed<"off" | number>(() => {
  if (!config.value.privacyMask) return "off";
  return privacyDurationChoice.value;
});

const privacyUntilLabel = computed<string>(() => {
  if (!status.value?.privacyMaskUntil) return "（直到手动关）";
  return `（将于 ${formatTime(status.value.privacyMaskUntil)} 自动关闭）`;
});

/**
 * 调度自动跳过原因的中文文案（review #11）。
 * 仅在未显式暂停 / 已启用时才显示——显式暂停由 banner+按钮透出，无需重复说明。
 */
const autoSkipBanner = computed<string>(() => {
  if (!config.value.enabled) return "";
  if (status.value?.paused) return "";
  switch (status.value?.autoSkipReason) {
    case "lock":
      return "检测到锁屏 / 屏保，已暂停刷新（解锁后自动恢复）";
    case "fullscreen":
      return "检测到全屏应用，已暂停刷新（退出全屏后自动恢复）";
    default:
      return "";
  }
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

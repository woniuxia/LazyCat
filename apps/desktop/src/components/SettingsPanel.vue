<template>
  <div class="settings-panel">
    <div class="settings-container">
      <!-- 菜单设置 -->
      <section class="settings-section">
        <div class="section-header">
          <div class="section-icon">📋</div>
          <div class="section-title">
            <h3>菜单</h3>
            <p>自定义侧边栏显示的工具</p>
          </div>
        </div>
        <div class="section-content">
          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">菜单显示</span>
              <span class="label-desc">自定义侧边栏显示的工具</span>
            </div>
            <div class="setting-control">
              <el-button @click="menuVisibilityDialog?.show()">配置显示项</el-button>
            </div>
          </div>

          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">常用工具显示数量</span>
              <span class="label-desc">首页"常用工具"区域显示的工具总数（收藏全部优先显示，其余按近30天点击次数排序）</span>
            </div>
            <div class="setting-control">
              <el-input-number
                :model-value="homeTopLimit"
                :min="1"
                :max="50"
                :step="1"
                controls-position="right"
                style="width: 120px"
                @update:model-value="(v) => emit('update:homeTopLimit', v ?? 12)"
              />
            </div>
          </div>
        </div>
      </section>

      <!-- 快捷键设置 -->
      <section class="settings-section">
        <div class="section-header">
          <div class="section-icon">⌨️</div>
          <div class="section-title">
            <h3>快捷键</h3>
            <p>设置全局快捷键，在任意位置快速调用功能</p>
          </div>
        </div>
        <div class="section-content">
          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">显示/隐藏窗口</span>
            </div>
            <div class="setting-control">
              <ShortcutRecorder
                :model-value="hotkeyInput"
                :check-conflict="makeConflictChecker('hotkeyInput')"
                @update:model-value="emit('update:hotkeyInput', $event)"
              />
            </div>
          </div>

          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">代码片段</span>
            </div>
            <div class="setting-control">
              <ShortcutRecorder
                :model-value="snippetsHotkeyInput"
                :check-conflict="makeConflictChecker('snippetsHotkeyInput')"
                @update:model-value="emit('update:snippetsHotkeyInput', $event)"
              />
            </div>
          </div>

          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">密码管理</span>
            </div>
            <div class="setting-control">
              <ShortcutRecorder
                :model-value="vaultHotkeyInput"
                :check-conflict="makeConflictChecker('vaultHotkeyInput')"
                @update:model-value="emit('update:vaultHotkeyInput', $event)"
              />
            </div>
          </div>

          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">快捷启动</span>
            </div>
            <div class="setting-control">
              <ShortcutRecorder
                :model-value="launcherHotkeyInput"
                :check-conflict="makeConflictChecker('launcherHotkeyInput')"
                @update:model-value="emit('update:launcherHotkeyInput', $event)"
              />
            </div>
          </div>

          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">任务清单</span>
            </div>
            <div class="setting-control">
              <ShortcutRecorder
                :model-value="todoHotkeyInput"
                :check-conflict="makeConflictChecker('todoHotkeyInput')"
                @update:model-value="emit('update:todoHotkeyInput', $event)"
              />
            </div>
          </div>

          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">快速捕获</span>
            </div>
            <div class="setting-control">
              <ShortcutRecorder
                :model-value="quickCaptureHotkeyInput"
                :check-conflict="makeConflictChecker('quickCaptureHotkeyInput')"
                @update:model-value="emit('update:quickCaptureHotkeyInput', $event)"
              />
            </div>
          </div>

          <div class="setting-actions">
            <el-button type="primary" @click="saveHotkeySettings">保存快捷键</el-button>
            <el-button @click="clearHotkeySettings">清除全部</el-button>
          </div>
        </div>
      </section>

      <!-- 加密与安全 -->
      <section class="settings-section">
        <div class="section-header">
          <div class="section-icon">🔐</div>
          <div class="section-title">
            <h3>加密与安全</h3>
            <p>配置密码管理的自动锁定策略</p>
          </div>
        </div>
        <div class="section-content">
          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">密码库锁定预设</span>
              <span class="label-desc">推荐使用“平衡”，兼顾临时离开时的保护与连续使用体验</span>
            </div>
            <div class="setting-control setting-control-column vault-lock-profile-control">
              <el-radio-group v-model="vaultLockProfile" @change="handleVaultLockProfileChange">
                <el-radio-button value="strict">严格</el-radio-button>
                <el-radio-button value="balanced">平衡</el-radio-button>
                <el-radio-button value="convenient">便捷</el-radio-button>
              </el-radio-group>
              <span class="setting-inline-hint">{{ vaultLockProfileHint }}</span>
              <div class="vault-lock-explainer">
                <div class="vault-lock-explainer-item">
                  <span class="vault-lock-explainer-title">敏感信息隐藏</span>
                  <span class="vault-lock-explainer-desc">隐藏当前已展示的密码等敏感内容，减少明文在屏幕上停留的时间。</span>
                </div>
                <div class="vault-lock-explainer-item">
                  <span class="vault-lock-explainer-title">自动硬锁</span>
                  <span class="vault-lock-explainer-desc">空闲达到阈值后彻底锁定密码库并清空当前解锁会话，需要重新解锁后才能继续访问。</span>
                </div>
                <div class="vault-lock-explainer-item">
                  <span class="vault-lock-explainer-title">失焦隐藏</span>
                  <span class="vault-lock-explainer-desc">窗口失去焦点时立即恢复密码等敏感内容的掩码显示，但不会触发锁定。</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      <!-- 系统集成 -->
      <section class="settings-section">
        <div class="section-header">
          <div class="section-icon">⚙️</div>
          <div class="section-title">
            <h3>系统集成</h3>
            <p>配置应用与操作系统的交互行为</p>
          </div>
        </div>
        <div class="section-content">
          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">开机自启动</span>
              <span class="label-desc">应用将在系统启动时自动运行</span>
            </div>
            <div class="setting-control">
              <el-switch
                v-model="autostartEnabled"
                @change="handleAutostartChange"
              />
            </div>
          </div>

          <div class="setting-item" :class="{ 'is-disabled': !autostartEnabled }">
            <div class="setting-label">
              <span class="label-text">启动时最小化到托盘</span>
              <span class="label-desc">仅在开机自启动时生效，手动启动时始终显示窗口</span>
            </div>
            <div class="setting-control">
              <el-switch
                v-model="autostartMinimized"
                :disabled="!autostartEnabled"
                @change="handleAutostartMinimizedChange"
              />
            </div>
          </div>

          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">关闭时最小化到托盘</span>
              <span class="label-desc">关闭时隐藏到系统托盘而非退出应用</span>
            </div>
            <div class="setting-control">
              <el-switch
                v-model="closeToTray"
                @change="handleCloseToTrayChange"
              />
            </div>
          </div>
        </div>
      </section>

      <!-- 智能助手 -->
      <section class="settings-section">
        <div class="section-header">
          <div class="section-icon">&#x1F50D;</div>
          <div class="section-title">
            <h3>智能助手</h3>
            <p>自动检测剪贴板内容并提供快捷操作</p>
          </div>
        </div>
        <div class="section-content">
          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">剪贴板智能检测</span>
              <span class="label-desc">窗口激活时自动检测剪贴板内容类型，提供一键跳转</span>
            </div>
            <div class="setting-control">
              <el-switch
                v-model="clipboardDetection"
                @change="handleClipboardDetectionChange"
              />
            </div>
          </div>
        </div>
      </section>

      <section class="settings-section">
        <div class="section-header">
          <div class="section-icon">📥</div>
          <div class="section-title">
            <h3>收纳箱</h3>
            <p>后台采集剪贴板历史，并整理到历史流 / 收件箱</p>
          </div>
        </div>
        <div class="section-content">
          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">后台采集</span>
              <span class="label-desc">应用运行期间记录最近复制内容，可随时关闭</span>
            </div>
            <div class="setting-control">
              <el-switch
                v-model="inboxCaptureEnabled"
                @change="handleInboxCaptureEnabledChange"
              />
            </div>
          </div>

          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">隐藏时继续采集</span>
              <span class="label-desc">主窗口隐藏到托盘或最小化后是否继续记录</span>
            </div>
            <div class="setting-control">
              <el-switch
                v-model="inboxCaptureWhenHidden"
                @change="handleInboxCaptureWhenHiddenChange"
              />
            </div>
          </div>

          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">历史保留天数</span>
              <span class="label-desc">仅对历史流生效，收件箱和已归档默认长期保留</span>
            </div>
            <div class="setting-control">
              <el-input-number
                v-model="inboxHistoryRetentionDays"
                :min="1"
                :max="365"
                @change="handleInboxHistoryRetentionChange"
              />
            </div>
          </div>

          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">临时暂停采集</span>
              <span class="label-desc">
                {{ inboxPausedLabel || "暂停 5 分钟，恢复后继续按当前设置采集" }}
              </span>
            </div>
            <div class="setting-control setting-control-column">
              <div class="import-export-actions">
                <el-button @click="handlePauseInboxCapture">暂停 5 分钟</el-button>
                <el-button v-if="inboxPaused" @click="handleResumeInboxCapture">立即恢复</el-button>
              </div>
            </div>
          </div>
        </div>
      </section>

      <!-- 数据管理 -->
      <section class="settings-section">
        <div class="section-header">
          <div class="section-icon">💾</div>
          <div class="section-title">
            <h3>数据管理</h3>
            <p>管理应用数据的存储位置和备份</p>
          </div>
        </div>
        <div class="section-content">
          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">数据目录</span>
              <span class="label-desc">应用数据（数据库、Hosts 备份）存储位置，更改后需重启</span>
            </div>
            <div class="setting-control setting-control-column">
              <div class="data-dir-input">
                <el-input
                  :model-value="dataDirPath"
                  readonly
                  placeholder="加载中..."
                />
                <el-button @click="handleChangeDataDir">更改</el-button>
                <el-button
                  v-if="dataDirIsCustom"
                  @click="handleResetDataDir"
                >恢复默认</el-button>
              </div>
            </div>
          </div>

          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">导入/导出</span>
              <span class="label-desc">备份或恢复应用数据（设置、收藏、使用记录、Hosts 配置）</span>
            </div>
            <div class="setting-control setting-control-column">
              <div class="import-export-actions">
                <el-button type="primary" @click="handleExport">导出数据</el-button>
                <el-button @click="handleImport">导入数据</el-button>
                <el-radio-group v-model="importMode" size="small">
                  <el-radio-button value="merge">合并</el-radio-button>
                  <el-radio-button value="overwrite">覆盖</el-radio-button>
                </el-radio-group>
              </div>
            </div>
          </div>
        </div>
      </section>
    </div>

    <MenuVisibilityDialog
      ref="menuVisibilityDialog"
      :sidebar-items="sidebarItems"
      :get-hidden-ids="getHiddenIds"
      :set-hidden-ids="setHiddenIds"
      :get-tool-search-meta-map="getToolSearchMetaMap"
      :set-tool-search-meta-map="setToolSearchMetaMap"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { save, open } from "@tauri-apps/plugin-dialog";
import { registerHotkey, unregisterHotkey, registerNamedHotkey, unregisterNamedHotkey, invokeToolByChannel } from "../bridge/tauri";
import {
  DEFAULT_VAULT_LOCK_PROFILE,
  getSetting,
  getVaultLockProfile,
  getVaultLockProfilePolicy,
  setSetting,
  setVaultLockProfile,
  useAutostartSettings,
  type VaultLockProfile,
} from "../composables/useSettings";
import type { SidebarItem, ToolSearchMetaMap } from "../types";
import MenuVisibilityDialog from "./MenuVisibilityDialog.vue";
import ShortcutRecorder from "./ShortcutRecorder.vue";

const {
  autostartEnabled,
  autostartMinimized,
  closeToTray,
  enableAutostart,
  disableAutostart,
  setAutostartMinimized,
  setCloseToTray,
} = useAutostartSettings();

const props = defineProps<{
  hotkeyInput: string;
  snippetsHotkeyInput: string;
  vaultHotkeyInput: string;
  launcherHotkeyInput: string;
  todoHotkeyInput: string;
  quickCaptureHotkeyInput: string;
  homeTopLimit: number;
  sidebarItems: SidebarItem[];
  getHiddenIds: () => string[];
  setHiddenIds: (ids: string[]) => void;
  getToolSearchMetaMap: () => ToolSearchMetaMap;
  setToolSearchMetaMap: (map: ToolSearchMetaMap) => void;
}>();

const emit = defineEmits<{
  (event: "update:hotkeyInput", value: string): void;
  (event: "update:snippetsHotkeyInput", value: string): void;
  (event: "update:vaultHotkeyInput", value: string): void;
  (event: "update:launcherHotkeyInput", value: string): void;
  (event: "update:todoHotkeyInput", value: string): void;
  (event: "update:quickCaptureHotkeyInput", value: string): void;
  (event: "update:homeTopLimit", value: number): void;
}>();

const importMode = ref<"merge" | "overwrite">("merge");
const dataDirPath = ref("");
const dataDirIsCustom = ref(false);
const clipboardDetection = ref(true);
const inboxCaptureEnabled = ref(false);
const inboxCaptureWhenHidden = ref(true);
const inboxHistoryRetentionDays = ref(14);
const inboxPaused = ref(false);
const inboxPausedUntil = ref("");
const vaultLockProfile = ref<VaultLockProfile>(DEFAULT_VAULT_LOCK_PROFILE);
const menuVisibilityDialog = ref<InstanceType<typeof MenuVisibilityDialog>>();

const vaultLockProfileHint = computed(() => {
  const policy = getVaultLockProfilePolicy(vaultLockProfile.value);
  return `敏感信息 ${policy.hideSensitiveAfterSecs}s 隐藏，${Math.round(policy.hardLockAfterSecs / 60)} 分钟自动硬锁，失焦立即隐藏敏感信息`;
});

const inboxPausedLabel = computed(() => {
  if (!inboxPaused.value || !inboxPausedUntil.value) return "";
  const until = new Date(inboxPausedUntil.value);
  if (Number.isNaN(until.getTime())) return "当前处于暂停状态";
  return `当前暂停至 ${until.toLocaleString("zh-CN", { hour12: false })}`;
});

const HOTKEY_FIELDS = [
  { key: "hotkeyInput" as const, label: "显示/隐藏" },
  { key: "snippetsHotkeyInput" as const, label: "代码片段" },
  { key: "vaultHotkeyInput" as const, label: "密码管理" },
  { key: "launcherHotkeyInput" as const, label: "快捷启动" },
  { key: "todoHotkeyInput" as const, label: "任务清单" },
  { key: "quickCaptureHotkeyInput" as const, label: "快速捕获" },
] as const;

function makeConflictChecker(selfKey: typeof HOTKEY_FIELDS[number]["key"]) {
  return (shortcut: string): string | undefined => {
    for (const f of HOTKEY_FIELDS) {
      if (f.key !== selfKey && props[f.key] === shortcut) return f.label;
    }
    return undefined;
  };
}

onMounted(async () => {
  await loadDataDir();
  clipboardDetection.value = getSetting("clipboard_detection") !== "false";
  await loadInboxCaptureStatus();
  vaultLockProfile.value = getVaultLockProfile();
});

async function loadInboxCaptureStatus() {
  inboxCaptureEnabled.value = getSetting("inbox_capture_enabled") === "true";
  inboxCaptureWhenHidden.value = getSetting("inbox_capture_when_hidden") !== "false";
  inboxHistoryRetentionDays.value = Number(getSetting("inbox_history_retention_days") || "14");
  try {
    const status = (await invokeToolByChannel("tool:inbox:capture-status", {})) as {
      captureEnabled: boolean;
      captureWhenHidden: boolean;
      historyRetentionDays: number;
      paused: boolean;
      pausedUntil: string | null;
    };
    inboxCaptureEnabled.value = status.captureEnabled;
    inboxCaptureWhenHidden.value = status.captureWhenHidden;
    inboxHistoryRetentionDays.value = status.historyRetentionDays;
    inboxPaused.value = status.paused;
    inboxPausedUntil.value = status.pausedUntil || "";
  } catch {
    inboxPaused.value = false;
    inboxPausedUntil.value = "";
  }
}

async function loadDataDir() {
  try {
    const result = (await invokeToolByChannel("tool:settings:get-data-dir", {})) as {
      dataDir: string;
      isCustom: boolean;
    };
    dataDirPath.value = result.dataDir;
    dataDirIsCustom.value = result.isCustom;
  } catch {
    // IPC unavailable
  }
}

async function saveHotkeySettings() {
  try {
    const toggle = props.hotkeyInput.trim();
    await registerHotkey(toggle);
    setSetting("hotkey", toggle);

    const snippets = props.snippetsHotkeyInput.trim();
    await registerNamedHotkey("snippets", snippets);
    setSetting("hotkey_snippets", snippets);

    const vault = props.vaultHotkeyInput.trim();
    await registerNamedHotkey("vault", vault);
    setSetting("hotkey_vault", vault);

    const launcher = props.launcherHotkeyInput.trim();
    await registerNamedHotkey("launcher", launcher);
    setSetting("hotkey_launcher", launcher);

    const todo = props.todoHotkeyInput.trim();
    await registerNamedHotkey("todo", todo);
    setSetting("hotkey_todo", todo);

    const quickCapture = props.quickCaptureHotkeyInput.trim();
    await registerNamedHotkey("quick-capture", quickCapture);
    setSetting("hotkey_quick_capture", quickCapture);

    ElMessage.success("快捷键已保存");
  } catch (e) {
    ElMessage.error(`保存失败：${(e as Error).message}`);
  }
}

async function clearHotkeySettings() {
  emit("update:hotkeyInput", "");
  emit("update:snippetsHotkeyInput", "");
  emit("update:vaultHotkeyInput", "");
  emit("update:launcherHotkeyInput", "");
  emit("update:todoHotkeyInput", "");
  emit("update:quickCaptureHotkeyInput", "");
  try {
    await unregisterHotkey();
    await unregisterNamedHotkey("snippets");
    await unregisterNamedHotkey("vault");
    await unregisterNamedHotkey("launcher");
    await unregisterNamedHotkey("todo");
    await unregisterNamedHotkey("quick-capture");
    setSetting("hotkey", "");
    setSetting("hotkey_snippets", "");
    setSetting("hotkey_vault", "");
    setSetting("hotkey_launcher", "");
    setSetting("hotkey_todo", "");
    setSetting("hotkey_quick_capture", "");
    ElMessage.success("快捷键已清除");
  } catch (e) {
    ElMessage.error(`清除失败：${(e as Error).message}`);
  }
}

async function handleExport() {
  try {
    const filePath = await save({
      defaultPath: `lazycat-backup-${new Date().toISOString().slice(0, 10)}.json`,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!filePath) return;
    await invokeToolByChannel("tool:settings:export-to-file", { path: filePath });
    ElMessage.success("数据已导出");
  } catch (e) {
    ElMessage.error(`导出失败：${(e as Error).message}`);
  }
}

async function handleImport() {
  try {
    const filePath = await open({
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!filePath) return;
    if (importMode.value === "overwrite") {
      await ElMessageBox.confirm(
        "覆盖模式将清除所有现有数据并替换为导入内容，确定继续？",
        "确认覆盖",
        { type: "warning" },
      );
    }
    await invokeToolByChannel("tool:settings:import-from-file", {
      path: filePath,
      mode: importMode.value,
    });
    ElMessage.success("数据已导入，重启应用后完全生效");
  } catch (e) {
    if ((e as { toString?: () => string })?.toString?.()?.includes("cancel")) return;
    ElMessage.error(`导入失败：${(e as Error).message}`);
  }
}

async function handleChangeDataDir() {
  try {
    const dirPath = await open({
      directory: true,
      multiple: false,
      title: "选择数据目录",
    });
    if (!dirPath) return;
    await ElMessageBox.confirm(
      `将数据迁移到：${dirPath}\n\n迁移后需要重启应用。原目录数据保留作为安全备份。`,
      "确认更改数据目录",
      { type: "warning" },
    );
    await invokeToolByChannel("tool:settings:set-data-dir", { path: dirPath });
    dataDirPath.value = dirPath as string;
    dataDirIsCustom.value = true;
    ElMessage.success("数据目录已更改，请重启应用");
  } catch (e) {
    if ((e as { toString?: () => string })?.toString?.()?.includes("cancel")) return;
    ElMessage.error(`更改失败：${(e as Error).message}`);
  }
}

async function handleResetDataDir() {
  try {
    await ElMessageBox.confirm(
      "恢复为默认数据目录，重启后生效。自定义目录中的数据不会被删除。",
      "确认恢复默认",
      { type: "info" },
    );
    await invokeToolByChannel("tool:settings:reset-data-dir", {});
    await loadDataDir();
    ElMessage.success("已恢复默认数据目录，请重启应用");
  } catch (e) {
    if ((e as { toString?: () => string })?.toString?.()?.includes("cancel")) return;
    ElMessage.error(`恢复失败：${(e as Error).message}`);
  }
}

async function handleAutostartChange(value: boolean) {
  try {
    if (value) {
      await enableAutostart();
      ElMessage.success("已启用开机自启动");
    } else {
      await disableAutostart();
      if (autostartMinimized.value) {
        await setAutostartMinimized(false);
      }
      ElMessage.success("已禁用开机自启动");
    }
  } catch (error) {
    ElMessage.error(`设置失败：${(error as Error).message}`);
    autostartEnabled.value = !value;
  }
}

async function handleAutostartMinimizedChange(value: boolean) {
  try {
    await setAutostartMinimized(value);
    ElMessage.success(value ? "已启用启动时最小化" : "已禁用启动时最小化");
  } catch (error) {
    ElMessage.error(`设置失败：${(error as Error).message}`);
    autostartMinimized.value = !value;
  }
}

async function handleCloseToTrayChange(value: boolean) {
  try {
    await setCloseToTray(value);
    ElMessage.success(value ? "关闭时将最小化到托盘" : "关闭时将退出应用");
  } catch (error) {
    ElMessage.error(`设置失败：${(error as Error).message}`);
    closeToTray.value = !value;
  }
}

function handleClipboardDetectionChange(value: boolean) {
  setSetting("clipboard_detection", value ? "true" : "false");
  ElMessage.success(value ? "已启用剪贴板智能检测" : "已关闭剪贴板智能检测");
}

function handleInboxCaptureEnabledChange(value: boolean) {
  setSetting("inbox_capture_enabled", value ? "true" : "false");
  inboxCaptureEnabled.value = value;
  ElMessage.success(value ? "已启用收纳箱后台采集" : "已关闭收纳箱后台采集");
}

function handleInboxCaptureWhenHiddenChange(value: boolean) {
  setSetting("inbox_capture_when_hidden", value ? "true" : "false");
  inboxCaptureWhenHidden.value = value;
  ElMessage.success(value ? "隐藏后会继续采集" : "隐藏后将暂停采集");
}

function handleInboxHistoryRetentionChange(value: string | number | null | undefined) {
  const normalized = Math.max(1, Math.min(365, Number(value || 14) || 14));
  inboxHistoryRetentionDays.value = normalized;
  setSetting("inbox_history_retention_days", String(normalized));
  ElMessage.success(`历史保留已更新为 ${normalized} 天`);
}

async function handlePauseInboxCapture() {
  try {
    await invokeToolByChannel("tool:inbox:capture-pause", { minutes: 5 });
    await loadInboxCaptureStatus();
    ElMessage.success("收纳箱采集已暂停 5 分钟");
  } catch (error) {
    ElMessage.error(`设置失败：${(error as Error).message}`);
  }
}

async function handleResumeInboxCapture() {
  try {
    await invokeToolByChannel("tool:inbox:capture-pause", { minutes: 0 });
    await loadInboxCaptureStatus();
    ElMessage.success("收纳箱采集已恢复");
  } catch (error) {
    ElMessage.error(`设置失败：${(error as Error).message}`);
  }
}

function handleVaultLockProfileChange(value: string | number | boolean) {
  const nextProfile = (value || DEFAULT_VAULT_LOCK_PROFILE) as VaultLockProfile;
  vaultLockProfile.value = nextProfile;
  setVaultLockProfile(nextProfile);
  ElMessage.success("密码库锁定预设已更新");
}
</script>

<style scoped>
.settings-panel {
  height: 100%;
  overflow-y: auto;
  background: var(--el-bg-color);
}

.settings-container {
  max-width: 900px;
  margin: 0 auto;
  padding: 32px 24px;
}

.settings-section {
  background: var(--el-bg-color-overlay);
  border: 1px solid var(--el-border-color-light);
  border-radius: 12px;
  padding: 24px;
  margin-bottom: 24px;
  transition: all 0.3s ease;
}

.settings-section:hover {
  border-color: var(--el-border-color);
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.04);
}

.section-header {
  display: flex;
  align-items: flex-start;
  gap: 16px;
  margin-bottom: 24px;
  padding-bottom: 20px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}

.section-icon {
  font-size: 32px;
  line-height: 1;
  flex-shrink: 0;
}

.section-title h3 {
  margin: 0 0 4px 0;
  font-size: 18px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}

.section-title p {
  margin: 0;
  font-size: 13px;
  color: var(--el-text-color-secondary);
  line-height: 1.5;
}

.section-content {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.setting-item {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 24px;
  padding: 16px;
  background: var(--el-fill-color-blank);
  border-radius: 8px;
  transition: all 0.2s ease;
  flex-wrap: wrap;
}

.setting-item:hover {
  background: var(--el-fill-color-light);
}

.setting-item.is-disabled {
  opacity: 0.5;
}

.setting-label {
  flex: 1;
  min-width: 0;
}

.label-text {
  display: block;
  font-size: 14px;
  font-weight: 500;
  color: var(--el-text-color-primary);
  margin-bottom: 4px;
}

.label-desc {
  display: block;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  line-height: 1.5;
}

.setting-control {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 8px;
}

.setting-control-column {
  flex: 1;
  min-width: 0;
  flex-direction: column;
  align-items: stretch;
}

.vault-lock-profile-control {
  gap: 10px;
}

.vault-lock-explainer {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 12px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 10px;
  background: var(--el-fill-color-extra-light);
}

.vault-lock-explainer-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.vault-lock-explainer-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}

.vault-lock-explainer-desc {
  font-size: 12px;
  line-height: 1.6;
  color: var(--el-text-color-secondary);
}

.setting-inline-hint {
  font-size: 12px;
  line-height: 1.6;
  color: var(--el-text-color-secondary);
}

.setting-actions {
  display: flex;
  gap: 12px;
  padding-top: 8px;
  justify-content: flex-end;
}

.data-dir-input {
  display: flex;
  gap: 8px;
  align-items: center;
}

.data-dir-input .el-input {
  flex: 1;
}

.import-export-actions {
  display: flex;
  gap: 12px;
  align-items: center;
  flex-wrap: wrap;
}
</style>

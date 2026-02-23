<template>
  <div class="settings-panel">
    <div class="settings-container">
      <!-- 外观设置 -->
      <section class="settings-section">
        <div class="section-header">
          <div class="section-icon">🎨</div>
          <div class="section-title">
            <h3>外观</h3>
            <p>自定义应用的视觉效果</p>
          </div>
        </div>
        <div class="section-content">
          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">主题模式</span>
            </div>
            <div class="setting-control">
              <el-radio-group
                :model-value="themeMode"
                @update:model-value="emit('update:themeMode', $event as 'system' | 'dark' | 'light')"
                size="default"
              >
                <el-radio-button value="system">跟随系统</el-radio-button>
                <el-radio-button value="dark">深色</el-radio-button>
                <el-radio-button value="light">浅色</el-radio-button>
              </el-radio-group>
            </div>
          </div>

          <div class="setting-item">
            <div class="setting-label">
              <span class="label-text">菜单显示</span>
              <span class="label-desc">自定义侧边栏显示的工具</span>
            </div>
            <div class="setting-control">
              <el-button @click="menuVisibilityDialog?.show()">配置显示项</el-button>
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

          <div class="setting-actions">
            <el-button type="primary" @click="saveHotkeySettings">保存快捷键</el-button>
            <el-button @click="clearHotkeySettings">清除全部</el-button>
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
    />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { save, open } from "@tauri-apps/plugin-dialog";
import { registerHotkey, unregisterHotkey, registerNamedHotkey, unregisterNamedHotkey, invokeToolByChannel } from "../bridge/tauri";
import { setSetting, useAutostartSettings } from "../composables/useSettings";
import type { SidebarItem } from "../types";
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
  themeMode: "system" | "dark" | "light";
  hotkeyInput: string;
  snippetsHotkeyInput: string;
  vaultHotkeyInput: string;
  launcherHotkeyInput: string;
  sidebarItems: SidebarItem[];
  getHiddenIds: () => string[];
  setHiddenIds: (ids: string[]) => void;
}>();

const emit = defineEmits<{
  (event: "update:themeMode", value: "system" | "dark" | "light"): void;
  (event: "update:hotkeyInput", value: string): void;
  (event: "update:snippetsHotkeyInput", value: string): void;
  (event: "update:vaultHotkeyInput", value: string): void;
  (event: "update:launcherHotkeyInput", value: string): void;
}>();

const importMode = ref<"merge" | "overwrite">("merge");
const dataDirPath = ref("");
const dataDirIsCustom = ref(false);
const menuVisibilityDialog = ref<InstanceType<typeof MenuVisibilityDialog>>();

const HOTKEY_FIELDS = [
  { key: "hotkeyInput" as const, label: "显示/隐藏" },
  { key: "snippetsHotkeyInput" as const, label: "代码片段" },
  { key: "vaultHotkeyInput" as const, label: "密码管理" },
  { key: "launcherHotkeyInput" as const, label: "快捷启动" },
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
});

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
  try {
    await unregisterHotkey();
    await unregisterNamedHotkey("snippets");
    await unregisterNamedHotkey("vault");
    await unregisterNamedHotkey("launcher");
    setSetting("hotkey", "");
    setSetting("hotkey_snippets", "");
    setSetting("hotkey_vault", "");
    setSetting("hotkey_launcher", "");
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
  width: 100%;
  flex-direction: column;
  align-items: stretch;
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

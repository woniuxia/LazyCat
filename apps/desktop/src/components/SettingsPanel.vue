<template>
  <div class="panel-grid">
    <div class="panel-grid-full">
      <p style="margin-bottom: 8px; color: var(--el-text-color-secondary); font-size: 13px;">
        设置全局快捷键后，可在任意位置显示/隐藏主窗口。
      </p>
      <el-form label-width="120px" style="max-width: 480px;">
        <el-form-item label="外观主题">
          <el-radio-group
            :model-value="themeMode"
            @update:model-value="emit('update:themeMode', $event as 'system' | 'dark' | 'light')"
          >
            <el-radio-button value="system">跟随系统</el-radio-button>
            <el-radio-button value="dark">深色</el-radio-button>
            <el-radio-button value="light">浅色</el-radio-button>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="菜单显示">
          <el-button @click="menuVisibilityDialog?.show()">配置显示项</el-button>
          <span style="margin-left: 8px; color: var(--el-text-color-secondary); font-size: 12px;">
            自定义侧边栏显示的工具
          </span>
        </el-form-item>
        <el-form-item label="显示/隐藏快捷键">
          <ShortcutRecorder
            :model-value="hotkeyInput"
            :check-conflict="makeConflictChecker('hotkeyInput')"
            @update:model-value="emit('update:hotkeyInput', $event)"
          />
        </el-form-item>
        <el-form-item label="代码片段快捷键">
          <ShortcutRecorder
            :model-value="snippetsHotkeyInput"
            :check-conflict="makeConflictChecker('snippetsHotkeyInput')"
            @update:model-value="emit('update:snippetsHotkeyInput', $event)"
          />
        </el-form-item>
        <el-form-item label="密码管理快捷键">
          <ShortcutRecorder
            :model-value="vaultHotkeyInput"
            :check-conflict="makeConflictChecker('vaultHotkeyInput')"
            @update:model-value="emit('update:vaultHotkeyInput', $event)"
          />
        </el-form-item>
        <el-form-item label="快捷启动快捷键">
          <ShortcutRecorder
            :model-value="launcherHotkeyInput"
            :check-conflict="makeConflictChecker('launcherHotkeyInput')"
            @update:model-value="emit('update:launcherHotkeyInput', $event)"
          />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="saveHotkeySettings">保存</el-button>
          <el-button @click="clearHotkeySettings" style="margin-left: 8px;">清除快捷键</el-button>
        </el-form-item>
      </el-form>

      <el-divider content-position="left">系统集成</el-divider>

      <el-form label-width="140px" style="max-width: 520px;">
        <el-form-item label="开机自启动">
          <el-switch
            v-model="autostartEnabled"
            @change="handleAutostartChange"
          />
          <div style="margin-top: 4px; color: var(--el-text-color-secondary); font-size: 12px;">
            应用将在系统启动时自动运行
          </div>
        </el-form-item>

        <el-form-item label="启动时最小化到托盘">
          <el-switch
            v-model="autostartMinimized"
            :disabled="!autostartEnabled"
            @change="handleAutostartMinimizedChange"
          />
          <div style="margin-top: 4px; color: var(--el-text-color-secondary); font-size: 12px;">
            仅在开机自启动时生效，手动启动时始终显示窗口
          </div>
        </el-form-item>

        <el-form-item label="关闭时最小化到托盘">
          <el-switch
            v-model="closeToTray"
            @change="handleCloseToTrayChange"
          />
          <div style="margin-top: 4px; color: var(--el-text-color-secondary); font-size: 12px;">
            关闭时隐藏到系统托盘而非退出应用
          </div>
        </el-form-item>
      </el-form>

      <el-divider />

      <h3 style="margin-bottom: 12px;">数据目录</h3>
      <p style="margin-bottom: 12px; color: var(--el-text-color-secondary); font-size: 13px;">
        应用数据（数据库、Hosts 备份）存储在此目录。更改目录后需重启应用。
      </p>
      <div style="display: flex; gap: 8px; align-items: center; margin-bottom: 16px;">
        <el-input
          :model-value="dataDirPath"
          readonly
          style="flex: 1; max-width: 400px;"
          placeholder="加载中..."
        />
        <el-button @click="handleChangeDataDir">更改</el-button>
        <el-button
          v-if="dataDirIsCustom"
          @click="handleResetDataDir"
        >恢复默认</el-button>
      </div>

      <el-divider />

      <h3 style="margin-bottom: 12px;">数据管理</h3>
      <p style="margin-bottom: 12px; color: var(--el-text-color-secondary); font-size: 13px;">
        导出或导入应用数据（设置、收藏、使用记录、Hosts 配置）。升级或迁移时可用于备份恢复。
      </p>
      <div style="display: flex; gap: 12px; align-items: center;">
        <el-button type="primary" @click="handleExport">导出数据</el-button>
        <el-button @click="handleImport">导入数据</el-button>
        <el-radio-group v-model="importMode" size="small" style="margin-left: 8px;">
          <el-radio-button value="merge">合并</el-radio-button>
          <el-radio-button value="overwrite">覆盖</el-radio-button>
        </el-radio-group>
      </div>
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
      // 关闭开机自启动时，同时关闭"启动时最小化"
      if (autostartMinimized.value) {
        await setAutostartMinimized(false);
      }
      ElMessage.success("已禁用开机自启动");
    }
  } catch (error) {
    ElMessage.error(`设置失败：${(error as Error).message}`);
    // 回滚状态
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

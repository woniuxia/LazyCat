<template>
  <div class="panel-grid">
    <div class="panel-grid-full">
      <p style="margin-bottom: 8px; color: var(--el-text-color-secondary); font-size: 13px;">
        设置全局快捷键后，可在任意位置显示/隐藏主窗口。关闭窗口时会最小化到系统托盘。
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
        <el-form-item label="显示/隐藏快捷键">
          <el-input
            :model-value="hotkeyInput"
            placeholder="例如：Alt+Space 或 Ctrl+Shift+L"
            clearable
            style="width: 260px;"
            @update:model-value="emit('update:hotkeyInput', String($event ?? ''))"
          />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="saveHotkeySettings">保存</el-button>
          <el-button @click="clearHotkeySettings" style="margin-left: 8px;">清除快捷键</el-button>
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
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { save, open } from "@tauri-apps/plugin-dialog";
import { registerHotkey, unregisterHotkey, invokeToolByChannel } from "../bridge/tauri";
import { setSetting } from "../composables/useSettings";

const props = defineProps<{
  themeMode: "system" | "dark" | "light";
  hotkeyInput: string;
}>();

const emit = defineEmits<{
  (event: "update:themeMode", value: "system" | "dark" | "light"): void;
  (event: "update:hotkeyInput", value: string): void;
}>();

const importMode = ref<"merge" | "overwrite">("merge");
const dataDirPath = ref("");
const dataDirIsCustom = ref(false);

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
  const shortcut = props.hotkeyInput.trim();
  try {
    await registerHotkey(shortcut);
    setSetting("hotkey", shortcut);
    ElMessage.success(shortcut ? `快捷键 ${shortcut} 已保存` : "快捷键已清除");
  } catch (e) {
    ElMessage.error(`保存失败：${(e as Error).message}`);
  }
}

async function clearHotkeySettings() {
  emit("update:hotkeyInput", "");
  try {
    await unregisterHotkey();
    setSetting("hotkey", "");
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
</script>

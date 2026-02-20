<template>
  <div class="panel-grid">
    <div class="panel-grid-full">
      <p style="margin-bottom: 8px; color: var(--el-text-color-secondary); font-size: 13px;">
        设置全局快捷键后，可在任意位置显示/隐藏主窗口。关闭窗口时会最小化到系统托盘。
      </p>
      <el-form label-width="120px" style="max-width: 480px;">
        <el-form-item label="外观主题">
          <el-switch
            :model-value="isDarkMode"
            active-text="深色"
            inactive-text="浅色"
            @update:model-value="emit('update:isDarkMode', Boolean($event))"
          />
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
    </div>
  </div>
</template>

<script setup lang="ts">
import { ElMessage } from "element-plus";
import { registerHotkey, unregisterHotkey } from "../bridge/tauri";

const HOTKEY_STORAGE_KEY = "lazycat:hotkey:v1";

const props = defineProps<{
  isDarkMode: boolean;
  hotkeyInput: string;
}>();

const emit = defineEmits<{
  (event: "update:isDarkMode", value: boolean): void;
  (event: "update:hotkeyInput", value: string): void;
}>();

async function saveHotkeySettings() {
  const shortcut = props.hotkeyInput.trim();
  try {
    await registerHotkey(shortcut);
    localStorage.setItem(HOTKEY_STORAGE_KEY, shortcut);
    ElMessage.success(shortcut ? `快捷键 ${shortcut} 已保存` : "快捷键已清除");
  } catch (e) {
    ElMessage.error(`保存失败：${(e as Error).message}`);
  }
}

async function clearHotkeySettings() {
  emit("update:hotkeyInput", "");
  try {
    await unregisterHotkey();
    localStorage.removeItem(HOTKEY_STORAGE_KEY);
    ElMessage.success("快捷键已清除");
  } catch (e) {
    ElMessage.error(`清除失败：${(e as Error).message}`);
  }
}
</script>

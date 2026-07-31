<template>
  <div class="quick-capture" @keydown="onKeydown">
    <input
      ref="inputRef"
      v-model="title"
      class="capture-input"
      placeholder="输入待办标题，回车创建..."
      spellcheck="false"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invokeToolByChannel } from "../bridge/tauri";
import { APP_EVENTS } from "../bridge/events";

const title = ref("");
const inputRef = ref<HTMLInputElement | null>(null);
let unlistenReset: UnlistenFn | null = null;

onMounted(async () => {
  inputRef.value?.focus();

  try {
    unlistenReset = await listen(APP_EVENTS.QUICK_CAPTURE_RESET, () => {
      title.value = "";
      inputRef.value?.focus();
    });
  } catch {
    /* ignore in non-Tauri env */
  }
});

onBeforeUnmount(() => {
  unlistenReset?.();
});

async function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    await closeWindow();
    return;
  }

  if (e.key === "Enter") {
    e.preventDefault();
    const text = title.value.trim();
    if (!text) return;

    try {
      await invokeToolByChannel("tool:todo:item-create", {
        title: text,
        reminderPresets: ["none"],
      });
      title.value = "";
      await closeWindow();
    } catch (err) {
      console.error("[QuickCapture] 创建失败:", err);
    }
  }
}

async function closeWindow() {
  try {
    const win = getCurrentWindow();
    await win.close();
  } catch {
    /* ignore in non-Tauri env */
  }
}
</script>

<style scoped>
.quick-capture {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100vw;
  height: 100vh;
  background: #fff;
  border-radius: 8px;
  box-shadow: 0 4px 24px rgba(0, 0, 0, 0.12);
  overflow: hidden;
}

.capture-input {
  width: 100%;
  height: 100%;
  border: none;
  outline: none;
  font-size: 16px;
  padding: 0 20px;
  color: #303133;
  background: transparent;
  font-family: inherit;
}

.capture-input::placeholder {
  color: #c0c4cc;
}
</style>

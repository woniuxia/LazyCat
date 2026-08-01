<template>
  <header class="top-bar">
    <div class="top-bar-left">
      <button class="top-bar-brand" type="button" title="点击返回首页" @click="goHome">
        <img class="brand-logo" src="../assets/icon.png" alt="Lazycat" />
        <span class="brand-name">Lazycat</span>
      </button>
    </div>

    <div class="top-bar-search">
      <button
        class="search-box"
        type="button"
        title="打开 Spotlight"
        aria-label="打开 Spotlight 搜索"
        aria-haspopup="dialog"
        aria-keyshortcuts="/"
        @click="openSpotlight"
      >
        <el-icon class="search-icon"><Search /></el-icon>
        <span class="search-placeholder">搜索工具、动作与数据</span>
        <kbd class="search-shortcut">/</kbd>
      </button>
    </div>

    <div class="top-bar-right">
      <button
        class="top-bar-btn"
        title="设置"
        :class="{ 'is-active': isSettingsActive }"
        @click="goSettings"
      >
        <el-icon><Setting /></el-icon>
      </button>
    </div>
  </header>
</template>

<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount } from "vue";
import { ElMessage } from "element-plus";
import { invoke } from "@tauri-apps/api/core";
import { Search, Setting } from "@element-plus/icons-vue";

const props = defineProps<{
  activeTool: string;
}>();

const emit = defineEmits<{
  "goto-home": [];
  "goto-settings": [];
}>();

const isSettingsActive = computed(() => props.activeTool === "settings");

async function openSpotlight() {
  try {
    await invoke("spotlight_open");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    ElMessage.error(`打开 Spotlight 失败：${message}`);
  }
}

function onGlobalKeydown(e: KeyboardEvent) {
  const target = e.target as HTMLElement | null;
  if (
    e.key === "/" &&
    !e.ctrlKey &&
    !e.metaKey &&
    !e.altKey &&
    !["INPUT", "TEXTAREA"].includes(target?.tagName ?? "")
  ) {
    e.preventDefault();
    void openSpotlight();
  }
}

function goHome() {
  emit("goto-home");
}

function goSettings() {
  emit("goto-settings");
}

onMounted(() => {
  window.addEventListener("keydown", onGlobalKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onGlobalKeydown);
});

function focusSearch() {
  void openSpotlight();
}

defineExpose({
  focusSearch,
});
</script>

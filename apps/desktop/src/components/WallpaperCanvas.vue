<template>
  <!-- Living Wallpaper · 信息层 canvas，360×800 -->
  <div class="wallpaper-canvas" :class="`mode-${colorMode}`" :data-state="state">
    <WallpaperOverviewBlock v-if="data" :overview="data.overview" />
    <WallpaperTodoList v-if="data" :items="data.todoList" />
    <WallpaperExtensionSlot />
  </div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from "vue";
import { listen, emit, type UnlistenFn } from "@tauri-apps/api/event";
import type { WallpaperDashboardData } from "@/types/wallpaper";
import WallpaperOverviewBlock from "./WallpaperOverviewBlock.vue";
import WallpaperTodoList from "./WallpaperTodoList.vue";
import WallpaperExtensionSlot from "./WallpaperExtensionSlot.vue";

type ColorMode = "light" | "dark";

const data = ref<WallpaperDashboardData | null>(null);
const colorMode = ref<ColorMode>("dark");
// "boot" → "rendering" → "ready"；供 e2e / 调试观察
const state = ref<"boot" | "rendering" | "ready">("boot");

const unlisteners: UnlistenFn[] = [];

onMounted(async () => {
  // 订阅后端推送
  unlisteners.push(
    await listen<WallpaperDashboardData>("wallpaper://dashboard-data", (e) => {
      data.value = e.payload;
      state.value = "rendering";
      // 等 Vue 完成 reconcile + 浏览器至少绘制一次后再通知后端
      waitTwoFrames().then(() => {
        state.value = "ready";
        // 用全局 emit；后端通过 `app.listen_any` 订阅
        void emit("wallpaper://canvas-ready", {
          generatedAt: e.payload.generatedAt,
        });
      });
    }),
  );

  unlisteners.push(
    await listen<ColorMode>("wallpaper://color-mode", (e) => {
      colorMode.value = e.payload;
    }),
  );

  // 通知后端 canvas 已挂载（可接收事件）
  void emit("wallpaper://canvas-mounted", {});
});

onBeforeUnmount(() => {
  unlisteners.splice(0).forEach((un) => un());
});

function waitTwoFrames(): Promise<void> {
  return new Promise((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  });
}
</script>

<style scoped>
/*
 * 360×800 信息层。
 * 字体 / 字号尽量沿用 PoC 校验过的视觉，避免重新调参。
 * 浅 / 深玻璃蒙层在 :root[data-color-mode] 上切换 CSS 变量。
 */
.wallpaper-canvas {
  width: 360px;
  height: 800px;
  padding: 16px;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 12px;
  border-radius: 16px;
  font-family: "Segoe UI", "Microsoft YaHei", sans-serif;
  backdrop-filter: blur(10px);
}

/* mode-light：浅色文字 + 深玻璃蒙层（深色壁纸下用） */
.wallpaper-canvas.mode-light {
  --wc-text: #ffffff;
  --wc-text-muted: rgba(255, 255, 255, 0.6);
  --wc-text-strong: #ffffff;
  --wc-glass: rgba(15, 23, 42, 0.55);
  --wc-block-bg: rgba(255, 255, 255, 0.06);
  --wc-block-border: rgba(255, 255, 255, 0.1);
  --wc-divider: rgba(255, 255, 255, 0.05);
  background: var(--wc-glass);
  color: var(--wc-text);
}

/* mode-dark：深色文字 + 浅玻璃蒙层（浅色壁纸下用） */
.wallpaper-canvas.mode-dark {
  --wc-text: #1a1a1a;
  --wc-text-muted: rgba(26, 26, 26, 0.55);
  --wc-text-strong: #0f172a;
  --wc-glass: rgba(255, 255, 255, 0.6);
  --wc-block-bg: rgba(0, 0, 0, 0.04);
  --wc-block-border: rgba(0, 0, 0, 0.08);
  --wc-divider: rgba(0, 0, 0, 0.06);
  background: var(--wc-glass);
  color: var(--wc-text);
}
</style>

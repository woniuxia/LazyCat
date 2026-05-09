<template>
  <!-- LazyCat 桌面挂件 · 360×800 信息层 -->
  <div class="widget-canvas" :class="`mode-${colorMode}`">
    <!-- 顶部拖拽把手：仅 16px 高，CSS app-region: drag 让用户沿屏幕沿移动挂件 Y -->
    <div class="drag-handle" data-tauri-drag-region>
      <span class="grip">⋮⋮</span>
    </div>

    <template v-if="data">
      <WidgetOverviewBlock :overview="data.overview" />
      <WidgetTodoList
        :items="data.todoList"
        :privacy-mask="privacyMask"
        @complete="onCompleteItem"
      />
      <WidgetExtensionSlot />
    </template>
    <div v-else class="boot">
      <span>加载中…</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from "vue";
import { listen, emit, type UnlistenFn } from "@tauri-apps/api/event";
import { invokeToolByChannel } from "../bridge/tauri";
import type { WidgetDashboardData, WidgetTodoItem } from "../types/widget";
import WidgetOverviewBlock from "./WidgetOverviewBlock.vue";
import WidgetTodoList from "./WidgetTodoList.vue";
import WidgetExtensionSlot from "./WidgetExtensionSlot.vue";

type ColorMode = "light" | "dark";

const data = ref<WidgetDashboardData | null>(null);
const colorMode = ref<ColorMode>("dark");
const privacyMask = ref(false);

const unlisteners: UnlistenFn[] = [];

onMounted(async () => {
  console.log("[widget-canvas] onMounted start", Date.now());
  try {
    const unlistenData = await listen<WidgetDashboardData>("widget://dashboard-data", (e) => {
      console.log("[widget-canvas] received dashboard-data", e.payload);
      data.value = e.payload;
      privacyMask.value = e.payload?.privacyMask === true;
    });
    console.log("[widget-canvas] dashboard-data listener registered");
    unlisteners.push(unlistenData);
  } catch (e) {
    console.error("[widget-canvas] dashboard-data listen failed", e);
  }
  try {
    const unlistenColor = await listen<ColorMode>("widget://color-mode", (e) => {
      console.log("[widget-canvas] received color-mode", e.payload);
      colorMode.value = e.payload;
    });
    console.log("[widget-canvas] color-mode listener registered");
    unlisteners.push(unlistenColor);
  } catch (e) {
    console.error("[widget-canvas] color-mode listen failed", e);
  }
});

onBeforeUnmount(() => {
  unlisteners.splice(0).forEach((un) => un());
});

/**
 * 用户点击 todo 行 checkbox 完成事项。
 *
 * id 形如 `pm:123` / `todo:456`，按 prefix 路由到对应的 change-status 通道。
 * 完成后通知后端 invalidate hash + 立即推新数据。
 */
async function onCompleteItem(item: WidgetTodoItem) {
  const [source, idRaw] = item.id.split(":", 2);
  const id = Number(idRaw);
  if (!Number.isFinite(id) || id <= 0) {
    console.warn("[widget-canvas] invalid item id", item.id);
    return;
  }
  // 乐观更新：从本地列表移除，避免等待后端往返的卡顿
  if (data.value) {
    data.value.todoList = data.value.todoList.filter((i) => i.id !== item.id);
  }
  try {
    if (source === "pm") {
      await invokeToolByChannel("tool:pm:item-change-status", { id, status: "done" });
    } else if (source === "todo") {
      await invokeToolByChannel("tool:todo:item-change-status", { id, status: "done" });
    } else {
      console.warn("[widget-canvas] unknown source", source);
      return;
    }
    // 通知后端：让 dashboard 重算并推一份新数据
    void emit("widget://canvas-action", { kind: "todo-completed", id: item.id });
  } catch (e) {
    console.error("[widget-canvas] complete failed", e);
    // 失败回滚：把项放回去（按原顺序难精确恢复，简单 unshift 到顶部）
    if (data.value) {
      data.value.todoList = [item, ...data.value.todoList];
    }
  }
}
</script>

<style scoped>
/*
 * 360×800 挂件信息层。
 * 浅 / 深玻璃蒙层在 :class="mode-light/dark" 上切换 CSS 变量。
 */
.widget-canvas {
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

/* 顶部 16px 拖拽把手；data-tauri-drag-region 让 Tauri 把鼠标拖动事件映射到窗口移动 */
.drag-handle {
  height: 16px;
  margin: -16px -16px 0 -16px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: grab;
  user-select: none;
  border-radius: 16px 16px 0 0;
  flex-shrink: 0;
}
.drag-handle:active {
  cursor: grabbing;
}
.grip {
  font-size: 10px;
  color: var(--wc-text-muted);
  letter-spacing: 2px;
  opacity: 0.5;
  transition: opacity 0.15s ease;
}
.drag-handle:hover .grip {
  opacity: 1;
}

/* mode-light：浅色文字 + 深玻璃蒙层（深色壁纸下用） */
.widget-canvas.mode-light {
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
.widget-canvas.mode-dark {
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

.boot {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  color: var(--wc-text-muted);
}
</style>
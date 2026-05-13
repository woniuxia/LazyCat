<template>
  <!-- LazyCat 桌面挂件 · 360×800 信息层 -->
  <div class="widget-canvas" :class="glassTheme">
    <!-- 顶部拖拽把手 -->
    <div class="drag-handle" data-tauri-drag-region>
      <svg class="grip-icon" viewBox="0 0 28 12" fill="none">
        <circle cx="6" cy="6" r="1.2" fill="currentColor"/>
        <circle cx="14" cy="6" r="1.2" fill="currentColor"/>
        <circle cx="22" cy="6" r="1.2" fill="currentColor"/>
      </svg>
    </div>

    <template v-if="data">
      <WidgetTodoList
        :items="data.todoList"
        :privacy-mask="privacyMask"
        @complete="onCompleteItem"
        @action="onCanvasAction"
      />
      <WidgetExtensionSlot :hot-tools="hotToolsForSlot" @action="onCanvasAction" />
      <div v-if="showStaleHint" class="stale-hint">
        <span class="stale-dot" />
        <span>刷新中…</span>
      </div>
    </template>
    <div v-else class="boot">
      <div class="boot-spinner" />
      <span>加载中…</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref, computed } from "vue";
import { listen, emit, type UnlistenFn } from "@tauri-apps/api/event";
import { invokeToolByChannel } from "../bridge/tauri";
import type { WidgetDashboardData, WidgetTodoItem, WidgetHotTool } from "../types/widget";
import { getAllToolMap } from "../composables/toolCatalog";
import WidgetTodoList from "./WidgetTodoList.vue";
import WidgetExtensionSlot from "./WidgetExtensionSlot.vue";

type ColorMode = "light" | "dark";

const data = ref<WidgetDashboardData | null>(null);
const colorMode = ref<ColorMode>("dark");
const privacyMask = ref(false);
const lastDataReceivedAt = ref(0);

const unlisteners: UnlistenFn[] = [];
let pingHandle: ReturnType<typeof setInterval> | null = null;

/** 将后端 colorMode 映射为语义化 CSS class */
const glassTheme = computed(() => {
  // colorMode 表示文字颜色模式
  // "light" = 浅色文字 = 深色壁纸上用深玻璃 → glass-on-dark
  // "dark"  = 深色文字 = 浅色壁纸上用浅玻璃 → glass-on-light
  return colorMode.value === "light" ? "glass-on-dark" : "glass-on-light";
});

/** 如果超过 60s 无数据，显示"刷新中…"提示 */
const showStaleHint = computed(() => {
  if (!data.value) return false;
  return lastDataReceivedAt.value > 0 && Date.now() - lastDataReceivedAt.value > 60_000;
});

/** 解析工具名后的热门工具列表，供 WidgetExtensionSlot 渲染。无点击数据时提供默认推荐。 */
const hotToolsForSlot = computed(() => {
  const hotTools = data.value?.hotTools ?? [];
  const map = getAllToolMap();
  if (hotTools.length > 0) {
    return hotTools
      .map((t: WidgetHotTool) => {
        const def = map.get(t.id);
        return def ? { id: t.id, label: def.name } : null;
      })
      .filter((t): t is { id: string; label: string } => t !== null);
  }
  const defaults = ["pm", "inbox", "snippets"];
  return defaults
    .map((id) => {
      const def = map.get(id);
      return def ? { id, label: def.name } : null;
    })
    .filter((t): t is { id: string; label: string } => t !== null);
});

onMounted(async () => {
  unlisteners.push(
    await listen<WidgetDashboardData>("widget://dashboard-data", (e) => {
      data.value = e.payload;
      privacyMask.value = e.payload?.privacyMask === true;
      lastDataReceivedAt.value = Date.now();
    }),
  );
  unlisteners.push(
    await listen<ColorMode>("widget://color-mode", (e) => {
      colorMode.value = e.payload;
    }),
  );

  // 握手：通知后端挂件已就绪，触发立即推送数据
  // 解决启动时 apply 在 Vue 挂载前发射事件导致数据丢失的竞态问题
  void emit("widget://ready");

  // 看门狗 ping：每 5s 通知后端挂件存活
  pingHandle = setInterval(() => {
    void emit("widget://ping");
  }, 5000);
});

onBeforeUnmount(() => {
  unlisteners.splice(0).forEach((un) => un());
  if (pingHandle !== null) {
    clearInterval(pingHandle);
    pingHandle = null;
  }
});

/** 子组件（WidgetTodoList / WidgetExtensionSlot）的 action 统一通过 Tauri event 转发后端。 */
function onCanvasAction(payload: { kind: string; [key: string]: unknown }) {
  void emit("widget://canvas-action", payload);
}

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
      await invokeToolByChannel("tool:todo:item-change-status", { id, status: "completed" });
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
 * glass-on-dark：深色壁纸上用深色玻璃 + 浅色文字
 * glass-on-light：浅色壁纸上用浅色玻璃 + 深色文字
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
  user-select: none;
}

/* 顶部 16px 拖拽把手 */
.drag-handle {
  height: 16px;
  margin: -16px -16px 0 -16px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: grab;
  border-radius: 16px 16px 0 0;
  flex-shrink: 0;
}

.drag-handle:active {
  cursor: grabbing;
}

.grip-icon {
  width: 28px;
  height: 12px;
  opacity: 0.35;
  transition: opacity 0.2s ease;
}

.drag-handle:hover .grip-icon {
  opacity: 0.7;
}

/* glass-on-dark：深色壁纸 → 深玻璃 + 浅色文字 */
.widget-canvas.glass-on-dark {
  --wc-text: #ffffff;
  --wc-text-muted: rgba(255, 255, 255, 0.6);
  --wc-text-strong: #ffffff;
  --wc-glass: rgba(15, 23, 42, 0.55);
  --wc-block-bg: rgba(255, 255, 255, 0.06);
  --wc-block-border: rgba(255, 255, 255, 0.1);
  --wc-divider: rgba(255, 255, 255, 0.05);
  --wc-bg-tertiary: rgba(255, 255, 255, 0.15);
  background: var(--wc-glass);
  color: var(--wc-text);
}

/* glass-on-light：浅色壁纸 → 浅玻璃 + 深色文字 */
.widget-canvas.glass-on-light {
  --wc-text: #1a1a1a;
  --wc-text-muted: rgba(26, 26, 26, 0.55);
  --wc-text-strong: #0f172a;
  --wc-glass: rgba(255, 255, 255, 0.6);
  --wc-block-bg: rgba(0, 0, 0, 0.04);
  --wc-block-border: rgba(0, 0, 0, 0.08);
  --wc-divider: rgba(0, 0, 0, 0.06);
  --wc-bg-tertiary: rgba(0, 0, 0, 0.12);
  background: var(--wc-glass);
  color: var(--wc-text);
}

.stale-hint {
  display: flex;
  align-items: center;
  gap: 5px;
  justify-content: center;
  font-size: 11px;
  color: var(--wc-text-muted);
  opacity: 0.6;
  pointer-events: none;
  padding: 4px 0;
}

.stale-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--wc-text-muted);
  animation: pulse-dot 1.2s ease-in-out infinite;
}

@keyframes pulse-dot {
  0%, 100% { opacity: 0.3; }
  50% { opacity: 1; }
}

.boot {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  font-size: 13px;
  color: var(--wc-text-muted);
}

.boot-spinner {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  border: 2px solid var(--wc-bg-tertiary);
  border-top-color: var(--wc-text-muted);
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}
</style>
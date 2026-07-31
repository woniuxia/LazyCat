<template>
  <!-- LazyCat 桌面挂件 · 360×800 信息层 -->
  <div class="widget-canvas">
    <!-- 顶部拖拽把手 -->
    <div class="drag-handle" data-tauri-drag-region>
      <svg class="grip-icon" viewBox="0 0 28 12" fill="none">
        <circle cx="6" cy="6" r="1.4" fill="currentColor"/>
        <circle cx="14" cy="6" r="1.4" fill="currentColor"/>
        <circle cx="22" cy="6" r="1.4" fill="currentColor"/>
      </svg>
    </div>

    <template v-if="data">
      <WidgetTodoList
        :items="data.todoList"
        :privacy-mask="privacyMask"
        :truncated="data.todoTruncated"
        :total-count="data.todoTotalCount"
        @complete="onCompleteItem"
        @action="onCanvasAction"
      />
      <WidgetExtensionSlot
        :hot-tools="hotToolsForSlot"
        :fixed-tool-ids="extensionFixedToolIds"
        @action="onCanvasAction"
      />
      <div v-if="showStaleHint" class="stale-hint" :class="{ 'is-stale-long': isStaleLong }">
        <span class="stale-dot" />
        <template v-if="!isStaleLong">
          <span>刷新中…</span>
        </template>
        <template v-else>
          <span>连接异常</span>
          <button class="stale-retry" type="button" @click="onManualRefresh">重试</button>
        </template>
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
import { APP_EVENTS } from "../bridge/events";
import type { WidgetDashboardData, WidgetTodoItem, WidgetHotTool } from "../types/widget";
import { getAllToolMap } from "../composables/toolCatalog";
import WidgetTodoList from "./WidgetTodoList.vue";
import WidgetExtensionSlot from "./WidgetExtensionSlot.vue";

const data = ref<WidgetDashboardData | null>(null);
const privacyMask = ref(false);
const lastDataReceivedAt = ref(0);
/** 用于让 stale 状态判定每 5s 重新计算 */
const stalenessTick = ref(0);

const unlisteners: UnlistenFn[] = [];
let pingHandle: ReturnType<typeof setInterval> | null = null;
let stalenessHandle: ReturnType<typeof setInterval> | null = null;

/** 如果超过 60s 无数据，显示"刷新中…"提示 */
const showStaleHint = computed(() => {
  // touch stalenessTick 让 computed 周期性重算
  void stalenessTick.value;
  if (!data.value) return false;
  return lastDataReceivedAt.value > 0 && Date.now() - lastDataReceivedAt.value > 60_000;
});

/** 超过 120s 视为连接异常，显示"重试"按钮 */
const isStaleLong = computed(() => {
  void stalenessTick.value;
  if (!data.value) return false;
  return lastDataReceivedAt.value > 0 && Date.now() - lastDataReceivedAt.value > 120_000;
});

/** 拓展区固定工具 ID 列表，来自后端配置（dashboard data 同步下发）。 */
const extensionFixedToolIds = computed(() => {
  return data.value?.extensionFixedTools ?? ["pm", "todo", "inbox"];
});

/** 解析工具名后的热门工具列表，供 WidgetExtensionSlot 渲染。无点击数据时提供默认推荐。 */
const hotToolsForSlot = computed(() => {
  const hotTools = data.value?.hotTools ?? [];
  const limit = data.value?.extensionHotToolsLimit ?? 3;
  const map = getAllToolMap();
  if (hotTools.length > 0) {
    return hotTools
      .slice(0, limit)
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
    await listen<WidgetDashboardData>(APP_EVENTS.WIDGET_DASHBOARD_DATA, (e) => {
      data.value = e.payload;
      privacyMask.value = e.payload?.privacyMask === true;
      lastDataReceivedAt.value = Date.now();
    }),
  );

  // 握手：通知后端挂件已就绪，触发立即推送数据
  // 解决启动时 apply 在 Vue 挂载前发射事件导致数据丢失的竞态问题
  void emit("widget://ready");

  // 看门狗 ping：每 5s 通知后端挂件存活
  pingHandle = setInterval(() => {
    void emit("widget://ping");
  }, 5000);

  // 每 5s 触发一次 staleness 重新计算（Date.now 不响应式，需 tick）
  stalenessHandle = setInterval(() => {
    stalenessTick.value = Date.now();
  }, 5000);
});

onBeforeUnmount(() => {
  unlisteners.splice(0).forEach((un) => un());
  if (pingHandle !== null) {
    clearInterval(pingHandle);
    pingHandle = null;
  }
  if (stalenessHandle !== null) {
    clearInterval(stalenessHandle);
    stalenessHandle = null;
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
  // 记录原索引，失败时按原位置 splice 回去，避免顺序错乱
  const originalIndex = data.value
    ? data.value.todoList.findIndex((i) => i.id === item.id)
    : -1;
  // 乐观更新：从本地列表移除，避免等待后端往返的卡顿
  if (data.value && originalIndex >= 0) {
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
    // 失败回滚：按原位置插回
    if (data.value && originalIndex >= 0) {
      const list = [...data.value.todoList];
      const insertAt = Math.min(originalIndex, list.length);
      list.splice(insertAt, 0, item);
      data.value.todoList = list;
    }
  }
}

/** stale 状态下用户手动重试：通知后端 invalidate + 立即推新数据 */
function onManualRefresh() {
  void emit("widget://canvas-action", { kind: "manual-refresh" });
  // 重置时间戳避免按钮被反复点击；后端推送后会刷新这个时间
  lastDataReceivedAt.value = Date.now();
  stalenessTick.value = Date.now();
}
</script>

<style scoped>
/* 360×800 挂件信息层 · 清新紫韵 */
.widget-canvas {
  --wc-text: #1e293b;
  --wc-text-muted: #94a3b8;
  --wc-text-strong: #0f172a;
  --wc-glass: rgba(255, 255, 255, 0.75);
  --wc-block-bg: rgba(0, 0, 0, 0.03);
  --wc-block-border: rgba(0, 0, 0, 0.05);
  --wc-divider: rgba(0, 0, 0, 0.04);
  --wc-bg-tertiary: rgba(0, 0, 0, 0.08);
  --wc-row-hover: rgba(99, 102, 241, 0.04);
  --wc-rim-light: rgba(255, 255, 255, 0.5);
  --wc-accent: #6366f1;
  --wc-accent-purple: #9333ea;
  --wc-accent-teal: #0d9488;

  width: 360px;
  height: 800px;
  padding: 16px;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 12px;
  border-radius: 12px;
  font-family: "Segoe UI", "Microsoft YaHei", sans-serif;
  backdrop-filter: blur(10px);
  user-select: none;
  background: linear-gradient(135deg, rgba(248, 250, 252, 0.85), rgba(241, 245, 249, 0.85));
  color: var(--wc-text);
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.04), inset 0 1px 0 var(--wc-rim-light);
}

/* 顶部 16px 拖拽把手 */
.drag-handle {
  height: 16px;
  margin: -16px -16px 0 -16px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: grab;
  border-radius: 12px 12px 0 0;
  flex-shrink: 0;
  transition: background-color 0.2s ease;
}

.drag-handle:hover {
  background: var(--wc-row-hover);
}

.drag-handle:active {
  cursor: grabbing;
  background: var(--wc-block-border);
}

.grip-icon {
  width: 28px;
  height: 12px;
  opacity: 0.3;
  transition: opacity 0.2s ease;
}

.drag-handle:hover .grip-icon {
  opacity: 0.65;
}

.stale-hint {
  display: flex;
  align-items: center;
  gap: 6px;
  justify-content: center;
  font-size: 11px;
  color: var(--wc-text-muted);
  opacity: 0.5;
  pointer-events: none;
  padding: 2px 0;
}

.stale-hint.is-stale-long {
  color: #ef4444;
  opacity: 1;
  pointer-events: auto;
  font-weight: 500;
}

.stale-retry {
  margin-left: 4px;
  padding: 2px 10px;
  border-radius: 10px;
  background: rgba(239, 68, 68, 0.08);
  border: 1px solid rgba(239, 68, 68, 0.25);
  color: #ef4444;
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  font-family: inherit;
  transition: background-color 0.15s ease, transform 0.1s ease;
}

.stale-retry:hover {
  background: rgba(239, 68, 68, 0.16);
}

.stale-retry:active {
  transform: scale(0.95);
}

.stale-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--wc-text-muted);
  animation: pulse-dot 1.5s ease-in-out infinite;
}

@keyframes pulse-dot {
  0%, 100% { opacity: 0.25; }
  50% { opacity: 0.9; }
}

.boot {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  font-size: 13px;
  color: var(--wc-text-muted);
}

.boot-spinner {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  border: 2px solid var(--wc-bg-tertiary);
  border-top-color: var(--wc-text-muted);
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}
</style>

<template>
  <Transition name="cb-pill">
    <div
      v-if="visible && suggestion"
      class="cb-strip"
      @mouseenter="pauseAutoClose"
      @mouseleave="resumeAutoClose"
    >
      <!-- Left accent + pulse -->
      <div class="cb-accent-edge" />

      <!-- Type badge -->
      <span class="cb-type-badge">{{ suggestion.label }}</span>

      <!-- Truncated preview -->
      <span class="cb-preview" :title="suggestion.preview">{{ suggestion.preview }}</span>

      <!-- Action chips -->
      <div class="cb-actions">
        <button
          v-for="action in suggestion.actions"
          :key="getActionKey(action)"
          class="cb-action-chip"
          @click="onAction(action)"
        >
          <span class="cb-chip-label">{{ action.label }}</span>
          <svg class="cb-chip-arrow" viewBox="0 0 16 16" fill="none">
            <path d="M6 3l5 5-5 5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </button>
      </div>

      <!-- Dismiss -->
      <button class="cb-dismiss" @click="dismiss" title="关闭 (Esc)">
        <svg viewBox="0 0 16 16" fill="none">
          <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
        </svg>
      </button>

      <!-- Auto-close progress bar -->
      <div class="cb-progress">
        <div class="cb-progress-bar" :style="{ transform: 'scaleX(' + progressPercent / 100 + ')' }" />
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import { useClipboardSuggestion } from "../composables/useClipboardSuggestion";
import type { ClipboardAction } from "../utils/clipboard-detect";

const emit = defineEmits<{
  (event: "open-tool", toolId: string, toolName: string): void;
}>();

const { suggestion, visible, applyAction, dismiss } = useClipboardSuggestion();

const autoCloseDuration = 6000;
const isPaused = ref(false);
const progressPercent = ref(100);
let autoCloseTimer: ReturnType<typeof setTimeout> | null = null;
let remainingTime = autoCloseDuration;
let timerStart = 0;
let rafId: number | null = null;

function clearAutoClose() {
  if (autoCloseTimer) {
    clearTimeout(autoCloseTimer);
    autoCloseTimer = null;
  }
  if (rafId !== null) {
    cancelAnimationFrame(rafId);
    rafId = null;
  }
}

function updateProgress() {
  if (isPaused.value) return;

  const elapsed = Date.now() - timerStart;
  const remaining = Math.max(0, remainingTime - elapsed);
  progressPercent.value = (remaining / autoCloseDuration) * 100;

  if (remaining > 0) {
    rafId = requestAnimationFrame(updateProgress);
  }
}

function startAutoClose(duration: number) {
  clearAutoClose();
  remainingTime = duration;
  timerStart = Date.now();
  progressPercent.value = (duration / autoCloseDuration) * 100;
  autoCloseTimer = setTimeout(() => dismiss(), duration);
  rafId = requestAnimationFrame(updateProgress);
}

function pauseAutoClose() {
  isPaused.value = true;
  if (autoCloseTimer) {
    const elapsed = Date.now() - timerStart;
    remainingTime = Math.max(0, remainingTime - elapsed);
    clearAutoClose();
  }
}

function resumeAutoClose() {
  isPaused.value = false;
  if (remainingTime > 0) {
    startAutoClose(remainingTime);
  }
}

watch([visible, suggestion], ([v, currentSuggestion]) => {
  clearAutoClose();
  isPaused.value = false;
  if (v && currentSuggestion) {
    remainingTime = autoCloseDuration;
    progressPercent.value = 100;
    startAutoClose(autoCloseDuration);
  } else {
    progressPercent.value = 100;
  }
});

function getActionKey(action: ClipboardAction): string {
  if (action.kind === "tool") return `tool:${action.toolId}`;
  return `open-path:${action.path}:${action.reveal ? "reveal" : "open"}`;
}

async function onAction(action: ClipboardAction) {
  clearAutoClose();
  if (action.kind === "tool") {
    applyAction(action.toolId);
    emit("open-tool", action.toolId, action.toolName);
    return;
  }

  try {
    await invokeToolByChannel("tool:inbox:open-path", {
      path: action.path,
      reveal: action.reveal,
    });
    dismiss();
  } catch (error) {
    startAutoClose(autoCloseDuration);
    ElMessage.error((error as Error).message || "打开路径失败");
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape" && visible.value) {
    dismiss();
  }
}

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
  clearAutoClose();
});
</script>

<style scoped>
/* ---- Strip container ---- */
.cb-strip {
  position: relative;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 14px 0 0;
  margin-bottom: 16px;
  border-radius: var(--lc-radius-md);
  background: #f0f7ff;
  border: 1px solid rgba(56, 189, 248, 0.2);
  overflow: hidden;
  box-shadow:
    0 1px 4px rgba(0, 0, 0, 0.06),
    0 0 16px rgba(56, 189, 248, 0.06);
  flex-shrink: 0;
  min-height: 44px;
}

/* ---- Left accent edge ---- */
.cb-accent-edge {
  width: 3px;
  align-self: stretch;
  background: #0284c7;
  border-radius: 3px 0 0 3px;
  flex-shrink: 0;
  animation: accentPulse 2s ease-in-out infinite;
}

@keyframes accentPulse {
  0%, 100% { opacity: 0.7; }
  50% { opacity: 1; }
}

/* ---- Type badge ---- */
.cb-type-badge {
  font-family: var(--lc-font-mono);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.5px;
  text-transform: uppercase;
  color: #0284c7;
  background: rgba(56, 189, 248, 0.12);
  padding: 3px 10px;
  border-radius: 6px;
  white-space: nowrap;
  flex-shrink: 0;
  margin-left: 10px;
}

/* ---- Preview ---- */
.cb-preview {
  font-size: 12px;
  color: #64748b;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
  flex: 1;
  font-family: var(--lc-font-mono);
  line-height: 1.4;
}

/* ---- Action chips ---- */
.cb-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}

.cb-action-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 5px 12px;
  font-size: 12px;
  font-weight: 500;
  font-family: var(--lc-font-body);
  color: #0f172a;
  background: rgba(56, 189, 248, 0.08);
  border: 1px solid rgba(56, 189, 248, 0.2);
  border-radius: 6px;
  cursor: pointer;
  transition:
    background 180ms var(--lc-ease),
    border-color 180ms var(--lc-ease),
    color 180ms var(--lc-ease),
    box-shadow 180ms var(--lc-ease);
  white-space: nowrap;
}

.cb-action-chip:hover {
  background: rgba(56, 189, 248, 0.16);
  border-color: rgba(56, 189, 248, 0.4);
  color: #0284c7;
  box-shadow: 0 0 12px rgba(56, 189, 248, 0.12);
}

.cb-action-chip:active {
  background: rgba(56, 189, 248, 0.25);
  transform: scale(0.97);
}

.cb-chip-arrow {
  width: 12px;
  height: 12px;
  opacity: 0.42;
  transform: translateX(0);
  transition: opacity 180ms var(--lc-ease), transform 180ms var(--lc-ease);
}

.cb-action-chip:hover .cb-chip-arrow {
  opacity: 1;
  transform: translateX(1px);
}

/* ---- Dismiss button ---- */
.cb-dismiss {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  border: none;
  background: transparent;
  color: #94a3b8;
  cursor: pointer;
  border-radius: 4px;
  flex-shrink: 0;
  transition: color 180ms var(--lc-ease), background 180ms var(--lc-ease);
}

.cb-dismiss svg {
  width: 14px;
  height: 14px;
}

.cb-dismiss:hover {
  color: #475569;
  background: rgba(0, 0, 0, 0.05);
}

/* ---- Auto-close progress bar ---- */
.cb-progress {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 2px;
}

.cb-progress-bar {
  height: 100%;
  background: #0284c7;
  opacity: 0.4;
  border-radius: 0 2px 2px 0;
  transform-origin: left;
  transition: transform 50ms linear;
}

/* ---- Transition ---- */
.cb-pill-enter-active {
  transition:
    opacity 280ms var(--lc-ease-out),
    transform 280ms var(--lc-ease-out),
    max-height 280ms var(--lc-ease-out),
    margin-bottom 280ms var(--lc-ease-out);
}

.cb-pill-leave-active {
  transition:
    opacity 200ms var(--lc-ease),
    transform 200ms var(--lc-ease),
    max-height 200ms var(--lc-ease),
    margin-bottom 200ms var(--lc-ease);
}

.cb-pill-enter-from {
  opacity: 0;
  transform: translateY(-8px);
  max-height: 0;
  margin-bottom: 0;
}

.cb-pill-leave-to {
  opacity: 0;
  transform: translateY(-6px);
  max-height: 0;
  margin-bottom: 0;
}
</style>

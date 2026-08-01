<template>
  <div v-if="message" class="spotlight-error" role="alert">
    <WarningFilled class="spotlight-error-icon" aria-hidden="true" />
    <span class="spotlight-error-text" :title="message">{{ message }}</span>
    <button v-if="canRetry" class="spotlight-error-btn" @click="$emit('retry')">
      重试 (Ctrl+R)
    </button>
    <button class="spotlight-error-btn" @click="$emit('dismiss')">关闭</button>
  </div>
</template>

<script setup lang="ts">
import { WarningFilled } from "@element-plus/icons-vue";

defineProps<{
  message: string | null;
  canRetry?: boolean;
}>();

defineEmits<{
  (e: "retry"): void;
  (e: "dismiss"): void;
}>();
</script>

<style scoped>
.spotlight-error {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 16px;
  background: rgba(245, 108, 108, 0.08);
  border-top: 1px solid rgba(245, 108, 108, 0.25);
  color: #c45656;
  font-size: 12px;
}

.spotlight-error-icon {
  width: 14px;
  height: 14px;
  flex-shrink: 0;
}

.spotlight-error-text {
  flex: 1;
  min-width: 0;
  white-space: normal;
  word-break: break-word;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  line-height: 1.4;
}

.spotlight-error-btn {
  border: 1px solid rgba(245, 108, 108, 0.4);
  background: transparent;
  color: #c45656;
  font-size: 11px;
  padding: 3px 8px;
  border-radius: 6px;
  cursor: pointer;
}

.spotlight-error-btn:hover {
  background: rgba(245, 108, 108, 0.1);
}
</style>

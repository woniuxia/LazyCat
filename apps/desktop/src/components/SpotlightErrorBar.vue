<template>
  <div v-if="message" class="spotlight-error">
    <span class="spotlight-error-icon">⚠</span>
    <span class="spotlight-error-text">{{ message }}</span>
    <button v-if="canRetry" class="spotlight-error-btn" @click="$emit('retry')">
      重试 (Ctrl+R)
    </button>
    <button class="spotlight-error-btn" @click="$emit('dismiss')">关闭</button>
  </div>
</template>

<script setup lang="ts">
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
  font-size: 14px;
}

.spotlight-error-text {
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
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

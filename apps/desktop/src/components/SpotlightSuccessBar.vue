<template>
  <div v-if="message" class="spotlight-success" role="status" aria-live="polite">
    <CircleCheckFilled class="spotlight-success-icon" aria-hidden="true" />
    <span class="spotlight-success-text">{{ message }}</span>
  </div>
</template>

<script setup lang="ts">
import { CircleCheckFilled } from "@element-plus/icons-vue";

defineProps<{
  message: string | null;
}>();
</script>

<style scoped>
.spotlight-success {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 16px;
  background: rgba(34, 197, 94, 0.1);
  border-top: 1px solid rgba(34, 197, 94, 0.28);
  color: #15803d;
  font-size: 12px;
}

.spotlight-success-icon {
  width: 14px;
  height: 14px;
  flex-shrink: 0;
}

.spotlight-success-text {
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
</style>

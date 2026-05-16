<template>
  <div class="vault-unlock" @keydown.stop>
    <div class="vault-unlock-head">
      <div class="vault-unlock-icon">🔒</div>
      <div class="vault-unlock-title">
        <div class="vault-unlock-line1">输入主密码以复制</div>
        <div class="vault-unlock-line2">{{ entryTitle }}</div>
      </div>
    </div>
    <input
      ref="inputRef"
      v-model="password"
      type="password"
      class="vault-unlock-input"
      :placeholder="locked ? `已锁定 ${remaining}s 后重试` : '主密码'"
      :disabled="locked || pending"
      autocomplete="off"
      spellcheck="false"
      @keydown="onKeydown"
    />
    <div v-if="errorText" class="vault-unlock-error">{{ errorText }}</div>
    <div class="vault-unlock-hint">Enter 确认 · Esc 取消</div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";

const props = defineProps<{
  entryTitle: string;
}>();

const emit = defineEmits<{
  (e: "submit", password: string): void;
  (e: "cancel"): void;
}>();

const inputRef = ref<HTMLInputElement | null>(null);
const password = ref("");
const pending = ref(false);
const errorText = ref<string | null>(null);
const failedAttempts = ref(0);
const locked = ref(false);
const remaining = ref(0);
let lockTimer: ReturnType<typeof setInterval> | null = null;

const LOCK_DURATION_SEC = 60;
const MAX_FAILURES = 3;

function startLockCountdown() {
  locked.value = true;
  remaining.value = LOCK_DURATION_SEC;
  if (lockTimer) clearInterval(lockTimer);
  lockTimer = setInterval(() => {
    remaining.value -= 1;
    if (remaining.value <= 0) {
      locked.value = false;
      failedAttempts.value = 0;
      errorText.value = null;
      if (lockTimer) {
        clearInterval(lockTimer);
        lockTimer = null;
      }
      nextTick(() => inputRef.value?.focus());
    }
  }, 1000);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    emit("cancel");
    return;
  }
  if (e.key === "Enter") {
    e.preventDefault();
    if (locked.value || pending.value) return;
    const value = password.value;
    if (!value) return;
    pending.value = true;
    emit("submit", value);
  }
}

function reportError(message: string) {
  errorText.value = message;
  pending.value = false;
  password.value = "";
  // 后端已限流的提示不再叠加前端计数，避免双重锁定
  const isThrottled =
    message.includes("尝试次数过多") || message.includes("稍后再试");
  if (!isThrottled) {
    failedAttempts.value += 1;
    if (failedAttempts.value >= MAX_FAILURES) {
      startLockCountdown();
      return;
    }
  }
  nextTick(() => inputRef.value?.focus());
}

defineExpose({ reportError });

onMounted(async () => {
  await nextTick();
  inputRef.value?.focus();
});

onBeforeUnmount(() => {
  if (lockTimer) clearInterval(lockTimer);
});
</script>

<style scoped>
.vault-unlock {
  padding: 16px 20px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  border-bottom: 1px solid rgba(0, 0, 0, 0.06);
}

.vault-unlock-head {
  display: flex;
  align-items: center;
  gap: 12px;
}

.vault-unlock-icon {
  width: 32px;
  height: 32px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(245, 158, 11, 0.12);
  font-size: 16px;
}

.vault-unlock-title {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.vault-unlock-line1 {
  font-size: 12px;
  color: #909399;
}

.vault-unlock-line2 {
  font-size: 14px;
  color: #303133;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 360px;
}

.vault-unlock-input {
  border: 1px solid rgba(0, 0, 0, 0.12);
  border-radius: 8px;
  padding: 10px 12px;
  font-size: 14px;
  outline: none;
  background: #fff;
  color: #303133;
  transition: border-color 0.15s ease;
}

.vault-unlock-input:focus {
  border-color: #409eff;
}

.vault-unlock-input:disabled {
  background: #f5f7fa;
  color: #a8abb2;
}

.vault-unlock-error {
  color: #f56c6c;
  font-size: 12px;
}

.vault-unlock-hint {
  color: #909399;
  font-size: 11px;
}
</style>

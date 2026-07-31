<template>
  <div class="vault-unlock" @keydown.stop>
    <div
      class="vault-unlock-row"
      :class="{
        'is-error': !!errorText && !locked,
        'is-locked': locked,
        'is-success': justUnlocked,
        'is-shake': shakeNonce > 0,
      }"
      :key="shakeNonce"
    >
      <span class="vault-unlock-prefix" :class="`is-${prefixState}`" aria-hidden="true">
        <svg
          v-if="prefixState === 'pending'"
          class="vault-unlock-prefix-spinner"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.4"
          stroke-linecap="round"
        >
          <path d="M21 12a9 9 0 1 1-6.219-8.56" />
        </svg>
        <svg
          v-else-if="prefixState === 'success'"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.6"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <polyline points="20 6 9 17 4 12" />
        </svg>
        <svg
          v-else
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <rect x="4" y="11" width="16" height="10" rx="2" />
          <path d="M8 11V7a4 4 0 0 1 8 0v4" />
        </svg>
      </span>
      <input
        ref="inputRef"
        v-model="password"
        type="password"
        class="vault-unlock-input"
        :placeholder="placeholder"
        :disabled="locked || justUnlocked"
        autocomplete="off"
        spellcheck="false"
        :aria-label="`输入主密码以复制 ${entryTitle}`"
        :aria-invalid="!!errorText && !locked"
        @input="onInput"
        @keydown="onKeydown"
      />
      <span class="vault-unlock-chip" :title="entryTitle">
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <rect x="4" y="11" width="16" height="10" rx="2" />
          <path d="M8 11V7a4 4 0 0 1 8 0v4" />
        </svg>
        <span class="vault-unlock-chip-label">{{ chipLabel }}</span>
      </span>
    </div>
    <transition name="vault-unlock-banner">
      <div v-if="errorText" class="vault-unlock-banner" role="alert">
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <circle cx="12" cy="12" r="9" />
          <path d="M12 8v4" />
          <circle cx="12" cy="16" r="0.6" fill="currentColor" />
        </svg>
        <span>{{ errorText }}</span>
      </div>
    </transition>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { invokeToolByChannel } from "../bridge/tauri";

const props = defineProps<{
  entryTitle: string;
}>();

const emit = defineEmits<{
  (e: "unlocked"): void;
  (e: "cancel"): void;
}>();

const AUTO_UNLOCK_DEBOUNCE_MS = 150;
const LOCK_DURATION_SEC = 60;
const MAX_FAILURES = 3;

const inputRef = ref<HTMLInputElement | null>(null);
const password = ref("");
const pending = ref(false);
const errorText = ref<string | null>(null);
const failedAttempts = ref(0);
const locked = ref(false);
const remaining = ref(0);
const justUnlocked = ref(false);
const shakeNonce = ref(0);

const attemptedPasswords = new Set<string>();
let debounceTimer: ReturnType<typeof setTimeout> | null = null;
let lockTimer: ReturnType<typeof setInterval> | null = null;
let attemptNonce = 0;
let isUnmounted = false;

const chipLabel = computed(() => {
  const raw = props.entryTitle?.trim();
  if (!raw) return "凭据";
  if (raw.length <= 22) return raw;
  return raw.slice(0, 21) + "…";
});

const placeholder = computed(() => {
  if (locked.value) return `已锁定 ${remaining.value}s 后重试`;
  if (justUnlocked.value) return "已解锁，正在复制…";
  return "输入主密码";
});

const prefixState = computed<"idle" | "pending" | "success" | "error" | "locked">(() => {
  if (justUnlocked.value) return "success";
  if (locked.value) return "locked";
  if (pending.value) return "pending";
  if (errorText.value) return "error";
  return "idle";
});

function clearDebounceTimer() {
  if (debounceTimer !== null) {
    clearTimeout(debounceTimer);
    debounceTimer = null;
  }
}

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
      attemptedPasswords.clear();
      if (lockTimer) {
        clearInterval(lockTimer);
        lockTimer = null;
      }
      nextTick(() => inputRef.value?.focus());
    }
  }, 1000);
}

function mapErrorMessage(raw: string): string {
  if (raw.includes("too_many_attempts")) return "尝试次数过多，请稍后再试";
  if (raw.includes("wrong_password") || raw.includes("bad_master_password")) {
    return "主密码错误";
  }
  return raw || "解锁失败";
}

function triggerShake() {
  shakeNonce.value += 1;
}

async function attemptUnlock(snapshot: string, manual: boolean) {
  if (!snapshot || isUnmounted || justUnlocked.value || locked.value) return;
  if (!manual && attemptedPasswords.has(snapshot)) return;
  attemptedPasswords.add(snapshot);

  const nonce = ++attemptNonce;
  pending.value = true;
  if (manual) errorText.value = null;

  try {
    await invokeToolByChannel("tool:vault:unlock", { masterPassword: snapshot });
    if (isUnmounted || nonce !== attemptNonce) return;
    justUnlocked.value = true;
    errorText.value = null;
    clearDebounceTimer();
    pending.value = false;
    emit("unlocked");
  } catch (err) {
    if (isUnmounted || nonce !== attemptNonce) return;
    const raw = err instanceof Error ? err.message : String(err);
    const mapped = mapErrorMessage(raw);
    const throttled = raw.includes("too_many_attempts");
    if (throttled) {
      errorText.value = mapped;
      triggerShake();
    }
    if (manual) {
      if (!throttled) {
        errorText.value = mapped;
        triggerShake();
        failedAttempts.value += 1;
        if (failedAttempts.value >= MAX_FAILURES) {
          startLockCountdown();
          errorText.value = null;
        }
      }
      password.value = "";
      attemptedPasswords.clear();
      nextTick(() => inputRef.value?.focus());
    }
  } finally {
    if (nonce === attemptNonce) pending.value = false;
  }
}

function onInput() {
  if (justUnlocked.value || locked.value) return;
  errorText.value = null;
  clearDebounceTimer();
  const snapshot = password.value;
  if (!snapshot) return;
  if (attemptedPasswords.has(snapshot)) return;
  debounceTimer = setTimeout(() => {
    debounceTimer = null;
    void attemptUnlock(snapshot, false);
  }, AUTO_UNLOCK_DEBOUNCE_MS);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    emit("cancel");
    return;
  }
  if (e.key === "Enter") {
    e.preventDefault();
    if (locked.value || justUnlocked.value) return;
    const snapshot = password.value;
    if (!snapshot) return;
    clearDebounceTimer();
    void attemptUnlock(snapshot, true);
  }
}

// 兼容性外部 API：父组件 vault:get 失败的兜底提示通过此入口注入
function reportError(message: string) {
  errorText.value = message;
  pending.value = false;
  password.value = "";
  attemptedPasswords.clear();
  triggerShake();
  nextTick(() => inputRef.value?.focus());
}

defineExpose({ reportError });

onMounted(async () => {
  await nextTick();
  inputRef.value?.focus();
});

onBeforeUnmount(() => {
  isUnmounted = true;
  clearDebounceTimer();
  if (lockTimer) clearInterval(lockTimer);
});
</script>

<style scoped>
.vault-unlock {
  flex-shrink: 0;
}

.vault-unlock-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 20px;
  min-height: 64px;
  border-bottom: 1px solid rgba(0, 0, 0, 0.06);
  background: #ffffff;
  transition:
    border-color 0.18s ease,
    background-color 0.18s ease;
}

.vault-unlock-row.is-error {
  border-bottom-color: #f56c6c;
}

.vault-unlock-row.is-success {
  background: linear-gradient(180deg, rgba(34, 197, 94, 0.06), transparent);
  border-bottom-color: rgba(34, 197, 94, 0.5);
}

.vault-unlock-row.is-locked {
  background: #fafbfc;
}

.vault-unlock-row.is-shake {
  animation: vault-unlock-shake 0.32s cubic-bezier(0.36, 0.07, 0.19, 0.97);
}

.vault-unlock-prefix {
  width: 22px;
  height: 22px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #909399;
  transition: color 0.18s ease;
}

.vault-unlock-prefix svg {
  width: 18px;
  height: 18px;
  transition: opacity 0.18s ease;
}

.vault-unlock-prefix.is-idle {
  color: #909399;
}

.vault-unlock-prefix.is-pending {
  color: #2563eb;
}

.vault-unlock-prefix.is-success {
  color: #15803d;
}

.vault-unlock-prefix.is-error {
  color: #f56c6c;
}

.vault-unlock-prefix.is-locked {
  color: #c0c4cc;
}

.vault-unlock-prefix-spinner {
  animation: vault-unlock-spin 0.85s linear infinite;
}

.vault-unlock-input {
  flex: 1;
  min-width: 0;
  height: 64px;
  border: none;
  outline: none;
  background: transparent;
  font-size: 16px;
  color: #303133;
  font-family: inherit;
  letter-spacing: 0.02em;
}

.vault-unlock-input::placeholder {
  color: #c0c4cc;
  letter-spacing: normal;
}

.vault-unlock-input:disabled {
  color: #a8abb2;
  cursor: not-allowed;
}

.vault-unlock-chip {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  max-width: 240px;
  padding: 4px 10px;
  border-radius: 999px;
  background: rgba(245, 158, 11, 0.12);
  color: #b45309;
  font-size: 11px;
  font-weight: 500;
  line-height: 1;
  transition:
    background-color 0.18s ease,
    color 0.18s ease;
}

.vault-unlock-row.is-success .vault-unlock-chip {
  background: rgba(34, 197, 94, 0.14);
  color: #15803d;
}

.vault-unlock-chip svg {
  width: 11px;
  height: 11px;
  flex-shrink: 0;
}

.vault-unlock-chip-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 200px;
}

.vault-unlock-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 20px;
  background: rgba(245, 108, 108, 0.08);
  color: #c45656;
  font-size: 12px;
  line-height: 1.4;
  border-bottom: 1px solid rgba(245, 108, 108, 0.22);
}

.vault-unlock-banner svg {
  width: 13px;
  height: 13px;
  flex-shrink: 0;
}

.vault-unlock-banner-enter-active,
.vault-unlock-banner-leave-active {
  transition:
    opacity 0.16s ease,
    max-height 0.16s ease;
  overflow: hidden;
}

.vault-unlock-banner-enter-from,
.vault-unlock-banner-leave-to {
  opacity: 0;
  max-height: 0;
}

.vault-unlock-banner-enter-to,
.vault-unlock-banner-leave-from {
  opacity: 1;
  max-height: 40px;
}

@keyframes vault-unlock-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

@keyframes vault-unlock-shake {
  0%,
  100% {
    transform: translateX(0);
  }
  20% {
    transform: translateX(-4px);
  }
  40% {
    transform: translateX(4px);
  }
  60% {
    transform: translateX(-3px);
  }
  80% {
    transform: translateX(2px);
  }
}

@media (prefers-reduced-motion: reduce) {
  .vault-unlock-prefix-spinner {
    animation: none;
  }
  .vault-unlock-row.is-shake {
    animation: none;
  }
  .vault-unlock-banner-enter-active,
  .vault-unlock-banner-leave-active {
    transition: opacity 0.12s linear;
  }
}
</style>

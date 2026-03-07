<template>
  <div class="vault-lock">
    <div class="vault-lock__card">
      <div class="vault-lock__icon">
        <svg viewBox="0 0 48 48" fill="none" xmlns="http://www.w3.org/2000/svg">
          <rect x="8" y="22" width="32" height="22" rx="4" stroke="currentColor" stroke-width="2.5" />
          <path d="M16 22V16a8 8 0 0 1 16 0v6" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" />
          <circle cx="24" cy="33" r="3" fill="currentColor" />
          <path d="M24 36v3" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" />
        </svg>
      </div>

      <!-- Setup mode -->
      <template v-if="mode === 'setup'">
        <h2 class="vault-lock__title">创建密码库</h2>
        <p class="vault-lock__hint">设置主密码以保护你的凭据数据。请务必牢记此密码，密码丢失将无法恢复数据。</p>
        <el-form @submit.prevent="onSetup" class="vault-lock__form">
          <el-form-item>
            <el-input
              :key="`setup-password-${maskVersion}`"
              ref="setupPasswordRef"
              v-model="setupPassword"
              type="password"
              placeholder="输入主密码"
              show-password
              size="large"
              @keyup.enter="$refs.setupConfirmRef?.focus()"
            />
          </el-form-item>
          <el-form-item>
            <el-input
              :key="`setup-confirm-${maskVersion}`"
              ref="setupConfirmRef"
              v-model="setupConfirm"
              type="password"
              placeholder="再次确认密码"
              show-password
              size="large"
              @keyup.enter="onSetup"
            />
          </el-form-item>
          <el-button
            type="primary"
            size="large"
            :loading="loading"
            :disabled="!setupPassword || !setupConfirm"
            class="vault-lock__btn"
            @click="onSetup"
          >创建密码库</el-button>
        </el-form>
      </template>

      <!-- Unlock mode -->
      <template v-else>
        <h2 class="vault-lock__title">解锁密码库</h2>
        <p class="vault-lock__hint">输入主密码以访问凭据数据</p>
        <el-form @submit.prevent="onUnlock" class="vault-lock__form">
          <el-form-item>
            <el-input
              :key="`unlock-password-${maskVersion}`"
              ref="unlockPasswordRef"
              v-model="unlockPassword"
              type="password"
              placeholder="输入主密码"
              show-password
              size="large"
              @keyup.enter="onUnlock"
            />
          </el-form-item>
          <el-button
            type="primary"
            size="large"
            :loading="loading"
            :disabled="!unlockPassword"
            class="vault-lock__btn"
            @click="onUnlock"
          >解锁</el-button>
        </el-form>
      </template>

      <p v-if="errorMsg" class="vault-lock__error">{{ errorMsg }}</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, nextTick } from "vue";
import type { InputInstance } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const props = defineProps<{
  mode: "setup" | "unlock";
  maskVersion?: number;
}>();

const maskVersion = computed(() => props.maskVersion ?? 0);

const emit = defineEmits<{
  (e: "unlocked"): void;
}>();

const setupPassword = ref("");
const setupConfirm = ref("");
const unlockPassword = ref("");
const loading = ref(false);
const errorMsg = ref("");

const setupPasswordRef = ref<InputInstance | null>(null);
const unlockPasswordRef = ref<InputInstance | null>(null);

onMounted(async () => {
  await nextTick();
  setTimeout(() => {
    if (props.mode === "setup") {
      setupPasswordRef.value?.focus();
    } else {
      unlockPasswordRef.value?.focus();
    }
  }, 150);
});

async function onSetup() {
  errorMsg.value = "";
  if (!setupPassword.value || !setupConfirm.value) return;
  if (setupPassword.value !== setupConfirm.value) {
    errorMsg.value = "两次输入的密码不一致";
    return;
  }
  if (setupPassword.value.length < 4) {
    errorMsg.value = "密码长度不能少于 4 位";
    return;
  }
  loading.value = true;
  try {
    await invokeToolByChannel("tool:vault:setup", { masterPassword: setupPassword.value });
    emit("unlocked");
  } catch (err) {
    errorMsg.value = (err as Error).message || "创建失败";
  } finally {
    loading.value = false;
  }
}

async function onUnlock() {
  errorMsg.value = "";
  if (!unlockPassword.value) return;
  loading.value = true;
  try {
    await invokeToolByChannel("tool:vault:unlock", { masterPassword: unlockPassword.value });
    emit("unlocked");
  } catch (err) {
    const msg = (err as Error).message || "";
    if (msg.includes("wrong_password")) {
      errorMsg.value = "密码错误，请重试";
    } else {
      errorMsg.value = msg || "解锁失败";
    }
  } finally {
    loading.value = false;
  }
}
</script>

<style scoped>
.vault-lock {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  background:
    radial-gradient(ellipse at 30% 20%, var(--lc-accent-dim) 0%, transparent 50%),
    var(--lc-bg);
}

.vault-lock__card {
  display: flex;
  flex-direction: column;
  align-items: center;
  width: 380px;
  padding: 40px 36px 36px;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-lg);
  background: var(--lc-surface-0);
  box-shadow: var(--lc-shadow-md);
}

.vault-lock__icon {
  width: 56px;
  height: 56px;
  color: var(--lc-accent);
  margin-bottom: 20px;
  opacity: 0.85;
  animation: iconPulse 3s ease-in-out infinite;
}

@keyframes iconPulse {
  0%, 100% {
    opacity: 0.85;
    transform: scale(1);
  }
  50% {
    opacity: 1;
    transform: scale(1.02);
  }
}

.vault-lock__title {
  margin: 0 0 8px;
  font-family: var(--lc-font-display);
  font-size: 22px;
  font-weight: 700;
  color: var(--lc-text);
  letter-spacing: -0.3px;
}

.vault-lock__hint {
  margin: 0 0 24px;
  font-size: 13px;
  color: var(--lc-text-secondary);
  text-align: center;
  line-height: 1.6;
}

.vault-lock__form {
  width: 100%;
}

.vault-lock__form .el-form-item {
  margin-bottom: 16px;
}

.vault-lock__btn {
  width: 100%;
  margin-top: 4px;
}

.vault-lock__error {
  margin: 12px 0 0;
  font-size: 13px;
  color: var(--el-color-danger);
  text-align: center;
}
</style>

<template>
  <section v-if="visible" class="vault-inline-unlock" aria-live="polite">
    <div class="vault-inline-unlock__summary">
      <span>绑定凭据</span>
      <strong>{{ credentialLabel }}</strong>
    </div>
    <el-form-item label="Vault 主密码" :error="error">
      <el-input
        ref="inputRef"
        :model-value="modelValue"
        type="password"
        show-password
        autocomplete="current-password"
        placeholder="输入主密码"
        :disabled="submitting"
        @update:model-value="$emit('update:modelValue', $event)"
        @keyup.enter="$emit('submit')"
      />
    </el-form-item>
    <el-button
      type="primary"
      :loading="submitting"
      :disabled="submitting || !modelValue"
      @click="$emit('submit')"
    >
      解锁并继续
    </el-button>
  </section>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import type { InputInstance } from "element-plus";

const props = defineProps<{
  visible: boolean;
  credentialLabel: string;
  modelValue: string;
  error: string;
  submitting: boolean;
  focusNonce: number;
}>();

defineEmits<{
  (event: "update:modelValue", value: string): void;
  (event: "submit"): void;
}>();

const inputRef = ref<InputInstance | null>(null);

watch(
  () => [props.visible, props.focusNonce] as const,
  async ([visible]) => {
    if (!visible) return;
    await nextTick();
    if (typeof inputRef.value?.focus === "function") inputRef.value.focus();
  },
  { immediate: true },
);
</script>

<style scoped>
.vault-inline-unlock {
  display: grid;
  gap: 10px;
  margin-top: 14px;
  padding: 12px;
  border: 1px solid #e6a23c;
  border-radius: 6px;
  background: #fdf6ec;
}

.vault-inline-unlock__summary {
  display: flex;
  min-width: 0;
  gap: 10px;
  align-items: baseline;
}

.vault-inline-unlock__summary span {
  flex: 0 0 auto;
  color: #606266;
  font-size: 12px;
}

.vault-inline-unlock__summary strong {
  min-width: 0;
  overflow-wrap: anywhere;
}

.vault-inline-unlock :deep(.el-form-item) {
  margin-bottom: 0;
}

.vault-inline-unlock > :deep(.el-button) {
  justify-self: start;
}
</style>

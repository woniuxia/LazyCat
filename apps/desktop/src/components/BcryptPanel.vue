<template>
  <div class="bcrypt-panel">
    <el-card shadow="never">
      <template #header><span>Bcrypt 生成</span></template>
      <div class="form-row">
        <el-input v-model="hashPassword" placeholder="输入密码" clearable />
        <el-input-number v-model="hashCost" :min="4" :max="16" controls-position="right" />
        <span class="cost-label">cost</span>
        <el-button type="primary" @click="doHash">生成</el-button>
      </div>
      <div v-if="hashResult" class="result-box">
        <el-input v-model="hashResult" readonly>
          <template #append>
            <el-button @click="copyText(hashResult)">复制</el-button>
          </template>
        </el-input>
      </div>
    </el-card>

    <el-card shadow="never">
      <template #header><span>Bcrypt 验证</span></template>
      <div class="form-col">
        <el-input v-model="verifyPassword" placeholder="输入密码" clearable />
        <el-input v-model="verifyHash" placeholder="输入 bcrypt hash" clearable />
        <el-button type="primary" @click="doVerify">验证</el-button>
      </div>
      <div v-if="verifyResult !== null" class="verify-result" :class="verifyResult ? 'pass' : 'fail'">
        {{ verifyResult ? "匹配" : "不匹配" }}
      </div>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invokeToolByChannel } from "../bridge/tauri";
import { useClipboardSuggestion } from "../composables/useClipboardSuggestion";

const hashPassword = ref("");
const hashCost = ref(10);
const hashResult = ref("");

const verifyPassword = ref("");
const verifyHash = ref("");
const verifyResult = ref<boolean | null>(null);

async function doHash() {
  if (!hashPassword.value) return;
  try {
    const res = (await invokeToolByChannel("tool:crypto:bcrypt-hash", {
      password: hashPassword.value,
      cost: hashCost.value,
    })) as { hash: string };
    hashResult.value = res.hash;
  } catch {
    hashResult.value = "";
  }
}

async function doVerify() {
  if (!verifyPassword.value || !verifyHash.value) return;
  try {
    const res = (await invokeToolByChannel("tool:crypto:bcrypt-verify", {
      password: verifyPassword.value,
      hash: verifyHash.value,
    })) as { valid: boolean };
    verifyResult.value = res.valid;
  } catch {
    verifyResult.value = null;
  }
}

function copyText(text: string) {
  navigator.clipboard.writeText(text);
}

const { consumePendingInput } = useClipboardSuggestion();
onMounted(() => {
  const pending = consumePendingInput("bcrypt");
  if (pending) verifyHash.value = pending;
});
</script>

<style scoped>
.bcrypt-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.form-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.form-row .el-input {
  flex: 1;
}
.form-row .el-input-number {
  width: 100px;
}
.cost-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.form-col {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.result-box {
  margin-top: 12px;
}
.verify-result {
  margin-top: 12px;
  padding: 8px 16px;
  border-radius: 6px;
  font-weight: 600;
  text-align: center;
}
.verify-result.pass {
  background: var(--el-color-success-light-9);
  color: var(--el-color-success);
}
.verify-result.fail {
  background: var(--el-color-danger-light-9);
  color: var(--el-color-danger);
}
</style>

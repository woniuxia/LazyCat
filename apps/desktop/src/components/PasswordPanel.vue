<template>
  <div class="password-panel">
    <el-card shadow="never">
      <template #header><span>密码生成</span></template>
      <div class="gen-options">
        <div class="opt-row">
          <span class="opt-label">长度</span>
          <el-input-number v-model="passwordLength" :min="4" :max="128" controls-position="right" />
        </div>
        <div class="opt-row">
          <el-checkbox v-model="passwordUppercase">大写字母</el-checkbox>
          <el-checkbox v-model="passwordLowercase">小写字母</el-checkbox>
          <el-checkbox v-model="passwordNumbers">数字</el-checkbox>
          <el-checkbox v-model="passwordSymbols">特殊符号</el-checkbox>
        </div>
      </div>
      <div class="gen-output">
        <el-input v-model="generatedPassword" readonly>
          <template #append>
            <el-button @click="copyText(generatedPassword)">复制</el-button>
          </template>
        </el-input>
        <el-button type="primary" @click="generatePassword">生成密码</el-button>
      </div>
    </el-card>

    <el-card shadow="never">
      <template #header><span>密码强度分析</span></template>
      <div class="analyze-input">
        <el-input v-model="manualPassword" placeholder="输入密码进行强度分析" clearable @keyup.enter="analyzeManual">
          <template #append>
            <el-button @click="analyzeManual">分析</el-button>
          </template>
        </el-input>
      </div>
      <div v-if="strengthResult" class="strength-result">
        <div class="strength-bar">
          <span class="strength-text" :style="{ color: strengthColor }">{{ strengthLevelText }}</span>
          <el-progress :percentage="strengthResult.score" :color="strengthColor" :show-text="false" />
          <span class="strength-score">{{ strengthResult.score }} / 100</span>
        </div>
        <div class="strength-details">
          <div v-for="d in strengthResult.details" :key="d.rule" class="detail-item" :class="d.passed ? 'pass' : 'fail'">
            <span class="detail-icon">{{ d.passed ? "\u2713" : "\u2717" }}</span>
            <span>{{ d.message }}</span>
          </div>
        </div>
      </div>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const passwordLength = ref(20);
const passwordSymbols = ref(true);
const passwordNumbers = ref(true);
const passwordUppercase = ref(true);
const passwordLowercase = ref(true);
const generatedPassword = ref("");
const manualPassword = ref("");

interface StrengthDetail {
  rule: string;
  passed: boolean;
  message: string;
}
interface StrengthResult {
  score: number;
  level: string;
  details: StrengthDetail[];
}
const strengthResult = ref<StrengthResult | null>(null);

const strengthLevelText = computed(() => {
  const map: Record<string, string> = {
    weak: "弱",
    medium: "中等",
    strong: "强",
    very_strong: "非常强",
  };
  return map[strengthResult.value?.level ?? ""] ?? "";
});

const strengthColor = computed(() => {
  const map: Record<string, string> = {
    weak: "#F56C6C",
    medium: "#E6A23C",
    strong: "#409EFF",
    very_strong: "#67C23A",
  };
  return map[strengthResult.value?.level ?? ""] ?? "#909399";
});

async function analyzeStrength(password: string) {
  try {
    const data = (await invokeToolByChannel("tool:gen:password-strength", {
      password,
    })) as StrengthResult;
    strengthResult.value = data;
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function generatePassword() {
  try {
    const pw = String(
      await invokeToolByChannel("tool:gen:password", {
        length: passwordLength.value,
        symbols: passwordSymbols.value,
        numbers: passwordNumbers.value,
        uppercase: passwordUppercase.value,
        lowercase: passwordLowercase.value,
      })
    );
    generatedPassword.value = pw;
    manualPassword.value = pw;
    await analyzeStrength(pw);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function analyzeManual() {
  if (!manualPassword.value) {
    ElMessage.warning("请输入密码");
    return;
  }
  await analyzeStrength(manualPassword.value);
}

function copyText(text: string) {
  if (!text) return;
  navigator.clipboard.writeText(text).then(() => ElMessage.success("已复制"));
}

let timer: ReturnType<typeof setTimeout> | null = null;
watch(manualPassword, (val) => {
  if (timer) clearTimeout(timer);
  if (!val) {
    strengthResult.value = null;
    return;
  }
  timer = setTimeout(() => analyzeStrength(val), 300);
});
</script>

<style scoped>
.password-panel {
  display: flex;
  flex-direction: column;
  gap: 14px;
  max-width: 600px;
}
.gen-options {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-bottom: 12px;
}
.opt-row {
  display: flex;
  align-items: center;
  gap: 12px;
}
.opt-row .el-checkbox {
  margin-right: 0;
}
.opt-label {
  font-size: 13px;
  color: var(--el-text-color-secondary);
}
.gen-output {
  display: flex;
  gap: 8px;
  align-items: center;
}
.gen-output .el-input {
  flex: 1;
}
.analyze-input {
  margin-bottom: 12px;
}
.strength-result {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.strength-bar {
  display: flex;
  align-items: center;
  gap: 10px;
}
.strength-text {
  font-weight: 700;
  font-size: 14px;
  min-width: 56px;
}
.strength-bar .el-progress {
  flex: 1;
}
.strength-score {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  white-space: nowrap;
}
.strength-details {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.detail-item {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  padding: 4px 8px;
  border-radius: 4px;
}
.detail-item.pass {
  color: var(--el-color-success);
  background: var(--el-color-success-light-9);
}
.detail-item.fail {
  color: var(--el-color-danger);
  background: var(--el-color-danger-light-9);
}
.detail-icon {
  font-weight: 700;
}
</style>

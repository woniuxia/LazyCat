<template>
  <el-config-provider :locale="zhCn">
  <div class="date-calc-panel">
    <el-card shadow="never">
      <template #header><span>日期间隔</span></template>
      <div class="calc-row">
        <el-date-picker v-model="diffStart" type="date" placeholder="开始日期" value-format="YYYY-MM-DD" />
        <span>至</span>
        <el-date-picker v-model="diffEnd" type="date" placeholder="结束日期" value-format="YYYY-MM-DD" />
        <el-button type="primary" @click="calcDiff">计算</el-button>
      </div>
      <div v-if="diffResult" class="result-grid">
        <div class="result-item">
          <div class="result-label">天数</div>
          <div class="result-value">{{ diffResult.days }}</div>
        </div>
        <div class="result-item">
          <div class="result-label">小时</div>
          <div class="result-value">{{ diffResult.hours }}</div>
        </div>
        <div class="result-item">
          <div class="result-label">分钟</div>
          <div class="result-value">{{ diffResult.minutes }}</div>
        </div>
        <div class="result-item">
          <div class="result-label">秒</div>
          <div class="result-value">{{ diffResult.seconds }}</div>
        </div>
        <div class="result-item natural">
          <div class="result-label">自然语言</div>
          <div class="result-value">{{ diffResult.natural }}</div>
        </div>
      </div>
    </el-card>

    <el-card shadow="never">
      <template #header><span>日期加减</span></template>
      <div class="calc-row">
        <el-date-picker v-model="addDate" type="date" placeholder="基准日期" value-format="YYYY-MM-DD" />
        <span>加</span>
        <el-input-number v-model="addDays" :min="-99999" :max="99999" controls-position="right" />
        <span>天</span>
        <el-input-number v-model="addHours" :min="-99999" :max="99999" controls-position="right" />
        <span>时</span>
        <el-input-number v-model="addMinutes" :min="-99999" :max="99999" controls-position="right" />
        <span>分</span>
        <el-button type="primary" @click="calcAdd">计算</el-button>
      </div>
      <div v-if="addResult" class="result-grid">
        <div class="result-item">
          <div class="result-label">结果日期</div>
          <div class="result-value">{{ addResult.result }}</div>
        </div>
        <div class="result-item">
          <div class="result-label">完整时间</div>
          <div class="result-value">{{ addResult.resultDatetime }}</div>
        </div>
      </div>
    </el-card>
  </div>
  </el-config-provider>
</template>

<script setup lang="ts">
import { ref } from "vue";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import { invokeToolByChannel } from "../bridge/tauri";

const diffStart = ref("");
const diffEnd = ref("");
const diffResult = ref<{ days: number; hours: number; minutes: number; seconds: number; natural: string } | null>(null);

const addDate = ref("");
const addDays = ref(0);
const addHours = ref(0);
const addMinutes = ref(0);
const addResult = ref<{ result: string; resultDatetime: string } | null>(null);

async function calcDiff() {
  if (!diffStart.value || !diffEnd.value) return;
  try {
    const res = (await invokeToolByChannel("tool:time:date-diff", {
      start: diffStart.value,
      end: diffEnd.value,
    })) as { days: number; hours: number; minutes: number; seconds: number; natural: string };
    diffResult.value = res;
  } catch (e) {
    diffResult.value = null;
  }
}

async function calcAdd() {
  if (!addDate.value) return;
  try {
    const res = (await invokeToolByChannel("tool:time:date-add", {
      date: addDate.value,
      add: { days: addDays.value, hours: addHours.value, minutes: addMinutes.value },
    })) as { result: string; resultDatetime: string };
    addResult.value = res;
  } catch (e) {
    addResult.value = null;
  }
}
</script>

<style scoped>
.date-calc-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.calc-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.calc-row .el-input-number {
  width: 120px;
}
.result-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  margin-top: 12px;
}
.result-item {
  background: var(--el-fill-color-light);
  border-radius: 6px;
  padding: 10px 16px;
  min-width: 100px;
}
.result-item.natural {
  flex: 1 1 100%;
}
.result-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.result-value {
  font-size: 16px;
  font-weight: 600;
  margin-top: 2px;
}
</style>

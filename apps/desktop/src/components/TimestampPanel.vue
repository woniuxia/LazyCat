<template>
  <div class="panel-grid">
    <div class="panel-grid-full" style="display:flex;gap:8px;align-items:center;">
      <span style="white-space:nowrap;color:var(--el-text-color-secondary);font-size:13px;">时间戳 → 日期</span>
      <el-input v-model="timeInput" placeholder="时间戳" style="flex:1;" />
      <el-button-group>
        <el-button :type="timePrecision === 's' ? 'primary' : ''" @click="timePrecision = 's'; onTimePrecisionChange()">秒</el-button>
        <el-button :type="timePrecision === 'ms' ? 'primary' : ''" @click="timePrecision = 'ms'; onTimePrecisionChange()">毫秒</el-button>
      </el-button-group>
      <el-input v-model="timeOutput" readonly placeholder="日期结果" style="flex:1;" />
    </div>
    <div class="panel-grid-full" style="display:flex;gap:8px;align-items:center;">
      <span style="white-space:nowrap;color:var(--el-text-color-secondary);font-size:13px;">日期 → 时间戳</span>
      <el-input v-model="dateInput" placeholder="日期，如 2024-01-01 00:00:00" style="flex:1;" />
      <el-button-group>
        <el-button :type="datePrecision === 's' ? 'primary' : ''" @click="datePrecision = 's'; onDatePrecisionChange()">秒</el-button>
        <el-button :type="datePrecision === 'ms' ? 'primary' : ''" @click="datePrecision = 'ms'; onDatePrecisionChange()">毫秒</el-button>
      </el-button-group>
      <el-input v-model="dateOutput" readonly placeholder="时间戳结果" style="flex:1;" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const timeInput = ref("");
const timeOutput = ref("");
const timePrecision = ref<"s" | "ms">("s");
const dateInput = ref("");
const dateOutput = ref("");
const datePrecision = ref<"s" | "ms">("s");

// Initialize with current time on mount
const now = new Date();
const pad = (n: number) => String(n).padStart(2, "0");
timeInput.value = String(Math.floor(Date.now() / 1000));
dateInput.value = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}`;

async function timestampToDate() {
  try {
    const ts = Number(timeInput.value);
    timeOutput.value = String(
      await invokeToolByChannel("tool:time:timestamp-to-date", { input: ts, timezone: "local" }),
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function dateToTimestamp() {
  try {
    const data = (await invokeToolByChannel("tool:time:date-to-timestamp", {
      input: dateInput.value,
    })) as { seconds: number; milliseconds: number };
    dateOutput.value =
      datePrecision.value === "s" ? String(data.seconds) : String(data.milliseconds);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function onDatePrecisionChange() {
  if (!dateOutput.value) return;
  const num = Number(dateOutput.value);
  if (!Number.isFinite(num)) return;
  if (datePrecision.value === "ms" && num < 1_000_000_000_000) {
    dateOutput.value = String(num * 1000);
  } else if (datePrecision.value === "s" && num >= 1_000_000_000_000) {
    dateOutput.value = String(Math.floor(num / 1000));
  }
}

function onTimePrecisionChange() {
  const val = timeInput.value.trim();
  if (/^\d+$/.test(val)) {
    const num = Number(val);
    if (timePrecision.value === "ms" && num < 1_000_000_000_000) {
      timeInput.value = String(num * 1000);
    } else if (timePrecision.value === "s" && num >= 1_000_000_000_000) {
      timeInput.value = String(Math.floor(num / 1000));
    }
  }
}

// Auto-convert on input change
let timer: ReturnType<typeof setTimeout> | null = null;
watch([timeInput, timePrecision, dateInput, datePrecision], () => {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    if (timeInput.value.trim()) timestampToDate();
    if (dateInput.value.trim()) dateToTimestamp();
  }, 300);
});
</script>

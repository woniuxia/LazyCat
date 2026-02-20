<template>
  <div class="panel-grid">
    <div class="panel-grid-full" style="display:flex;flex-direction:column;gap:8px;width:50%;">
      <div style="display:flex;align-items:center;gap:8px;">
        <span style="width:28px;flex-shrink:0;font-size:13px;color:var(--el-text-color-secondary);">秒</span>
        <el-input v-model="cronSecond" placeholder="0" style="flex:1;" />
      </div>
      <div style="display:flex;align-items:center;gap:8px;">
        <span style="width:28px;flex-shrink:0;font-size:13px;color:var(--el-text-color-secondary);">分</span>
        <el-input v-model="cronMinute" placeholder="*" style="flex:1;" />
      </div>
      <div style="display:flex;align-items:center;gap:8px;">
        <span style="width:28px;flex-shrink:0;font-size:13px;color:var(--el-text-color-secondary);">时</span>
        <el-input v-model="cronHour" placeholder="*" style="flex:1;" />
      </div>
      <div style="display:flex;align-items:center;gap:8px;">
        <span style="width:28px;flex-shrink:0;font-size:13px;color:var(--el-text-color-secondary);">日</span>
        <el-input v-model="cronDom" placeholder="*" style="flex:1;" />
      </div>
      <div style="display:flex;align-items:center;gap:8px;">
        <span style="width:28px;flex-shrink:0;font-size:13px;color:var(--el-text-color-secondary);">月</span>
        <el-input v-model="cronMonth" placeholder="*" style="flex:1;" />
      </div>
      <div style="display:flex;align-items:center;gap:8px;">
        <span style="width:28px;flex-shrink:0;font-size:13px;color:var(--el-text-color-secondary);">周</span>
        <el-input v-model="cronDow" placeholder="*" style="flex:1;" />
      </div>
    </div>
    <el-input class="panel-grid-full" v-model="cronExpression" placeholder="Cron 表达式（可直接粘贴后点解析）" style="margin-left:36px;width:calc(50% - 36px);" />
    <div class="panel-grid-full">
      <el-space>
        <el-button @click="parseCron">解析表达式</el-button>
        <el-button @click="previewCron">预览触发时间</el-button>
      </el-space>
    </div>
    <el-input class="panel-grid-full" v-model="cronOutput" type="textarea" :rows="8" readonly />
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const cronSecond = ref("0");
const cronMinute = ref("*");
const cronHour = ref("*");
const cronDom = ref("*");
const cronMonth = ref("*");
const cronDow = ref("*");
const cronExpression = ref("0 * * * * *");
const cronOutput = ref("");

watch([cronSecond, cronMinute, cronHour, cronDom, cronMonth, cronDow], ([s, m, h, d, mo, dw]) => {
  cronExpression.value = `${s} ${m} ${h} ${d} ${mo} ${dw}`;
});

async function previewCron() {
  try {
    const data = await invokeToolByChannel("tool:cron:preview", {
      expression: cronExpression.value,
      count: 8,
    });
    cronOutput.value = (data as string[]).join("\n");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function parseCron() {
  try {
    const data = (await invokeToolByChannel("tool:cron:parse", {
      expression: cronExpression.value,
    })) as {
      second: string;
      minute: string;
      hour: string;
      dayOfMonth: string;
      month: string;
      dayOfWeek: string;
    };
    cronSecond.value = data.second;
    cronMinute.value = data.minute;
    cronHour.value = data.hour;
    cronDom.value = data.dayOfMonth;
    cronMonth.value = data.month;
    cronDow.value = data.dayOfWeek;
    ElMessage.success("解析成功");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}
</script>

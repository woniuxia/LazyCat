<template>
  <div class="panel-grid">
    <div class="panel-grid-full">
      <el-button type="primary" @click="detectEnv">检测 Node / Java 版本</el-button>
    </div>
    <el-input class="panel-grid-full" v-model="envOutput" type="textarea" :rows="8" readonly />
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const envOutput = ref("");

async function detectEnv() {
  try {
    const data = await invokeToolByChannel("tool:env:detect", {});
    envOutput.value = JSON.stringify(data, null, 2);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}
</script>

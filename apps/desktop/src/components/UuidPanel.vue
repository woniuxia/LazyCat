<template>
  <div class="uuid-panel">
    <el-input v-model="idOutput" type="textarea" :rows="8" readonly placeholder="生成结果" />
    <div class="btn-row">
      <el-button type="primary" @click="gen('tool:gen:uuid')">UUID</el-button>
      <el-button @click="gen('tool:gen:uuid-simple')">UUID (无横线)</el-button>
      <el-button @click="gen('tool:gen:guid')">GUID</el-button>
      <el-button @click="gen('tool:gen:snowflake')">雪花 ID</el-button>
      <el-button v-if="idOutput" text type="primary" @click="copyText">复制</el-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const idOutput = ref("");

async function gen(channel: string) {
  try {
    idOutput.value = String(await invokeToolByChannel(channel, {}));
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function copyText() {
  navigator.clipboard.writeText(idOutput.value).then(() => ElMessage.success("已复制"));
}
</script>

<style scoped>
.uuid-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  max-width: 600px;
}
.btn-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
</style>

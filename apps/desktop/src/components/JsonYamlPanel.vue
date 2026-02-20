<template>
  <div class="panel-grid">
    <el-input v-model="convertInput" type="textarea" :rows="12" placeholder="输入 JSON" />
    <el-input v-model="convertOutput" type="textarea" :rows="12" readonly placeholder="YAML 结果" />
    <div class="panel-grid-full">
      <el-button type="primary" @click="convert">JSON -> YAML</el-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const convertInput = ref("");
const convertOutput = ref("");

async function convert() {
  try {
    convertOutput.value = String(
      await invokeToolByChannel("tool:convert:json-to-yaml", { input: convertInput.value }),
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

let timer: ReturnType<typeof setTimeout> | null = null;
watch(convertInput, () => {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    if (!convertInput.value.trim()) {
      convertOutput.value = "";
      return;
    }
    convert();
  }, 300);
});
</script>

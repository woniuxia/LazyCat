<template>
  <div class="panel-grid">
    <el-input v-model="convertInput" type="textarea" :rows="12" placeholder="输入 JSON 或 XML" />
    <el-input v-model="convertOutput" type="textarea" :rows="12" readonly placeholder="转换结果" />
    <div class="panel-grid-full">
      <el-space>
        <el-button type="primary" @click="convert('tool:convert:json-to-xml')">JSON -> XML</el-button>
        <el-button @click="convert('tool:convert:xml-to-json')">XML -> JSON</el-button>
      </el-space>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const convertInput = ref("");
const convertOutput = ref("");

async function convert(channel: string) {
  try {
    convertOutput.value = String(
      await invokeToolByChannel(channel, { input: convertInput.value }),
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

// Auto-convert on input change (debounced)
let timer: ReturnType<typeof setTimeout> | null = null;
watch(convertInput, () => {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    if (!convertInput.value.trim()) {
      convertOutput.value = "";
      return;
    }
    convert("tool:convert:json-to-xml");
  }, 300);
});
</script>

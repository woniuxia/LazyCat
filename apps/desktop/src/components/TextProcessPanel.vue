<template>
  <div class="panel-grid">
    <el-input v-model="textInput" type="textarea" :rows="12" placeholder="输入多行文本" />
    <el-input v-model="textOutput" type="textarea" :rows="12" readonly placeholder="处理结果" />
    <el-switch v-model="caseSensitive" active-text="区分大小写" />
    <div>
      <el-space>
        <el-button type="primary" @click="dedupeLines">按行去重</el-button>
        <el-button @click="sortLines">按行排序</el-button>
      </el-space>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const textInput = ref("");
const textOutput = ref("");
const caseSensitive = ref(false);

async function dedupeLines() {
  try {
    textOutput.value = String(
      await invokeToolByChannel("tool:text:unique-lines", {
        input: textInput.value,
        caseSensitive: caseSensitive.value,
      }),
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function sortLines() {
  try {
    textOutput.value = String(
      await invokeToolByChannel("tool:text:sort-lines", {
        input: textInput.value,
        caseSensitive: caseSensitive.value,
      }),
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

// Auto-process on input change
let timer: ReturnType<typeof setTimeout> | null = null;
watch([textInput, caseSensitive], () => {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    if (!textInput.value.trim()) {
      textOutput.value = "";
      return;
    }
    dedupeLines();
  }, 300);
});
</script>

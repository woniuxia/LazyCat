<template>
  <div class="panel-grid">
    <el-input v-model="sourcePath" placeholder="源文件路径" />
    <el-input v-model="outputDir" placeholder="分片输出目录" />
    <el-input-number v-model="chunkSizeMb" :min="1" :max="2048" />
    <div>
      <el-button type="primary" @click="splitFile">切割文件</el-button>
    </div>
    <el-input class="panel-grid-full" v-model="partsInput" type="textarea" :rows="5" placeholder="待合并分片路径（每行一个）" />
    <el-input v-model="mergeOutputPath" placeholder="合并输出文件路径" />
    <div>
      <el-button @click="mergeFiles">合并文件</el-button>
    </div>
    <el-input class="panel-grid-full" v-model="fileToolOutput" type="textarea" :rows="10" readonly />
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const sourcePath = ref("");
const outputDir = ref("");
const chunkSizeMb = ref(100);
const partsInput = ref("");
const mergeOutputPath = ref("");
const fileToolOutput = ref("");

async function splitFile() {
  try {
    const data = await invokeToolByChannel("tool:file:split", {
      sourcePath: sourcePath.value,
      outputDir: outputDir.value,
      chunkSizeMb: chunkSizeMb.value,
    });
    fileToolOutput.value = JSON.stringify(data, null, 2);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function mergeFiles() {
  try {
    const parts = partsInput.value
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);
    const data = await invokeToolByChannel("tool:file:merge", {
      parts,
      outputPath: mergeOutputPath.value,
    });
    fileToolOutput.value = JSON.stringify(data, null, 2);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}
</script>

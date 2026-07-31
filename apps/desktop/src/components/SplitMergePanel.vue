<template>
  <div class="panel-grid">
    <div class="panel-grid-full split-merge-section-title">文件切分</div>
    <div class="panel-grid-full split-merge-path-row">
      <el-button @click="pickSourceFile">选择源文件</el-button>
      <el-input v-model="sourcePath" placeholder="待切分文件路径" />
    </div>
    <div class="panel-grid-full split-merge-path-row">
      <el-button @click="pickSplitOutputDir">选择输出文件夹</el-button>
      <el-input v-model="outputDir" placeholder="切分后文件夹路径" />
    </div>
    <div class="split-merge-size-row">
      <span class="split-merge-size-label">分片大小(MB)</span>
      <el-input-number v-model="chunkSizeMb" :min="1" :max="2048" />
      <el-button type="primary" @click="splitFile">执行切分</el-button>
    </div>

    <div class="panel-grid-full split-merge-section-title">文件合并</div>
    <div class="panel-grid-full split-merge-path-row">
      <el-button @click="pickMergeInputDir">选择分片文件夹</el-button>
      <el-input v-model="partsDir" placeholder="待合并分片所在文件夹" />
    </div>
    <div class="panel-grid-full split-merge-path-row">
      <el-button @click="pickMergeOutputPath">选择输出文件</el-button>
      <el-input v-model="mergeOutputPath" placeholder="合并输出文件路径" />
    </div>
    <el-input
      class="panel-grid-full"
      v-model="partsInput"
      type="textarea"
      :rows="4"
      placeholder="可选：手动指定分片路径（每行一个，优先于文件夹选择）"
    />
    <div>
      <el-button @click="mergeFiles">执行合并</el-button>
    </div>

    <el-input
      class="panel-grid-full"
      v-model="fileToolOutput"
      type="textarea"
      :rows="10"
      readonly
    />
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import { open, save } from "@tauri-apps/plugin-dialog";
import { invokeToolByChannel } from "../bridge/tauri";

const sourcePath = ref("");
const outputDir = ref("");
const chunkSizeMb = ref(100);
const partsDir = ref("");
const partsInput = ref("");
const mergeOutputPath = ref("");
const fileToolOutput = ref("");

function toDialogPath(result: string | { path: string } | null): string {
  if (!result) return "";
  return typeof result === "string" ? result : result.path;
}

async function pickSourceFile() {
  try {
    const selected = await open({ multiple: false });
    sourcePath.value =
      toDialogPath(selected as string | { path: string } | null) || sourcePath.value;
  } catch {
    // dialog cancelled
  }
}

async function pickSplitOutputDir() {
  try {
    const selected = await open({ directory: true, multiple: false });
    outputDir.value = toDialogPath(selected as string | { path: string } | null) || outputDir.value;
  } catch {
    // dialog cancelled
  }
}

async function pickMergeInputDir() {
  try {
    const selected = await open({ directory: true, multiple: false });
    partsDir.value = toDialogPath(selected as string | { path: string } | null) || partsDir.value;
  } catch {
    // dialog cancelled
  }
}

async function pickMergeOutputPath() {
  try {
    const selected = await save({
      defaultPath: mergeOutputPath.value || undefined,
    });
    if (selected) {
      mergeOutputPath.value = selected;
    }
  } catch {
    // dialog cancelled
  }
}

async function splitFile() {
  if (!sourcePath.value.trim()) {
    ElMessage.warning("请先选择要切分的文件");
    return;
  }
  if (!outputDir.value.trim()) {
    ElMessage.warning("请先选择切分输出文件夹");
    return;
  }

  try {
    const data = await invokeToolByChannel("tool:file:split", {
      sourcePath: sourcePath.value,
      outputDir: outputDir.value,
      chunkSizeMb: chunkSizeMb.value,
    });
    fileToolOutput.value = JSON.stringify(data, null, 2);
    ElMessage.success("文件切分完成");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function mergeFiles() {
  if (!mergeOutputPath.value.trim()) {
    ElMessage.warning("请先选择合并输出文件");
    return;
  }
  const parts = partsInput.value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
  if (parts.length === 0 && !partsDir.value.trim()) {
    ElMessage.warning("请先选择分片文件夹或填写分片路径列表");
    return;
  }

  try {
    const data = await invokeToolByChannel("tool:file:merge", {
      parts,
      partsDir: partsDir.value,
      outputPath: mergeOutputPath.value,
    });
    fileToolOutput.value = JSON.stringify(data, null, 2);
    ElMessage.success("文件合并完成");
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}
</script>

<style scoped>
.split-merge-section-title {
  margin-top: 8px;
  font-weight: 600;
}

.split-merge-path-row {
  display: grid;
  grid-template-columns: 140px 1fr;
  gap: 8px;
}

.split-merge-size-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.split-merge-size-label {
  color: var(--el-text-color-secondary);
  font-size: 13px;
}
</style>

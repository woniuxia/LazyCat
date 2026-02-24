<template>
  <div class="json-process-panel">
    <div class="toolbar">
      <el-space wrap>
        <el-button type="primary" @click="formatJson">JSON 格式化</el-button>
        <el-button @click="minifyJson">JSON 压缩</el-button>
        <el-divider direction="vertical" />
        <el-button @click="callIpc('tool:convert:json-to-xml')">JSON → XML</el-button>
        <el-button @click="callIpc('tool:convert:xml-to-json')">XML → JSON</el-button>
        <el-divider direction="vertical" />
        <el-button @click="callIpc('tool:convert:json-to-yaml')">JSON → YAML</el-button>
        <el-button @click="validateYaml">YAML 校验</el-button>
        <el-button @click="formatYaml">YAML 格式化</el-button>
      </el-space>
    </div>
    <div class="editor-area">
      <el-input v-model="input" type="textarea" placeholder="输入 JSON / XML / YAML" resize="none" />
      <div class="output-wrap">
        <el-input v-model="output" type="textarea" readonly placeholder="处理结果" resize="none" />
        <el-button v-show="output" class="copy-btn" size="small" @click="copyOutput">复制</el-button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import { useClipboardSuggestion } from "../composables/useClipboardSuggestion";

const input = ref("");
const output = ref("");

function copyOutput() {
  navigator.clipboard.writeText(output.value).then(() => {
    ElMessage.success("已复制");
  });
}

function formatJson() {
  if (!input.value.trim()) return;
  try {
    const parsed = JSON.parse(input.value);
    output.value = JSON.stringify(parsed, null, 2);
  } catch (e: unknown) {
    ElMessage.error(`JSON 解析失败: ${(e as Error).message}`);
  }
}

function minifyJson() {
  if (!input.value.trim()) return;
  try {
    const parsed = JSON.parse(input.value);
    output.value = JSON.stringify(parsed);
  } catch (e: unknown) {
    ElMessage.error(`JSON 解析失败: ${(e as Error).message}`);
  }
}

async function callIpc(channel: string) {
  if (!input.value.trim()) return;
  try {
    output.value = String(
      await invokeToolByChannel(channel, { input: input.value })
    );
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function validateYaml() {
  if (!input.value.trim()) {
    ElMessage.warning("请输入 YAML 内容");
    return;
  }
  try {
    const data = (await invokeToolByChannel("tool:convert:yaml-validate", {
      input: input.value,
    })) as { valid: boolean; error: { line: number; message: string } | null };
    if (data.valid) {
      ElMessage.success("YAML 语法正确");
    } else {
      ElMessage.error(`YAML 语法错误: ${data.error?.message ?? "未知错误"}`);
    }
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function formatYaml() {
  if (!input.value.trim()) {
    ElMessage.warning("请输入 YAML 内容");
    return;
  }
  try {
    const data = (await invokeToolByChannel("tool:convert:yaml-format", {
      input: input.value,
      indent: 2,
    })) as { output: string };
    output.value = data.output;
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

const { consumePendingInput } = useClipboardSuggestion();
onMounted(() => {
  const pending = consumePendingInput("json-process");
  if (pending) input.value = pending;
});
</script>

<style scoped>
.json-process-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex: 1;
  min-height: 0;
}
.toolbar {
  flex-shrink: 0;
}
.editor-area {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
  flex: 1;
  min-height: 0;
}
.editor-area :deep(.el-textarea) {
  height: 100%;
}
.editor-area :deep(.el-textarea__inner) {
  height: 100% !important;
  min-height: 200px;
}
.output-wrap {
  position: relative;
  min-height: 0;
}
.output-wrap :deep(.el-textarea),
.output-wrap :deep(.el-textarea__inner) {
  height: 100%;
}
.output-wrap :deep(.el-textarea__inner) {
  min-height: 200px;
}
.copy-btn {
  position: absolute;
  top: 6px;
  right: 6px;
  opacity: 0.6;
  z-index: 1;
  transition: opacity 0.15s;
}
.copy-btn:hover {
  opacity: 1;
}
</style>

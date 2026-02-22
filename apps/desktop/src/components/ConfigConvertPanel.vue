<template>
  <div class="config-convert-panel">
    <div class="config-toolbar">
      <el-select v-model="fromFormat" style="width: 140px">
        <el-option v-for="f in formats" :key="f.value" :label="f.label" :value="f.value" />
      </el-select>
      <el-button text @click="swapFormats">
        &#8644;
      </el-button>
      <el-select v-model="toFormat" style="width: 140px">
        <el-option v-for="f in formats" :key="f.value" :label="f.label" :value="f.value" />
      </el-select>
      <el-button type="primary" :loading="converting" @click="convert">转换</el-button>
    </div>
    <div class="config-editors">
      <div class="editor-col">
        <div class="editor-label">输入 ({{ fromLabel }})</div>
        <el-input v-model="input" type="textarea" :rows="16" resize="vertical" placeholder="粘贴配置内容" />
      </div>
      <div class="editor-col">
        <div class="editor-label">
          输出 ({{ toLabel }})
          <el-button size="small" @click="copyOutput">复制</el-button>
        </div>
        <el-input v-model="output" type="textarea" :rows="16" resize="vertical" readonly />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

const formats = [
  { label: "Properties", value: "properties" },
  { label: "YAML", value: "yaml" },
  { label: "TOML", value: "toml" },
  { label: ".env", value: "env" },
];

const fromFormat = ref("properties");
const toFormat = ref("yaml");
const input = ref("");
const output = ref("");
const converting = ref(false);

const fromLabel = computed(() => formats.find((f) => f.value === fromFormat.value)?.label ?? "");
const toLabel = computed(() => formats.find((f) => f.value === toFormat.value)?.label ?? "");

function swapFormats() {
  const tmp = fromFormat.value;
  fromFormat.value = toFormat.value;
  toFormat.value = tmp;
  if (output.value) {
    input.value = output.value;
    output.value = "";
  }
}

async function convert() {
  if (!input.value.trim()) {
    ElMessage.warning("请输入配置内容");
    return;
  }
  converting.value = true;
  try {
    const data = (await invokeToolByChannel("tool:convert:config-convert", {
      input: input.value,
      from: fromFormat.value,
      to: toFormat.value,
    })) as { output: string };
    output.value = data.output;
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    converting.value = false;
  }
}

async function copyOutput() {
  if (!output.value) {
    ElMessage.warning("没有可复制的结果");
    return;
  }
  try {
    await navigator.clipboard.writeText(output.value);
    ElMessage.success("已复制");
  } catch {
    ElMessage.error("复制失败");
  }
}
</script>

<style scoped>
.config-convert-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.config-toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
}
.config-editors {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}
.editor-col {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.editor-label {
  font-weight: 600;
  font-size: 13px;
  display: flex;
  align-items: center;
  gap: 8px;
}
@media (max-width: 900px) {
  .config-editors {
    grid-template-columns: 1fr;
  }
}
</style>
<template>
  <div class="json-process-panel">
    <div class="toolbar">
      <el-space wrap>
        <el-button type="primary" @click="formatJson">JSON 格式化</el-button>
        <el-button @click="minifyJson">JSON 压缩</el-button>
        <el-button @click="sortJsonFields">字段排序</el-button>
        <el-divider direction="vertical" />
        <el-button @click="callIpc('tool:convert:json-to-xml')">JSON → XML</el-button>
        <el-button @click="callIpc('tool:convert:xml-to-json')">XML → JSON</el-button>
        <el-divider direction="vertical" />
        <el-button @click="callIpc('tool:convert:json-to-yaml')">JSON → YAML</el-button>
        <el-button @click="validateYaml">YAML 校验</el-button>
        <el-button @click="formatYaml">YAML 格式化</el-button>
        <el-divider direction="vertical" />
        <el-segmented
          :model-value="inputMode"
          :options="INPUT_MODE_OPTIONS"
          size="small"
          aria-label="输入模式"
          @update:model-value="setInputMode($event as InputMode)"
        />
      </el-space>
    </div>
    <div class="editor-area">
      <el-input
        v-if="inputMode === 'text'"
        v-model="input"
        type="textarea"
        placeholder="输入 JSON / XML / YAML"
        resize="none"
      />
      <JsonTreeViewer
        v-else
        class="input-tree"
        :value="inputTreeValue"
        editable
        :default-expand-depth="2"
        aria-label="JSON 树形编辑"
        @update:value="onInputTreeUpdate"
      />
      <div class="output-wrap">
        <template v-if="outputMode === 'text' || !outputTreeAvailable">
          <el-input
            v-model="output"
            type="textarea"
            readonly
            placeholder="处理结果"
            resize="none"
          />
          <el-button v-show="output" class="copy-btn" size="small" @click="copyOutput"
            >复制</el-button
          >
        </template>
        <JsonTreeViewer
          v-else
          class="output-tree"
          :value="outputTreeValue"
          :default-expand-depth="2"
          :copy-text="output"
          aria-label="处理结果树形查看"
        />
        <el-segmented
          v-if="outputTreeAvailable"
          class="output-mode-switch"
          :model-value="outputMode"
          :options="OUTPUT_MODE_OPTIONS"
          size="small"
          aria-label="输出模式"
          @update:model-value="outputMode = $event as InputMode"
        />
      </div>
    </div>
  </div>
</template>

<script lang="ts">
const jsonProcessState = { input: "", output: "" };
</script>

<script setup lang="ts">
// 数字精度边界:内容经 parse/树内编辑/序列化后,所有数字按 JS number 语义重写,
// 超过 Number.MAX_SAFE_INTEGER 的大整数会丢失精度(与既有"格式化"行为一致)。
import { computed, onBeforeUnmount, ref, shallowRef, watch } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import { useClipboardSuggestion } from "../composables/useClipboardSuggestion";
import JsonTreeViewer from "./common/JsonTreeViewer.vue";
import { stringifyJsonWithSortedKeys } from "../utils/jsonProcess";
import { canEnterJsonTree } from "../utils/jsonProcessTree";

type InputMode = "text" | "tree";

const INPUT_MODE_OPTIONS = [
  { label: "文本", value: "text" },
  { label: "树形", value: "tree" },
];
const OUTPUT_MODE_OPTIONS = INPUT_MODE_OPTIONS;

const input = ref(jsonProcessState.input);
const output = ref(jsonProcessState.output);

// 模式不持久化:面板经 <component :is> 切换即卸载,重挂载重置为文本属预期
const inputMode = ref<InputMode>("text");
const outputMode = ref<InputMode>("text");
const inputTreeValue = shallowRef<unknown>(null);
// 最近一次树编辑回写的 input 文本;不一致的 input 变化视为外部写入(换文档)
let lastWrittenBack: string | null = null;

onBeforeUnmount(() => {
  jsonProcessState.input = input.value;
  jsonProcessState.output = output.value;
});

function setInputMode(mode: InputMode) {
  if (mode === inputMode.value) return;
  if (mode === "tree") {
    const gate = canEnterJsonTree(input.value);
    if (!gate.ok) {
      ElMessage.error(gate.reason);
      return;
    }
    inputTreeValue.value = gate.value;
    lastWrittenBack = input.value;
  }
  inputMode.value = mode;
}

/** 树内编辑即时序列化回写 input:文本任何时刻都是事实源,卸载不丢编辑。 */
function onInputTreeUpdate(value: unknown) {
  inputTreeValue.value = value;
  const text = JSON.stringify(value, null, 2);
  lastWrittenBack = text;
  input.value = text;
}

watch(input, (text) => {
  if (inputMode.value !== "tree") return;
  if (text === lastWrittenBack) return;
  // 外部写入(如剪贴板建议注入):按换文档重过闸门
  const gate = canEnterJsonTree(text);
  if (gate.ok) {
    inputTreeValue.value = gate.value;
    lastWrittenBack = text;
  } else {
    inputMode.value = "text";
    ElMessage.warning(`已切回文本模式:${gate.reason}`);
  }
});

/** 文本类操作执行前切回文本模式(input 已因即时回写保持最新)。 */
function ensureTextMode() {
  if (inputMode.value === "tree") inputMode.value = "text";
}

const outputTreeGate = computed(() => (output.value ? canEnterJsonTree(output.value) : null));
const outputTreeAvailable = computed(() => outputTreeGate.value?.ok === true);
const outputTreeValue = computed(() =>
  outputTreeGate.value?.ok ? outputTreeGate.value.value : null,
);

// 新输出重置为文本展示,与"默认文本"保持一致
watch(output, () => {
  outputMode.value = "text";
});

function copyOutput() {
  navigator.clipboard.writeText(output.value).then(() => {
    ElMessage.success("已复制");
  });
}

function formatJson() {
  ensureTextMode();
  if (!input.value.trim()) return;
  try {
    const parsed = JSON.parse(input.value);
    output.value = JSON.stringify(parsed, null, 2);
  } catch (e: unknown) {
    ElMessage.error(`JSON 解析失败: ${(e as Error).message}`);
  }
}

function minifyJson() {
  ensureTextMode();
  if (!input.value.trim()) return;
  try {
    const parsed = JSON.parse(input.value);
    output.value = JSON.stringify(parsed);
  } catch (e: unknown) {
    ElMessage.error(`JSON 解析失败: ${(e as Error).message}`);
  }
}

function sortJsonFields() {
  ensureTextMode();
  if (!input.value.trim()) return;
  try {
    const parsed = JSON.parse(input.value);
    output.value = stringifyJsonWithSortedKeys(parsed);
  } catch (e: unknown) {
    ElMessage.error(`JSON 解析失败: ${(e as Error).message}`);
  }
}

async function callIpc(channel: string) {
  ensureTextMode();
  if (!input.value.trim()) return;
  try {
    output.value = String(await invokeToolByChannel(channel, { input: input.value }));
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function validateYaml() {
  ensureTextMode();
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
  ensureTextMode();
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

const { watchPendingInput } = useClipboardSuggestion();
watchPendingInput("json-process", (text) => {
  input.value = text;
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
.input-tree,
.output-tree {
  min-height: 200px;
}
.output-wrap {
  position: relative;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.output-wrap :deep(.el-textarea),
.output-wrap :deep(.el-textarea__inner) {
  height: 100%;
}
.output-wrap :deep(.el-textarea__inner) {
  min-height: 200px;
}
.output-wrap .output-tree {
  flex: 1 1 auto;
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
.output-mode-switch {
  position: absolute;
  right: 6px;
  bottom: 6px;
  z-index: 1;
  opacity: 0.85;
}
</style>

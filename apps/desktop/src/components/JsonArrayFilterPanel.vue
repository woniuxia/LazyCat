<template>
  <div class="json-array-filter-panel">
    <div class="filter-toolbar">
      <div class="filter-status" aria-live="polite">
        <strong>数组过滤</strong>
        <span v-if="status === 'loading'" class="status-hint">正在解析...</span>
        <span v-else-if="selectedPath" class="status-hint">
          首个可用数组路径：<code>{{ selectedPath }}</code>
        </span>
        <span v-if="target" class="status-hint">
          已选 {{ selectedProperties.length }} / {{ propertyCandidates.length }} 个属性
        </span>
      </div>
      <div class="filter-actions">
        <el-button text data-action="clear-input" :disabled="!input" @click="clearInput"
          >清空</el-button
        >
        <el-button type="primary" data-action="copy-result" :disabled="!output" @click="copyResult"
          >复制结果</el-button
        >
      </div>
    </div>

    <el-alert
      v-if="parseError"
      class="parse-alert"
      type="error"
      :title="parseError"
      :closable="false"
      show-icon
      role="alert"
    />

    <div class="filter-editors">
      <section class="editor-column" aria-labelledby="array-filter-input-title">
        <header class="editor-header">
          <div>
            <strong id="array-filter-input-title">JSON 输入文档</strong>
            <span>停止编辑后自动解析</span>
          </div>
        </header>
        <el-input
          v-model="input"
          class="json-editor"
          type="textarea"
          resize="none"
          placeholder="输入包含对象数组的 JSON"
          aria-label="JSON 输入文档"
        />
      </section>

      <section class="editor-column" aria-labelledby="array-filter-output-title">
        <header class="editor-header">
          <div>
            <strong id="array-filter-output-title">数组过滤结果</strong>
            <span>只读根数组</span>
          </div>
        </header>
        <el-input
          v-model="output"
          class="json-editor"
          type="textarea"
          resize="none"
          readonly
          placeholder="数组过滤结果"
          aria-label="数组过滤结果"
        />
      </section>
    </div>

    <section v-if="target && propertyCandidates.length" class="property-section">
      <div class="property-header">
        <div>
          <strong>属性候选</strong>
          <span>默认全选，取消后即时更新结果</span>
        </div>
        <span class="property-count"
          >{{ selectedProperties.length }} / {{ propertyCandidates.length }}</span
        >
      </div>
      <div class="property-options" role="group" aria-label="属性候选">
        <el-checkbox
          v-for="property in propertyCandidates"
          :key="property"
          :model-value="selectedProperties.includes(property)"
          :label="property"
          @change="setPropertySelected(property, $event)"
        >
          {{ property }}
        </el-checkbox>
      </div>
    </section>

    <div v-if="status === 'idle'" class="state-panel">
      <el-empty description="输入 JSON 文档后自动解析" />
    </div>
    <div v-else-if="status === 'empty'" class="state-panel">
      <el-empty description="未找到可过滤的对象数组" />
    </div>
    <div v-else-if="target && !propertyCandidates.length" class="state-panel compact-state">
      <el-empty description="对象数组没有属性候选" />
    </div>
  </div>
</template>

<script lang="ts">
import type { JsonObject } from "../utils/jsonArrayFilter";

type FilterStatus = "idle" | "loading" | "ready" | "empty" | "error";

const jsonArrayFilterState = {
  input: "",
  selectedPath: "",
  target: null as JsonObject[] | null,
  propertyCandidates: [] as string[],
  selectedProperties: [] as string[],
  output: "",
  parseError: "",
  status: "idle" as FilterStatus,
};
</script>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import {
  collectArrayProperties,
  findFirstObjectArray,
  projectObjectArray,
  type JsonObject,
} from "../utils/jsonArrayFilter";

const input = ref(jsonArrayFilterState.input);
const selectedPath = ref(jsonArrayFilterState.selectedPath);
const target = ref<JsonObject[] | null>(jsonArrayFilterState.target);
const propertyCandidates = ref([...jsonArrayFilterState.propertyCandidates]);
const selectedProperties = ref([...jsonArrayFilterState.selectedProperties]);
const output = ref(jsonArrayFilterState.output);
const parseError = ref(jsonArrayFilterState.parseError);
const status = ref<"idle" | "loading" | "ready" | "empty" | "error">(jsonArrayFilterState.status);
const parseTimer = ref<ReturnType<typeof setTimeout> | null>(null);

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function clearDerived(nextStatus: "idle" | "loading" | "empty") {
  selectedPath.value = "";
  target.value = null;
  propertyCandidates.value = [];
  selectedProperties.value = [];
  output.value = "";
  parseError.value = "";
  status.value = nextStatus;
}

function refreshOutput() {
  if (!target.value) {
    output.value = "";
    return;
  }

  try {
    output.value = JSON.stringify(
      projectObjectArray(target.value, new Set(selectedProperties.value)),
      null,
      2,
    );
  } catch (error) {
    output.value = "";
    parseError.value = `数组过滤结果序列化失败：${messageOf(error)}`;
    status.value = "ready";
  }
}

function parseDocument(text: string) {
  if (!text.trim()) {
    clearDerived("idle");
    return;
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch (error) {
    clearDerived("idle");
    parseError.value = `JSON 解析失败：${messageOf(error)}`;
    status.value = "error";
    return;
  }

  const found = findFirstObjectArray(parsed);
  if (!found) {
    clearDerived("empty");
    return;
  }

  selectedPath.value = found.path;
  target.value = found.value;
  propertyCandidates.value = collectArrayProperties(found.value);
  selectedProperties.value = [...propertyCandidates.value];
  parseError.value = "";
  status.value = "ready";
  refreshOutput();
}

function scheduleParse(text: string) {
  if (parseTimer.value) clearTimeout(parseTimer.value);
  parseTimer.value = null;

  if (!text.trim()) {
    parseDocument(text);
    return;
  }

  parseTimer.value = setTimeout(() => {
    parseTimer.value = null;
    parseDocument(text);
  }, 300);
}

function setPropertySelected(property: string, checked: unknown) {
  const next = new Set(selectedProperties.value);
  if (checked === true) next.add(property);
  else next.delete(property);
  selectedProperties.value = propertyCandidates.value.filter((candidate) => next.has(candidate));
}

function clearInput() {
  if (parseTimer.value) clearTimeout(parseTimer.value);
  parseTimer.value = null;
  input.value = "";
  clearDerived("idle");
}

async function copyResult() {
  if (!output.value) {
    ElMessage.warning("没有可复制的数组过滤结果");
    return;
  }

  try {
    await navigator.clipboard.writeText(output.value);
    ElMessage.success("数组过滤结果已复制");
  } catch (error) {
    ElMessage.error(`复制数组过滤结果失败：${messageOf(error)}`);
  }
}

watch(input, (text) => {
  clearDerived(text.trim() ? "loading" : "idle");
  scheduleParse(text);
});

watch(selectedProperties, refreshOutput, { deep: true });

onMounted(() => {
  if (input.value.trim() && !parseError.value && (!target.value || status.value === "loading")) {
    scheduleParse(input.value);
  }
});

onBeforeUnmount(() => {
  if (parseTimer.value) clearTimeout(parseTimer.value);
  jsonArrayFilterState.input = input.value;
  jsonArrayFilterState.selectedPath = selectedPath.value;
  jsonArrayFilterState.target = target.value;
  jsonArrayFilterState.propertyCandidates = [...propertyCandidates.value];
  jsonArrayFilterState.selectedProperties = [...selectedProperties.value];
  jsonArrayFilterState.output = output.value;
  jsonArrayFilterState.parseError = parseError.value;
  jsonArrayFilterState.status = status.value;
});
</script>

<style scoped>
.json-array-filter-panel {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 12px;
  min-height: 0;
  container-name: json-array-filter;
  container-type: inline-size;
}

.filter-toolbar,
.property-header,
.editor-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.filter-toolbar {
  flex-shrink: 0;
}

.filter-status,
.filter-actions,
.property-header > div,
.editor-header > div {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.status-hint,
.editor-header span,
.property-header span {
  color: var(--lc-text-muted);
  font-size: 12px;
}

.status-hint code {
  color: var(--el-color-primary);
  font-family: var(--lc-font-mono, "Cascadia Code", "JetBrains Mono", monospace);
}

.parse-alert {
  flex-shrink: 0;
}

.filter-editors {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  flex: 1;
  gap: 12px;
  min-height: 280px;
}

.editor-column {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
  min-height: 0;
}

.editor-header {
  flex-shrink: 0;
  justify-content: space-between;
  min-height: 32px;
}

.editor-header strong,
.property-header strong {
  font-size: 13px;
}

.editor-column :deep(.json-editor),
.editor-column :deep(.el-textarea),
.editor-column :deep(.el-textarea__inner) {
  height: 100%;
}

.editor-column :deep(.el-textarea__inner) {
  min-height: 220px;
}

.property-section {
  flex-shrink: 0;
  padding: 10px 12px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  background: var(--el-fill-color-blank);
}

.property-count {
  white-space: nowrap;
}

.property-options {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 16px;
  margin-top: 10px;
}

.state-panel {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 104px;
  border: 1px dashed var(--el-border-color);
  border-radius: 6px;
  background: var(--el-fill-color-lighter);
}

.compact-state {
  min-height: 72px;
}

@container json-array-filter (max-width: 900px) {
  .filter-toolbar,
  .property-header,
  .editor-header {
    align-items: flex-start;
    flex-direction: column;
  }

  .filter-actions {
    width: 100%;
  }

  .filter-editors {
    grid-template-columns: 1fr;
    grid-template-rows: repeat(2, minmax(220px, 1fr));
    overflow: auto;
  }
}
</style>

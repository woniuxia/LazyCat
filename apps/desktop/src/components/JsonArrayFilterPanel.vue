<template>
  <div class="json-array-filter-panel">
    <div class="filter-toolbar">
      <div class="filter-status" role="status" aria-live="polite" aria-atomic="true">
        <div class="filter-title-row">
          <strong>数组过滤</strong>
          <span class="status-badge" :class="statusClass">{{ statusLabel }}</span>
        </div>
        <div class="filter-meta">
          <span v-if="selectedPath" class="status-hint">
            数组路径：<code>{{ selectedPath }}</code>
          </span>
          <span v-if="target" class="status-hint">{{ target.length }} 条记录</span>
          <span v-if="target" class="status-hint">
            已选 {{ selectedProperties.length }} / {{ propertyCandidates.length }} 个字段
          </span>
        </div>
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
            <span>{{ target ? `${target.length} 条记录` : "等待解析" }}</span>
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
        <div class="property-title">
          <div class="property-title-row">
            <strong>输出字段</strong>
            <span class="property-count"
              >{{ selectedProperties.length }} / {{ propertyCandidates.length }}</span
            >
          </div>
          <span>取消字段后即时更新结果</span>
        </div>
        <div class="property-actions" role="toolbar" aria-label="字段选择操作">
          <el-button
            text
            size="small"
            data-action="toggle-all-properties"
            @click="toggleAllProperties"
          >
            {{ allPropertiesSelected ? "取消全选" : "全选" }}
          </el-button>
          <el-button
            text
            size="small"
            data-action="clear-properties"
            :disabled="!selectedProperties.length"
            @click="clearProperties"
          >
            清空选择
          </el-button>
        </div>
      </div>
      <div v-if="propertyCandidates.length > 5" class="property-tools">
        <el-input
          v-model="propertySearch"
          class="property-search"
          clearable
          placeholder="搜索字段名"
          aria-label="搜索输出字段"
        />
        <span v-if="propertySearch.trim()" class="property-filter-count">
          匹配 {{ filteredProperties.length }} 个
        </span>
      </div>
      <div v-if="filteredProperties.length" class="property-options" role="group" aria-label="输出字段">
        <el-checkbox
          v-for="property in filteredProperties"
          :key="property"
          :model-value="selectedProperties.includes(property)"
          :label="property"
          @change="setPropertySelected(property, $event)"
        >
          {{ property }}
        </el-checkbox>
      </div>
      <div v-else class="property-empty" role="status">没有匹配字段</div>
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
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
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
const propertySearch = ref("");
const output = ref(jsonArrayFilterState.output);
const parseError = ref(jsonArrayFilterState.parseError);
const status = ref<"idle" | "loading" | "ready" | "empty" | "error">(jsonArrayFilterState.status);
const parseTimer = ref<ReturnType<typeof setTimeout> | null>(null);

const filteredProperties = computed(() => {
  const query = propertySearch.value.trim().toLocaleLowerCase();
  if (!query) return propertyCandidates.value;
  return propertyCandidates.value.filter((property) =>
    property.toLocaleLowerCase().includes(query),
  );
});

const allPropertiesSelected = computed(
  () =>
    propertyCandidates.value.length > 0 &&
    propertyCandidates.value.every((property) => selectedProperties.value.includes(property)),
);

const statusLabel = computed(() => {
  switch (status.value) {
    case "loading":
      return "解析中";
    case "ready":
      return "已生成";
    case "empty":
      return "未找到数组";
    case "error":
      return "解析失败";
    default:
      return "等待输入";
  }
});

const statusClass = computed(() => `is-${status.value}`);

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function clearDerived(nextStatus: "idle" | "loading" | "empty") {
  selectedPath.value = "";
  target.value = null;
  propertyCandidates.value = [];
  selectedProperties.value = [];
  propertySearch.value = "";
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
  propertySearch.value = "";
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

function toggleAllProperties() {
  selectedProperties.value = allPropertiesSelected.value ? [] : [...propertyCandidates.value];
}

function clearProperties() {
  selectedProperties.value = [];
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
  overflow: auto;
  container-name: json-array-filter;
  container-type: inline-size;
}

.filter-toolbar,
.property-header,
.editor-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}

.filter-toolbar {
  flex-shrink: 0;
}

.filter-status,
.filter-actions,
.filter-title-row,
.filter-meta,
.property-header > div,
.property-title,
.property-title-row,
.property-actions,
.editor-header > div {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.filter-status {
  flex: 1;
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
}

.filter-title-row,
.property-title-row {
  flex-wrap: wrap;
}

.filter-meta {
  flex-wrap: wrap;
  row-gap: 4px;
}

.filter-actions,
.property-actions {
  flex-shrink: 0;
}

.property-title {
  flex-direction: column;
  align-items: flex-start !important;
  gap: 4px !important;
}

.status-badge {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 1px 8px;
  border: 1px solid var(--lc-border);
  border-radius: 999px;
  color: var(--lc-text-muted);
  background: var(--lc-surface-2);
  font-size: 12px;
  line-height: 18px;
}

.status-badge.is-loading {
  color: var(--lc-accent);
  border-color: var(--lc-border-active);
  background: var(--lc-accent-dim);
}

.status-badge.is-ready {
  color: var(--lc-success);
  border-color: color-mix(in srgb, var(--lc-success) 35%, var(--lc-border));
  background: color-mix(in srgb, var(--lc-success) 10%, var(--lc-surface-0));
}

.status-badge.is-error {
  color: var(--lc-danger);
  border-color: color-mix(in srgb, var(--lc-danger) 35%, var(--lc-border));
  background: color-mix(in srgb, var(--lc-danger) 8%, var(--lc-surface-0));
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
  flex: 1 1 auto;
  gap: 12px;
  min-height: 240px;
  overflow: hidden;
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
  min-height: 0;
}

.editor-column :deep(.el-textarea__inner) {
  min-height: 220px;
}

.property-section {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  max-height: 240px;
  padding: 10px 12px;
  overflow: hidden;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  background: var(--el-fill-color-blank);
}

.property-count {
  white-space: nowrap;
}

.property-tools {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  margin-top: 10px;
}

.property-search {
  width: min(100%, 280px);
}

.property-filter-count {
  flex-shrink: 0;
  color: var(--lc-text-muted);
  font-size: 12px;
}

.property-options {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: 6px 12px;
  max-height: 132px;
  overflow: auto;
  margin-top: 10px;
}

.property-options :deep(.el-checkbox) {
  min-width: 0;
  margin-right: 0;
}

.property-options :deep(.el-checkbox__label) {
  overflow-wrap: anywhere;
  white-space: normal;
}

.property-empty {
  padding: 12px 0 2px;
  color: var(--lc-text-muted);
  font-size: 12px;
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
    justify-content: flex-end;
  }

  .filter-editors {
    flex: 0 0 auto;
    grid-template-columns: 1fr;
    grid-template-rows: repeat(2, minmax(220px, 1fr));
    overflow: visible;
  }

  .property-section {
    max-height: 220px;
  }

  .property-header,
  .property-tools {
    width: 100%;
  }

  .property-actions {
    width: 100%;
    justify-content: flex-end;
  }

  .property-search {
    flex: 1;
  }
}
</style>

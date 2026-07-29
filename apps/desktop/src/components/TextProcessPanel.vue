<template>
  <div class="text-process-panel">
    <div class="panel-toolbar">
      <el-select
        v-model="selectedPresetId"
        class="preset-select"
        placeholder="选择预设"
        clearable
        @change="onPresetChange"
      >
        <el-option
          v-for="preset in presets"
          :key="preset.id"
          :label="`${preset.name} - ${preset.description}`"
          :value="preset.id"
        />
      </el-select>
      <el-select v-model="lineEnding" class="line-ending-select">
        <el-option label="保持原始换行" value="keep" />
        <el-option label="LF (\\n)" value="lf" />
        <el-option label="CRLF (\\r\\n)" value="crlf" />
      </el-select>
      <el-switch v-model="autoRun" active-text="自动执行" />
      <el-button type="primary" :loading="processing" @click="runProcess">执行处理</el-button>
      <el-button @click="swapInputOutput">结果覆盖输入</el-button>
      <el-button @click="copyOutput">复制结果</el-button>
      <el-button @click="clearAll">清空</el-button>
    </div>

    <div class="text-grid">
      <div class="textarea-card">
        <div class="card-head">
          <span>原始文本</span>
          <span class="meta">{{ inputLines }} 行 / {{ textInput.length }} 字符</span>
        </div>
        <el-input
          v-model="textInput"
          type="textarea"
          resize="none"
          placeholder="输入日志、配置或多行文本"
        />
      </div>

      <div class="textarea-card">
        <div class="card-head">
          <span>处理结果</span>
          <span class="meta">{{ outputLines }} 行 / {{ textOutput.length }} 字符</span>
        </div>
        <el-input
          v-model="textOutput"
          type="textarea"
          resize="none"
          readonly
          placeholder="处理结果"
        />
      </div>
    </div>

    <el-tabs v-model="activeTab" class="process-tabs">
      <el-tab-pane label="操作选项" name="ops">
        <div class="operation-grid">
          <section class="operation-card">
            <div class="op-title">基础清洗</div>
            <el-space wrap>
              <el-checkbox v-model="ops.trim">行首尾去空白</el-checkbox>
              <el-checkbox v-model="ops.removeEmpty">移除空行</el-checkbox>
              <el-checkbox v-model="ops.dedupe">按行去重</el-checkbox>
              <el-checkbox v-model="ops.sort">按行排序</el-checkbox>
            </el-space>
            <div class="inline-options">
              <el-checkbox v-model="ops.caseSensitive">区分大小写</el-checkbox>
              <el-select v-model="ops.sortOrder" style="width: 120px" :disabled="!ops.sort">
                <el-option label="升序" value="asc" />
                <el-option label="降序" value="desc" />
              </el-select>
            </div>
          </section>

          <section class="operation-card">
            <div class="op-title">过滤与替换</div>
            <div class="op-row">
              <el-checkbox v-model="ops.includeFilter">仅保留匹配行</el-checkbox>
              <el-select v-model="ops.includeMode" style="width: 120px" :disabled="!ops.includeFilter">
                <el-option label="包含" value="contains" />
                <el-option label="全等" value="equals" />
                <el-option label="正则" value="regex" />
              </el-select>
              <el-input v-model="ops.includePattern" :disabled="!ops.includeFilter" placeholder="匹配规则" />
            </div>
            <div class="op-row">
              <el-checkbox v-model="ops.excludeFilter">排除匹配行</el-checkbox>
              <el-select v-model="ops.excludeMode" style="width: 120px" :disabled="!ops.excludeFilter">
                <el-option label="包含" value="contains" />
                <el-option label="全等" value="equals" />
                <el-option label="正则" value="regex" />
              </el-select>
              <el-input v-model="ops.excludePattern" :disabled="!ops.excludeFilter" placeholder="匹配规则" />
            </div>
            <div class="op-row">
              <el-checkbox v-model="ops.replace">替换</el-checkbox>
              <el-select v-model="ops.replaceMode" style="width: 120px" :disabled="!ops.replace">
                <el-option label="文本" value="contains" />
                <el-option label="正则" value="regex" />
              </el-select>
              <el-input v-model="ops.replacePattern" :disabled="!ops.replace" placeholder="待替换内容" />
              <el-input v-model="ops.replaceWith" :disabled="!ops.replace" placeholder="替换为" />
            </div>
          </section>

          <section class="operation-card">
            <div class="op-title">提取与拼接</div>
            <div class="op-row">
              <el-checkbox v-model="ops.extractColumn">提取列</el-checkbox>
              <el-input v-model="ops.delimiter" :disabled="!ops.extractColumn" placeholder="分隔符，如 =" />
              <el-input-number
                v-model="ops.columnIndex"
                :disabled="!ops.extractColumn"
                :min="1"
                :max="50"
                controls-position="right"
              />
              <el-checkbox v-model="ops.keepUnmatched" :disabled="!ops.extractColumn">保留未命中行</el-checkbox>
            </div>
            <div class="op-row">
              <el-checkbox v-model="ops.addPrefix">添加前缀</el-checkbox>
              <el-input v-model="ops.prefixValue" :disabled="!ops.addPrefix" placeholder="前缀内容" />
            </div>
            <div class="op-row">
              <el-checkbox v-model="ops.addSuffix">添加后缀</el-checkbox>
              <el-input v-model="ops.suffixValue" :disabled="!ops.addSuffix" placeholder="后缀内容" />
            </div>
          </section>
        </div>
      </el-tab-pane>

      <el-tab-pane label="统计信息" name="stats">
        <div class="summary-grid">
          <div class="summary-item">
            <div class="summary-label">输入行数</div>
            <div class="summary-value">{{ stats.inputLines }}</div>
          </div>
          <div class="summary-item">
            <div class="summary-label">输出行数</div>
            <div class="summary-value">{{ stats.outputLines }}</div>
          </div>
          <div class="summary-item">
            <div class="summary-label">变更行数</div>
            <div class="summary-value accent">{{ stats.changedLines }}</div>
          </div>
          <div class="summary-item">
            <div class="summary-label">耗时</div>
            <div class="summary-value">{{ stats.durationMs }} ms</div>
          </div>
          <div class="summary-item">
            <div class="summary-label">字符数(含空格)</div>
            <div class="summary-value">{{ stats.charsWithSpaces }}</div>
          </div>
          <div class="summary-item">
            <div class="summary-label">字符数(不含空格)</div>
            <div class="summary-value">{{ stats.charsNoSpaces }}</div>
          </div>
          <div class="summary-item">
            <div class="summary-label">中文字数</div>
            <div class="summary-value">{{ stats.chineseChars }}</div>
          </div>
          <div class="summary-item">
            <div class="summary-label">英文单词</div>
            <div class="summary-value">{{ stats.englishWords }}</div>
          </div>
          <div class="summary-item">
            <div class="summary-label">UTF-8 字节</div>
            <div class="summary-value">{{ stats.bytesUtf8 }}</div>
          </div>
          <div class="summary-item">
            <div class="summary-label">最长行</div>
            <div class="summary-value">{{ stats.longestLine }}</div>
          </div>
        </div>
      </el-tab-pane>

      <el-tab-pane label="变更预览" name="preview">
        <el-alert
          v-if="warnings.length > 0"
          type="warning"
          :closable="false"
          show-icon
          :title="warnings[0]"
          style="margin-bottom: 8px"
        />
        <div class="preview-card">
          <div class="card-head">
            <span>变更样本 ({{ preview.changed }})</span>
            <span class="meta">最多展示 {{ previewLimit }} 条</span>
          </div>
          <el-table :data="preview.samples" border stripe size="small" max-height="240">
            <el-table-column prop="line" label="行号" width="80" align="center" />
            <el-table-column prop="before" label="处理前" min-width="260" show-overflow-tooltip />
            <el-table-column prop="after" label="处理后" min-width="260" show-overflow-tooltip />
          </el-table>
        </div>
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<script lang="ts">
const textProcessState = {
  input: "",
  output: "",
  selectedPresetId: "",
  lineEnding: "keep",
  autoRun: true,
  ops: {
    trim: true,
    removeEmpty: true,
    dedupe: false,
    sort: false,
    caseSensitive: false,
    sortOrder: "asc",
    includeFilter: false,
    includeMode: "contains",
    includePattern: "",
    excludeFilter: false,
    excludeMode: "contains",
    excludePattern: "",
    replace: false,
    replaceMode: "contains",
    replacePattern: "",
    replaceWith: "",
    extractColumn: false,
    delimiter: "=",
    columnIndex: 1,
    keepUnmatched: false,
    addPrefix: false,
    prefixValue: "",
    addSuffix: false,
    suffixValue: "",
  },
};
</script>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type { TextLineEnding, TextMatchMode, TextOperation, TextPreset, TextProcessResponse } from "../types";

const previewLimit = 200;
const processing = ref(false);
const autoRun = ref(true);
const activeTab = ref("ops");
const lineEnding = ref<TextLineEnding>("keep");

const textInput = ref("");
const textOutput = ref("");
const selectedPresetId = ref("");
const presets = ref<TextPreset[]>([]);
const warnings = ref<string[]>([]);

const stats = reactive({
  inputLines: 0,
  outputLines: 0,
  changedLines: 0,
  inputChars: 0,
  outputChars: 0,
  durationMs: 0,
  charsWithSpaces: 0,
  charsNoSpaces: 0,
  chineseChars: 0,
  englishWords: 0,
  bytesUtf8: 0,
  longestLine: 0,
});

const preview = reactive({
  changed: 0,
  samples: [] as Array<{ before: string; after: string; line: number }>,
});

const ops = reactive({
  trim: true,
  removeEmpty: true,
  dedupe: false,
  sort: false,
  caseSensitive: false,
  sortOrder: "asc" as "asc" | "desc",
  includeFilter: false,
  includeMode: "contains" as TextMatchMode,
  includePattern: "",
  excludeFilter: false,
  excludeMode: "contains" as TextMatchMode,
  excludePattern: "",
  replace: false,
  replaceMode: "contains" as TextMatchMode,
  replacePattern: "",
  replaceWith: "",
  extractColumn: false,
  delimiter: "=",
  columnIndex: 1,
  keepUnmatched: false,
  addPrefix: false,
  prefixValue: "",
  addSuffix: false,
  suffixValue: "",
});

const inputLines = computed(() => countLines(textInput.value));
const outputLines = computed(() => countLines(textOutput.value));

const operationSignature = computed(() => JSON.stringify(buildOperations()));

function countLines(text: string): number {
  if (!text) return 0;
  return text.replaceAll("\r\n", "\n").replaceAll("\r", "\n").split("\n").length;
}

function buildOperations(): TextOperation[] {
  return [
    { type: "trim", enabled: ops.trim },
    { type: "remove_empty", enabled: ops.removeEmpty },
    { type: "dedupe", enabled: ops.dedupe, caseSensitive: ops.caseSensitive },
    {
      type: "sort",
      enabled: ops.sort,
      caseSensitive: ops.caseSensitive,
      sortOrder: ops.sortOrder,
    },
    {
      type: "include_filter",
      enabled: ops.includeFilter,
      matchMode: ops.includeMode,
      pattern: ops.includePattern,
      caseSensitive: ops.caseSensitive,
    },
    {
      type: "exclude_filter",
      enabled: ops.excludeFilter,
      matchMode: ops.excludeMode,
      pattern: ops.excludePattern,
      caseSensitive: ops.caseSensitive,
    },
    {
      type: "replace",
      enabled: ops.replace,
      matchMode: ops.replaceMode,
      pattern: ops.replacePattern,
      replacement: ops.replaceWith,
      caseSensitive: ops.caseSensitive,
    },
    { type: "add_prefix", enabled: ops.addPrefix, pattern: ops.prefixValue },
    { type: "add_suffix", enabled: ops.addSuffix, pattern: ops.suffixValue },
    {
      type: "extract_column",
      enabled: ops.extractColumn,
      delimiter: ops.delimiter,
      columnIndex: ops.columnIndex,
      keepUnmatched: ops.keepUnmatched,
    },
  ];
}

function assignStats(next: TextProcessResponse["stats"]) {
  stats.inputLines = next.inputLines;
  stats.outputLines = next.outputLines;
  stats.changedLines = next.changedLines;
  stats.inputChars = next.inputChars;
  stats.outputChars = next.outputChars;
  stats.durationMs = next.durationMs;
  stats.charsWithSpaces = next.charsWithSpaces ?? 0;
  stats.charsNoSpaces = next.charsNoSpaces ?? 0;
  stats.chineseChars = next.chineseChars ?? 0;
  stats.englishWords = next.englishWords ?? 0;
  stats.bytesUtf8 = next.bytesUtf8 ?? 0;
  stats.longestLine = next.longestLine ?? 0;
}

function assignPreview(next: TextProcessResponse["preview"]) {
  preview.changed = next.changed;
  preview.samples = next.samples;
}

async function runProcess() {
  if (processing.value) return;
  processing.value = true;
  try {
    const data = (await invokeToolByChannel("tool:text:process", {
      input: textInput.value,
      lineEnding: lineEnding.value,
      operations: buildOperations(),
      previewLimit,
    })) as TextProcessResponse;

    textOutput.value = data.output ?? "";
    assignStats(data.stats);
    assignPreview(data.preview);
    warnings.value = data.warnings || [];
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    processing.value = false;
  }
}

function resetOperationState() {
  ops.trim = true;
  ops.removeEmpty = true;
  ops.dedupe = false;
  ops.sort = false;
  ops.caseSensitive = false;
  ops.sortOrder = "asc";
  ops.includeFilter = false;
  ops.includeMode = "contains";
  ops.includePattern = "";
  ops.excludeFilter = false;
  ops.excludeMode = "contains";
  ops.excludePattern = "";
  ops.replace = false;
  ops.replaceMode = "contains";
  ops.replacePattern = "";
  ops.replaceWith = "";
  ops.extractColumn = false;
  ops.delimiter = "=";
  ops.columnIndex = 1;
  ops.keepUnmatched = false;
  ops.addPrefix = false;
  ops.prefixValue = "";
  ops.addSuffix = false;
  ops.suffixValue = "";
}

function applyPreset(preset: TextPreset) {
  resetOperationState();
  for (const item of preset.operations || []) {
    switch (item.type) {
      case "trim":
        ops.trim = item.enabled;
        break;
      case "remove_empty":
        ops.removeEmpty = item.enabled;
        break;
      case "dedupe":
        ops.dedupe = item.enabled;
        ops.caseSensitive = item.caseSensitive ?? ops.caseSensitive;
        break;
      case "sort":
        ops.sort = item.enabled;
        ops.caseSensitive = item.caseSensitive ?? ops.caseSensitive;
        ops.sortOrder = item.sortOrder ?? "asc";
        break;
      case "include_filter":
        ops.includeFilter = item.enabled;
        ops.includeMode = item.matchMode ?? "contains";
        ops.includePattern = item.pattern ?? "";
        ops.caseSensitive = item.caseSensitive ?? ops.caseSensitive;
        break;
      case "exclude_filter":
        ops.excludeFilter = item.enabled;
        ops.excludeMode = item.matchMode ?? "contains";
        ops.excludePattern = item.pattern ?? "";
        ops.caseSensitive = item.caseSensitive ?? ops.caseSensitive;
        break;
      case "replace":
        ops.replace = item.enabled;
        ops.replaceMode = item.matchMode ?? "contains";
        ops.replacePattern = item.pattern ?? "";
        ops.replaceWith = item.replacement ?? "";
        ops.caseSensitive = item.caseSensitive ?? ops.caseSensitive;
        break;
      case "extract_column":
        ops.extractColumn = item.enabled;
        ops.delimiter = item.delimiter ?? "=";
        ops.columnIndex = item.columnIndex ?? 1;
        ops.keepUnmatched = item.keepUnmatched ?? false;
        break;
      case "add_prefix":
        ops.addPrefix = item.enabled;
        ops.prefixValue = item.pattern ?? "";
        break;
      case "add_suffix":
        ops.addSuffix = item.enabled;
        ops.suffixValue = item.pattern ?? "";
        break;
    }
  }
}

function onPresetChange(id: string) {
  if (!id) return;
  const preset = presets.value.find((item) => item.id === id);
  if (!preset) return;
  applyPreset(preset);
  void runProcess();
}

function clearAll() {
  textInput.value = "";
  textOutput.value = "";
  warnings.value = [];
  resetOperationState();
  selectedPresetId.value = "";
  assignStats({
    inputLines: 0,
    outputLines: 0,
    changedLines: 0,
    inputChars: 0,
    outputChars: 0,
    durationMs: 0,
    charsWithSpaces: 0,
    charsNoSpaces: 0,
    chineseChars: 0,
    englishWords: 0,
    bytesUtf8: 0,
    longestLine: 0,
  });
  assignPreview({ changed: 0, samples: [] });
}

function swapInputOutput() {
  textInput.value = textOutput.value;
}

async function copyOutput() {
  if (!textOutput.value) {
    ElMessage.warning("没有可复制的结果");
    return;
  }
  try {
    await navigator.clipboard.writeText(textOutput.value);
    ElMessage.success("已复制结果");
  } catch {
    ElMessage.error("复制失败");
  }
}

async function loadPresets() {
  try {
    const data = await invokeToolByChannel("tool:text:presets", {});
    presets.value = Array.isArray(data) ? (data as TextPreset[]) : [];
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

let timer: ReturnType<typeof setTimeout> | null = null;
watch([textInput, lineEnding, operationSignature, autoRun], () => {
  if (!autoRun.value) return;
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    if (!textInput.value) {
      textOutput.value = "";
      warnings.value = [];
      return;
    }
    void runProcess();
  }, 300);
});

onMounted(() => {
  void loadPresets();
});
</script>

<style scoped>
.text-process-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex: 1;
  min-height: 0;
}

.panel-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  align-items: center;
  flex-shrink: 0;
}

.preset-select {
  width: 320px;
}

.line-ending-select {
  width: 160px;
}

.text-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
  flex: 1;
  min-height: 240px;
}

.textarea-card {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}

.textarea-card,
.operation-card,
.preview-card {
  border: 1px solid var(--lc-border);
  border-radius: 10px;
  background: var(--el-bg-color-page);
  padding: 10px;
}

.textarea-card :deep(.el-textarea) {
  flex: 1;
  min-height: 0;
}

.textarea-card :deep(.el-textarea__inner) {
  height: 100% !important;
  min-height: 240px;
}

.process-tabs {
  flex-shrink: 0;
}

.card-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
  font-weight: 600;
}

.meta {
  font-size: 12px;
  color: var(--lc-text-muted);
  font-weight: 500;
}

.operation-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 10px;
}

.op-title {
  font-size: 13px;
  font-weight: 600;
  margin-bottom: 10px;
}

.op-row {
  display: grid;
  grid-template-columns: 140px 120px minmax(0, 1fr) minmax(0, 1fr);
  gap: 10px;
  align-items: center;
  margin-bottom: 8px;
}

.inline-options {
  margin-top: 8px;
  display: flex;
  align-items: center;
  gap: 10px;
}

.summary-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
  gap: 10px;
}

.summary-item {
  border: 1px solid var(--lc-border);
  border-radius: 10px;
  background: var(--lc-surface-1);
  padding: 10px;
}

.summary-label {
  font-size: 12px;
  color: var(--lc-text-muted);
}

.summary-value {
  font-size: 20px;
  font-weight: 600;
}

.summary-value.accent {
  color: var(--lc-accent-light);
}

@media (max-width: 1100px) {
  .text-grid {
    grid-template-columns: 1fr;
    grid-template-rows: repeat(2, minmax(200px, 1fr));
    overflow: auto;
  }

  .summary-grid {
    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
  }

  .op-row {
    grid-template-columns: 1fr;
  }

  .preset-select {
    width: 100%;
  }
}
</style>

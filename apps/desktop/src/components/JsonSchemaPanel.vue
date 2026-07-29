<template>
  <div class="json-schema-panel">
    <div class="schema-toolbar">
      <el-button type="primary" :loading="validating" @click="validateSchema">校验 JSON</el-button>
      <el-button :loading="generating" @click="generateExample">生成样例</el-button>
      <el-select
        v-if="combination"
        v-model="branchIndex"
        class="branch-select"
        aria-label="样例组合分支"
      >
        <el-option
          v-for="index in combination.count"
          :key="index - 1"
          :label="`${combination.keyword} · 第 ${index} 分支`"
          :value="index - 1"
        />
      </el-select>
      <span v-if="combination" class="toolbar-hint">样例按所选根分支生成，嵌套组合沿用该序号</span>
    </div>

    <div class="schema-editors">
      <section class="editor-card" aria-labelledby="schema-editor-title">
        <header class="editor-header">
          <div>
            <strong id="schema-editor-title">JSON Schema</strong>
            <span>支持本地 $ref、allOf、oneOf / anyOf</span>
          </div>
          <el-button size="small" @click="formatInput('schema')">格式化</el-button>
        </header>
        <MonacoPane
          ref="schemaEditorRef"
          v-model="schemaInput"
          language="json"
          aria-label="JSON Schema 编辑器"
        />
      </section>

      <section class="editor-card" aria-labelledby="document-editor-title">
        <header class="editor-header">
          <div>
            <strong id="document-editor-title">待校验 JSON</strong>
            <span>校验错误可从下方列表定位到相关字段</span>
          </div>
          <el-button size="small" @click="formatInput('document')">格式化</el-button>
        </header>
        <MonacoPane
          ref="documentEditorRef"
          v-model="documentInput"
          language="json"
          aria-label="待校验 JSON 编辑器"
        />
      </section>
    </div>

    <el-alert
      v-if="operationError"
      class="operation-alert"
      type="error"
      :title="operationError"
      show-icon
      :closable="false"
      role="alert"
    />

    <el-alert
      v-if="validationResult"
      :type="validationResult.valid ? 'success' : 'error'"
      :title="validationResult.valid ? '校验通过' : `校验失败（${validationResult.errors.length} 条）`"
      show-icon
      :closable="false"
    />

    <el-table
      v-if="validationResult && !validationResult.valid"
      :data="validationResult.errors"
      border
      max-height="260"
      size="small"
      class="validation-table"
      @row-dblclick="locateValidationError"
    >
      <el-table-column prop="instancePath" label="实例路径" min-width="180">
        <template #default="{ row }">
          <code>{{ row.instancePath || "/" }}</code>
        </template>
      </el-table-column>
      <el-table-column prop="schemaPath" label="Schema 路径" min-width="220">
        <template #default="{ row }">
          <code>{{ row.schemaPath || "/" }}</code>
        </template>
      </el-table-column>
      <el-table-column prop="message" label="错误信息" min-width="300" />
      <el-table-column label="操作" width="82" fixed="right">
        <template #default="{ row }">
          <el-button link type="primary" @click="locateValidationError(row)">定位</el-button>
        </template>
      </el-table-column>
    </el-table>

    <el-alert
      v-if="generationWarnings.length"
      type="warning"
      :title="`样例已生成，但有 ${generationWarnings.length} 项需要确认`"
      :closable="false"
      show-icon
    >
      <ul class="warning-list">
        <li v-for="warning in generationWarnings" :key="warning">{{ warning }}</li>
      </ul>
    </el-alert>

    <section class="example-card" aria-labelledby="example-output-title">
      <header class="editor-header">
        <div>
          <strong id="example-output-title">生成样例</strong>
          <span>生成后会反向校验；警告不会被静默忽略</span>
        </div>
        <el-space>
          <el-button size="small" :disabled="!exampleOutput" @click="copyExample">复制</el-button>
          <el-button size="small" :disabled="!exampleOutput" @click="applyExample">填入待校验区</el-button>
        </el-space>
      </header>
      <MonacoPane
        v-model="exampleOutput"
        language="json"
        read-only
        aria-label="生成样例只读编辑器"
      />
    </section>
  </div>
</template>

<script lang="ts">
const jsonSchemaDefaults = {
  schema: `{
  "type": "object",
  "required": ["id", "name"],
  "properties": {
    "id": { "type": "integer", "minimum": 1 },
    "name": { "type": "string" },
    "email": { "type": "string", "format": "email" }
  }
}`,
  document: `{
  "id": 1,
  "name": "lazycat"
}`,
};
const jsonSchemaState = {
  schema: jsonSchemaDefaults.schema,
  document: jsonSchemaDefaults.document,
  example: "",
  warnings: [] as string[],
  branchIndex: 0,
};
</script>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import {
  formatJsonDocument,
  parseJsonErrorLocation,
  pointerLastToken,
  rootCombination,
} from "../utils/jsonSchema";
import MonacoPane from "./MonacoPane.vue";

interface MonacoPaneApi {
  focusLine(line: number, column?: number): void;
  focusText(text: string): boolean;
}

interface SchemaValidationError {
  instancePath: string;
  schemaPath: string;
  message: string;
}

interface SchemaValidationResult {
  valid: boolean;
  errors: SchemaValidationError[];
}

interface GenerateExampleResult {
  example?: unknown;
  warnings?: unknown;
}

const schemaInput = ref(jsonSchemaState.schema);
const documentInput = ref(jsonSchemaState.document);
const exampleOutput = ref(jsonSchemaState.example);
const generationWarnings = ref([...jsonSchemaState.warnings]);
const branchIndex = ref(jsonSchemaState.branchIndex);
const validationResult = ref<SchemaValidationResult | null>(null);
const operationError = ref("");
const validating = ref(false);
const generating = ref(false);
const schemaEditorRef = ref<MonacoPaneApi | null>(null);
const documentEditorRef = ref<MonacoPaneApi | null>(null);
const combination = computed(() => rootCombination(schemaInput.value));
let validateSequence = 0;
let generateSequence = 0;

watch(combination, (value) => {
  if (!value || branchIndex.value >= value.count) branchIndex.value = 0;
});

watch([schemaInput, documentInput], () => {
  validationResult.value = null;
  operationError.value = "";
});

async function validateSchema() {
  const sequence = ++validateSequence;
  const schema = schemaInput.value;
  const document = documentInput.value;
  validating.value = true;
  operationError.value = "";
  try {
    const data = (await invokeToolByChannel("tool:schema:validate", { schema, document })) as SchemaValidationResult;
    if (sequence !== validateSequence || schema !== schemaInput.value || document !== documentInput.value) return;
    validationResult.value = {
      valid: Boolean(data?.valid),
      errors: Array.isArray(data?.errors) ? data.errors : [],
    };
  } catch (error) {
    if (sequence !== validateSequence) return;
    showOperationError(error, "document");
  } finally {
    if (sequence === validateSequence) validating.value = false;
  }
}

async function generateExample() {
  const sequence = ++generateSequence;
  const schema = schemaInput.value;
  generating.value = true;
  operationError.value = "";
  try {
    const data = (await invokeToolByChannel("tool:schema:generate-example", {
      schema,
      branchIndex: branchIndex.value,
    })) as GenerateExampleResult;
    if (sequence !== generateSequence || schema !== schemaInput.value) return;
    exampleOutput.value = JSON.stringify(data?.example ?? null, null, 2);
    generationWarnings.value = Array.isArray(data?.warnings)
      ? data.warnings.filter((warning): warning is string => typeof warning === "string")
      : [];
  } catch (error) {
    if (sequence !== generateSequence) return;
    showOperationError(error, "schema");
  } finally {
    if (sequence === generateSequence) generating.value = false;
  }
}

function formatInput(target: "schema" | "document") {
  const input = target === "schema" ? schemaInput : documentInput;
  try {
    input.value = formatJsonDocument(input.value);
    ElMessage.success("已格式化 JSON");
  } catch (error) {
    showOperationError(error, target);
  }
}

function showOperationError(error: unknown, preferredTarget: "schema" | "document") {
  const message = error instanceof Error ? error.message : String(error);
  operationError.value = message;
  const location = parseJsonErrorLocation(message);
  if (location) {
    const target = message.includes("Schema") ? schemaEditorRef.value : preferredTarget === "schema"
      ? schemaEditorRef.value
      : documentEditorRef.value;
    target?.focusLine(location.line, location.column);
  }
}

function locateValidationError(error: SchemaValidationError) {
  const token = pointerLastToken(error.instancePath);
  if (token && documentEditorRef.value?.focusText(token)) return;
  documentEditorRef.value?.focusLine(1);
  ElMessage.info(token ? "未找到唯一字段位置，已定位到文档开头" : "该错误位于文档根节点");
}

function applyExample() {
  if (!exampleOutput.value.trim()) return;
  documentInput.value = exampleOutput.value;
  ElMessage.success("样例已填入待校验区");
}

async function copyExample() {
  if (!exampleOutput.value) return;
  try {
    await navigator.clipboard.writeText(exampleOutput.value);
    ElMessage.success("样例已复制");
  } catch {
    ElMessage.error("复制失败，请检查剪贴板权限");
  }
}

onBeforeUnmount(() => {
  jsonSchemaState.schema = schemaInput.value;
  jsonSchemaState.document = documentInput.value;
  jsonSchemaState.example = exampleOutput.value;
  jsonSchemaState.warnings = [...generationWarnings.value];
  jsonSchemaState.branchIndex = branchIndex.value;
});
</script>

<style scoped>
.json-schema-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex: 1;
  min-height: 0;
}

.schema-toolbar,
.editor-header {
  display: flex;
  align-items: center;
  gap: 8px;
}

.schema-toolbar {
  flex-wrap: wrap;
}

.toolbar-hint,
.editor-header span {
  color: var(--lc-text-muted);
  font-size: 12px;
}

.branch-select {
  width: 190px;
}

.schema-editors {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 12px;
  flex: 1;
  min-height: 300px;
}

.editor-card,
.example-card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

.editor-card :deep(.monaco-pane) {
  flex: 1;
  height: auto;
  min-height: 300px;
}

.example-card :deep(.monaco-pane) {
  height: 220px;
}

.editor-header {
  justify-content: space-between;
  min-height: 32px;
}

.editor-header > div {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
}

.editor-header strong {
  font-size: 13px;
}

.validation-table :deep(code) {
  font-family: var(--lc-font-mono, "Cascadia Code", "JetBrains Mono", monospace);
  font-size: 12px;
}

.warning-list {
  margin: 8px 0 0;
  padding-left: 20px;
  line-height: 1.6;
}

@media (max-width: 900px) {
  .schema-editors {
    grid-template-columns: 1fr;
    grid-template-rows: repeat(2, minmax(240px, 1fr));
    overflow: auto;
  }

  .editor-card :deep(.monaco-pane) {
    min-height: 240px;
  }
}
</style>

<template>
  <div class="panel-grid">
    <el-input
      v-model="schemaInput"
      type="textarea"
      :rows="12"
      placeholder="输入 JSON Schema"
    />
    <el-input
      v-model="documentInput"
      type="textarea"
      :rows="12"
      placeholder="输入待校验 JSON"
    />
    <div class="panel-grid-full">
      <el-space>
        <el-button type="primary" @click="validateSchema">校验</el-button>
        <el-button @click="generateExample">生成样例</el-button>
        <el-button @click="applyExample">样例填入右侧</el-button>
      </el-space>
    </div>
    <el-alert
      v-if="validationResult"
      class="panel-grid-full"
      :type="validationResult.valid ? 'success' : 'error'"
      :title="validationResult.valid ? '校验通过' : `校验失败（${validationResult.errors.length} 条）`"
      show-icon
      :closable="false"
    />
    <el-table
      v-if="validationResult && !validationResult.valid"
      class="panel-grid-full"
      :data="validationResult.errors"
      border
      max-height="260"
    >
      <el-table-column prop="instancePath" label="实例路径" min-width="180" />
      <el-table-column prop="schemaPath" label="Schema 路径" min-width="220" />
      <el-table-column prop="message" label="错误信息" min-width="260" />
    </el-table>
    <el-input
      v-model="exampleOutput"
      class="panel-grid-full"
      type="textarea"
      :rows="8"
      readonly
      placeholder="样例输出"
    />
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

interface SchemaValidationError {
  instancePath: string;
  schemaPath: string;
  message: string;
}

interface SchemaValidationResult {
  valid: boolean;
  errors: SchemaValidationError[];
}

const schemaInput = ref(`{
  "type": "object",
  "required": ["id", "name"],
  "properties": {
    "id": { "type": "integer" },
    "name": { "type": "string" },
    "email": { "type": "string", "format": "email" }
  }
}`);
const documentInput = ref(`{
  "id": 1,
  "name": "lazycat"
}`);
const exampleOutput = ref("");
const validationResult = ref<SchemaValidationResult | null>(null);

async function validateSchema() {
  try {
    const data = (await invokeToolByChannel("tool:schema:validate", {
      schema: schemaInput.value,
      document: documentInput.value,
    })) as SchemaValidationResult;
    validationResult.value = {
      valid: !!data?.valid,
      errors: Array.isArray(data?.errors) ? data.errors : [],
    };
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

async function generateExample() {
  try {
    const data = (await invokeToolByChannel("tool:schema:generate-example", {
      schema: schemaInput.value,
    })) as { example?: unknown };
    exampleOutput.value = JSON.stringify(data?.example ?? {}, null, 2);
  } catch (error) {
    ElMessage.error((error as Error).message);
  }
}

function applyExample() {
  if (!exampleOutput.value.trim()) return;
  documentInput.value = exampleOutput.value;
}
</script>

<template>
  <div class="panel-grid">
    <!-- CSV Input -->
    <div class="panel-grid-full" style="display:flex;gap:8px;align-items:center;">
      <el-button @click="pickFile">选择 CSV 文件</el-button>
      <span v-if="filePath" style="font-size:13px;color:var(--el-text-color-secondary);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">{{ filePath }}</span>
    </div>
    <el-input v-model="csvInput" type="textarea" :rows="10" placeholder="输入 CSV 或通过上方按钮选择文件" />
    <el-input :model-value="jsonOutput" type="textarea" :rows="10" readonly placeholder="JSON 结果" />

    <!-- Options row -->
    <div class="panel-grid-full" style="display:flex;flex-wrap:wrap;gap:16px;align-items:center;">
      <el-input v-model="delimiter" placeholder="分隔符" style="width:100px;" />
      <el-switch v-model="hasHeader" active-text="CSV 含表头" />
    </div>

    <!-- Custom headers (when no header) -->
    <div v-if="!hasHeader && columnCount > 0" class="panel-grid-full">
      <p style="margin:0 0 8px;font-size:13px;color:var(--el-text-color-secondary);">自定义列名（共 {{ columnCount }} 列）</p>
      <div style="display:flex;flex-wrap:wrap;gap:8px;">
        <el-input
          v-for="(_, idx) in customHeaders"
          :key="idx"
          v-model="customHeaders[idx]"
          :placeholder="'col' + (idx + 1)"
          style="width:120px;"
        />
      </div>
    </div>

    <!-- Column selection (when has header and columns detected) -->
    <div v-if="hasHeader && detectedHeaders.length > 0" class="panel-grid-full">
      <div style="display:flex;align-items:center;gap:12px;margin-bottom:8px;">
        <span style="font-size:13px;color:var(--el-text-color-secondary);">选择导出列</span>
        <el-button text size="small" @click="toggleAllColumns">{{ allColumnsSelected ? '取消全选' : '全选' }}</el-button>
      </div>
      <el-checkbox-group v-model="selectedColumnIndices">
        <el-checkbox v-for="(header, idx) in detectedHeaders" :key="idx" :value="idx">
          {{ header }}
        </el-checkbox>
      </el-checkbox-group>
    </div>

    <!-- Actions -->
    <div class="panel-grid-full">
      <el-space>
        <el-button type="primary" @click="convert">CSV -> JSON</el-button>
        <el-button @click="copyOutput">复制结果</el-button>
        <el-button @click="clearAll">清空</el-button>
      </el-space>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { open } from "@tauri-apps/plugin-dialog";
import { invokeToolByChannel } from "../bridge/tauri";

const csvInput = ref("");
const jsonOutput = ref("");
const delimiter = ref(",");
const hasHeader = ref(true);
const filePath = ref("");
const customHeaders = ref<string[]>([]);
const selectedColumnIndices = ref<number[]>([]);
const detectedHeaders = ref<string[]>([]);

const columnCount = computed(() => {
  const firstLine = csvInput.value.split(/\r?\n/).find((l) => l.trim());
  if (!firstLine) return 0;
  const d = delimiter.value || ",";
  return firstLine.split(d).length;
});

const allColumnsSelected = computed(() => {
  return detectedHeaders.value.length > 0 && selectedColumnIndices.value.length === detectedHeaders.value.length;
});

function toggleAllColumns() {
  if (allColumnsSelected.value) {
    selectedColumnIndices.value = [];
  } else {
    selectedColumnIndices.value = detectedHeaders.value.map((_, i) => i);
  }
}

// When hasHeader is turned off, generate default custom header names
watch([() => hasHeader.value, columnCount], ([hh, count]) => {
  if (!hh && count > 0) {
    customHeaders.value = Array.from({ length: count }, (_, i) => `col${i + 1}`);
  }
});

// When CSV input or delimiter changes and hasHeader is on, detect headers
watch([() => csvInput.value, () => delimiter.value, () => hasHeader.value], () => {
  if (!hasHeader.value) {
    detectedHeaders.value = [];
    selectedColumnIndices.value = [];
    return;
  }
  const firstLine = csvInput.value.split(/\r?\n/).find((l) => l.trim());
  if (!firstLine) {
    detectedHeaders.value = [];
    selectedColumnIndices.value = [];
    return;
  }
  const d = delimiter.value || ",";
  const headers = firstLine.split(d).map((h) => h.trim());
  detectedHeaders.value = headers;
  // Select all by default
  selectedColumnIndices.value = headers.map((_, i) => i);
});

async function pickFile() {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: "CSV", extensions: ["csv", "tsv", "txt"] }]
    });
    if (!selected) return;
    const path = typeof selected === "string" ? selected : selected.path;
    if (!path) return;
    filePath.value = path;
    const content = await invokeToolByChannel("tool:convert:csv-read-file", { path });
    csvInput.value = typeof content === "string" ? content : "";
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function convert() {
  if (!csvInput.value.trim()) {
    jsonOutput.value = "";
    return;
  }
  try {
    const payload: Record<string, unknown> = {
      input: csvInput.value,
      delimiter: delimiter.value || ",",
      hasHeader: hasHeader.value
    };
    if (!hasHeader.value && customHeaders.value.length > 0) {
      payload.customHeaders = customHeaders.value;
    }
    if (hasHeader.value && selectedColumnIndices.value.length > 0 && selectedColumnIndices.value.length < detectedHeaders.value.length) {
      payload.selectedColumns = selectedColumnIndices.value;
    }
    jsonOutput.value = String(await invokeToolByChannel("tool:convert:csv-to-json", payload));
  } catch (e) {
    ElMessage.error((e as Error).message);
  }
}

async function copyOutput() {
  if (!jsonOutput.value) return;
  try {
    await navigator.clipboard.writeText(jsonOutput.value);
    ElMessage.success("已复制");
  } catch {
    ElMessage.error("复制失败");
  }
}

function clearAll() {
  csvInput.value = "";
  jsonOutput.value = "";
  filePath.value = "";
  customHeaders.value = [];
  selectedColumnIndices.value = [];
  detectedHeaders.value = [];
}
</script>

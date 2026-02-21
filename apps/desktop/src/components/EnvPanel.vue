<template>
  <div class="env-panel">
    <div class="env-toolbar">
      <el-button type="primary" :loading="detecting" @click="detectEnv">检测开发环境</el-button>
      <el-button :disabled="!result" @click="copyReport">复制报告</el-button>
    </div>

    <el-alert
      v-if="result"
      :title="`已检测 ${result.summary.total} 项，已安装 ${result.summary.installed} 项，缺失 ${result.summary.missing} 项`"
      type="info"
      :closable="false"
      show-icon
    />

    <el-table v-if="result" :data="result.tools" border stripe size="small" class="env-table">
      <el-table-column label="工具" min-width="120">
        <template #default="{ row }">
          <span class="tool-name">{{ row.name }}</span>
        </template>
      </el-table-column>
      <el-table-column label="状态" width="90" align="center">
        <template #default="{ row }">
          <el-tag size="small" :type="row.installed ? 'success' : 'danger'">
            {{ row.installed ? "已安装" : "缺失" }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column label="版本" min-width="220" show-overflow-tooltip>
        <template #default="{ row }">
          <code>{{ row.version || "UNKNOWN" }}</code>
        </template>
      </el-table-column>
      <el-table-column label="路径" min-width="260" show-overflow-tooltip>
        <template #default="{ row }">
          <code class="text-muted">{{ row.path || "-" }}</code>
        </template>
      </el-table-column>
      <el-table-column label="备注" min-width="220" show-overflow-tooltip>
        <template #default="{ row }">
          <span class="text-muted">{{ row.error || "-" }}</span>
        </template>
      </el-table-column>
    </el-table>

    <el-input
      class="env-raw-json"
      v-model="envOutput"
      type="textarea"
      :rows="8"
      readonly
      placeholder="点击“检测开发环境”后查看 JSON 原始结果"
    />

    <div v-if="result" class="env-meta text-muted">
      平台：{{ result.platform }} / {{ result.arch }}，检测耗时：{{ result.duration_ms }} ms
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";

interface EnvToolItem {
  key: string;
  name: string;
  installed: boolean;
  version: string;
  path: string;
  error: string | null;
}

interface EnvDetectResponse {
  platform: string;
  arch: string;
  duration_ms: number;
  summary: {
    total: number;
    installed: number;
    missing: number;
  };
  tools: EnvToolItem[];
}

const envOutput = ref("");
const detecting = ref(false);
const result = ref<EnvDetectResponse | null>(null);

async function detectEnv() {
  detecting.value = true;
  try {
    const data = await invokeToolByChannel("tool:env:detect", {});
    envOutput.value = JSON.stringify(data, null, 2);
    result.value = data as EnvDetectResponse;
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    detecting.value = false;
  }
}

async function copyReport() {
  if (!result.value) {
    ElMessage.warning("请先执行环境检测");
    return;
  }
  await navigator.clipboard.writeText(JSON.stringify(result.value, null, 2));
  ElMessage.success("环境报告已复制");
}
</script>

<style scoped>
.env-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.env-toolbar {
  display: flex;
  gap: 10px;
}

.env-table :deep(code) {
  font-family: var(--lc-font-mono, "Cascadia Code", "JetBrains Mono", monospace);
  font-size: 12px;
}

.tool-name {
  font-weight: 600;
}

.env-raw-json :deep(.el-textarea__inner) {
  font-family: var(--lc-font-mono, "Cascadia Code", "JetBrains Mono", monospace);
  font-size: 12px;
}

.env-meta {
  font-size: 12px;
}
</style>

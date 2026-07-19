<template>
  <div class="env-panel">
    <div class="env-toolbar">
      <el-button type="primary" :loading="detecting" @click="detectEnv">
        {{ result ? "重新检测" : "检测开发环境" }}
      </el-button>
      <el-button :disabled="!result" @click="copyReport">复制排障报告</el-button>
      <span v-if="inspectedAt" class="toolbar-meta">本次会话最近检测：{{ inspectedAt }}</span>
    </div>

    <el-alert
      v-if="operationError"
      type="error"
      :title="operationError"
      description="请确认系统允许执行本地命令，然后重新检测。"
      :closable="false"
      show-icon
      role="alert"
    />

    <el-alert
      v-if="result"
      :type="summaryType"
      :title="summaryTitle"
      :description="`检测 ${result.summary.total} 项工具，正常 ${result.summary.installed} 项；耗时 ${result.durationMs} ms。未安装的可选工具不会影响其他功能。`"
      :closable="false"
      show-icon
    />

    <section v-if="result?.diagnostics.length" class="diagnostics-section" aria-labelledby="diagnostics-title">
      <div class="section-heading">
        <strong id="diagnostics-title">诊断与处理建议</strong>
        <span>按错误、警告、提示排序</span>
      </div>
      <div class="diagnostic-list">
        <article
          v-for="diagnostic in sortedDiagnostics"
          :key="`${diagnostic.level}:${diagnostic.title}`"
          class="diagnostic-item"
          :class="`is-${diagnostic.level}`"
        >
          <el-tag size="small" :type="diagnosticTagType(diagnostic.level)">
            {{ diagnosticLevelLabel(diagnostic.level) }}
          </el-tag>
          <div>
            <strong>{{ diagnostic.title }}</strong>
            <p>{{ diagnostic.detail }}</p>
            <p v-if="diagnostic.suggestion" class="diagnostic-suggestion">处理：{{ diagnostic.suggestion }}</p>
          </div>
        </article>
      </div>
    </section>

    <section v-if="result" aria-labelledby="tools-title">
      <div class="section-heading">
        <strong id="tools-title">工具与版本</strong>
        <span>状态以版本命令的真实退出结果为准</span>
      </div>
      <el-table :data="result.tools" border stripe size="small" class="env-table">
        <el-table-column label="工具" min-width="120">
          <template #default="{ row }"><span class="tool-name">{{ row.name }}</span></template>
        </el-table-column>
        <el-table-column label="状态" width="92" align="center">
          <template #default="{ row }">
            <el-tag size="small" :type="toolTagType(row.status)">{{ toolStatusLabel(row.status) }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column label="版本" min-width="210" show-overflow-tooltip>
          <template #default="{ row }"><code>{{ row.version }}</code></template>
        </el-table-column>
        <el-table-column label="当前命令路径" min-width="280" show-overflow-tooltip>
          <template #default="{ row }">
            <code class="text-muted">{{ row.path || "-" }}</code>
            <el-tag v-if="row.paths.length > 1" size="small" type="warning" class="path-count">
              {{ row.paths.length }} 个 PATH 命中
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="检测说明" min-width="260" show-overflow-tooltip>
          <template #default="{ row }"><span class="text-muted">{{ row.error || row.suggestion || "正常" }}</span></template>
        </el-table-column>
      </el-table>
    </section>

    <section v-if="result" aria-labelledby="variables-title">
      <div class="section-heading">
        <strong id="variables-title">关键环境变量</strong>
        <span>只检查常用开发工具目录，不读取敏感变量</span>
      </div>
      <el-table :data="result.environment" border size="small" class="env-table">
        <el-table-column prop="key" label="变量" min-width="140" />
        <el-table-column label="状态" width="92" align="center">
          <template #default="{ row }">
            <el-tag size="small" :type="toolTagType(row.status)">{{ toolStatusLabel(row.status) }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column label="值" min-width="300" show-overflow-tooltip>
          <template #default="{ row }"><code>{{ row.value || "-" }}</code></template>
        </el-table-column>
        <el-table-column prop="detail" label="说明" min-width="240" />
      </el-table>
    </section>

    <el-collapse v-if="result" class="raw-details">
      <el-collapse-item title="查看原始检测数据（用于开发排查）" name="raw">
        <pre>{{ rawOutput }}</pre>
      </el-collapse-item>
    </el-collapse>
  </div>
</template>

<script lang="ts">
import type { EnvDetectResponse } from "../utils/envDiagnostics";

const envPanelState: { result: EnvDetectResponse | null; inspectedAt: string } = {
  result: null,
  inspectedAt: "",
};
</script>

<script setup lang="ts">
import { computed, ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import {
  buildEnvironmentReport,
  environmentSummaryType,
  toolStatusLabel,
  type EnvDetectResponse,
  type EnvDiagnostic,
  type EnvironmentStatus,
} from "../utils/envDiagnostics";

const detecting = ref(false);
const result = ref<EnvDetectResponse | null>(envPanelState.result);
const inspectedAt = ref(envPanelState.inspectedAt);
const operationError = ref("");
const rawOutput = computed(() => result.value ? JSON.stringify(result.value, null, 2) : "");
const summaryType = computed(() => result.value ? environmentSummaryType(result.value) : "success");
const summaryTitle = computed(() => {
  if (!result.value) return "";
  if (result.value.summary.problems) return `发现 ${result.value.summary.problems} 项命令异常或超时`;
  if (result.value.summary.warnings) return `环境可用，但有 ${result.value.summary.warnings} 项配置建议`;
  return "已完成检测，未发现需要立即处理的问题";
});
const sortedDiagnostics = computed(() => {
  const weights = { error: 0, warning: 1, info: 2 };
  return [...(result.value?.diagnostics ?? [])].sort((left, right) => weights[left.level] - weights[right.level]);
});

async function detectEnv() {
  detecting.value = true;
  operationError.value = "";
  try {
    const data = await invokeToolByChannel("tool:env:detect", {}) as EnvDetectResponse;
    result.value = data;
    inspectedAt.value = new Date().toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
    envPanelState.result = data;
    envPanelState.inspectedAt = inspectedAt.value;
  } catch (error) {
    operationError.value = error instanceof Error ? error.message : String(error);
  } finally {
    detecting.value = false;
  }
}

async function copyReport() {
  if (!result.value) return;
  try {
    await navigator.clipboard.writeText(buildEnvironmentReport(result.value));
    ElMessage.success("排障报告已复制");
  } catch {
    ElMessage.error("复制失败，请检查剪贴板权限");
  }
}

function toolTagType(status: EnvironmentStatus) {
  if (status === "ok") return "success";
  if (status === "missing") return "info";
  if (status === "timeout") return "warning";
  return "danger";
}

function diagnosticTagType(level: EnvDiagnostic["level"]) {
  return level === "error" ? "danger" : level === "warning" ? "warning" : "info";
}

function diagnosticLevelLabel(level: EnvDiagnostic["level"]) {
  return level === "error" ? "错误" : level === "warning" ? "警告" : "提示";
}
</script>

<style scoped>
.env-panel {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.env-toolbar,
.section-heading {
  display: flex;
  align-items: center;
  gap: 10px;
}

.env-toolbar {
  flex-wrap: wrap;
}

.toolbar-meta,
.section-heading span {
  color: var(--lc-text-muted);
  font-size: 12px;
}

.toolbar-meta {
  margin-left: auto;
}

.section-heading {
  margin-bottom: 7px;
}

.section-heading strong {
  font-size: 13px;
}

.diagnostic-list {
  display: grid;
  gap: 8px;
}

.diagnostic-item {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  align-items: start;
  gap: 10px;
  padding: 10px 12px;
  border: 1px solid var(--lc-border);
  border-left-width: 3px;
  border-radius: var(--lc-radius-md, 10px);
  background: var(--lc-bg-panel, #fff);
}

.diagnostic-item.is-error { border-left-color: var(--el-color-danger); }
.diagnostic-item.is-warning { border-left-color: var(--el-color-warning); }
.diagnostic-item.is-info { border-left-color: var(--el-color-info); }

.diagnostic-item strong,
.diagnostic-item p {
  font-size: 12px;
  line-height: 1.55;
}

.diagnostic-item p {
  margin: 2px 0 0;
  color: var(--lc-text-muted);
}

.diagnostic-item .diagnostic-suggestion {
  color: var(--lc-text);
}

.env-table :deep(code),
.raw-details pre {
  font-family: var(--lc-font-mono, "Cascadia Code", "JetBrains Mono", monospace);
  font-size: 12px;
}

.tool-name {
  font-weight: 600;
}

.path-count {
  margin-left: 6px;
}

.raw-details pre {
  max-height: 320px;
  margin: 0;
  padding: 10px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
  background: var(--lc-bg-soft);
  border-radius: var(--lc-radius-sm, 6px);
}

@media (max-width: 720px) {
  .toolbar-meta {
    width: 100%;
    margin-left: 0;
  }

  .section-heading {
    align-items: flex-start;
    flex-direction: column;
    gap: 2px;
  }
}
</style>

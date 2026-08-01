<template>
  <div class="panel-grid file-lock-panel">
    <div class="panel-grid-full file-lock-heading">
      <div>
        <h2>Windows 文件占用</h2>
        <p>检查当前由哪些进程关联指定文件。</p>
      </div>
      <el-tag type="info" effect="plain">只读诊断</el-tag>
    </div>

    <div class="panel-grid-full file-lock-toolbar">
      <el-input
        v-model="path"
        class="file-lock-path-input"
        clearable
        placeholder="输入或选择文件路径，例如 D:\\work\\demo\\target\\app.jar"
        @keyup.enter="inspect"
      >
        <template #prepend>文件路径</template>
      </el-input>
      <el-button :icon="FolderOpened" @click="pickFile">选择文件</el-button>
      <el-button
        type="primary"
        :icon="Search"
        :loading="loading"
        :disabled="!path.trim()"
        @click="inspect"
      >
        检查占用
      </el-button>
    </div>

    <div v-if="errorMessage" class="panel-grid-full">
      <el-alert :title="errorMessage" type="error" show-icon :closable="false" />
    </div>

    <template v-if="result">
      <section class="panel-grid-full file-lock-summary" aria-labelledby="file-lock-summary-title">
        <div class="file-lock-summary__header">
          <div>
            <span id="file-lock-summary-title" class="file-lock-label">检查对象</span>
            <el-tooltip :content="result.canonicalPath" placement="top">
              <code class="file-lock-path">{{ result.canonicalPath }}</code>
            </el-tooltip>
          </div>
          <div class="file-lock-summary__actions">
            <el-tag :type="result.processes.length > 0 ? 'warning' : 'success'">
              {{ result.processes.length }} 个关联进程
            </el-tag>
            <el-tooltip content="复制规范化路径" placement="top">
              <el-button
                link
                :icon="CopyDocument"
                aria-label="复制规范化路径"
                @click="copyText(result.canonicalPath, '已复制文件路径')"
              />
            </el-tooltip>
          </div>
        </div>
        <div class="file-lock-summary__meta">
          <span>扫描时间：{{ formatDate(result.scannedAt) }}</span>
          <el-button link :icon="Refresh" :loading="loading" @click="inspect">重新扫描</el-button>
        </div>
      </section>

      <div v-if="result.warnings.length > 0" class="panel-grid-full file-lock-warnings">
        <el-alert
          v-for="warning in result.warnings"
          :key="warning"
          :title="warning"
          type="warning"
          show-icon
          :closable="false"
        />
      </div>

      <div v-if="result.processes.length === 0" class="panel-grid-full">
        <el-alert
          title="Windows 未报告可关联进程"
          description="这不等同于绝对没有占用。受保护进程、权限限制或文件过滤驱动可能导致结果不完整。"
          type="info"
          show-icon
          :closable="false"
        />
      </div>

      <div v-else class="panel-grid-full file-lock-table-wrap" v-loading="loading">
        <el-table :data="result.processes" border stripe row-key="pid" empty-text="没有可显示的进程">
          <el-table-column label="PID" width="100">
            <template #default="{ row }">
              <code class="file-lock-mono">{{ row.pid }}</code>
            </template>
          </el-table-column>
          <el-table-column label="应用" prop="appName" min-width="160" show-overflow-tooltip />
          <el-table-column label="类型" width="130">
            <template #default="{ row }">
              <span>{{ appTypeLabel(row.appType) }}</span>
            </template>
          </el-table-column>
          <el-table-column label="状态" width="110">
            <template #default="{ row }">
              <el-tag size="small" :type="statusTagType(row.status)">
                {{ statusLabel(row.status) }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column label="可执行文件" min-width="280" show-overflow-tooltip>
            <template #default="{ row }">
              <code v-if="row.executablePath" class="file-lock-path-cell">{{ row.executablePath }}</code>
              <span v-else class="file-lock-muted">无法读取</span>
            </template>
          </el-table-column>
          <el-table-column label="操作" width="118" fixed="right">
            <template #default="{ row }">
              <el-space :size="2">
                <el-tooltip content="复制 PID" placement="top">
                  <el-button
                    link
                    :icon="CopyDocument"
                    :aria-label="`复制 PID ${row.pid}`"
                    @click="copyText(String(row.pid), '已复制 PID')"
                  />
                </el-tooltip>
                <el-tooltip v-if="row.executablePath" content="复制可执行文件路径" placement="top">
                  <el-button
                    link
                    :icon="DocumentCopy"
                    :aria-label="`复制 ${row.appName} 路径`"
                    @click="copyText(row.executablePath, '已复制进程路径')"
                  />
                </el-tooltip>
                <el-tooltip v-if="row.executablePath" content="在资源管理器中定位" placement="top">
                  <el-button
                    link
                    :icon="Location"
                    :aria-label="`定位 ${row.appName} 文件`"
                    @click="revealPath(row.executablePath)"
                  />
                </el-tooltip>
              </el-space>
            </template>
          </el-table-column>
        </el-table>
      </div>
    </template>

    <div v-else-if="!loading" class="panel-grid-full file-lock-empty">
      <el-empty description="选择一个文件后开始检查" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import {
  CopyDocument,
  DocumentCopy,
  FolderOpened,
  Location,
  Refresh,
  Search,
} from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type { FileLockInspectResponse } from "../types";

const path = ref("");
const loading = ref(false);
const errorMessage = ref("");
const result = ref<FileLockInspectResponse | null>(null);

const APP_TYPE_LABELS: Record<string, string> = {
  "main-window": "主窗口",
  "other-window": "其他窗口",
  service: "服务",
  explorer: "资源管理器",
  console: "控制台",
  critical: "关键进程",
  unknown: "未知",
};

const STATUS_LABELS: Record<string, string> = {
  running: "运行中",
  stopped: "已停止",
  "stopped-other": "其他状态",
  unknown: "未知",
};

function appTypeLabel(value: string): string {
  return APP_TYPE_LABELS[value] ?? (value || "未知");
}

function statusLabel(value: string): string {
  return STATUS_LABELS[value] ?? (value || "未知");
}

function statusTagType(value: string): "success" | "warning" | "info" | "danger" | "" {
  if (value === "running") return "success";
  if (value === "stopped") return "info";
  if (value === "stopped-other") return "warning";
  return "";
}

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value || "-";
  return date.toLocaleString();
}

function dialogPath(value: string | string[] | null): string {
  if (Array.isArray(value)) return value[0] ?? "";
  return value ?? "";
}

async function pickFile() {
  try {
    const selected = await open({
      directory: false,
      multiple: false,
      title: "选择要检查的文件",
    });
    const selectedPath = dialogPath(selected);
    if (selectedPath) path.value = selectedPath;
  } catch (error) {
    ElMessage.error(`选择文件失败：${(error as Error).message}`);
  }
}

async function inspect() {
  const selectedPath = path.value.trim();
  if (!selectedPath) {
    errorMessage.value = "请输入文件路径";
    return;
  }

  loading.value = true;
  errorMessage.value = "";
  result.value = null;
  try {
    const data = await invokeToolByChannel("tool:file-lock:inspect", { path: selectedPath });
    result.value = data as FileLockInspectResponse;
  } catch (error) {
    errorMessage.value = (error as Error).message;
  } finally {
    loading.value = false;
  }
}

async function copyText(value: string, successMessage: string) {
  try {
    await navigator.clipboard.writeText(value);
    ElMessage.success(successMessage);
  } catch (error) {
    ElMessage.error(`复制失败：${(error as Error).message}`);
  }
}

async function revealPath(value: string) {
  try {
    await invokeToolByChannel("tool:system:reveal-in-folder", { path: value });
  } catch (error) {
    ElMessage.error(`打开资源管理器失败：${(error as Error).message}`);
  }
}
</script>

<style scoped>
.file-lock-panel {
  align-content: start;
}

.file-lock-heading,
.file-lock-toolbar,
.file-lock-summary__header,
.file-lock-summary__meta {
  display: flex;
  align-items: center;
  gap: 12px;
}

.file-lock-heading,
.file-lock-summary__header {
  justify-content: space-between;
}

.file-lock-heading h2 {
  margin: 0;
  color: var(--lc-text);
  font-size: 18px;
  line-height: 1.4;
}

.file-lock-heading p {
  margin: 4px 0 0;
  color: var(--lc-text-secondary);
  font-size: 13px;
}

.file-lock-toolbar {
  align-items: stretch;
}

.file-lock-path-input {
  min-width: 0;
  flex: 1;
}

.file-lock-summary {
  padding: 14px 16px;
  border: 1px solid var(--el-border-color);
  border-radius: var(--el-border-radius-base);
  background: var(--el-fill-color-extra-light);
}

.file-lock-label {
  display: block;
  margin-bottom: 5px;
  color: var(--lc-text-secondary);
  font-size: 12px;
}

.file-lock-path,
.file-lock-path-cell,
.file-lock-mono {
  font-family: "Cascadia Code", "Consolas", "Courier New", monospace;
}

.file-lock-path {
  display: block;
  max-width: min(72vw, 820px);
  overflow: hidden;
  color: var(--lc-text);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-lock-summary__actions,
.file-lock-summary__meta {
  flex-shrink: 0;
}

.file-lock-summary__meta {
  justify-content: space-between;
  margin-top: 10px;
  color: var(--lc-text-secondary);
  font-size: 12px;
}

.file-lock-warnings {
  display: grid;
  gap: 8px;
}

.file-lock-table-wrap {
  min-width: 0;
}

.file-lock-path-cell {
  color: var(--lc-text);
}

.file-lock-muted {
  color: var(--lc-text-secondary);
}

.file-lock-empty {
  min-height: 220px;
  display: grid;
  place-items: center;
}

@media (max-width: 720px) {
  .file-lock-toolbar {
    flex-wrap: wrap;
  }

  .file-lock-path-input {
    flex-basis: 100%;
  }

  .file-lock-summary__header {
    align-items: flex-start;
    flex-direction: column;
  }

  .file-lock-path {
    max-width: 100%;
  }
}
</style>

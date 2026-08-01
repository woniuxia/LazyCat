<template>
  <div class="panel-grid file-lock-panel">
    <section class="panel-grid-full file-lock-heading" aria-labelledby="file-lock-title">
      <div>
        <div class="file-lock-kicker">Windows 诊断</div>
        <h2 id="file-lock-title">文件占用</h2>
        <p>定位当前关联目标文件的进程，并快速复制或定位进程文件。</p>
      </div>
      <el-tag type="info" effect="plain">只读诊断</el-tag>
    </section>

    <section class="panel-grid-full file-lock-query" aria-label="文件占用扫描">
      <div class="file-lock-query__field">
        <label class="file-lock-query__label" for="file-lock-path">目标文件</label>
        <el-input
          id="file-lock-path"
          v-model="path"
          class="file-lock-path-input"
          clearable
          placeholder="输入或选择文件路径，例如 D:\\work\\demo\\target\\app.jar"
          @keyup.enter="inspect"
        >
          <template #prefix>
            <el-icon><Document /></el-icon>
          </template>
        </el-input>
        <span class="file-lock-query__hint"
          >结果来自 Windows Restart Manager，可能受权限和系统过滤驱动影响。</span
        >
      </div>
      <div class="file-lock-query__actions">
        <el-button :icon="FolderOpened" :disabled="loading" @click="pickFile">选择文件</el-button>
        <el-button
          type="primary"
          :icon="Search"
          :loading="loading"
          :disabled="!path.trim()"
          @click="inspect"
        >
          扫描占用
        </el-button>
      </div>
    </section>

    <div v-if="errorMessage" class="panel-grid-full">
      <el-alert :title="errorMessage" type="error" show-icon :closable="false" />
    </div>

    <div v-if="loading && !hasCurrentResult" class="panel-grid-full file-lock-loading">
      <el-skeleton :rows="5" animated />
      <span>正在扫描文件关联进程...</span>
    </div>

    <template v-if="hasCurrentResult">
      <section class="panel-grid-full file-lock-summary" aria-labelledby="file-lock-summary-title">
        <div class="file-lock-summary__header">
          <div class="file-lock-summary__path-block">
            <span id="file-lock-summary-title" class="file-lock-label">最近扫描对象</span>
            <el-tooltip :content="result.canonicalPath" placement="top">
              <code class="file-lock-path">{{ result.canonicalPath }}</code>
            </el-tooltip>
          </div>
          <div class="file-lock-summary__actions">
            <el-tag :type="result.processes.length > 0 ? 'warning' : 'success'">
              {{ result.processes.length > 0 ? "发现关联进程" : "未发现关联进程" }}
            </el-tag>
            <el-tooltip content="复制规范化路径" placement="top">
              <el-button
                link
                :icon="CopyDocument"
                aria-label="复制规范化路径"
                @click="copyText(result.canonicalPath, '已复制文件路径')"
              />
            </el-tooltip>
            <el-tooltip content="复制扫描结果" placement="top">
              <el-button link :icon="DocumentCopy" aria-label="复制扫描结果" @click="copyResult" />
            </el-tooltip>
          </div>
        </div>
        <div class="file-lock-summary__meta">
          <span>扫描时间：{{ formatDate(result.scannedAt) }}</span>
          <span v-if="loading" class="file-lock-scan-status">
            <el-icon class="is-loading"><Loading /></el-icon>
            正在重新扫描
          </span>
          <el-button link :icon="Refresh" :loading="loading" @click="inspect">重新扫描</el-button>
        </div>
      </section>

      <section class="panel-grid-full file-lock-metrics" aria-label="扫描摘要">
        <div class="file-lock-metric">
          <span class="file-lock-metric__label">关联进程</span>
          <strong>{{ result.processes.length }}</strong>
          <span class="file-lock-metric__hint">Windows 返回的进程数</span>
        </div>
        <div class="file-lock-metric file-lock-metric--success">
          <span class="file-lock-metric__label">运行中</span>
          <strong>{{ runningProcessCount }}</strong>
          <span class="file-lock-metric__hint">当前状态为运行中</span>
        </div>
        <div class="file-lock-metric file-lock-metric--accent">
          <span class="file-lock-metric__label">可定位</span>
          <strong>{{ processPathCount }}</strong>
          <span class="file-lock-metric__hint">已读取可执行文件路径</span>
        </div>
        <div
          class="file-lock-metric"
          :class="{ 'file-lock-metric--warning': result.warnings.length > 0 }"
        >
          <span class="file-lock-metric__label">扫描提示</span>
          <strong>{{ result.warnings.length }}</strong>
          <span class="file-lock-metric__hint">权限或系统返回的附加信息</span>
        </div>
      </section>

      <section v-if="result.warnings.length > 0" class="panel-grid-full file-lock-warnings">
        <el-alert
          title="结果可能不完整"
          description="部分进程信息无法读取时，Windows 仍会返回其余结果。"
          type="warning"
          show-icon
          :closable="false"
        />
        <ul class="file-lock-warning-list">
          <li v-for="warning in result.warnings" :key="warning">{{ warning }}</li>
        </ul>
      </section>

      <section v-if="result.processes.length === 0" class="panel-grid-full file-lock-no-result">
        <el-alert
          title="Windows 未报告可关联进程"
          description="这不等同于绝对没有占用。受保护进程、权限限制或文件过滤驱动可能导致结果不完整。"
          type="info"
          show-icon
          :closable="false"
        />
      </section>

      <section v-else class="panel-grid-full file-lock-results" v-loading="loading">
        <div class="file-lock-results__header">
          <div>
            <h3>关联进程</h3>
            <span class="file-lock-results__count">
              显示 {{ filteredProcesses.length }} / {{ result.processes.length }}
            </span>
          </div>
          <div class="file-lock-results__controls">
            <el-input
              v-model="processQuery"
              class="file-lock-process-filter"
              size="small"
              clearable
              placeholder="按应用、PID 或路径筛选"
              aria-label="筛选关联进程"
            >
              <template #prefix>
                <el-icon><Search /></el-icon>
              </template>
            </el-input>
            <el-select
              v-model="sortKey"
              class="file-lock-process-sort"
              size="small"
              aria-label="进程排序"
            >
              <el-option label="PID 升序" value="pid-asc" />
              <el-option label="PID 降序" value="pid-desc" />
              <el-option label="应用名称" value="app" />
              <el-option label="状态" value="status" />
            </el-select>
          </div>
        </div>

        <el-table
          :data="filteredProcesses"
          border
          stripe
          row-key="pid"
          max-height="520"
          :empty-text="processQuery.trim() ? '没有匹配的进程' : '没有可显示的进程'"
        >
          <el-table-column label="PID" width="100">
            <template #default="{ row }">
              <code class="file-lock-mono">{{ row.pid }}</code>
            </template>
          </el-table-column>
          <el-table-column label="应用" prop="appName" min-width="160" show-overflow-tooltip />
          <el-table-column label="类型" width="130">
            <template #default="{ row }">
              <span>{{ fileLockAppTypeLabel(row.appType) }}</span>
            </template>
          </el-table-column>
          <el-table-column label="状态" width="110">
            <template #default="{ row }">
              <el-tag size="small" :type="fileLockStatusTagType(row.status)">
                {{ fileLockStatusLabel(row.status) }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column label="可执行文件" min-width="280" show-overflow-tooltip>
            <template #default="{ row }">
              <code v-if="row.executablePath" class="file-lock-path-cell">{{
                row.executablePath
              }}</code>
              <span v-else class="file-lock-muted">无法读取</span>
            </template>
          </el-table-column>
          <el-table-column label="操作" width="158" fixed="right" align="center">
            <template #default="{ row }">
              <el-space :size="2">
                <el-tooltip content="查看详情" placement="top">
                  <el-button
                    link
                    :icon="InfoFilled"
                    :aria-label="`查看 ${row.appName} 详情`"
                    @click="openProcessDetail(row)"
                  />
                </el-tooltip>
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
      </section>
    </template>

    <section
      v-else-if="result && path.trim() && !loading"
      class="panel-grid-full file-lock-path-changed"
    >
      <el-empty description="当前输入路径尚未扫描">
        <el-button type="primary" :icon="Search" @click="inspect">扫描当前路径</el-button>
      </el-empty>
    </section>

    <section v-else-if="!loading" class="panel-grid-full file-lock-empty">
      <el-empty description="选择一个文件后开始检查">
        <template #image>
          <el-icon class="file-lock-empty__icon"><Document /></el-icon>
        </template>
      </el-empty>
    </section>

    <el-drawer v-model="detailVisible" title="进程详情" size="min(460px, 92vw)">
      <template v-if="selectedProcess">
        <div class="file-lock-detail-head">
          <div>
            <span class="file-lock-label">关联进程</span>
            <h3>{{ selectedProcess.appName }}</h3>
          </div>
          <el-tag size="small" :type="fileLockStatusTagType(selectedProcess.status)">
            {{ fileLockStatusLabel(selectedProcess.status) }}
          </el-tag>
        </div>
        <dl class="file-lock-detail-list">
          <div>
            <dt>PID</dt>
            <dd>
              <code class="file-lock-mono">{{ selectedProcess.pid }}</code>
            </dd>
          </div>
          <div>
            <dt>类型</dt>
            <dd>{{ fileLockAppTypeLabel(selectedProcess.appType) }}</dd>
          </div>
          <div>
            <dt>可执行文件</dt>
            <dd>
              <code v-if="selectedProcess.executablePath" class="file-lock-detail-path">
                {{ selectedProcess.executablePath }}
              </code>
              <span v-else class="file-lock-muted">无法读取，可能需要提升权限</span>
            </dd>
          </div>
        </dl>
        <div class="file-lock-detail-actions">
          <el-button
            :icon="CopyDocument"
            @click="copyText(String(selectedProcess.pid), '已复制 PID')"
          >
            复制 PID
          </el-button>
          <el-button
            v-if="selectedProcess.executablePath"
            :icon="Location"
            @click="revealPath(selectedProcess.executablePath)"
          >
            定位文件
          </el-button>
        </div>
      </template>
    </el-drawer>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import {
  CopyDocument,
  Document,
  DocumentCopy,
  FolderOpened,
  InfoFilled,
  Loading,
  Location,
  Refresh,
  Search,
} from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type { FileLockInspectResponse, FileLockProcess } from "../types";
import {
  fileLockAppTypeLabel,
  fileLockPathsMatch,
  fileLockStatusLabel,
  fileLockStatusTagType,
  filterAndSortFileLockProcesses,
  type FileLockSortKey,
} from "../utils/fileLock";

const path = ref("");
const loading = ref(false);
const errorMessage = ref("");
const result = ref<FileLockInspectResponse | null>(null);
const resultPath = ref("");
const processQuery = ref("");
const sortKey = ref<FileLockSortKey>("pid-asc");
const selectedProcess = ref<FileLockProcess | null>(null);
const detailVisible = ref(false);
let requestSequence = 0;

const hasCurrentResult = computed(
  () => result.value !== null && fileLockPathsMatch(path.value, resultPath.value),
);

const filteredProcesses = computed(() => {
  if (!result.value) return [];
  return filterAndSortFileLockProcesses(result.value.processes, processQuery.value, sortKey.value);
});

const runningProcessCount = computed(
  () => result.value?.processes.filter((process) => process.status === "running").length ?? 0,
);

const processPathCount = computed(
  () => result.value?.processes.filter((process) => Boolean(process.executablePath)).length ?? 0,
);

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
    if (selectedPath) {
      path.value = selectedPath;
      await inspect();
    }
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

  const requestId = ++requestSequence;
  loading.value = true;
  errorMessage.value = "";
  try {
    const data = await invokeToolByChannel("tool:file-lock:inspect", { path: selectedPath });
    if (requestId !== requestSequence || !fileLockPathsMatch(path.value, selectedPath)) return;
    result.value = data as FileLockInspectResponse;
    resultPath.value = selectedPath;
  } catch (error) {
    if (requestId !== requestSequence || !fileLockPathsMatch(path.value, selectedPath)) return;
    errorMessage.value = (error as Error).message;
  } finally {
    if (requestId === requestSequence) loading.value = false;
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

async function copyResult() {
  if (!result.value) return;
  const lines = [
    `文件：${result.value.canonicalPath}`,
    `扫描时间：${formatDate(result.value.scannedAt)}`,
    `关联进程：${result.value.processes.length}`,
    ...result.value.processes.map((process) => {
      const executablePath = process.executablePath ?? "无法读取路径";
      return `PID ${process.pid}\t${process.appName}\t${fileLockStatusLabel(process.status)}\t${executablePath}`;
    }),
  ];
  await copyText(lines.join("\n"), "已复制扫描结果");
}

function openProcessDetail(process: FileLockProcess) {
  selectedProcess.value = process;
  detailVisible.value = true;
}

async function revealPath(value: string) {
  try {
    await invokeToolByChannel("tool:system:reveal-in-folder", { path: value });
  } catch (error) {
    ElMessage.error(`打开资源管理器失败：${(error as Error).message}`);
  }
}

watch(path, () => {
  errorMessage.value = "";
});
</script>

<style scoped>
.file-lock-panel {
  align-content: start;
}

.file-lock-heading,
.file-lock-summary__header,
.file-lock-summary__meta,
.file-lock-results__header,
.file-lock-detail-head {
  display: flex;
  align-items: center;
  gap: 12px;
}

.file-lock-heading,
.file-lock-summary__header,
.file-lock-results__header,
.file-lock-detail-head {
  justify-content: space-between;
}

.file-lock-kicker {
  margin-bottom: 5px;
  color: var(--lc-accent);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.file-lock-heading h2 {
  margin: 0;
  color: var(--lc-text);
  font-size: 22px;
  line-height: 1.3;
}

.file-lock-heading p {
  margin: 6px 0 0;
  color: var(--lc-text-secondary);
  font-size: 13px;
}

.file-lock-query {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 14px;
  align-items: end;
  padding: 14px 16px;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-1);
}

.file-lock-query__field {
  min-width: 0;
}

.file-lock-query__label {
  display: block;
  margin-bottom: 6px;
  color: var(--lc-text-secondary);
  font-size: 12px;
  font-weight: 600;
}

.file-lock-query__hint {
  display: block;
  margin-top: 6px;
  color: var(--lc-text-muted);
  font-size: 12px;
  line-height: 1.4;
}

.file-lock-query__actions {
  display: flex;
  gap: 8px;
  align-items: center;
}

.file-lock-path-input {
  min-width: 0;
}

.file-lock-loading {
  display: grid;
  gap: 12px;
  padding: 18px 16px;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-0);
  color: var(--lc-text-secondary);
  font-size: 13px;
}

.file-lock-summary {
  padding: 14px 16px;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  background: var(--el-fill-color-extra-light);
}

.file-lock-summary__path-block {
  min-width: 0;
}

.file-lock-label {
  display: block;
  margin-bottom: 5px;
  color: var(--lc-text-secondary);
  font-size: 12px;
}

.file-lock-path,
.file-lock-path-cell,
.file-lock-mono,
.file-lock-detail-path {
  font-family: var(--lc-font-mono);
}

.file-lock-path {
  display: block;
  max-width: min(72vw, 900px);
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
  justify-content: flex-start;
  margin-top: 10px;
  color: var(--lc-text-secondary);
  font-size: 12px;
}

.file-lock-summary__meta .el-button {
  margin-left: auto;
}

.file-lock-scan-status {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--lc-accent);
}

.file-lock-metrics {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 10px;
}

.file-lock-metric {
  display: grid;
  gap: 4px;
  min-width: 0;
  padding: 12px 14px;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-0);
}

.file-lock-metric--success {
  border-color: rgba(52, 211, 153, 0.35);
  background: rgba(52, 211, 153, 0.05);
}

.file-lock-metric--accent {
  border-color: var(--lc-border-active);
  background: var(--lc-accent-dim);
}

.file-lock-metric--warning {
  border-color: rgba(251, 191, 36, 0.45);
  background: rgba(251, 191, 36, 0.08);
}

.file-lock-metric__label {
  color: var(--lc-text-secondary);
  font-size: 12px;
}

.file-lock-metric strong {
  color: var(--lc-text);
  font-family: var(--lc-font-mono);
  font-size: 24px;
  line-height: 1.1;
}

.file-lock-metric__hint {
  overflow: hidden;
  color: var(--lc-text-muted);
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-lock-warnings {
  display: grid;
  gap: 8px;
}

.file-lock-warning-list {
  margin: 0;
  padding: 0 0 0 28px;
  color: var(--lc-text-secondary);
  font-size: 12px;
  line-height: 1.6;
}

.file-lock-no-result {
  min-width: 0;
}

.file-lock-results {
  min-width: 0;
  overflow: hidden;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md);
  background: var(--lc-surface-0);
}

.file-lock-results__header {
  padding: 12px 14px;
  border-bottom: 1px solid var(--lc-border);
}

.file-lock-results__header h3 {
  margin: 0;
  color: var(--lc-text);
  font-size: 15px;
}

.file-lock-results__count {
  display: block;
  margin-top: 3px;
  color: var(--lc-text-muted);
  font-size: 12px;
}

.file-lock-results__controls {
  display: flex;
  gap: 8px;
  min-width: 0;
}

.file-lock-process-filter {
  width: 240px;
}

.file-lock-process-sort {
  width: 120px;
}

.file-lock-results :deep(.el-table) {
  border: 0;
}

.file-lock-results :deep(.el-table th.el-table__cell:first-child),
.file-lock-results :deep(.el-table td.el-table__cell:first-child) {
  padding-left: 14px;
}

.file-lock-path-cell {
  color: var(--lc-text);
}

.file-lock-muted {
  color: var(--lc-text-secondary);
}

.file-lock-empty,
.file-lock-path-changed {
  min-height: 230px;
  display: grid;
  place-items: center;
}

.file-lock-empty__icon {
  color: var(--lc-accent-light);
  font-size: 42px;
}

.file-lock-detail-head {
  margin-bottom: 22px;
}

.file-lock-detail-head h3 {
  max-width: 280px;
  margin: 0;
  overflow: hidden;
  color: var(--lc-text);
  font-size: 18px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-lock-detail-list {
  display: grid;
  gap: 14px;
  margin: 0;
}

.file-lock-detail-list > div {
  display: grid;
  gap: 5px;
  padding-bottom: 14px;
  border-bottom: 1px solid var(--lc-border);
}

.file-lock-detail-list dt {
  color: var(--lc-text-muted);
  font-size: 12px;
}

.file-lock-detail-list dd {
  min-width: 0;
  margin: 0;
  color: var(--lc-text);
  font-size: 13px;
}

.file-lock-detail-path {
  display: block;
  overflow-wrap: anywhere;
  line-height: 1.5;
}

.file-lock-detail-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 22px;
}

@media (max-width: 900px) {
  .file-lock-metrics {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 720px) {
  .file-lock-heading,
  .file-lock-summary__header,
  .file-lock-results__header {
    align-items: flex-start;
    flex-direction: column;
  }

  .file-lock-heading > .el-tag {
    align-self: flex-start;
  }

  .file-lock-query {
    grid-template-columns: 1fr;
  }

  .file-lock-query__actions {
    justify-content: flex-end;
  }

  .file-lock-path {
    max-width: 100%;
  }

  .file-lock-summary__actions {
    width: 100%;
    justify-content: flex-end;
  }

  .file-lock-summary__meta {
    flex-wrap: wrap;
  }

  .file-lock-summary__meta .el-button {
    margin-left: 0;
  }

  .file-lock-results__controls {
    width: 100%;
  }

  .file-lock-process-filter,
  .file-lock-process-sort {
    width: 100%;
  }

  .file-lock-results__controls {
    flex-direction: column;
  }
}

@media (max-width: 480px) {
  .file-lock-metrics {
    grid-template-columns: 1fr;
  }

  .file-lock-query__actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }

  .file-lock-query__actions .el-button {
    width: 100%;
    margin-left: 0;
  }
}
</style>

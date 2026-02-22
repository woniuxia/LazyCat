<template>
  <div class="pdf-panel">
    <el-tabs v-model="activeTab" type="border-card">
      <!-- Tab 1: Info -->
      <el-tab-pane label="信息" name="info">
        <div class="pdf-section">
          <div class="pdf-file-row">
            <el-button @click="pickInfoFile">选择文件</el-button>
            <el-input v-model="infoPath" placeholder="PDF 文件路径" class="pdf-path-input" />
            <el-button type="primary" :loading="infoLoading" @click="fetchInfo">查看信息</el-button>
          </div>

          <table v-if="pdfInfo" class="pdf-info-table">
            <colgroup>
              <col class="col-label" />
              <col class="col-value" />
              <col class="col-label" />
              <col class="col-value" />
            </colgroup>
            <tbody>
              <tr>
                <td class="info-label">页数</td>
                <td class="info-value">{{ pdfInfo.pages }}</td>
                <td class="info-label">文件大小</td>
                <td class="info-value">{{ formatSize(pdfInfo.fileSize) }}</td>
              </tr>
              <tr>
                <td class="info-label">页面尺寸</td>
                <td class="info-value">
                  {{ pdfInfo.pageWidthMm }} x {{ pdfInfo.pageHeightMm }} mm
                  <span v-if="pdfInfo.paperSize" class="paper-size-tag">（{{ pdfInfo.paperSize }}）</span>
                </td>
                <td class="info-label">PDF 版本</td>
                <td class="info-value">{{ pdfInfo.pdfVersion }}</td>
              </tr>
              <tr>
                <td class="info-label">标题</td>
                <td class="info-value">{{ pdfInfo.title || '-' }}</td>
                <td class="info-label">作者</td>
                <td class="info-value">{{ pdfInfo.author || '-' }}</td>
              </tr>
              <tr>
                <td class="info-label">主题</td>
                <td class="info-value">{{ pdfInfo.subject || '-' }}</td>
                <td class="info-label">关键词</td>
                <td class="info-value">{{ pdfInfo.keywords || '-' }}</td>
              </tr>
              <tr>
                <td class="info-label">创建者</td>
                <td class="info-value">{{ pdfInfo.creator || '-' }}</td>
                <td class="info-label">生成器</td>
                <td class="info-value">{{ pdfInfo.producer || '-' }}</td>
              </tr>
              <tr>
                <td class="info-label">创建日期</td>
                <td class="info-value">{{ pdfInfo.creationDate || '-' }}</td>
                <td class="info-label">修改日期</td>
                <td class="info-value">{{ pdfInfo.modDate || '-' }}</td>
              </tr>
              <tr>
                <td class="info-label">加密</td>
                <td class="info-value" colspan="3">{{ pdfInfo.encrypted ? '是' : '否' }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </el-tab-pane>

      <!-- Tab 2: Split -->
      <el-tab-pane label="拆分" name="split">
        <div class="pdf-section">
          <div class="pdf-file-row">
            <el-button @click="pickSplitFile">选择文件</el-button>
            <el-input v-model="splitPath" placeholder="源 PDF 文件路径" class="pdf-path-input" />
            <el-tag v-if="splitPageCount > 0" type="info" class="page-count-tag">共 {{ splitPageCount }} 页</el-tag>
          </div>
          <el-input
            v-model="splitRanges"
            :placeholder="splitPageCount > 0 ? `页码范围（留空则逐页拆分），如 1-3,5,7-${splitPageCount}` : '页码范围（留空则逐页拆分），如 1-3,5,7-10'"
          />
          <div class="pdf-file-row">
            <el-button @click="pickSplitOutputDir">输出目录</el-button>
            <el-input v-model="splitOutputDir" placeholder="输出文件夹路径" class="pdf-path-input" />
          </div>
          <div class="pdf-actions">
            <el-button type="primary" :loading="splitLoading" @click="doSplit">执行拆分</el-button>
          </div>
          <el-alert
            v-if="splitResultMsg"
            :title="splitResultMsg"
            type="success"
            show-icon
            :closable="false"
          />
          <div v-if="splitResultFiles.length > 0" class="split-file-list">
            <div
              v-for="(f, idx) in splitResultFiles"
              :key="idx"
              class="split-file-item"
            >
              <span class="split-file-index">{{ idx + 1 }}.</span>
              <span class="split-file-name" :title="f.path">{{ f.filename }}</span>
              <el-tag size="small" type="info">{{ f.pages }} 页</el-tag>
            </div>
          </div>
        </div>
      </el-tab-pane>

      <!-- Tab 3: Merge -->
      <el-tab-pane label="合并" name="merge">
        <div class="pdf-section">
          <div class="pdf-actions">
            <el-button @click="addMergeFiles">添加文件</el-button>
            <el-button
              v-if="mergeFiles.length > 0"
              type="danger"
              plain
              @click="mergeFiles = []"
            >清空列表</el-button>
          </div>

          <div v-if="mergeFiles.length > 0" class="merge-file-list">
            <div
              v-for="(f, idx) in mergeFiles"
              :key="idx"
              class="merge-file-item"
            >
              <span class="merge-file-index">{{ idx + 1 }}.</span>
              <span class="merge-file-name" :title="f.path">{{ shortName(f.path) }}</span>
              <el-tag v-if="f.pages > 0" size="small" type="info">{{ f.pages }} 页</el-tag>
              <el-button text size="small" type="danger" @click="removeMergeFile(idx)">移除</el-button>
            </div>
            <div class="merge-total">
              共 {{ mergeFiles.length }} 个文件，{{ mergeTotalPages }} 页
            </div>
          </div>
          <el-empty v-else description="点击上方按钮添加 PDF 文件" :image-size="60" />

          <div class="pdf-file-row">
            <el-button @click="pickMergeOutput">保存到</el-button>
            <el-input v-model="mergeOutputPath" placeholder="合并后的 PDF 文件路径" class="pdf-path-input" />
          </div>
          <div class="pdf-actions">
            <el-button
              type="primary"
              :loading="mergeLoading"
              :disabled="mergeFiles.length < 2"
              @click="doMerge"
            >执行合并</el-button>
          </div>
          <el-alert
            v-if="mergeResult"
            :title="mergeResult"
            type="success"
            show-icon
            :closable="false"
          />
        </div>
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { ElMessage } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import { open, save } from "@tauri-apps/plugin-dialog";

interface SplitResultFile { filename: string; pages: number; path: string }

const activeTab = ref("info");

// --- Info Tab ---
const infoPath = ref("");
const infoLoading = ref(false);
const pdfInfo = ref<{
  pages: number;
  fileSize: number;
  pdfVersion: string;
  encrypted: boolean;
  pageWidthMm: number;
  pageHeightMm: number;
  paperSize: string;
  title: string;
  author: string;
  subject: string;
  keywords: string;
  creator: string;
  producer: string;
  creationDate: string;
  modDate: string;
} | null>(null);

async function pickInfoFile() {
  try {
    const result = await open({
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      multiple: false,
    });
    if (result) {
      infoPath.value = typeof result === "string" ? result : result.path;
    }
  } catch { /* dialog cancelled */ }
}

async function fetchInfo() {
  if (!infoPath.value.trim()) {
    ElMessage.warning("请选择或输入 PDF 文件路径");
    return;
  }
  infoLoading.value = true;
  pdfInfo.value = null;
  try {
    const data = await invokeToolByChannel("tool:pdf:info", {
      path: infoPath.value,
    });
    pdfInfo.value = data as typeof pdfInfo.value;
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    infoLoading.value = false;
  }
}

// --- Split Tab ---
const splitPath = ref("");
const splitRanges = ref("");
const splitOutputDir = ref("");
const splitLoading = ref(false);
const splitResultMsg = ref("");
const splitResultFiles = ref<SplitResultFile[]>([]);
const splitPageCount = ref(0);

async function fetchPageCount(filePath: string): Promise<number> {
  try {
    const data = await invokeToolByChannel("tool:pdf:info", { path: filePath }) as { pages: number };
    return data.pages;
  } catch {
    return 0;
  }
}

async function pickSplitFile() {
  try {
    const result = await open({
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      multiple: false,
    });
    if (result) {
      const p = typeof result === "string" ? result : result.path;
      splitPath.value = p;
      splitPageCount.value = await fetchPageCount(p);
      // Auto-generate default output directory (same folder as source file)
      const sep = p.includes("\\") ? "\\" : "/";
      const lastSep = p.lastIndexOf(sep);
      splitOutputDir.value = lastSep > 0 ? p.substring(0, lastSep) : "";
    }
  } catch { /* dialog cancelled */ }
}

async function pickSplitOutputDir() {
  try {
    const result = await open({
      directory: true,
      title: "选择输出文件夹",
      defaultPath: splitOutputDir.value || undefined,
    });
    if (result) {
      splitOutputDir.value = typeof result === "string" ? result : result.path;
    }
  } catch { /* dialog cancelled */ }
}

async function doSplit() {
  if (!splitPath.value.trim()) {
    ElMessage.warning("请选择源 PDF 文件");
    return;
  }
  if (!splitOutputDir.value.trim()) {
    ElMessage.warning("请选择输出文件夹");
    return;
  }
  splitLoading.value = true;
  splitResultMsg.value = "";
  splitResultFiles.value = [];
  try {
    const data = (await invokeToolByChannel("tool:pdf:split", {
      path: splitPath.value,
      ranges: splitRanges.value,
      outputDir: splitOutputDir.value,
    })) as { files: SplitResultFile[]; totalFiles: number };
    splitResultMsg.value = `拆分完成: 生成 ${data.totalFiles} 个文件`;
    splitResultFiles.value = data.files;
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    splitLoading.value = false;
  }
}

// --- Merge Tab ---
interface MergeFileEntry { path: string; pages: number }
const mergeFiles = ref<MergeFileEntry[]>([]);
const mergeOutputPath = ref("");
const mergeLoading = ref(false);
const mergeResult = ref("");

const mergeTotalPages = computed(() => mergeFiles.value.reduce((sum, f) => sum + f.pages, 0));

async function addMergeFiles() {
  try {
    const result = await open({
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      multiple: true,
    });
    if (result) {
      const paths = Array.isArray(result)
        ? result.map((f) => (typeof f === "string" ? f : f.path))
        : [typeof result === "string" ? result : result.path];
      for (const p of paths) {
        const pages = await fetchPageCount(p);
        mergeFiles.value.push({ path: p, pages });
      }
    }
  } catch { /* dialog cancelled */ }
}

function removeMergeFile(idx: number) {
  mergeFiles.value.splice(idx, 1);
}

async function pickMergeOutput() {
  try {
    const result = await save({
      title: "保存合并后的 PDF",
      defaultPath: mergeOutputPath.value || undefined,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (result) {
      mergeOutputPath.value = result;
    }
  } catch { /* dialog cancelled */ }
}

async function doMerge() {
  if (mergeFiles.value.length < 2) {
    ElMessage.warning("至少需要 2 个 PDF 文件");
    return;
  }
  if (!mergeOutputPath.value.trim()) {
    ElMessage.warning("请选择输出路径");
    return;
  }
  mergeLoading.value = true;
  mergeResult.value = "";
  try {
    const data = (await invokeToolByChannel("tool:pdf:merge", {
      paths: mergeFiles.value.map((f) => f.path),
      outputPath: mergeOutputPath.value,
    })) as { pages: number; outputPath: string; sources: number };
    mergeResult.value = `合并完成: ${data.sources} 个文件，共 ${data.pages} 页，保存至 ${data.outputPath}`;
  } catch (error) {
    ElMessage.error((error as Error).message);
  } finally {
    mergeLoading.value = false;
  }
}

// --- Helpers ---
function formatSize(bytes: number): string {
  if (bytes < 1024) return bytes + " B";
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
  return (bytes / (1024 * 1024)).toFixed(2) + " MB";
}

function shortName(path: string): string {
  const sep = path.includes("\\") ? "\\" : "/";
  const parts = path.split(sep);
  return parts[parts.length - 1] || path;
}
</script>

<style scoped>
.pdf-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.pdf-section {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.pdf-file-row {
  display: flex;
  gap: 8px;
  align-items: center;
}

.pdf-path-input {
  flex: 1;
}

.pdf-actions {
  display: flex;
  gap: 8px;
  align-items: center;
}

.pdf-info-table {
  width: 100%;
  table-layout: fixed;
  border-collapse: collapse;
  margin-top: 4px;
  border: 1px solid var(--lc-border, #dcdfe6);
  font-size: 13px;
}

.pdf-info-table .col-label {
  width: 80px;
}

.pdf-info-table .col-value {
  width: calc(50% - 80px);
}

.pdf-info-table td {
  padding: 8px 12px;
  border: 1px solid var(--lc-border, #dcdfe6);
  line-height: 1.5;
  word-break: break-word;
}

.pdf-info-table .info-label {
  font-weight: 600;
  color: var(--lc-accent, #409eff);
  background: var(--lc-surface-1, #f5f7fa);
  white-space: nowrap;
}

.pdf-info-table .info-value {
  color: var(--lc-text, #303133);
}

.merge-file-list {
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md, 6px);
  padding: 8px;
  background: var(--lc-surface-1);
  max-height: 240px;
  overflow-y: auto;
}

.merge-file-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
  border-bottom: 1px solid var(--lc-border);
}

.merge-file-item:last-child {
  border-bottom: none;
}

.merge-file-index {
  color: var(--lc-text-muted);
  min-width: 24px;
  text-align: right;
  font-size: 13px;
}

.merge-file-name {
  flex: 1;
  font-size: 13px;
  color: var(--lc-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.merge-total {
  padding: 6px 0 2px;
  font-size: 13px;
  color: var(--lc-text-muted);
  text-align: right;
}

.page-count-tag {
  flex-shrink: 0;
}

.paper-size-tag {
  color: var(--lc-accent);
  font-weight: 500;
}

.split-file-list {
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-md, 6px);
  padding: 8px;
  background: var(--lc-surface-1);
  max-height: 240px;
  overflow-y: auto;
}

.split-file-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
  border-bottom: 1px solid var(--lc-border);
}

.split-file-item:last-child {
  border-bottom: none;
}

.split-file-index {
  color: var(--lc-text-muted);
  min-width: 24px;
  text-align: right;
  font-size: 13px;
}

.split-file-name {
  flex: 1;
  font-size: 13px;
  color: var(--lc-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>

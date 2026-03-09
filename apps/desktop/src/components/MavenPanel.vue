<template>
  <div class="panel-grid">
    <!-- 仓库路径 -->
    <div class="panel-grid-full maven-repo-row">
      <label class="maven-label">仓库路径</label>
      <div class="maven-repo-input">
        <el-input v-model="repoPath" placeholder="C:\Users\xxx\.m2\repository" clearable />
        <el-button @click="pickRepoDir">浏览</el-button>
      </div>
    </div>

    <!-- 依赖输入 -->
    <div class="panel-grid-full">
      <label class="maven-label">依赖 XML</label>
      <el-input
        v-model="xmlInput"
        type="textarea"
        :rows="4"
        placeholder="粘贴 Maven 依赖 XML，例如：
<dependency>
  <groupId>org.bouncycastle</groupId>
  <artifactId>bcprov-jdk15on</artifactId>
  <version>1.63</version>
</dependency>"
      />
    </div>

    <!-- 操作按钮 -->
    <div class="panel-grid-full maven-actions">
      <el-button type="primary" :loading="loading" @click="doLocate">定位</el-button>
    </div>

    <!-- 解析结果标签 -->
    <div v-if="parsed" class="panel-grid-full maven-parsed">
      <el-tag>groupId: {{ parsed.groupId }}</el-tag>
      <el-tag type="success">artifactId: {{ parsed.artifactId }}</el-tag>
      <el-tag type="warning">version: {{ parsed.version }}</el-tag>
    </div>

    <!-- 错误提示 -->
    <div v-if="error" class="panel-grid-full">
      <el-alert :title="error" type="error" :closable="false" />
    </div>

    <!-- 定位结果 -->
    <div v-if="result" class="panel-grid-full maven-result">
      <el-alert
        :title="result.found ? 'Jar 文件已找到' : 'Jar 文件未找到'"
        :type="result.found ? 'success' : 'warning'"
        :closable="false"
        show-icon
      />

      <div class="maven-jar-path">
        <label class="maven-label">Jar 路径</label>
        <div class="maven-path-row">
          <code class="maven-code">{{ result.targetJarPath }}</code>
          <el-button size="small" @click="copyText(result.targetJarPath)">复制</el-button>
          <el-button size="small" @click="openPath(result.targetJarPath)">打开目录</el-button>
          <el-button size="small" @click="openPath(result.artifactDir)">打开父目录</el-button>
        </div>
      </div>

      <div v-if="result.found" class="maven-jar-size">
        <label class="maven-label">文件大小</label>
        <span>{{ formatSize(result.jarSize) }}</span>
      </div>

      <!-- 版本列表 -->
      <div v-if="result.versions.length > 0" class="maven-versions">
        <label class="maven-label">本地版本 ({{ result.totalVersions }})</label>
        <el-table
          :data="result.versions"
          size="small"
          :row-class-name="versionRowClass"
          max-height="320"
        >
          <el-table-column prop="version" label="版本号" min-width="140" />
          <el-table-column label="Jar 状态" width="100" align="center">
            <template #default="{ row }">
              <el-tag :type="row.hasJar ? 'success' : 'info'" size="small">
                {{ row.hasJar ? "存在" : "缺失" }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column label="文件大小" width="120" align="right">
            <template #default="{ row }">
              {{ row.hasJar ? formatSize(row.jarSize) : "-" }}
            </template>
          </el-table-column>
          <el-table-column label="操作" width="90" align="center">
            <template #default="{ row }">
              <el-button v-if="row.hasJar" link size="small" @click="openVersionDir(row.version)">
                打开
              </el-button>
            </template>
          </el-table-column>
        </el-table>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invokeToolByChannel } from "../bridge/tauri";
import { getSetting, setSetting } from "../composables/useSettings";
import { open } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";

interface VersionInfo {
  version: string;
  hasJar: boolean;
  jarSize: number;
}

interface LocateResult {
  groupId: string;
  artifactId: string;
  version: string;
  artifactDir: string;
  targetJarPath: string;
  found: boolean;
  jarSize: number;
  versions: VersionInfo[];
  totalVersions: number;
}

const repoPath = ref("");
const xmlInput = ref("");
const loading = ref(false);
const error = ref("");
const parsed = ref<{ groupId: string; artifactId: string; version: string } | null>(null);
const result = ref<LocateResult | null>(null);

onMounted(() => {
  const saved = getSetting("maven.repoPath");
  if (saved) {
    repoPath.value = saved;
  }
});

function toDialogPath(r: string | { path: string } | null): string {
  if (!r) return "";
  return typeof r === "string" ? r : r.path;
}

async function pickRepoDir() {
  try {
    const selected = await open({ directory: true, multiple: false });
    const p = toDialogPath(selected as string | { path: string } | null);
    if (p) {
      repoPath.value = p;
      setSetting("maven.repoPath", p);
    }
  } catch {
    // dialog cancelled
  }
}

function parseXml(xml: string): { groupId: string; artifactId: string; version: string } | null {
  const extract = (tag: string): string => {
    const re = new RegExp(`<${tag}>\\s*([^<]+?)\\s*</${tag}>`);
    const m = xml.match(re);
    return m ? m[1] : "";
  };
  const groupId = extract("groupId");
  const artifactId = extract("artifactId");
  const version = extract("version");
  if (!groupId || !artifactId || !version) return null;
  return { groupId, artifactId, version };
}

async function doLocate() {
  error.value = "";
  result.value = null;
  parsed.value = null;

  if (!repoPath.value.trim()) {
    error.value = "请先设置仓库路径";
    return;
  }

  const p = parseXml(xmlInput.value);
  if (!p) {
    error.value = "无法解析 XML，请确保包含 groupId、artifactId、version 标签";
    return;
  }
  parsed.value = p;

  // Persist repo path
  setSetting("maven.repoPath", repoPath.value.trim());

  loading.value = true;
  try {
    const data = (await invokeToolByChannel("tool:maven:locate", {
      groupId: p.groupId,
      artifactId: p.artifactId,
      version: p.version,
      repoPath: repoPath.value.trim(),
    })) as LocateResult;
    result.value = data;
  } catch (e) {
    error.value = (e as Error).message;
  } finally {
    loading.value = false;
  }
}

async function openPath(path: string) {
  try {
    await invokeToolByChannel("tool:maven:open-path", { path });
  } catch (e) {
    ElMessage.error(`打开目录失败：${(e as Error).message}`);
  }
}

function openVersionDir(version: string) {
  if (!result.value) return;
  const dir = `${result.value.artifactDir}/${version}`;
  openPath(dir);
}

function formatSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

function copyText(text: string) {
  navigator.clipboard.writeText(text).catch(() => {});
}

function versionRowClass({ row }: { row: VersionInfo }): string {
  return row.version === parsed.value?.version ? "maven-current-version" : "";
}
</script>

<style scoped>
.maven-repo-row {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.maven-repo-input {
  display: flex;
  gap: 8px;
}

.maven-repo-input .el-input {
  flex: 1;
}

.maven-label {
  font-size: 13px;
  font-weight: 600;
  color: var(--lc-text-secondary);
}

.maven-actions {
  display: flex;
  gap: 8px;
}

.maven-parsed {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.maven-result {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.maven-jar-path {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.maven-path-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.maven-code {
  flex: 1;
  font-family: var(--lc-font-mono);
  font-size: 13px;
  padding: 6px 10px;
  background: var(--lc-surface-1);
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-sm);
  word-break: break-all;
}

.maven-jar-size {
  display: flex;
  gap: 8px;
  align-items: center;
}

.maven-versions {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

:deep(.maven-current-version) {
  background-color: var(--el-color-primary-light-9) !important;
}
</style>

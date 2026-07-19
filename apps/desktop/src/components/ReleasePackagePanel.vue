<template>
  <section class="release-package-panel">
    <div class="release-package-toolbar">
      <span class="toolbar-label">归档根目录</span>
      <el-input v-model="outputRoot" class="release-package-root" placeholder="选择所有上线包的归档根目录" readonly :disabled="running" />
      <el-button :icon="FolderOpened" :disabled="running" @click="chooseOutputRoot">选择目录</el-button>
      <el-button :icon="Refresh" :disabled="running || loading" @click="loadProjects">刷新</el-button>
    </div>

    <div class="release-package-workspace">
      <aside class="release-package-projects" aria-label="项目列表">
        <div class="projects-heading">
          <strong>项目配置</strong>
          <el-button :icon="Plus" size="small" text :disabled="running" @click="newProject">新建</el-button>
        </div>
        <div v-if="projects.length === 0" class="projects-empty">暂无项目配置</div>
        <button
          v-for="project in projects"
          :key="project.id"
          type="button"
          class="project-item"
          :class="{ active: project.id === selectedId }"
          :disabled="running"
          @click="selectProject(project)"
        >
          <span class="project-name">{{ project.name }}</span>
          <span class="project-updated">{{ project.updatedAt || "未保存" }}</span>
        </button>
      </aside>

      <main class="release-package-editor">
        <div class="editor-header">
          <div>
            <h2>{{ selectedProject ? selectedProject.name : "新建上线包项目" }}</h2>
            <span class="editor-hint">构建前端与后端工程，并将产物归档到新目录</span>
          </div>
          <div class="editor-actions">
            <el-button v-if="selectedProject" :icon="Delete" type="danger" text :disabled="running || saving" @click="deleteProject">
              删除配置
            </el-button>
            <el-button :icon="DocumentChecked" :loading="saving" :disabled="running" @click="saveProject">保存配置</el-button>
            <el-button :icon="VideoPlay" type="primary" :disabled="running || !selectedProject || dirty" @click="prepareStart">开始打包</el-button>
            <el-button v-if="running" :icon="VideoPause" type="danger" @click="cancelRun">终止打包</el-button>
            <el-button v-else-if="status === 'succeeded' && archivePath" :icon="FolderOpened" @click="openArchive">打开归档目录</el-button>
          </div>
        </div>

        <el-form label-position="top" class="release-package-form">
          <div class="project-basics">
            <el-form-item label="项目名称" required>
              <el-input v-model="draft.name" :disabled="running" placeholder="例如：订单管理系统" />
            </el-form-item>
          </div>

          <div class="engineering-grid">
            <section class="engineering-card frontend-card">
              <header class="engineering-card-header">
                <div>
                  <span class="engineering-kicker">FRONTEND</span>
                  <h3>前端工程</h3>
                </div>
                <span class="engineering-index">01</span>
              </header>

              <el-form-item label="工程目录" required>
                <el-input v-model="draft.frontendProjectPath" :disabled="running" placeholder="前端工程绝对路径">
                  <template #append><el-button :icon="FolderOpened" :disabled="running" @click="chooseFrontendProject">选择</el-button></template>
                </el-input>
              </el-form-item>
              <el-form-item required>
                <template #label>
                  <div class="command-label-row">
                    <span>构建命令</span>
                    <el-popover
                      placement="bottom-start"
                      trigger="click"
                      :width="440"
                      :teleported="true"
                      popper-class="release-package-command-examples"
                    >
                      <template #reference>
                        <el-button type="primary" text size="small">常用示例</el-button>
                      </template>
                      <div class="command-example-list">
                        <article v-for="example in RELEASE_PACKAGE_COMMAND_EXAMPLES" :key="example.id" class="command-example-item">
                          <div class="command-example-heading">
                            <strong>{{ example.title }}</strong>
                            <el-button
                              :icon="CopyDocument"
                              :aria-label="`复制${example.title}命令`"
                              size="small"
                              @click="copyCommandExample(example.command)"
                            >
                              复制
                            </el-button>
                          </div>
                          <p>{{ example.description }}</p>
                          <pre>{{ example.command }}</pre>
                        </article>
                      </div>
                    </el-popover>
                  </div>
                </template>
                <el-input
                  v-model="draft.frontendBuildCommand"
                  class="command-input"
                  type="textarea"
                  :autosize="{ minRows: 4, maxRows: 9 }"
                  :disabled="running"
                  placeholder="例如：pnpm build"
                />
                <p class="command-hint">多行命令将在同一 PowerShell 会话中顺序执行，前面设置的环境变量可在后续命令中复用。</p>
              </el-form-item>
              <div class="artifact-grid">
                <el-form-item label="产物路径" required>
                  <el-input v-model="draft.frontendArtifactPath" :disabled="running" placeholder="相对工程目录，可为文件或目录" />
                </el-form-item>
                <el-form-item label="产物处理方式" required>
                  <el-select v-model="draft.frontendArtifactMode" :disabled="running" class="full-width">
                    <el-option label="直接复制目录" value="copy_directory" />
                    <el-option label="压缩为 ZIP" value="zip_directory" />
                  </el-select>
                </el-form-item>
              </div>
            </section>

            <section class="engineering-card backend-card">
              <header class="engineering-card-header">
                <div>
                  <span class="engineering-kicker">BACKEND</span>
                  <h3>后端工程</h3>
                </div>
                <span class="engineering-index">02</span>
              </header>

              <el-form-item label="工程目录" required>
                <el-input v-model="draft.backendProjectPath" :disabled="running" placeholder="后端工程绝对路径">
                  <template #append><el-button :icon="FolderOpened" :disabled="running" @click="chooseBackendProject">选择</el-button></template>
                </el-input>
              </el-form-item>
              <el-form-item required>
                <template #label>
                  <div class="command-label-row">
                    <span>构建命令</span>
                    <el-popover
                      placement="bottom-start"
                      trigger="click"
                      :width="440"
                      :teleported="true"
                      popper-class="release-package-command-examples"
                    >
                      <template #reference>
                        <el-button type="primary" text size="small">常用示例</el-button>
                      </template>
                      <div class="command-example-list">
                        <article v-for="example in RELEASE_PACKAGE_COMMAND_EXAMPLES" :key="example.id" class="command-example-item">
                          <div class="command-example-heading">
                            <strong>{{ example.title }}</strong>
                            <el-button
                              :icon="CopyDocument"
                              :aria-label="`复制${example.title}命令`"
                              size="small"
                              @click="copyCommandExample(example.command)"
                            >
                              复制
                            </el-button>
                          </div>
                          <p>{{ example.description }}</p>
                          <pre>{{ example.command }}</pre>
                        </article>
                      </div>
                    </el-popover>
                  </div>
                </template>
                <el-input
                  v-model="draft.backendBuildCommand"
                  class="command-input"
                  type="textarea"
                  :autosize="{ minRows: 4, maxRows: 9 }"
                  :disabled="running"
                  placeholder="例如：mvn clean package"
                />
                <p class="command-hint">
                  多行命令将在同一 PowerShell 会话中顺序执行，环境变量可复用；关键外部工具失败后请检查 $LASTEXITCODE。
                </p>
              </el-form-item>
              <el-form-item label="产物路径" required>
                <el-input v-model="draft.backendArtifactPath" :disabled="running" placeholder="相对工程目录，可为文件或目录" />
              </el-form-item>
            </section>
          </div>
        </el-form>
      </main>
    </div>

    <section class="release-package-log-card">
      <header class="log-card-header">
        <div>
          <h3>运行日志</h3>
          <p>按执行顺序记录构建、归档及异常输出。</p>
        </div>
        <el-tag class="log-status" :type="statusTagTypes[status]" effect="plain" size="small">
          {{ statusLabels[status] }}
        </el-tag>
      </header>
      <div ref="logContainer" class="release-package-log" aria-live="polite" aria-label="打包日志">
        <div v-if="logs.length === 0" class="log-empty">暂无运行日志</div>
        <div v-for="(entry, index) in logs" :key="`${entry.runId}-${index}`" class="log-line" :class="{ stderr: entry.stream === 'stderr' }">
          <span class="log-meta">[{{ entry.phase }}] [{{ entry.stream }}]</span>
          <span>{{ entry.line }}</span>
        </div>
      </div>
    </section>

    <el-dialog v-model="confirmVisible" title="确认打包" width="min(560px, calc(100vw - 32px))" :close-on-click-modal="false">
      <el-form label-position="top">
        <el-form-item label="归档目录名" required>
          <el-input v-model="folderName" placeholder="例如：20260723-订单管理系统" />
        </el-form-item>
      </el-form>
      <p class="archive-preview">完整归档路径：{{ archivePathPreview || "请先设置归档根目录" }}</p>
      <template #footer>
        <el-button v-if="starting" type="danger" :disabled="cancelPendingStart" @click="cancelRun">
          {{ cancelPendingStart ? "等待终止" : "终止打包" }}
        </el-button>
        <el-button v-else @click="confirmVisible = false">取消</el-button>
        <el-button type="primary" :loading="starting" :disabled="starting" @click="confirmStart">确认打包</el-button>
      </template>
    </el-dialog>
  </section>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, reactive, ref, watch } from "vue";
import { CopyDocument, Delete, DocumentChecked, FolderOpened, Plus, Refresh, VideoPause, VideoPlay } from "@element-plus/icons-vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { open } from "@tauri-apps/plugin-dialog";
import { invokeToolByChannel } from "../bridge/tauri";
import { getSetting, initSettings, setSettingAndWait } from "../composables/useSettings";
import { useReleasePackageRuntime } from "../composables/useReleasePackageRuntime";
import type {
  ReleasePackagePrepareResult,
  ReleasePackageProject,
  ReleasePackageProjectDraft,
  ReleasePackageRunStatus,
  ReleasePackageStartResult,
} from "../types/release-package";
import {
  RELEASE_PACKAGE_COMMAND_EXAMPLES,
  createEmptyReleasePackageDraft,
  isReleasePackageDraftDirty,
  projectToReleasePackageDraft,
  validateReleasePackageDraft,
} from "../utils/releasePackage";

const projects = ref<ReleasePackageProject[]>([]);
const selectedId = ref<number | null>(null);
const draft = reactive<ReleasePackageProjectDraft>(createEmptyReleasePackageDraft());
const outputRoot = ref("");
const loading = ref(false);
const saving = ref(false);
const starting = ref(false);
const cancelPendingStart = ref(false);
const confirmVisible = ref(false);
const prepareResult = ref<ReleasePackagePrepareResult | null>(null);
const folderName = ref("");
const logContainer = ref<HTMLElement | null>(null);
const runtime = useReleasePackageRuntime();
const logs = runtime.logs;
const status = runtime.status;
const archivePath = runtime.archivePath;
const statusLabels: Record<ReleasePackageRunStatus, string> = {
  idle: "未运行",
  running: "运行中",
  succeeded: "已完成",
  failed: "失败",
  cancelled: "已终止",
};
const statusTagTypes: Record<ReleasePackageRunStatus, "primary" | "success" | "info" | "warning" | "danger"> = {
  idle: "info",
  running: "primary",
  succeeded: "success",
  failed: "danger",
  cancelled: "warning",
};

const selectedProject = computed(() => projects.value.find((item) => item.id === selectedId.value) ?? null);
const dirty = computed(() => isReleasePackageDraftDirty(selectedProject.value, draft));
const running = computed(() => runtime.status.value === "running");
const archivePathPreview = computed(() => {
  const preparedRoot = prepareResult.value?.outputRoot;
  if (!preparedRoot || !folderName.value) return "";
  if (folderName.value === prepareResult.value?.defaultFolderName) {
    return prepareResult.value.archivePath;
  }
  return `${preparedRoot.replace(/[\\/]+$/, "")}/${folderName.value}`;
});

function showError(error: unknown): void {
  ElMessage.error(error instanceof Error ? error.message : String(error));
}

async function copyCommandExample(command: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(command);
    ElMessage.success("命令示例已复制");
  } catch (error) {
    showError(error);
  }
}

async function loadProjects(): Promise<boolean> {
  loading.value = true;
  try {
    const result = (await invokeToolByChannel("tool:release-package:project-list", {})) as { projects?: ReleasePackageProject[] };
    projects.value = result.projects ?? [];
    const current = projects.value.find((project) => project.id === selectedId.value);
    const active = projects.value.find((project) => project.id === runtime.activeProjectId.value);
    const preferActiveProject = (selectedId.value === null && !dirty.value) || runtime.status.value === "running";
    const preserveUnsavedDraft = selectedId.value === null && dirty.value && !preferActiveProject;
    const target = preferActiveProject ? active ?? current ?? projects.value[0] : current;
    if (target) {
      const selectionChanged = selectedId.value !== target.id;
      selectedId.value = target.id;
      if (selectionChanged || !dirty.value) Object.assign(draft, projectToReleasePackageDraft(target));
    } else if (!preserveUnsavedDraft) {
      selectedId.value = null;
      Object.assign(draft, createEmptyReleasePackageDraft());
    }
    return true;
  } catch (error) {
    showError(error);
    return false;
  } finally {
    loading.value = false;
  }
}

async function confirmDiscardChanges(): Promise<boolean> {
  if (!dirty.value) return true;
  try {
    await ElMessageBox.confirm("当前有未保存的修改，直接切换将丢失这些修改。", "未保存的修改", { type: "warning" });
    return true;
  } catch {
    return false;
  }
}

async function selectProject(project: ReleasePackageProject): Promise<void> {
  if (project.id === selectedId.value || !(await confirmDiscardChanges())) return;
  selectedId.value = project.id;
  Object.assign(draft, projectToReleasePackageDraft(project));
}

async function newProject(): Promise<void> {
  if (!(await confirmDiscardChanges())) return;
  selectedId.value = null;
  Object.assign(draft, createEmptyReleasePackageDraft());
}

async function saveProject(): Promise<void> {
  const validationError = validateReleasePackageDraft(draft);
  if (validationError) {
    ElMessage.warning(validationError);
    return;
  }
  saving.value = true;
  try {
    const payload = { ...draft };
    const channel = selectedId.value ? "tool:release-package:project-update" : "tool:release-package:project-create";
    const result = (await invokeToolByChannel(channel, selectedId.value ? { id: selectedId.value, ...payload } : payload)) as { id?: number };
    const savedId = result.id ?? selectedId.value;
    if (savedId) selectedId.value = savedId;
    const refreshed = await loadProjects();
    if (!refreshed) return;
    if (savedId) {
      selectedId.value = savedId;
      const saved = projects.value.find((project) => project.id === savedId);
      if (saved) Object.assign(draft, projectToReleasePackageDraft(saved));
    }
    ElMessage.success("项目配置已保存");
  } catch (error) {
    showError(error);
  } finally {
    saving.value = false;
  }
}

async function deleteProject(): Promise<void> {
  const project = selectedProject.value;
  if (!project) return;
  try {
    await ElMessageBox.confirm(
      `确定删除「${project.name}」的配置吗？只删除配置，不删除工程或归档文件。`,
      "删除项目配置",
      { type: "warning" },
    );
    await invokeToolByChannel("tool:release-package:project-delete", { id: project.id });
    projects.value = projects.value.filter((item) => item.id !== project.id);
    selectedId.value = null;
    Object.assign(draft, createEmptyReleasePackageDraft());
    const refreshed = await loadProjects();
    if (!refreshed) return;
    ElMessage.success("项目配置已删除");
  } catch (error) {
    if (error !== "cancel" && error !== "close") showError(error);
  }
}

async function chooseDirectory(): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false });
  return typeof selected === "string" ? selected : null;
}

async function chooseOutputRoot(): Promise<void> {
  try {
    const path = await chooseDirectory();
    if (!path) return;
    await setSettingAndWait("release_package.output_root", path);
    outputRoot.value = path;
    ElMessage.success("归档根目录已保存");
  } catch (error) {
    showError(error);
  }
}

async function chooseFrontendProject(): Promise<void> {
  try {
    const path = await chooseDirectory();
    if (path) draft.frontendProjectPath = path;
  } catch (error) {
    showError(error);
  }
}

async function chooseBackendProject(): Promise<void> {
  try {
    const path = await chooseDirectory();
    if (path) draft.backendProjectPath = path;
  } catch (error) {
    showError(error);
  }
}

async function prepareStart(): Promise<void> {
  if (!selectedProject.value || dirty.value) {
    ElMessage.warning(dirty.value ? "请先保存项目配置" : "请先选择项目");
    return;
  }
  if (running.value) return;
  try {
    prepareResult.value = (await invokeToolByChannel("tool:release-package:prepare", {
      projectId: selectedProject.value.id,
    })) as ReleasePackagePrepareResult;
    folderName.value = prepareResult.value.defaultFolderName;
    confirmVisible.value = true;
  } catch (error) {
    showError(error);
  }
}

async function confirmStart(): Promise<void> {
  const projectId = selectedProject.value?.id;
  const folderNameError = validateArchiveFolderName(folderName.value);
  if (!projectId || folderNameError) {
    ElMessage.warning(folderNameError ?? "请先选择项目");
    return;
  }
  starting.value = true;
  cancelPendingStart.value = false;
  try {
    await runtime.ensureListeners();
    if (cancelPendingStart.value) {
      confirmVisible.value = false;
      ElMessage.info("已取消打包");
      return;
    }
    runtime.beginStart(projectId);
    const result = (await invokeToolByChannel("tool:release-package:start", {
      projectId,
      folderName: folderName.value,
    })) as ReleasePackageStartResult;
    runtime.bindStartedRun(result.runId, projectId);
    confirmVisible.value = false;
    if (cancelPendingStart.value) {
      try {
        await runtime.cancel();
        cancelPendingStart.value = false;
        ElMessage.info("已请求终止打包");
      } catch (error) {
        showError(error);
      }
    }
  } catch (error) {
    runtime.abortStart(error instanceof Error ? error.message : String(error));
    showError(error);
  } finally {
    starting.value = false;
    cancelPendingStart.value = false;
  }
}

async function cancelRun(): Promise<void> {
  if (starting.value && !runtime.activeRunId.value) {
    cancelPendingStart.value = true;
    ElMessage.info("启动完成后将立即终止打包");
    return;
  }
  try {
    await runtime.cancel();
    cancelPendingStart.value = false;
    ElMessage.info("已请求终止打包");
  } catch (error) {
    showError(error);
  }
}

function validateArchiveFolderName(value: string): string | null {
  if (!value.trim()) return "请输入归档目录名";
  if (value !== value.trim()) return "归档目录名首尾不能包含空格";
  if (value === "." || value === "..") return "归档目录名不能为 . 或 ..";
  if (value.length > 255) return "归档目录名不能超过 255 个字符";
  if (/[<>:\"/\\|?*\u0000-\u001f]/.test(value)) return "归档目录名包含 Windows 非法字符";
  if (/[. ]$/.test(value)) return "归档目录名不能以点或空格结尾";
  if (/^(?:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\..*)?$/i.test(value)) {
    return "归档目录名不能使用 Windows 保留设备名";
  }
  return null;
}

async function openArchive(): Promise<void> {
  if (!archivePath.value) return;
  try {
    await invokeToolByChannel("tool:system:open-local-path", { path: archivePath.value });
  } catch (error) {
    showError(error);
  }
}

watch(
  () => runtime.logs.value.length,
  async () => {
    await nextTick();
    const element = logContainer.value;
    if (!element) return;
    const nearBottom = element.scrollHeight - element.scrollTop - element.clientHeight < 48;
    if (nearBottom) element.scrollTop = element.scrollHeight;
  },
);

onMounted(async () => {
  try {
    await initSettings();
    outputRoot.value = getSetting("release_package.output_root") ?? "";
    await runtime.ensureListeners();
    await loadProjects();
  } catch (error) {
    showError(error);
  }
});
</script>

<style scoped>
.release-package-panel {
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-height: 0;
}
.release-package-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 14px;
  border: 1px solid #e4e7ed;
  border-radius: 10px;
  background: #fff;
  box-shadow: 0 2px 10px rgb(31 45 61 / 4%);
}
.toolbar-label { flex: none; color: var(--lc-text-secondary, #606266); font-size: 13px; font-weight: 600; }
.release-package-root { flex: 1; min-width: 0; }
.release-package-workspace {
  display: grid;
  grid-template-columns: 220px minmax(0, 1fr);
  min-height: 0;
  overflow: hidden;
  border: 1px solid #e4e7ed;
  border-radius: 10px;
  background: #f7f8fa;
  box-shadow: 0 4px 18px rgb(31 45 61 / 5%);
}
.release-package-projects {
  padding: 14px 12px;
  border-right: 1px solid #e4e7ed;
  background: #fbfcfd;
}
.projects-heading, .editor-header, .editor-actions, .engineering-card-header, .command-label-row, .log-card-header, .command-example-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.projects-heading { margin-bottom: 8px; color: #303133; }
.projects-empty, .log-empty, .editor-hint { color: var(--lc-text-secondary, #909399); font-size: 13px; }
.project-item {
  display: flex;
  width: 100%;
  flex-direction: column;
  align-items: flex-start;
  gap: 3px;
  margin-top: 4px;
  padding: 9px 10px;
  border: 1px solid transparent;
  border-radius: 7px;
  color: inherit;
  background: transparent;
  cursor: pointer;
  text-align: left;
}
.project-item:hover { border-color: #dcdfe6; background: #fff; }
.project-item.active { border-color: #b9d7fb; color: var(--el-color-primary, #409eff); background: #eef6ff; }
.project-item:disabled { cursor: not-allowed; opacity: .65; }
.project-name { overflow: hidden; max-width: 100%; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.project-updated { color: var(--lc-text-secondary, #909399); font-size: 11px; }
.release-package-editor { min-width: 0; padding: 18px; }
.editor-header { align-items: flex-start; margin-bottom: 14px; }
.editor-header h2 { margin: 0 0 4px; color: #303133; font-size: 18px; }
.editor-actions { flex-wrap: wrap; justify-content: flex-end; }
.release-package-form { min-width: 0; }
.release-package-form :deep(.el-form-item) { margin-bottom: 14px; }
.project-basics, .engineering-card {
  border: 1px solid #e4e7ed;
  border-radius: 9px;
  background: #fff;
  box-shadow: 0 2px 10px rgb(31 45 61 / 4%);
}
.project-basics { margin-bottom: 14px; padding: 14px 16px 0; }
.engineering-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 380px), 1fr));
  gap: 14px;
  align-items: start;
}
.engineering-card { min-width: 0; padding: 16px 16px 2px; }
.engineering-card-header { align-items: flex-start; margin-bottom: 16px; padding-bottom: 11px; border-bottom: 1px solid #ebeef5; }
.engineering-card-header h3 { margin: 2px 0 0; color: #303133; font-size: 16px; }
.engineering-kicker { color: var(--el-color-primary, #409eff); font-size: 10px; font-weight: 700; letter-spacing: .12em; }
.engineering-index { color: #c0c4cc; font: 600 20px/1 var(--lc-font-mono, Consolas, monospace); }
.command-label-row { width: 100%; }
.command-label-row :deep(.el-button) { height: auto; min-height: 22px; padding: 2px 4px; }
.command-input { width: 100%; }
.command-input :deep(.el-textarea__inner) {
  resize: vertical;
  font-family: var(--lc-font-mono, Consolas, monospace);
  line-height: 1.55;
}
.command-hint { margin: 7px 0 0; color: #909399; font-size: 12px; line-height: 1.55; }
.artifact-grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(150px, .65fr); gap: 12px; }
.full-width { width: 100%; }
.release-package-log-card {
  overflow: hidden;
  border: 1px solid #e4e7ed;
  border-radius: 10px;
  background: #fff;
  box-shadow: 0 2px 12px rgb(31 45 61 / 5%);
}
.log-card-header { padding: 12px 14px; border-bottom: 1px solid #ebeef5; }
.log-card-header h3 { margin: 0 0 3px; color: #303133; font-size: 15px; }
.log-card-header p { margin: 0; color: #5f6b7a; font-size: 12px; }
.log-status { flex: none; }
.release-package-log-card :deep(.log-status.el-tag--primary) {
  --el-tag-text-color: #1d4ed8;
  --el-tag-bg-color: #eff6ff;
  --el-tag-border-color: #bfdbfe;
}
.release-package-log-card :deep(.log-status.el-tag--success) {
  --el-tag-text-color: #237a3b;
  --el-tag-bg-color: #eefbf2;
  --el-tag-border-color: #b7e4c3;
}
.release-package-log-card :deep(.log-status.el-tag--info) {
  --el-tag-text-color: #4b5563;
  --el-tag-bg-color: #f3f4f6;
  --el-tag-border-color: #d1d5db;
}
.release-package-log-card :deep(.log-status.el-tag--warning) {
  --el-tag-text-color: #8a4b08;
  --el-tag-bg-color: #fff7ed;
  --el-tag-border-color: #fed7aa;
}
.release-package-log-card :deep(.log-status.el-tag--danger) {
  --el-tag-text-color: #b42318;
  --el-tag-bg-color: #fff1f0;
  --el-tag-border-color: #fecaca;
}
.release-package-log {
  min-height: 180px;
  max-height: 320px;
  overflow: auto;
  padding: 12px 14px;
  color: #303133;
  background: #fff;
  font: 12px/1.65 var(--lc-font-mono, Consolas, monospace);
}
.log-line { display: flex; gap: 8px; white-space: pre-wrap; word-break: break-word; }
.log-line.stderr { color: #d03050; }
.log-meta { flex: none; color: #5f6b7a; }
.archive-preview { margin: 0; overflow-wrap: anywhere; color: var(--lc-text-secondary, #606266); font-size: 13px; }

:global(.release-package-command-examples) {
  max-width: calc(100vw - 32px);
  padding: 10px !important;
  border-color: #dcdfe6 !important;
  background: #fff !important;
  box-shadow: 0 10px 30px rgb(31 45 61 / 14%) !important;
}
:global(.release-package-command-examples .command-example-list) {
  display: grid;
  gap: 8px;
  max-height: min(560px, calc(100vh - 120px));
  overflow: auto;
}
:global(.release-package-command-examples .command-example-item) {
  padding: 10px;
  border: 1px solid #e4e7ed;
  border-radius: 7px;
  color: #303133;
  background: #fff;
}
:global(.release-package-command-examples .command-example-heading) { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
:global(.release-package-command-examples .command-example-heading strong) { font-size: 13px; }
:global(.release-package-command-examples .command-example-item p) { margin: 5px 0 8px; color: #606266; font-size: 12px; line-height: 1.5; }
:global(.release-package-command-examples .command-example-item pre) {
  overflow-x: auto;
  margin: 0;
  padding: 9px 10px;
  border: 1px solid #ebeef5;
  border-radius: 5px;
  color: #303133;
  background: #f7f8fa;
  font: 11px/1.55 var(--lc-font-mono, Consolas, monospace);
  white-space: pre-wrap;
  word-break: break-word;
}
@media (max-width: 960px) {
  .release-package-toolbar { flex-wrap: wrap; }
  .release-package-root { flex-basis: calc(100% - 84px); }
  .release-package-workspace { grid-template-columns: 1fr; }
  .release-package-projects { display: flex; gap: 8px; overflow-x: auto; border-right: 0; border-bottom: 1px solid #e4e7ed; }
  .projects-heading { flex: none; flex-direction: column; align-items: flex-start; }
  .project-item { flex: 0 0 150px; }
  .release-package-editor { padding: 14px; }
}
@media (max-width: 640px) {
  .editor-header { flex-direction: column; }
  .editor-actions { justify-content: flex-start; }
  .artifact-grid { grid-template-columns: 1fr; gap: 0; }
  .release-package-toolbar { padding: 10px; }
  .release-package-editor { padding: 10px; }
  .engineering-card { padding: 14px 12px 0; }
  .log-card-header { align-items: flex-start; }
}
</style>

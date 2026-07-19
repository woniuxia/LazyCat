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
          <el-divider content-position="left">基本信息</el-divider>
          <el-form-item label="项目名称" required>
            <el-input v-model="draft.name" :disabled="running" placeholder="例如：订单管理系统" />
          </el-form-item>

          <el-divider content-position="left">前端工程</el-divider>
          <div class="form-grid">
            <el-form-item label="工程目录" required>
              <el-input v-model="draft.frontendProjectPath" :disabled="running" placeholder="前端工程绝对路径">
                <template #append><el-button :icon="FolderOpened" :disabled="running" @click="chooseFrontendProject">选择</el-button></template>
              </el-input>
            </el-form-item>
            <el-form-item label="构建命令" required>
              <el-input v-model="draft.frontendBuildCommand" :disabled="running" placeholder="例如：pnpm build" />
            </el-form-item>
          </div>
          <div class="form-grid">
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

          <el-divider content-position="left">后端工程</el-divider>
          <div class="form-grid">
            <el-form-item label="工程目录" required>
              <el-input v-model="draft.backendProjectPath" :disabled="running" placeholder="后端工程绝对路径">
                <template #append><el-button :icon="FolderOpened" :disabled="running" @click="chooseBackendProject">选择</el-button></template>
              </el-input>
            </el-form-item>
            <el-form-item label="构建命令" required>
              <el-input v-model="draft.backendBuildCommand" :disabled="running" placeholder="例如：mvn clean package" />
            </el-form-item>
          </div>
          <el-form-item label="产物路径" required>
            <el-input v-model="draft.backendArtifactPath" :disabled="running" placeholder="相对工程目录，可为文件或目录" />
          </el-form-item>
        </el-form>
      </main>
    </div>

    <section ref="logContainer" class="release-package-log" aria-live="polite" aria-label="打包日志">
      <div v-if="logs.length === 0" class="log-empty">暂无运行日志</div>
      <div v-for="(entry, index) in logs" :key="`${entry.runId}-${index}`" class="log-line" :class="{ stderr: entry.stream === 'stderr' }">
        <span class="log-meta">[{{ entry.phase }}] [{{ entry.stream }}]</span>
        <span>{{ entry.line }}</span>
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
import { Delete, DocumentChecked, FolderOpened, Plus, Refresh, VideoPause, VideoPlay } from "@element-plus/icons-vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { open } from "@tauri-apps/plugin-dialog";
import { invokeToolByChannel } from "../bridge/tauri";
import { getSetting, initSettings, setSettingAndWait } from "../composables/useSettings";
import { useReleasePackageRuntime } from "../composables/useReleasePackageRuntime";
import type {
  ReleasePackagePrepareResult,
  ReleasePackageProject,
  ReleasePackageProjectDraft,
  ReleasePackageStartResult,
} from "../types/release-package";
import {
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

async function loadProjects(): Promise<boolean> {
  loading.value = true;
  try {
    const result = (await invokeToolByChannel("tool:release-package:project-list", {})) as { projects?: ReleasePackageProject[] };
    projects.value = result.projects ?? [];
    const current = projects.value.find((project) => project.id === selectedId.value);
    const active = projects.value.find((project) => project.id === runtime.activeProjectId.value);
    const preferActiveProject = selectedId.value === null || runtime.status.value === "running";
    const target = preferActiveProject ? active ?? current ?? projects.value[0] : current ?? active ?? projects.value[0];
    if (target) {
      const selectionChanged = selectedId.value !== target.id;
      selectedId.value = target.id;
      if (selectionChanged || !dirty.value) Object.assign(draft, projectToReleasePackageDraft(target));
    } else {
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
.release-package-panel { display: flex; flex-direction: column; gap: 12px; min-height: 0; }
.release-package-toolbar { display: flex; align-items: center; gap: 8px; }
.toolbar-label { flex: none; color: var(--lc-text-secondary, #606266); font-size: 13px; }
.release-package-root { flex: 1; min-width: 0; }
.release-package-workspace { display: grid; grid-template-columns: 220px minmax(0, 1fr); min-height: 0; border-top: 1px solid var(--lc-border, #e5e7eb); }
.release-package-projects { padding: 12px 12px 12px 0; border-right: 1px solid var(--lc-border, #e5e7eb); }
.projects-heading, .editor-header, .editor-actions { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.projects-empty, .log-empty, .editor-hint { color: var(--lc-text-secondary, #909399); font-size: 13px; }
.project-item { display: flex; width: 100%; flex-direction: column; align-items: flex-start; gap: 3px; padding: 9px 10px; border: 0; border-radius: 4px; color: inherit; background: transparent; cursor: pointer; text-align: left; }
.project-item:hover, .project-item.active { background: var(--el-fill-color-light, #f5f7fa); }
.project-item.active { color: var(--el-color-primary, #409eff); }
.project-item:disabled { cursor: not-allowed; opacity: .65; }
.project-name { overflow: hidden; max-width: 100%; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.project-updated { color: var(--lc-text-secondary, #909399); font-size: 11px; }
.release-package-editor { min-width: 0; padding: 12px 0 0 16px; }
.editor-header { align-items: flex-start; margin-bottom: 4px; }
.editor-header h2 { margin: 0 0 4px; font-size: 18px; }
.editor-actions { flex-wrap: wrap; justify-content: flex-end; }
.release-package-form { min-width: 0; }
.release-package-form :deep(.el-divider) { margin: 16px 0 12px; }
.release-package-form :deep(.el-form-item) { margin-bottom: 12px; }
.form-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
.full-width { width: 100%; }
.release-package-log { min-height: 180px; max-height: 320px; overflow: auto; padding: 12px; color: #d7dae0; background: #1f2329; font: 12px/1.6 var(--lc-font-mono, Consolas, monospace); }
.log-line { display: flex; gap: 8px; white-space: pre-wrap; word-break: break-word; }
.log-line.stderr { color: #f56c6c; }
.log-meta { flex: none; color: #98a2b3; }
.archive-preview { margin: 0; overflow-wrap: anywhere; color: var(--lc-text-secondary, #606266); font-size: 13px; }
@media (max-width: 960px) {
  .release-package-toolbar { flex-wrap: wrap; }
  .release-package-root { flex-basis: calc(100% - 84px); }
  .release-package-workspace { grid-template-columns: 1fr; }
  .release-package-projects { display: flex; gap: 8px; overflow-x: auto; border-right: 0; border-bottom: 1px solid var(--lc-border, #e5e7eb); }
  .projects-heading { flex: none; flex-direction: column; align-items: flex-start; }
  .project-item { flex: 0 0 150px; }
  .release-package-editor { padding-left: 0; }
}
@media (max-width: 640px) {
  .form-grid { grid-template-columns: 1fr; gap: 0; }
  .editor-header { flex-direction: column; }
  .editor-actions { justify-content: flex-start; }
}
</style>

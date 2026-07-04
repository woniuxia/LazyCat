<template>
  <div class="api-mock-panel">
    <ApiMockProjectList
      :projects="projects"
      :selected-id="selectedProjectId"
      @select="selectProject"
      @create="createProjectDraft"
      @reorder="handleProjectsReorder"
    />

    <ApiMockRouteList
      :routes="routes"
      :selected-id="selectedRouteId"
      :has-project="!!selectedProject"
      :enabled-count="selectedProject?.enabledRouteCount ?? null"
      @select="selectRoute"
      @create="createRouteDraft"
      @toggle="handleRouteToggle"
      @duplicate="duplicateRoute"
      @reorder="handleRoutesReorder"
    />

    <main class="api-mock-detail">
      <div class="api-mock-actions">
        <div>
          <h2>{{ selectedProject?.name || "项目配置" }}</h2>
          <span v-if="selectedProject" class="endpoint-line">
            <span class="endpoint-url">
              {{ selectedProjectAccessUrl }}
              <el-button
                :icon="CopyDocument"
                size="small"
                text
                title="复制访问地址"
                @click="copyAccessUrl"
              />
            </span>
            <small v-if="selectedProject.host === '0.0.0.0'">监听 {{ selectedProject.host }}:{{ selectedProject.port }}</small>
          </span>
        </div>
        <div class="action-buttons">
          <el-button :icon="Refresh" size="small" :disabled="!selectedProject" @click="manualRefresh">刷新</el-button>
          <el-button
            v-if="selectedProjectRuntimeAction === 'restart'"
            :icon="Refresh"
            size="small"
            type="primary"
            :disabled="selectedProject?.enabledRouteCount === 0"
            @click="restartProject"
          >
            重启生效
          </el-button>
          <el-button
            v-if="selectedProject && selectedProject.runtime.running"
            :icon="VideoPause"
            size="small"
            type="warning"
            @click="stopProject"
          >
            停止
          </el-button>
          <el-button
            v-else-if="selectedProject"
            :icon="VideoPlay"
            size="small"
            type="primary"
            :disabled="selectedProject.enabledRouteCount === 0"
            @click="startProject"
          >
            启动
          </el-button>
        </div>
      </div>
      <div v-if="selectedProject && selectedProject.enabledRouteCount === 0" class="api-mock-notice">
        没有启用路由，启动或重启服务前至少启用一条路由。
      </div>
      <div v-else-if="selectedProjectRuntimeAction === 'restart'" class="api-mock-notice">
        运行中的服务仍使用旧配置，重启后本次修改才会生效。
      </div>
      <div v-else-if="selectedProject?.runtime.lastError && !selectedProject.runtime.running" class="api-mock-notice error">
        上次启动失败：{{ selectedProject.runtime.lastError }}
      </div>

      <el-tabs v-model="activeTab" class="api-mock-tabs">
        <el-tab-pane label="项目" name="project">
          <el-form label-position="top" class="api-mock-form">
            <div class="form-grid">
              <el-form-item label="名称">
                <el-input v-model="projectForm.name" placeholder="例如：管理后台 Mock" />
              </el-form-item>
              <el-form-item label="监听地址">
                <el-select v-model="projectForm.host">
                  <el-option label="127.0.0.1" value="127.0.0.1" />
                  <el-option label="0.0.0.0" value="0.0.0.0" />
                </el-select>
              </el-form-item>
              <el-form-item label="端口">
                <el-input-number v-model="projectForm.port" :min="1" :max="65535" controls-position="right" />
              </el-form-item>
            </div>
            <el-form-item label="描述">
              <el-input v-model="projectForm.description" type="textarea" :rows="3" />
            </el-form-item>
            <div class="form-footer">
              <el-checkbox v-model="projectForm.enabledCorsDefault">新路由默认启用 CORS</el-checkbox>
              <div>
                <el-button v-if="selectedProject" :icon="Delete" type="danger" text @click="deleteProject">删除</el-button>
                <el-button type="primary" @click="saveProject()">保存项目</el-button>
              </div>
            </div>
          </el-form>
        </el-tab-pane>

        <el-tab-pane label="路由" name="route">
          <ApiMockRouteForm
            :form="routeForm"
            :file="routeFile"
            :project="selectedProject"
            :path-error="pathError"
            :cors-error="corsError"
            :can-save="!!selectedProject"
            @save="saveRoute()"
            @delete="deleteRoute"
            @pick-file="pickFile"
          />
        </el-tab-pane>

        <el-tab-pane label="请求日志" name="logs">
          <ApiMockLogList :logs="logs" :status="logsStatus" @clear="clearLogs" @jump="jumpToLogRoute" />
        </el-tab-pane>
      </el-tabs>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { CopyDocument, Delete, Refresh, VideoPause, VideoPlay } from "@element-plus/icons-vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { open } from "@tauri-apps/plugin-dialog";
import { invokeToolByChannel } from "../bridge/tauri";
import ApiMockProjectList from "./api-mock/ApiMockProjectList.vue";
import ApiMockRouteList from "./api-mock/ApiMockRouteList.vue";
import ApiMockRouteForm from "./api-mock/ApiMockRouteForm.vue";
import ApiMockLogList from "./api-mock/ApiMockLogList.vue";
import type {
  ApiMockFileInfo,
  ApiMockProjectFormModel,
  ApiMockProjectSummary,
  ApiMockRequestLog,
  ApiMockRouteDetail,
  ApiMockRouteFormModel,
  ApiMockRouteSummary,
} from "../types/api-mock";
import type { ApiMockRouteFormSnapshot } from "../utils/apiMock";
import {
  DEFAULT_API_MOCK_CONTENT_TYPE,
  DEFAULT_API_MOCK_CORS,
  deriveMockProjectRuntimeState,
  findMockPortConflict,
  getMockFileContentTypeWarning,
  getMockProjectAccessUrl,
  getMockProjectRuntimeAction,
  normalizeMockHeaderRows,
  resolveMockFileContentType,
  serializeMockProjectForm,
  serializeMockRouteForm,
  trimMockContentType,
  validateMockContentTypeHeader,
  validateMockCorsConfig,
  validateMockPathPattern,
  validateMockStaticResponseContent,
} from "../utils/apiMock";

const LOGS_POLL_INTERVAL_MS = 2000;
const LOGS_POLL_MAX_FAILURES = 3;

const projects = ref<ApiMockProjectSummary[]>([]);
const routes = ref<ApiMockRouteSummary[]>([]);
const logs = ref<ApiMockRequestLog[]>([]);
const selectedProjectId = ref<number | null>(null);
const selectedRouteId = ref<number | null>(null);
const routeFile = ref<ApiMockFileInfo | null>(null);
const activeTab = ref("project");

const projectForm = reactive<ApiMockProjectFormModel>({
  id: null,
  name: "",
  description: "",
  host: "127.0.0.1",
  port: 18080,
  enabledCorsDefault: true,
});

const routeForm = reactive<ApiMockRouteFormModel>({
  id: null,
  name: "",
  method: "GET",
  pathPattern: "/api/demo",
  statusCode: 200,
  responseKind: "static_body",
  contentType: DEFAULT_API_MOCK_CONTENT_TYPE,
  headers: [],
  bodyText: "{\n  \"ok\": true\n}",
  enabled: true,
  delayMs: 0,
  cors: { ...DEFAULT_API_MOCK_CORS },
});

const projectBaseline = ref("");
const routeBaseline = ref("");

const selectedProject = computed(() => projects.value.find((item) => item.id === selectedProjectId.value) ?? null);
const selectedProjectAccessUrl = computed(() => {
  return selectedProject.value ? getMockProjectAccessUrl(selectedProject.value) : "";
});
const selectedProjectRuntimeAction = computed(() => {
  return selectedProject.value ? getMockProjectRuntimeAction(selectedProject.value) : "start";
});
const pathError = computed(() => {
  const result = validateMockPathPattern(routeForm.pathPattern);
  return result.ok ? "" : result.message;
});
const corsError = computed(() => {
  const result = validateMockCorsConfig(routeForm.cors);
  return result.ok ? "" : result.message;
});

function projectFormSnapshotText(): string {
  return serializeMockProjectForm({
    name: projectForm.name,
    description: projectForm.description,
    host: projectForm.host,
    port: projectForm.port,
    enabledCorsDefault: projectForm.enabledCorsDefault,
  });
}

function routeFormSnapshotText(): string {
  return serializeMockRouteForm({
    name: routeForm.name,
    method: routeForm.method,
    pathPattern: routeForm.pathPattern,
    statusCode: routeForm.statusCode,
    responseKind: routeForm.responseKind,
    contentType: routeForm.contentType,
    headers: routeForm.headers.map((row) => ({ enabled: row.enabled, key: row.key, value: row.value })),
    bodyText: routeForm.bodyText,
    enabled: routeForm.enabled,
    delayMs: routeForm.delayMs,
    fileId: routeFile.value?.id ?? null,
    cors: { ...routeForm.cors, allowMethods: [...routeForm.cors.allowMethods] },
  });
}

const isProjectDirty = computed(() => projectFormSnapshotText() !== projectBaseline.value);
const isRouteDirty = computed(() => routeFormSnapshotText() !== routeBaseline.value);

function assignProjectForm(project: ApiMockProjectSummary | null) {
  projectForm.id = project?.id ?? null;
  projectForm.name = project?.name ?? "";
  projectForm.description = project?.description ?? "";
  projectForm.host = project?.host ?? "127.0.0.1";
  projectForm.port = project?.port ?? 18080;
  projectForm.enabledCorsDefault = project?.enabledCorsDefault ?? true;
  projectBaseline.value = projectFormSnapshotText();
}

function assignRouteForm(route: ApiMockRouteDetail | null) {
  routeForm.id = route?.id ?? null;
  routeForm.name = route?.name ?? "";
  routeForm.method = route?.method ?? "GET";
  routeForm.pathPattern = route?.pathPattern ?? "/api/demo";
  routeForm.statusCode = route?.statusCode ?? 200;
  routeForm.responseKind = route?.responseKind ?? "static_body";
  routeForm.contentType = route?.contentType ?? DEFAULT_API_MOCK_CONTENT_TYPE;
  routeForm.headers = route?.headers ? route.headers.map((row) => ({ ...row })) : [];
  routeForm.bodyText = route?.bodyText ?? "{\n  \"ok\": true\n}";
  routeForm.enabled = route?.enabled ?? true;
  routeForm.delayMs = route?.delayMs ?? 0;
  routeForm.cors = route?.cors ? { ...route.cors } : { ...DEFAULT_API_MOCK_CORS };
  routeFile.value = route?.file ?? null;
  routeBaseline.value = routeFormSnapshotText();
}

function patchRouteBaseline(patch: Partial<ApiMockRouteFormSnapshot>) {
  try {
    routeBaseline.value = JSON.stringify({ ...JSON.parse(routeBaseline.value), ...patch });
  } catch {
    // 基线解析失败时跳过，等待下次 assign 重建。
  }
}

function getErrorMessage(error: unknown, fallback: string): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string" && error) return error;
  return fallback;
}

function showSaveResult(subject: string) {
  if (selectedProject.value && deriveMockProjectRuntimeState(selectedProject.value) === "restart-required") {
    ElMessage.warning(`${subject}已保存，重启 Mock 服务后生效`);
    return;
  }
  ElMessage.success(`${subject}已保存`);
}

async function confirmLeaveForms(scope: "route" | "all"): Promise<boolean> {
  const dirtyProject = scope === "all" && isProjectDirty.value;
  const dirtyRoute = isRouteDirty.value;
  if (!dirtyProject && !dirtyRoute) return true;

  let action: "confirm" | "cancel" | "close";
  try {
    await ElMessageBox.confirm("当前有未保存的修改，直接切换将丢失这些修改。", "未保存的修改", {
      distinguishCancelAndClose: true,
      confirmButtonText: "保存并切换",
      cancelButtonText: "放弃修改",
      type: "warning",
    });
    action = "confirm";
  } catch (reason) {
    action = reason === "cancel" ? "cancel" : "close";
  }
  if (action === "close") return false;
  if (action === "cancel") return true;
  if (dirtyProject && !(await saveProject())) return false;
  if (dirtyRoute && !(await saveRoute({ reselect: false }))) return false;
  return true;
}

async function refreshProjects() {
  const result = (await invokeToolByChannel("tool:api-mock:project-list", {})) as { projects: ApiMockProjectSummary[] };
  projects.value = result.projects;
  if (!selectedProjectId.value && projects.value[0]) {
    selectedProjectId.value = projects.value[0].id;
  }
  assignProjectForm(selectedProject.value);
}

async function refreshRoutes() {
  if (!selectedProjectId.value) {
    routes.value = [];
    logs.value = [];
    return;
  }
  const result = (await invokeToolByChannel("tool:api-mock:route-list", {
    projectId: selectedProjectId.value,
  })) as { routes: ApiMockRouteSummary[] };
  routes.value = result.routes;
  if (!selectedRouteId.value && routes.value[0] && !isRouteDirty.value) {
    await loadRoute(routes.value[0].id, { switchTab: false });
  }
}

async function refreshLogs() {
  if (!selectedProjectId.value) return;
  const result = (await invokeToolByChannel("tool:api-mock:request-logs", {
    projectId: selectedProjectId.value,
  })) as { logs: ApiMockRequestLog[] };
  logs.value = result.logs;
}

async function refreshAll() {
  await refreshProjects();
  await refreshRoutes();
  await refreshLogs();
}

function manualRefresh() {
  resetLogsPolling();
  void refreshAll();
}

async function loadRoute(id: number, options: { switchTab?: boolean } = {}) {
  const { switchTab = true } = options;
  const result = (await invokeToolByChannel("tool:api-mock:route-get", { id })) as { route: ApiMockRouteDetail };
  selectedRouteId.value = id;
  assignRouteForm(result.route);
  if (switchTab) activeTab.value = "route";
}

async function selectRoute(id: number) {
  if (id === selectedRouteId.value) {
    activeTab.value = "route";
    return;
  }
  if (!(await confirmLeaveForms("route"))) return;
  await loadRoute(id);
}

async function selectProject(id: number) {
  if (id === selectedProjectId.value) return;
  if (!(await confirmLeaveForms("all"))) return;
  selectedProjectId.value = id;
  selectedRouteId.value = null;
  assignProjectForm(selectedProject.value);
  assignRouteForm(null);
  await refreshRoutes();
  await refreshLogs();
}

async function createProjectDraft() {
  if (!(await confirmLeaveForms("all"))) return;
  selectedProjectId.value = null;
  selectedRouteId.value = null;
  routes.value = [];
  logs.value = [];
  assignProjectForm(null);
  assignRouteForm(null);
  activeTab.value = "project";
}

async function createRouteDraft() {
  if (!(await confirmLeaveForms("route"))) return;
  selectedRouteId.value = null;
  assignRouteForm(null);
  activeTab.value = "route";
}

async function duplicateRoute(route: ApiMockRouteSummary) {
  if (!(await confirmLeaveForms("route"))) return;
  try {
    const result = (await invokeToolByChannel("tool:api-mock:route-get", { id: route.id })) as {
      route: ApiMockRouteDetail;
    };
    assignRouteForm(result.route);
    routeForm.id = null;
    routeForm.name = `${result.route.name} 副本`;
    selectedRouteId.value = null;
    activeTab.value = "route";
    ElMessage.success("已创建副本，保存后生效");
  } catch (error) {
    ElMessage.error(getErrorMessage(error, "复制路由失败"));
  }
}

async function saveProject(): Promise<boolean> {
  if (!projectForm.name.trim()) {
    ElMessage.error("项目名称不能为空");
    return false;
  }
  const payload = {
    id: projectForm.id ?? undefined,
    name: projectForm.name.trim(),
    description: projectForm.description,
    host: projectForm.host,
    port: projectForm.port,
    enabledCorsDefault: projectForm.enabledCorsDefault,
  };
  try {
    const channel = projectForm.id ? "tool:api-mock:project-update" : "tool:api-mock:project-create";
    const result = (await invokeToolByChannel(channel, payload)) as { id: number };
    selectedProjectId.value = result.id;
    await refreshProjects();
    await refreshRoutes();
    showSaveResult("项目");
    const conflict = findMockPortConflict(projects.value, result.id, projectForm.port);
    if (conflict) {
      ElMessage.warning(`端口与项目「${conflict.name}」相同，二者不能同时运行`);
    }
    return true;
  } catch (error) {
    ElMessage.error(getErrorMessage(error, "保存项目失败"));
    return false;
  }
}

async function deleteProject() {
  if (!selectedProjectId.value) return;
  try {
    await ElMessageBox.confirm("删除该 Mock 项目及其路由配置？运行中的服务会同时停止。", "删除项目", { type: "warning" });
  } catch {
    return;
  }
  try {
    await invokeToolByChannel("tool:api-mock:project-delete", { id: selectedProjectId.value });
    selectedProjectId.value = null;
    selectedRouteId.value = null;
    assignRouteForm(null);
    await refreshAll();
    ElMessage.success("项目已删除");
  } catch (error) {
    ElMessage.error(getErrorMessage(error, "删除项目失败"));
  }
}

async function saveRoute(options: { reselect?: boolean } = {}): Promise<boolean> {
  const { reselect = true } = options;
  if (!selectedProjectId.value) {
    ElMessage.error("请先选择或保存一个 Mock 项目");
    return false;
  }
  if (pathError.value) {
    ElMessage.error(pathError.value);
    return false;
  }
  if (corsError.value) {
    ElMessage.error(corsError.value);
    return false;
  }

  const contentType = trimMockContentType(routeForm.contentType);
  const contentTypeResult = validateMockContentTypeHeader(contentType);
  if (!contentTypeResult.ok) {
    ElMessage.error(contentTypeResult.message);
    return false;
  }

  const staticContentNotice =
    routeForm.responseKind === "static_body"
      ? validateMockStaticResponseContent({ contentType, bodyText: routeForm.bodyText })
      : null;
  if (staticContentNotice?.level === "error") {
    ElMessage.error(staticContentNotice.message);
    return false;
  }
  if (staticContentNotice?.level === "warning") {
    ElMessage.warning(staticContentNotice.message);
  }

  if (routeForm.responseKind === "file" && routeFile.value) {
    const fileContentTypeWarning = getMockFileContentTypeWarning({
      contentType,
      fileName: routeFile.value.originalName,
    });
    if (fileContentTypeWarning) {
      ElMessage.warning(fileContentTypeWarning);
    }
  }
  routeForm.contentType = contentType;

  if (routeForm.responseKind === "file" && !routeFile.value) {
    ElMessage.error("文件响应需要先导入文件");
    return false;
  }
  try {
    const result = (await invokeToolByChannel("tool:api-mock:route-save", {
      id: routeForm.id ?? undefined,
      projectId: selectedProjectId.value,
      name: routeForm.name.trim() || `${routeForm.method} ${routeForm.pathPattern}`,
      method: routeForm.method,
      pathPattern: routeForm.pathPattern,
      statusCode: routeForm.statusCode,
      responseKind: routeForm.responseKind,
      contentType,
      headers: normalizeMockHeaderRows(routeForm.headers),
      bodyText: routeForm.bodyText,
      fileId: routeFile.value?.id,
      delayMs: routeForm.delayMs,
      cors: routeForm.cors,
      enabled: routeForm.enabled,
    })) as { id: number };
    routeBaseline.value = routeFormSnapshotText();
    await refreshProjects();
    if (reselect) {
      selectedRouteId.value = result.id;
      await refreshRoutes();
      await loadRoute(result.id);
    } else {
      await refreshRoutes();
    }
    showSaveResult("路由");
    return true;
  } catch (error) {
    ElMessage.error(getErrorMessage(error, "保存路由失败"));
    return false;
  }
}

async function deleteRoute() {
  if (!selectedRouteId.value) return;
  try {
    await ElMessageBox.confirm("删除该 Mock 路由？运行中的服务需要重启后才会移除该路由。", "删除路由", { type: "warning" });
  } catch {
    return;
  }
  try {
    await invokeToolByChannel("tool:api-mock:route-delete", { id: selectedRouteId.value });
    selectedRouteId.value = null;
    assignRouteForm(null);
    await refreshProjects();
    await refreshRoutes();
    ElMessage.success("路由已删除");
  } catch (error) {
    ElMessage.error(getErrorMessage(error, "删除路由失败"));
  }
}

async function handleRouteToggle(route: ApiMockRouteSummary, enabled: boolean) {
  try {
    await invokeToolByChannel("tool:api-mock:route-toggle", { id: route.id, enabled });
    if (routeForm.id === route.id) {
      routeForm.enabled = enabled;
      patchRouteBaseline({ enabled });
    }
    await refreshProjects();
    await refreshRoutes();
  } catch (error) {
    ElMessage.error(getErrorMessage(error, "切换路由状态失败"));
    await refreshRoutes().catch(() => undefined);
  }
}

async function handleProjectsReorder(ids: number[]) {
  const previous = projects.value;
  projects.value = ids
    .map((id) => previous.find((project) => project.id === id))
    .filter((project): project is ApiMockProjectSummary => !!project);
  try {
    await invokeToolByChannel("tool:api-mock:project-reorder", { ids });
    await refreshProjects();
  } catch (error) {
    ElMessage.error(getErrorMessage(error, "保存项目排序失败"));
    await refreshProjects().catch(() => undefined);
  }
}

async function handleRoutesReorder(ids: number[]) {
  if (!selectedProjectId.value) return;
  const previous = routes.value;
  routes.value = ids
    .map((id) => previous.find((route) => route.id === id))
    .filter((route): route is ApiMockRouteSummary => !!route);
  try {
    await invokeToolByChannel("tool:api-mock:route-reorder", { projectId: selectedProjectId.value, ids });
    await refreshProjects();
    await refreshRoutes();
  } catch (error) {
    ElMessage.error(getErrorMessage(error, "保存路由排序失败"));
    await Promise.allSettled([refreshProjects(), refreshRoutes()]);
  }
}

async function pickFile() {
  const selected = await open({ multiple: false });
  if (!selected || Array.isArray(selected)) return;
  const contentType = resolveMockFileContentType(routeForm.contentType, selected);
  try {
    const result = (await invokeToolByChannel("tool:api-mock:file-import", {
      path: selected,
      contentType,
    })) as { file: ApiMockFileInfo };
    routeForm.contentType = contentType;
    routeFile.value = result.file;
    const fileContentTypeWarning = getMockFileContentTypeWarning({
      contentType,
      fileName: selected,
    });
    if (fileContentTypeWarning) {
      ElMessage.warning(fileContentTypeWarning);
    }
  } catch (error) {
    ElMessage.error(getErrorMessage(error, "导入文件失败"));
  }
}

async function startProject() {
  if (!selectedProjectId.value) return;
  try {
    await invokeToolByChannel("tool:api-mock:service-start", { projectId: selectedProjectId.value });
    await refreshAll();
    resetLogsPolling();
    activeTab.value = "logs";
    ElMessage.success("Mock 服务已启动");
  } catch (error) {
    await refreshAll().catch(() => undefined);
    ElMessage.error(getErrorMessage(error, "启动 Mock 服务失败"));
  }
}

async function stopProject() {
  if (!selectedProjectId.value) return;
  try {
    await invokeToolByChannel("tool:api-mock:service-stop", { projectId: selectedProjectId.value });
    await refreshAll();
    ElMessage.success("Mock 服务已停止");
  } catch (error) {
    await refreshAll().catch(() => undefined);
    ElMessage.error(getErrorMessage(error, "停止 Mock 服务失败"));
  }
}

async function restartProject() {
  if (!selectedProjectId.value) return;
  try {
    await invokeToolByChannel("tool:api-mock:service-stop", { projectId: selectedProjectId.value });
    await invokeToolByChannel("tool:api-mock:service-start", { projectId: selectedProjectId.value });
    await refreshAll();
    resetLogsPolling();
    ElMessage.success("Mock 服务已重启");
  } catch (error) {
    await refreshAll().catch(() => undefined);
    ElMessage.error(getErrorMessage(error, "重启 Mock 服务失败"));
  }
}

async function clearLogs() {
  if (!selectedProjectId.value) return;
  try {
    await invokeToolByChannel("tool:api-mock:request-logs-clear", { projectId: selectedProjectId.value });
    logs.value = [];
  } catch (error) {
    ElMessage.error(getErrorMessage(error, "清空日志失败"));
  }
}

async function jumpToLogRoute(log: ApiMockRequestLog) {
  if (log.routeId === null) return;
  const exists = routes.value.some((route) => route.id === log.routeId);
  if (!exists) {
    ElMessage.warning("该路由已删除");
    return;
  }
  await selectRoute(log.routeId);
}

async function copyAccessUrl() {
  try {
    await navigator.clipboard.writeText(selectedProjectAccessUrl.value);
    ElMessage.success("已复制访问地址");
  } catch {
    ElMessage.error("复制失败");
  }
}

// ---- 日志轮询：仅在日志页激活且服务运行中时开启 ----
let logsTimer: ReturnType<typeof setInterval> | null = null;
let logsFailureCount = 0;
const logsPaused = ref(false);

const shouldPollLogs = computed(
  () => activeTab.value === "logs" && !!selectedProject.value?.runtime.running && !logsPaused.value,
);

const logsStatus = computed<"active" | "stopped" | "paused">(() => {
  if (!selectedProject.value?.runtime.running) return "stopped";
  if (logsPaused.value) return "paused";
  return "active";
});

function stopLogsTimer() {
  if (logsTimer !== null) {
    clearInterval(logsTimer);
    logsTimer = null;
  }
}

function startLogsTimer() {
  stopLogsTimer();
  logsTimer = setInterval(() => {
    void pollLogs();
  }, LOGS_POLL_INTERVAL_MS);
}

async function pollLogs() {
  try {
    await refreshLogs();
    logsFailureCount = 0;
  } catch {
    logsFailureCount += 1;
    if (logsFailureCount >= LOGS_POLL_MAX_FAILURES) {
      logsPaused.value = true;
      ElMessage.warning("请求日志自动刷新已暂停，可点击刷新恢复");
    }
  }
}

function resetLogsPolling() {
  logsFailureCount = 0;
  logsPaused.value = false;
}

watch(shouldPollLogs, (active) => {
  if (active) {
    startLogsTimer();
  } else {
    stopLogsTimer();
  }
});

watch([selectedProjectId, activeTab], () => {
  resetLogsPolling();
});

onBeforeUnmount(stopLogsTimer);

onMounted(() => {
  assignProjectForm(null);
  assignRouteForm(null);
  void refreshAll();
});
</script>

<style scoped>
.api-mock-panel {
  display: grid;
  grid-template-columns: 260px 320px minmax(0, 1fr);
  gap: 14px;
  height: 100%;
  min-height: 0;
  color: #172033;
}

.api-mock-detail {
  display: flex;
  flex-direction: column;
  min-height: 0;
  border: 1px solid #dbe4ef;
  border-radius: 8px;
  background: #ffffff;
  overflow: hidden;
}

.api-mock-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 16px;
  border-bottom: 1px solid #e2e8f0;
}

.api-mock-actions h2 {
  margin: 0;
  font-size: 15px;
  line-height: 1.4;
}

.api-mock-actions span {
  font-size: 12px;
  color: #64748b;
}

.endpoint-line {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.endpoint-url {
  display: inline-flex;
  align-items: center;
  gap: 2px;
}

.endpoint-line small {
  font-size: 12px;
  color: #94a3b8;
}

.action-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  justify-content: flex-end;
}

.api-mock-notice {
  padding: 9px 16px;
  border-bottom: 1px solid #fed7aa;
  background: #fff7ed;
  color: #9a3412;
  font-size: 13px;
}

.api-mock-notice.error {
  border-bottom-color: #fecaca;
  background: #fef2f2;
  color: #991b1b;
}

.api-mock-tabs {
  min-height: 0;
  padding: 0 16px 16px;
  overflow: auto;
}

.api-mock-form {
  max-width: 980px;
}

.form-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

.form-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

@media (max-width: 1180px) {
  .api-mock-panel {
    grid-template-columns: 240px minmax(0, 1fr);
  }

  .api-mock-panel > :nth-child(2) {
    display: none;
  }
}

@media (max-width: 860px) {
  .api-mock-panel {
    grid-template-columns: 1fr;
  }

  .form-grid {
    grid-template-columns: 1fr;
  }
}
</style>

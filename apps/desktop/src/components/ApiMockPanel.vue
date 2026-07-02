<template>
  <div class="api-mock-panel">
    <aside class="api-mock-projects">
      <div class="api-mock-toolbar">
        <div>
          <h2>API Mock</h2>
          <span>{{ projects.length }} 个项目</span>
        </div>
        <el-button :icon="Plus" size="small" @click="createProjectDraft">项目</el-button>
      </div>

      <div v-if="projects.length === 0" class="api-mock-empty">
        暂无 Mock 项目。先新建项目，再添加路由并启动本地服务。
      </div>

      <button
        v-for="project in projects"
        :key="project.id"
        class="api-mock-project"
        :class="{ active: project.id === selectedProjectId }"
        type="button"
        @click="selectProject(project.id)"
      >
        <span class="project-main">
          <strong>{{ project.name }}</strong>
          <small>{{ project.host }}:{{ project.port }}</small>
        </span>
        <el-tag size="small" :type="runtimeTagType(project)">
          {{ runtimeLabel(project) }}
        </el-tag>
      </button>
    </aside>

    <section class="api-mock-routes">
      <div class="api-mock-toolbar">
        <div>
          <h2>路由</h2>
          <span v-if="selectedProject">{{ selectedProject.enabledRouteCount }}/{{ selectedProject.routeCount }} 启用</span>
        </div>
        <el-button :icon="Plus" size="small" :disabled="!selectedProject" @click="createRouteDraft">路由</el-button>
      </div>

      <div v-if="!selectedProject" class="api-mock-empty">选择或新建一个 Mock 项目。</div>
      <template v-else>
        <div v-if="routes.length === 0" class="api-mock-empty">暂无路由。添加第一条路由后即可启动服务。</div>
        <button
          v-for="route in routes"
          :key="route.id"
          class="api-mock-route"
          :class="{ active: route.id === selectedRouteId, disabled: !route.enabled }"
          type="button"
          @click="selectRoute(route.id)"
        >
          <span class="method">{{ route.method }}</span>
          <span class="route-path">{{ route.pathPattern }}</span>
          <span class="route-meta">
            {{ route.statusCode }} · {{ route.responseKind === "file" ? "文件" : "文本" }}
            <span v-if="!route.enabled"> · 已禁用</span>
          </span>
        </button>
      </template>
    </section>

    <main class="api-mock-detail">
      <div class="api-mock-actions">
        <div>
          <h2>{{ selectedProject?.name || "项目配置" }}</h2>
          <span v-if="selectedProject" class="endpoint-line">
            <span>{{ selectedProjectAccessUrl }}</span>
            <small v-if="selectedProject.host === '0.0.0.0'">监听 {{ selectedProject.host }}:{{ selectedProject.port }}</small>
          </span>
        </div>
        <div class="action-buttons">
          <el-button :icon="Refresh" size="small" :disabled="!selectedProject" @click="refreshAll">刷新</el-button>
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
                <el-button type="primary" @click="saveProject">保存项目</el-button>
              </div>
            </div>
          </el-form>
        </el-tab-pane>

        <el-tab-pane label="路由" name="route">
          <el-form label-position="top" class="api-mock-form">
            <div class="form-grid route-grid">
              <el-form-item label="名称">
                <el-input v-model="routeForm.name" />
              </el-form-item>
              <el-form-item label="方法">
                <el-select v-model="routeForm.method">
                  <el-option v-for="method in API_MOCK_METHODS" :key="method" :label="method" :value="method" />
                </el-select>
              </el-form-item>
              <el-form-item label="路径匹配">
                <el-input v-model="routeForm.pathPattern" placeholder="/api/users/:id 或 /files/*" />
              </el-form-item>
              <el-form-item label="状态码">
                <el-input-number v-model="routeForm.statusCode" :min="100" :max="599" controls-position="right" />
              </el-form-item>
            </div>

            <div v-if="pathError" class="api-mock-error">{{ pathError }}</div>

            <div class="form-grid">
              <el-form-item label="响应类型">
                <el-radio-group v-model="routeForm.responseKind">
                  <el-radio-button label="static_body">文本</el-radio-button>
                  <el-radio-button label="file">文件</el-radio-button>
                </el-radio-group>
              </el-form-item>
              <el-form-item label="Content-Type">
                <el-input v-model="routeForm.contentType" placeholder="application/json; charset=utf-8" />
              </el-form-item>
              <el-form-item label="启用">
                <el-switch v-model="routeForm.enabled" />
              </el-form-item>
            </div>

            <el-form-item v-if="routeForm.responseKind === 'static_body'" label="响应 Body">
              <el-input v-model="routeForm.bodyText" type="textarea" :rows="10" spellcheck="false" />
            </el-form-item>

            <div v-else class="file-box">
              <div>
                <strong>{{ routeFile?.originalName || "未选择文件" }}</strong>
                <span v-if="routeFile">
                  {{ routeFile.contentType || "application/octet-stream" }} · {{ formatMockFileSize(routeFile.size) }}
                </span>
              </div>
              <el-button :icon="Upload" @click="pickFile">导入文件</el-button>
            </div>

            <div class="subsection-head">
              <h3>响应头</h3>
              <el-button :icon="Plus" size="small" text @click="addHeader">添加</el-button>
            </div>
            <div class="header-rows">
              <div v-for="(row, index) in routeForm.headers" :key="index" class="header-row">
                <el-checkbox v-model="row.enabled" />
                <el-input v-model="row.key" placeholder="Header" />
                <el-input v-model="row.value" placeholder="Value" />
                <el-button :icon="Delete" text @click="routeForm.headers.splice(index, 1)" />
              </div>
            </div>

            <el-collapse>
              <el-collapse-item title="CORS" name="cors">
                <div class="form-grid">
                  <el-form-item label="启用 CORS">
                    <el-switch v-model="routeForm.cors.enabled" />
                  </el-form-item>
                  <el-form-item label="Allow-Origin">
                    <el-input v-model="routeForm.cors.allowOrigin" />
                  </el-form-item>
                  <el-form-item label="Allow-Headers">
                    <el-input v-model="routeForm.cors.allowHeaders" />
                  </el-form-item>
                  <el-form-item label="Expose-Headers">
                    <el-input v-model="routeForm.cors.exposeHeaders" />
                  </el-form-item>
                  <el-form-item label="Allow-Credentials">
                    <el-switch v-model="routeForm.cors.allowCredentials" />
                  </el-form-item>
                  <el-form-item label="Max-Age">
                    <el-input-number v-model="routeForm.cors.maxAgeSeconds" :min="0" controls-position="right" />
                  </el-form-item>
                </div>
                <div v-if="corsError" class="api-mock-error">{{ corsError }}</div>
              </el-collapse-item>
            </el-collapse>

            <div class="form-footer">
              <span>{{ getMockRouteSpecificityLabel(routeForm.pathPattern) }}匹配</span>
              <div>
                <el-button v-if="selectedRouteId" :icon="Delete" type="danger" text @click="deleteRoute">删除</el-button>
                <el-button type="primary" :disabled="!selectedProject" @click="saveRoute">保存路由</el-button>
              </div>
            </div>
          </el-form>
        </el-tab-pane>

        <el-tab-pane label="请求日志" name="logs">
          <div class="log-list">
            <div v-if="logs.length === 0" class="api-mock-empty">暂无运行期请求日志。</div>
            <div v-for="log in logs" :key="log.id" class="log-row">
              <span class="method">{{ log.method }}</span>
              <strong>{{ log.path }}</strong>
              <span>{{ log.status }}</span>
              <span>{{ log.durationMs }} ms</span>
              <small>{{ log.routeName || log.error || "未命中" }}</small>
            </div>
          </div>
        </el-tab-pane>
      </el-tabs>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { Delete, Plus, Refresh, Upload, VideoPause, VideoPlay } from "@element-plus/icons-vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { open } from "@tauri-apps/plugin-dialog";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  ApiMockCorsConfig,
  ApiMockFileInfo,
  ApiMockHeaderRow,
  ApiMockMethod,
  ApiMockProjectSummary,
  ApiMockRequestLog,
  ApiMockResponseKind,
  ApiMockRouteDetail,
  ApiMockRouteSummary,
} from "../types/api-mock";
import {
  API_MOCK_METHODS,
  DEFAULT_API_MOCK_CONTENT_TYPE,
  DEFAULT_API_MOCK_CORS,
  deriveMockProjectRuntimeState,
  formatMockFileSize,
  getMockProjectAccessUrl,
  getMockProjectRuntimeAction,
  getMockRouteSpecificityLabel,
  normalizeMockHeaderRows,
  resolveMockFileContentType,
  validateMockCorsConfig,
  validateMockPathPattern,
} from "../utils/apiMock";

interface ProjectForm {
  id: number | null;
  name: string;
  description: string;
  host: "127.0.0.1" | "0.0.0.0";
  port: number;
  enabledCorsDefault: boolean;
}

interface RouteForm {
  id: number | null;
  name: string;
  method: ApiMockMethod;
  pathPattern: string;
  statusCode: number;
  responseKind: ApiMockResponseKind;
  contentType: string;
  headers: ApiMockHeaderRow[];
  bodyText: string;
  enabled: boolean;
  cors: ApiMockCorsConfig;
}

const projects = ref<ApiMockProjectSummary[]>([]);
const routes = ref<ApiMockRouteSummary[]>([]);
const logs = ref<ApiMockRequestLog[]>([]);
const selectedProjectId = ref<number | null>(null);
const selectedRouteId = ref<number | null>(null);
const routeFile = ref<ApiMockFileInfo | null>(null);
const activeTab = ref("project");

const projectForm = reactive<ProjectForm>({
  id: null,
  name: "",
  description: "",
  host: "127.0.0.1",
  port: 18080,
  enabledCorsDefault: true,
});

const routeForm = reactive<RouteForm>({
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
  cors: { ...DEFAULT_API_MOCK_CORS },
});

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

function assignProjectForm(project: ApiMockProjectSummary | null) {
  projectForm.id = project?.id ?? null;
  projectForm.name = project?.name ?? "";
  projectForm.description = project?.description ?? "";
  projectForm.host = project?.host ?? "127.0.0.1";
  projectForm.port = project?.port ?? 18080;
  projectForm.enabledCorsDefault = project?.enabledCorsDefault ?? true;
}

function assignRouteForm(route: ApiMockRouteDetail | null) {
  routeForm.id = route?.id ?? null;
  routeForm.name = route?.name ?? "";
  routeForm.method = route?.method ?? "GET";
  routeForm.pathPattern = route?.pathPattern ?? "/api/demo";
  routeForm.statusCode = route?.statusCode ?? 200;
  routeForm.responseKind = route?.responseKind ?? "static_body";
  routeForm.contentType = route?.contentType ?? DEFAULT_API_MOCK_CONTENT_TYPE;
  routeForm.headers = route?.headers ? [...route.headers] : [];
  routeForm.bodyText = route?.bodyText ?? "{\n  \"ok\": true\n}";
  routeForm.enabled = route?.enabled ?? true;
  routeForm.cors = route?.cors ? { ...route.cors } : { ...DEFAULT_API_MOCK_CORS };
  routeFile.value = route?.file ?? null;
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
  if (!selectedRouteId.value && routes.value[0]) {
    await selectRoute(routes.value[0].id);
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

async function selectProject(id: number) {
  selectedProjectId.value = id;
  selectedRouteId.value = null;
  assignProjectForm(selectedProject.value);
  assignRouteForm(null);
  await refreshRoutes();
  await refreshLogs();
}

async function selectRoute(id: number) {
  selectedRouteId.value = id;
  const result = (await invokeToolByChannel("tool:api-mock:route-get", { id })) as { route: ApiMockRouteDetail };
  assignRouteForm(result.route);
  activeTab.value = "route";
}

function createProjectDraft() {
  selectedProjectId.value = null;
  routes.value = [];
  logs.value = [];
  assignProjectForm(null);
  activeTab.value = "project";
}

function createRouteDraft() {
  selectedRouteId.value = null;
  assignRouteForm(null);
  activeTab.value = "route";
}

async function saveProject() {
  if (!projectForm.name.trim()) {
    ElMessage.error("项目名称不能为空");
    return;
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
  } catch (error) {
    ElMessage.error(getErrorMessage(error, "保存项目失败"));
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
    await refreshAll();
    ElMessage.success("项目已删除");
  } catch (error) {
    ElMessage.error(getErrorMessage(error, "删除项目失败"));
  }
}

async function saveRoute() {
  if (!selectedProjectId.value) return;
  if (pathError.value) {
    ElMessage.error(pathError.value);
    return;
  }
  if (corsError.value) {
    ElMessage.error(corsError.value);
    return;
  }
  if (routeForm.responseKind === "file" && !routeFile.value) {
    ElMessage.error("文件响应需要先导入文件");
    return;
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
      contentType: routeForm.contentType,
      headers: normalizeMockHeaderRows(routeForm.headers),
      bodyText: routeForm.bodyText,
      fileId: routeFile.value?.id,
      cors: routeForm.cors,
      enabled: routeForm.enabled,
    })) as { id: number };
    selectedRouteId.value = result.id;
    await refreshProjects();
    await refreshRoutes();
    await selectRoute(result.id);
    showSaveResult("路由");
  } catch (error) {
    ElMessage.error(getErrorMessage(error, "保存路由失败"));
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
  } catch (error) {
    ElMessage.error(getErrorMessage(error, "导入文件失败"));
  }
}

async function startProject() {
  if (!selectedProjectId.value) return;
  try {
    await invokeToolByChannel("tool:api-mock:service-start", { projectId: selectedProjectId.value });
    await refreshAll();
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
    ElMessage.success("Mock 服务已重启");
  } catch (error) {
    await refreshAll().catch(() => undefined);
    ElMessage.error(getErrorMessage(error, "重启 Mock 服务失败"));
  }
}

function addHeader() {
  routeForm.headers.push({ enabled: true, key: "", value: "" });
}

function runtimeLabel(project: ApiMockProjectSummary): string {
  const state = deriveMockProjectRuntimeState(project);
  if (state === "running") return "运行中";
  if (state === "restart-required") return "需重启";
  if (state === "error") return "异常";
  return "已停止";
}

function runtimeTagType(project: ApiMockProjectSummary): "success" | "warning" | "danger" | "info" {
  const state = deriveMockProjectRuntimeState(project);
  if (state === "running") return "success";
  if (state === "restart-required") return "warning";
  if (state === "error") return "danger";
  return "info";
}

onMounted(refreshAll);
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

.api-mock-projects,
.api-mock-routes,
.api-mock-detail {
  min-height: 0;
  border: 1px solid #dbe4ef;
  border-radius: 8px;
  background: #ffffff;
}

.api-mock-projects,
.api-mock-routes {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  overflow: auto;
}

.api-mock-detail {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.api-mock-toolbar,
.api-mock-actions,
.subsection-head,
.form-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.api-mock-toolbar h2,
.api-mock-actions h2,
.subsection-head h3 {
  margin: 0;
  font-size: 15px;
  line-height: 1.4;
}

.api-mock-toolbar span,
.api-mock-actions span,
.form-footer span,
.file-box span,
.log-row small {
  font-size: 12px;
  color: #64748b;
}

.endpoint-line {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.endpoint-line small {
  font-size: 12px;
  color: #94a3b8;
}

.api-mock-project,
.api-mock-route {
  width: 100%;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  background: #f8fafc;
  color: inherit;
  text-align: left;
  cursor: pointer;
}

.api-mock-project {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 10px;
}

.api-mock-project.active,
.api-mock-route.active {
  border-color: #2563eb;
  background: #eff6ff;
}

.api-mock-route.disabled {
  background: #f8fafc;
  color: #94a3b8;
}

.api-mock-route.disabled .method {
  color: #64748b;
}

.project-main {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 4px;
}

.project-main strong,
.route-path,
.log-row strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.api-mock-route {
  display: grid;
  grid-template-columns: 58px minmax(0, 1fr);
  gap: 4px 8px;
  padding: 10px;
}

.method {
  font-size: 12px;
  font-weight: 700;
  color: #0f766e;
}

.route-meta {
  grid-column: 2;
  font-size: 12px;
  color: #64748b;
}

.api-mock-actions {
  padding: 14px 16px;
  border-bottom: 1px solid #e2e8f0;
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

.route-grid {
  grid-template-columns: minmax(140px, 1fr) 120px minmax(220px, 1.3fr) 120px;
}

.header-rows {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 14px;
}

.header-row {
  display: grid;
  grid-template-columns: 28px minmax(0, 1fr) minmax(0, 1.4fr) 36px;
  gap: 8px;
  align-items: center;
}

.file-box {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 16px;
  padding: 12px;
  border: 1px solid #dbe4ef;
  border-radius: 8px;
  background: #f8fafc;
}

.file-box > div {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 4px;
}

.api-mock-error {
  margin: -6px 0 12px;
  font-size: 13px;
  color: #b91c1c;
}

.api-mock-empty {
  padding: 20px 10px;
  font-size: 13px;
  line-height: 1.6;
  color: #64748b;
}

.log-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.log-row {
  display: grid;
  grid-template-columns: 64px minmax(0, 1fr) 60px 80px minmax(120px, 0.8fr);
  gap: 10px;
  align-items: center;
  padding: 9px 10px;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  background: #f8fafc;
  font-size: 13px;
}

@media (max-width: 1180px) {
  .api-mock-panel {
    grid-template-columns: 240px minmax(0, 1fr);
  }

  .api-mock-routes {
    display: none;
  }
}

@media (max-width: 860px) {
  .api-mock-panel {
    grid-template-columns: 1fr;
  }

  .form-grid,
  .route-grid,
  .log-row {
    grid-template-columns: 1fr;
  }
}
</style>

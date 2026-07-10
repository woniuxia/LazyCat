<template>
  <div class="api-workbench-panel">
    <ApiWorkbenchSidebar
      ref="sidebarRef"
      :collections="collections"
      :selected-collection-id="selectedCollectionId"
      :selected-request-id="selectedRequestId"
      :loading="loading"
      @select-collection="selectCollection"
      @open-request="loadRequest"
      @command="handleSidebarCommand"
    />

    <main class="api-workbench-editor">
      <div class="api-workbench-compose">
        <div class="api-workbench-meta-row">
          <el-input v-model="requestName" class="request-name-input" placeholder="接口名称" />
          <div class="api-workbench-primary-actions">
            <el-select
              :model-value="selectedEnvironmentSelectValue"
              class="environment-select meta-environment-select"
              placeholder="环境"
              @update:model-value="handleEnvironmentSelect"
            >
              <el-option v-for="env in environments" :key="env.id" :label="env.name" :value="env.id" />
              <el-option
                class="api-workbench-environment-manage-option"
                :value="API_WORKBENCH_ENVIRONMENT_MANAGER_VALUE"
                label="环境管理"
              >
                <div class="environment-manage-option">
                  <el-icon><Setting /></el-icon>
                  <span>环境管理</span>
                </div>
              </el-option>
            </el-select>
          </div>
        </div>

        <div class="api-workbench-request-bar">
          <el-select v-model="draft.method" class="method-select">
            <el-option v-for="method in methods" :key="method" :label="method" :value="method">
              <span :class="getApiWorkbenchMethodClass(method)">{{ method }}</span>
            </el-option>
          </el-select>
          <el-input
            v-model="draft.url"
            class="request-url-input"
            placeholder="https://example.com/api 或 /api/users"
            @blur="applyUrlQuerySplit"
            @paste="handleUrlPaste"
          />
          <el-popover placement="bottom-end" :width="300" trigger="click">
            <template #reference>
              <el-button
                class="request-settings-button"
                :icon="Setting"
                title="请求设置"
                aria-label="请求设置"
              />
            </template>
            <div class="request-settings">
              <div class="request-settings-item">
                <span>超时（ms）</span>
                <el-input-number
                  v-model="draft.timeoutMs"
                  :min="1000"
                  :max="120000"
                  :step="1000"
                  size="small"
                />
              </div>
              <div class="request-settings-item">
                <span>跟随重定向</span>
                <el-switch v-model="draft.followRedirects" />
              </div>
              <p class="request-settings-note">301/302/303 按标准跟随；307/308 带请求体不跟随</p>
            </div>
          </el-popover>
          <el-button
            class="save-request-button"
            :icon="DocumentChecked"
            title="保存接口"
            aria-label="保存接口"
            @click="saveRequest"
          />
          <el-button
            class="send-button"
            type="primary"
            :icon="Promotion"
            :loading="sending"
            @click="sendRequest"
          >
            发送
          </el-button>
        </div>

        <div class="api-workbench-utility-row">
          <div class="utility-main">
            <button
              type="button"
              class="final-url-preview"
              :class="{ 'is-clickable': finalUrlPreview !== null }"
              :title="finalUrlPreview ? '点击复制最终 URL' : ''"
              @click="copyFinalUrlPreview"
            >
              <span class="final-url-label">最终 URL</span>
              <template v-if="finalUrlPreview">
                <span
                  v-for="(segment, index) in finalUrlPreview.segments"
                  :key="index"
                  :class="{ 'final-url-missing': segment.missing }"
                >{{ segment.text }}</span>
              </template>
              <span v-else class="final-url-placeholder">填写 URL 后显示最终请求地址</span>
            </button>
            <div v-if="variableUsages.length > 0" class="variable-summary">
              <el-tag
                v-for="usage in variableUsages"
                :key="usage.name"
                size="small"
                :type="usage.source === 'missing' ? 'warning' : usage.source === 'environment' ? 'success' : 'info'"
              >
                {{ usage.name }} · {{ variableSourceLabel(usage.source) }}
              </el-tag>
            </div>
          </div>
          <div class="curl-actions">
            <el-button :icon="CopyDocument" @click="copyCurrentCurl">复制 cURL</el-button>
          </div>
        </div>
      </div>

      <el-alert
        v-if="baseUrlError"
        type="warning"
        :title="baseUrlError"
        show-icon
        :closable="false"
      />

      <el-tabs v-model="editorTab" class="api-workbench-editor-tabs">
        <el-tab-pane name="query">
          <template #label>
            <span class="editor-tab-label">
              Query<span v-if="queryRowCount > 0" class="editor-tab-badge">({{ queryRowCount }})</span>
            </span>
          </template>
          <ApiWorkbenchKeyValueEditor v-model="draft.query" variant="query" />
        </el-tab-pane>
        <el-tab-pane name="headers">
          <template #label>
            <span class="editor-tab-label">
              Headers<span v-if="headerRowCount > 0" class="editor-tab-badge">({{ headerRowCount }})</span>
            </span>
          </template>
          <ApiWorkbenchKeyValueEditor v-model="draft.headers" variant="headers" />
        </el-tab-pane>
        <el-tab-pane name="body">
          <template #label>
            <span class="editor-tab-label">
              Body<span v-if="bodyHasContent" class="editor-tab-badge">(·)</span>
            </span>
          </template>
          <div class="body-toolbar">
            <el-radio-group v-model="draft.bodyType">
              <el-radio-button label="none">none</el-radio-button>
              <el-radio-button label="json">json</el-radio-button>
              <el-radio-button label="text">text</el-radio-button>
              <el-radio-button label="form-urlencoded">form</el-radio-button>
            </el-radio-group>
            <div class="body-toolbar-actions">
              <template v-if="draft.bodyType === 'json'">
                <el-button size="small" @click="formatBodyJson">格式化</el-button>
                <el-button size="small" @click="minifyBodyJson">压缩</el-button>
              </template>
            </div>
          </div>
          <ApiWorkbenchKeyValueEditor v-if="draft.bodyType === 'form-urlencoded'" v-model="draft.form" variant="form" />
          <div v-else-if="draft.bodyType !== 'none'" class="body-monaco">
            <MonacoPane
              v-model="draft.body"
              :language="draft.bodyType === 'json' ? 'json' : 'plaintext'"
            />
          </div>
          <el-empty v-else description="无请求体" />
        </el-tab-pane>
      </el-tabs>
    </main>

    <section class="api-workbench-response">
      <div class="response-panel-heading">
        <strong>调试结果</strong>
        <div v-if="response" class="response-summary">
          <el-tag :type="getApiWorkbenchStatusTone(response.status, response.error)">
            {{ response.status ?? "ERR" }}
          </el-tag>
          <span>{{ formatDurationMs(response.durationMs) }}</span>
          <span>{{ formatByteSize(response.bodySize) }}</span>
        </div>
        <span v-else class="response-empty-status">未发送</span>
      </div>

      <el-tabs v-model="responseTab" class="api-workbench-response-tabs">
        <el-tab-pane label="响应" name="response">
          <ApiWorkbenchResponseViewer
            v-if="response"
            :response="response"
            @copy-body="copyResponseBody"
            @copy-url="copyFinalUrl"
            @save-example="saveCurrentResponseAsExample"
          />
          <el-empty v-else description="发送请求后查看响应" />
        </el-tab-pane>
        <el-tab-pane name="headers">
          <template #label>
            响应头{{ response && response.responseHeaders.length > 0 ? ` (${response.responseHeaders.length})` : "" }}
          </template>
          <div class="response-actions response-actions-header">
            <el-button
              size="small"
              :icon="CopyDocument"
              :disabled="!response"
              @click="copyResponseHeaders"
            >
              复制响应头
            </el-button>
          </div>
          <el-empty
            v-if="!response || response.responseHeaders.length === 0"
            description="暂无响应头"
          />
          <div v-else class="headers-table">
            <div
              v-for="(row, index) in response.responseHeaders"
              :key="`${row.key}-${index}`"
              class="headers-table-row"
            >
              <span class="headers-table-key">{{ row.key }}</span>
              <span class="headers-table-value">{{ row.value }}</span>
              <el-button
                class="headers-table-copy"
                size="small"
                text
                :icon="CopyDocument"
                @click="copyText(row.value, '响应头值已复制')"
              >
                复制值
              </el-button>
            </div>
          </div>
        </el-tab-pane>
        <el-tab-pane label="历史" name="history">
          <div class="history-toolbar">
            <el-input
              v-model="historyQuery"
              size="small"
              clearable
              placeholder="搜索历史"
              @keyup.enter="loadHistory"
              @clear="loadHistory"
            />
            <el-radio-group v-model="historyPinnedOnly" size="small" @change="loadHistory">
              <el-radio-button :label="false">全部</el-radio-button>
              <el-radio-button :label="true">标星</el-radio-button>
            </el-radio-group>
            <el-button size="small" :disabled="!history.length" @click="clearHistory">清理</el-button>
          </div>
          <div class="history-list" v-loading="historyLoading">
            <div
              v-for="item in history"
              :key="item.id"
              class="history-item"
            >
              <div class="history-main" @click="loadHistoryIntoTemporaryEditor(item)">
                <strong :class="getApiWorkbenchMethodClass(item.method)">{{ item.method }}</strong>
                <span>{{ defaultApiWorkbenchHistoryDisplayName(item) }}</span>
                <small>
                  <span :class="`history-status history-status-${getApiWorkbenchStatusTone(item.status, item.error)}`">
                    {{ item.status ?? "ERR" }}
                  </span>
                  · {{ formatDurationMs(item.durationMs) }} · {{ item.hasRequestSnapshot ? "完整快照" : "摘要历史" }}
                </small>
              </div>
              <div class="history-actions">
                <el-button size="small" text :icon="Star" @click.stop="toggleHistoryPinned(item)">
                  {{ item.pinned ? "取消标星" : "标星" }}
                </el-button>
                <el-button
                  size="small"
                  :icon="Refresh"
                  :loading="replayingHistoryId === item.id"
                  :disabled="!canReplayApiWorkbenchHistory(item)"
                  @click.stop="replayHistory(item)"
                >
                  重放
                </el-button>
                <el-button size="small" @click.stop="loadHistoryIntoTemporaryEditor(item)">
                  载入
                </el-button>
                <el-button size="small" @click.stop="saveHistoryAsRequest(item)">
                  保存为接口
                </el-button>
                <el-button size="small" @click.stop="editHistoryMeta(item)">备注</el-button>
              </div>
            </div>
          </div>
        </el-tab-pane>
      </el-tabs>
    </section>

    <el-dialog
      v-model="environmentDialogVisible"
      title="环境管理"
      width="min(880px, calc(100vw - 32px))"
      class="api-workbench-environment-dialog"
      append-to-body
    >
      <div v-if="selectedCollectionId" class="environment-dialog-body">
        <header class="environment-dialog-overview">
          <div class="environment-current">
            <span>当前集合</span>
            <strong>{{ selectedCollection?.name ?? "未选择集合" }}</strong>
          </div>
          <div class="environment-overview-stats">
            <el-tag size="small" effect="plain">{{ environments.length }} 个环境</el-tag>
            <el-tag size="small" effect="plain">{{ environmentDraftSummary.variableCount }} 个变量</el-tag>
            <el-tag
              size="small"
              effect="plain"
              :type="environmentDraftSummary.hasBaseUrl ? 'success' : 'warning'"
            >
              BASE_URL {{ environmentDraftSummary.hasBaseUrl ? "已配置" : "未配置" }}
            </el-tag>
            <el-tag
              size="small"
              effect="plain"
              :type="environmentRowsDirty ? 'warning' : 'success'"
            >
              {{ environmentRowsDirty ? "有未保存修改" : "已保存" }}
            </el-tag>
          </div>
        </header>

        <div class="environment-manager-layout">
          <aside class="environment-list-panel" aria-label="环境列表">
            <div class="environment-list-heading">
              <span>环境</span>
              <el-button
                size="small"
                type="primary"
                plain
                :icon="Plus"
                :disabled="!selectedCollectionId"
                @click="createEnvironment"
              >
                新增
              </el-button>
            </div>
            <button
              v-for="env in environments"
              :key="env.id"
              type="button"
              class="environment-list-item"
              :class="{ active: env.id === selectedEnvironmentId }"
              @click="selectEnvironmentInDialog(env.id)"
            >
              <span class="environment-list-name">{{ env.name }}</span>
              <span class="environment-list-meta">{{ env.variables.length }} 个变量</span>
              <span v-if="env.id === selectedEnvironmentId" class="environment-list-badge">当前</span>
            </button>
          </aside>

          <section class="environment-editor-panel" aria-label="环境变量编辑">
            <div class="environment-editor-heading">
              <div class="environment-current">
                <span>当前环境</span>
                <strong>{{ selectedEnvironment?.name ?? "未选择环境" }}</strong>
              </div>
              <div class="environment-actions">
                <el-button
                  size="small"
                  :icon="CopyDocument"
                  :disabled="!selectedEnvironment"
                  @click="copyEnvironment"
                >
                  复制
                </el-button>
                <el-button
                  size="small"
                  :icon="EditPen"
                  :disabled="!selectedEnvironment"
                  @click="renameEnvironment"
                >
                  重命名
                </el-button>
                <el-button
                  size="small"
                  :icon="Delete"
                  :disabled="!selectedEnvironment"
                  @click="deleteEnvironment"
                >
                  删除
                </el-button>
              </div>
            </div>
            <ApiWorkbenchKeyValueEditor v-model="environmentRows" variant="env" />
          </section>
        </div>
      </div>
      <el-empty v-else description="请先选择集合" />
      <template #footer>
        <div class="environment-dialog-footer">
          <span class="environment-save-status" :class="{ warning: environmentRowsDirty || environmentDraftSummary.duplicateNames.length }">
            {{ environmentSaveStatusText }}
          </span>
          <div class="environment-footer-actions">
            <el-button @click="environmentDialogVisible = false">关闭</el-button>
            <el-button
              type="primary"
              :loading="savingEnvironment"
              :disabled="!selectedEnvironment || !environmentRowsDirty || environmentDraftSummary.duplicateNames.length > 0"
              @click="saveCurrentEnvironment"
            >
              保存环境
            </el-button>
          </div>
        </div>
      </template>
    </el-dialog>

    <el-dialog
      v-model="moveDialogVisible"
      :title="moveDialogTitle"
      width="360px"
      append-to-body
      @close="cancelMoveDialog"
    >
      <el-radio-group v-model="moveDialogSelectedKey" class="api-workbench-move-options">
        <el-radio
          v-for="target in moveDialogTargets"
          :key="moveTargetKey(target.folderId)"
          :label="moveTargetKey(target.folderId)"
        >
          <span :style="{ paddingLeft: target.depth * 12 + 'px' }">{{ target.label.trim() }}</span>
        </el-radio>
      </el-radio-group>
      <template #footer>
        <el-button @click="cancelMoveDialog">取消</el-button>
        <el-button type="primary" @click="confirmMoveDialog">移动</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import {
  CopyDocument,
  Delete,
  DocumentChecked,
  EditPen,
  Plus,
  Promotion,
  Refresh,
  Setting,
  Star,
} from "@element-plus/icons-vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import ApiWorkbenchKeyValueEditor from "./ApiWorkbenchKeyValueEditor.vue";
import ApiWorkbenchResponseViewer from "./ApiWorkbenchResponseViewer.vue";
import ApiWorkbenchSidebar from "./ApiWorkbenchSidebar.vue";
import MonacoPane from "./MonacoPane.vue";
import type {
  ApiWorkbenchCollection,
  ApiWorkbenchEnvironment,
  ApiWorkbenchHistoryDetail,
  ApiWorkbenchHistoryItem,
  ApiWorkbenchKeyValueRow,
  ApiWorkbenchListResult,
  ApiWorkbenchMoveTarget,
  ApiWorkbenchNavCommand,
  ApiWorkbenchNavTarget,
  ApiWorkbenchOrderDirection,
  ApiWorkbenchRequestDraft,
  ApiWorkbenchRequestDetail,
  ApiWorkbenchSendResult,
} from "../types/api-workbench";
import {
  API_WORKBENCH_ENVIRONMENT_MANAGER_VALUE,
  API_WORKBENCH_METHODS,
  DEFAULT_API_WORKBENCH_DRAFT,
  buildApiWorkbenchEnvironmentDraftSummary,
  buildApiWorkbenchNewRequestState,
  buildApiWorkbenchPreviewUrl,
  buildApiWorkbenchSelectionState,
  countApiWorkbenchActiveRows,
  draftApiWorkbenchEnvironmentRows,
  findDuplicateApiWorkbenchEnvironmentVariableNames,
  getApiWorkbenchMethodClass,
  getApiWorkbenchStatusTone,
  hasApiWorkbenchBody,
  normalizeApiWorkbenchDraft,
  resolveApiWorkbenchEnvironmentSelect,
  serializeApiWorkbenchEnvironmentRows,
  splitApiWorkbenchUrlQuery,
} from "../utils/apiWorkbench";
import {
  buildApiWorkbenchExampleResponse,
  buildApiWorkbenchResponseFromHistory,
  formatApiWorkbenchPreviewBody,
} from "../utils/apiWorkbenchResponsePreview";
import {
  buildApiWorkbenchDraftFromHistory,
  canReplayApiWorkbenchHistory,
  defaultApiWorkbenchHistoryDisplayName,
} from "../utils/apiWorkbenchHistory";
import { parseApiWorkbenchCurl } from "../utils/apiWorkbenchCurl";
import { formatByteSize, formatDurationMs } from "../utils/format";
import {
  resolveApiWorkbenchTemplate,
  summarizeApiWorkbenchVariables,
  type ApiWorkbenchVariableUsage,
} from "../utils/apiWorkbenchVariables";
import {
  buildApiWorkbenchFolderMoveTargets,
  buildApiWorkbenchRequestMoveTargets,
  getApiWorkbenchFolderAncestorIds,
  moveApiWorkbenchOrderedId,
} from "../utils/apiWorkbenchTree";

type ApiWorkbenchSidebarExpose = {
  expandFolder(folderId: number | null): void;
};

const methods = API_WORKBENCH_METHODS;
const sidebarRef = ref<ApiWorkbenchSidebarExpose | null>(null);
const loading = ref(false);
const sending = ref(false);
const savingEnvironment = ref(false);
const collections = ref<ApiWorkbenchCollection[]>([]);
const environments = ref<ApiWorkbenchEnvironment[]>([]);
const globalVariables = ref<ApiWorkbenchKeyValueRow[]>([]);
const history = ref<ApiWorkbenchHistoryItem[]>([]);
const sourceHistoryId = ref<number | null>(null);
const historyQuery = ref("");
const historyPinnedOnly = ref(false);
const historyLoading = ref(false);
const replayingHistoryId = ref<number | null>(null);
const selectedCollectionId = ref<number | null>(null);
const selectedEnvironmentId = ref<number | null>(null);
const selectedRequestId = ref<number | null>(null);
const selectedRequestFolderId = ref<number | null>(null);
const requestName = ref("");
const requestDescription = ref("");
const draft = ref({ ...DEFAULT_API_WORKBENCH_DRAFT });
const environmentRows = ref<ApiWorkbenchKeyValueRow[]>([]);
const response = ref<ApiWorkbenchSendResult | null>(null);
const editorTab = ref("query");
const responseTab = ref("response");
const environmentDialogVisible = ref(false);
const moveDialogVisible = ref(false);
const moveDialogTitle = ref("");
const moveDialogTargets = ref<ApiWorkbenchMoveTarget[]>([]);
const moveDialogSelectedKey = ref("__null__");
let moveDialogResolver: ((value: number | null | undefined) => void) | null = null;

const selectedCollection = computed(
  () => collections.value.find((item) => item.id === selectedCollectionId.value) ?? null,
);
const selectedEnvironment = computed(
  () => environments.value.find((item) => item.id === selectedEnvironmentId.value) ?? null,
);
const selectedEnvironmentSelectValue = computed(() => selectedEnvironmentId.value);
const environmentDraftSummary = computed(() =>
  buildApiWorkbenchEnvironmentDraftSummary(
    environmentRows.value,
    selectedEnvironment.value?.variables ?? [],
  ),
);
const environmentRowsDirty = computed(() => environmentDraftSummary.value.changed);
const environmentSaveStatusText = computed(() => {
  if (environmentDraftSummary.value.duplicateNames.length > 0) {
    return `变量名重复：${environmentDraftSummary.value.duplicateNames.join("、")}`;
  }
  return environmentRowsDirty.value ? "当前环境有未保存修改" : "当前环境已保存";
});
const baseUrl = computed(
  () => selectedEnvironment.value?.variables.find((item) => item.name === "BASE_URL")?.value ?? "",
);
const baseUrlError = computed(() => {
  if (/^https?:\/\//i.test(draft.value.url.trim())) return "";
  if (!draft.value.url.trim()) return "";
  return baseUrl.value.trim() ? "" : "相对 URL 需要当前环境配置 BASE_URL";
});
const variableUsages = computed(() =>
  summarizeApiWorkbenchVariables({
    draft: normalizeApiWorkbenchDraft(draft.value),
    environmentVariables: selectedEnvironment.value?.variables ?? [],
    globalVariables: globalVariables.value.map((row) => ({
      name: row.key,
      value: row.value,
      isSecret: false,
    })),
  }),
);
const finalUrlPreview = computed<{
  text: string;
  segments: Array<{ text: string; missing: boolean }>;
} | null>(() => {
  if (!draft.value.url.trim()) return null;
  const joined = buildApiWorkbenchPreviewUrl(baseUrl.value, draft.value.url, draft.value.query);
  const resolved = resolveApiWorkbenchTemplate(joined, [
    selectedEnvironment.value?.variables ?? [],
    globalVariables.value.map((row) => ({ name: row.key, value: row.value })),
  ]);
  const segments = resolved.text
    .split(/(\{\{\s*[^{}]+?\s*\}\})/g)
    .filter((part) => part !== "")
    .map((part) => ({ text: part, missing: /^\{\{[\s\S]*\}\}$/.test(part) }));
  return { text: resolved.text, segments };
});
const responseHeadersText = computed(
  () => response.value?.responseHeaders.map((row) => `${row.key}: ${row.value}`).join("\n") ?? "",
);
const queryRowCount = computed(() => countApiWorkbenchActiveRows(draft.value.query));
const headerRowCount = computed(() => countApiWorkbenchActiveRows(draft.value.headers));
const bodyHasContent = computed(() => hasApiWorkbenchBody(draft.value));

function resetRequestState() {
  selectedRequestId.value = null;
  selectedRequestFolderId.value = null;
  sourceHistoryId.value = null;
  requestName.value = "";
  requestDescription.value = "";
  draft.value = normalizeApiWorkbenchDraft({});
  response.value = null;
}

function startNewRequest(folderId: number | null) {
  if (!selectedCollectionId.value) {
    ElMessage.warning("请先选择集合");
    return;
  }
  const next = buildApiWorkbenchNewRequestState({ folderId });
  selectedRequestId.value = next.selectedRequestId;
  selectedRequestFolderId.value = next.selectedRequestFolderId;
  sourceHistoryId.value = null;
  requestName.value = next.requestName;
  requestDescription.value = next.requestDescription;
  draft.value = next.draft;
  response.value = next.response;
  editorTab.value = "query";
  if (folderId !== null) sidebarRef.value?.expandFolder(folderId);
}

function isMessageBoxCancel(error: unknown): boolean {
  return error === "cancel" || error === "close";
}

function errorMessage(error: unknown): string {
  const message = error instanceof Error ? error.message : typeof error === "string" ? error : "";
  if (
    message.includes(
      "UNIQUE constraint failed: api_workbench_environment_variables.environment_id, api_workbench_environment_variables.name",
    )
  ) {
    return "环境变量名称不能重复";
  }
  if (message) return message;
  return "操作失败";
}

function variableSourceLabel(source: ApiWorkbenchVariableUsage["source"]): string {
  if (source === "environment") return "当前环境";
  if (source === "global") return "全局";
  return "缺失";
}

async function copyText(text: string, successMessage: string) {
  await navigator.clipboard.writeText(text);
  ElMessage.success(successMessage);
}

function bySortThenId<T extends { sortOrder: number; id: number }>(a: T, b: T): number {
  return a.sortOrder - b.sortOrder || a.id - b.id;
}

function moveTargetKey(folderId: number | null): string {
  return folderId === null ? "__null__" : String(folderId);
}

function keyToMoveTarget(key: string): number | null {
  return key === "__null__" ? null : Number(key);
}

function curlPreviewText(nextDraft: ApiWorkbenchRequestDraft): string {
  const bodyLabel =
    nextDraft.bodyType === "none"
      ? "无"
      : nextDraft.bodyType === "form-urlencoded"
        ? `${nextDraft.bodyType} (${nextDraft.form.filter((row) => row.enabled && row.key.trim()).length} 项)`
        : nextDraft.bodyType;
  return [
    `Method：${nextDraft.method}`,
    `URL：${nextDraft.url}`,
    `Query：${nextDraft.query.filter((row) => row.enabled && row.key.trim()).length} 项`,
    `Headers：${nextDraft.headers.filter((row) => row.enabled && row.key.trim()).length} 项`,
    `Body：${bodyLabel}`,
  ].join("\n");
}

function chooseMoveTarget(
  title: string,
  targets: ApiWorkbenchMoveTarget[],
): Promise<number | null | undefined> {
  if (targets.length === 0) return Promise.resolve(undefined);
  moveDialogTitle.value = title;
  moveDialogTargets.value = targets;
  moveDialogSelectedKey.value = moveTargetKey(targets[0].folderId);
  moveDialogVisible.value = true;
  return new Promise((resolve) => {
    moveDialogResolver = resolve;
  });
}

function settleMoveDialog(value: number | null | undefined) {
  const resolve = moveDialogResolver;
  moveDialogResolver = null;
  moveDialogVisible.value = false;
  if (resolve) resolve(value);
}

function confirmMoveDialog() {
  settleMoveDialog(keyToMoveTarget(moveDialogSelectedKey.value));
}

function cancelMoveDialog() {
  if (!moveDialogResolver) return;
  settleMoveDialog(undefined);
}

async function copyFinalUrlPreview() {
  if (!finalUrlPreview.value) return;
  await copyText(finalUrlPreview.value.text, "最终 URL 已复制");
}

function formatBodyJson() {
  try {
    draft.value.body = JSON.stringify(JSON.parse(draft.value.body), null, 2);
  } catch (error) {
    ElMessage.error(`JSON 格式化失败：${error instanceof Error ? error.message : String(error)}`);
  }
}

function minifyBodyJson() {
  try {
    draft.value.body = JSON.stringify(JSON.parse(draft.value.body));
  } catch (error) {
    ElMessage.error(`JSON 压缩失败：${error instanceof Error ? error.message : String(error)}`);
  }
}

function applyUrlQuerySplit() {
  const result = splitApiWorkbenchUrlQuery(draft.value.url);
  if (!result) return;
  draft.value.url = result.url;
  draft.value.query.push(...result.rows);
  ElMessage.success(`已拆分 ${result.rows.length} 个参数到 Query`);
}

function handleUrlPaste() {
  void nextTick().then(applyUrlQuerySplit);
}

function handleWorkbenchKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && moveDialogVisible.value) {
    event.preventDefault();
    cancelMoveDialog();
    return;
  }
  if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
    event.preventDefault();
    void sendRequest();
    return;
  }
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
    event.preventDefault();
    void saveRequest();
  }
}

async function loadAll() {
  loading.value = true;
  try {
    const result = (await invokeToolByChannel(
      "tool:api-workbench:list",
      {},
    )) as ApiWorkbenchListResult;
    const globals = (await invokeToolByChannel("tool:api-workbench:global-variables-list", {})) as {
      items: Array<{ name: string; value: string }>;
    };
    collections.value = result.collections ?? [];
    globalVariables.value = (globals.items ?? []).map((item) => ({
      enabled: true,
      key: item.name,
      value: item.value,
    }));
    history.value = result.history ?? [];
    if (!selectedCollectionId.value && collections.value.length > 0) {
      await selectCollection(collections.value[0].id);
    }
  } finally {
    loading.value = false;
  }
}

async function loadHistory() {
  historyLoading.value = true;
  try {
    const result = (await invokeToolByChannel("tool:api-workbench:history-list", {
      query: historyQuery.value,
      pinnedOnly: historyPinnedOnly.value,
      limit: 200,
    })) as { items: ApiWorkbenchHistoryItem[] };
    history.value = result.items ?? [];
  } finally {
    historyLoading.value = false;
  }
}

async function selectCollection(id: number) {
  if (selectedCollectionId.value === id) return;
  const collection = collections.value.find((item) => item.id === id);
  const nextState = buildApiWorkbenchSelectionState({
    nextCollection: collection ?? null,
  });
  selectedCollectionId.value = nextState.selectedCollectionId;
  selectedEnvironmentId.value = nextState.selectedEnvironmentId;
  selectedRequestId.value = nextState.selectedRequestId;
  selectedRequestFolderId.value = null;
  sourceHistoryId.value = null;
  requestName.value = nextState.requestName;
  requestDescription.value = "";
  draft.value = nextState.draft;
  response.value = nextState.response;
  await refreshEnvironments(id);
}

async function createCollection() {
  const { value } = await ElMessageBox.prompt("集合名称", "新建接口集合", {
    inputValue: "默认集合",
    confirmButtonText: "创建",
    cancelButtonText: "取消",
  });
  const created = (await invokeToolByChannel("tool:api-workbench:collection-create", {
    name: value,
    description: "",
  })) as { id: number; activeEnvironmentId: number };
  await loadAll();
  await selectCollection(created.id);
}

async function promptName(title: string, inputValue: string): Promise<string> {
  const { value } = await ElMessageBox.prompt("名称", title, {
    inputValue,
    confirmButtonText: "确定",
    cancelButtonText: "取消",
    inputValidator: (input: string) => (input.trim() ? true : "名称不能为空"),
  });
  return value.trim();
}

function currentCollection(): ApiWorkbenchCollection | null {
  return selectedCollection.value;
}

async function createFolder(parentId: number | null) {
  if (!selectedCollectionId.value) {
    ElMessage.warning("请先选择集合");
    return;
  }
  const name = await promptName(parentId === null ? "新建根文件夹" : "新建子文件夹", "新建文件夹");
  await invokeToolByChannel("tool:api-workbench:folder-create", {
    collectionId: selectedCollectionId.value,
    parentId,
    name,
  });
  await loadAll();
  if (parentId !== null) sidebarRef.value?.expandFolder(parentId);
  ElMessage.success("文件夹已创建");
}

async function renameCollection(collectionId: number) {
  const collection = collections.value.find((item) => item.id === collectionId);
  if (!collection) {
    ElMessage.warning("集合不存在");
    return;
  }
  const name = await promptName("重命名集合", collection.name);
  await invokeToolByChannel("tool:api-workbench:collection-update", {
    id: collectionId,
    name,
    description: collection.description,
  });
  await loadAll();
  ElMessage.success("集合已重命名");
}

async function deleteCollection(collectionId: number) {
  const collection = collections.value.find((item) => item.id === collectionId);
  if (!collection) {
    ElMessage.warning("集合不存在");
    return;
  }
  await ElMessageBox.confirm(
    `确定删除集合「${collection.name}」？集合内接口、文件夹和环境会一起删除。`,
    "删除集合",
    { type: "warning", confirmButtonText: "删除", cancelButtonText: "取消" },
  );
  const wasSelected = selectedCollectionId.value === collectionId;
  await invokeToolByChannel("tool:api-workbench:collection-delete", { id: collectionId });
  if (wasSelected) {
    selectedCollectionId.value = null;
    selectedEnvironmentId.value = null;
    environments.value = [];
    resetRequestState();
  }
  await loadAll();
  ElMessage.success("集合已删除");
}

async function renameFolder(folderId: number) {
  const collection = currentCollection();
  const folder = collection?.folders.find((item) => item.id === folderId);
  if (!folder) {
    ElMessage.warning("文件夹不存在");
    return;
  }
  const name = await promptName("重命名文件夹", folder.name);
  await invokeToolByChannel("tool:api-workbench:folder-update", { id: folderId, name });
  await loadAll();
  ElMessage.success("文件夹已重命名");
}

async function deleteFolder(folderId: number) {
  const collection = currentCollection();
  const folder = collection?.folders.find((item) => item.id === folderId);
  if (!collection || !folder) {
    ElMessage.warning("文件夹不存在");
    return;
  }
  await ElMessageBox.confirm(
    `确定删除文件夹「${folder.name}」？内部接口会移动到未分组，子文件夹会删除。`,
    "删除文件夹",
    { type: "warning", confirmButtonText: "删除", cancelButtonText: "取消" },
  );
  const openFolderId = selectedRequestFolderId.value;
  const openRequestInsideDeletedFolder =
    openFolderId !== null &&
    (openFolderId === folderId ||
      getApiWorkbenchFolderAncestorIds(collection.folders, openFolderId).includes(folderId));
  await invokeToolByChannel("tool:api-workbench:folder-delete", { id: folderId });
  if (openRequestInsideDeletedFolder) selectedRequestFolderId.value = null;
  await loadAll();
  ElMessage.success("文件夹已删除，接口已移动到未分组");
}

async function loadRequestDetail(requestId: number): Promise<ApiWorkbenchRequestDetail> {
  return (await invokeToolByChannel("tool:api-workbench:request-get", {
    id: requestId,
  })) as ApiWorkbenchRequestDetail;
}

async function duplicateRequest(requestId: number) {
  const detail = await loadRequestDetail(requestId);
  const saved = (await invokeToolByChannel("tool:api-workbench:request-save", {
    id: null,
    collectionId: detail.collectionId,
    folderId: detail.folderId,
    name: `${detail.name} 副本`,
    description: detail.description,
    draft: normalizeApiWorkbenchDraft(detail.draft),
  })) as { id: number };
  await loadAll();
  await loadRequest(saved.id);
  ElMessage.success("已复制接口");
}

async function renameRequest(requestId: number) {
  const isCurrentOpen = selectedRequestId.value === requestId && selectedCollectionId.value !== null;
  const detail = isCurrentOpen ? null : await loadRequestDetail(requestId);
  const name = await promptName("重命名接口", isCurrentOpen ? requestName.value : detail?.name ?? "");
  await invokeToolByChannel("tool:api-workbench:request-save", {
    id: requestId,
    collectionId: isCurrentOpen ? selectedCollectionId.value : detail?.collectionId,
    folderId: isCurrentOpen ? selectedRequestFolderId.value : detail?.folderId,
    name,
    description: isCurrentOpen ? requestDescription.value : detail?.description ?? "",
    draft: isCurrentOpen
      ? normalizeApiWorkbenchDraft(draft.value)
      : normalizeApiWorkbenchDraft(detail?.draft),
  });
  if (isCurrentOpen) requestName.value = name;
  await loadAll();
  ElMessage.success("接口已重命名");
}

async function deleteRequest(requestId: number) {
  const summary = currentCollection()?.requests.find((item) => item.id === requestId);
  const name = summary?.name ?? (await loadRequestDetail(requestId)).name;
  await ElMessageBox.confirm(`确定删除接口「${name}」？历史记录不会删除。`, "删除接口", {
    type: "warning",
    confirmButtonText: "删除",
    cancelButtonText: "取消",
  });
  await invokeToolByChannel("tool:api-workbench:request-delete", { id: requestId });
  if (selectedRequestId.value === requestId) resetRequestState();
  await loadAll();
  ElMessage.success("接口已删除");
}

async function moveFolder(folderId: number) {
  const collection = currentCollection();
  if (!collection) return;
  const targetParentId = await chooseMoveTarget(
    "移动文件夹到",
    buildApiWorkbenchFolderMoveTargets(collection, folderId),
  );
  if (targetParentId === undefined) return;
  await invokeToolByChannel("tool:api-workbench:folder-move", { id: folderId, targetParentId });
  await loadAll();
  if (targetParentId !== null) sidebarRef.value?.expandFolder(targetParentId);
  ElMessage.success("文件夹已移动");
}

async function moveRequest(requestId: number) {
  const collection = currentCollection();
  if (!collection) return;
  const targetFolderId = await chooseMoveTarget(
    "移动接口到",
    buildApiWorkbenchRequestMoveTargets(collection),
  );
  if (targetFolderId === undefined) return;
  await invokeToolByChannel("tool:api-workbench:request-move", { id: requestId, targetFolderId });
  if (selectedRequestId.value === requestId) selectedRequestFolderId.value = targetFolderId;
  await loadAll();
  sidebarRef.value?.expandFolder(targetFolderId);
  ElMessage.success("接口已移动");
}

async function reorderFolder(folderId: number, direction: ApiWorkbenchOrderDirection) {
  const collection = currentCollection();
  const folder = collection?.folders.find((item) => item.id === folderId);
  if (!collection || !folder) return;
  const orderedIds = collection.folders
    .filter((item) => item.parentId === folder.parentId)
    .sort(bySortThenId)
    .map((item) => item.id);
  const next = moveApiWorkbenchOrderedId(orderedIds, folderId, direction);
  if (next.join(",") === orderedIds.join(",")) return;
  await invokeToolByChannel("tool:api-workbench:folder-reorder", {
    collectionId: collection.id,
    parentId: folder.parentId,
    orderedIds: next,
  });
  await loadAll();
  ElMessage.success("文件夹顺序已更新");
}

async function reorderRequest(requestId: number, direction: ApiWorkbenchOrderDirection) {
  const collection = currentCollection();
  const request = collection?.requests.find((item) => item.id === requestId);
  if (!collection || !request) return;
  const orderedIds = collection.requests
    .filter((item) => item.folderId === request.folderId)
    .sort(bySortThenId)
    .map((item) => item.id);
  const next = moveApiWorkbenchOrderedId(orderedIds, requestId, direction);
  if (next.join(",") === orderedIds.join(",")) return;
  await invokeToolByChannel("tool:api-workbench:request-reorder", {
    collectionId: collection.id,
    folderId: request.folderId,
    orderedIds: next,
  });
  await loadAll();
  ElMessage.success("接口顺序已更新");
}

async function handleSidebarCommand(command: ApiWorkbenchNavCommand, target: ApiWorkbenchNavTarget) {
  try {
    if (command === "collection:create") return await createCollection();
    if (command === "request:create" && target.type === "blank") return startNewRequest(null);
    if (command === "folder:create-root") return await createFolder(null);
    if (command === "request:import-curl") return await importCurl();
    if (command === "collection:export") {
      const collectionId = target.type === "blank" ? selectedCollectionId.value : target.collectionId;
      if (!collectionId) {
        ElMessage.warning("请先选择集合");
        return;
      }
      return await exportMarkdownForCollection(collectionId);
    }
    if (target.type === "collection") {
      if (command === "collection:select") return await selectCollection(target.collectionId);
      if (command === "request:create") {
        if (selectedCollectionId.value !== target.collectionId) {
          await selectCollection(target.collectionId);
        }
        return startNewRequest(null);
      }
      if (command === "collection:rename") return await renameCollection(target.collectionId);
      if (command === "collection:delete") return await deleteCollection(target.collectionId);
    }
    if (target.type === "folder") {
      if (command === "request:create") return startNewRequest(target.folderId);
      if (command === "folder:create-child") return await createFolder(target.folderId);
      if (command === "folder:rename") return await renameFolder(target.folderId);
      if (command === "folder:delete") return await deleteFolder(target.folderId);
      if (command === "folder:move") return await moveFolder(target.folderId);
      if (command === "folder:up") return await reorderFolder(target.folderId, "up");
      if (command === "folder:down") return await reorderFolder(target.folderId, "down");
    }
    if (target.type === "request") {
      if (command === "request:open") return await loadRequest(target.requestId);
      if (command === "request:duplicate") return await duplicateRequest(target.requestId);
      if (command === "request:rename") return await renameRequest(target.requestId);
      if (command === "request:delete") return await deleteRequest(target.requestId);
      if (command === "request:move") return await moveRequest(target.requestId);
      if (command === "request:up") return await reorderRequest(target.requestId, "up");
      if (command === "request:down") return await reorderRequest(target.requestId, "down");
    }
  } catch (error) {
    if (isMessageBoxCancel(error)) return;
    ElMessage.error(errorMessage(error));
  }
}

function syncEnvironmentRows() {
  environmentRows.value = draftApiWorkbenchEnvironmentRows(
    selectedEnvironment.value?.variables ?? [],
  );
}

function serializeEnvironmentRowsForSave() {
  const duplicateNames = findDuplicateApiWorkbenchEnvironmentVariableNames(environmentRows.value);
  if (duplicateNames.length > 0) {
    ElMessage.warning(`环境变量名称重复：${duplicateNames.join("、")}`);
    return null;
  }
  return serializeApiWorkbenchEnvironmentRows(environmentRows.value);
}

async function refreshEnvironments(collectionId: number) {
  const result = (await invokeToolByChannel("tool:api-workbench:environment-list", {
    collectionId,
  })) as { items: ApiWorkbenchEnvironment[] };
  environments.value = result.items ?? [];
  if (
    selectedEnvironmentId.value === null ||
    !environments.value.some((item) => item.id === selectedEnvironmentId.value)
  ) {
    selectedEnvironmentId.value = environments.value[0]?.id ?? null;
  }
  syncEnvironmentRows();
}

async function handleEnvironmentChange() {
  await persistActiveEnvironment();
  syncEnvironmentRows();
}

async function handleEnvironmentSelect(value: string | number | null) {
  const next = resolveApiWorkbenchEnvironmentSelect(value, selectedEnvironmentId.value);
  if (next.kind === "manage") {
    environmentDialogVisible.value = true;
    syncEnvironmentRows();
    return;
  }
  selectedEnvironmentId.value = next.environmentId;
  await handleEnvironmentChange();
}

async function confirmDiscardEnvironmentDraft() {
  if (!environmentRowsDirty.value) return true;
  try {
    await ElMessageBox.confirm(
      "当前环境有未保存修改，切换后会丢弃这些修改。",
      "切换环境",
      { type: "warning", confirmButtonText: "继续切换", cancelButtonText: "取消" },
    );
    return true;
  } catch (error) {
    if (isMessageBoxCancel(error)) return false;
    throw error;
  }
}

async function selectEnvironmentInDialog(environmentId: number) {
  if (environmentId === selectedEnvironmentId.value) return;
  try {
    const shouldSwitch = await confirmDiscardEnvironmentDraft();
    if (!shouldSwitch) return;
    const previousEnvironmentId = selectedEnvironmentId.value;
    selectedEnvironmentId.value = environmentId;
    try {
      await handleEnvironmentChange();
    } catch (error) {
      selectedEnvironmentId.value = previousEnvironmentId;
      syncEnvironmentRows();
      throw error;
    }
  } catch (error) {
    ElMessage.error(errorMessage(error));
  }
}

async function persistActiveEnvironment() {
  if (!selectedCollectionId.value || !selectedEnvironmentId.value) return;
  await invokeToolByChannel("tool:api-workbench:collection-set-active-environment", {
    collectionId: selectedCollectionId.value,
    environmentId: selectedEnvironmentId.value,
  });
}

async function saveCurrentEnvironment() {
  if (!selectedCollectionId.value || !selectedEnvironment.value) {
    ElMessage.warning("请先选择环境");
    return;
  }
  const variables = serializeEnvironmentRowsForSave();
  if (!variables) return;
  savingEnvironment.value = true;
  try {
    await invokeToolByChannel("tool:api-workbench:environment-save", {
      id: selectedEnvironment.value.id,
      collectionId: selectedCollectionId.value,
      name: selectedEnvironment.value.name,
      variables,
    });
    const result = (await invokeToolByChannel("tool:api-workbench:environment-list", {
      collectionId: selectedCollectionId.value,
    })) as { items: ApiWorkbenchEnvironment[] };
    environments.value = result.items ?? [];
    syncEnvironmentRows();
    ElMessage.success("环境已保存");
  } catch (error) {
    ElMessage.error(errorMessage(error));
  } finally {
    savingEnvironment.value = false;
  }
}

async function createEnvironment() {
  if (!selectedCollectionId.value) {
    ElMessage.warning("请先选择集合");
    return;
  }
  const variables = serializeEnvironmentRowsForSave();
  if (!variables) return;
  try {
    const { value } = await ElMessageBox.prompt("环境名称", "新增环境", {
      inputValue: "新环境",
      confirmButtonText: "创建",
      cancelButtonText: "取消",
      inputValidator: (input: string) => (input.trim() ? true : "环境名称不能为空"),
    });
    const created = (await invokeToolByChannel("tool:api-workbench:environment-save", {
      collectionId: selectedCollectionId.value,
      name: value.trim(),
      variables,
    })) as { id: number };
    selectedEnvironmentId.value = created.id;
    await persistActiveEnvironment();
    await refreshEnvironments(selectedCollectionId.value);
    ElMessage.success("环境已创建");
  } catch (error) {
    if (isMessageBoxCancel(error)) return;
    ElMessage.error(errorMessage(error));
  }
}

async function copyEnvironment() {
  if (!selectedCollectionId.value || !selectedEnvironment.value) return;
  const variables = serializeEnvironmentRowsForSave();
  if (!variables) return;
  try {
    const { value } = await ElMessageBox.prompt("环境名称", "复制环境", {
      inputValue: `${selectedEnvironment.value.name} 副本`,
      confirmButtonText: "复制",
      cancelButtonText: "取消",
      inputValidator: (input: string) => (input.trim() ? true : "环境名称不能为空"),
    });
    const created = (await invokeToolByChannel("tool:api-workbench:environment-save", {
      collectionId: selectedCollectionId.value,
      name: value.trim(),
      variables,
    })) as { id: number };
    selectedEnvironmentId.value = created.id;
    await persistActiveEnvironment();
    await refreshEnvironments(selectedCollectionId.value);
    ElMessage.success("环境已复制");
  } catch (error) {
    if (isMessageBoxCancel(error)) return;
    ElMessage.error(errorMessage(error));
  }
}

async function renameEnvironment() {
  if (!selectedCollectionId.value || !selectedEnvironment.value) return;
  const variables = serializeEnvironmentRowsForSave();
  if (!variables) return;
  try {
    const { value } = await ElMessageBox.prompt("环境名称", "重命名环境", {
      inputValue: selectedEnvironment.value.name,
      confirmButtonText: "保存",
      cancelButtonText: "取消",
      inputValidator: (input: string) => (input.trim() ? true : "环境名称不能为空"),
    });
    await invokeToolByChannel("tool:api-workbench:environment-save", {
      id: selectedEnvironment.value.id,
      collectionId: selectedCollectionId.value,
      name: value.trim(),
      variables,
    });
    await refreshEnvironments(selectedCollectionId.value);
    ElMessage.success("环境已重命名");
  } catch (error) {
    if (isMessageBoxCancel(error)) return;
    ElMessage.error(errorMessage(error));
  }
}

async function deleteEnvironment() {
  if (!selectedCollectionId.value || !selectedEnvironment.value) return;
  try {
    await ElMessageBox.confirm(
      `确定删除环境「${selectedEnvironment.value.name}」？`,
      "删除环境",
      { type: "warning", confirmButtonText: "删除", cancelButtonText: "取消" },
    );
    const result = (await invokeToolByChannel("tool:api-workbench:environment-delete", {
      id: selectedEnvironment.value.id,
    })) as { activeEnvironmentId?: number };
    selectedEnvironmentId.value = result.activeEnvironmentId ?? null;
    await refreshEnvironments(selectedCollectionId.value);
    await loadAll();
    ElMessage.success("环境已删除");
  } catch (error) {
    if (isMessageBoxCancel(error)) return;
    ElMessage.error(errorMessage(error));
  }
}

async function importCurl() {
  try {
    const { value } = await ElMessageBox.prompt("粘贴 cURL 命令", "导入 cURL", {
      inputType: "textarea",
      inputValue: "curl ",
      confirmButtonText: "导入",
      cancelButtonText: "取消",
      inputValidator: (input: string) => (input.trim() ? true : "请输入 cURL 命令"),
    });
    const result = parseApiWorkbenchCurl(value);
    await ElMessageBox.confirm(curlPreviewText(result.draft), "确认导入 cURL", {
      confirmButtonText: "覆盖当前草稿",
      cancelButtonText: "取消",
    });
    draft.value = result.draft;
    response.value = null;
    if (result.warnings.length > 0) {
      ElMessage.warning(result.warnings.join("；"));
    } else {
      ElMessage.success("已导入 cURL");
    }
  } catch (error) {
    if (isMessageBoxCancel(error)) return;
    ElMessage.error(errorMessage(error));
  }
}

async function copyCurrentCurl() {
  if (!selectedCollectionId.value || !selectedEnvironmentId.value) {
    ElMessage.warning("请先选择集合和环境");
    return;
  }
  try {
    const result = (await invokeToolByChannel("tool:api-workbench:export-curl", {
      collectionId: selectedCollectionId.value,
      environmentId: selectedEnvironmentId.value,
      targetShell: "powershell",
      draft: normalizeApiWorkbenchDraft(draft.value),
    })) as { shell: "powershell" | "bash"; command: string };
    await copyText(result.command, "cURL 已复制");
  } catch (error) {
    ElMessage.error(errorMessage(error));
  }
}

async function copyResponseBody() {
  if (!response.value) return;
  await copyText(formatApiWorkbenchPreviewBody(response.value), "响应体已复制");
}

async function copyResponseHeaders() {
  if (!response.value) return;
  await copyText(responseHeadersText.value, "响应头已复制");
}

async function copyFinalUrl() {
  if (!response.value) return;
  await copyText(response.value.finalUrl, "最终 URL 已复制");
}

async function saveCurrentResponseAsExample() {
  if (!response.value) {
    ElMessage.warning("请先发送请求");
    return;
  }
  if (!selectedCollectionId.value || !selectedRequestId.value) {
    ElMessage.warning("请先保存接口，再保存示例响应");
    return;
  }
  try {
    await invokeToolByChannel("tool:api-workbench:request-save-example-response", {
      requestId: selectedRequestId.value,
      collectionId: selectedCollectionId.value,
      response: {
        ...buildApiWorkbenchExampleResponse(response.value),
      },
    });
    ElMessage.success("示例响应已保存");
  } catch (error) {
    ElMessage.error(errorMessage(error));
  }
}

async function saveHistoryAsRequest(item: ApiWorkbenchHistoryItem) {
  const collectionId = item.collectionId ?? selectedCollectionId.value;
  if (!collectionId) {
    ElMessage.warning("请先选择目标集合");
    return;
  }
  const collection = collections.value.find((entry) => entry.id === collectionId);
  if (!collection) {
    ElMessage.warning("目标集合不存在，请刷新后重试");
    return;
  }
  const folderId = await chooseMoveTarget(
    "保存历史到",
    buildApiWorkbenchRequestMoveTargets(collection),
  );
  if (folderId === undefined) return;
  const saved = (await invokeToolByChannel("tool:api-workbench:history-save-request", {
    historyId: item.id,
    collectionId,
    folderId,
    name: defaultApiWorkbenchHistoryDisplayName(item),
  })) as { id: number };
  await loadAll();
  if (selectedCollectionId.value !== collectionId) {
    await selectCollection(collectionId);
  }
  await loadRequest(saved.id);
  ElMessage.success("历史已保存为接口");
}

async function sendRequest() {
  if (!selectedEnvironmentId.value) {
    ElMessage.warning("请先选择环境");
    return;
  }
  if (baseUrlError.value) {
    ElMessage.warning(baseUrlError.value);
    return;
  }
  sending.value = true;
  try {
    const normalized = normalizeApiWorkbenchDraft(draft.value);
    response.value = (await invokeToolByChannel("tool:api-workbench:send", {
      collectionId: selectedCollectionId.value,
      environmentId: selectedEnvironmentId.value,
      requestId: selectedRequestId.value,
      name: requestName.value,
      draft: normalized,
    })) as ApiWorkbenchSendResult;
    await loadHistory();
  } finally {
    sending.value = false;
  }
}

async function saveRequest() {
  if (!selectedCollectionId.value) {
    ElMessage.warning("请先选择集合");
    return;
  }
  if (!requestName.value.trim()) {
    ElMessage.warning("请填写接口名称");
    return;
  }
  const saved = (await invokeToolByChannel("tool:api-workbench:request-save", {
    id: selectedRequestId.value,
    collectionId: selectedCollectionId.value,
    folderId: selectedRequestFolderId.value,
    name: requestName.value.trim(),
    description: requestDescription.value,
    draft: normalizeApiWorkbenchDraft(draft.value),
  })) as { id: number };
  selectedRequestId.value = saved.id;
  sourceHistoryId.value = null;
  await loadAll();
  ElMessage.success("已保存接口");
}

async function loadRequest(id: number) {
  const detail = (await invokeToolByChannel("tool:api-workbench:request-get", {
    id,
  })) as ApiWorkbenchRequestDetail;
  selectedRequestId.value = detail.id;
  selectedRequestFolderId.value = detail.folderId;
  sourceHistoryId.value = null;
  requestName.value = detail.name;
  requestDescription.value = detail.description;
  draft.value = normalizeApiWorkbenchDraft(detail.draft);
  sidebarRef.value?.expandFolder(detail.folderId);
}

async function exportMarkdownForCollection(collectionId: number) {
  const result = (await invokeToolByChannel("tool:api-workbench:export-markdown", {
    collectionId,
  })) as { fileName: string; markdown: string };
  await navigator.clipboard.writeText(result.markdown);
  ElMessage.success(`Markdown 已复制：${result.fileName}`);
}

async function loadHistoryIntoTemporaryEditor(item: ApiWorkbenchHistoryItem) {
  const detail = (await invokeToolByChannel("tool:api-workbench:history-get", {
    historyId: item.id,
  })) as ApiWorkbenchHistoryDetail;
  const { draft: nextDraft, degraded } = buildApiWorkbenchDraftFromHistory(detail);
  if (sourceHistoryId.value !== null) {
    await ElMessageBox.confirm("当前临时接口草稿会被历史记录覆盖，是否继续？", "载入历史", {
      type: "warning",
    });
  }
  selectedRequestId.value = null;
  selectedRequestFolderId.value =
    detail.collectionId === selectedCollectionId.value ? selectedRequestFolderId.value : null;
  sourceHistoryId.value = detail.id;
  requestName.value = defaultApiWorkbenchHistoryDisplayName(detail);
  requestDescription.value = "";
  draft.value = nextDraft;
  response.value = buildApiWorkbenchResponseFromHistory(detail);
  responseTab.value = "response";
  if (degraded) {
    ElMessage.warning("旧历史仅包含摘要，已恢复 Method 和 URL");
  }
}

async function replayHistory(item: ApiWorkbenchHistoryItem) {
  if (!canReplayApiWorkbenchHistory(item)) {
    ElMessage.warning("旧历史缺少执行快照，请载入后手动发送");
    return;
  }
  replayingHistoryId.value = item.id;
  try {
    response.value = (await invokeToolByChannel("tool:api-workbench:history-replay", {
      historyId: item.id,
    })) as ApiWorkbenchSendResult;
    responseTab.value = "response";
    await loadHistory();
  } finally {
    replayingHistoryId.value = null;
  }
}

async function toggleHistoryPinned(item: ApiWorkbenchHistoryItem) {
  await invokeToolByChannel("tool:api-workbench:history-update", {
    id: item.id,
    name: item.name,
    note: item.note,
    pinned: !item.pinned,
  });
  await loadHistory();
}

async function editHistoryMeta(item: ApiWorkbenchHistoryItem) {
  const nameResult = await ElMessageBox.prompt("历史名称可留空，留空时按 Method 和路径展示", "编辑历史名称", {
    inputValue: item.name,
    inputPlaceholder: defaultApiWorkbenchHistoryDisplayName(item),
  });
  const noteResult = await ElMessageBox.prompt("备注最多 2000 字", "编辑历史备注", {
    inputValue: item.note,
    inputType: "textarea",
  });
  await invokeToolByChannel("tool:api-workbench:history-update", {
    id: item.id,
    name: String(nameResult.value ?? ""),
    note: String(noteResult.value ?? ""),
    pinned: item.pinned,
  });
  await loadHistory();
}

async function clearHistory() {
  const includePinned = await ElMessageBox.confirm(
    "默认只清空非标星历史。是否同时清空标星历史？",
    "清空历史",
    {
      confirmButtonText: "清空全部",
      cancelButtonText: "仅清空非标星",
      distinguishCancelAndClose: true,
      type: "warning",
    },
  )
    .then(() => true)
    .catch((action) => {
      if (action === "cancel") return false;
      throw action;
    });
  await invokeToolByChannel("tool:api-workbench:history-clear", { includePinned });
  await loadHistory();
}

onMounted(() => {
  window.addEventListener("keydown", handleWorkbenchKeydown);
  void loadAll();
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleWorkbenchKeydown);
});
</script>

<style scoped>
/* Method 色板为全局类（供下拉 popper 与子组件共用），定义见文件尾部非 scoped 样式块 */
.api-workbench-panel {
  display: grid;
  grid-template-columns: 272px minmax(460px, 1fr) minmax(360px, 40%);
  gap: 14px;
  height: 100%;
  min-height: 0;
  padding: 12px;
  background: var(--el-bg-color-page);
}

.api-workbench-editor,
.api-workbench-response {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  background: var(--el-bg-color);
  padding: 12px;
}

.api-workbench-editor {
  gap: 12px;
}

.api-workbench-compose {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: 10px;
}

.api-workbench-meta-row {
  display: grid;
  grid-template-columns: minmax(180px, 1fr) auto;
  gap: 8px;
  align-items: center;
}

.api-workbench-request-bar,
.body-toolbar,
.environment-actions,
.environment-dialog-footer,
.environment-editor-heading,
.environment-footer-actions,
.environment-overview-stats {
  display: flex;
  align-items: center;
  gap: 8px;
}

.api-workbench-request-bar {
  display: grid;
  grid-template-columns: 104px minmax(240px, 1fr) 32px auto;
}

.api-workbench-primary-actions,
.curl-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}

.api-workbench-utility-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
  min-height: 32px;
}

.environment-dialog-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.environment-dialog-overview {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 12px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  background: var(--el-fill-color-extra-light);
}

.environment-overview-stats {
  flex-wrap: wrap;
  justify-content: flex-end;
}

.environment-manager-layout {
  display: grid;
  min-height: 360px;
  grid-template-columns: 220px minmax(0, 1fr);
  gap: 12px;
}

.environment-list-panel,
.environment-editor-panel {
  min-width: 0;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  background: var(--el-bg-color);
}

.environment-list-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px;
  background: var(--el-fill-color-extra-light);
}

.environment-list-heading {
  display: flex;
  min-height: 32px;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.environment-list-heading span {
  color: var(--el-text-color-primary);
  font-weight: 600;
}

.environment-list-item {
  display: grid;
  width: 100%;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 2px 8px;
  align-items: center;
  padding: 9px 10px;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: var(--el-text-color-primary);
  cursor: pointer;
  font: inherit;
  text-align: left;
  transition:
    border-color 0.18s ease,
    background-color 0.18s ease,
    color 0.18s ease;
}

.environment-list-item:hover {
  border-color: var(--el-border-color);
  background: var(--el-bg-color);
}

.environment-list-item:focus-visible {
  outline: 2px solid var(--el-color-primary);
  outline-offset: 2px;
}

.environment-list-item.active {
  border-color: var(--el-color-primary-light-5);
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
}

.environment-list-name {
  min-width: 0;
  overflow: hidden;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.environment-list-meta {
  min-width: 0;
  overflow: hidden;
  color: var(--el-text-color-secondary);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.environment-list-item.active .environment-list-meta {
  color: var(--el-color-primary);
}

.environment-list-badge {
  grid-row: 1 / span 2;
  grid-column: 2;
  color: var(--el-color-primary);
  font-size: 12px;
  font-weight: 600;
}

.environment-editor-panel {
  display: flex;
  min-height: 0;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
}

.environment-editor-heading {
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
}

.environment-current {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
}

.environment-current span {
  color: var(--el-text-color-secondary);
  font-size: 12px;
}

.environment-current strong {
  min-width: 0;
  overflow: hidden;
  color: var(--el-text-color-primary);
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.environment-actions {
  flex-wrap: wrap;
  justify-content: flex-end;
}

.environment-dialog-footer {
  width: 100%;
  justify-content: space-between;
  gap: 12px;
}

.environment-save-status {
  min-width: 0;
  overflow: hidden;
  color: var(--el-text-color-secondary);
  font-size: 13px;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.environment-save-status.warning {
  color: var(--el-color-warning);
}

.environment-footer-actions {
  flex: none;
  justify-content: flex-end;
}

:global(.api-workbench-environment-manage-option) {
  margin-top: 4px;
  border-top: 1px solid var(--el-border-color-lighter);
}

:global(.api-workbench-environment-manage-option .environment-manage-option) {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--el-color-primary);
}

.variable-summary {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  padding-top: 2px;
}

.utility-main {
  display: flex;
  min-width: 0;
  flex: 1;
  flex-direction: column;
  gap: 4px;
}

.final-url-preview {
  display: block;
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--el-text-color-secondary);
  cursor: default;
  font-family: var(--lc-font-mono);
  font-size: 12px;
  line-height: 1.6;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.final-url-preview.is-clickable {
  cursor: pointer;
}

.final-url-preview.is-clickable:hover {
  color: var(--el-text-color-primary);
}

.final-url-label {
  margin-right: 6px;
  color: var(--el-text-color-placeholder);
  font-family: inherit;
  font-weight: 600;
}

.final-url-missing {
  color: var(--el-color-warning);
  font-weight: 600;
}

.final-url-placeholder {
  color: var(--el-text-color-placeholder);
}

.method-select {
  width: 104px;
  flex: none;
}

.environment-select {
  width: 100%;
  flex: none;
}

.meta-environment-select {
  width: 180px;
}

.request-name-input,
.request-url-input {
  min-width: 0;
}

.send-button {
  min-width: 92px;
}

.save-request-button {
  width: 32px;
  padding-right: 0;
  padding-left: 0;
}

.api-workbench-editor-tabs,
.api-workbench-response-tabs {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
}

.api-workbench-editor-tabs :deep(.el-tabs__content),
.api-workbench-response-tabs :deep(.el-tabs__content) {
  min-height: 0;
  flex: 1;
  overflow: auto;
}

.api-workbench-editor-tabs :deep(.el-tab-pane),
.api-workbench-response-tabs :deep(.el-tab-pane) {
  min-height: 100%;
}

.api-workbench-editor-tabs :deep(.el-tabs__header),
.api-workbench-response-tabs :deep(.el-tabs__header) {
  flex: none;
  margin-bottom: 10px;
}

.editor-tab-label {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.editor-tab-badge {
  color: var(--el-text-color-secondary);
  font-size: 12px;
}

.el-tabs__item.is-active .editor-tab-badge {
  color: inherit;
}

.body-toolbar {
  justify-content: space-between;
  margin-bottom: 8px;
}

.body-toolbar-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.body-monaco {
  height: 280px;
  min-height: 280px;
  flex: none;
}

.body-toolbar :deep(.el-radio-group) {
  flex-wrap: wrap;
}

.response-panel-heading {
  display: flex;
  min-height: 32px;
  flex: none;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 6px;
}

.response-panel-heading strong {
  color: var(--el-text-color-primary);
  font-weight: 600;
}

.response-summary {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  color: var(--el-text-color-secondary);
  font-size: 12px;
}

.response-empty-status {
  color: var(--el-text-color-secondary);
  font-size: 12px;
}

.history-status {
  font-weight: 600;
}

.history-status-success {
  color: var(--el-color-success);
}

.history-status-warning {
  color: var(--el-color-warning);
}

.history-status-danger {
  color: var(--el-color-danger);
}

.history-status-info {
  color: var(--el-text-color-secondary);
}

.response-toolbar {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  margin-bottom: 8px;
}

.response-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
}

.response-actions-header {
  margin-bottom: 8px;
}

.headers-table {
  min-height: 180px;
  max-height: 48vh;
  overflow: auto;
  border: 1px solid var(--el-border-color-extra-light);
  border-radius: 6px;
  background: var(--el-fill-color-blank);
  font-size: 12px;
}

.headers-table-row {
  display: grid;
  grid-template-columns: minmax(140px, 220px) minmax(0, 1fr) auto;
  align-items: start;
  gap: 8px;
  border-bottom: 1px solid var(--el-border-color-extra-light);
  padding: 6px 10px;
}

.headers-table-row:last-child {
  border-bottom: none;
}

.headers-table-key {
  color: var(--el-text-color-primary);
  font-family: var(--lc-font-mono);
  font-weight: 600;
  word-break: break-all;
}

.headers-table-value {
  color: var(--el-text-color-regular);
  font-family: var(--lc-font-mono);
  overflow-wrap: anywhere;
}

.headers-table-copy {
  visibility: hidden;
}

.headers-table-row:hover .headers-table-copy {
  visibility: visible;
}

.response-body-input :deep(.el-textarea__inner) {
  min-height: 360px;
  font-family: var(--lc-font-mono);
  font-size: 12px;
  line-height: 1.55;
}

.history-toolbar {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  gap: 8px;
  align-items: center;
  margin-bottom: 8px;
}

.history-list {
  min-height: 120px;
}

.history-item {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
  align-items: center;
  padding: 8px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  cursor: pointer;
}

.history-main {
  display: grid;
  grid-template-columns: 64px 1fr;
  gap: 6px;
  min-width: 0;
  cursor: pointer;
}

.history-main span,
.history-main small {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.history-main small {
  grid-column: 2;
  color: var(--el-text-color-secondary);
}

.history-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 4px;
}

:global(.api-workbench-move-options) {
  display: flex;
  max-height: 320px;
  flex-direction: column;
  align-items: stretch;
  gap: 6px;
  overflow: auto;
}

:global(.api-workbench-move-options .el-radio) {
  height: auto;
  min-height: 30px;
  margin-right: 0;
}

@media (max-width: 1380px) {
  .api-workbench-panel {
    grid-template-columns: 260px minmax(420px, 1fr) minmax(340px, 38%);
  }

  .api-workbench-request-bar {
    grid-template-columns: 104px minmax(220px, 1fr) 32px auto;
  }
}

@media (max-width: 1180px) {
  .api-workbench-panel {
    grid-template-columns: 240px 1fr;
  }

  .api-workbench-response {
    grid-column: 1 / -1;
    min-height: 360px;
  }

  .api-workbench-request-bar,
  .api-workbench-meta-row {
    grid-template-columns: 1fr;
  }

  .api-workbench-primary-actions,
  .curl-actions,
  .response-actions {
    justify-content: flex-start;
  }

  .meta-environment-select {
    width: 100%;
  }
}

@media (max-width: 820px) {
  .api-workbench-panel {
    grid-template-columns: 1fr;
    overflow: auto;
  }

  .api-workbench-editor,
  .api-workbench-response {
    min-height: 360px;
  }

  .environment-dialog-overview,
  .environment-dialog-footer,
  .environment-editor-heading {
    flex-direction: column;
    align-items: stretch;
  }

  .environment-manager-layout {
    min-height: 0;
    grid-template-columns: 1fr;
  }

  .environment-list-panel {
    max-height: 220px;
    overflow: auto;
  }

  .environment-actions,
  .environment-footer-actions,
  .environment-overview-stats {
    justify-content: flex-start;
  }

  .environment-save-status {
    white-space: normal;
  }

  .history-toolbar,
  .history-item,
  .history-main {
    grid-template-columns: 1fr;
  }

  .history-main small {
    grid-column: 1;
  }

  .history-actions {
    justify-content: flex-start;
  }
}
</style>

<style>
.request-settings {
  display: grid;
  gap: 10px;
}

.request-settings-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  font-size: 13px;
}

.request-settings-note {
  margin: 0;
  color: var(--el-text-color-secondary);
  font-size: 12px;
  line-height: 1.5;
}

.method-get {
  color: #1a7f37;
}

.method-post {
  color: #bc4c00;
}

.method-put {
  color: #0969da;
}

.method-patch {
  color: #1b7c83;
}

.method-delete {
  color: #cf222e;
}

.method-head,
.method-options,
.method-default {
  color: var(--el-text-color-secondary);
}
</style>

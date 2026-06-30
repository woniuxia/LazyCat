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
      <div class="api-workbench-request-bar">
        <el-select v-model="draft.method" class="method-select">
          <el-option v-for="method in methods" :key="method" :label="method" :value="method" />
        </el-select>
        <el-input v-model="draft.url" placeholder="https://example.com/api 或 /api/users" />
        <el-select
          v-model="selectedEnvironmentId"
          class="environment-select"
          placeholder="环境"
          @change="handleEnvironmentChange"
        >
          <el-option v-for="env in environments" :key="env.id" :label="env.name" :value="env.id" />
        </el-select>
        <el-button type="primary" :loading="sending" @click="sendRequest">发送</el-button>
      </div>

      <el-alert
        v-if="baseUrlError"
        type="warning"
        :title="baseUrlError"
        show-icon
        :closable="false"
      />

      <el-tabs v-model="editorTab">
        <el-tab-pane label="Query" name="query">
          <KeyValueEditor v-model="draft.query" />
        </el-tab-pane>
        <el-tab-pane label="Headers" name="headers">
          <KeyValueEditor v-model="draft.headers" />
        </el-tab-pane>
        <el-tab-pane label="Body" name="body">
          <div class="body-toolbar">
            <el-radio-group v-model="draft.bodyType">
              <el-radio-button label="none">none</el-radio-button>
              <el-radio-button label="json">json</el-radio-button>
              <el-radio-button label="text">text</el-radio-button>
              <el-radio-button label="form-urlencoded">form</el-radio-button>
            </el-radio-group>
            <el-switch disabled inactive-text="跟随重定向" />
          </div>
          <KeyValueEditor v-if="draft.bodyType === 'form-urlencoded'" v-model="draft.form" />
          <el-input
            v-else-if="draft.bodyType !== 'none'"
            v-model="draft.body"
            type="textarea"
            :rows="12"
          />
          <el-empty v-else description="无请求体" />
        </el-tab-pane>
        <el-tab-pane label="环境" name="environment">
          <div class="environment-toolbar">
            <strong>{{ selectedEnvironment?.name ?? "未选择环境" }}</strong>
            <el-button
              size="small"
              type="primary"
              :loading="savingEnvironment"
              :disabled="!selectedEnvironment"
              @click="saveCurrentEnvironment"
            >
              保存环境
            </el-button>
          </div>
          <KeyValueEditor v-model="environmentRows" />
        </el-tab-pane>
      </el-tabs>

      <div class="api-workbench-actions">
        <el-input v-model="requestName" placeholder="接口名称" />
        <el-button @click="saveRequest">保存接口</el-button>
        <el-button @click="exportMarkdown">导出 Markdown</el-button>
      </div>
    </main>

    <section class="api-workbench-response">
      <el-tabs v-model="responseTab">
        <el-tab-pane label="响应" name="response">
          <div v-if="response" class="response-summary">
            <el-tag :type="response.ok ? 'success' : 'warning'">
              {{ response.status ?? "ERR" }}
            </el-tag>
            <span>{{ response.durationMs }}ms</span>
            <span>{{ response.bodySize }} bytes</span>
          </div>
          <el-input
            v-if="response"
            :model-value="formattedResponseBody"
            type="textarea"
            :rows="18"
            readonly
          />
          <el-empty v-else description="发送请求后查看响应" />
        </el-tab-pane>
        <el-tab-pane label="响应头" name="headers">
          <pre class="headers-view">{{ responseHeadersText }}</pre>
        </el-tab-pane>
        <el-tab-pane label="历史" name="history">
          <div
            v-for="item in history"
            :key="item.id"
            class="history-item"
            @click="reuseHistory(item)"
          >
            <strong>{{ item.method }}</strong>
            <span>{{ item.finalUrl }}</span>
            <small>{{ item.status ?? "ERR" }} · {{ item.durationMs }}ms</small>
          </div>
        </el-tab-pane>
      </el-tabs>
    </section>

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
import { computed, defineComponent, h, onMounted, ref } from "vue";
import { ElButton, ElInput, ElMessage, ElMessageBox, ElSwitch } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import ApiWorkbenchSidebar from "./ApiWorkbenchSidebar.vue";
import type {
  ApiWorkbenchCollection,
  ApiWorkbenchEnvironment,
  ApiWorkbenchHistoryItem,
  ApiWorkbenchKeyValueRow,
  ApiWorkbenchListResult,
  ApiWorkbenchMoveTarget,
  ApiWorkbenchNavCommand,
  ApiWorkbenchNavTarget,
  ApiWorkbenchOrderDirection,
  ApiWorkbenchRequestDetail,
  ApiWorkbenchSendResult,
} from "../types/api-workbench";
import {
  API_WORKBENCH_METHODS,
  DEFAULT_API_WORKBENCH_DRAFT,
  buildApiWorkbenchSelectionState,
  draftApiWorkbenchEnvironmentRows,
  formatApiWorkbenchResponseBody,
  normalizeApiWorkbenchDraft,
  serializeApiWorkbenchEnvironmentRows,
} from "../utils/apiWorkbench";
import {
  buildApiWorkbenchFolderMoveTargets,
  buildApiWorkbenchRequestMoveTargets,
  getApiWorkbenchFolderAncestorIds,
  moveApiWorkbenchOrderedId,
} from "../utils/apiWorkbenchTree";

type ApiWorkbenchSidebarExpose = {
  expandFolder(folderId: number | null): void;
};

const KeyValueEditor = defineComponent({
  props: {
    modelValue: { type: Array as () => ApiWorkbenchKeyValueRow[], required: true },
  },
  emits: ["update:modelValue"],
  setup(props, { emit }) {
    function update(index: number, patch: Partial<ApiWorkbenchKeyValueRow>) {
      const next = props.modelValue.map((row, i) => (i === index ? { ...row, ...patch } : row));
      emit("update:modelValue", next);
    }
    function addRow() {
      emit("update:modelValue", [...props.modelValue, { enabled: true, key: "", value: "" }]);
    }
    function removeRow(index: number) {
      emit(
        "update:modelValue",
        props.modelValue.filter((_, i) => i !== index),
      );
    }
    return () =>
      h("div", { class: "kv-editor" }, [
        ...props.modelValue.map((row, index) =>
          h("div", { class: "kv-row", key: index }, [
            h(ElSwitch, {
              modelValue: row.enabled,
              "onUpdate:modelValue": (value: boolean) => update(index, { enabled: value }),
            }),
            h(ElInput, {
              modelValue: row.key,
              placeholder: "Key",
              "onUpdate:modelValue": (value: string) => update(index, { key: value }),
            }),
            h(ElInput, {
              modelValue: row.value,
              placeholder: "Value",
              "onUpdate:modelValue": (value: string) => update(index, { value }),
            }),
            h(ElButton, { onClick: () => removeRow(index) }, () => "删除"),
          ]),
        ),
        h(ElButton, { onClick: addRow }, () => "添加一行"),
      ]);
  },
});

const methods = API_WORKBENCH_METHODS;
const sidebarRef = ref<ApiWorkbenchSidebarExpose | null>(null);
const loading = ref(false);
const sending = ref(false);
const savingEnvironment = ref(false);
const collections = ref<ApiWorkbenchCollection[]>([]);
const environments = ref<ApiWorkbenchEnvironment[]>([]);
const history = ref<ApiWorkbenchHistoryItem[]>([]);
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
const baseUrl = computed(
  () => selectedEnvironment.value?.variables.find((item) => item.name === "BASE_URL")?.value ?? "",
);
const baseUrlError = computed(() => {
  if (/^https?:\/\//i.test(draft.value.url.trim())) return "";
  if (!draft.value.url.trim()) return "";
  return baseUrl.value.trim() ? "" : "相对 URL 需要当前环境配置 BASE_URL";
});
const formattedResponseBody = computed(() =>
  response.value
    ? formatApiWorkbenchResponseBody(response.value.bodyText, response.value.contentType)
    : "",
);
const responseHeadersText = computed(
  () => response.value?.responseHeaders.map((row) => `${row.key}: ${row.value}`).join("\n") ?? "",
);

function resetRequestState() {
  selectedRequestId.value = null;
  selectedRequestFolderId.value = null;
  requestName.value = "";
  requestDescription.value = "";
  draft.value = normalizeApiWorkbenchDraft({});
  response.value = null;
}

function isMessageBoxCancel(error: unknown): boolean {
  return error === "cancel" || error === "close";
}

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return "操作失败";
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

async function loadAll() {
  loading.value = true;
  try {
    const result = (await invokeToolByChannel(
      "tool:api-workbench:list",
      {},
    )) as ApiWorkbenchListResult;
    collections.value = result.collections ?? [];
    history.value = result.history ?? [];
    if (!selectedCollectionId.value && collections.value.length > 0) {
      await selectCollection(collections.value[0].id);
    }
  } finally {
    loading.value = false;
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
  requestName.value = nextState.requestName;
  requestDescription.value = "";
  draft.value = nextState.draft;
  response.value = nextState.response;
  const result = (await invokeToolByChannel("tool:api-workbench:environment-list", {
    collectionId: id,
  })) as { items: ApiWorkbenchEnvironment[] };
  environments.value = result.items ?? [];
  syncEnvironmentRows();
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
    if (command === "folder:create-root") return await createFolder(null);
    if (target.type === "collection") {
      if (command === "collection:select") return await selectCollection(target.collectionId);
      if (command === "collection:rename") return await renameCollection(target.collectionId);
      if (command === "collection:delete") return await deleteCollection(target.collectionId);
      if (command === "collection:export") return await exportMarkdownForCollection(target.collectionId);
    }
    if (target.type === "folder") {
      if (command === "folder:create-child") return await createFolder(target.folderId);
      if (command === "folder:rename") return await renameFolder(target.folderId);
      if (command === "folder:delete") return await deleteFolder(target.folderId);
      if (command === "folder:move") return await moveFolder(target.folderId);
      if (command === "folder:up") return await reorderFolder(target.folderId, "up");
      if (command === "folder:down") return await reorderFolder(target.folderId, "down");
    }
    if (target.type === "request") {
      if (command === "request:open") return await loadRequest(target.requestId);
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

async function handleEnvironmentChange() {
  await persistActiveEnvironment();
  syncEnvironmentRows();
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
  savingEnvironment.value = true;
  try {
    await invokeToolByChannel("tool:api-workbench:environment-save", {
      id: selectedEnvironment.value.id,
      collectionId: selectedCollectionId.value,
      name: selectedEnvironment.value.name,
      variables: serializeApiWorkbenchEnvironmentRows(environmentRows.value),
    });
    const result = (await invokeToolByChannel("tool:api-workbench:environment-list", {
      collectionId: selectedCollectionId.value,
    })) as { items: ApiWorkbenchEnvironment[] };
    environments.value = result.items ?? [];
    syncEnvironmentRows();
    ElMessage.success("环境已保存");
  } finally {
    savingEnvironment.value = false;
  }
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
    const historyResult = (await invokeToolByChannel("tool:api-workbench:history-list", {})) as {
      items: ApiWorkbenchHistoryItem[];
    };
    history.value = historyResult.items ?? [];
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
  await loadAll();
  ElMessage.success("已保存接口");
}

async function loadRequest(id: number) {
  const detail = (await invokeToolByChannel("tool:api-workbench:request-get", {
    id,
  })) as ApiWorkbenchRequestDetail;
  selectedRequestId.value = detail.id;
  selectedRequestFolderId.value = detail.folderId;
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

async function exportMarkdown() {
  if (!selectedCollectionId.value) {
    ElMessage.warning("请先选择集合");
    return;
  }
  await exportMarkdownForCollection(selectedCollectionId.value);
}

function reuseHistory(item: ApiWorkbenchHistoryItem) {
  draft.value = normalizeApiWorkbenchDraft({
    ...draft.value,
    method: item.method,
    url: item.url,
  });
  responseTab.value = "response";
}

onMounted(loadAll);
</script>

<style scoped>
.api-workbench-panel {
  display: grid;
  grid-template-columns: 260px minmax(420px, 1fr) minmax(320px, 42%);
  gap: 12px;
  height: 100%;
  min-height: 0;
  padding: 12px;
  background: var(--el-bg-color-page);
}

.api-workbench-editor,
.api-workbench-response {
  min-height: 0;
  overflow: auto;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  background: var(--el-bg-color);
  padding: 12px;
}

.api-workbench-request-bar,
.api-workbench-actions,
.body-toolbar,
.environment-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
}

.environment-toolbar {
  justify-content: space-between;
  margin-bottom: 8px;
}

.method-select {
  width: 104px;
  flex: none;
}

.environment-select {
  width: 140px;
  flex: none;
}

.kv-editor {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.kv-row {
  display: grid;
  grid-template-columns: 52px minmax(120px, 1fr) minmax(160px, 1.4fr) 72px;
  gap: 8px;
  align-items: center;
}

.response-summary {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
}

.headers-view {
  white-space: pre-wrap;
  word-break: break-word;
  margin: 0;
  font-family: var(--lc-font-mono);
  font-size: 12px;
}

.history-item {
  display: grid;
  grid-template-columns: 64px 1fr;
  gap: 6px;
  padding: 8px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  cursor: pointer;
}

.history-item span,
.history-item small {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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

@media (max-width: 1180px) {
  .api-workbench-panel {
    grid-template-columns: 240px 1fr;
  }

  .api-workbench-response {
    grid-column: 1 / -1;
  }
}
</style>

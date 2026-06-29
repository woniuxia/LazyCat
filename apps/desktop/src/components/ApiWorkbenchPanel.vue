<template>
  <div class="api-workbench-panel">
    <aside class="api-workbench-sidebar">
      <div class="api-workbench-toolbar">
        <strong>接口集合</strong>
        <el-button size="small" type="primary" @click="createCollection">新建</el-button>
      </div>
      <el-empty v-if="!loading && collections.length === 0" description="暂无接口集合" />
      <div v-else class="api-workbench-tree">
        <button
          v-for="collection in collections"
          :key="collection.id"
          class="api-workbench-collection"
          :class="{ active: selectedCollectionId === collection.id }"
          @click="selectCollection(collection.id)"
        >
          <span>{{ collection.name }}</span>
          <small>{{ collection.requests.length }} 个接口</small>
        </button>
      </div>
      <div v-if="selectedCollection" class="request-list">
        <button
          v-for="request in selectedCollection.requests"
          :key="request.id"
          class="request-list-item"
          @click="loadRequest(request.id)"
        >
          <strong>{{ request.method }}</strong>
          <span>{{ request.name }}</span>
        </button>
      </div>
    </aside>

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
          @change="persistActiveEnvironment"
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
  </div>
</template>

<script setup lang="ts">
import { computed, defineComponent, h, onMounted, ref } from "vue";
import { ElButton, ElInput, ElMessage, ElMessageBox, ElSwitch } from "element-plus";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  ApiWorkbenchCollection,
  ApiWorkbenchEnvironment,
  ApiWorkbenchHistoryItem,
  ApiWorkbenchKeyValueRow,
  ApiWorkbenchListResult,
  ApiWorkbenchRequestDetail,
  ApiWorkbenchSendResult,
} from "../types/api-workbench";
import {
  API_WORKBENCH_METHODS,
  DEFAULT_API_WORKBENCH_DRAFT,
  formatApiWorkbenchResponseBody,
  normalizeApiWorkbenchDraft,
} from "../utils/apiWorkbench";

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
const loading = ref(false);
const sending = ref(false);
const collections = ref<ApiWorkbenchCollection[]>([]);
const environments = ref<ApiWorkbenchEnvironment[]>([]);
const history = ref<ApiWorkbenchHistoryItem[]>([]);
const selectedCollectionId = ref<number | null>(null);
const selectedEnvironmentId = ref<number | null>(null);
const selectedRequestId = ref<number | null>(null);
const requestName = ref("");
const draft = ref({ ...DEFAULT_API_WORKBENCH_DRAFT });
const response = ref<ApiWorkbenchSendResult | null>(null);
const editorTab = ref("query");
const responseTab = ref("response");

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
  selectedCollectionId.value = id;
  const collection = collections.value.find((item) => item.id === id);
  selectedEnvironmentId.value = collection?.activeEnvironmentId ?? null;
  const result = (await invokeToolByChannel("tool:api-workbench:environment-list", {
    collectionId: id,
  })) as { items: ApiWorkbenchEnvironment[] };
  environments.value = result.items ?? [];
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

async function persistActiveEnvironment() {
  if (!selectedCollectionId.value || !selectedEnvironmentId.value) return;
  await invokeToolByChannel("tool:api-workbench:collection-set-active-environment", {
    collectionId: selectedCollectionId.value,
    environmentId: selectedEnvironmentId.value,
  });
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
    folderId: null,
    name: requestName.value.trim(),
    description: "",
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
  requestName.value = detail.name;
  draft.value = normalizeApiWorkbenchDraft(detail.draft);
}

async function exportMarkdown() {
  if (!selectedCollectionId.value) {
    ElMessage.warning("请先选择集合");
    return;
  }
  const result = (await invokeToolByChannel("tool:api-workbench:export-markdown", {
    collectionId: selectedCollectionId.value,
  })) as { fileName: string; markdown: string };
  await navigator.clipboard.writeText(result.markdown);
  ElMessage.success(`Markdown 已复制：${result.fileName}`);
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

.api-workbench-sidebar,
.api-workbench-editor,
.api-workbench-response {
  min-height: 0;
  overflow: auto;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  background: var(--el-bg-color);
  padding: 12px;
}

.api-workbench-toolbar,
.api-workbench-request-bar,
.api-workbench-actions,
.body-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
}

.api-workbench-toolbar {
  justify-content: space-between;
  margin-bottom: 12px;
}

.api-workbench-tree {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.api-workbench-collection {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  background: var(--el-fill-color-blank);
  color: var(--el-text-color-primary);
  padding: 8px;
  cursor: pointer;
}

.api-workbench-collection.active {
  border-color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
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

.request-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: 12px;
}

.request-list-item {
  display: grid;
  grid-template-columns: 56px 1fr;
  gap: 6px;
  align-items: center;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--el-text-color-primary);
  padding: 6px 8px;
  text-align: left;
  cursor: pointer;
}

.request-list-item:hover {
  background: var(--el-fill-color-light);
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

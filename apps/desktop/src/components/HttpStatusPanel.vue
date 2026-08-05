<template>
  <div class="http-status-panel">
    <el-input
      v-model="searchQuery"
      placeholder="输入状态码、名称或排查关键词搜索"
      clearable
      prefix-icon="Search"
      aria-label="搜索 HTTP 状态码"
      style="max-width: 420px; margin-bottom: 4px"
    />

    <div v-if="isSearching" class="loading-hint">正在搜索...</div>

    <div v-if="loadError && !searchQuery.trim()" class="error-hint" role="alert">
      状态码列表加载失败：{{ loadError }}
    </div>

    <div v-if="searchError && searchQuery.trim()" class="error-hint" role="alert">
      状态码搜索失败：{{ searchError }}
    </div>

    <div v-if="classificationHint" class="classification-hint" role="status">
      <strong>{{ classificationHint.code }} {{ classificationHint.category }}</strong>
      <span>{{ classificationHint.message }}</span>
    </div>

    <div
      v-if="
        searchQuery.trim() &&
        !isSearching &&
        !searchError &&
        flatResults.length === 0 &&
        !classificationHint
      "
      class="empty-hint"
    >
      未找到匹配的状态码
    </div>

    <template v-if="searchQuery.trim()">
      <HttpStatusTable
        v-if="flatResults.length"
        :data="flatResults"
        :expanded-codes="expandedCodes"
        :show-header="true"
        @expand-change="handleExpandChange"
      />
    </template>

    <template v-else>
      <div v-if="isLoading" class="loading-hint">正在加载状态码...</div>
      <div v-for="group in groups" :key="group.category" class="group-section">
        <div class="group-header" :class="'header-' + group.category">
          {{ group.category }} {{ group.name }}
        </div>
        <HttpStatusTable
          :data="group.codes"
          :expanded-codes="expandedCodes"
          :show-header="group === groups[0]"
          @expand-change="handleExpandChange"
        />
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import HttpStatusTable from "./HttpStatusTable.vue";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  HttpStatusClassificationHint,
  HttpStatusCode,
  HttpStatusGroup,
  HttpStatusListResponse,
  HttpStatusLookupResponse,
} from "../types/httpStatus";

const searchQuery = ref("");
const groups = ref<HttpStatusGroup[]>([]);
const flatResults = ref<HttpStatusCode[]>([]);
const classificationHint = ref<HttpStatusClassificationHint | null>(null);
const expandedCodes = ref<number[]>([]);
const isLoading = ref(false);
const isSearching = ref(false);
const loadError = ref("");
const searchError = ref("");

function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function pruneExpandedCodes(): void {
  const availableCodes = new Set(
    groups.value.flatMap((group) => group.codes.map((item) => item.code)),
  );
  expandedCodes.value = expandedCodes.value.filter((code) => availableCodes.has(code));
}

async function loadAll(): Promise<void> {
  isLoading.value = true;
  loadError.value = "";
  try {
    const data = (await invokeToolByChannel(
      "tool:network:http-status-list",
      {},
    )) as HttpStatusListResponse;
    groups.value = data.groups;
    pruneExpandedCodes();
  } catch (error) {
    loadError.value = getErrorMessage(error);
    ElMessage.error(getErrorMessage(error));
  } finally {
    isLoading.value = false;
  }
}

async function search(query: string, requestId: number): Promise<void> {
  const normalizedQuery = query.trim();
  if (!normalizedQuery) {
    flatResults.value = [];
    classificationHint.value = null;
    searchError.value = "";
    isSearching.value = false;
    return;
  }

  try {
    const data = (await invokeToolByChannel("tool:network:http-status-lookup", {
      query: normalizedQuery,
    })) as HttpStatusLookupResponse;
    if (requestId !== searchRequestId || normalizedQuery !== searchQuery.value.trim()) return;
    flatResults.value = data.results;
    classificationHint.value = data.classificationHint;
  } catch (error) {
    if (requestId !== searchRequestId) return;
    flatResults.value = [];
    classificationHint.value = null;
    searchError.value = getErrorMessage(error);
    ElMessage.error(getErrorMessage(error));
  } finally {
    if (requestId === searchRequestId) isSearching.value = false;
  }
}

function handleExpandChange(row: HttpStatusCode, expandedRows: HttpStatusCode[]): void {
  const isExpanded = expandedRows.some((item) => item.code === row.code);
  if (isExpanded) {
    if (!expandedCodes.value.includes(row.code)) {
      expandedCodes.value = [...expandedCodes.value, row.code];
    }
    return;
  }
  expandedCodes.value = expandedCodes.value.filter((code) => code !== row.code);
}

let timer: ReturnType<typeof setTimeout> | null = null;
let searchRequestId = 0;
watch(searchQuery, (value) => {
  if (timer) clearTimeout(timer);
  searchRequestId += 1;
  flatResults.value = [];
  classificationHint.value = null;
  searchError.value = "";
  if (!value.trim()) {
    isSearching.value = false;
    return;
  }
  isSearching.value = true;
  const requestId = searchRequestId;
  timer = setTimeout(() => void search(value, requestId), 300);
});

onMounted(() => void loadAll());

onBeforeUnmount(() => {
  if (timer) clearTimeout(timer);
  searchRequestId += 1;
});
</script>

<style scoped>
.http-status-panel {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 8px;
}

.group-section {
  min-width: 0;
  margin-bottom: 4px;
}

.group-header {
  padding: 6px 4px 4px;
  font-size: 13px;
  font-weight: 600;
}

.header-1xx {
  color: #909399;
}

.header-2xx {
  color: #67c23a;
}

.header-3xx {
  color: #e6a23c;
}

.header-4xx,
.header-5xx {
  color: #f56c6c;
}

.classification-hint {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  gap: 4px 10px;
  align-items: baseline;
  padding: 9px 12px;
  border: 1px solid var(--el-color-warning-light-5);
  border-radius: 4px;
  background: var(--el-color-warning-light-9);
  color: var(--el-text-color-regular);
  font-size: 12px;
  line-height: 1.5;
}

.classification-hint strong {
  color: var(--el-color-warning-dark-2);
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
}

.loading-hint,
.empty-hint,
.error-hint {
  padding: 20px;
  color: var(--el-text-color-secondary);
  text-align: center;
}

.error-hint {
  border: 1px solid var(--el-color-danger-light-5);
  border-radius: 4px;
  background: var(--el-color-danger-light-9);
  color: var(--el-color-danger);
}
</style>

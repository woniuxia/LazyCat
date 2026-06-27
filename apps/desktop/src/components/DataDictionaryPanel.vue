<template>
  <div class="data-dictionary-panel">
    <aside class="dd-sidebar">
      <div class="dd-sidebar-head">
        <div>
          <h2>数据字典</h2>
          <span>{{ dictionaries.length }} 个字典</span>
        </div>
        <el-tooltip content="新建字典" placement="bottom">
          <el-button :icon="Plus" circle type="primary" @click="openCreateDialog" />
        </el-tooltip>
      </div>

      <div class="dd-dictionary-list">
        <button
          class="dd-dictionary-item dd-dictionary-all"
          :class="{ active: searchScope === 'all' }"
          type="button"
          @click="selectAllDictionaries"
        >
          <span class="dd-dictionary-name">全部</span>
          <span class="dd-dictionary-meta">{{ totalRecordCount }} 条</span>
        </button>

        <div
          ref="dictionarySortListRef"
          class="dd-dictionary-sort-list"
          :class="{ 'is-saving-order': savingDictionaryOrder }"
        >
          <el-dropdown
            v-for="dictionary in dictionaries"
            :key="dictionary.id"
            class="dd-dictionary-menu"
            :ref="(el) => setDictionaryMenuRef(dictionary.id, el)"
            trigger="contextmenu"
            @visible-change="(visible) => handleDictionaryMenuVisibleChange(visible, dictionary.id)"
            @command="(command) => handleDictionaryCommand(command, dictionary)"
          >
            <button
              class="dd-dictionary-item"
              :class="{ active: dictionary.id === selectedId }"
              type="button"
              title="点击选择，拖拽右侧手柄排序，右键打开字典菜单"
              @click="selectDictionary(dictionary.id)"
            >
              <span class="dd-dictionary-name">{{ dictionary.name }}</span>
              <span class="dd-dictionary-trailing">
                <span class="dd-dictionary-meta">{{ dictionary.recordCount }} 条</span>
                <el-icon class="dd-dictionary-drag-handle" title="拖拽排序"><Rank /></el-icon>
              </span>
            </button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="replace" :icon="Refresh">替换</el-dropdown-item>
                <el-dropdown-item command="fields" :icon="Setting">字段</el-dropdown-item>
                <el-dropdown-item command="rebuild" :icon="Refresh">重建索引</el-dropdown-item>
                <el-dropdown-item command="rename" :icon="Edit">重命名</el-dropdown-item>
                <el-dropdown-item command="delete" :icon="Delete" divided>删除</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>
      </div>

      <div v-if="!dictionaries.length" class="dd-empty">暂无字典</div>
    </aside>

    <main class="dd-results">
      <div class="dd-toolbar">
        <el-input
          v-model="keyword"
          class="dd-search-input"
          clearable
          placeholder="搜索字段值"
          :prefix-icon="Search"
          @input="scheduleSearch"
          @keyup.enter="runSearch"
        />
        <el-tooltip content="搜索" placement="bottom">
          <el-button :icon="Search" circle @click="runSearch" />
        </el-tooltip>
      </div>

      <div class="dd-result-list" v-loading="searching">
        <button
          v-for="item in searchItems"
          :key="item.id"
          class="dd-result-item"
          :class="{ active: selectedItem?.id === item.id }"
          @click="selectSearchItem(item)"
        >
          <div class="dd-result-head">
            <span class="dd-result-title">
              <span class="dd-result-title-text">{{ resultTitle(item) }}</span>
            </span>
            <el-tag v-if="item.matches.length" size="small" effect="plain">
              {{ item.matches.length }} 命中
            </el-tag>
          </div>
          <div class="dd-summary-line">
            <span
              v-for="part in resultSummary(item)"
              :key="part.fieldPath"
              class="dd-summary-part"
              title="点击复制"
              @click.stop="copySummaryValue(part.value)"
            >
              <b>{{ part.label }}</b>{{ part.value }}
            </span>
          </div>
        </button>

        <div v-if="searchHasMore" class="dd-limit-hint">
          仅显示前 100 条结果，请缩小关键词继续检索
        </div>
        <div v-if="!searchItems.length" class="dd-empty">无结果</div>
      </div>
    </main>

    <section class="dd-detail" v-loading="detailLoading">
      <div class="dd-detail-head">
        <div>
          <h3>{{ recordDetail?.record.title ?? (selectedItem ? dictionarySourceLabel(selectedItem) : "未选择记录") }}</h3>
          <span v-if="recordDetail">{{ recordDetail.record.dictionaryName }} #{{ recordDetail.record.rowIndex + 1 }}</span>
          <span v-else-if="selectedItem">{{ selectedItem.dictionaryName }}</span>
        </div>
        <el-button v-if="detailError && selectedItem" size="small" @click="loadRecordDetail(selectedItem)">
          重试
        </el-button>
      </div>

      <el-alert
        v-if="detailError"
        class="dd-detail-error"
        :title="detailError"
        type="error"
        :closable="false"
        show-icon
      />

      <div v-if="recordDetail" class="dd-match-list">
        <el-tag
          v-for="part in recordDetail.record.summary"
          :key="part.fieldPath"
          effect="plain"
          class="dd-match-tag"
        >
          {{ part.label }}: {{ part.value }}
        </el-tag>
      </div>

      <div v-if="recordDetail" class="dd-relation-groups">
        <section
          v-for="group in relationGroups"
          :key="`${group.direction}-${group.relationId}`"
          class="dd-relation-group"
        >
          <div class="dd-relation-group-head">
            <h4>{{ group.name }}</h4>
            <span>{{ group.itemCount }} 条</span>
          </div>
          <button
            v-for="item in group.items"
            :key="item.id"
            class="dd-related-record"
            type="button"
            @click="loadRelatedRecord(item.id)"
          >
            <strong>{{ item.title }}</strong>
            <span>{{ item.dictionaryName }} #{{ item.rowIndex + 1 }}</span>
            <small v-for="part in item.summary" :key="part.fieldPath">
              {{ part.label }}: {{ part.value }}
            </small>
          </button>
          <div v-if="!group.items.length" class="dd-empty dd-relation-empty">无关联记录</div>
        </section>
      </div>

      <pre class="dd-json-view">{{ selectedJson }}</pre>
    </section>

    <el-dialog
      v-model="importDialogVisible"
      :title="importMode === 'create' ? '导入数据字典' : '替换字典数据'"
      width="760px"
      destroy-on-close
    >
      <div class="dd-import-form">
        <el-input
          v-if="importMode === 'create'"
          v-model="importForm.name"
          placeholder="字典名称"
        />
        <el-input
          v-if="importMode === 'create'"
          v-model="importForm.description"
          placeholder="描述"
        />
        <el-input
          v-model="importForm.input"
          type="textarea"
          :rows="12"
          resize="none"
          placeholder='[{"id":1,"user":{"name":"张三"}}]'
          @input="invalidateImportPreview"
        />

        <div v-if="preview" class="dd-preview">
          <span>{{ preview.recordCount }} 条记录</span>
          <span>{{ preview.fields.length }} 个字段</span>
        </div>
        <el-table v-if="preview" :data="preview.fields" height="220" size="small">
          <el-table-column prop="fieldPath" label="字段" min-width="180" />
          <el-table-column prop="typeHint" label="类型" width="90" />
          <el-table-column prop="sampleValue" label="样例" min-width="180" show-overflow-tooltip />
        </el-table>
      </div>

      <template #footer>
        <el-button @click="importDialogVisible = false">取消</el-button>
        <el-button :loading="previewing" @click="previewImport">预览</el-button>
        <el-button type="primary" :loading="savingImport" :disabled="!canSaveImport" @click="saveImport">
          保存
        </el-button>
      </template>
    </el-dialog>

    <el-drawer
      v-model="fieldDrawerVisible"
      :title="fieldDrawerTitle"
      :size="fieldDrawerSize"
      class="dd-field-drawer"
      @opened="initFieldSortable"
      @closed="handleFieldDrawerClosed"
    >
      <div class="dd-field-config-panel">
        <div class="dd-field-config-summary">
          <span>
            <b>{{ visibleFieldDrafts.length }}</b>
            展示字段
          </span>
          <span>
            <b>{{ hiddenFieldDrafts.length }}</b>
            非展示字段
          </span>
        </div>
        <div class="dd-field-sort-config">
          <el-form-item label="主键字段">
            <el-select
              v-model="fieldPrimaryPath"
              clearable
              filterable
              placeholder="未配置"
              size="small"
            >
              <el-option
                v-for="option in fieldSortOptions"
                :key="option.value"
                :label="option.label"
                :value="option.value"
              />
            </el-select>
          </el-form-item>
          <el-form-item label="标题字段">
            <el-select
              v-model="fieldTitlePath"
              clearable
              filterable
              placeholder="默认字典来源"
              size="small"
            >
              <el-option
                v-for="option in fieldSortOptions"
                :key="option.value"
                :label="option.label"
                :value="option.value"
              />
            </el-select>
          </el-form-item>
          <el-form-item label="排序字段">
            <el-select
              v-model="fieldSortPath"
              clearable
              filterable
              placeholder="原始顺序"
              size="small"
            >
              <el-option
                v-for="option in fieldSortOptions"
                :key="option.value"
                :label="option.label"
                :value="option.value"
              />
            </el-select>
          </el-form-item>
          <el-form-item label="排序方向">
            <el-radio-group v-model="fieldSortDirection" :disabled="!fieldSortPath" size="small">
              <el-radio-button
                v-for="option in sortDirectionOptions"
                :key="option.value"
                :label="option.value"
              >
                {{ option.label }}
              </el-radio-button>
            </el-radio-group>
          </el-form-item>
        </div>
      </div>

      <section class="dd-relation-editor">
        <div class="dd-field-section-head">
          <h4>关系配置</h4>
          <el-button size="small" @click="addRelationDraft">添加关系</el-button>
        </div>
        <div v-if="!fieldRelationDrafts.length" class="dd-empty dd-relation-empty">
          暂无关系
        </div>
        <div
          v-for="(relation, index) in fieldRelationDrafts"
          :key="index"
          class="dd-relation-row"
        >
          <el-select
            v-model="relation.sourceFieldPath"
            filterable
            placeholder="源字段"
            size="small"
          >
            <el-option
              v-for="option in fieldSortOptions"
              :key="option.value"
              :label="option.label"
              :value="option.value"
            />
          </el-select>
          <el-input v-model="relation.relationName" placeholder="关系名" size="small" />
          <div class="dd-relation-target">
            <el-select
              v-model="relation.targetDictionaryId"
              filterable
              placeholder="目标字典"
              size="small"
            >
              <el-option
                v-for="dictionary in dictionaries"
                :key="dictionary.id"
                :label="dictionary.name"
                :value="dictionary.id"
              />
            </el-select>
            <span :class="{ 'is-error': hasInvalidRelationTarget(relation, dictionaries) }">
              {{ relationTargetPrimaryLabel(relation.targetDictionaryId, dictionaries) }}
            </span>
          </div>
          <el-input v-model="relation.reverseName" placeholder="反向关系名" size="small" />
          <el-button size="small" text type="danger" @click="removeRelationDraft(index)">
            删除
          </el-button>
        </div>
      </section>

      <div class="dd-field-sections">
        <section class="dd-field-section">
          <div class="dd-field-section-head">
            <h4>展示字段</h4>
            <span>{{ visibleFieldDrafts.length }} 个</span>
          </div>
          <el-table
            ref="visibleFieldTableRef"
            :data="visibleFieldDrafts"
            row-key="fieldPath"
            class="dd-visible-field-list"
            height="clamp(220px, calc((100vh - 360px) / 2), 360px)"
            empty-text="暂无展示字段"
            size="small"
          >
            <el-table-column width="44" align="center" class-name="dd-field-handle-cell">
              <template #default>
                <el-icon class="dd-field-drag-handle" title="拖拽排序"><Rank /></el-icon>
              </template>
            </el-table-column>
            <el-table-column prop="fieldPath" label="字段" min-width="190" show-overflow-tooltip />
            <el-table-column label="显示名" min-width="160">
              <template #default="{ row }">
                <el-input v-model="row.displayName" size="small" />
              </template>
            </el-table-column>
            <el-table-column label="含义" min-width="260">
              <template #default="{ row }">
                <el-input v-model="row.meaning" size="small" />
              </template>
            </el-table-column>
            <el-table-column label="检索" width="72" align="center">
              <template #default="{ row }">
                <el-switch v-model="row.searchable" size="small" />
              </template>
            </el-table-column>
            <el-table-column label="展示" width="72" align="center">
              <template #default="{ row }">
                <el-switch
                  :model-value="row.visible"
                  size="small"
                  @update:model-value="(visible) => setFieldVisible(row.fieldPath, Boolean(visible))"
                />
              </template>
            </el-table-column>
          </el-table>
        </section>

        <section class="dd-field-section">
          <div class="dd-field-section-head">
            <h4>非展示字段</h4>
            <span>{{ hiddenFieldDrafts.length }} 个</span>
          </div>
          <el-table
            :data="hiddenFieldDrafts"
            row-key="fieldPath"
            class="dd-hidden-field-list"
            height="clamp(220px, calc((100vh - 360px) / 2), 360px)"
            empty-text="暂无非展示字段"
            size="small"
          >
            <el-table-column prop="fieldPath" label="字段" min-width="220" show-overflow-tooltip />
            <el-table-column label="显示名" min-width="160">
              <template #default="{ row }">
                <el-input v-model="row.displayName" size="small" />
              </template>
            </el-table-column>
            <el-table-column label="含义" min-width="260">
              <template #default="{ row }">
                <el-input v-model="row.meaning" size="small" />
              </template>
            </el-table-column>
            <el-table-column label="检索" width="72" align="center">
              <template #default="{ row }">
                <el-switch v-model="row.searchable" size="small" />
              </template>
            </el-table-column>
            <el-table-column label="展示" width="72" align="center">
              <template #default="{ row }">
                <el-switch
                  :model-value="row.visible"
                  size="small"
                  @update:model-value="(visible) => setFieldVisible(row.fieldPath, Boolean(visible))"
                />
              </template>
            </el-table-column>
          </el-table>
        </section>
      </div>
      <template #footer>
        <el-button @click="fieldDrawerVisible = false">取消</el-button>
        <el-button type="primary" :loading="savingFields" @click="saveFields">保存</el-button>
      </template>
    </el-drawer>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { ComponentPublicInstance } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import {
  Delete,
  Edit,
  Plus,
  Rank,
  Refresh,
  Search,
  Setting,
} from "@element-plus/icons-vue";
import Sortable from "sortablejs";
import { invokeToolByChannel } from "../bridge/tauri";
import type {
  DataDictionaryField,
  DataDictionaryImportWriteResult,
  DataDictionaryImportPreview,
  DataDictionaryRecordDetail,
  DataDictionaryRelation,
  DataDictionaryRelationDraft,
  DataDictionarySearchItem,
  DataDictionarySearchResult,
  DataDictionarySearchScope,
  DataDictionarySortDirection,
  DataDictionarySummary,
  RebuildDataDictionaryIndexesResult,
} from "../types/data-dictionary";
import {
  buildResultTitle,
  buildResultSummary,
  dictionarySourceLabel,
  formatJsonDocument,
  moveDataDictionaryFieldDraft,
  orderDataDictionaryFieldDrafts,
  setDataDictionaryFieldVisibility,
} from "../utils/dataDictionary";
import { dispatchDictionaryMenuCommand } from "../utils/dataDictionaryMenu";
import {
  duplicateRelationKeys,
  hasInvalidRelationTarget,
  relationTargetPrimaryLabel,
  toRelationDrafts,
} from "../utils/dataDictionaryRelations";

interface DataDictionaryGetResponse {
  dictionary: DataDictionarySummary;
  fields: DataDictionaryField[];
  relations: DataDictionaryRelation[];
}

const dictionaries = ref<DataDictionarySummary[]>([]);
const selectedId = ref<number | null>(null);
const currentDictionary = ref<DataDictionarySummary | null>(null);
const fields = ref<DataDictionaryField[]>([]);
const fieldCache = ref<Record<number, DataDictionaryField[]>>({});
const searchScope = ref<DataDictionarySearchScope>("all");
const keyword = ref("");
const searchItems = ref<DataDictionarySearchItem[]>([]);
const selectedItem = ref<DataDictionarySearchItem | null>(null);
const recordDetail = ref<DataDictionaryRecordDetail | null>(null);
const detailLoading = ref(false);
const detailError = ref("");
const searching = ref(false);
const searchHasMore = ref(false);

const importDialogVisible = ref(false);
const importMode = ref<"create" | "replace">("create");
const importForm = ref({ name: "", description: "", input: "" });
const preview = ref<DataDictionaryImportPreview | null>(null);
const previewInput = ref("");
const replaceTarget = ref<DataDictionarySummary | null>(null);
const previewing = ref(false);
const savingImport = ref(false);

const fieldDrawerVisible = ref(false);
const fieldTarget = ref<DataDictionarySummary | null>(null);
const fieldDrafts = ref<DataDictionaryField[]>([]);
const fieldRelationDrafts = ref<DataDictionaryRelationDraft[]>([]);
const fieldPrimaryPath = ref("");
const fieldTitlePath = ref("");
const fieldSortPath = ref("");
const fieldSortDirection = ref<DataDictionarySortDirection>("asc");
const savingFields = ref(false);
const savingDictionaryOrder = ref(false);
const dictionarySortListRef = ref<HTMLElement | null>(null);
const visibleFieldTableRef = ref<ComponentPublicInstance | null>(null);

type DictionaryMenuInstance = ComponentPublicInstance & { handleClose?: () => void };

const dictionaryMenuRefs = new Map<number, DictionaryMenuInstance>();
let dictionarySortableInstance: Sortable | null = null;
let fieldSortableInstance: Sortable | null = null;

let searchTimer: ReturnType<typeof setTimeout> | null = null;
let dictionaryRequestSeq = 0;
let searchRequestSeq = 0;
let detailRequestSeq = 0;

const totalRecordCount = computed(() =>
  dictionaries.value.reduce((total, dictionary) => total + dictionary.recordCount, 0),
);

const selectedJson = computed(() =>
  recordDetail.value ? formatJsonDocument(recordDetail.value.record.rawJson) : "",
);

const relationGroups = computed(() =>
  recordDetail.value
    ? [...recordDetail.value.forwardRelations, ...recordDetail.value.reverseRelations]
    : [],
);

const canSaveImport = computed(
  () => Boolean(preview.value) && previewInput.value === importForm.value.input,
);

const fieldDrawerTitle = computed(() =>
  fieldTarget.value ? `字段配置 - ${fieldTarget.value.name}` : "字段配置",
);

const fieldDrawerSize = computed(() => "min(1040px, calc(100vw - 48px))");

const fieldSortOptions = computed(() =>
  fieldDrafts.value.map((field) => ({
    label: field.displayName.trim()
      ? `${field.displayName.trim()}（${field.fieldPath}）`
      : field.fieldPath,
    value: field.fieldPath,
  })),
);

const visibleFieldDrafts = computed(() => fieldDrafts.value.filter((field) => field.visible));
const hiddenFieldDrafts = computed(() => fieldDrafts.value.filter((field) => !field.visible));

const sortDirectionOptions = [
  { label: "升序", value: "asc" },
  { label: "降序", value: "desc" },
] as const;

async function ipc<T>(channel: string, payload: Record<string, unknown> = {}): Promise<T> {
  return (await invokeToolByChannel(channel, payload)) as T;
}

async function loadDictionaries(preferredId?: number) {
  const result = await ipc<{ items: DataDictionarySummary[] }>("tool:data-dictionary:list");
  dictionaries.value = result.items;
  const nextId =
    [preferredId, searchScope.value === "current" ? selectedId.value : null].find(
      (id): id is number =>
        typeof id === "number" && result.items.some((item) => item.id === id),
    ) ?? null;
  if (nextId) {
    await selectDictionary(nextId);
  } else {
    await selectAllDictionaries();
  }
  await nextTick();
  initDictionarySortable();
}

async function selectDictionary(id: number) {
  const requestId = ++dictionaryRequestSeq;
  searchScope.value = "current";
  selectedId.value = id;
  currentDictionary.value = null;
  fields.value = [];
  searchItems.value = [];
  selectedItem.value = null;
  resetRecordDetail();
  searchHasMore.value = false;
  const result = await ipc<DataDictionaryGetResponse>(
    "tool:data-dictionary:get",
    { id },
  );
  if (requestId !== dictionaryRequestSeq || selectedId.value !== id) return;
  currentDictionary.value = result.dictionary;
  fields.value = result.fields;
  fieldCache.value = { ...fieldCache.value, [id]: result.fields };
  if (searchScope.value === "current") {
    await runSearch();
  }
}

async function selectAllDictionaries() {
  dictionaryRequestSeq += 1;
  searchScope.value = "all";
  selectedId.value = null;
  currentDictionary.value = null;
  fields.value = [];
  searchItems.value = [];
  selectedItem.value = null;
  resetRecordDetail();
  searchHasMore.value = false;
  await runSearch();
}

function scheduleSearch() {
  if (searchTimer) clearTimeout(searchTimer);
  searchTimer = setTimeout(() => {
    void runSearch();
  }, 260);
}

async function runSearch() {
  if (searchTimer) {
    clearTimeout(searchTimer);
    searchTimer = null;
  }
  const requestId = ++searchRequestSeq;
  const scope = searchScope.value;
  const dictionaryId = selectedId.value;
  const searchKeyword = keyword.value;
  if (searchScope.value === "current" && !selectedId.value) {
    searchItems.value = [];
    selectedItem.value = null;
    resetRecordDetail();
    searchHasMore.value = false;
    return;
  }
  searching.value = true;
  try {
    const result = await ipc<DataDictionarySearchResult>("tool:data-dictionary:search", {
      scope,
      dictionaryId: dictionaryId ?? undefined,
      keyword: searchKeyword,
      limit: 100,
    });
    if (!isCurrentSearchRequest(requestId, scope, dictionaryId, searchKeyword)) return;
    await ensureFieldCache(result.items);
    if (!isCurrentSearchRequest(requestId, scope, dictionaryId, searchKeyword)) return;
    searchItems.value = result.items;
    searchHasMore.value = result.hasMore;
    if (result.items[0]) {
      await selectSearchItem(result.items[0]);
    } else {
      selectedItem.value = null;
      resetRecordDetail();
    }
  } catch (error) {
    if (isCurrentSearchRequest(requestId, scope, dictionaryId, searchKeyword)) {
      ElMessage.error((error as Error).message || "搜索失败");
    }
  } finally {
    if (requestId === searchRequestSeq) {
      searching.value = false;
    }
  }
}

function resetRecordDetail() {
  detailRequestSeq += 1;
  recordDetail.value = null;
  detailError.value = "";
  detailLoading.value = false;
}

async function selectSearchItem(item: DataDictionarySearchItem) {
  selectedItem.value = item;
  await loadRecordDetail(item);
}

async function loadRelatedRecord(recordId: number) {
  await loadRecordDetailById(recordId);
}

async function loadRecordDetail(item: DataDictionarySearchItem) {
  await loadRecordDetailById(item.id);
}

async function loadRecordDetailById(recordId: number) {
  const requestId = ++detailRequestSeq;
  detailLoading.value = true;
  detailError.value = "";
  try {
    const result = await ipc<DataDictionaryRecordDetail>("tool:data-dictionary:record-detail", {
      recordId,
    });
    if (requestId !== detailRequestSeq) return;
    recordDetail.value = result;
  } catch (error) {
    if (requestId === detailRequestSeq) {
      recordDetail.value = null;
      detailError.value = (error as Error).message || "加载详情失败";
    }
  } finally {
    if (requestId === detailRequestSeq) {
      detailLoading.value = false;
    }
  }
}

function isCurrentSearchRequest(
  requestId: number,
  scope: DataDictionarySearchScope,
  dictionaryId: number | null,
  searchKeyword: string,
) {
  return (
    requestId === searchRequestSeq &&
    scope === searchScope.value &&
    dictionaryId === selectedId.value &&
    searchKeyword === keyword.value
  );
}

async function ensureFieldCache(items: DataDictionarySearchItem[]) {
  const missing = Array.from(
    new Set(items.map((item) => item.dictionaryId).filter((id) => !fieldCache.value[id])),
  );
  for (const id of missing) {
    const result = await ipc<DataDictionaryGetResponse>(
      "tool:data-dictionary:get",
      { id },
    );
    fieldCache.value = { ...fieldCache.value, [id]: result.fields };
  }
}

function resultSummary(item: DataDictionarySearchItem) {
  return buildResultSummary(
    item,
    fieldCache.value[item.dictionaryId] ?? [],
    item.titleFieldPath,
  );
}

function resultTitle(item: DataDictionarySearchItem) {
  return buildResultTitle(item);
}

async function copySummaryValue(value: string) {
  try {
    await navigator.clipboard.writeText(value);
    ElMessage.success("已复制");
  } catch {
    ElMessage.error("复制失败");
  }
}

function initDictionarySortable() {
  if (dictionarySortableInstance) return;
  const listEl = dictionarySortListRef.value;
  if (!listEl) return;

  dictionarySortableInstance = Sortable.create(listEl, {
    animation: 150,
    handle: ".dd-dictionary-drag-handle",
    draggable: ".dd-dictionary-menu",
    ghostClass: "dd-sortable-ghost",
    forceFallback: true,
    disabled: savingDictionaryOrder.value,
    onEnd: (event) => {
      void handleDictionarySortEnd(event);
    },
  });
}

function destroyDictionarySortable() {
  dictionarySortableInstance?.destroy();
  dictionarySortableInstance = null;
}

function initFieldSortable(retries = 5) {
  if (fieldSortableInstance) return;
  const tableEl = visibleFieldTableRef.value?.$el as HTMLElement | undefined;
  const bodyEl = tableEl?.querySelector(".el-table__body-wrapper tbody") as HTMLElement | null;
  if (!bodyEl || bodyEl.children.length === 0) {
    if (visibleFieldDrafts.value.length === 0) return;
    if (fieldDrawerVisible.value && retries > 0) {
      setTimeout(() => initFieldSortable(retries - 1), 120);
    }
    return;
  }

  fieldSortableInstance = Sortable.create(bodyEl, {
    animation: 150,
    handle: ".dd-field-drag-handle",
    draggable: "tr",
    ghostClass: "dd-sortable-ghost",
    forceFallback: true,
    onEnd: (event) => {
      void handleFieldSortEnd(event);
    },
  });
}

function destroyFieldSortable() {
  fieldSortableInstance?.destroy();
  fieldSortableInstance = null;
}

async function handleFieldSortEnd(event: Sortable.SortableEvent) {
  const { oldIndex, newIndex } = event;
  if (oldIndex == null || newIndex == null) return;
  fieldDrafts.value = moveDataDictionaryFieldDraft(fieldDrafts.value, oldIndex, newIndex);
  await nextTick();
}

async function setFieldVisible(fieldPath: string, visible: boolean) {
  fieldDrafts.value = setDataDictionaryFieldVisibility(
    fieldDrafts.value,
    fieldPath,
    visible,
  );
  await nextTick();
  destroyFieldSortable();
  initFieldSortable();
}

function addRelationDraft() {
  fieldRelationDrafts.value.push({
    sourceFieldPath: "",
    targetDictionaryId: null,
    relationName: "",
    reverseName: "",
  });
}

function removeRelationDraft(index: number) {
  fieldRelationDrafts.value.splice(index, 1);
}

function validateRelationDrafts(): string {
  for (const relation of fieldRelationDrafts.value) {
    if (!relation.sourceFieldPath) return "请选择关系源字段";
    if (!relation.relationName.trim()) return "请输入关系名";
    if (!relation.targetDictionaryId) return "请选择目标字典";
    if (hasInvalidRelationTarget(relation, dictionaries.value)) return "目标字典未配置主键";
    if (!relation.reverseName.trim()) return "请输入反向关系名";
    if (
      relation.targetDictionaryId === fieldTarget.value?.id &&
      relation.sourceFieldPath === fieldPrimaryPath.value
    ) {
      return "自引用关系的源字段不能等于主键字段";
    }
  }
  if (duplicateRelationKeys(fieldRelationDrafts.value).length > 0) {
    return "同一源字段和目标字典不能重复配置关系";
  }
  return "";
}

async function handleDictionarySortEnd(event: Sortable.SortableEvent) {
  const { oldIndex, newIndex } = event;
  if (oldIndex == null || newIndex == null || oldIndex === newIndex) return;

  const previousOrder = dictionaries.value.slice();
  const next = dictionaries.value.slice();
  const [moved] = next.splice(oldIndex, 1);
  if (!moved) return;
  next.splice(newIndex, 0, moved);
  dictionaries.value = next;
  await saveDictionaryOrder(previousOrder);
}

async function saveDictionaryOrder(previousOrder: DataDictionarySummary[]) {
  savingDictionaryOrder.value = true;
  try {
    await ipc("tool:data-dictionary:reorder", {
      ids: dictionaries.value.map((dictionary) => dictionary.id),
    });
  } catch (error) {
    dictionaries.value = previousOrder;
    ElMessage.error((error as Error).message || "保存字典排序失败");
    try {
      await loadDictionaries(selectedId.value ?? undefined);
    } catch {
      // Keep the last known local order if reload also fails.
    }
  } finally {
    savingDictionaryOrder.value = false;
  }
}

function setDictionaryMenuRef(id: number, el: Element | ComponentPublicInstance | null) {
  if (!el) {
    dictionaryMenuRefs.delete(id);
    return;
  }
  if ("handleClose" in el) {
    dictionaryMenuRefs.set(id, el as DictionaryMenuInstance);
  } else {
    dictionaryMenuRefs.delete(id);
  }
}

function handleDictionaryMenuVisibleChange(visible: boolean, id: number) {
  if (visible) {
    closeOtherDictionaryMenus(id);
  }
}

function closeOtherDictionaryMenus(activeId: number) {
  for (const [id, menu] of dictionaryMenuRefs) {
    if (id === activeId) continue;
    menu?.handleClose();
  }
}

function handleDictionaryCommand(
  command: string | number | object,
  dictionary: DataDictionarySummary,
) {
  dispatchDictionaryMenuCommand(command, {
    replace: () => openReplaceDialog(dictionary),
    fields: () => openFieldDrawer(dictionary),
    rebuild: () => rebuildDictionaryIndexes(dictionary),
    rename: () => renameDictionary(dictionary),
    remove: () => deleteDictionary(dictionary),
  });
}

function openCreateDialog() {
  importMode.value = "create";
  importForm.value = { name: "", description: "", input: "" };
  preview.value = null;
  previewInput.value = "";
  replaceTarget.value = null;
  importDialogVisible.value = true;
}

function openReplaceDialog(target: DataDictionarySummary) {
  importMode.value = "replace";
  importForm.value = { name: target.name, description: "", input: "" };
  preview.value = null;
  previewInput.value = "";
  replaceTarget.value = target;
  importDialogVisible.value = true;
}

function invalidateImportPreview() {
  if (preview.value && previewInput.value !== importForm.value.input) {
    preview.value = null;
    previewInput.value = "";
  }
}

async function previewImport() {
  previewing.value = true;
  try {
    preview.value = await ipc<DataDictionaryImportPreview>("tool:data-dictionary:import-preview", {
      input: importForm.value.input,
    });
    previewInput.value = importForm.value.input;
  } catch (error) {
    preview.value = null;
    previewInput.value = "";
    ElMessage.error((error as Error).message || "预览失败");
  } finally {
    previewing.value = false;
  }
}

async function saveImport() {
  if (!canSaveImport.value || !preview.value) {
    ElMessage.warning("请先预览当前输入");
    return;
  }
  const target = replaceTarget.value;
  if (importMode.value === "replace") {
    if (!target) return;
    try {
      await ElMessageBox.confirm(
        `将清空「${target.name}」现有 ${target.recordCount} 条记录，并写入 ${preview.value.recordCount} 条新记录。`,
        "确认替换字典数据",
        {
          type: "warning",
          confirmButtonText: "确认替换",
          cancelButtonText: "取消",
        },
      );
    } catch (error) {
      if ((error as string) !== "cancel") {
        ElMessage.error((error as Error).message || "确认失败");
      }
      return;
    }
  }
  savingImport.value = true;
  try {
    if (importMode.value === "create") {
      const created = await ipc<DataDictionaryImportWriteResult>("tool:data-dictionary:create", {
        name: importForm.value.name,
        description: importForm.value.description,
        input: importForm.value.input,
      });
      importDialogVisible.value = false;
      await loadDictionaries(created.id);
      ElMessage.success(importWriteMessage("已导入", created));
    } else if (target) {
      const replaced = await ipc<DataDictionaryImportWriteResult>("tool:data-dictionary:replace-records", {
        dictionaryId: target.id,
        input: importForm.value.input,
      });
      importDialogVisible.value = false;
      await loadDictionaries(target.id);
      ElMessage.success(importWriteMessage("已替换", replaced));
    }
    await runSearch();
  } catch (error) {
    ElMessage.error((error as Error).message || "保存失败");
  } finally {
    savingImport.value = false;
  }
}

function importWriteMessage(prefix: string, result: DataDictionaryImportWriteResult): string {
  if (result.skippedPrimaryRecordCount > 0) {
    return `${prefix} ${result.recordCount} 条记录，${result.skippedPrimaryRecordCount} 条主键异常记录未导入`;
  }
  return `${prefix} ${result.recordCount} 条记录`;
}

async function openFieldDrawer(target: DataDictionarySummary) {
  destroyFieldSortable();
  fieldTarget.value = target;
  fieldDrafts.value = [];
  fieldRelationDrafts.value = [];
  fieldPrimaryPath.value = target.primaryFieldPath ?? "";
  fieldTitlePath.value = target.titleFieldPath ?? "";
  fieldSortPath.value = target.sortFieldPath ?? "";
  fieldSortDirection.value = target.sortDirection === "desc" ? "desc" : "asc";
  fieldDrawerVisible.value = true;
  try {
    const result = await ipc<DataDictionaryGetResponse>("tool:data-dictionary:get", {
      id: target.id,
    });
    fieldCache.value = { ...fieldCache.value, [target.id]: result.fields };
    if (selectedId.value === target.id) {
      currentDictionary.value = result.dictionary;
      fields.value = result.fields;
    }
    fieldDrafts.value = orderDataDictionaryFieldDrafts(result.fields);
    fieldRelationDrafts.value = toRelationDrafts(result.relations);
    fieldPrimaryPath.value = result.dictionary.primaryFieldPath ?? "";
    fieldTitlePath.value = result.dictionary.titleFieldPath ?? "";
    fieldSortPath.value = result.dictionary.sortFieldPath ?? "";
    fieldSortDirection.value = result.dictionary.sortDirection === "desc" ? "desc" : "asc";
    await nextTick();
    initFieldSortable();
  } catch (error) {
    fieldDrawerVisible.value = false;
    fieldTarget.value = null;
    ElMessage.error((error as Error).message || "加载字段失败");
  }
}

async function saveFields() {
  const target = fieldTarget.value;
  if (!target) return;
  const relationError = validateRelationDrafts();
  if (relationError) {
    ElMessage.warning(relationError);
    return;
  }
  savingFields.value = true;
  try {
    const fieldsToSave = orderDataDictionaryFieldDrafts(fieldDrafts.value);
    const result = await ipc<DataDictionaryImportWriteResult>("tool:data-dictionary:update-fields", {
      dictionaryId: target.id,
      fields: fieldsToSave,
      primaryFieldPath: fieldPrimaryPath.value || null,
      titleFieldPath: fieldTitlePath.value || null,
      sortFieldPath: fieldSortPath.value || null,
      sortDirection: fieldSortDirection.value,
      relations: fieldRelationDrafts.value.map((relation) => ({
        sourceFieldPath: relation.sourceFieldPath,
        targetDictionaryId: relation.targetDictionaryId,
        relationName: relation.relationName,
        reverseName: relation.reverseName,
      })),
    });
    fieldDrawerVisible.value = false;
    const refreshed = await ipc<DataDictionaryGetResponse>(
      "tool:data-dictionary:get",
      { id: target.id },
    );
    fieldCache.value = { ...fieldCache.value, [target.id]: refreshed.fields };
    dictionaries.value = dictionaries.value.map((dictionary) =>
      dictionary.id === target.id ? refreshed.dictionary : dictionary,
    );
    fieldTarget.value = refreshed.dictionary;
    if (selectedId.value === target.id) {
      currentDictionary.value = refreshed.dictionary;
      fields.value = refreshed.fields;
    }
    await runSearch();
    ElMessage.success(
      result.skippedPrimaryRecordCount > 0
        ? `字段配置已保存，${result.skippedPrimaryRecordCount} 条主键异常记录未纳入字典`
        : "字段配置已保存",
    );
  } catch (error) {
    ElMessage.error((error as Error).message || "保存字段失败");
  } finally {
    savingFields.value = false;
  }
}

function handleFieldDrawerClosed() {
  destroyFieldSortable();
}

async function rebuildDictionaryIndexes(target: DataDictionarySummary) {
  try {
    await ElMessageBox.confirm(
      `将使用「${target.name}」的原始 JSON 重建字段值索引和搜索索引。\n不会修改原始记录、字段配置和关系配置。`,
      "重建索引",
      {
        type: "warning",
        confirmButtonText: "重建",
        cancelButtonText: "取消",
      },
    );
    const result = await ipc<RebuildDataDictionaryIndexesResult>(
      "tool:data-dictionary:rebuild-indexes",
      { dictionaryId: target.id },
    );
    const suffix =
      result.skippedPrimaryRecordCount > 0
        ? `。${result.skippedPrimaryRecordCount} 条主键异常记录不会参与关系匹配`
        : "";
    ElMessage.success(
      `已重建索引：${result.recordCount} 条记录，${result.valueCount} 个字段值${suffix}`,
    );
    await runSearch();
  } catch (error) {
    if ((error as string) !== "cancel") {
      ElMessage.error((error as Error).message || "重建索引失败");
    }
  }
}

async function renameDictionary(target: DataDictionarySummary) {
  try {
    const { value } = await ElMessageBox.prompt("字典名称", "重命名", {
      inputValue: target.name,
      inputValidator: (text) => Boolean(text.trim()) || "请输入字典名称",
    });
    await ipc("tool:data-dictionary:rename", {
      id: target.id,
      name: value,
      description: target.description,
    });
    await loadDictionaries(target.id);
    await runSearch();
    ElMessage.success("已重命名");
  } catch (error) {
    if ((error as string) !== "cancel") {
      ElMessage.error((error as Error).message || "重命名失败");
    }
  }
}

async function deleteDictionary(target: DataDictionarySummary) {
  try {
    await ElMessageBox.confirm(`删除「${target.name}」？`, "删除字典", {
      type: "warning",
      confirmButtonText: "删除",
      cancelButtonText: "取消",
    });
    await ipc("tool:data-dictionary:delete", { id: target.id });
    const { [target.id]: _removedFields, ...restFieldCache } = fieldCache.value;
    fieldCache.value = restFieldCache;
    if (fieldTarget.value?.id === target.id) fieldTarget.value = null;
    await loadDictionaries(selectedId.value ?? undefined);
    await runSearch();
    ElMessage.success("已删除");
  } catch (error) {
    if ((error as string) !== "cancel") {
      ElMessage.error((error as Error).message || "删除失败");
    }
  }
}

onMounted(() => {
  void loadDictionaries();
});

onBeforeUnmount(() => {
  if (searchTimer) clearTimeout(searchTimer);
  destroyDictionarySortable();
  destroyFieldSortable();
});

watch(savingDictionaryOrder, (disabled) => {
  dictionarySortableInstance?.option("disabled", disabled);
});
</script>

<style scoped>
.data-dictionary-panel {
  display: grid;
  grid-template-columns: 260px minmax(360px, 1fr) minmax(320px, 38%);
  gap: 12px;
  height: 100%;
  min-height: 0;
  color: #172033;
}

.dd-sidebar,
.dd-results,
.dd-detail {
  min-height: 0;
  background: #ffffff;
  border: 1px solid #e5e9f2;
  border-radius: 8px;
}

.dd-sidebar {
  display: flex;
  flex-direction: column;
  padding: 14px;
}

.dd-sidebar-head,
.dd-toolbar,
.dd-detail-head {
  display: flex;
  align-items: center;
  gap: 10px;
}

.dd-sidebar-head {
  justify-content: space-between;
  margin-bottom: 12px;
}

.dd-sidebar-head h2,
.dd-detail-head h3 {
  margin: 0;
  font-size: 16px;
  line-height: 1.4;
}

.dd-sidebar-head span,
.dd-detail-head span,
.dd-dictionary-meta {
  color: #697386;
  font-size: 12px;
}

.dd-dictionary-list {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 6px;
  min-height: 0;
  overflow: auto;
}

.dd-dictionary-sort-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.dd-dictionary-sort-list.is-saving-order {
  opacity: 0.72;
}

.dd-dictionary-menu {
  display: block;
  width: 100%;
}

.dd-dictionary-item,
.dd-result-item {
  width: 100%;
  border: 1px solid transparent;
  background: transparent;
  color: inherit;
  cursor: pointer;
  text-align: left;
  transition: background-color 0.16s ease, border-color 0.16s ease;
}

.dd-dictionary-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 42px;
  padding: 8px 10px;
  border-radius: 7px;
}

.dd-dictionary-item:hover,
.dd-dictionary-item.active,
.dd-result-item:hover,
.dd-result-item.active {
  border-color: #c8d8ff;
  background: #f4f7ff;
}

.dd-dictionary-name {
  min-width: 0;
  overflow: hidden;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dd-dictionary-trailing {
  display: inline-flex;
  flex-shrink: 0;
  align-items: center;
  gap: 8px;
  margin-left: 8px;
}

.dd-dictionary-drag-handle {
  color: #697386;
  cursor: grab;
  font-size: 16px;
  opacity: 0.45;
  transition: opacity 0.16s ease;
}

.dd-dictionary-item:hover .dd-dictionary-drag-handle {
  opacity: 1;
}

.dd-dictionary-drag-handle:active {
  cursor: grabbing;
}

:deep(.dd-sortable-ghost) .dd-dictionary-item {
  border-color: #6f95ff;
  background: #eef4ff;
}

.dd-results {
  display: flex;
  flex-direction: column;
  padding: 14px;
}

.dd-toolbar {
  margin-bottom: 10px;
}

.dd-search-input {
  flex: 1;
}

.dd-result-list {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
  overflow: auto;
}

.dd-result-item {
  padding: 10px 12px;
  border-color: #edf0f6;
  border-radius: 8px;
}

.dd-result-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 8px;
  font-weight: 600;
}

.dd-result-title {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
}

.dd-result-title-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dd-summary-line {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.dd-summary-part {
  max-width: 100%;
  overflow: hidden;
  padding: 3px 7px;
  border-radius: 6px;
  background: #f6f8fb;
  color: #435066;
  cursor: copy;
  font-size: 12px;
  text-overflow: ellipsis;
  transition: background-color 0.16s ease, color 0.16s ease;
  white-space: nowrap;
}

.dd-summary-part:hover {
  background: #eaf0ff;
  color: #1f3f8f;
}

.dd-summary-part b {
  margin-right: 5px;
  color: #1f2a44;
}

.dd-detail {
  display: flex;
  flex-direction: column;
  padding: 14px;
}

.dd-detail-head {
  justify-content: space-between;
  margin-bottom: 10px;
}

.dd-match-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: 10px;
}

.dd-match-tag {
  max-width: 100%;
}

.dd-detail-error {
  margin-bottom: 10px;
}

.dd-relation-groups {
  display: flex;
  flex-direction: column;
  gap: 10px;
  max-height: 38%;
  min-height: 0;
  margin-bottom: 10px;
  overflow: auto;
}

.dd-relation-group {
  border: 1px solid #e2e8f4;
  border-radius: 8px;
  background: #ffffff;
}

.dd-relation-group-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 8px 10px;
  border-bottom: 1px solid #edf1f7;
  background: #fbfcff;
}

.dd-relation-group-head h4 {
  margin: 0;
  font-size: 13px;
}

.dd-relation-group-head span {
  color: #697386;
  font-size: 12px;
}

.dd-related-record {
  display: grid;
  width: 100%;
  gap: 3px;
  padding: 9px 10px;
  border: 0;
  border-bottom: 1px solid #f0f3f8;
  background: transparent;
  color: inherit;
  cursor: pointer;
  text-align: left;
}

.dd-related-record:hover {
  background: #f4f7ff;
}

.dd-related-record strong,
.dd-related-record span,
.dd-related-record small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dd-related-record span,
.dd-related-record small {
  color: #697386;
  font-size: 12px;
}

.dd-json-view {
  flex: 1;
  min-height: 0;
  margin: 0;
  overflow: auto;
  padding: 12px;
  border: 1px solid #e7eaf1;
  border-radius: 8px;
  background: #fbfcff;
  color: #263247;
  font-family: "Cascadia Mono", Consolas, monospace;
  font-size: 12px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
}

.dd-empty {
  display: grid;
  flex: 1;
  min-height: 120px;
  place-items: center;
  color: #8a94a6;
  font-size: 13px;
}

.dd-limit-hint {
  padding: 8px 10px;
  border: 1px solid #dbe5f5;
  border-radius: 7px;
  background: #f6f9ff;
  color: #52637a;
  font-size: 12px;
  line-height: 1.4;
}

.dd-import-form {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.dd-preview {
  display: flex;
  gap: 12px;
  color: #435066;
  font-size: 13px;
}

.dd-field-drawer :deep(.el-drawer__body) {
  display: flex;
  min-height: 0;
  flex-direction: column;
  padding: 14px 16px 12px;
  background: #f7f9fc;
}

.dd-field-drawer :deep(.el-drawer__footer) {
  padding: 12px 16px;
  border-top: 1px solid #e5e9f2;
  background: #ffffff;
}

.dd-field-config-panel {
  margin-bottom: 12px;
  padding: 12px;
  border: 1px solid #dde5f3;
  border-radius: 8px;
  background: #ffffff;
}

.dd-field-config-summary {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
  margin-bottom: 10px;
}

.dd-field-config-summary span {
  display: inline-flex;
  align-items: baseline;
  gap: 4px;
  padding: 4px 8px;
  border: 1px solid #e2e8f4;
  border-radius: 999px;
  background: #f8fafc;
  color: #52637a;
  font-size: 12px;
  line-height: 1.2;
}

.dd-field-config-summary b {
  color: #1f3f8f;
  font-size: 13px;
}

.dd-field-sort-config {
  display: grid;
  grid-template-columns: repeat(3, minmax(180px, 1fr)) 160px;
  gap: 10px;
}

.dd-field-sort-config :deep(.el-form-item) {
  margin-bottom: 0;
}

.dd-field-sort-config :deep(.el-select),
.dd-field-sort-config :deep(.el-radio-group) {
  width: 100%;
}

.dd-relation-editor {
  margin-bottom: 12px;
  overflow: hidden;
  border: 1px solid #dde5f3;
  border-radius: 8px;
  background: #ffffff;
}

.dd-relation-row {
  display: grid;
  grid-template-columns: minmax(160px, 1fr) minmax(140px, 0.8fr) minmax(180px, 1fr) minmax(140px, 0.8fr) auto;
  gap: 8px;
  align-items: start;
  padding: 10px 12px;
  border-top: 1px solid #edf1f7;
}

.dd-relation-target {
  display: grid;
  gap: 4px;
}

.dd-relation-target span {
  color: #697386;
  font-size: 12px;
  line-height: 1.3;
}

.dd-relation-target span.is-error {
  color: #b42318;
}

.dd-relation-empty {
  min-height: 52px;
}

.dd-field-sections {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 12px;
  min-height: 0;
}

.dd-field-section {
  min-height: 0;
  overflow: hidden;
  border: 1px solid #dde5f3;
  border-radius: 8px;
  background: #ffffff;
}

.dd-field-section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 10px 12px;
  border-bottom: 1px solid #edf1f7;
  background: #fbfcff;
}

.dd-field-section-head h4 {
  margin: 0;
  color: #1f2a44;
  font-size: 13px;
  line-height: 1.4;
}

.dd-field-section-head span {
  color: #697386;
  font-size: 12px;
}

.dd-field-section :deep(.el-table) {
  --el-table-border-color: #edf1f7;
  --el-table-header-bg-color: #f8fafc;
  --el-table-row-hover-bg-color: #f4f7ff;
  color: #263247;
}

.dd-field-section :deep(.el-table th.el-table__cell) {
  color: #52637a;
  font-weight: 600;
}

.dd-field-section :deep(.el-table .cell) {
  line-height: 1.35;
}

.dd-field-section :deep(.el-input__wrapper) {
  background: #ffffff;
  box-shadow: 0 0 0 1px #dbe3f0 inset;
}

.dd-field-section :deep(.el-input__wrapper:hover) {
  box-shadow: 0 0 0 1px #b9c8e3 inset;
}

:deep(.dd-field-handle-cell) {
  background: #fbfcff;
}

.dd-field-drag-handle {
  color: #697386;
  cursor: grab;
  font-size: 17px;
  opacity: 0.52;
  transition: color 0.16s ease, opacity 0.16s ease;
}

:deep(.el-table__row:hover) .dd-field-drag-handle {
  color: #1e40af;
  opacity: 1;
}

.dd-field-drag-handle:active {
  cursor: grabbing;
}

@media (max-width: 1180px) {
  .data-dictionary-panel {
    grid-template-columns: 220px minmax(320px, 1fr);
  }

  .dd-detail {
    grid-column: 1 / -1;
    min-height: 320px;
  }
}

@media (max-width: 760px) {
  .dd-field-drawer :deep(.el-drawer__body) {
    padding: 12px;
  }

  .dd-field-sort-config {
    grid-template-columns: 1fr;
  }

  .dd-relation-row {
    grid-template-columns: 1fr;
  }

  .dd-field-config-summary {
    justify-content: flex-start;
  }
}
</style>

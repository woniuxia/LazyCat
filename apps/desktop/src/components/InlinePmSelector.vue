<template>
  <div class="inline-pm-selector">
    <!-- 空态：无项目 + 无工作项 -->
    <template v-if="viewState === 'empty'">
      <div class="pm-empty-card" @click="openSearch('project')">
        <div class="pm-empty-icon">+</div>
        <div class="pm-empty-title">关联项目和工作项</div>
      </div>
    </template>

    <!-- 仅项目态：有项目 + 无工作项 -->
    <template v-else-if="viewState === 'project-only'">
      <div class="pm-project-only-card">
        <span class="proj-dot" :style="{ background: projectColor || '#909399' }" />
        <span class="pm-project-only-name">{{ projectName || '未命名项目' }}</span>
        <span class="pm-only-tag">仅项目</span>
        <button type="button" class="pm-close-btn" title="清除项目关联" @click="emit('clear-all')">
          <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
            <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
          </svg>
        </button>
      </div>
      <div class="pm-actions-row">
        <el-button size="small" link type="primary" @click="openSearch('work')">
          + 补充具体工作项
        </el-button>
        <el-button size="small" link class="muted-link" @click="openSearch('project')">
          切换项目
        </el-button>
      </div>
    </template>

    <!-- 已关联工作项态 -->
    <template v-else-if="viewState === 'linked'">
      <div class="pm-linked-card">
        <div class="pm-linked-body">
          <button type="button" class="pm-close-btn is-corner" title="解除关联" @click="emit('unlink')">
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
              <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
            </svg>
          </button>
          <div class="pm-linked-line-1">
            <el-tag
              v-if="pmItemStatus"
              size="small"
              effect="plain"
              :style="statusTagStyle(pmItemStatus)"
            >
              {{ pmStatusLabel(pmItemStatus) }}
            </el-tag>
            <span class="pm-linked-title" :title="pmItemTitle || ''">{{ pmItemTitle || `#${pmItemId}` }}</span>
          </div>
          <div v-if="projectName" class="pm-linked-line-2">
            <span class="pm-linked-project">
              <span class="proj-dot small" :style="{ background: projectColor || '#909399' }" />
              {{ projectName }}
            </span>
          </div>
        </div>
        <div class="pm-linked-actions">
          <el-button size="small" link type="primary" @click="openSearch('work')">
            切换关联
          </el-button>
        </div>
      </div>
    </template>

    <!-- 搜索/选择面板 -->
    <template v-else-if="viewState === 'searching'">
      <div class="pm-search-panel">
        <div class="pm-search-tabs">
          <div
            class="pm-search-tab"
            :class="{ 'is-active': activeTab === 'work' }"
            @click="setTab('work')"
          >
            工作项
          </div>
          <div
            class="pm-search-tab"
            :class="{ 'is-active': activeTab === 'project' }"
            @click="setTab('project')"
          >
            仅项目
          </div>
          <button type="button" class="pm-search-close" title="收起" @click="closeSearch">
            <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
              <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
            </svg>
          </button>
        </div>

        <!-- Tab: 工作项 -->
        <div v-if="activeTab === 'work'" class="pm-search-body">
          <el-input
            v-model="keyword"
            size="small"
            placeholder="搜索工作项标题"
            clearable
            @input="onSearchInput"
          />
          <div v-if="!projectId" class="pm-hint">
            请先在「仅项目」Tab 选择一个项目
          </div>
          <div v-else-if="candidatesLoading" class="pm-hint">搜索中...</div>
          <div v-else-if="candidates.length === 0" class="pm-hint">
            {{ keyword ? '未找到匹配的工作项' : '该项目暂无工作项' }}
          </div>
          <div v-else class="pm-cand-groups">
            <div v-for="group in groupedCandidates" :key="group.projectId ?? 0" class="pm-cand-group">
              <div class="pm-cand-group-head">
                <span class="proj-dot" :style="{ background: group.color || '#909399' }" />
                <span class="pm-cand-group-name">{{ group.name || '未命名项目' }}</span>
                <span class="pm-cand-group-count">{{ group.items.length }}</span>
              </div>
              <div class="pm-cand-group-body">
                <div
                  v-for="c in group.items"
                  :key="c.id"
                  class="pm-cand-row"
                  @click="emit('link', c.id)"
                >
                  <span class="pm-cand-status" :style="{ background: pmStatusColor(c.status) }" />
                  <span class="pm-cand-title" :title="c.title">{{ c.title }}</span>
                  <span class="pm-cand-meta">
                    <span class="prio-dot" :class="priorityClass(c.priority)" />
                  </span>
                </div>
              </div>
            </div>
          </div>
          <div class="pm-search-footer">
            <el-button
              size="small"
              link
              type="primary"
              :disabled="!projectId"
              @click="gotoCreate"
            >
              + 新建工作项
            </el-button>
          </div>
        </div>

        <!-- Tab: 仅项目 -->
        <div v-else class="pm-search-body">
          <el-input
            v-model="projectKeyword"
            size="small"
            placeholder="搜索项目名"
            clearable
          />
          <div v-if="filteredProjects.length === 0" class="pm-hint">
            {{ projectKeyword ? '未找到匹配的项目' : '暂无可用项目' }}
          </div>
          <div v-else class="pm-project-list">
            <div
              v-for="p in filteredProjects"
              :key="p.id"
              class="pm-project-row"
              :class="{ 'is-current': p.id === projectId }"
              @click="onPickProject(p.id)"
            >
              <span class="proj-dot" :style="{ background: p.color || '#909399' }" />
              <span class="pm-project-row-name">{{ p.name }}</span>
              <span v-if="p.id === projectId" class="pm-project-row-tag">当前</span>
            </div>
          </div>
        </div>
      </div>
    </template>

    <!-- 新建工作项态 -->
    <template v-else-if="viewState === 'creating'">
      <div class="pm-create-panel">
        <div class="pm-create-header">
          <span class="pm-create-title">新建工作项并关联</span>
          <el-button size="small" link class="muted-link" @click="viewState = 'searching'">
            返回
          </el-button>
        </div>
        <div class="pm-create-field">
          <label class="pm-create-label">所属项目</label>
          <el-select v-model="createProjectId" size="small" style="width: 100%">
            <el-option
              v-for="p in projectList"
              :key="p.id"
              :label="p.name"
              :value="p.id"
            />
          </el-select>
        </div>
        <div class="pm-create-field">
          <label class="pm-create-label">工作项标题</label>
          <el-input
            v-model="createTitle"
            size="small"
            placeholder="输入工作项标题"
            @keyup.enter="handleCreatePm"
          />
        </div>
        <div class="pm-create-actions">
          <el-button size="small" @click="viewState = 'searching'">取消</el-button>
          <el-button
            size="small"
            type="primary"
            :disabled="!createTitle.trim() || !createProjectId"
            @click="handleCreatePm"
          >
            创建并关联
          </el-button>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from "vue";
import type { PmCandidateItem } from "../types/pm";

type ViewState = "empty" | "project-only" | "linked" | "searching" | "creating";
type SearchTab = "work" | "project";

const props = defineProps<{
  projectId: number | null;
  projectName: string | null;
  projectColor: string | null;
  pmItemId: number | null;
  pmItemTitle: string | null;
  pmItemStatus: string | null;
  candidates: PmCandidateItem[];
  candidatesLoading: boolean;
  projectList: { id: number; name: string; color: string }[];
}>();

const emit = defineEmits<{
  link: [pmItemId: number];
  unlink: [];
  "create-pm": [title: string, projectId: number];
  search: [keyword: string];
  "change-project": [projectId: number];
  "clear-all": [];
}>();

const viewState = ref<ViewState>("empty");
const activeTab = ref<SearchTab>("work");
const keyword = ref("");
const projectKeyword = ref("");
const createTitle = ref("");
const createProjectId = ref<number | null>(null);

const hasPmLink = computed(() => props.pmItemId != null);
const hasProject = computed(() => props.projectId != null);

const restState = computed<ViewState>(() => {
  if (hasPmLink.value) return "linked";
  if (hasProject.value) return "project-only";
  return "empty";
});

watch(
  () => [props.pmItemId, props.projectId] as const,
  (newVals, oldVals) => {
    const [newPmId] = newVals;
    const oldPmId = oldVals?.[0];
    if (newPmId && newPmId !== oldPmId) {
      viewState.value = "linked";
      keyword.value = "";
      projectKeyword.value = "";
      return;
    }
    if (viewState.value === "searching" || viewState.value === "creating") return;
    viewState.value = restState.value;
  },
  { immediate: true },
);

watch(
  () => props.projectId,
  (val) => {
    if (val && !createProjectId.value) createProjectId.value = val;
  },
  { immediate: true },
);

function openSearch(initialTab: SearchTab = "work") {
  viewState.value = "searching";
  activeTab.value = hasProject.value ? initialTab : "project";
  keyword.value = "";
  projectKeyword.value = "";
  if (initialTab === "work" && hasProject.value) emit("search", "");
}

function closeSearch() {
  viewState.value = restState.value;
  keyword.value = "";
  projectKeyword.value = "";
}

function setTab(tab: SearchTab) {
  activeTab.value = tab;
  if (tab === "work" && hasProject.value) emit("search", keyword.value);
}

function onSearchInput() {
  emit("search", keyword.value);
}

function onPickProject(id: number) {
  emit("change-project", id);
  activeTab.value = "work";
  keyword.value = "";
  emit("search", "");
}

function gotoCreate() {
  if (!props.projectId) return;
  createProjectId.value = props.projectId;
  createTitle.value = "";
  viewState.value = "creating";
}

function handleCreatePm() {
  if (!createTitle.value.trim() || !createProjectId.value) return;
  emit("create-pm", createTitle.value.trim(), createProjectId.value);
  createTitle.value = "";
}

const filteredProjects = computed(() => {
  const kw = projectKeyword.value.trim().toLowerCase();
  if (!kw) return props.projectList;
  return props.projectList.filter((p) => p.name.toLowerCase().includes(kw));
});

interface CandGroup {
  projectId: number | null;
  name: string | null;
  color: string | null;
  items: PmCandidateItem[];
}

const groupedCandidates = computed<CandGroup[]>(() => {
  const map = new Map<number | "none", CandGroup>();
  for (const c of props.candidates) {
    const key = c.projectId ?? "none";
    if (!map.has(key)) {
      map.set(key, {
        projectId: c.projectId,
        name: c.projectName,
        color: c.projectColor,
        items: [],
      });
    }
    map.get(key)!.items.push(c);
  }
  return Array.from(map.values());
});

function pmStatusLabel(status: string | null | undefined): string {
  if (!status) return "未知";
  const map: Record<string, string> = {
    todo: "待办",
    in_progress: "进行中",
    testing: "测试中",
    done: "已完成",
    cancelled: "已取消",
  };
  return map[status] || status;
}

function pmStatusColor(status: string | null | undefined): string {
  if (!status) return "#909399";
  const map: Record<string, string> = {
    todo: "#909399",
    in_progress: "#409eff",
    testing: "#e6a23c",
    done: "#67c23a",
    cancelled: "#f56c6c",
  };
  return map[status] || "#909399";
}

function statusTagStyle(status: string): Record<string, string> {
  const color = pmStatusColor(status);
  return {
    backgroundColor: color + "1A",
    borderColor: color + "55",
    color,
  };
}

function priorityClass(priority: string | null | undefined): string {
  const p = (priority || "P2").toUpperCase();
  if (p === "P0") return "prio-dot--p0";
  if (p === "P1") return "prio-dot--p1";
  if (p === "P2") return "prio-dot--p2";
  return "prio-dot--p3";
}
</script>

<style scoped>
.inline-pm-selector {
  display: flex;
  flex-direction: column;
  gap: 8px;
  font-size: 13px;
  width: 100%;
}

/* --- 通用元素 --- */
.proj-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;
  display: inline-block;
}
.proj-dot.small {
  width: 7px;
  height: 7px;
}

.prio-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
  display: inline-block;
}
.prio-dot--p0 { background: #f56c6c; }
.prio-dot--p1 { background: #e6a23c; }
.prio-dot--p2 { background: #909399; }
.prio-dot--p3 { background: #c0c4cc; }

.muted-link {
  color: var(--el-text-color-secondary) !important;
}

.pm-close-btn {
  background: none;
  border: none;
  cursor: pointer;
  padding: 0;
  width: 22px;
  height: 22px;
  border-radius: 6px;
  color: var(--el-text-color-placeholder);
  display: inline-grid;
  place-items: center;
  transition: background 0.15s, color 0.15s;
  flex-shrink: 0;
}
.pm-close-btn:hover {
  background: var(--el-fill-color);
  color: var(--el-color-danger);
}
.pm-close-btn.is-corner {
  position: absolute;
  top: 10px;
  right: 10px;
}

/* --- 空态 --- */
.pm-empty-card {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border: 1px dashed var(--el-border-color);
  border-radius: 8px;
  cursor: pointer;
  background: var(--el-fill-color-lighter);
  transition: border-color 0.15s, background 0.15s;
}
.pm-empty-card:hover {
  border-color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
}
.pm-empty-icon {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: var(--el-fill-color);
  display: grid;
  place-items: center;
  font-size: 16px;
  font-weight: 300;
  color: var(--el-text-color-secondary);
  flex-shrink: 0;
  line-height: 1;
}
.pm-empty-title {
  font-size: 13px;
  color: var(--el-text-color-regular);
}

/* --- 仅项目态 --- */
.pm-project-only-card {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 5px 10px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  background: var(--el-bg-color);
  min-height: 32px;
}
.pm-project-only-name {
  flex: 1;
  font-size: 13px;
  font-weight: 500;
  color: var(--el-text-color-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pm-only-tag {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 4px;
  background: var(--el-fill-color);
  color: var(--el-text-color-placeholder);
  border: 1px solid var(--el-border-color-lighter);
}
.pm-actions-row {
  display: flex;
  gap: 14px;
  padding: 0 2px;
}

/* --- 已关联态 --- */
.pm-linked-card {
  display: flex;
  flex-direction: column;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 10px;
  background: var(--el-bg-color);
  overflow: hidden;
}
.pm-linked-body {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px 16px 12px;
}
.pm-linked-line-1 {
  display: flex;
  align-items: center;
  gap: 10px;
  padding-right: 30px;
}
.pm-linked-title {
  flex: 1;
  font-size: 14px;
  font-weight: 600;
  color: var(--el-text-color-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  line-height: 1.45;
}
.pm-linked-line-2 {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.pm-linked-project {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.pm-linked-actions {
  display: flex;
  gap: 14px;
  padding: 8px 16px;
  background: var(--el-fill-color-lighter);
  border-top: 1px solid var(--el-border-color-lighter);
}

/* --- 搜索面板 --- */
.pm-search-panel {
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 10px;
  background: var(--el-bg-color);
  overflow: hidden;
}
.pm-search-tabs {
  display: flex;
  align-items: center;
  padding: 0 12px;
  gap: 4px;
  background: var(--el-fill-color-lighter);
  border-bottom: 1px solid var(--el-border-color-lighter);
  position: relative;
}
.pm-search-tab {
  padding: 10px 12px;
  font-size: 13px;
  color: var(--el-text-color-secondary);
  border-bottom: 2px solid transparent;
  cursor: pointer;
  margin-bottom: -1px;
  transition: color 0.15s;
}
.pm-search-tab:hover {
  color: var(--el-text-color-primary);
}
.pm-search-tab.is-active {
  color: var(--el-color-primary);
  border-bottom-color: var(--el-color-primary);
  font-weight: 500;
}
.pm-search-close {
  margin-left: auto;
  background: none;
  border: none;
  cursor: pointer;
  color: var(--el-text-color-placeholder);
  padding: 4px;
  display: inline-grid;
  place-items: center;
  border-radius: 4px;
}
.pm-search-close:hover {
  color: var(--el-text-color-primary);
  background: var(--el-fill-color);
}
.pm-search-body {
  padding: 12px 14px 4px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.pm-hint {
  font-size: 12px;
  color: var(--el-text-color-placeholder);
  padding: 8px 0;
  text-align: center;
}

.pm-cand-groups {
  display: flex;
  flex-direction: column;
  gap: 14px;
  max-height: 260px;
  overflow-y: auto;
  padding-bottom: 2px;
}
.pm-cand-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.pm-cand-group-head {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  font-weight: 500;
  color: var(--el-text-color-regular);
  padding: 0 4px 5px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
.pm-cand-group-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pm-cand-group-count {
  font-size: 11px;
  color: var(--el-text-color-placeholder);
  font-weight: 400;
  margin-left: auto;
}
.pm-cand-group-body {
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.pm-cand-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 8px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.12s;
}
.pm-cand-row:hover {
  background: var(--el-fill-color-lighter);
}
.pm-cand-status {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.pm-cand-title {
  flex: 1;
  font-size: 13px;
  color: var(--el-text-color-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  line-height: 1.4;
}
.pm-cand-meta {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--el-text-color-placeholder);
}

.pm-search-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 4px 8px;
}

.pm-project-list {
  display: flex;
  flex-direction: column;
  gap: 1px;
  max-height: 260px;
  overflow-y: auto;
}
.pm-project-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 10px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.12s;
}
.pm-project-row:hover {
  background: var(--el-fill-color-lighter);
}
.pm-project-row.is-current {
  background: var(--el-color-primary-light-9);
}
.pm-project-row-name {
  flex: 1;
  font-size: 13px;
  color: var(--el-text-color-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pm-project-row-tag {
  font-size: 11px;
  color: var(--el-color-primary);
  padding: 1px 6px;
  background: var(--el-color-primary-light-9);
  border-radius: 3px;
}

/* --- 新建面板 --- */
.pm-create-panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px 16px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 10px;
  background: var(--el-bg-color);
}
.pm-create-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-bottom: 6px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
.pm-create-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}
.pm-create-field {
  display: flex;
  flex-direction: column;
  gap: 5px;
}
.pm-create-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.pm-create-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  padding-top: 4px;
}
</style>

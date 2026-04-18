<template>
  <div class="inline-pm-selector">
    <!-- 空态：无项目/无关联 -->
    <template v-if="state === 'empty'">
      <div class="inline-pm-empty" @click="state = 'searching'">
        <span class="inline-pm-empty-icon">+</span>
        <span class="inline-pm-empty-text">关联项目或工作项</span>
      </div>
    </template>

    <!-- 已关联态 -->
    <template v-else-if="state === 'linked'">
      <div class="inline-pm-card">
        <div class="inline-pm-card-body">
          <div v-if="projectName" class="inline-pm-project">
            <span class="inline-pm-project-dot" :style="{ backgroundColor: projectColor || '#909399' }" />
            <span class="inline-pm-project-name">{{ projectName }}</span>
          </div>
          <div v-if="pmItemTitle" class="inline-pm-item">
            <el-tag
              v-if="pmItemStatus"
              size="small"
              effect="plain"
              :style="statusTagStyle(pmItemStatus)"
            >
              {{ pmStatusLabel(pmItemStatus) }}
            </el-tag>
            <span class="inline-pm-item-title">{{ pmItemTitle }}</span>
          </div>
        </div>
        <button type="button" class="inline-pm-card-close" title="解除关联" @click="emit('unlink')">
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
            <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
          </svg>
        </button>
      </div>
      <el-button size="small" link type="primary" class="inline-pm-switch-link" @click="openSearch">
        切换关联
      </el-button>
    </template>

    <!-- 搜索态 -->
    <template v-else-if="state === 'searching'">
      <div class="inline-pm-search">
        <el-input
          v-model="keyword"
          size="small"
          placeholder="搜索工作项标题"
          clearable
          @input="emit('search', keyword)"
        />
        <div v-if="candidatesLoading" class="inline-pm-hint">搜索中...</div>
        <template v-else-if="candidates.length === 0">
          <div class="inline-pm-hint">{{ keyword ? '未找到匹配的工作项' : '暂无可用工作项' }}</div>
        </template>
        <div v-else class="inline-pm-candidates">
          <div
            v-for="c in candidates"
            :key="c.id"
            class="inline-pm-candidate-row"
            @click="emit('link', c.id)"
          >
            <el-tag size="small" effect="plain" :style="statusTagStyle(c.status)">
              {{ pmStatusLabel(c.status) }}
            </el-tag>
            <span class="inline-pm-candidate-title">{{ c.title }}</span>
          </div>
        </div>
        <div class="inline-pm-search-footer">
          <el-button size="small" link type="primary" @click="state = 'creating'">新建工作项并关联</el-button>
          <el-button size="small" link @click="state = hasPmLink ? 'linked' : 'empty'">收起</el-button>
        </div>
      </div>
    </template>

    <!-- 新建工作项态 -->
    <template v-else-if="state === 'creating'">
      <div class="inline-pm-create">
        <div class="inline-pm-create-header">
          <span class="inline-pm-create-title">新建工作项并关联</span>
          <el-button size="small" link @click="state = 'searching'">返回搜索</el-button>
        </div>
        <el-form-item label="所属项目" class="inline-pm-create-field">
          <el-select v-model="createProjectId" size="small" style="width: 100%;">
            <el-option
              v-for="p in projectList"
              :key="p.id"
              :label="p.name"
              :value="p.id"
            />
          </el-select>
        </el-form-item>
        <el-form-item label="工作项标题" class="inline-pm-create-field">
          <el-input
            v-model="createTitle"
            size="small"
            placeholder="输入工作项标题"
            @keyup.enter="handleCreatePm"
          />
        </el-form-item>
        <div class="inline-pm-create-actions">
          <el-button size="small" @click="state = 'searching'">取消</el-button>
          <el-button size="small" type="primary" :disabled="!createTitle.trim() || !createProjectId" @click="handleCreatePm">
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
}>();

const state = ref<"empty" | "linked" | "searching" | "creating">("empty");
const keyword = ref("");
const createTitle = ref("");
const createProjectId = ref<number | null>(null);

const hasPmLink = computed(() => props.pmItemId != null);

// Sync state with props
watch(() => props.pmItemId, (val) => {
  if (val) {
    state.value = "linked";
  } else if (state.value === "linked") {
    state.value = "empty";
  }
}, { immediate: true });

// Initialize createProjectId from current project
watch(() => props.projectId, (val) => {
  if (val && !createProjectId.value) {
    createProjectId.value = val;
  }
}, { immediate: true });

function openSearch() {
  state.value = "searching";
  keyword.value = "";
  emit("search", "");
}

function handleCreatePm() {
  if (!createTitle.value.trim() || !createProjectId.value) return;
  emit("create-pm", createTitle.value.trim(), createProjectId.value);
  createTitle.value = "";
}

function pmStatusLabel(status: string | null | undefined): string {
  if (!status) return "未知";
  const map: Record<string, string> = {
    todo: "待办", in_progress: "进行中", testing: "测试中", done: "已完成", cancelled: "已取消",
  };
  return map[status] || status;
}

function statusTagStyle(status: string): Record<string, string> {
  const color = pmStatusColor(status);
  return {
    marginRight: "6px",
    backgroundColor: color + "20",
    borderColor: color + "50",
    color,
  };
}

function pmStatusColor(status: string | null | undefined): string {
  if (!status) return "#909399";
  const map: Record<string, string> = {
    todo: "#909399", in_progress: "#409eff", testing: "#e6a23c", done: "#67c23a", cancelled: "#f56c6c",
  };
  return map[status] || "#909399";
}
</script>

<style scoped>
.inline-pm-selector {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

/* 空态 */
.inline-pm-empty {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 10px 12px;
  border: 1px dashed var(--el-border-color);
  border-radius: 8px;
  cursor: pointer;
  color: var(--el-text-color-secondary);
  font-size: 13px;
  transition: border-color 0.2s, color 0.2s;
}

.inline-pm-empty:hover {
  border-color: var(--el-color-primary);
  color: var(--el-color-primary);
}

.inline-pm-empty-icon {
  font-size: 16px;
  font-weight: 300;
}

.inline-pm-empty-text {
  line-height: 1;
}

/* 已关联态 */
.inline-pm-card {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 10px 12px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  background: var(--el-fill-color-lighter);
}

.inline-pm-card-body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.inline-pm-project {
  display: flex;
  align-items: center;
  gap: 6px;
}

.inline-pm-project-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.inline-pm-project-name {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.inline-pm-item {
  display: flex;
  align-items: center;
  gap: 6px;
}

.inline-pm-item-title {
  font-size: 13px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.inline-pm-card-close {
  flex-shrink: 0;
  background: none;
  border: none;
  cursor: pointer;
  padding: 2px;
  color: var(--el-text-color-placeholder);
  transition: color 0.15s;
}

.inline-pm-card-close:hover {
  color: var(--el-color-danger);
}

.inline-pm-switch-link {
  align-self: flex-start;
}

/* 搜索态 */
.inline-pm-search {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  background: var(--el-fill-color-lighter);
}

.inline-pm-hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  padding: 4px 0;
}

.inline-pm-candidates {
  max-height: 180px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.inline-pm-candidate-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 6px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
  transition: background-color 0.15s;
}

.inline-pm-candidate-row:hover {
  background: var(--el-fill-color);
}

.inline-pm-candidate-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.inline-pm-search-footer {
  display: flex;
  gap: 8px;
  justify-content: space-between;
}

/* 新建工作项态 */
.inline-pm-create {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 8px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 8px;
  background: var(--el-fill-color-lighter);
}

.inline-pm-create-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.inline-pm-create-title {
  font-size: 13px;
  font-weight: 600;
}

.inline-pm-create-field {
  margin-bottom: 0;
}

.inline-pm-create-actions {
  display: flex;
  gap: 6px;
  justify-content: flex-end;
}
</style>

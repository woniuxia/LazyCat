<template>
  <div class="inline-todo-list">
    <!-- 紧凑进度摘要 -->
    <div v-if="summary && summary.totalCount > 0" class="inline-todo-progress">
      <span class="inline-todo-progress-label">
        {{ summary.completedCount }} / {{ summary.totalCount }}
      </span>
      <div class="inline-todo-progress-track">
        <div
          class="inline-todo-progress-fill"
          :class="{ 'is-done': allCompleted }"
          :style="{ width: progressPercent + '%' }"
        />
      </div>
      <span v-if="allCompleted" class="inline-todo-progress-badge">全部完成</span>
    </div>

    <!-- 已关联任务列表 -->
    <div v-if="loading" class="inline-todo-hint">加载中...</div>
    <template v-else>
      <div v-if="displayItems.length === 0" class="inline-todo-hint inline-todo-hint--empty">
        {{ isCreateMode ? "暂无待关联的执行任务" : "暂无关联的执行任务" }}
      </div>
      <div v-else class="inline-todo-items">
        <div
          v-for="todo in displayItems"
          :key="todo.id"
          class="inline-todo-row"
          :class="{ 'is-completed': todo.status === 'completed' }"
        >
          <el-checkbox
            v-if="mode === 'edit'"
            :model-value="todo.status === 'completed'"
            class="inline-todo-row-check"
            @change="emit('toggle', todo.id)"
          />
          <span
            v-else
            class="inline-todo-check-readonly"
            :class="{ 'is-checked': todo.status === 'completed' }"
          />
          <span v-if="todo.isOverdue" class="inline-todo-overdue-dot" />
          <span class="inline-todo-title">{{ todo.title }}</span>
          <el-button
            v-if="mode === 'edit'"
            size="small"
            link
            type="danger"
            class="inline-todo-unlink-btn"
            @click="emit('unlink', todo.id)"
          >
            解绑
          </el-button>
          <span class="inline-todo-priority-pill" :class="priorityPillClass(todo.priority)">
            {{ todo.priority }}
          </span>
        </div>
      </div>
    </template>

    <!-- 编辑模式下的交互区 -->
    <template v-if="mode === 'edit'">
      <!-- 内联创建输入框 -->
      <div class="inline-todo-create-row">
        <span class="inline-todo-create-plus">+</span>
        <el-input
          v-model="createTitle"
          size="small"
          class="inline-todo-create-input"
          :placeholder="isCreateMode ? '输入任务标题，回车添加...' : '输入任务标题，回车创建...'"
          @keyup.enter="handleQuickCreate"
        />
        <el-select v-model="createPriority" size="small" class="inline-todo-create-priority">
          <el-option label="P0" value="P0" />
          <el-option label="P1" value="P1" />
          <el-option label="P2" value="P2" />
          <el-option label="P3" value="P3" />
        </el-select>
      </div>

      <!-- 绑定已有任务入口 -->
      <div class="inline-todo-link-entry">
        <template v-if="!linkSearchOpen">
          <button type="button" class="inline-todo-link-entry-btn" @click="openLinkSearch">
            绑定已有任务
          </button>
        </template>
        <template v-else>
          <div class="inline-todo-link-search">
            <el-input
              v-model="linkKeyword"
              size="small"
              placeholder="搜索任务标题"
              clearable
              @input="emit('search-candidates', linkKeyword)"
            />
            <div v-if="candidatesLoading" class="inline-todo-hint">搜索中...</div>
            <template v-else-if="candidates.length === 0">
              <div class="inline-todo-hint inline-todo-hint--empty">
                <template v-if="linkKeyword">未找到匹配的任务</template>
                <template v-else>暂无可绑定的任务</template>
              </div>
            </template>
            <div v-else class="inline-todo-candidates">
              <el-checkbox-group v-model="linkSelectedIds">
                <div v-for="c in candidates" :key="c.id" class="inline-todo-candidate-row">
                  <el-checkbox :value="c.id">
                    <span class="inline-todo-candidate-title">{{ c.title }}</span>
                    <span v-if="c.isUnassignedProject" class="inline-todo-candidate-tag is-warning"
                      >未归项目</span
                    >
                    <span
                      class="inline-todo-candidate-tag"
                      :class="c.status === 'completed' ? 'is-success' : 'is-info'"
                    >
                      {{ statusLabel(c.status) }}
                    </span>
                    <span class="inline-todo-candidate-priority">{{ c.priority }}</span>
                  </el-checkbox>
                </div>
              </el-checkbox-group>
            </div>
            <div class="inline-todo-link-actions">
              <el-button size="small" @click="closeLinkSearch">取消</el-button>
              <el-button
                size="small"
                type="primary"
                :disabled="linkSelectedIds.length === 0"
                @click="handleLinkSelected"
              >
                绑定 ({{ linkSelectedIds.length }})
              </el-button>
            </div>
          </div>
        </template>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue";
import type { PmTodoLinkItem, PmTodoSummary, PmTodoCandidateItem } from "../types/pm";

const props = defineProps<{
  pmItemId: () => number | undefined;
  items: PmTodoLinkItem[];
  summary: PmTodoSummary | null;
  loading: boolean;
  mode: "detail" | "edit";
  candidates: PmTodoCandidateItem[];
  candidatesLoading: boolean;
}>();

const emit = defineEmits<{
  create: [title: string, priority: string];
  toggle: [id: number];
  unlink: [id: number];
  link: [ids: number[]];
  "search-candidates": [keyword: string];
  "pending-change": [
    pendingCreates: Array<{ title: string; priority: string; description: string }>,
    pendingLinks: number[],
  ];
}>();

const createTitle = ref("");
const createPriority = ref("P2");
const linkSearchOpen = ref(false);
const linkKeyword = ref("");
const linkSelectedIds = ref<number[]>([]);

// 创建模式下的本地待关联数据
const localCreates = ref<Array<{ title: string; priority: string; description: string }>>([]);
const localLinkIds = ref<number[]>([]);

const isCreateMode = computed(() => props.pmItemId() == null);

// 合并后端数据和本地数据的显示列表
const displayItems = computed(() => {
  if (!isCreateMode.value) return props.items;
  return localCreates.value.map((c, idx) => ({
    id: -(idx + 1),
    title: c.title,
    status: "pending",
    priority: c.priority,
    kind: "local",
    eventAt: null,
    projectId: null,
    projectName: null,
    projectColor: null,
    completedAt: null,
    isOverdue: false,
  }));
});

const progressPercent = computed(() => {
  if (!props.summary || props.summary.totalCount === 0) return 0;
  return Math.round((props.summary.completedCount / props.summary.totalCount) * 100);
});

const allCompleted = computed(() => {
  return (
    props.summary != null &&
    props.summary.totalCount > 0 &&
    props.summary.completedCount === props.summary.totalCount
  );
});

function statusLabel(status: string): string {
  if (status === "completed") return "已完成";
  if (status === "in_progress") return "进行中";
  return "待办";
}

function priorityPillClass(priority: string | null | undefined): string {
  const key = (priority || "P2").toLowerCase();
  if (key === "p0" || key === "p1" || key === "p2" || key === "p3") {
    return "is-" + key;
  }
  return "is-p2";
}

function handleQuickCreate() {
  if (!createTitle.value.trim()) return;
  if (isCreateMode.value) {
    localCreates.value.push({
      title: createTitle.value.trim(),
      priority: createPriority.value,
      description: "",
    });
    emitPendingChange();
  } else {
    emit("create", createTitle.value.trim(), createPriority.value);
  }
  createTitle.value = "";
  createPriority.value = "P2";
}

function openLinkSearch() {
  linkSearchOpen.value = true;
  linkKeyword.value = "";
  linkSelectedIds.value = [];
  emit("search-candidates", "");
}

function closeLinkSearch() {
  linkSearchOpen.value = false;
  linkSelectedIds.value = [];
}

function handleLinkSelected() {
  if (linkSelectedIds.value.length === 0) return;
  if (isCreateMode.value) {
    localLinkIds.value.push(...linkSelectedIds.value);
    emitPendingChange();
  } else {
    emit("link", linkSelectedIds.value);
  }
  closeLinkSearch();
}

function emitPendingChange() {
  emit("pending-change", [...localCreates.value], [...localLinkIds.value]);
}

function resetLocal() {
  localCreates.value = [];
  localLinkIds.value = [];
  createTitle.value = "";
  createPriority.value = "P2";
  linkSearchOpen.value = false;
  linkKeyword.value = "";
  linkSelectedIds.value = [];
}

defineExpose({ resetLocal, localCreates, localLinkIds });
</script>

<style scoped>
/* 内嵌 PM 视觉 token 兜底：当组件被 Element Plus Dialog Teleport 到 body 外、脱离 .pm-panel 作用域时仍生效 */
.inline-todo-list {
  --pm-edge: #dbe5f1;
  --pm-edge-soft: #e3ebf5;
  --pm-accent: #0ea5e9;
  --pm-accent-soft: rgba(14, 165, 233, 0.12);
  --pm-text-main: #223042;
  --pm-text-muted: #6f8098;
  --pm-surface: #ffffff;

  display: flex;
  flex-direction: column;
  gap: 10px;
}

/* 紧凑进度摘要 */
.inline-todo-progress {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 4px 2px 0;
}
.inline-todo-progress-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--pm-text-muted);
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}
.inline-todo-progress-track {
  flex: 1;
  height: 4px;
  border-radius: 999px;
  background: rgba(219, 229, 241, 0.7);
  overflow: hidden;
}
.inline-todo-progress-fill {
  height: 100%;
  border-radius: 999px;
  background: linear-gradient(90deg, var(--pm-accent), #6da1ff);
  transition: width 0.25s ease;
}
.inline-todo-progress-fill.is-done {
  background: linear-gradient(90deg, #5cbf6a, #7bd08a);
}
.inline-todo-progress-badge {
  font-size: 11px;
  font-weight: 600;
  color: #479a55;
  white-space: nowrap;
}

.inline-todo-hint {
  font-size: 12px;
  color: var(--pm-text-muted);
  padding: 6px 2px;
}
.inline-todo-hint--empty {
  padding: 10px 2px;
}

/* 任务列表 */
.inline-todo-items {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.inline-todo-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border: 1px solid var(--pm-edge-soft);
  border-radius: 10px;
  background: #ffffff;
  transition:
    border-color 0.15s ease,
    box-shadow 0.15s ease,
    background 0.15s ease;
}

.inline-todo-row:hover {
  border-color: rgba(14, 165, 233, 0.32);
  box-shadow: 0 6px 14px rgba(14, 165, 233, 0.08);
}

.inline-todo-row.is-completed {
  background: rgba(244, 247, 251, 0.7);
  border-color: var(--pm-edge-soft);
  box-shadow: none;
}

.inline-todo-row.is-completed .inline-todo-title {
  color: var(--pm-text-muted);
  text-decoration: line-through;
}

.inline-todo-row-check {
  flex-shrink: 0;
}

.inline-todo-row :deep(.el-checkbox) {
  margin-right: 0;
  height: auto;
}

.inline-todo-row :deep(.el-checkbox__input) {
  display: inline-flex;
  align-items: center;
}

.inline-todo-row :deep(.el-checkbox__inner) {
  width: 16px;
  height: 16px;
  border-radius: 4px;
  border-color: var(--pm-edge);
}

.inline-todo-row :deep(.el-checkbox__input.is-checked .el-checkbox__inner) {
  background-color: var(--pm-accent);
  border-color: var(--pm-accent);
}

.inline-todo-check-readonly {
  width: 16px;
  height: 16px;
  border-radius: 4px;
  border: 1.5px solid var(--pm-edge);
  background: #fff;
  flex-shrink: 0;
}

.inline-todo-check-readonly.is-checked {
  background: var(--pm-accent);
  border-color: var(--pm-accent);
  position: relative;
}
.inline-todo-check-readonly.is-checked::after {
  content: "";
  position: absolute;
  left: 4px;
  top: 1px;
  width: 5px;
  height: 9px;
  border: solid #fff;
  border-width: 0 2px 2px 0;
  transform: rotate(45deg);
}

.inline-todo-overdue-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #f56c6c;
  box-shadow: 0 0 0 3px rgba(245, 108, 108, 0.14);
  flex-shrink: 0;
}

.inline-todo-title {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  font-weight: 500;
  color: var(--pm-text-main);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.inline-todo-unlink-btn {
  opacity: 0;
  flex-shrink: 0;
  transition: opacity 0.15s ease;
}

.inline-todo-row:hover .inline-todo-unlink-btn {
  opacity: 1;
}

.inline-todo-unlink-btn :deep(.el-button) {
  padding: 2px 6px;
}

/* 优先级 pill */
.inline-todo-priority-pill {
  display: inline-flex;
  align-items: center;
  padding: 2px 8px;
  font-size: 11px;
  font-weight: 600;
  border-radius: 999px;
  border: 1px solid transparent;
  flex-shrink: 0;
  font-variant-numeric: tabular-nums;
  line-height: 1.5;
}
.inline-todo-priority-pill.is-p0 {
  background: rgba(245, 108, 108, 0.1);
  color: #d84a4a;
  border-color: rgba(245, 108, 108, 0.22);
}
.inline-todo-priority-pill.is-p1 {
  background: rgba(230, 162, 60, 0.1);
  color: #b88230;
  border-color: rgba(230, 162, 60, 0.22);
}
.inline-todo-priority-pill.is-p2 {
  background: rgba(14, 165, 233, 0.08);
  color: var(--pm-accent);
  border-color: rgba(14, 165, 233, 0.2);
}
.inline-todo-priority-pill.is-p3 {
  background: rgba(144, 147, 153, 0.08);
  color: #6a6d73;
  border-color: rgba(144, 147, 153, 0.22);
}

/* 内联创建行：虚线容器 -> 聚焦转实 */
.inline-todo-create-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border: 1px dashed rgba(14, 165, 233, 0.35);
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.7);
  transition:
    border-color 0.15s,
    background 0.15s,
    box-shadow 0.15s;
}
.inline-todo-create-row:focus-within {
  border-style: solid;
  border-color: var(--pm-accent);
  background: #fff;
  box-shadow: 0 0 0 4px rgba(14, 165, 233, 0.1);
}
.inline-todo-create-plus {
  width: 16px;
  height: 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--pm-accent);
  font-size: 18px;
  font-weight: 300;
  flex-shrink: 0;
  user-select: none;
}

/* el-input 内嵌到创建行：移除自身边框，交给容器 */
.inline-todo-create-input {
  flex: 1;
}
.inline-todo-create-input :deep(.el-input__wrapper) {
  background: transparent;
  box-shadow: none !important;
  padding: 0;
}
.inline-todo-create-input :deep(.el-input__inner) {
  font-size: 13px;
  color: var(--pm-text-main);
}
.inline-todo-create-input :deep(.el-input__inner::placeholder) {
  color: var(--pm-text-muted);
}

/* el-select 做成 pill */
.inline-todo-create-priority {
  flex-shrink: 0;
  width: 34px;
}
.inline-todo-create-priority :deep(.el-select__wrapper) {
  min-height: 22px;
  padding: 2px 8px;
  border-radius: 999px;
  background: rgba(14, 165, 233, 0.08);
  box-shadow: 0 0 0 1px rgba(14, 165, 233, 0.18) inset !important;
}
.inline-todo-create-priority :deep(.el-select__wrapper.is-focused) {
  box-shadow: 0 0 0 1px var(--pm-accent) inset !important;
}
.inline-todo-create-priority :deep(.el-select__placeholder),
.inline-todo-create-priority :deep(.el-select__selected-item) {
  font-size: 11px;
  font-weight: 600;
  color: var(--pm-accent);
}
.inline-todo-create-priority :deep(.el-select__suffix) {
  display: none;
}

/* 绑定已有任务入口：居中 pill 按钮 */
.inline-todo-link-entry {
  display: flex;
  justify-content: center;
  margin-top: 2px;
}
.inline-todo-link-entry-btn {
  background: transparent;
  border: none;
  color: var(--pm-accent);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  padding: 6px 14px;
  border-radius: 999px;
  transition: background 0.15s;
}
.inline-todo-link-entry-btn:hover {
  background: rgba(14, 165, 233, 0.08);
}

/* 搜索态容器 */
.inline-todo-link-search {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px;
  border: 1px solid var(--pm-edge);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.96);
  box-shadow: 0 4px 10px rgba(34, 48, 66, 0.04);
  width: 100%;
}

.inline-todo-candidates {
  max-height: 200px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.inline-todo-candidate-row {
  padding: 4px 6px;
  border-radius: 6px;
  transition: background 0.12s;
}
.inline-todo-candidate-row:hover {
  background: rgba(14, 165, 233, 0.06);
}

.inline-todo-candidate-row :deep(.el-checkbox) {
  width: 100%;
  height: auto;
  padding: 2px 0;
}

.inline-todo-candidate-row :deep(.el-checkbox__label) {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  font-size: 12px;
  color: var(--pm-text-main);
}

.inline-todo-candidate-title {
  color: var(--pm-text-main);
}

.inline-todo-candidate-tag {
  display: inline-flex;
  align-items: center;
  padding: 1px 8px;
  font-size: 11px;
  border-radius: 999px;
  font-weight: 500;
  border: 1px solid transparent;
}
.inline-todo-candidate-tag.is-success {
  background: rgba(103, 194, 58, 0.1);
  border-color: rgba(103, 194, 58, 0.22);
  color: #479a55;
}
.inline-todo-candidate-tag.is-info {
  background: rgba(144, 147, 153, 0.08);
  border-color: rgba(144, 147, 153, 0.22);
  color: #6a6d73;
}
.inline-todo-candidate-tag.is-warning {
  background: rgba(230, 162, 60, 0.1);
  border-color: rgba(230, 162, 60, 0.22);
  color: #b88230;
}

.inline-todo-candidate-priority {
  font-size: 11px;
  color: var(--pm-text-muted);
  font-variant-numeric: tabular-nums;
}

.inline-todo-link-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}
</style>

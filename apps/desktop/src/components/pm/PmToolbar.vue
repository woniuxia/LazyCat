<template>
  <div class="pm-toolbar">
    <div class="pm-toolbar-shell">
      <div class="toolbar-row pm-toolbar-head">
        <div class="toolbar-left pm-toolbar-context">
          <span
            class="pm-toolbar-context-dot"
            :class="{ 'is-overview': isOverview }"
            :style="isOverview ? undefined : { backgroundColor: selectedProject.color }"
          />
          <div class="pm-toolbar-context-copy">
            <div class="pm-toolbar-context-title-row">
              <span class="project-title-display" :style="{ color: isOverview ? '' : selectedProject.color }">{{ selectedProject.name }}</span>
              <el-tag v-if="!isOverview && selectedProject.status === 'archived'" size="small" type="info" class="pm-toolbar-project-tag">已归档</el-tag>
            </div>
          </div>
        </div>
        <div class="toolbar-right pm-toolbar-head-actions">
          <div class="pm-view-switch">
            <PmViewSwitcher :model-value="viewId" @update:model-value="emit('update:viewId', $event)" />
          </div>
          <el-button
            class="pm-toolbar-primary-btn"
            type="primary"
            :disabled="!canCreateItems"
            :title="createBlockedReason || undefined"
            @click="emit('create-item')"
          >
            <el-icon><Plus /></el-icon>
            新建工作项
          </el-button>
        </div>
      </div>
      <div class="toolbar-row toolbar-filters pm-toolbar-controls">
        <div class="pm-toolbar-filter-bar">
          <div class="pm-toolbar-search-wrap">
            <el-input
              :model-value="searchInput"
              class="pm-toolbar-search-input"
              size="default"
              placeholder="标题、描述、标签关键词..."
              clearable
              @update:model-value="emit('update:searchInput', $event)"
            >
              <template #prefix>
                <el-icon class="pm-toolbar-search-icon"><Search /></el-icon>
              </template>
            </el-input>
          </div>
          <div class="pm-toolbar-filter-cluster">
            <el-select
              :model-value="filterType"
              class="pm-toolbar-select"
              size="default"
              placeholder="类型"
              clearable
              @update:model-value="emit('update:filterType', $event)"
            >
              <el-option v-for="(meta, key) in PM_ITEM_TYPE_MAP" :key="key" :label="meta.label" :value="key" />
            </el-select>
            <el-select
              :model-value="filterPriority"
              class="pm-toolbar-select"
              size="default"
              placeholder="优先级"
              clearable
              @update:model-value="emit('update:filterPriority', $event)"
            >
              <el-option v-for="(meta, key) in PM_PRIORITY_MAP" :key="key" :label="meta.label" :value="key" />
            </el-select>
            <div class="pm-status-filter-wrap pm-toolbar-select pm-toolbar-select--status">
              <span class="pm-status-filter-label">状态筛选</span>
              <el-select
                :model-value="selectedStatuses"
                class="pm-status-filter-select"
                size="default"
                multiple
                collapse-tags
                placeholder="状态筛选"
                @update:model-value="emit('update:selectedStatuses', $event)"
              >
                <el-option v-for="column in PM_STATUS_COLUMNS" :key="column.key" :label="column.label" :value="column.key" />
              </el-select>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Plus, Search } from "@element-plus/icons-vue";
import type { PmProject, PmItemType, PmPriority, PmItemStatus } from "../../types/pm";
import { PM_STATUS_COLUMNS, PM_ITEM_TYPE_MAP, PM_PRIORITY_MAP } from "../../types/pm";
import type { ViewId } from "../../composables/pmViewRegistry";
import PmViewSwitcher from "./PmViewSwitcher.vue";

defineProps<{
  selectedProject: PmProject;
  isOverview: boolean;
  viewId: ViewId;
  canCreateItems: boolean;
  createBlockedReason: string;
  searchInput: string;
  filterType: PmItemType | "";
  filterPriority: PmPriority | "";
  selectedStatuses: PmItemStatus[];
}>();

const emit = defineEmits<{
  (e: "update:viewId", value: ViewId): void;
  (e: "update:searchInput", value: string): void;
  (e: "update:filterType", value: PmItemType | ""): void;
  (e: "update:filterPriority", value: PmPriority | ""): void;
  (e: "update:selectedStatuses", value: PmItemStatus[]): void;
  (e: "create-item"): void;
}>();
</script>

<style scoped>
.pm-toolbar {
  display: flex;
  flex-direction: column;
  padding: 8px 16px;
  flex-shrink: 0;
  gap: 6px;
}
.toolbar-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.toolbar-filters {
  justify-content: flex-end;
  gap: 8px;
}
.pm-status-filter-wrap {
  position: relative;
  width: 128px;
  flex-shrink: 0;
}
.pm-status-filter-label {
  position: absolute;
  left: 12px;
  top: 50%;
  z-index: 1;
  transform: translateY(-50%);
  color: var(--el-text-color-regular);
  font-size: 13px;
  pointer-events: none;
}
.pm-status-filter-select {
  width: 100%;
}
.pm-status-filter-select :deep(.el-select__wrapper) {
  min-height: 32px;
  border-radius: 8px;
}
.pm-status-filter-select :deep(.el-select__selection),
.pm-status-filter-select :deep(.el-select__selected-item),
.pm-status-filter-select :deep(.el-select__input-wrapper),
.pm-status-filter-select :deep(.el-select__placeholder),
.pm-status-filter-select :deep(.el-tag),
.pm-status-filter-select :deep(.el-select__tags-text) {
  opacity: 0;
}
.toolbar-left {
  display: flex;
  align-items: center;
  gap: 8px;
}
.project-title-display {
  font-weight: 600;
  font-size: 16px;
}
.toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* PM visual unification */
.pm-toolbar {
  padding: 0;
  background: var(--lc-surface-1);
}

.pm-toolbar-shell {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 16px;
  border: 1px solid rgba(255, 255, 255, 0.92);
  border-radius: 22px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.95), rgba(245, 249, 255, 0.9)),
    radial-gradient(circle at top left, rgba(14, 165, 233, 0.09), transparent 36%);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.8);
}

.toolbar-row {
  gap: 14px;
  flex-wrap: wrap;
}

.pm-toolbar-head {
  align-items: center;
}

.pm-toolbar-controls {
  align-items: flex-end;
  width: 100%;
}

.pm-toolbar-filter-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  width: 100%;
  padding: 10px 12px;
  border: 1px solid rgba(219, 229, 241, 0.95);
  border-radius: 18px;
  background: rgba(255, 255, 255, 0.62);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.72);
  flex-wrap: wrap;
}

.pm-toolbar-context {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
  flex: 1 1 300px;
}

.pm-toolbar-context-dot {
  width: 14px;
  height: 14px;
  border-radius: 999px;
  flex-shrink: 0;
  box-shadow: 0 0 0 4px rgba(14, 165, 233, 0.12);
}

.pm-toolbar-context-dot.is-overview {
  background: linear-gradient(135deg, var(--lc-accent), #73a0ff 46%, #88c9ff 100%);
}

.pm-toolbar-context-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.pm-toolbar-context-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex-wrap: wrap;
}

.pm-toolbar-project-tag {
  border-radius: 999px;
}

.pm-toolbar-head-actions {
  gap: 10px;
  flex-wrap: wrap;
}

.pm-view-switch {
  display: inline-flex;
  align-items: center;
}

.pm-toolbar-primary-btn {
  min-height: 42px;
  padding: 0 16px;
  border: 0;
  border-radius: 14px;
  background: linear-gradient(180deg, var(--lc-accent), #0284c7);
  box-shadow: 0 12px 24px rgba(14, 165, 233, 0.22);
}

.pm-toolbar-primary-btn:hover {
  background: linear-gradient(180deg, #38bdf8, var(--lc-accent));
}

.pm-toolbar-primary-btn :deep(.el-icon) {
  margin-right: 2px;
}

.pm-toolbar-search-wrap {
  display: flex;
  flex-direction: column;
  min-width: 260px;
  flex: 1 1 320px;
}

.pm-toolbar-search-input {
  width: 100%;
}

.pm-toolbar-search-input :deep(.el-input__wrapper) {
  min-height: 42px;
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.98);
  box-shadow: inset 0 0 0 1px rgba(14, 165, 233, 0.12);
  transition: box-shadow 0.18s ease, border-color 0.18s ease;
}

.pm-toolbar-search-input :deep(.el-input__wrapper.is-focus) {
  box-shadow:
    inset 0 0 0 1px rgba(14, 165, 233, 0.32),
    0 10px 20px rgba(14, 165, 233, 0.10);
}

.pm-toolbar-search-input :deep(.el-input__inner) {
  font-size: 13px;
  color: var(--pm-text-main);
}

.pm-toolbar-search-icon {
  color: #7a90a8;
  font-size: 15px;
}

.pm-toolbar-filter-cluster {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
  flex: 0 1 auto;
  flex-wrap: wrap;
}

.pm-toolbar-select {
  width: 108px;
  flex-shrink: 0;
}

.pm-toolbar-select.pm-toolbar-select--status {
  width: 138px;
}

.pm-toolbar-select :deep(.el-select__wrapper) {
  min-height: 42px;
  padding: 0 12px;
  border-radius: 14px;
  border-color: rgba(14, 165, 233, 0.12);
  background: rgba(255, 255, 255, 0.96);
  box-shadow: none;
  transition: box-shadow 0.18s ease, border-color 0.18s ease;
}

.pm-toolbar-select :deep(.el-select__wrapper.is-focused) {
  box-shadow: 0 10px 20px rgba(14, 165, 233, 0.10);
}

.pm-toolbar-select :deep(.el-select__placeholder),
.pm-toolbar-select .pm-status-filter-label {
  color: #5a748f;
  font-weight: 600;
}

.pm-status-filter-wrap.pm-toolbar-select {
  position: relative;
}

.pm-status-filter-wrap.pm-toolbar-select .pm-status-filter-label {
  left: 14px;
  top: 50%;
  font-size: 13px;
}

.pm-status-filter-wrap.pm-toolbar-select :deep(.el-select__wrapper) {
  min-height: 42px;
  border-radius: 14px;
}

.pm-toolbar-secondary-btn {
  min-height: 42px;
  padding: 0 14px;
  border-radius: 14px;
  border-color: rgba(14, 165, 233, 0.12);
  background: rgba(245, 249, 255, 0.9);
  color: var(--pm-text-main);
}

.pm-toolbar-secondary-btn:hover {
  border-color: rgba(14, 165, 233, 0.22);
  background: rgba(240, 246, 255, 0.98);
}

.project-title-display {
  font-size: 18px;
  font-weight: 700;
  color: var(--pm-text-main);
}

.pm-status-filter-label {
  color: var(--pm-text-main);
}

@media (max-width: 1380px) {
  .pm-toolbar-controls {
    align-items: stretch;
  }

  .pm-toolbar-filter-bar {
    align-items: stretch;
  }
}

@media (max-width: 1120px) {
  .pm-toolbar-shell {
    padding: 14px;
    border-radius: 20px;
  }

  .pm-toolbar-head {
    align-items: flex-start;
  }

  .pm-toolbar-head-actions {
    width: 100%;
    justify-content: space-between;
  }

  .pm-toolbar-controls {
    flex-direction: column;
  }

  .pm-toolbar-filter-bar,
  .pm-toolbar-search-wrap,
  .pm-toolbar-filter-cluster {
    width: 100%;
    flex: 1 1 auto;
  }

  .pm-toolbar-filter-cluster {
    justify-content: flex-start;
  }
}

@media (max-width: 820px) {
  .pm-toolbar {
    padding: 14px 14px 12px;
  }

  .pm-toolbar-head-actions {
    flex-direction: column;
    align-items: stretch;
  }

  .pm-view-switch {
    width: 100%;
    justify-content: space-between;
  }

  .pm-toolbar-primary-btn {
    width: 100%;
  }

  .pm-toolbar-filter-bar {
    padding: 10px;
  }

  .pm-toolbar-filter-cluster {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    align-items: stretch;
  }

  .pm-toolbar-select,
  .pm-toolbar-select.pm-toolbar-select--status,
  .pm-toolbar-secondary-btn {
    width: 100%;
  }
}

@media (max-width: 560px) {
  .pm-toolbar-filter-cluster {
    grid-template-columns: 1fr;
  }

  .pm-view-switch {
    flex-direction: column;
    align-items: stretch;
  }
}
</style>

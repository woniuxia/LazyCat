<script setup lang="ts">
import { Plus } from "@element-plus/icons-vue";

import type { ActionCombinationSummary } from "../../types/action-center";
import { combinationRunStatusLabel } from "../../utils/actionCombination";

defineProps<{
  items: ActionCombinationSummary[];
  selectedId: number | null;
  runActive: boolean;
}>();

defineEmits<{
  create: [];
  select: [id: number];
}>();
</script>

<template>
  <aside class="combination-list" aria-label="组合动作列表">
    <header class="combination-list__header">
      <div>
        <h2>动作中心</h2>
        <span>{{ items.length }} 个组合</span>
      </div>
      <el-button
        :icon="Plus"
        circle
        title="新建组合"
        :disabled="runActive"
        @click="$emit('create')"
      />
    </header>

    <div v-if="items.length" class="combination-list__items">
      <button
        v-for="item in items"
        :key="item.id"
        type="button"
        class="combination-list__item"
        :class="{ 'is-selected': item.id === selectedId }"
        :disabled="runActive"
        @click="$emit('select', item.id)"
      >
        <span class="combination-list__name">{{ item.name }}</span>
        <span class="combination-list__meta">
          {{ item.stepCount }} 步 · {{ item.executionMode === "serial" ? "串行" : "并行" }}
        </span>
        <span v-if="item.latestRunStatus" class="combination-list__status">
          {{ combinationRunStatusLabel(item.latestRunStatus) }}
        </span>
      </button>
    </div>
    <el-empty v-else description="暂无组合动作" :image-size="56" />
  </aside>
</template>

<style scoped>
.combination-list {
  min-width: 0;
  border-right: 1px solid var(--lc-border-subtle);
  background: var(--lc-surface-1);
  overflow: hidden;
}

.combination-list__header {
  height: 64px;
  padding: 0 14px 0 18px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--lc-border-subtle);
}

.combination-list__header h2 {
  margin: 0;
  color: var(--lc-text);
  font-size: 16px;
  font-weight: 650;
}

.combination-list__header span,
.combination-list__meta,
.combination-list__status {
  color: var(--lc-text-muted);
  font-size: 12px;
}

.combination-list__items {
  height: calc(100% - 64px);
  padding: 8px;
  overflow-y: auto;
}

.combination-list__item {
  width: 100%;
  min-height: 66px;
  padding: 10px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 4px 8px;
  text-align: left;
  color: var(--lc-text);
  border: 1px solid transparent;
  border-radius: var(--lc-radius-sm);
  background: transparent;
  cursor: pointer;
  transition:
    background var(--lc-duration) var(--lc-ease),
    border-color var(--lc-duration) var(--lc-ease);
}

.combination-list__item:hover:not(:disabled) {
  background: var(--lc-surface-0);
  border-color: var(--lc-border);
}

.combination-list__item.is-selected {
  background: var(--lc-accent-dim);
  border-color: color-mix(in srgb, var(--lc-accent) 32%, var(--lc-border));
}

.combination-list__item:focus-visible {
  outline: 2px solid var(--lc-accent);
  outline-offset: -2px;
}

.combination-list__item:disabled {
  cursor: not-allowed;
  opacity: 0.65;
}

.combination-list__name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
  font-weight: 600;
}

.combination-list__meta {
  grid-column: 1;
}

.combination-list__status {
  grid-column: 2;
  grid-row: 2;
}
</style>

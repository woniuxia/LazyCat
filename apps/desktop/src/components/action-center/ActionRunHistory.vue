<script setup lang="ts">
import { computed } from "vue";

import type { ActionCombinationRunDetail } from "../../types/action-center";
import {
  combinationRunStatusLabel,
  combinationStepStatusLabel,
} from "../../utils/actionCombination";

const props = defineProps<{
  activeRun: ActionCombinationRunDetail | null;
  history: ActionCombinationRunDetail[];
}>();

const visibleHistory = computed(() =>
  props.history.filter((run) => run.id !== props.activeRun?.id).slice(0, 20),
);

function tagType(status: string): "success" | "warning" | "danger" | "info" {
  if (status === "succeeded" || status === "already_satisfied") return "success";
  if (status === "partially_succeeded" || status === "running" || status === "pending") {
    return "warning";
  }
  if (status === "failed") return "danger";
  return "info";
}
</script>

<template>
  <aside class="run-history" aria-label="组合动作运行记录">
    <header class="run-history__header">
      <h2>运行记录</h2>
      <span>最近 {{ history.length }} 次</span>
    </header>

    <section v-if="activeRun" class="run-history__active">
      <div class="run-history__summary">
        <strong>{{ activeRun.combinationName }}</strong>
        <el-tag size="small" :type="tagType(activeRun.status)">
          {{ combinationRunStatusLabel(activeRun.status) }}
        </el-tag>
      </div>
      <p v-if="activeRun.error" class="run-history__error">
        {{ activeRun.error }}
      </p>
      <ol class="run-step-results">
        <li v-for="step in activeRun.steps" :key="step.id">
          <span class="run-step-results__label"
            >{{ step.actionLabel }} · {{ step.targetLabel }}</span
          >
          <el-tag size="small" effect="plain" :type="tagType(step.status)">
            {{ combinationStepStatusLabel(step.status) }}
          </el-tag>
          <span v-if="step.message" class="run-step-results__message">{{ step.message }}</span>
        </li>
      </ol>
    </section>

    <el-collapse v-if="visibleHistory.length" class="run-history__archive">
      <el-collapse-item v-for="run in visibleHistory" :key="run.id" :name="run.id">
        <template #title>
          <span class="run-history__archive-title">{{ run.createdAt }}</span>
          <el-tag size="small" effect="plain" :type="tagType(run.status)">
            {{ combinationRunStatusLabel(run.status) }}
          </el-tag>
        </template>
        <p v-if="run.error" class="run-history__error">
          {{ run.error }}
        </p>
        <ol class="run-step-results">
          <li v-for="step in run.steps" :key="step.id">
            <span class="run-step-results__label"
              >{{ step.actionLabel }} · {{ step.targetLabel }}</span
            >
            <span class="run-step-results__message">
              {{ combinationStepStatusLabel(step.status)
              }}{{ step.message ? ` · ${step.message}` : "" }}
            </span>
          </li>
        </ol>
      </el-collapse-item>
    </el-collapse>
    <el-empty v-else-if="!activeRun" description="暂无运行记录" :image-size="52" />
  </aside>
</template>

<style scoped>
.run-history {
  min-width: 0;
  padding: 16px;
  border-left: 1px solid var(--lc-border-subtle);
  background: var(--lc-surface-1);
  overflow-y: auto;
}

.run-history__header,
.run-history__summary,
.run-history__archive-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.run-history__header {
  height: 32px;
  margin-bottom: 10px;
}

.run-history__header h2 {
  margin: 0;
  color: var(--lc-text);
  font-size: 14px;
}

.run-history__header span,
.run-history__archive-title {
  color: var(--lc-text-muted);
  font-size: 12px;
}

.run-history__error {
  margin: 8px 0 0;
  color: var(--lc-danger);
  font-size: 12px;
  overflow-wrap: anywhere;
}

.run-history__active {
  padding: 10px 0 14px;
  border-top: 1px solid var(--lc-border-subtle);
  border-bottom: 1px solid var(--lc-border-subtle);
}

.run-history__summary strong {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--lc-text);
  font-size: 13px;
}

.run-step-results {
  margin: 10px 0 0;
  padding: 0;
  list-style: none;
}

.run-step-results li {
  padding: 7px 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 3px 8px;
  border-bottom: 1px solid var(--lc-border-subtle);
}

.run-step-results__label {
  min-width: 0;
  overflow-wrap: anywhere;
  color: var(--lc-text-secondary);
  font-size: 12px;
}

.run-step-results__message {
  grid-column: 1 / -1;
  color: var(--lc-text-muted);
  font-size: 12px;
  overflow-wrap: anywhere;
}

.run-history__archive {
  margin-top: 10px;
  border: 0;
}

.run-history__archive-title {
  justify-content: flex-start;
  flex: 1;
}

@media (max-width: 1100px) {
  .run-history {
    border-top: 1px solid var(--lc-border-subtle);
    border-left: 0;
    max-height: 320px;
  }
}
</style>

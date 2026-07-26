<script setup lang="ts">
import { computed, onMounted, onUnmounted, watch } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";

import { useActionCombinations } from "../composables/useActionCombinations";
import { isCombinationRunTerminal } from "../utils/actionCombination";
import ActionCombinationEditor from "./action-center/ActionCombinationEditor.vue";
import ActionCombinationList from "./action-center/ActionCombinationList.vue";
import ActionRunHistory from "./action-center/ActionRunHistory.vue";

const emit = defineEmits<{
  "open-tool": [toolId: string];
}>();

const {
  definitions,
  combinations,
  selectedId,
  draft,
  dirty,
  stepTargets,
  activeRun,
  runHistory,
  runActive,
  start,
  stop,
  selectCombination: selectStoredCombination,
  createCombination: createDraft,
  copyCombination,
  loadStepTargets,
  reorderSteps,
  saveCombination: persistCombination,
  deleteCombination,
  runCombination: startCombination,
} = useActionCombinations();

let notifiedRunId = "";

const displayedRun = computed(() =>
  draft.value?.id !== undefined
  && draft.value?.id === activeRun.value?.combinationId
    ? activeRun.value
    : null,
);

async function loadDraftTargets(): Promise<void> {
  if (!draft.value) return;
  await Promise.all(
    draft.value.steps
      .filter((step) => step.actionType)
      .map((step) => loadStepTargets(step.localId, step.actionType)),
  );
}

async function selectCombination(id: number): Promise<void> {
  if (runActive.value || id === selectedId.value) return;
  if (dirty.value) {
    try {
      await ElMessageBox.confirm("当前修改尚未保存，切换后将丢失。", "切换组合", {
        type: "warning",
        confirmButtonText: "继续切换",
        cancelButtonText: "取消",
      });
    } catch {
      return;
    }
  }
  await selectStoredCombination(id);
  await loadDraftTargets();
}

function createCombination(): void {
  if (runActive.value) return;
  createDraft();
}

async function saveCombination(): Promise<void> {
  try {
    await persistCombination();
    await loadDraftTargets();
    ElMessage.success("组合动作已保存");
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  }
}

async function confirmDeleteCombination(): Promise<void> {
  if (!draft.value?.id) return;
  try {
    await ElMessageBox.confirm(`确定删除“${draft.value.name}”吗？`, "删除组合", {
      type: "warning",
      confirmButtonText: "删除",
      cancelButtonText: "取消",
    });
    await deleteCombination(draft.value.id);
    ElMessage.success("组合动作已删除");
  } catch (error) {
    if (error === "cancel" || error === "close") return;
    ElMessage.error(error instanceof Error ? error.message : String(error));
  }
}

async function runCombination(): Promise<void> {
  if (!draft.value?.id || dirty.value || runActive.value) return;
  try {
    notifiedRunId = "";
    await startCombination(draft.value.id);
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  }
}

function openTool(toolId: string): void {
  if (toolId) emit("open-tool", toolId);
}

watch(
  () => activeRun.value?.status,
  (status) => {
    const run = activeRun.value;
    if (!run || !status || !isCombinationRunTerminal(status) || notifiedRunId === run.id) return;
    notifiedRunId = run.id;
    const failures = run.steps
      .filter((step) => step.status === "failed")
      .map((step) => `${step.actionLabel} · ${step.targetLabel}`);
    const message = failures.length ? `失败步骤：${failures.join("、")}` : "组合动作运行完成";
    if (status === "succeeded") ElMessage.success(message);
    else if (status === "partially_succeeded") ElMessage.warning(message);
    else ElMessage.error(message);
  },
);

onMounted(async () => {
  try {
    await start();
    if (combinations.value.length) {
      await selectStoredCombination(combinations.value[0].id);
      await loadDraftTargets();
    } else {
      createDraft();
    }
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  }
});

onUnmounted(stop);
</script>

<template>
  <section class="action-center-panel">
    <ActionCombinationList
      :items="combinations"
      :selected-id="selectedId"
      :run-active="runActive"
      @create="createCombination"
      @select="selectCombination"
    />
    <main class="action-center-workspace">
      <ActionCombinationEditor
        v-if="draft"
        v-model="draft"
        :definitions="definitions"
        :targets="stepTargets"
        :dirty="dirty"
        :run-active="runActive"
        @load-targets="loadStepTargets"
        @save="saveCombination"
        @copy="copyCombination"
        @delete="confirmDeleteCombination"
        @run="runCombination"
        @reorder="reorderSteps"
        @open-tool="openTool"
      />
      <div v-else class="action-center-empty">
        <el-empty description="请选择或新建组合动作" />
      </div>
      <ActionRunHistory
        :active-run="displayedRun"
        :history="runHistory"
      />
    </main>
  </section>
</template>

<style scoped>
.action-center-panel {
  width: 100%;
  height: 100%;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(210px, 248px) minmax(0, 1fr);
  color: var(--lc-text);
  background: var(--lc-surface-0);
  overflow: hidden;
}

.action-center-workspace {
  min-width: 0;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(260px, 320px);
  overflow: hidden;
}

.action-center-empty {
  display: grid;
  place-items: center;
}

@media (max-width: 1100px) {
  .action-center-workspace {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(420px, 1fr) auto;
    overflow-y: auto;
  }
}

@media (max-width: 720px) {
  .action-center-panel {
    grid-template-columns: 1fr;
    grid-template-rows: minmax(150px, 34vh) minmax(0, 1fr);
  }

  .action-center-panel :deep(.combination-list) {
    border-right: 0;
    border-bottom: 1px solid var(--lc-border-subtle);
  }
}
</style>

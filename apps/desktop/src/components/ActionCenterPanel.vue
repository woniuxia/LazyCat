<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";

import { useActionCombinations } from "../composables/useActionCombinations";
import {
  useActionCenterNavigation,
  type ActionCenterNavigationTarget,
} from "../composables/useActionCenterNavigation";
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
  operationPending,
  start,
  stop,
  selectCombination: selectStoredCombination,
  createCombination: createDraft,
  copyCombination,
  loadStepTargets,
  getRun,
  trackRun,
  reorderSteps,
  saveCombination: persistCombination,
  deleteCombination,
  runCombination: startCombination,
} = useActionCombinations();
const actionCenterNavigation = useActionCenterNavigation();

let panelMounted = true;
let notifiedRunId = "";
const initializing = ref(true);
const selecting = ref(false);

const displayedRun = computed(() =>
  activeRun.value
  && (
    activeRun.value.combinationId == null
    || (
      draft.value?.id !== undefined
      && draft.value.id === activeRun.value.combinationId
    )
  )
    ? activeRun.value
    : null,
);
const interactionLocked = computed(
  () =>
    runActive.value || operationPending.value || initializing.value || selecting.value,
);

async function confirmDiscardChanges(title: string): Promise<boolean> {
  if (!dirty.value) return true;
  try {
    await ElMessageBox.confirm("当前修改尚未保存，继续后将丢失。", title, {
      type: "warning",
      confirmButtonText: "继续",
      cancelButtonText: "取消",
    });
    return true;
  } catch {
    return false;
  }
}

async function loadDraftTargets(): Promise<void> {
  if (!draft.value) return;
  await Promise.all(
    draft.value.steps
      .filter((step) => step.actionType)
      .map((step) => loadStepTargets(step.localId, step.actionType)),
  );
}

async function loadStoredCombination(id: number): Promise<void> {
  await selectStoredCombination(id);
  if (!panelMounted) return;
  await loadDraftTargets();
}

async function focusNavigationTarget(target: ActionCenterNavigationTarget): Promise<void> {
  if (!panelMounted || selecting.value) return;
  selecting.value = true;
  try {
    if (!(await confirmDiscardChanges("打开动作记录"))) return;
    if (target.kind === "combination") {
      await loadStoredCombination(target.combinationId);
    } else {
      const run = await getRun(target.runId);
      if (typeof run.combinationId === "number") {
        await loadStoredCombination(run.combinationId);
      }
      if (!panelMounted) return;
      await trackRun(run);
    }
    actionCenterNavigation.consume(target);
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    selecting.value = false;
  }
}

async function selectCombination(id: number): Promise<void> {
  if (interactionLocked.value || id === selectedId.value) return;
  selecting.value = true;
  try {
    if (!(await confirmDiscardChanges("切换组合"))) return;
    await loadStoredCombination(id);
  } finally {
    selecting.value = false;
  }
}

async function createCombination(): Promise<void> {
  if (interactionLocked.value) return;
  if (!(await confirmDiscardChanges("新建组合"))) return;
  createDraft();
}

async function saveCombination(): Promise<void> {
  if (interactionLocked.value) return;
  try {
    const result = await persistCombination();
    const refreshErrors = result.refreshError ? [result.refreshError] : [];
    try {
      await loadDraftTargets();
    } catch (error) {
      refreshErrors.push(error instanceof Error ? error.message : String(error));
    }
    if (refreshErrors.length) {
      ElMessage.warning("组合动作已保存，但刷新失败：" + refreshErrors.join("；"));
    } else {
      ElMessage.success("组合动作已保存");
    }
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  }
}

async function copyCombinationDraft(): Promise<void> {
  if (interactionLocked.value) return;
  copyCombination();
  await loadDraftTargets();
}

async function confirmDeleteCombination(): Promise<void> {
  if (interactionLocked.value || !draft.value?.id) return;
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
  if (!draft.value?.id || dirty.value || interactionLocked.value) return;
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
  () => [activeRun.value?.id, activeRun.value?.status] as const,
  ([, status]) => {
    const run = activeRun.value;
    if (!run || !status || !isCombinationRunTerminal(status) || notifiedRunId === run.id) return;
    notifiedRunId = run.id;
    const failures = run.steps
      .filter((step) => step.status === "failed")
      .map((step) => `${step.actionLabel} · ${step.targetLabel}`);
    const details = [
      run.error?.trim(),
      failures.length ? `失败步骤：${failures.join("、")}` : "",
    ].filter(Boolean);
    const fallback = status === "succeeded" ? "组合动作运行完成"
      : status === "partially_succeeded" ? "组合动作部分完成"
        : "组合动作运行失败";
    const message = details.join("；") || fallback;
    if (status === "succeeded") ElMessage.success(message);
    else if (status === "partially_succeeded") ElMessage.warning(message);
    else ElMessage.error(message);
  },
);

watch(
  actionCenterNavigation.pendingTarget,
  (target) => {
    if (target && !initializing.value) void focusNavigationTarget(target);
  },
);

onMounted(async () => {
  try {
    await start();
    if (!panelMounted) return;
    const navigationTarget = actionCenterNavigation.pendingTarget.value;
    if (navigationTarget) {
      await focusNavigationTarget(navigationTarget);
    } else if (combinations.value.length) {
      const initialId = activeRun.value?.combinationId ?? combinations.value[0].id;
      await loadStoredCombination(initialId);
    } else {
      createDraft();
    }
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    initializing.value = false;
  }
});

onUnmounted(() => {
  panelMounted = false;
  stop();
});
</script>

<template>
  <section class="action-center-panel">
    <ActionCombinationList
      :items="combinations"
      :selected-id="selectedId"
      :run-active="interactionLocked"
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
        :run-active="interactionLocked"
        @load-targets="loadStepTargets"
        @save="saveCombination"
        @copy="copyCombinationDraft"
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

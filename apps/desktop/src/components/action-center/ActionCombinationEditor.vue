<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { CopyDocument, Delete, Link, Plus, Rank, VideoPlay } from "@element-plus/icons-vue";
import Sortable from "sortablejs";

import type {
  ActionCombinationDraft,
  ActionCombinationTarget,
  CombinationAtomicDefinition,
} from "../../types/action-center";
import { createEmptyCombinationStep } from "../../utils/actionCombination";

const props = defineProps<{
  modelValue: ActionCombinationDraft;
  definitions: CombinationAtomicDefinition[];
  targets: Map<string, ActionCombinationTarget[]>;
  dirty: boolean;
  runActive: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: ActionCombinationDraft];
  "load-targets": [localStepId: string, actionType: string];
  save: [];
  copy: [];
  delete: [];
  run: [];
  reorder: [fromIndex: number, toIndex: number];
  "open-tool": [toolId: string];
}>();

const stepListRef = ref<HTMLElement | null>(null);
let sortable: Sortable | null = null;

const canRun = computed(() =>
  Boolean(props.modelValue.id)
  && Boolean(props.modelValue.name.trim())
  && props.modelValue.steps.length > 0
  && props.modelValue.steps.every((step) => {
    if (!step.actionType || !step.targetId) return false;
    return props.targets
      .get(step.localId)
      ?.some((target) => target.id === step.targetId && target.available) === true;
  }),
);

function updateDraft(patch: Partial<ActionCombinationDraft>): void {
  emit("update:modelValue", { ...props.modelValue, ...patch });
}

function updateStep(index: number, patch: Partial<ActionCombinationDraft["steps"][number]>): void {
  const steps = props.modelValue.steps.map((step, stepIndex) =>
    stepIndex === index ? { ...step, ...patch } : step,
  );
  updateDraft({ steps });
}

function addStep(): void {
  updateDraft({ steps: [...props.modelValue.steps, createEmptyCombinationStep()] });
}

function removeStep(index: number): void {
  updateDraft({ steps: props.modelValue.steps.filter((_, stepIndex) => stepIndex !== index) });
}

function changeAction(index: number, value: unknown): void {
  const actionType = typeof value === "string" ? value : "";
  const step = props.modelValue.steps[index];
  updateStep(index, { actionType, targetId: "" });
  if (actionType) emit("load-targets", step.localId, actionType);
}

function changeExecutionMode(value: unknown): void {
  if (value !== "serial" && value !== "parallel") return;
  updateDraft({ executionMode: value });
}

function changeTarget(index: number, value: unknown): void {
  updateStep(index, { targetId: typeof value === "string" ? value : "" });
}

function definitionFor(actionType: string): CombinationAtomicDefinition | undefined {
  return props.definitions.find((definition) => definition.actionType === actionType);
}

function initSortable(): void {
  sortable?.destroy();
  sortable = null;
  if (!stepListRef.value) return;
  sortable = Sortable.create(stepListRef.value, {
    animation: 150,
    handle: ".action-step-drag",
    ghostClass: "action-step-ghost",
    disabled: props.runActive,
    onEnd: ({ oldIndex, newIndex }) => {
      if (oldIndex === undefined || newIndex === undefined || oldIndex === newIndex) return;
      emit("reorder", oldIndex, newIndex);
    },
  });
}

watch(
  () => props.modelValue.steps.length,
  () => void nextTick(initSortable),
);

watch(
  () => props.runActive,
  (disabled) => sortable?.option("disabled", disabled),
);

onMounted(() => void nextTick(initSortable));
onUnmounted(() => {
  sortable?.destroy();
  sortable = null;
});
</script>

<template>
  <section class="combination-editor" :aria-busy="runActive">
    <header class="combination-editor__header">
      <div>
        <h2>{{ modelValue.id ? "编辑组合" : "新建组合" }}</h2>
        <span v-if="dirty" class="combination-editor__dirty">未保存</span>
      </div>
      <div class="combination-editor__actions">
        <el-button :icon="CopyDocument" :disabled="runActive || !modelValue.name" @click="$emit('copy')">
          复制
        </el-button>
        <el-button
          v-if="modelValue.id"
          :icon="Delete"
          :disabled="runActive"
          @click="$emit('delete')"
        >
          删除
        </el-button>
        <el-button
          type="primary"
          plain
          :disabled="!dirty || runActive"
          @click="$emit('save')"
        >
          保存
        </el-button>
        <el-button
          type="success"
          :icon="VideoPlay"
          :disabled="dirty || !canRun || runActive"
          @click="$emit('run')"
        >
          运行
        </el-button>
      </div>
    </header>

    <div class="combination-editor__form">
      <label class="field-label" for="combination-name">组合名称</label>
      <el-input
        id="combination-name"
        :model-value="modelValue.name"
        maxlength="80"
        show-word-limit
        :disabled="runActive"
        @update:model-value="updateDraft({ name: $event })"
      />

      <span class="field-label">执行方式</span>
      <el-segmented
        :model-value="modelValue.executionMode"
        :options="[
          { label: '串行', value: 'serial' },
          { label: '并行', value: 'parallel' },
        ]"
        :disabled="runActive"
        @update:model-value="changeExecutionMode"
      />
    </div>

    <div class="combination-editor__steps-header">
      <h3>动作步骤</h3>
      <el-button :icon="Plus" :disabled="runActive" @click="addStep">新增步骤</el-button>
    </div>

    <div ref="stepListRef" class="combination-editor__steps">
      <div
        v-for="(step, index) in modelValue.steps"
        :key="step.localId"
        class="action-step"
      >
        <button
          type="button"
          class="action-step-drag"
          title="拖动排序"
          :disabled="runActive"
        >
          <el-icon><Rank /></el-icon>
        </button>
        <span class="action-step__index">{{ index + 1 }}</span>
        <el-select
          :model-value="step.actionType"
          placeholder="选择动作"
          :disabled="runActive"
          @update:model-value="changeAction(index, $event)"
        >
          <el-option
            v-for="definition in definitions"
            :key="definition.actionType"
            :label="definition.label"
            :value="definition.actionType"
          />
        </el-select>
        <div class="action-step__target">
          <el-select
            :model-value="step.targetId"
            placeholder="选择目标"
            :disabled="runActive || !step.actionType"
            @visible-change="$event && step.actionType && $emit('load-targets', step.localId, step.actionType)"
            @update:model-value="changeTarget(index, $event)"
          >
            <el-option
              v-for="target in targets.get(step.localId) ?? []"
              :key="target.id"
              :label="target.label"
              :value="target.id"
              :disabled="!target.available"
            />
          </el-select>
          <div
            v-if="step.targetId && targets.get(step.localId)?.find((item) => item.id === step.targetId)?.available === false"
            class="action-step__unavailable"
          >
            <span>
              {{ targets.get(step.localId)?.find((item) => item.id === step.targetId)?.unavailableReason }}
            </span>
            <el-button
              link
              type="primary"
              :icon="Link"
              @click="$emit('open-tool', definitionFor(step.actionType)?.targetToolId ?? '')"
            >
              打开工具
            </el-button>
          </div>
        </div>
        <el-button
          :icon="Delete"
          circle
          title="删除步骤"
          :disabled="runActive"
          @click="removeStep(index)"
        />
      </div>
      <el-empty v-if="!modelValue.steps.length" description="暂无动作步骤" :image-size="52" />
    </div>
  </section>
</template>

<style scoped>
.combination-editor {
  min-width: 0;
  padding: 18px 20px 24px;
  overflow-y: auto;
  background: var(--lc-surface-0);
}

.combination-editor__header,
.combination-editor__steps-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.combination-editor__header {
  min-height: 42px;
  margin-bottom: 18px;
}

.combination-editor__header h2,
.combination-editor__steps-header h3 {
  margin: 0;
  color: var(--lc-text);
}

.combination-editor__header h2 { font-size: 17px; }
.combination-editor__steps-header h3 { font-size: 14px; }

.combination-editor__dirty {
  margin-left: 8px;
  color: var(--lc-warning);
  font-size: 12px;
}

.combination-editor__actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
}

.combination-editor__actions :deep(.el-button + .el-button) { margin-left: 0; }

.combination-editor__form {
  display: grid;
  grid-template-columns: 90px minmax(180px, 1fr);
  gap: 12px 14px;
  align-items: center;
  max-width: 720px;
  margin-bottom: 24px;
}

.field-label {
  color: var(--lc-text-secondary);
  font-size: 13px;
}

.combination-editor__steps-header {
  padding-bottom: 8px;
  border-bottom: 1px solid var(--lc-border-subtle);
}

.combination-editor__steps {
  padding-top: 8px;
}

.action-step {
  min-height: 58px;
  padding: 8px 4px;
  display: grid;
  grid-template-columns: 30px 24px minmax(150px, 0.8fr) minmax(190px, 1.2fr) 34px;
  gap: 8px;
  align-items: start;
  border-bottom: 1px solid var(--lc-border-subtle);
}

.action-step-drag {
  width: 30px;
  height: 32px;
  display: grid;
  place-items: center;
  color: var(--lc-text-muted);
  border: 0;
  border-radius: var(--lc-radius-sm);
  background: transparent;
  cursor: grab;
}

.action-step-drag:hover:not(:disabled) {
  color: var(--lc-text);
  background: var(--lc-surface-2);
}

.action-step-drag:focus-visible {
  outline: 2px solid var(--lc-accent);
}

.action-step-drag:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.action-step__index {
  padding-top: 7px;
  color: var(--lc-text-muted);
  font: 12px var(--lc-font-mono);
}

.action-step__target {
  min-width: 0;
}

.action-step__target > :deep(.el-select) { width: 100%; }

.action-step__unavailable {
  margin-top: 4px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  color: var(--lc-danger);
  font-size: 12px;
}

.action-step__unavailable span {
  min-width: 0;
  overflow-wrap: anywhere;
}

:deep(.action-step-ghost) {
  opacity: 0.45;
  background: var(--lc-accent-dim);
}

@media (max-width: 900px) {
  .combination-editor__header {
    align-items: flex-start;
    flex-direction: column;
  }

  .combination-editor__actions {
    justify-content: flex-start;
  }

  .action-step {
    grid-template-columns: 30px 24px minmax(0, 1fr) 34px;
  }

  .action-step__target {
    grid-column: 3 / 4;
  }
}

@media (max-width: 560px) {
  .combination-editor { padding: 14px; }
  .combination-editor__form { grid-template-columns: 1fr; gap: 6px; }
  .combination-editor__actions { width: 100%; }
}
</style>

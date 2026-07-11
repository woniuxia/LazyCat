<template>
  <div class="todo-quick-add">
    <el-input
      ref="titleInputRef"
      v-model="title"
      class="quick-add-input"
      :class="{ 'is-create-success': createFlash }"
      placeholder="添加任务，回车创建…"
      @keydown.enter.exact.prevent="onTitleEnter"
      @keydown.esc="resetAll"
    >
      <template #prefix>
        <el-icon class="quick-add-plus"><Plus /></el-icon>
      </template>
      <template #suffix>
        <el-icon v-if="loading" class="is-loading"><Loading /></el-icon>
      </template>
    </el-input>

    <el-dropdown trigger="click" @command="onDateCommand">
      <button type="button" class="quick-add-chip" :class="{ 'is-set': dateChoice !== null }">
        <el-icon><Calendar /></el-icon>
        <span>{{ dateLabel }}</span>
        <el-icon
          v-if="dateChoice !== null"
          class="chip-clear"
          @click.stop="clearDate"
        ><Close /></el-icon>
      </button>
      <template #dropdown>
        <el-dropdown-menu>
          <el-dropdown-item command="today">今天</el-dropdown-item>
          <el-dropdown-item command="tomorrow">明天</el-dropdown-item>
          <el-dropdown-item command="pick">选日期…</el-dropdown-item>
          <el-dropdown-item command="clear" divided>清除日期</el-dropdown-item>
        </el-dropdown-menu>
      </template>
    </el-dropdown>
    <div class="quick-add-date-anchor">
      <el-date-picker
        ref="datePickerRef"
        v-model="pickedDate"
        type="date"
        value-format="YYYY-MM-DD"
        @change="onPickedDate"
      />
    </div>

    <el-dropdown trigger="click" @command="onPriorityCommand">
      <button
        type="button"
        class="quick-add-chip"
        :class="['is-priority-' + effectivePriority.toLowerCase(), { 'is-set': priorityOverride !== null }]"
      >
        <span>{{ effectivePriority }}</span>
        <el-icon
          v-if="priorityOverride !== null"
          class="chip-clear"
          @click.stop="priorityOverride = null"
        ><Close /></el-icon>
      </button>
      <template #dropdown>
        <el-dropdown-menu>
          <el-dropdown-item v-for="p in PRIORITY_OPTIONS" :key="p" :command="p">{{ p }}</el-dropdown-item>
          <el-dropdown-item command="clear" divided>清除</el-dropdown-item>
        </el-dropdown-menu>
      </template>
    </el-dropdown>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref } from "vue";
import { Calendar, Close, Loading, Plus } from "@element-plus/icons-vue";
import type { TodoPriority } from "../../types";
import { useToolInvoke } from "../../composables/useToolInvoke";
import {
  buildQuickAddPayload,
  type QuickAddContext,
  type QuickAddDateChoice,
} from "../../utils/todoQuickAdd";

const props = defineProps<{ context: QuickAddContext }>();
const emit = defineEmits<{ (e: "created", id: number): void }>();

const PRIORITY_OPTIONS: TodoPriority[] = ["P0", "P1", "P2", "P3"];

const { loading, invokeWithLoading } = useToolInvoke();

const title = ref("");
const dateChoice = ref<QuickAddDateChoice>(null);
const priorityOverride = ref<TodoPriority | null>(null);
const pickedDate = ref<string | null>(null);
const createFlash = ref(false);
const titleInputRef = ref<{ focus: () => void } | null>(null);
const datePickerRef = ref<{ handleOpen?: () => void } | null>(null);
let flashTimer: ReturnType<typeof setTimeout> | null = null;

// 手动优先级是独立模型：仅随用户选择/清除/Esc 变化，不跟随 priorityDefault
const effectivePriority = computed(() => priorityOverride.value ?? props.context.priorityDefault);

const dateLabel = computed(() => {
  const choice = dateChoice.value;
  if (!choice) return "日期";
  if (choice.kind === "today") return "今天";
  if (choice.kind === "tomorrow") return "明天";
  return choice.date.slice(5);
});

function onDateCommand(command: string) {
  if (command === "today") {
    dateChoice.value = { kind: "today" };
  } else if (command === "tomorrow") {
    dateChoice.value = { kind: "tomorrow" };
  } else if (command === "pick") {
    // 等 dropdown 关闭流程结束再开日历弹层
    void nextTick(() => datePickerRef.value?.handleOpen?.());
  } else if (command === "clear") {
    clearDate();
  }
}

function onPickedDate(value: string | null) {
  if (value) dateChoice.value = { kind: "date", date: value };
}

function clearDate() {
  dateChoice.value = null;
  pickedDate.value = null;
}

function onPriorityCommand(command: string) {
  if (command === "clear") {
    priorityOverride.value = null;
  } else {
    priorityOverride.value = command as TodoPriority;
  }
}

function onTitleEnter(event: KeyboardEvent) {
  if (event.isComposing) return;
  void submit();
}

async function submit() {
  if (loading.value) return;
  const payload = buildQuickAddPayload(
    { title: title.value, dateChoice: dateChoice.value, priorityOverride: priorityOverride.value },
    props.context,
  );
  if (!payload) return;
  const response = await invokeWithLoading<{ ok: boolean; id: number; rootId: number }>(
    "tool:todo:item-create",
    payload,
  );
  // 失败提示由 invokeWithLoading 承担；保留输入便于重试
  if (!response) return;
  title.value = "";
  flashSuccess();
  emit("created", response.id);
  titleInputRef.value?.focus();
}

function resetAll() {
  title.value = "";
  dateChoice.value = null;
  priorityOverride.value = null;
  pickedDate.value = null;
}

function flashSuccess() {
  createFlash.value = true;
  if (flashTimer) clearTimeout(flashTimer);
  flashTimer = setTimeout(() => {
    createFlash.value = false;
    flashTimer = null;
  }, 600);
}

onBeforeUnmount(() => {
  if (flashTimer) clearTimeout(flashTimer);
});
</script>

<style scoped>
.todo-quick-add {
  display: flex;
  align-items: center;
  gap: 8px;
}

.quick-add-input {
  flex: 1;
}

.quick-add-plus {
  color: var(--lc-text-muted);
}

.quick-add-input :deep(.el-input__wrapper) {
  transition: box-shadow 0.25s var(--lc-ease);
}

.quick-add-input.is-create-success :deep(.el-input__wrapper) {
  box-shadow: 0 0 0 1px var(--lc-success) inset;
}

.quick-add-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 32px;
  padding: 0 10px;
  border: 1px solid var(--lc-border);
  border-radius: var(--lc-radius-sm);
  background: var(--lc-surface-1);
  color: var(--lc-text-muted);
  font-size: 13px;
  cursor: pointer;
  white-space: nowrap;
  transition:
    color var(--lc-duration) var(--lc-ease),
    border-color var(--lc-duration) var(--lc-ease),
    background var(--lc-duration) var(--lc-ease);
}

.quick-add-chip:hover {
  border-color: var(--lc-border-hover);
  background: var(--lc-surface-2);
}

.quick-add-chip.is-set {
  color: var(--lc-accent);
  border-color: var(--lc-accent);
}

.quick-add-chip.is-set.is-priority-p0 {
  color: var(--lc-danger);
  border-color: var(--lc-danger);
}

.quick-add-chip.is-set.is-priority-p1 {
  color: var(--lc-warning);
  border-color: var(--lc-warning);
}

.quick-add-chip.is-set.is-priority-p2 {
  color: var(--lc-accent);
  border-color: var(--lc-accent);
}

.quick-add-chip.is-set.is-priority-p3 {
  color: var(--lc-text-muted);
  border-color: var(--lc-border-hover);
}

.chip-clear {
  margin-left: 2px;
  border-radius: 50%;
  color: var(--lc-text-muted);
}

.chip-clear:hover {
  color: var(--lc-danger);
}

/* 隐藏的日期选择器只作为“选日期…”的弹层锚点 */
.quick-add-date-anchor {
  width: 0;
  height: 0;
  overflow: hidden;
}
</style>

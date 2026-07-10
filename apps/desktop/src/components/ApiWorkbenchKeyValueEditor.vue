<template>
  <div class="api-workbench-kv-editor">
    <div class="api-workbench-kv-header">
      <span>启用</span>
      <span>键名（Key）</span>
      <span>值（Value）</span>
      <span>操作</span>
    </div>
    <div
      v-for="(row, index) in displayRows"
      :key="index"
      class="api-workbench-kv-row"
    >
      <el-switch
        :model-value="row.enabled"
        @update:model-value="update(index, { enabled: Boolean($event) })"
      />
      <el-autocomplete
        v-if="variant === 'headers'"
        :model-value="row.key"
        class="api-workbench-kv-input"
        placeholder="Key"
        :fetch-suggestions="fetchHeaderNameSuggestions"
        :trigger-on-focus="false"
        clearable
        @update:model-value="update(index, { key: String($event) })"
        @paste="handleKeyPaste(index, $event)"
      />
      <el-input
        v-else
        :model-value="row.key"
        class="api-workbench-kv-input"
        placeholder="Key"
        @update:model-value="update(index, { key: String($event) })"
        @paste="handleKeyPaste(index, $event)"
      />
      <el-autocomplete
        v-if="isContentTypeRow(row)"
        :model-value="row.value"
        class="api-workbench-kv-input"
        placeholder="Value"
        :fetch-suggestions="fetchContentTypeSuggestions"
        :trigger-on-focus="true"
        clearable
        @update:model-value="update(index, { value: String($event) })"
      />
      <el-input
        v-else
        :model-value="row.value"
        class="api-workbench-kv-input"
        placeholder="Value"
        @update:model-value="update(index, { value: String($event) })"
        @focus="variablePopover?.onFocus($event)"
        @input="variablePopover?.refresh()"
        @blur="variablePopover?.onBlur()"
        @keydown="variablePopover?.onKeydown($event)"
      />
      <el-button
        class="api-workbench-kv-remove"
        text
        :icon="Delete"
        :disabled="index >= modelValue.length"
        title="删除此行"
        aria-label="删除此行"
        @click="removeRow(index)"
      />
    </div>
    <ApiWorkbenchVariablePopover
      v-if="variableNames.length > 0"
      ref="variablePopover"
      :candidates="variableNames"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { Delete } from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import type { ApiWorkbenchKeyValueRow } from "../types/api-workbench";
import ApiWorkbenchVariablePopover from "./ApiWorkbenchVariablePopover.vue";
import { parseApiWorkbenchKvPaste } from "../utils/apiWorkbenchKvPaste";
import { COMMON_CONTENT_TYPES, COMMON_HEADER_NAMES } from "../utils/apiWorkbenchHeaders";

const props = withDefaults(
  defineProps<{
    modelValue: ApiWorkbenchKeyValueRow[];
    variant?: "query" | "headers" | "form" | "env";
    variableNames?: string[];
  }>(),
  {
    variant: "query",
    variableNames: () => [],
  },
);

const emit = defineEmits<{
  (event: "update:modelValue", value: ApiWorkbenchKeyValueRow[]): void;
}>();

const variablePopover = ref<InstanceType<typeof ApiWorkbenchVariablePopover> | null>(null);

const displayRows = computed<ApiWorkbenchKeyValueRow[]>(() => {
  const rows = props.modelValue;
  const last = rows[rows.length - 1];
  const needsPlaceholder = !last || last.key !== "" || last.value !== "";
  return needsPlaceholder ? [...rows, { enabled: true, key: "", value: "" }] : rows;
});

function update(index: number, patch: Partial<ApiWorkbenchKeyValueRow>) {
  if (index >= props.modelValue.length) {
    emit("update:modelValue", [
      ...props.modelValue,
      { enabled: true, key: "", value: "", ...patch },
    ]);
    return;
  }
  emit(
    "update:modelValue",
    props.modelValue.map((row, i) => (i === index ? { ...row, ...patch } : row)),
  );
}

function removeRow(index: number) {
  if (index >= props.modelValue.length) return;
  emit(
    "update:modelValue",
    props.modelValue.filter((_, i) => i !== index),
  );
}

function handleKeyPaste(index: number, event: Event) {
  const clipboard = (event as ClipboardEvent).clipboardData;
  if (!clipboard) return;
  const parsed = parseApiWorkbenchKvPaste(clipboard.getData("text"));
  if (!parsed) return;
  event.preventDefault();
  const rows = props.modelValue;
  const next =
    index >= rows.length
      ? [...rows, ...parsed.rows]
      : [...rows.slice(0, index), ...parsed.rows, ...rows.slice(index + 1)];
  emit("update:modelValue", next);
  ElMessage.success(`已拆分 ${parsed.rows.length} 行`);
}

function isContentTypeRow(row: ApiWorkbenchKeyValueRow): boolean {
  return props.variant === "headers" && row.key.trim().toLowerCase() === "content-type";
}

type SuggestionCallback = (items: Array<{ value: string }>) => void;

function fetchHeaderNameSuggestions(query: string, callback: SuggestionCallback) {
  const prefix = query.trim().toLowerCase();
  callback(
    COMMON_HEADER_NAMES.filter((name) => name.toLowerCase().startsWith(prefix)).map(
      (value) => ({ value }),
    ),
  );
}

function fetchContentTypeSuggestions(query: string, callback: SuggestionCallback) {
  const prefix = query.trim().toLowerCase();
  callback(
    COMMON_CONTENT_TYPES.filter((name) => name.toLowerCase().startsWith(prefix)).map(
      (value) => ({ value }),
    ),
  );
}
</script>

<style scoped>
.api-workbench-kv-editor {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.api-workbench-kv-header,
.api-workbench-kv-row {
  display: grid;
  grid-template-columns: 48px minmax(0, 1fr) minmax(0, 1.35fr) 68px;
  gap: 8px;
  align-items: center;
}

.api-workbench-kv-header {
  min-height: 24px;
  padding: 0 4px;
  color: var(--el-text-color-secondary);
  font-size: 12px;
  font-weight: 600;
}

.api-workbench-kv-header span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.api-workbench-kv-input {
  min-width: 0;
  width: 100%;
}

.api-workbench-kv-remove {
  justify-self: center;
  opacity: 0;
  transition: opacity 0.15s ease;
}

.api-workbench-kv-row:hover .api-workbench-kv-remove,
.api-workbench-kv-remove:focus-visible {
  opacity: 1;
}
</style>

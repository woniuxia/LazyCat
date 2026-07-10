<template>
  <Teleport to="body">
    <ul
      v-if="visible && filtered.length > 0"
      class="api-workbench-variable-popover"
      :style="{ left: `${position.x}px`, top: `${position.y}px` }"
    >
      <li
        v-for="(name, index) in filtered"
        :key="name"
        :class="{ active: index === activeIndex }"
        @mousedown.prevent
        @click="applyCandidate(name)"
        @mousemove="activeIndex = index"
      >
        <span class="variable-name">{{ name }}</span>
      </li>
    </ul>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import {
  applyApiWorkbenchVariableCompletion,
  matchApiWorkbenchVariablePrefix,
} from "../utils/apiWorkbench";

const props = defineProps<{
  candidates: string[];
}>();

const visible = ref(false);
const activeIndex = ref(0);
const query = ref("");
const position = ref({ x: 0, y: 0 });
let target: HTMLInputElement | null = null;

const filtered = computed(() => {
  const q = query.value.toLowerCase();
  return props.candidates.filter((name) => name.toLowerCase().startsWith(q)).slice(0, 8);
});

function close() {
  visible.value = false;
  query.value = "";
  activeIndex.value = 0;
}

function syncFromTarget() {
  if (!target) {
    close();
    return;
  }
  const cursor = target.selectionStart ?? target.value.length;
  const match = matchApiWorkbenchVariablePrefix(target.value, cursor);
  if (!match) {
    close();
    return;
  }
  query.value = match.query;
  activeIndex.value = 0;
  const rect = target.getBoundingClientRect();
  position.value = { x: rect.left, y: rect.bottom + 4 };
  visible.value = true;
}

function resolveInput(event: Event): HTMLInputElement | null {
  const node = event.target;
  return node instanceof HTMLInputElement ? node : null;
}

function onFocus(event: FocusEvent) {
  target = resolveInput(event);
  syncFromTarget();
}

function refresh() {
  syncFromTarget();
}

function onBlur() {
  target = null;
  close();
}

function onKeydown(event: KeyboardEvent) {
  if (!visible.value || filtered.value.length === 0) return;
  if (event.key === "ArrowDown") {
    event.preventDefault();
    activeIndex.value = (activeIndex.value + 1) % filtered.value.length;
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    activeIndex.value = (activeIndex.value - 1 + filtered.value.length) % filtered.value.length;
  } else if (event.key === "Enter") {
    event.preventDefault();
    applyCandidate(filtered.value[activeIndex.value]);
  } else if (event.key === "Escape") {
    event.stopPropagation();
    close();
  }
}

function applyCandidate(name: string) {
  if (!target) return;
  const cursor = target.selectionStart ?? target.value.length;
  const result = applyApiWorkbenchVariableCompletion(target.value, cursor, name);
  if (!result) {
    close();
    return;
  }
  const input = target;
  input.value = result.text;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  input.setSelectionRange(result.cursor, result.cursor);
  close();
}

defineExpose({ onFocus, refresh, onBlur, onKeydown });
</script>

<style>
.api-workbench-variable-popover {
  position: fixed;
  z-index: 3000;
  min-width: 180px;
  max-height: 220px;
  margin: 0;
  overflow: auto;
  border: 1px solid var(--el-border-color-light);
  border-radius: 6px;
  background: var(--el-bg-color-overlay);
  box-shadow: var(--el-box-shadow-light);
  padding: 4px;
  list-style: none;
}

.api-workbench-variable-popover li {
  border-radius: 4px;
  color: var(--el-text-color-primary);
  cursor: pointer;
  font-family: var(--lc-font-mono);
  font-size: 12px;
  line-height: 1.6;
  padding: 4px 8px;
}

.api-workbench-variable-popover li.active {
  background: var(--el-fill-color-light);
}
</style>

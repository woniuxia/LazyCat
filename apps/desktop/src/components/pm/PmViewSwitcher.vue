<template>
  <div
    ref="containerRef"
    class="pm-view-switcher"
    :class="{ 'is-compact': isCompact }"
    role="tablist"
  >
    <el-tooltip
      v-for="view in PM_VIEWS"
      :key="view.id"
      :content="view.label"
      placement="bottom"
      :disabled="!isCompact"
      :hide-after="0"
      :show-after="200"
    >
      <button
        type="button"
        role="tab"
        class="switcher-item"
        :class="{ 'is-active': modelValue === view.id }"
        :aria-selected="modelValue === view.id"
        :aria-label="view.label"
        @click="$emit('update:modelValue', view.id)"
      >
        <span class="switcher-icon" aria-hidden="true">{{ view.icon }}</span>
        <span v-if="!isCompact" class="switcher-label">{{ view.label }}</span>
      </button>
    </el-tooltip>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { ElTooltip } from "element-plus";
import { PM_VIEWS, type ViewId } from "../../composables/pmViewRegistry";

defineProps<{ modelValue: ViewId }>();
defineEmits<{ (e: "update:modelValue", value: ViewId): void }>();

const containerRef = ref<HTMLElement | null>(null);
const isCompact = ref(false);
const COMPACT_BREAKPOINT = 1100;

let observer: ResizeObserver | null = null;

function evaluateCompact(width: number) {
  if (width <= 0) return;
  isCompact.value = width < COMPACT_BREAKPOINT;
}

onMounted(() => {
  if (!containerRef.value) return;
  if (typeof ResizeObserver === "undefined") {
    evaluateCompact(typeof window !== "undefined" ? window.innerWidth : 0);
    return;
  }
  observer = new ResizeObserver((entries) => {
    const width = entries[0]?.contentRect.width ?? 0;
    evaluateCompact(width);
  });
  observer.observe(document.documentElement);
  evaluateCompact(document.documentElement.clientWidth);
});

onBeforeUnmount(() => {
  observer?.disconnect();
  observer = null;
});
</script>

<style scoped>
.pm-view-switcher {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 4px;
  background: var(--el-fill-color-light);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 10px;
}

.switcher-item {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 30px;
  padding: 0 12px;
  font-size: 13px;
  line-height: 1;
  color: var(--el-text-color-regular);
  background: transparent;
  border: 0;
  border-radius: 8px;
  cursor: pointer;
  transition:
    background-color 160ms ease,
    color 160ms ease,
    transform 160ms ease;
  user-select: none;
}

.switcher-item:hover {
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
}

.switcher-item.is-active {
  background: var(--el-color-primary);
  color: #fff;
  box-shadow: 0 2px 6px rgba(56, 189, 248, 0.28);
}

.switcher-item.is-active:hover {
  background: var(--el-color-primary-dark-2);
  color: #fff;
}

.switcher-icon {
  font-size: 14px;
  line-height: 1;
  font-family: "Segoe UI Symbol", "Apple Symbols", sans-serif;
}

.switcher-label {
  font-weight: 500;
  white-space: nowrap;
}

.pm-view-switcher.is-compact .switcher-item {
  padding: 0 8px;
  min-width: 32px;
  justify-content: center;
}
</style>

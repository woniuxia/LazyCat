<template>
  <Teleport to="body">
    <Transition name="api-workbench-menu-fade">
      <div
        v-if="visible"
        ref="menuRef"
        class="api-workbench-context-menu"
        :style="{ left: pos.x + 'px', top: pos.y + 'px' }"
        role="menu"
        @click.stop
        @contextmenu.prevent.stop
      >
        <template v-for="item in items" :key="item.key">
          <div v-if="item.divider" class="api-workbench-context-menu-divider" />
          <button
            v-else
            type="button"
            class="api-workbench-context-menu-item"
            :class="{ 'is-danger': item.danger }"
            :disabled="item.disabled"
            role="menuitem"
            @click="select(item)"
          >
            {{ item.label }}
          </button>
        </template>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import type { ApiWorkbenchMenuItem } from "../types/api-workbench";
import { clampContextMenuPosition } from "../utils/contextMenu";

const props = defineProps<{
  visible: boolean;
  x: number;
  y: number;
  items: ApiWorkbenchMenuItem[];
}>();

const emit = defineEmits<{
  close: [];
  select: [item: ApiWorkbenchMenuItem];
}>();

const menuRef = ref<HTMLElement | null>(null);
const pos = ref({ x: props.x, y: props.y });

function reposition() {
  const menu = menuRef.value;
  if (!menu) return;
  pos.value = clampContextMenuPosition({
    anchorX: props.x,
    anchorY: props.y,
    menuWidth: menu.offsetWidth,
    menuHeight: menu.offsetHeight,
    viewportWidth: window.innerWidth,
    viewportHeight: window.innerHeight,
  });
}

function closeFromOutside(event: Event) {
  const target = event.target;
  if (target instanceof Node && menuRef.value?.contains(target)) return;
  emit("close");
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") emit("close");
}

function addListeners() {
  document.addEventListener("pointerdown", closeFromOutside);
  document.addEventListener("contextmenu", closeFromOutside);
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("resize", closeFromOutside);
  document.addEventListener("scroll", closeFromOutside, true);
}

function removeListeners() {
  document.removeEventListener("pointerdown", closeFromOutside);
  document.removeEventListener("contextmenu", closeFromOutside);
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("resize", closeFromOutside);
  document.removeEventListener("scroll", closeFromOutside, true);
}

watch(
  () => props.visible,
  (visible) => {
    if (visible) {
      pos.value = { x: props.x, y: props.y };
      nextTick(reposition);
      addListeners();
    } else {
      removeListeners();
    }
  },
);

onBeforeUnmount(removeListeners);

function select(item: ApiWorkbenchMenuItem) {
  if (item.disabled || item.divider) return;
  emit("select", item);
}
</script>

<style>
.api-workbench-context-menu {
  position: fixed;
  z-index: 9999;
  min-width: 152px;
  padding: 4px;
  background: var(--el-bg-color-overlay);
  border: 1px solid var(--el-border-color-light);
  border-radius: 6px;
  box-shadow: 0 10px 24px rgba(15, 23, 42, 0.14);
}

.api-workbench-context-menu-item {
  display: flex;
  width: 100%;
  min-height: 32px;
  align-items: center;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--el-text-color-primary);
  cursor: pointer;
  font: inherit;
  font-size: 13px;
  line-height: 1.3;
  padding: 6px 10px;
  text-align: left;
  transition: background 0.12s ease, color 0.12s ease;
}

.api-workbench-context-menu-item:hover,
.api-workbench-context-menu-item:focus-visible {
  background: var(--el-fill-color-light);
  outline: none;
}

.api-workbench-context-menu-item:disabled {
  color: var(--el-text-color-disabled);
  cursor: not-allowed;
}

.api-workbench-context-menu-item.is-danger {
  color: var(--el-color-danger);
}

.api-workbench-context-menu-item.is-danger:hover,
.api-workbench-context-menu-item.is-danger:focus-visible {
  background: var(--el-color-danger-light-9);
}

.api-workbench-context-menu-divider {
  height: 1px;
  margin: 4px 6px;
  background: var(--el-border-color-extra-light);
}

.api-workbench-menu-fade-enter-active {
  transition: opacity 0.1s ease, transform 0.1s ease;
}

.api-workbench-menu-fade-leave-active {
  transition: opacity 0.08s ease;
}

.api-workbench-menu-fade-enter-from {
  opacity: 0;
  transform: scale(0.98);
}

.api-workbench-menu-fade-leave-to {
  opacity: 0;
}
</style>

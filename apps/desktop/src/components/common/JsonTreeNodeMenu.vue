<template>
  <Teleport to="body">
    <Transition name="json-tree-node-menu-fade">
      <div
        v-if="visible"
        ref="menuRef"
        class="json-tree-node-menu"
        :style="{ left: pos.x + 'px', top: pos.y + 'px' }"
        role="menu"
        @click.stop
        @contextmenu.prevent.stop
      >
        <button
          type="button"
          class="json-tree-node-menu-item"
          role="menuitem"
          @click="select({ kind: 'copy-path' })"
        >
          复制路径
        </button>
        <button
          type="button"
          class="json-tree-node-menu-item"
          role="menuitem"
          @click="select({ kind: 'copy-value' })"
        >
          复制值
        </button>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import type { JsonTreeNodeMenuAction } from "../../types/json-tree";
import { clampContextMenuPosition } from "../../utils/contextMenu";

const props = defineProps<{
  visible: boolean;
  x: number;
  y: number;
}>();

const emit = defineEmits<{
  close: [];
  action: [action: JsonTreeNodeMenuAction];
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
      void nextTick(reposition);
      addListeners();
    } else {
      removeListeners();
    }
  },
);

onBeforeUnmount(removeListeners);

function select(action: JsonTreeNodeMenuAction) {
  emit("action", action);
}
</script>

<!-- Teleport 到 body 的内容:样式不入 scoped,只用全局变量或硬编码设计色 -->
<style>
.json-tree-node-menu {
  position: fixed;
  z-index: 9999;
  min-width: 132px;
  padding: 4px;
  background: var(--el-bg-color-overlay);
  border: 1px solid var(--el-border-color-light);
  border-radius: 6px;
  box-shadow: 0 10px 24px rgba(15, 23, 42, 0.14);
}

.json-tree-node-menu-item {
  display: flex;
  width: 100%;
  min-height: 30px;
  align-items: center;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--el-text-color-primary);
  cursor: pointer;
  font: inherit;
  font-size: 13px;
  line-height: 1.3;
  padding: 5px 10px;
  text-align: left;
  transition:
    background 0.12s ease,
    color 0.12s ease;
}

.json-tree-node-menu-item:hover,
.json-tree-node-menu-item:focus-visible {
  background: var(--el-fill-color-light);
  outline: none;
}

.json-tree-node-menu-item:disabled {
  color: var(--el-text-color-disabled);
  cursor: not-allowed;
}

.json-tree-node-menu-fade-enter-active {
  transition:
    opacity 0.1s ease,
    transform 0.1s ease;
}

.json-tree-node-menu-fade-leave-active {
  transition: opacity 0.08s ease;
}

.json-tree-node-menu-fade-enter-from {
  opacity: 0;
  transform: scale(0.98);
}

.json-tree-node-menu-fade-leave-to {
  opacity: 0;
}
</style>

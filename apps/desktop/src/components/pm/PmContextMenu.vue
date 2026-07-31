<template>
  <Teleport to="body">
    <Transition name="ctx-fade">
      <div
        v-if="visible"
        ref="menuRef"
        class="pm-ctx-menu"
        :style="{ left: pos.x + 'px', top: pos.y + 'px' }"
        @contextmenu.prevent
      >
        <template v-for="(act, idx) in actions" :key="idx">
          <div v-if="act.divider" class="pm-ctx-divider" />
          <div
            v-else
            class="pm-ctx-item"
            :class="{ 'is-danger': act.danger }"
            @click="onSelect(act)"
          >
            {{ act.label }}
          </div>
        </template>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch, nextTick, onBeforeUnmount } from "vue";
import { clampContextMenuPosition } from "../../utils/contextMenu";
import type { CtxMenuAction } from "../../types/pm";

const MENU_WIDTH = 168;
const ITEM_HEIGHT = 34;
const DIVIDER_HEIGHT = 9;
const VERTICAL_PADDING = 8;

const props = defineProps<{
  visible: boolean;
  x: number;
  y: number;
  actions: CtxMenuAction[];
}>();

const emit = defineEmits<{
  close: [];
  select: [action: CtxMenuAction];
}>();

const menuRef = ref<HTMLElement | null>(null);
const pos = ref({ x: props.x, y: props.y });

function estimateHeight(actions: CtxMenuAction[]): number {
  const dividerCount = actions.filter((a) => a.divider).length;
  const itemCount = actions.length - dividerCount;
  return itemCount * ITEM_HEIGHT + dividerCount * DIVIDER_HEIGHT + VERTICAL_PADDING;
}

function reposition() {
  const width = menuRef.value?.offsetWidth ?? MENU_WIDTH;
  const height = menuRef.value?.offsetHeight ?? estimateHeight(props.actions);
  pos.value = clampContextMenuPosition({
    anchorX: props.x,
    anchorY: props.y,
    menuWidth: width,
    menuHeight: height,
    viewportWidth: window.innerWidth,
    viewportHeight: window.innerHeight,
  });
}

function handleClickAway(e: PointerEvent) {
  const target = e.target;
  if (!(target instanceof Element) || !target.closest(".pm-ctx-menu")) {
    emit("close");
  }
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") emit("close");
}

function handleGlobalContextMenu(event: MouseEvent) {
  const target = event.target;
  if (target instanceof Element && target.closest(".pm-ctx-menu")) return;
  emit("close");
}

function handleViewportChange() {
  emit("close");
}

function addListeners() {
  setTimeout(() => {
    if (!props.visible) return;
    document.addEventListener("pointerdown", handleClickAway);
    document.addEventListener("keydown", handleKeydown);
    document.addEventListener("contextmenu", handleGlobalContextMenu);
    document.addEventListener("scroll", handleViewportChange, true);
    window.addEventListener("resize", handleViewportChange);
  }, 0);
}

function removeListeners() {
  document.removeEventListener("pointerdown", handleClickAway);
  document.removeEventListener("keydown", handleKeydown);
  document.removeEventListener("contextmenu", handleGlobalContextMenu);
  document.removeEventListener("scroll", handleViewportChange, true);
  window.removeEventListener("resize", handleViewportChange);
}

watch(
  () => props.visible,
  (val) => {
    if (val) {
      pos.value = { x: props.x, y: props.y };
      nextTick(reposition);
      addListeners();
    } else {
      removeListeners();
    }
  },
);

onBeforeUnmount(removeListeners);

function onSelect(act: CtxMenuAction) {
  emit("close");
  void act.action();
}
</script>

<style>
.pm-ctx-menu {
  position: fixed;
  z-index: 9999;
  background: var(--el-bg-color-overlay);
  border: 1px solid var(--el-border-color-light);
  border-radius: 6px;
  padding: 4px 0;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.12);
  min-width: 140px;
}
.pm-ctx-item {
  padding: 6px 16px;
  font-size: 15px;
  cursor: pointer;
  transition: background 0.15s;
}
.pm-ctx-item:hover {
  background: var(--el-fill-color-light);
}
.pm-ctx-item.is-danger {
  color: var(--el-color-danger);
}
.pm-ctx-item.is-danger:hover {
  background: var(--el-color-danger-light-9);
}
.pm-ctx-divider {
  height: 1px;
  margin: 4px 8px;
  background: var(--el-border-color-extra-light);
}

/* Context menu transition */
.ctx-fade-enter-active {
  transition:
    opacity 0.1s ease,
    transform 0.1s ease;
}
.ctx-fade-leave-active {
  transition: opacity 0.08s ease;
}
.ctx-fade-enter-from {
  opacity: 0;
  transform: scale(0.95);
}
.ctx-fade-leave-to {
  opacity: 0;
}
</style>

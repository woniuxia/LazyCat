<template>
  <Teleport to="body">
    <div
      v-if="visible"
      ref="menuRef"
      class="todo-context-menu"
      :style="{ left: `${pos.x}px`, top: `${pos.y}px` }"
      role="menu"
      aria-label="任务操作菜单"
      @click.stop
      @contextmenu.prevent.stop
    >
      <button
        type="button"
        class="todo-context-menu-item"
        role="menuitem"
        @click="onSelect('pin')"
      >
        {{ pinned ? "取消置顶" : "置顶" }}
      </button>
      <button
        type="button"
        class="todo-context-menu-item"
        role="menuitem"
        @click="onSelect('complete')"
      >
        完成
      </button>
      <button
        type="button"
        class="todo-context-menu-item"
        role="menuitem"
        @click="onSelect('edit-time')"
      >
        编辑任务时间
      </button>
      <div class="todo-context-menu-divider" />
      <button
        type="button"
        class="todo-context-menu-item is-danger"
        role="menuitem"
        @click="onSelect('delete')"
      >
        删除
      </button>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import { clampContextMenuPosition } from "../utils/contextMenu";

export type TodoContextMenuCommand = "pin" | "complete" | "edit-time" | "delete";

const PADDING = 12;

const props = defineProps<{
  visible: boolean;
  x: number;
  y: number;
  pinned: boolean;
}>();

const emit = defineEmits<{
  close: [];
  select: [command: TodoContextMenuCommand];
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
    padding: PADDING,
  });
}

function onGlobalClick(event: MouseEvent) {
  if (!props.visible) return;
  const target = event.target;
  if (target instanceof Node && menuRef.value?.contains(target)) return;
  emit("close");
}

function onGlobalContextMenu(event: MouseEvent) {
  if (!props.visible) return;
  const target = event.target;
  if (target instanceof Node && menuRef.value?.contains(target)) return;
  emit("close");
}

function onGlobalKeydown(event: KeyboardEvent) {
  if (event.key !== "Escape") return;
  emit("close");
}

function addListeners() {
  document.addEventListener("click", onGlobalClick);
  document.addEventListener("contextmenu", onGlobalContextMenu);
  window.addEventListener("keydown", onGlobalKeydown);
}

function removeListeners() {
  document.removeEventListener("click", onGlobalClick);
  document.removeEventListener("contextmenu", onGlobalContextMenu);
  window.removeEventListener("keydown", onGlobalKeydown);
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
  { immediate: true },
);

watch(
  () => [props.x, props.y],
  () => {
    if (props.visible) nextTick(reposition);
  },
);

onBeforeUnmount(removeListeners);

function onSelect(command: TodoContextMenuCommand) {
  emit("select", command);
}
</script>

<style>
.todo-context-menu {
  position: fixed;
  z-index: 3000;
  min-width: 164px;
  padding: 6px;
  border-radius: 14px;
  border: 1px solid rgba(148, 163, 184, 0.22);
  background: rgba(255, 255, 255, 0.96);
  box-shadow: 0 18px 42px rgba(15, 23, 42, 0.16);
  backdrop-filter: blur(16px);
  animation: todoContextMenuEnter 0.16s var(--lc-ease-out);
}

.todo-context-menu-item {
  width: 100%;
  display: flex;
  align-items: center;
  padding: 10px 12px;
  border: none;
  border-radius: 10px;
  background: transparent;
  color: var(--lc-text);
  font-size: 13px;
  text-align: left;
  cursor: pointer;
  transition:
    background var(--lc-duration) var(--lc-ease),
    color var(--lc-duration) var(--lc-ease);
}

.todo-context-menu-item:hover {
  background: rgba(14, 165, 233, 0.08);
  color: var(--lc-accent-strong);
}

.todo-context-menu-item.is-danger {
  color: var(--lc-danger);
}

.todo-context-menu-item.is-danger:hover {
  background: rgba(248, 113, 113, 0.12);
  color: var(--lc-danger);
}

.todo-context-menu-divider {
  height: 1px;
  margin: 6px 4px;
  background: rgba(148, 163, 184, 0.18);
}

@keyframes todoContextMenuEnter {
  from {
    opacity: 0;
    transform: translateY(6px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}
</style>

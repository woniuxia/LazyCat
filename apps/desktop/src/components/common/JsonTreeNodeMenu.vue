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

        <template v-if="editable">
          <div class="json-tree-node-menu-divider" />

          <button
            v-if="isScalar"
            type="button"
            class="json-tree-node-menu-item"
            role="menuitem"
            @click="select({ kind: 'edit-value' })"
          >
            编辑值
          </button>
          <button
            v-if="isObjectField"
            type="button"
            class="json-tree-node-menu-item"
            role="menuitem"
            @click="select({ kind: 'rename-key' })"
          >
            重命名 key
          </button>
          <button
            v-if="isContainer"
            type="button"
            class="json-tree-node-menu-item"
            role="menuitem"
            @click="select({ kind: 'add-child' })"
          >
            添加子字段
          </button>
          <template v-if="isArrayElement">
            <button
              type="button"
              class="json-tree-node-menu-item"
              role="menuitem"
              @click="select({ kind: 'insert-before' })"
            >
              在此前插入
            </button>
            <button
              type="button"
              class="json-tree-node-menu-item"
              role="menuitem"
              @click="select({ kind: 'insert-after' })"
            >
              在此后插入
            </button>
          </template>

          <div
            class="json-tree-node-menu-item json-tree-node-menu-subtrigger"
            role="menuitem"
            aria-haspopup="menu"
            @mouseenter="typeMenuOpen = true"
            @mouseleave="typeMenuOpen = false"
          >
            <span>类型切换</span>
            <span class="json-tree-node-menu-caret" aria-hidden="true">▸</span>
            <div v-if="typeMenuOpen" class="json-tree-node-menu-sub" role="menu">
              <button
                v-for="targetType in SWITCHABLE_TYPES"
                :key="targetType"
                type="button"
                class="json-tree-node-menu-item"
                role="menuitem"
                :disabled="targetType === node?.valueType"
                @click="select({ kind: 'switch-type', valueType: targetType })"
              >
                {{ targetType }}
              </button>
            </div>
          </div>

          <template v-if="!isRoot">
            <button
              type="button"
              class="json-tree-node-menu-item"
              role="menuitem"
              :disabled="!canMoveUp"
              @click="select({ kind: 'move-up' })"
            >
              上移
            </button>
            <button
              type="button"
              class="json-tree-node-menu-item"
              role="menuitem"
              :disabled="!canMoveDown"
              @click="select({ kind: 'move-down' })"
            >
              下移
            </button>
          </template>

          <div class="json-tree-node-menu-divider" />
          <button
            type="button"
            class="json-tree-node-menu-item is-danger"
            role="menuitem"
            :disabled="isRoot"
            @click="select({ kind: 'remove' })"
          >
            删除
          </button>
        </template>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import type { JsonTreeNodeMenuAction } from "../../types/json-tree";
import type { JsonTreeNode as JsonTreeNodeModel } from "../../utils/jsonTreeView";
import type { JsonTreeSwitchableType } from "../../utils/jsonTreeEdit";
import { clampContextMenuPosition } from "../../utils/contextMenu";

const SWITCHABLE_TYPES: JsonTreeSwitchableType[] = [
  "string",
  "number",
  "boolean",
  "null",
  "object",
  "array",
];

const props = withDefaults(
  defineProps<{
    visible: boolean;
    x: number;
    y: number;
    /** 菜单目标节点;编辑项按其形态显隐。 */
    node?: JsonTreeNodeModel | null;
    /** 目标的父节点,用于上移/下移边界判断;根节点为 null。 */
    parent?: JsonTreeNodeModel | null;
    editable?: boolean;
  }>(),
  {
    node: null,
    parent: null,
    editable: false,
  },
);

const emit = defineEmits<{
  close: [];
  action: [action: JsonTreeNodeMenuAction];
}>();

const menuRef = ref<HTMLElement | null>(null);
const pos = ref({ x: props.x, y: props.y });
const typeMenuOpen = ref(false);

const isRoot = computed(() => !!props.node && props.node.path.length === 0);
const lastSegment = computed(() => props.node?.path[props.node.path.length - 1]);
const isObjectField = computed(() => typeof lastSegment.value === "string");
const isArrayElement = computed(() => typeof lastSegment.value === "number");
const isContainer = computed(
  () => props.node?.valueType === "object" || props.node?.valueType === "array",
);
const isScalar = computed(() => !!props.node && !isContainer.value);
const siblingIndex = computed(() => {
  const children = props.parent?.children ?? [];
  return children.findIndex((child) => child.key === props.node?.key);
});
const canMoveUp = computed(() => siblingIndex.value > 0);
const canMoveDown = computed(() => {
  const children = props.parent?.children ?? [];
  return siblingIndex.value >= 0 && siblingIndex.value < children.length - 1;
});

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
    typeMenuOpen.value = false;
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
  background: transparent;
}

.json-tree-node-menu-item.is-danger {
  color: var(--el-color-danger);
}

.json-tree-node-menu-item.is-danger:hover:not(:disabled),
.json-tree-node-menu-item.is-danger:focus-visible:not(:disabled) {
  background: var(--el-color-danger-light-9);
}

.json-tree-node-menu-item.is-danger:disabled {
  color: var(--el-text-color-disabled);
}

.json-tree-node-menu-subtrigger {
  position: relative;
  justify-content: space-between;
  gap: 8px;
}

.json-tree-node-menu-caret {
  color: var(--el-text-color-secondary);
  font-size: 11px;
}

.json-tree-node-menu-sub {
  position: absolute;
  top: -5px;
  left: calc(100% - 2px);
  min-width: 108px;
  padding: 4px;
  background: var(--el-bg-color-overlay);
  border: 1px solid var(--el-border-color-light);
  border-radius: 6px;
  box-shadow: 0 10px 24px rgba(15, 23, 42, 0.14);
}

.json-tree-node-menu-divider {
  height: 1px;
  margin: 4px 6px;
  background: var(--el-border-color-extra-light);
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

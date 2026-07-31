<template>
  <Teleport to="body">
    <div
      v-if="visible"
      ref="menuRef"
      class="rte-file-menu"
      :style="{ left: `${x}px`, top: `${y}px` }"
      @click.stop
      @contextmenu.prevent
    >
      <button type="button" class="rte-file-menu-item" @click="handle('open')">
        <span class="rte-file-menu-icon">↗</span>
        <span>用默认程序打开</span>
      </button>
      <button type="button" class="rte-file-menu-item" @click="handle('reveal')">
        <span class="rte-file-menu-icon">▦</span>
        <span>在资源管理器中显示</span>
      </button>
      <button type="button" class="rte-file-menu-item" @click="handle('copy-path')">
        <span class="rte-file-menu-icon">⎘</span>
        <span>复制{{ kind === "path" ? "路径" : "附件路径" }}</span>
      </button>
      <div v-if="canDelete" class="rte-file-menu-sep" />
      <button
        v-if="canDelete"
        type="button"
        class="rte-file-menu-item is-danger"
        @click="handle('delete')"
      >
        <span class="rte-file-menu-icon">✕</span>
        <span>从描述中移除</span>
      </button>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";

type MenuAction = "open" | "reveal" | "copy-path" | "delete";

const props = withDefaults(
  defineProps<{
    visible: boolean;
    x: number;
    y: number;
    kind: "attachment" | "path";
    canDelete?: boolean;
  }>(),
  { canDelete: false },
);

const emit = defineEmits<{
  (e: "close"): void;
  (e: "action", action: MenuAction): void;
}>();

const menuRef = ref<HTMLElement | null>(null);

function handle(action: MenuAction): void {
  emit("action", action);
  emit("close");
}

function onOutsideClick(e: MouseEvent): void {
  const el = menuRef.value;
  if (!el) return;
  if (e.target instanceof Node && el.contains(e.target)) return;
  emit("close");
}

function onKeyDown(e: KeyboardEvent): void {
  if (e.key === "Escape") emit("close");
}

function attachGlobal(): void {
  // mousedown 捕获阶段：确保在其他处理器前关闭；click 只能关闭菜单内的空白点击
  window.addEventListener("mousedown", onOutsideClick, true);
  window.addEventListener("keydown", onKeyDown, true);
  window.addEventListener("resize", () => emit("close"), { once: true });
  window.addEventListener("blur", () => emit("close"), { once: true });
}

function detachGlobal(): void {
  window.removeEventListener("mousedown", onOutsideClick, true);
  window.removeEventListener("keydown", onKeyDown, true);
}

watch(
  () => props.visible,
  async (v) => {
    if (v) {
      await nextTick();
      attachGlobal();
    } else {
      detachGlobal();
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => detachGlobal());
</script>

<style scoped>
.rte-file-menu {
  position: fixed;
  z-index: 9999;
  min-width: 180px;
  background: var(--el-bg-color, #fff);
  border: 1px solid var(--el-border-color-light, #e4e7ed);
  border-radius: 6px;
  box-shadow: 0 6px 16px rgba(0, 0, 0, 0.12);
  padding: 4px 0;
  user-select: none;
}
.rte-file-menu-item {
  display: flex;
  width: 100%;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  border: 0;
  background: transparent;
  font-size: 13px;
  color: var(--el-text-color-primary);
  cursor: pointer;
  text-align: left;
}
.rte-file-menu-item:hover {
  background: var(--el-fill-color-light);
}
.rte-file-menu-item:focus {
  outline: none;
  background: var(--el-fill-color-light);
}
.rte-file-menu-item.is-danger {
  color: var(--el-color-danger);
}
.rte-file-menu-icon {
  display: inline-block;
  width: 14px;
  color: var(--el-text-color-secondary);
  font-family: var(--el-font-family-monospace, Menlo, Consolas, monospace);
  font-size: 12px;
  text-align: center;
}
.rte-file-menu-sep {
  height: 1px;
  margin: 4px 0;
  background: var(--el-border-color-lighter);
}
</style>

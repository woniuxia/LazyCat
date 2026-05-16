<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="spotlight-action-menu"
      :style="positionStyle"
      role="menu"
      aria-label="备选动作"
      @keydown="onKeydown"
      @click.stop
    >
      <button
        v-for="(action, idx) in actions"
        :key="action.id"
        ref="itemRefs"
        class="spotlight-action-item"
        :class="{ 'is-active': idx === activeIndex, 'is-danger': action.danger }"
        type="button"
        role="menuitem"
        @mouseenter="activeIndex = idx"
        @click="select(action)"
      >
        <span class="spotlight-action-label">{{ action.label }}</span>
        <span v-if="action.needsMasterPassword" class="spotlight-action-hint">需主密码</span>
        <span v-else-if="action.shortcut" class="spotlight-action-hint">{{ action.shortcut }}</span>
      </button>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import type { SpotlightAction } from "../spotlight/types";

const props = defineProps<{
  open: boolean;
  actions: SpotlightAction[];
  anchorRect: DOMRect | null;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "select", action: SpotlightAction): void;
}>();

const activeIndex = ref(0);
const itemRefs = ref<HTMLButtonElement[]>([]);

const positionStyle = computed(() => {
  if (!props.anchorRect) return { display: "none" };
  const left = props.anchorRect.right + 8;
  const top = props.anchorRect.top;
  return {
    left: `${Math.min(left, window.innerWidth - 296)}px`,
    top: `${Math.min(top, window.innerHeight - 200)}px`,
  };
});

watch(
  () => props.open,
  async (open) => {
    if (open) {
      activeIndex.value = 0;
      await nextTick();
      itemRefs.value[0]?.focus();
    }
  },
);

watch(
  () => props.actions,
  () => {
    activeIndex.value = 0;
  },
);

function select(action: SpotlightAction) {
  emit("select", action);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    e.preventDefault();
    emit("close");
    return;
  }
  if (e.key === "ArrowDown") {
    e.preventDefault();
    if (props.actions.length === 0) return;
    activeIndex.value = (activeIndex.value + 1) % props.actions.length;
    itemRefs.value[activeIndex.value]?.focus();
    return;
  }
  if (e.key === "ArrowUp") {
    e.preventDefault();
    if (props.actions.length === 0) return;
    activeIndex.value = (activeIndex.value - 1 + props.actions.length) % props.actions.length;
    itemRefs.value[activeIndex.value]?.focus();
    return;
  }
  if (e.key === "Enter") {
    e.preventDefault();
    const action = props.actions[activeIndex.value];
    if (action) select(action);
    return;
  }
  if (e.key === "Tab") {
    e.preventDefault();
    const dir = e.shiftKey ? -1 : 1;
    if (props.actions.length === 0) return;
    activeIndex.value =
      (activeIndex.value + dir + props.actions.length) % props.actions.length;
    itemRefs.value[activeIndex.value]?.focus();
  }
}
</script>

<style scoped>
.spotlight-action-menu {
  position: fixed;
  min-width: 220px;
  max-width: 280px;
  background: #ffffff;
  border-radius: 10px;
  box-shadow: 0 12px 36px rgba(0, 0, 0, 0.18);
  border: 1px solid rgba(0, 0, 0, 0.06);
  padding: 4px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  z-index: 9999;
}

.spotlight-action-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 10px;
  border: none;
  border-radius: 6px;
  background: transparent;
  cursor: pointer;
  text-align: left;
  font-size: 13px;
  color: #303133;
  font-family: inherit;
}

.spotlight-action-item:focus {
  outline: none;
}

.spotlight-action-item.is-active,
.spotlight-action-item:hover {
  background: #f3f6fb;
}

.spotlight-action-item.is-danger {
  color: #c45656;
}

.spotlight-action-label {
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.spotlight-action-hint {
  font-size: 11px;
  color: #909399;
  flex-shrink: 0;
}
</style>

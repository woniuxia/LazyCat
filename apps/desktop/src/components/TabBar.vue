<template>
  <div class="tabbar">
    <div class="tabbar-list" ref="listRef" @wheel.prevent="onWheel">
      <div
        v-for="tab in tabs"
        :key="tab.id"
        class="tabbar-item"
        :class="{ 'is-active': tab.id === activeId }"
        @click="$emit('select', tab.id)"
        @mousedown.middle.prevent="onMiddleClick(tab)"
        @contextmenu.prevent="onContextMenu($event, tab)"
      >
        <span class="tabbar-item-label">{{ tab.name }}</span>
        <span
          v-if="!tab.pinned"
          class="tabbar-item-close"
          @click.stop="$emit('close', tab.id)"
        >&times;</span>
        <span v-else class="tabbar-item-close-placeholder" />
      </div>
    </div>
  </div>

  <Teleport to="body">
    <div
      v-if="ctxMenu.visible"
      class="tabbar-context-menu"
      :style="{ left: ctxMenu.x + 'px', top: ctxMenu.y + 'px' }"
    >
      <button
        class="tabbar-context-menu-item"
        :class="{ 'is-disabled': ctxMenu.tab?.pinned }"
        @click="onCtxClose"
      >
        关闭
      </button>
      <button
        class="tabbar-context-menu-item"
        @click="onCtxCloseOthers"
      >
        关闭其他
      </button>
      <button
        class="tabbar-context-menu-item"
        @click="onCtxCloseLeft"
      >
        关闭左侧
      </button>
      <button
        class="tabbar-context-menu-item"
        @click="onCtxCloseRight"
      >
        关闭右侧
      </button>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { reactive, ref, onMounted, onBeforeUnmount } from "vue";
import type { TabItem } from "../types/tabs";

defineProps<{
  tabs: TabItem[];
  activeId: string;
}>();

const emit = defineEmits<{
  select: [id: string];
  close: [id: string];
  closeOthers: [id: string];
  closeLeft: [id: string];
  closeRight: [id: string];
}>();

const listRef = ref<HTMLElement | null>(null);

const ctxMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
  tab: null as TabItem | null,
});

function onWheel(e: WheelEvent) {
  if (listRef.value) {
    listRef.value.scrollLeft += e.deltaY || e.deltaX;
  }
}

function onMiddleClick(tab: TabItem) {
  if (!tab.pinned) {
    emit("close", tab.id);
  }
}

function onContextMenu(e: MouseEvent, tab: TabItem) {
  ctxMenu.visible = true;
  ctxMenu.x = e.clientX;
  ctxMenu.y = e.clientY;
  ctxMenu.tab = tab;
}

function hideCtxMenu() {
  ctxMenu.visible = false;
  ctxMenu.tab = null;
}

function onCtxClose() {
  if (ctxMenu.tab && !ctxMenu.tab.pinned) {
    emit("close", ctxMenu.tab.id);
  }
  hideCtxMenu();
}

function onCtxCloseOthers() {
  if (ctxMenu.tab) emit("closeOthers", ctxMenu.tab.id);
  hideCtxMenu();
}

function onCtxCloseLeft() {
  if (ctxMenu.tab) emit("closeLeft", ctxMenu.tab.id);
  hideCtxMenu();
}

function onCtxCloseRight() {
  if (ctxMenu.tab) emit("closeRight", ctxMenu.tab.id);
  hideCtxMenu();
}

function onDocClick() {
  if (ctxMenu.visible) hideCtxMenu();
}

onMounted(() => {
  document.addEventListener("click", onDocClick);
});

onBeforeUnmount(() => {
  document.removeEventListener("click", onDocClick);
});
</script>

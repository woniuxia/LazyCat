<template>
  <div class="api-workbench-tabs-bar">
    <div class="api-workbench-tabs-scroll">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        type="button"
        class="api-workbench-tab"
        :class="{ active: tab.id === activeTabId }"
        :title="tab.name || '未命名请求'"
        @click="emit('activate', tab.id)"
        @mousedown.middle.prevent="emit('close', tab.id)"
        @contextmenu.prevent.stop="openMenu($event, tab.id)"
      >
        <span class="api-workbench-tab-method" :class="getApiWorkbenchMethodClass(tab.draft.method)">
          {{ tab.draft.method }}
        </span>
        <span class="api-workbench-tab-name">
          {{ tab.name || "未命名请求" }}<template v-if="tab.kind === 'temp'"> *</template>
        </span>
        <span v-if="isTabDirty(tab)" class="api-workbench-tab-dirty">●</span>
        <span
          class="api-workbench-tab-close"
          role="button"
          aria-label="关闭标签"
          @click.stop="emit('close', tab.id)"
        >×</span>
      </button>
    </div>
    <el-button
      class="api-workbench-tab-new"
      text
      :icon="Plus"
      title="新建临时请求"
      aria-label="新建临时请求"
      @click="emit('new-temp')"
    />
    <ApiWorkbenchContextMenu
      :visible="menuVisible"
      :x="menuX"
      :y="menuY"
      :items="menuItems"
      @close="menuVisible = false"
      @select="selectMenuItem"
    />
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { Plus } from "@element-plus/icons-vue";
import type { ApiWorkbenchMenuItem, ApiWorkbenchTab } from "../types/api-workbench";
import ApiWorkbenchContextMenu from "./ApiWorkbenchContextMenu.vue";
import { getApiWorkbenchMethodClass } from "../utils/apiWorkbench";
import { isApiWorkbenchTabDirty } from "../utils/apiWorkbenchTabs";

defineProps<{
  tabs: ApiWorkbenchTab[];
  activeTabId: number | null;
}>();

const emit = defineEmits<{
  activate: [tabId: number];
  close: [tabId: number];
  "close-others": [tabId: number];
  "close-left": [tabId: number];
  "close-right": [tabId: number];
  "new-temp": [];
}>();

const isTabDirty = isApiWorkbenchTabDirty;

const menuVisible = ref(false);
const menuX = ref(0);
const menuY = ref(0);
const menuTabId = ref<number | null>(null);
const menuItems: ApiWorkbenchMenuItem[] = [
  { key: "close", label: "关闭" },
  { key: "close-others", label: "关闭其他" },
  { key: "close-left", label: "关闭左侧" },
  { key: "close-right", label: "关闭右侧" },
];

function openMenu(event: MouseEvent, tabId: number) {
  menuTabId.value = tabId;
  menuX.value = event.clientX;
  menuY.value = event.clientY;
  menuVisible.value = true;
}

function selectMenuItem(item: ApiWorkbenchMenuItem) {
  menuVisible.value = false;
  const tabId = menuTabId.value;
  if (tabId === null) return;
  if (item.key === "close") emit("close", tabId);
  else if (item.key === "close-others") emit("close-others", tabId);
  else if (item.key === "close-left") emit("close-left", tabId);
  else if (item.key === "close-right") emit("close-right", tabId);
}
</script>

<style scoped>
.api-workbench-tabs-bar {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 4px;
  border-bottom: 1px solid var(--el-border-color-extra-light);
  padding: 4px 4px 0;
}

.api-workbench-tabs-scroll {
  display: flex;
  min-width: 0;
  flex: 1;
  align-items: flex-end;
  gap: 2px;
  overflow-x: auto;
  scrollbar-width: thin;
}

.api-workbench-tab {
  display: inline-flex;
  max-width: 220px;
  min-width: 0;
  flex: none;
  align-items: center;
  gap: 6px;
  border: 1px solid transparent;
  border-bottom: none;
  border-radius: 6px 6px 0 0;
  background: transparent;
  color: var(--el-text-color-regular);
  cursor: pointer;
  font: inherit;
  font-size: 12px;
  line-height: 1.4;
  padding: 6px 8px;
}

.api-workbench-tab:hover {
  background: var(--el-fill-color-light);
}

.api-workbench-tab.active {
  border-color: var(--el-border-color-extra-light);
  background: var(--el-bg-color);
  color: var(--el-text-color-primary);
  font-weight: 600;
}

.api-workbench-tab-method {
  flex: none;
  font-size: 11px;
  font-weight: 700;
}

.api-workbench-tab-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.api-workbench-tab-dirty {
  flex: none;
  color: var(--el-color-warning);
  font-size: 10px;
}

.api-workbench-tab-close {
  flex: none;
  border-radius: 3px;
  color: var(--el-text-color-secondary);
  font-size: 13px;
  line-height: 1;
  padding: 1px 3px;
}

.api-workbench-tab-close:hover {
  background: var(--el-fill-color);
  color: var(--el-text-color-primary);
}

.api-workbench-tab-new {
  flex: none;
}
</style>

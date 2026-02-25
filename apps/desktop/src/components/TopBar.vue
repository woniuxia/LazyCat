<template>
  <header class="top-bar">
    <div class="top-bar-left">
      <button
        class="top-bar-brand"
        type="button"
        title="点击返回首页"
        @click="goHome"
      >
        <img class="brand-logo" src="../assets/icon.png" alt="Lazycat" />
        <span class="brand-name">Lazycat</span>
      </button>
    </div>

    <div class="top-bar-search">
      <div class="search-box" :class="{ 'is-active': isDropdownOpen }">
        <el-icon class="search-icon"><Search /></el-icon>
        <input
          ref="searchInputRef"
          v-model="searchQuery"
          type="text"
          class="search-input"
          placeholder="搜索工具... 按 / 聚焦"
          @keydown="onSearchKeydown"
          @focus="isDropdownOpen = true"
        />
        <span v-if="searchQuery" class="search-clear" @click="clearSearch">
          <el-icon><CircleClose /></el-icon>
        </span>
        <span v-else class="search-shortcut">/</span>
      </div>

      <!-- Search Dropdown -->
      <div v-if="isDropdownOpen" class="search-dropdown" ref="dropdownRef">
        <template v-if="filteredItems.length > 0">
          <div
            v-for="(item, index) in flattenedResults"
            :key="item.key"
            class="search-result-item"
            :class="{
              'is-highlighted': highlightedIndex === index,
              'is-group-header': item.isGroupHeader
            }"
            @click="selectTool(item.id)"
            @mouseenter="highlightedIndex = index"
          >
            <template v-if="item.isGroupHeader">
              <span class="result-group-name">{{ item.groupName }}</span>
            </template>
            <template v-else>
              <span class="result-name">{{ item.name }}</span>
              <span class="result-group">{{ item.groupName }}</span>
            </template>
          </div>
        </template>
        <div v-else-if="searchQuery.trim()" class="search-empty">
          无匹配工具
        </div>
        <div v-else class="search-hint">
          <div class="search-hint-item">
            <kbd>↑</kbd> <kbd>↓</kbd> 导航
          </div>
          <div class="search-hint-item">
            <kbd>Enter</kbd> 选择
          </div>
          <div class="search-hint-item">
            <kbd>Esc</kbd> 关闭
          </div>
        </div>
      </div>
    </div>

    <div class="top-bar-right">
      <button
        class="top-bar-btn"
        title="设置"
        :class="{ 'is-active': isSettingsActive }"
        @click="goSettings"
      >
        <el-icon><Setting /></el-icon>
      </button>
    </div>
  </header>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, nextTick } from "vue";
import { Search, CircleClose, Setting } from "@element-plus/icons-vue";
import type { SidebarItem, ToolDef } from "../types";

const props = defineProps<{
  allItems: SidebarItem[];
  activeTool: string;
}>();

const emit = defineEmits<{
  select: [id: string];
  "goto-home": [];
  "goto-settings": [];
}>();

const searchQuery = ref("");
const searchInputRef = ref<HTMLInputElement | null>(null);
const dropdownRef = ref<HTMLElement | null>(null);
const isDropdownOpen = ref(false);
const highlightedIndex = ref(-1);

const isSettingsActive = computed(() => props.activeTool === "settings");

// Filter logic adapted from SidebarNav.vue
const filteredItems = computed<SidebarItem[]>(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return props.allItems;
  return props.allItems
    .map((item): SidebarItem | null => {
      if (item.kind === "tool") {
        const t = item.tool;
        if (t.name.toLowerCase().includes(q) || t.desc.toLowerCase().includes(q)) {
          return item;
        }
        return null;
      }
      const group = item.group;
      if (group.name.toLowerCase().includes(q)) {
        return item;
      }
      const matched = group.tools.filter(
        (tool) =>
          tool.name.toLowerCase().includes(q) ||
          tool.desc.toLowerCase().includes(q)
      );
      if (matched.length === 0) return null;
      return { kind: "group", group: { ...group, tools: matched } };
    })
    .filter((item): item is SidebarItem => item !== null);
});

// Flatten results for dropdown display with group headers
interface FlattenedResult {
  key: string;
  id: string;
  name: string;
  groupName: string;
  isGroupHeader: boolean;
}

const flattenedResults = computed<FlattenedResult[]>(() => {
  const results: FlattenedResult[] = [];
  for (const item of filteredItems.value) {
    if (item.kind === "tool") {
      results.push({
        key: item.tool.id,
        id: item.tool.id,
        name: item.tool.name,
        groupName: "工具",
        isGroupHeader: false,
      });
    } else {
      // Add group header
      results.push({
        key: `header-${item.group.id}`,
        id: item.group.id,
        name: item.group.name,
        groupName: item.group.name,
        isGroupHeader: true,
      });
      // Add tools in group
      for (const tool of item.group.tools) {
        results.push({
          key: tool.id,
          id: tool.id,
          name: tool.name,
          groupName: item.group.name,
          isGroupHeader: false,
        });
      }
    }
  }
  return results;
});

function selectTool(id: string) {
  if (!id || id.startsWith("header-")) return;
  emit("select", id);
  clearSearch();
  isDropdownOpen.value = false;
  highlightedIndex.value = -1;
}

function clearSearch() {
  searchQuery.value = "";
  highlightedIndex.value = -1;
  nextTick(() => {
    searchInputRef.value?.focus();
  });
}

function onSearchKeydown(e: KeyboardEvent) {
  const results = flattenedResults.value;

  switch (e.key) {
    case "ArrowDown":
      e.preventDefault();
      if (!isDropdownOpen.value) {
        isDropdownOpen.value = true;
      }
      highlightedIndex.value = (highlightedIndex.value + 1) % results.length;
      break;
    case "ArrowUp":
      e.preventDefault();
      if (!isDropdownOpen.value) {
        isDropdownOpen.value = true;
      }
      highlightedIndex.value =
        highlightedIndex.value <= 0
          ? results.length - 1
          : highlightedIndex.value - 1;
      break;
    case "Enter":
      e.preventDefault();
      if (highlightedIndex.value >= 0 && highlightedIndex.value < results.length) {
        const item = results[highlightedIndex.value];
        if (!item.isGroupHeader) {
          selectTool(item.id);
        }
      } else if (searchQuery.value.trim() && results.length > 0) {
        // Select first non-header result if none highlighted
        const firstTool = results.find((r) => !r.isGroupHeader);
        if (firstTool) {
          selectTool(firstTool.id);
        }
      }
      break;
    case "Escape":
      e.preventDefault();
      isDropdownOpen.value = false;
      highlightedIndex.value = -1;
      searchInputRef.value?.blur();
      break;
  }
}

// Global / key handler
function onGlobalKeydown(e: KeyboardEvent) {
  // Ignore if in input/textarea or modifier keys pressed
  if (
    e.key === "/" &&
    !e.ctrlKey &&
    !e.metaKey &&
    !e.altKey &&
    !["INPUT", "TEXTAREA"].includes((e.target as HTMLElement).tagName)
  ) {
    e.preventDefault();
    searchInputRef.value?.focus();
    isDropdownOpen.value = true;
  }
}

// Click outside to close dropdown
function onDocumentClick(e: MouseEvent) {
  const target = e.target as HTMLElement;
  const dropdown = dropdownRef.value;
  const searchBox = searchInputRef.value?.closest(".search-box");

  if (
    dropdown &&
    !dropdown.contains(target) &&
    searchBox &&
    !searchBox.contains(target)
  ) {
    isDropdownOpen.value = false;
    highlightedIndex.value = -1;
  }
}

function goHome() {
  emit("goto-home");
}

function goSettings() {
  emit("goto-settings");
}

// Focus search input on mount
onMounted(() => {
  window.addEventListener("keydown", onGlobalKeydown);
  document.addEventListener("click", onDocumentClick);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onGlobalKeydown);
  document.removeEventListener("click", onDocumentClick);
});

// Expose method to focus search
function focusSearch() {
  searchInputRef.value?.focus();
  isDropdownOpen.value = true;
}

defineExpose({
  focusSearch,
  clearSearch,
});
</script>

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
          placeholder="搜索工具、快捷启动... 按 / 聚焦"
          @keydown="onSearchKeydown"
          @focus="onSearchFocus"
        />
        <span v-if="searchQuery" class="search-clear" @click="clearSearch">
          <el-icon><CircleClose /></el-icon>
        </span>
        <span v-else class="search-shortcut">/</span>
      </div>

      <div v-if="isDropdownOpen" ref="dropdownRef" class="search-dropdown">
        <div class="search-dropdown-body">
          <template v-if="flattenedResults.length > 0">
            <div
              v-for="(item, index) in flattenedResults"
              :key="item.key"
              class="search-result-item"
              :class="{
                'is-highlighted': highlightedIndex === index,
                'is-group-header': item.kind === 'header',
              }"
              @click="onRowClick(item)"
              @mouseenter="highlightedIndex = index"
            >
              <template v-if="item.kind === 'header'">
                <span class="result-group-name">{{ item.label }}</span>
              </template>
              <template v-else>
                <span class="result-name">{{ item.name }}</span>
                <span class="result-group">{{ item.group }}</span>
              </template>
            </div>
          </template>
          <div v-else-if="searchQuery.trim()" class="search-empty">
            无匹配结果
          </div>
        </div>
        <div class="search-hint">
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
import { ref, computed, onMounted, onBeforeUnmount, nextTick, watch } from "vue";
import { ElMessage } from "element-plus";
import { Search, CircleClose, Setting } from "@element-plus/icons-vue";
import type { SidebarItem, ToolSearchMetaMap } from "../types";
import { invokeToolByChannel } from "../bridge/tauri";
import { matchScore } from "../utils/fuzzy-match";
import { buildToolIndex } from "../utils/search-index";

interface LauncherEntry {
  id: number;
  name: string;
  exe_path: string;
  arguments: string;
  group_name: string;
  launch_count: number;
}

interface ToolResult {
  kind: "tool";
  key: string;
  id: string;
  name: string;
  group: string;
  score: number;
}

interface LauncherResult {
  kind: "launcher";
  key: string;
  id: string;
  name: string;
  group: string;
  score: number;
  entry: LauncherEntry;
}

interface HeaderRow {
  kind: "header";
  key: string;
  label: string;
}

type SearchRow = ToolResult | LauncherResult | HeaderRow;

const props = defineProps<{
  allItems: SidebarItem[];
  activeTool: string;
  searchMetaMap: ToolSearchMetaMap;
  clickCountFn: (toolId: string) => number;
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
const launcherEntries = ref<LauncherEntry[]>([]);
const launcherLoaded = ref(false);
const launcherLoading = ref(false);

const isSettingsActive = computed(() => props.activeTool === "settings");
const indexedTools = computed(() => buildToolIndex(props.allItems, props.searchMetaMap));

const matchedTools = computed<ToolResult[]>(() => {
  const q = searchQuery.value.trim();
  if (!q) return [];

  const matches = indexedTools.value
    .map((item) => ({
      item,
      score: matchScore(q, item.fields),
    }))
    .filter((row) => row.score > 0)
    .sort((a, b) => b.score - a.score || a.item.tool.name.localeCompare(b.item.tool.name, "zh-CN"))
    .slice(0, 40)
    .map((row) => ({
      kind: "tool" as const,
      key: `tool-${row.item.tool.id}`,
      id: row.item.tool.id,
      name: row.item.tool.name,
      group: row.item.groupName,
      score: row.score,
    }));

  return matches;
});

const matchedLauncher = computed<LauncherResult[]>(() => {
  const q = searchQuery.value.trim();
  if (!q) return [];

  return launcherEntries.value
    .map((entry) => {
      const score = matchScore(q, [
        { text: entry.name, initials: "", weight: 1.15 },
        { text: entry.exe_path, initials: "", weight: 0.75 },
        { text: entry.group_name, initials: "", weight: 0.7 },
      ]);
      return { entry, score };
    })
    .filter((row) => row.score > 0)
    .sort((a, b) => b.score - a.score || b.entry.launch_count - a.entry.launch_count)
    .slice(0, 20)
    .map((row) => ({
      kind: "launcher" as const,
      key: `launcher-${row.entry.id}`,
      id: `launcher-${row.entry.id}`,
      name: row.entry.name,
      group: "快捷启动",
      score: row.score,
      entry: row.entry,
    }));
});

const recommendations = computed<SearchRow[]>(() => {
  const tools = indexedTools.value
    .map((item) => ({
      item,
      count: props.clickCountFn(item.tool.id),
    }))
    .filter((row) => row.count > 0)
    .sort((a, b) => b.count - a.count)
    .slice(0, 8)
    .map((row) => ({
      kind: "tool" as const,
      key: `rec-${row.item.tool.id}`,
      id: row.item.tool.id,
      name: row.item.tool.name,
      group: row.item.groupName,
      score: row.count,
    }));

  if (tools.length === 0) return [];
  return [
    { kind: "header" as const, key: "header-recent", label: "最近常用" },
    ...tools,
  ];
});

const flattenedResults = computed<SearchRow[]>(() => {
  const q = searchQuery.value.trim();
  if (!q) return recommendations.value;

  const rows: SearchRow[] = [];
  if (matchedTools.value.length > 0) {
    rows.push({ kind: "header", key: "header-tools", label: "功能" });
    rows.push(...matchedTools.value);
  }
  if (matchedLauncher.value.length > 0) {
    rows.push({ kind: "header", key: "header-launcher", label: "快捷启动" });
    rows.push(...matchedLauncher.value);
  }
  return rows;
});

async function ensureLauncherEntries() {
  if (launcherLoaded.value || launcherLoading.value) return;
  launcherLoading.value = true;
  try {
    const res = (await invokeToolByChannel("tool:launcher:list", {})) as { items: LauncherEntry[] };
    launcherEntries.value = res.items ?? [];
    launcherLoaded.value = true;
  } catch {
    launcherEntries.value = [];
  } finally {
    launcherLoading.value = false;
  }
}

function firstActionableIndex(): number {
  return flattenedResults.value.findIndex((row) => row.kind !== "header");
}

function moveHighlight(direction: 1 | -1) {
  const rows = flattenedResults.value;
  if (rows.length === 0) return;

  let index = highlightedIndex.value;
  for (let i = 0; i < rows.length; i += 1) {
    index = (index + direction + rows.length) % rows.length;
    if (rows[index].kind !== "header") {
      highlightedIndex.value = index;
      return;
    }
  }
}

async function selectRow(row: SearchRow | undefined) {
  if (!row || row.kind === "header") return;

  if (row.kind === "tool") {
    emit("select", row.id);
    clearSearch();
    isDropdownOpen.value = false;
    highlightedIndex.value = -1;
    return;
  }

  try {
    await invokeToolByChannel("tool:launcher:launch", {
      exe_path: row.entry.exe_path,
      arguments: row.entry.arguments,
      admin: false,
    });
    clearSearch();
    isDropdownOpen.value = false;
    highlightedIndex.value = -1;
  } catch (e) {
    ElMessage.error(`启动失败：${(e as Error).message}`);
  }
}

function onRowClick(row: SearchRow) {
  void selectRow(row);
}

function clearSearch() {
  searchQuery.value = "";
  highlightedIndex.value = -1;
  nextTick(() => {
    searchInputRef.value?.focus();
  });
}

function onSearchFocus() {
  isDropdownOpen.value = true;
  if (searchQuery.value.trim()) {
    void ensureLauncherEntries();
  }
}

function onSearchKeydown(e: KeyboardEvent) {
  const rows = flattenedResults.value;
  switch (e.key) {
    case "ArrowDown":
      e.preventDefault();
      if (!isDropdownOpen.value) isDropdownOpen.value = true;
      moveHighlight(1);
      break;
    case "ArrowUp":
      e.preventDefault();
      if (!isDropdownOpen.value) isDropdownOpen.value = true;
      moveHighlight(-1);
      break;
    case "Enter":
      e.preventDefault();
      if (rows.length === 0) return;
      if (highlightedIndex.value < 0 || rows[highlightedIndex.value]?.kind === "header") {
        highlightedIndex.value = firstActionableIndex();
      }
      void selectRow(rows[highlightedIndex.value]);
      break;
    case "Escape":
      e.preventDefault();
      isDropdownOpen.value = false;
      highlightedIndex.value = -1;
      searchInputRef.value?.blur();
      break;
  }
}

function onGlobalKeydown(e: KeyboardEvent) {
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

function onDocumentClick(e: MouseEvent) {
  const target = e.target as HTMLElement;
  const dropdown = dropdownRef.value;
  const searchBox = searchInputRef.value?.closest(".search-box");

  if (dropdown && !dropdown.contains(target) && searchBox && !searchBox.contains(target)) {
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

watch(
  () => searchQuery.value,
  (value) => {
    highlightedIndex.value = -1;
    if (value.trim()) {
      isDropdownOpen.value = true;
      void ensureLauncherEntries();
    }
  },
);

onMounted(() => {
  window.addEventListener("keydown", onGlobalKeydown);
  document.addEventListener("click", onDocumentClick);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onGlobalKeydown);
  document.removeEventListener("click", onDocumentClick);
});

function focusSearch() {
  searchInputRef.value?.focus();
  isDropdownOpen.value = true;
}

defineExpose({
  focusSearch,
  clearSearch,
});
</script>

<template>
  <aside class="nav">
    <button
      class="brand brand-link"
      type="button"
      title="左键返回首页，右键打开代码片段工作区"
      @click="goHome"
      @contextmenu.prevent="openSnippetWorkspace"
    >
      <span class="brand-name">Lazycat</span>
      <span class="brand-zh">懒猫</span>
    </button>

    <div class="nav-toolbar">
      <el-input
        v-model="searchQuery"
        placeholder="搜索工具..."
        clearable
        :prefix-icon="Search"
      />
      <div class="nav-toolbar-actions">
        <button
          class="nav-toolbar-btn"
          title="全部折叠"
          @click="collapseAll"
        >
          <el-icon><Fold /></el-icon>
          <span>折叠</span>
        </button>
        <button
          class="nav-toolbar-btn"
          title="全部展开"
          @click="expandAll"
        >
          <el-icon><Expand /></el-icon>
          <span>展开</span>
        </button>
        <button
          class="nav-toolbar-btn"
          title="定位当前工具"
          :disabled="activeTool === 'home' || activeTool === 'settings'"
          @click="locateCurrentTool"
        >
          <el-icon><Aim /></el-icon>
          <span>定位</span>
        </button>
      </div>
    </div>

    <el-menu
      ref="menuRef"
      :default-active="activeTool"
      @select="(index: string) => emit('select', index)"
    >
      <el-menu-item v-if="showHome" index="home">首页</el-menu-item>
      <template v-if="filteredItems.length">
        <template v-for="item in filteredItems" :key="item.kind === 'group' ? item.group.id : item.tool.id">
          <el-menu-item
            v-if="item.kind === 'tool'"
            :index="item.tool.id"
            class="nav-top-tool"
          >
            {{ item.tool.name }}
          </el-menu-item>
          <el-sub-menu
            v-else
            :index="item.group.id"
          >
            <template #title>{{ item.group.name }}</template>
            <el-menu-item
              v-for="tool in item.group.tools"
              :key="tool.id"
              :index="tool.id"
            >
              {{ tool.name }}
            </el-menu-item>
          </el-sub-menu>
        </template>
      </template>
      <div v-else-if="searchQuery.trim()" class="nav-empty">
        无匹配工具      </div>
    </el-menu>

    <div class="nav-bottom">
      <div
        class="nav-bottom-item"
        :class="{ 'is-active': activeTool === 'settings' }"
        @click="emit('select', 'settings')"
      >
        设置
      </div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { Search, Fold, Expand, Aim } from "@element-plus/icons-vue";
import type { SidebarItem } from "../types";

const props = defineProps<{
  items: SidebarItem[];
  activeTool: string;
}>();

const emit = defineEmits<{
  select: [id: string];
  openSnippetWorkspace: [];
}>();

const searchQuery = ref("");
const menuRef = ref<{ open: (index: string) => void; close: (index: string) => void } | null>(null);

const showHome = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return true;
  return "首页".includes(q) || "home".includes(q);
});

const filteredItems = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return props.items;
  return props.items
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

const groupItems = computed(() =>
  props.items.filter((item): item is SidebarItem & { kind: "group" } => item.kind === "group")
);

const filteredGroupItems = computed(() =>
  filteredItems.value.filter((item): item is SidebarItem & { kind: "group" } => item.kind === "group")
);

function collapseAll() {
  const menu = menuRef.value;
  if (!menu) return;
  for (const item of groupItems.value) {
    menu.close(item.group.id);
  }
}

function expandAll() {
  const menu = menuRef.value;
  if (!menu) return;
  for (const item of filteredGroupItems.value) {
    menu.open(item.group.id);
  }
}

function locateCurrentTool() {
  const tool = props.activeTool;
  if (tool === "home" || tool === "settings") return;

  searchQuery.value = "";

  const targetGroup = groupItems.value.find((item) =>
    item.group.tools.some((t) => t.id === tool)
  );
  if (!targetGroup) return;

  const menu = menuRef.value;
  if (!menu) return;

  menu.open(targetGroup.group.id);

  void nextTick(() => {
    setTimeout(() => {
      const el = document.querySelector(".nav .el-menu-item.is-active");
      if (el) {
        el.scrollIntoView({ behavior: "smooth", block: "center" });
      }
    }, 300);
  });
}

function goHome() {
  searchQuery.value = "";
  emit("select", "home");
}

function openSnippetWorkspace() {
  emit("openSnippetWorkspace");
}

watch(searchQuery, () => {
  void nextTick(() => {
    const menu = menuRef.value;
    if (!menu) return;
    for (const item of filteredGroupItems.value) {
      menu.open(item.group.id);
    }
  });
});
</script>



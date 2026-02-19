<template>
  <aside class="nav">
    <div class="brand">
      <span class="brand-name">Lazycat</span>
      <span class="brand-zh">懒猫</span>
    </div>

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
      <template v-if="filteredGroups.length">
        <el-sub-menu
          v-for="group in filteredGroups"
          :key="group.id"
          :index="group.id"
        >
          <template #title>{{ group.name }}</template>
          <el-menu-item
            v-for="tool in group.tools"
            :key="tool.id"
            :index="tool.id"
          >
            {{ tool.name }}
          </el-menu-item>
        </el-sub-menu>
      </template>
      <div v-else-if="searchQuery.trim()" class="nav-empty">
        无匹配工具
      </div>
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

interface ToolDef {
  id: string;
  name: string;
  desc: string;
}
interface GroupDef {
  id: string;
  name: string;
  tools: ToolDef[];
}

const props = defineProps<{
  groups: GroupDef[];
  activeTool: string;
}>();

const emit = defineEmits<{
  select: [id: string];
}>();

const searchQuery = ref("");
const menuRef = ref<{ open: (index: string) => void; close: (index: string) => void } | null>(null);

const showHome = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return true;
  return "首页".includes(q) || "home".includes(q);
});

const filteredGroups = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return props.groups;
  return props.groups
    .map((group) => {
      if (group.name.toLowerCase().includes(q)) {
        return group;
      }
      const matched = group.tools.filter(
        (tool) =>
          tool.name.toLowerCase().includes(q) ||
          tool.desc.toLowerCase().includes(q)
      );
      if (matched.length === 0) return null;
      return { ...group, tools: matched };
    })
    .filter((g): g is GroupDef => g !== null);
});

function collapseAll() {
  const menu = menuRef.value;
  if (!menu) return;
  for (const group of props.groups) {
    menu.close(group.id);
  }
}

function expandAll() {
  const menu = menuRef.value;
  if (!menu) return;
  for (const group of filteredGroups.value) {
    menu.open(group.id);
  }
}

function locateCurrentTool() {
  const tool = props.activeTool;
  if (tool === "home" || tool === "settings") return;

  searchQuery.value = "";

  const targetGroup = props.groups.find((g) =>
    g.tools.some((t) => t.id === tool)
  );
  if (!targetGroup) return;

  const menu = menuRef.value;
  if (!menu) return;

  menu.open(targetGroup.id);

  void nextTick(() => {
    setTimeout(() => {
      const el = document.querySelector(".nav .el-menu-item.is-active");
      if (el) {
        el.scrollIntoView({ behavior: "smooth", block: "center" });
      }
    }, 300);
  });
}

watch(searchQuery, () => {
  void nextTick(() => {
    const menu = menuRef.value;
    if (!menu) return;
    for (const group of filteredGroups.value) {
      menu.open(group.id);
    }
  });
});
</script>

<template>
  <div class="home-panel">
    <!-- Favorite Tools Section -->
    <section v-if="favoriteTools.length" class="home-section">
      <div class="home-section-header">
        <h2>★ 常用工具</h2>
      </div>
      <div class="home-card-grid">
        <div
          v-for="(tool, index) in favoriteTools"
          :key="tool.id"
          class="home-tool-card"
          :style="{ '--card-index': index }"
          tabindex="0"
          @click="emit('openTool', tool.id)"
          @keyup.enter="emit('openTool', tool.id)"
          @mousemove="onCardMouseMove"
          @mouseleave="onCardMouseLeave"
        >
          <el-button class="home-tool-card-action" text type="warning" @click.stop="emit('toggleFavorite', tool.id)">
            取消收藏
          </el-button>
          <div class="home-tool-card-title">{{ tool.name }}</div>
          <div class="home-tool-card-desc">{{ tool.desc }}</div>
        </div>
      </div>
    </section>

    <!-- Top Monthly Tools Section -->
    <section v-if="topMonthlyTools.length" class="home-section">
      <div class="home-section-header">
        <h2>最近常用</h2>
      </div>
      <div class="home-card-grid">
        <div
          v-for="(item, index) in topMonthlyTools"
          :key="item.tool.id"
          class="home-tool-card"
          :style="{ '--card-index': index }"
          tabindex="0"
          @click="emit('openTool', item.tool.id)"
          @keyup.enter="emit('openTool', item.tool.id)"
          @mousemove="onCardMouseMove"
          @mouseleave="onCardMouseLeave"
        >
          <el-button
            class="home-tool-card-action"
            text
            :type="isFavorite(item.tool.id) ? 'warning' : 'primary'"
            @click.stop="emit('toggleFavorite', item.tool.id)"
          >
            {{ isFavorite(item.tool.id) ? "取消收藏" : "收藏" }}
          </el-button>
          <div class="home-tool-card-title">{{ item.tool.name }}</div>
          <div class="home-tool-card-desc">{{ item.tool.desc }}</div>
        </div>
      </div>
    </section>

    <!-- Grouped Tools Sections -->
    <section
      v-for="(group, groupIndex) in groupedTools"
      :key="group.id"
      class="home-section"
    >
      <div class="home-section-header">
        <h2>{{ group.name }}</h2>
        <span class="group-count">{{ group.tools.length }} 个工具</span>
      </div>
      <div class="home-card-grid">
        <div
          v-for="(tool, toolIndex) in group.tools"
          :key="tool.id"
          class="home-tool-card"
          :style="{ '--card-index': groupIndex * 10 + toolIndex }"
          tabindex="0"
          @click="emit('openTool', tool.id)"
          @keyup.enter="emit('openTool', tool.id)"
          @mousemove="onCardMouseMove"
          @mouseleave="onCardMouseLeave"
        >
          <el-button
            class="home-tool-card-action"
            text
            :type="isFavorite(tool.id) ? 'warning' : 'primary'"
            @click.stop="emit('toggleFavorite', tool.id)"
          >
            {{ isFavorite(tool.id) ? "取消收藏" : "收藏" }}
          </el-button>
          <div class="home-tool-card-title">{{ tool.name }}</div>
          <div class="home-tool-card-desc">{{ tool.desc }}</div>
        </div>
      </div>
    </section>

    <!-- Empty State -->
    <section v-if="!hasAnyContent" class="home-section">
      <el-empty description="暂无工具，点击右上角设置可显示更多工具">
        <template #image>
          <div style="font-size: 48px;">🐱</div>
        </template>
      </el-empty>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import type { ToolDef, SidebarItem } from "../types";

interface TopMonthlyItem {
  tool: ToolDef;
  count: number;
}

interface GroupedTool {
  id: string;
  name: string;
  tools: ToolDef[];
}

const props = defineProps<{
  allItems: SidebarItem[];
  favoriteTools: ToolDef[];
  topMonthlyTools: TopMonthlyItem[];
  isFavorite: (id: string) => boolean;
}>();

const emit = defineEmits<{
  (event: "openTool", id: string): void;
  (event: "toggleFavorite", id: string): void;
}>();
// Extract grouped tools from allItems
const groupedTools = computed<GroupedTool[]>(() => {
  const groups: GroupedTool[] = [];
  for (const item of props.allItems) {
    if (item.kind === "group") {
      groups.push({
        id: item.group.id,
        name: item.group.name,
        tools: item.group.tools,
      });
    } else {
      // Single tools go into an "其他" group or individual section
      // For single tools, we create a pseudo-group
      groups.push({
        id: item.tool.id,
        name: item.tool.name,
        tools: [item.tool],
      });
    }
  }
  return groups;
});

const hasAnyContent = computed(() => {
  return (
    props.favoriteTools.length > 0 ||
    props.topMonthlyTools.length > 0 ||
    groupedTools.value.length > 0
  );
});

function onCardMouseMove(e: MouseEvent) {
  const card = e.currentTarget as HTMLElement;
  const rect = card.getBoundingClientRect();
  card.style.setProperty("--mx", `${e.clientX - rect.left}px`);
  card.style.setProperty("--my", `${e.clientY - rect.top}px`);
}

function onCardMouseLeave(e: MouseEvent) {
  const card = e.currentTarget as HTMLElement;
  card.style.removeProperty("--mx");
  card.style.removeProperty("--my");
}
</script>

<style scoped>
.group-count {
  font-size: 12px;
  color: var(--lc-text-secondary);
  font-weight: 400;
}
</style>

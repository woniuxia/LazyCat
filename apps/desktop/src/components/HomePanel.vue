<template>
  <div class="home-panel">
    <!-- Merged Tools Section -->
    <section v-if="mergedHomeTools.length" class="home-section">
      <div class="home-section-header">
        <h2>★ 常用工具</h2>
      </div>
      <div ref="mergedGridRef" class="home-card-grid">
        <div
          v-for="(item, index) in mergedHomeTools"
          :key="item.tool.id"
          class="home-tool-card"
          :class="{ 'is-favorite': item.isFavorite }"
          :data-id="item.tool.id"
          :style="{ '--card-index': index }"
          tabindex="0"
          @click="emit('openTool', item.tool.id)"
          @keyup.enter="emit('openTool', item.tool.id)"
          @mousemove="onCardMouseMove"
          @mouseleave="onCardMouseLeave"
        >
          <span v-if="item.isFavorite" class="drag-handle">⠿</span>
          <el-button
            class="home-tool-card-action"
            text
            :type="item.isFavorite ? 'warning' : 'primary'"
            @click.stop="emit('toggleFavorite', item.tool.id)"
          >
            {{ item.isFavorite ? "取消收藏" : "收藏" }}
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
import { computed, ref, onMounted, onBeforeUnmount, nextTick } from "vue";
import Sortable from "sortablejs";
import type { SidebarItem } from "../types";
import type { MergedHomeTool } from "../composables/useFavorites";

interface GroupedTool {
  id: string;
  name: string;
  tools: { id: string; name: string; desc: string }[];
}

const props = defineProps<{
  allItems: SidebarItem[];
  mergedHomeTools: MergedHomeTool[];
  isFavorite: (id: string) => boolean;
}>();

const emit = defineEmits<{
  (event: "openTool", id: string): void;
  (event: "toggleFavorite", id: string): void;
  (event: "reorderFavorites", newIds: string[]): void;
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
      groups.push({
        id: item.tool.id,
        name: item.tool.name,
        tools: [item.tool],
      });
    }
  }
  return groups;
});

const hasAnyContent = computed(() =>
  props.mergedHomeTools.length > 0 || groupedTools.value.length > 0
);

// SortableJS for drag-reorder of favorite cards
const mergedGridRef = ref<HTMLElement | null>(null);
let sortableInstance: Sortable | null = null;

function initSortable() {
  if (sortableInstance) {
    sortableInstance.destroy();
    sortableInstance = null;
  }
  const el = mergedGridRef.value;
  if (!el) return;
  sortableInstance = Sortable.create(el, {
    animation: 150,
    handle: ".drag-handle",
    draggable: ".is-favorite",
    ghostClass: "sortable-ghost",
    forceFallback: true,
    onMove: (evt) => evt.related.classList.contains("is-favorite"),
    onEnd: (evt) => {
      if (evt.oldIndex == null || evt.newIndex == null || evt.oldIndex === evt.newIndex) return;
      const favCount = props.mergedHomeTools.filter((i) => i.isFavorite).length;
      if (evt.newIndex >= favCount) return;
      const newIds = [...el.querySelectorAll(".is-favorite")]
        .map((node) => (node as HTMLElement).dataset.id!)
        .filter(Boolean);
      emit("reorderFavorites", newIds);
    },
  });
}

onMounted(() => nextTick(initSortable));
onBeforeUnmount(() => {
  sortableInstance?.destroy();
  sortableInstance = null;
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

.drag-handle {
  position: absolute;
  top: 6px;
  left: 6px;
  cursor: grab;
  color: var(--lc-text-secondary);
  font-size: 14px;
  opacity: 0.4;
  user-select: none;
}

.drag-handle:active {
  cursor: grabbing;
}

:deep(.sortable-ghost) {
  opacity: 0.4;
  background: var(--lc-accent-dim);
  border: 1px dashed var(--lc-accent);
}
</style>

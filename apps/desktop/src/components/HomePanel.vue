<template>
  <div class="home-panel">
    <!-- Merged Tools Section -->
    <section v-if="mergedHomeTools.length" class="home-section">
      <div class="home-section-header">
        <h2>
          <svg
            class="section-icon"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="currentColor"
            aria-hidden="true"
          >
            <path
              d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"
            />
          </svg>
          常用工具
        </h2>
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
    <section v-for="(group, groupIndex) in groupedTools" :key="group.id" class="home-section">
      <div class="home-section-header">
        <h2>{{ group.name }}</h2>
        <span class="group-count">{{ group.tools.length }}</span>
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
          <svg
            width="64"
            height="64"
            viewBox="0 0 64 64"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
            aria-hidden="true"
          >
            <path d="M10 18 L10 8 L20 18" fill="var(--lc-text-muted)" opacity="0.7" />
            <path d="M54 18 L54 8 L44 18" fill="var(--lc-text-muted)" opacity="0.7" />
            <ellipse cx="32" cy="36" rx="22" ry="20" fill="var(--lc-surface-3)" />
            <circle cx="23" cy="34" r="3" fill="var(--lc-text-secondary)" />
            <circle cx="41" cy="34" r="3" fill="var(--lc-text-secondary)" />
            <path
              d="M27 42 Q32 46 37 42"
              stroke="var(--lc-text-secondary)"
              stroke-width="2"
              stroke-linecap="round"
              fill="none"
            />
            <line
              x1="14"
              y1="30"
              x2="24"
              y2="33"
              stroke="var(--lc-text-muted)"
              stroke-width="1.5"
              stroke-linecap="round"
            />
            <line
              x1="14"
              y1="35"
              x2="24"
              y2="35"
              stroke="var(--lc-text-muted)"
              stroke-width="1.5"
              stroke-linecap="round"
            />
            <line
              x1="50"
              y1="30"
              x2="40"
              y2="33"
              stroke="var(--lc-text-muted)"
              stroke-width="1.5"
              stroke-linecap="round"
            />
            <line
              x1="50"
              y1="35"
              x2="40"
              y2="35"
              stroke="var(--lc-text-muted)"
              stroke-width="1.5"
              stroke-linecap="round"
            />
          </svg>
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

const hasAnyContent = computed(
  () => props.mergedHomeTools.length > 0 || groupedTools.value.length > 0,
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
.section-icon {
  color: var(--lc-accent);
  flex-shrink: 0;
  margin-right: 8px;
}

.group-count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 22px;
  height: 20px;
  padding: 0 7px;
  border-radius: 10px;
  background: var(--lc-accent-dim);
  color: var(--lc-accent);
  font-size: 11px;
  font-weight: 600;
  line-height: 1;
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

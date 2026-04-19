<template>
  <section class="pm-today-section" :style="{ '--pm-today-accent': accentColor }">
    <header class="pm-today-section-head" @click="toggle">
      <span class="pm-today-section-icon">{{ icon }}</span>
      <span class="pm-today-section-title">{{ title }}</span>
      <span class="pm-today-section-count">{{ items.length }}</span>
      <span v-if="collapsible" class="pm-today-section-toggle" aria-hidden="true">
        {{ collapsed ? "展开" : "收起" }}
      </span>
    </header>
    <div v-if="!collapsed" class="pm-today-section-body">
      <div v-if="items.length === 0" class="pm-today-section-empty">
        <span>{{ loading ? "加载中…" : emptyText }}</span>
      </div>
      <div v-else class="pm-today-card-list">
        <PmTodayCard
          v-for="item in items"
          :key="item.id"
          :item="item"
          :selected="selectedItemId === item.id"
          @click="$emit('select', item)"
          @dblclick="$emit('edit', item)"
          @contextmenu.prevent="(e: MouseEvent) => emit('item-context', e, item)"
          @start="(i: PmItem) => emit('start', i)"
          @postpone="(i: PmItem) => emit('postpone', i)"
          @complete="(i: PmItem) => emit('complete', i)"
          @detail="(i: PmItem) => emit('select', i)"
        />
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { ref } from "vue";
import type { PmItem } from "../types/pm";
import PmTodayCard from "./PmTodayCard.vue";

const props = defineProps<{
  keyName: string;
  icon: string;
  title: string;
  accentColor: string;
  emptyText: string;
  items: PmItem[];
  selectedItemId: number | null;
  loading: boolean;
  collapsible?: boolean;
  defaultCollapsed?: boolean;
}>();

const emit = defineEmits<{
  (e: "select", item: PmItem): void;
  (e: "edit", item: PmItem): void;
  (e: "item-context", event: MouseEvent, item: PmItem): void;
  (e: "start", item: PmItem): void;
  (e: "postpone", item: PmItem): void;
  (e: "complete", item: PmItem): void;
}>();

const collapsed = ref<boolean>(Boolean(props.defaultCollapsed));

function toggle() {
  if (!props.collapsible) return;
  collapsed.value = !collapsed.value;
}
</script>

<style scoped>
.pm-today-section {
  background: var(--el-bg-color, #fff);
  border: 1px solid var(--pm-edge-soft, #e4e7ed);
  border-radius: 10px;
  padding: 12px 16px 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.pm-today-section-head {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  user-select: none;
}

.pm-today-section-icon {
  width: 20px;
  height: 20px;
  border-radius: 4px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: var(--pm-today-accent);
  color: #fff;
  font-size: 12px;
  line-height: 1;
  font-weight: 600;
}

.pm-today-section-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--el-text-color-primary, #303133);
}

.pm-today-section-count {
  font-size: 12px;
  color: var(--el-text-color-secondary, #909399);
  background: var(--el-fill-color-light, #f5f7fa);
  padding: 1px 8px;
  border-radius: 10px;
  min-width: 22px;
  text-align: center;
}

.pm-today-section-toggle {
  margin-left: auto;
  font-size: 12px;
  color: var(--el-text-color-secondary, #909399);
}

.pm-today-section-body {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.pm-today-section-empty {
  font-size: 12px;
  color: var(--el-text-color-placeholder, #a8abb2);
  padding: 12px 4px;
}

.pm-today-card-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
</style>

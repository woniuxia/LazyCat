<template>
  <div class="weekly-panel">
    <div class="weekly-header">
      <div class="header-left">
        <h3>最近 7 天工作</h3>
        <span class="window-hint">{{ windowHint }}</span>
      </div>
      <el-button size="small" @click="loadData" :loading="loading">刷新</el-button>
    </div>

    <div v-if="groupedData.length > 0" class="weekly-body">
      <div v-for="group in groupedData" :key="group.key" class="work-group">
        <div class="group-header">
          <span v-if="group.projectColor" class="project-dot" :style="{ backgroundColor: group.projectColor }" />
          <span class="group-name">{{ group.projectName }}</span>
          <el-tag v-if="group.projectArchived" size="small" type="info">已归档</el-tag>
          <span class="group-count">{{ group.items.length }} 项</span>
        </div>
        <div class="group-items">
          <div
            v-for="item in group.items"
            :key="`${item.source}-${item.id}`"
            class="work-item"
          >
            <div class="item-main">
              <el-tag size="small" :type="item.source === 'pm' ? '' : 'success'" effect="plain">
                {{ item.source === 'pm' ? 'PM' : 'Todo' }}
              </el-tag>
              <span class="item-title">{{ item.title }}</span>
            </div>
            <div class="item-meta">
              <el-tag v-if="item.itemType" size="small" type="info">{{ itemTypeLabel(item.itemType) }}</el-tag>
              <span class="item-priority" :style="{ color: priorityColor(item.priority) }">{{ item.priority }}</span>
              <span class="item-time">{{ formatTime(item.completedAt) }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div v-else-if="!loading" class="weekly-empty">
      <el-empty description="最近 7 天没有已完成的工作项" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { useToolInvoke } from "../composables/useToolInvoke";
import type { WeeklyWorkResult, WeeklyWorkItem } from "../types/pm";
import { PM_ITEM_TYPE_MAP, PM_PRIORITY_MAP } from "../types/pm";

const { invoke } = useToolInvoke();
const loading = ref(false);
const data = ref<WeeklyWorkResult | null>(null);

const windowHint = computed(() => {
  if (!data.value) return "";
  const start = new Date(data.value.windowStart).toLocaleDateString("zh-CN");
  const end = new Date(data.value.windowEnd).toLocaleDateString("zh-CN");
  return `${start} ~ ${end}`;
});

interface WorkGroup {
  key: string;
  projectName: string;
  projectColor: string | null;
  projectArchived: boolean;
  items: WeeklyWorkItem[];
  latestCompletedAt: string;
}

const groupedData = computed<WorkGroup[]>(() => {
  if (!data.value) return [];
  const allItems = [...data.value.pmItems, ...data.value.todoItems];
  const groups = new Map<string, WorkGroup>();

  for (const item of allItems) {
    const key = item.projectId ? `p-${item.projectId}` : "no-project";
    if (!groups.has(key)) {
      groups.set(key, {
        key,
        projectName: item.projectName ?? "未归项目",
        projectColor: item.projectColor ?? null,
        projectArchived: item.projectStatus === "archived",
        items: [],
        latestCompletedAt: item.completedAt ?? "",
      });
    }
    const group = groups.get(key)!;
    group.items.push(item);
    if (item.completedAt && item.completedAt > group.latestCompletedAt) {
      group.latestCompletedAt = item.completedAt;
    }
  }

  return Array.from(groups.values()).sort((a, b) =>
    b.latestCompletedAt.localeCompare(a.latestCompletedAt)
  );
});

async function loadData() {
  loading.value = true;
  try {
    data.value = (await invoke<WeeklyWorkResult>("tool:pm:weekly-work", {})) ?? null;
  } catch (e) {
    console.error(e);
  } finally {
    loading.value = false;
  }
}

function itemTypeLabel(type: string): string {
  return PM_ITEM_TYPE_MAP[type as keyof typeof PM_ITEM_TYPE_MAP]?.label ?? type;
}

function priorityColor(priority: string): string {
  return PM_PRIORITY_MAP[priority as keyof typeof PM_PRIORITY_MAP]?.color ?? "#909399";
}

function formatTime(dateStr: string | null): string {
  if (!dateStr) return "";
  const d = new Date(dateStr);
  return d.toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" });
}

onMounted(loadData);
</script>

<style scoped>
.weekly-panel {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.weekly-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  flex-shrink: 0;
}
.header-left {
  display: flex;
  align-items: baseline;
  gap: 12px;
}
.header-left h3 {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
}
.window-hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.weekly-body {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}
.work-group {
  margin-bottom: 20px;
}
.group-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--el-border-color-extra-light);
  margin-bottom: 8px;
}
.project-dot {
  width: 10px;
  height: 10px;
  border-radius: 2px;
  flex-shrink: 0;
}
.group-name {
  font-weight: 600;
  font-size: 14px;
}
.group-count {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-left: auto;
}
.work-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 8px;
  border-radius: 4px;
  transition: background 0.15s;
}
.work-item:hover {
  background: var(--el-fill-color-light);
}
.item-main {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.item-title {
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.item-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}
.item-priority {
  font-size: 12px;
  font-weight: 600;
}
.item-time {
  font-size: 11px;
  color: var(--el-text-color-secondary);
  min-width: 80px;
  text-align: right;
}
.weekly-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}
</style>

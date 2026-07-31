<template>
  <aside class="pm-sidebar">
    <div
      class="sidebar-today-card"
      :class="{ 'is-active': viewId === 'today' }"
      @click="emit('show-today')"
    >
      <div class="sidebar-today-head">
        <span class="sidebar-today-icon" aria-hidden="true">◷</span>
        <span class="sidebar-today-name">今日</span>
        <span class="sidebar-today-badge">{{ todayBadgeCount }}</span>
      </div>
    </div>

    <div
      class="sidebar-overview-card"
      :class="{ 'is-active': selectedProjectId === 'overview' }"
      @click="emit('select-project', 'overview')"
    >
      <div class="sidebar-overview-card-head">
        <div class="sidebar-overview-card-title">
          <span class="project-color overview-color" />
          <span class="project-name">总览</span>
        </div>
      </div>
      <div class="sidebar-overview-metrics">
        <div class="sidebar-overview-metric">
          <span class="sidebar-overview-metric-label">项目数</span>
          <strong class="sidebar-overview-metric-value">{{ projects.length }}</strong>
        </div>
        <div class="sidebar-overview-metric">
          <span class="sidebar-overview-metric-label">待办总数</span>
          <strong class="sidebar-overview-metric-value">{{ overviewUndoneCount }}</strong>
        </div>
      </div>
    </div>

    <div v-if="sidebarProjects.length > 0" class="sidebar-projects">
      <div class="sidebar-projects-head">
        <span class="sidebar-projects-title">全部项目</span>
        <el-button class="sidebar-create-btn" type="primary" @click="emit('create-project')">
          <el-icon><Plus /></el-icon>
          新建
        </el-button>
      </div>
      <div
        v-for="p in sidebarProjects"
        :key="p.id"
        class="project-card"
        :class="{
          'is-active': selectedProjectId === p.id,
          'is-drop-target': dropTargetProjectId === p.id,
          'is-archived': p.status === 'archived',
        }"
        @click="emit('select-project', p.id)"
        @contextmenu.prevent="emit('project-context', $event, p)"
        @dragover.prevent="emit('project-drag-over', p)"
        @dragleave="emit('project-drag-leave', p)"
        @drop.prevent="emit('project-drop', p)"
      >
        <div class="project-card-main">
          <span class="project-color" :style="{ backgroundColor: p.color }" />
          <span class="project-name">{{ p.name }}</span>
          <el-tag
            v-if="p.status === 'archived'"
            size="small"
            effect="plain"
            class="project-archived-tag"
            >已归档</el-tag
          >
          <span class="project-pending-badge">{{ p.pendingCount }}</span>
        </div>
      </div>
    </div>

    <div v-if="projects.length === 0" class="empty-hint">暂无项目，点击 + 创建</div>

    <div class="sidebar-footer">
      <button class="sidebar-footer-btn" @click="emit('open-settings')">
        <span class="sidebar-footer-btn-icon" aria-hidden="true">
          <el-icon><Setting /></el-icon>
        </span>
        <span class="sidebar-footer-btn-text">导入与设置</span>
      </button>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { Plus, Setting } from "@element-plus/icons-vue";
import type { PmProject } from "../../types/pm";
import type { ViewId } from "../../composables/pmViewRegistry";
import { sortPmProjectsForSidebar } from "../../utils/pmVisual";

const props = defineProps<{
  projects: PmProject[];
  projectItemCounts: Record<number, { total: number; done: number }>;
  selectedProjectId: number | "overview" | null;
  todayBadgeCount: number;
  dropTargetProjectId: number | null;
  viewId: ViewId;
}>();

const emit = defineEmits<{
  (e: "select-project", id: number | "overview"): void;
  (e: "show-today"): void;
  (e: "open-settings"): void;
  (e: "create-project"): void;
  (e: "project-context", event: MouseEvent, project: PmProject): void;
  (e: "project-drag-over", project: PmProject): void;
  (e: "project-drag-leave", project: PmProject): void;
  (e: "project-drop", project: PmProject): void;
}>();

const sidebarProjects = computed(() =>
  sortPmProjectsForSidebar(props.projects, props.projectItemCounts),
);
const overviewUndoneCount = computed(() => {
  let total = 0;
  for (const c of Object.values(props.projectItemCounts)) {
    total += c.total - c.done;
  }
  return total;
});
</script>

<style scoped>
/* Sidebar */
.pm-sidebar {
  display: flex;
  flex-direction: column;
  width: 200px;
  min-width: 200px;
  padding: 12px 0;
  overflow-y: auto;
  background: var(--el-bg-color);
}
.sidebar-footer {
  margin-top: auto;
  padding: 10px 10px 4px;
  border-top: 1px solid var(--pm-edge);
}

.sidebar-footer-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  width: 100%;
  padding: 10px 14px;
  border: 1px solid var(--pm-edge-soft);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.72);
  color: var(--pm-text-muted);
  font-size: 13px;
  font-family: inherit;
  cursor: pointer;
  transition:
    border-color 0.18s ease,
    background 0.18s ease,
    color 0.18s ease,
    box-shadow 0.18s ease,
    transform 0.18s ease;
}

.sidebar-footer-btn:hover {
  border-color: rgba(14, 165, 233, 0.28);
  background: rgba(255, 255, 255, 0.95);
  color: var(--pm-accent);
  transform: translateY(-1px);
}

.sidebar-footer-btn:active {
  transform: translateY(0);
  box-shadow: none;
}

.sidebar-footer-btn-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 8px;
  background: transparent;
  color: var(--pm-accent);
  font-size: 15px;
  flex-shrink: 0;
  transition: background 0.18s ease;
}

.sidebar-footer-btn:hover .sidebar-footer-btn-icon {
  background: transparent;
}

.sidebar-footer-btn-text {
  font-weight: 500;
  white-space: nowrap;
}
.project-color {
  width: 12px;
  height: 12px;
  border-radius: 2px;
  flex-shrink: 0;
}
.project-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  min-width: 0;
}
.empty-hint {
  padding: 24px 12px;
  color: var(--el-text-color-secondary);
  font-size: 14px;
  text-align: center;
}
.overview-color {
  background: linear-gradient(135deg, #409eff, #67c23a, #e6a23c);
}

/* PM visual unification */
.pm-sidebar {
  width: 248px;
  min-width: 248px;
  padding: 16px 14px 18px;
  border: 1px solid var(--pm-edge);
  border-radius: var(--lc-radius-lg);
  background: var(--lc-surface-0);
}

.sidebar-create-btn {
  min-height: 28px;
  padding-inline: 10px;
  border-radius: 10px;
  box-shadow: 0 8px 16px rgba(14, 165, 233, 0.2);
  opacity: 0;
  visibility: hidden;
  pointer-events: none;
  transform: translateY(-4px);
  transition:
    opacity 0.18s ease,
    visibility 0.18s ease,
    transform 0.18s ease,
    box-shadow 0.18s ease;
  font-size: 12px;
}

.sidebar-projects-head:hover .sidebar-create-btn,
.sidebar-projects-head:focus-within .sidebar-create-btn {
  opacity: 1;
  visibility: visible;
  pointer-events: auto;
  transform: translateY(0);
}

.sidebar-today-card {
  margin-bottom: 12px;
  padding: 12px 14px;
  border: 1px solid var(--pm-edge);
  border-radius: 14px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.96), rgba(248, 251, 255, 0.92));
  box-shadow: var(--pm-shadow-soft);
  cursor: pointer;
  transition:
    border-color 0.18s ease,
    box-shadow 0.18s ease,
    transform 0.18s ease,
    background 0.18s ease;
}

.sidebar-today-card:hover,
.sidebar-today-card.is-active {
  border-color: rgba(14, 165, 233, 0.45);
  box-shadow: 0 14px 24px rgba(14, 165, 233, 0.16);
  transform: translateY(-1px);
}

.sidebar-today-card.is-active {
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(229, 240, 255, 0.95));
}

.sidebar-today-head {
  display: flex;
  align-items: center;
  gap: 10px;
}

.sidebar-today-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 10px;
  background: rgba(14, 165, 233, 0.12);
  color: var(--pm-accent);
  font-size: 16px;
  font-family: "Segoe UI Symbol", "Apple Symbols", sans-serif;
}

.sidebar-today-name {
  flex: 1;
  font-size: 14px;
  font-weight: 600;
  color: var(--pm-text-main);
}

.sidebar-today-badge {
  min-width: 24px;
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--pm-accent);
  color: #ffffff;
  font-size: 12px;
  font-weight: 600;
  text-align: center;
  line-height: 18px;
}

.sidebar-overview-card {
  margin-bottom: 16px;
  padding: 14px;
  border: 1px solid var(--pm-edge);
  border-radius: 16px;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.98), rgba(240, 246, 255, 0.94));
  box-shadow: var(--pm-shadow-soft);
  cursor: pointer;
  transition:
    border-color 0.18s ease,
    box-shadow 0.18s ease,
    transform 0.18s ease,
    background 0.18s ease;
}

.sidebar-overview-card:hover,
.sidebar-overview-card.is-active {
  border-color: rgba(14, 165, 233, 0.35);
  box-shadow: 0 16px 28px rgba(14, 165, 233, 0.14);
  transform: translateY(-1px);
}

.sidebar-overview-card.is-active {
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(226, 237, 255, 0.95));
}

.sidebar-overview-card-head,
.sidebar-projects-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.sidebar-overview-card-head {
  margin-bottom: 12px;
  align-items: flex-start;
}

.sidebar-overview-card-title {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.sidebar-overview-metrics {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.sidebar-overview-metric {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px 12px;
  border: 1px solid rgba(255, 255, 255, 0.95);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.78);
}

.sidebar-overview-metric-label {
  font-size: 12px;
  color: var(--pm-text-muted);
}

.sidebar-overview-metric-value {
  font-size: 16px;
  font-weight: 700;
  color: var(--pm-text-main);
}

.sidebar-projects {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.sidebar-projects-head {
  padding: 0 2px;
  position: relative;
}

.sidebar-projects-title {
  font-size: 13px;
  font-weight: 700;
  color: var(--pm-text-main);
}

.project-card {
  padding: 12px 14px;
  border: 1px solid var(--pm-edge-soft);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.82);
  box-shadow: 0 8px 18px rgba(34, 48, 66, 0.04);
  cursor: pointer;
  transition:
    border-color 0.18s ease,
    box-shadow 0.18s ease,
    transform 0.18s ease,
    background 0.18s ease;
}

.project-card:hover {
  border-color: rgba(14, 165, 233, 0.25);
  background: rgba(255, 255, 255, 0.98);
  box-shadow: 0 14px 24px rgba(34, 48, 66, 0.08);
  transform: translateY(-1px);
}

.project-card.is-active {
  border-color: rgba(14, 165, 233, 0.36);
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(232, 240, 255, 0.94));
  box-shadow: 0 16px 28px rgba(14, 165, 233, 0.14);
}

.project-card.is-archived {
  opacity: 1;
}

.project-card.is-drop-target {
  border-color: rgba(14, 165, 233, 0.45);
  background: linear-gradient(180deg, rgba(255, 255, 255, 1), rgba(223, 234, 255, 0.96));
  box-shadow:
    0 0 0 2px rgba(14, 165, 233, 0.16),
    0 18px 30px rgba(14, 165, 233, 0.16);
  animation: none;
}

.project-card-main {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.project-pending-badge {
  display: inline-flex;
  align-items: center;
  margin-left: auto;
  min-width: 24px;
  justify-content: center;
  padding: 4px 8px;
  border-radius: 999px;
  background: rgba(14, 165, 233, 0.1);
  color: var(--pm-accent);
  font-size: 12px;
  font-weight: 700;
  line-height: 1;
}

.project-card .project-color,
.sidebar-overview-card .project-color {
  width: 10px;
  height: 10px;
  border-radius: 999px;
}

.project-card .project-name,
.sidebar-overview-card .project-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--pm-text-main);
}

.overview-color {
  background: linear-gradient(135deg, var(--lc-accent), #73a0ff 46%, #88c9ff 100%);
}

.empty-hint {
  margin-top: 20px;
  padding: 18px 12px;
  border: 1px dashed var(--pm-edge);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.6);
  color: var(--pm-text-muted);
}
</style>
